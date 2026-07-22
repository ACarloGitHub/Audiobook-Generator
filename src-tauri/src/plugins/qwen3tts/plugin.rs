use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use async_trait::async_trait;
use tracing::{info, warn};

use crate::base_plugin::{BaseTTSPlugin, EngineHandle, SynthesizeRequest};
use crate::chunker;
use crate::merger;
use crate::recovery;
use crate::plugin_manager::QwenPaths;

pub struct QwenPlugin {
    pub paths: QwenPaths,
    pub variant_name: String,
    inner: Mutex<Option<()>>,
    current: Mutex<Option<String>>,
}

impl QwenPlugin {
    pub fn new(paths: QwenPaths, variant_name: &str) -> Self {
        Self {
            paths,
            variant_name: variant_name.to_string(),
            inner: Mutex::new(None),
            current: Mutex::new(None),
        }
    }

    fn resolve_talker_gguf(&self) -> Result<PathBuf> {
        let variant_dir = self.paths.models_dir.join(&self.variant_name);
        for name in ["talker-Q4_K_M.gguf", "talker-Q8_0.gguf", "talker-BF16.gguf"] {
            let p = variant_dir.join(name);
            if p.exists() {
                return Ok(p);
            }
        }
        anyhow::bail!(
            "no Qwen3-TTS talker GGUF found in {}. Download from the Models panel.",
            variant_dir.display()
        );
    }

    fn resolve_tokenizer_gguf(&self) -> Result<PathBuf> {
        for name in ["tokenizer-Q4_K_M.gguf", "tokenizer-Q8_0.gguf", "tokenizer-BF16.gguf"] {
            let p = self.paths.tokenizer_dir.join(name);
            if p.exists() {
                return Ok(p);
            }
        }
        anyhow::bail!(
            "no Qwen3-TTS tokenizer GGUF found in {}. Download from the Models panel.",
            self.paths.tokenizer_dir.display()
        );
    }

    fn find_qwen_tts_binary() -> Result<PathBuf> {
        let exe_name = if cfg!(windows) { "qwen-tts.exe" } else { "qwen-tts" };
        if let Some(p) = crate::sidecars::sidecar_binary("qwentts", exe_name) {
            return Ok(p);
        }
        anyhow::bail!(
            "qwen-tts binary not found. It ships inside the installer; reinstall the app or, for development, place it in resources/qwentts/."
        )
    }

    /// Get the directory containing the qwen-tts binary (for DLL resolution).
    fn binary_dir() -> Result<PathBuf> {
        Ok(Self::find_qwen_tts_binary()?
            .parent()
            .ok_or_else(|| anyhow::anyhow!("cannot get parent of qwen-tts binary"))?
            .to_path_buf())
    }

    /// Get the shared CUDA runtime DLL directory (single copy used by all engines).
    fn cuda_shared_dir() -> Option<PathBuf> {
        crate::sidecars::sidecar_dir("cuda-shared")
    }

    fn parse_variant_mode(&self) -> &'static str {
        if self.variant_name.contains("CustomVoice") {
            "customvoice"
        } else if self.variant_name.contains("VoiceDesign") {
            "voicedesign"
        } else {
            "base"
        }
    }

    fn build_command(
        &self,
        talker: &Path,
        tokenizer: &Path,
        request: &SynthesizeRequest,
        output_path: &Path,
    ) -> Result<std::process::Command> {
        let binary = Self::find_qwen_tts_binary()?;
        // GPU-only rule: refuse to run when no GPU backend is visible
        // (never fall back to CPU silently).
        crate::gpu_guard::ensure_gpu()?;
        let mut cmd = std::process::Command::new(&binary);
        crate::utils::hide_console_window(&mut cmd);

        // Ensure the qwen-tts binary dir and the shared CUDA runtime dir
        // are visible to the loader (PATH on Windows, LD_LIBRARY_PATH on
        // Linux, DYLD_LIBRARY_PATH on macOS).
        let mut path_dirs = vec![Self::binary_dir()?];
        if let Some(cuda_dir) = Self::cuda_shared_dir() {
            path_dirs.push(cuda_dir);
        }
        crate::sidecars::apply_loader_path(&mut cmd, &path_dirs);

        cmd.arg("--model").arg(talker);
        cmd.arg("--codec").arg(tokenizer);
        cmd.arg("-o").arg(output_path);

        // Language
        let lang = request.language.as_deref().unwrap_or("auto");
        cmd.arg("--lang").arg(lang);

        // Mode-specific args
        let mode = self.parse_variant_mode();
        match mode {
            "customvoice" => {
                if let Some(speaker) = &request.voice {
                    cmd.arg("--speaker").arg(speaker);
                }
                if let Some(instruct) = request.extra.get("instruct") {
                    if !instruct.is_empty() {
                        cmd.arg("--instruct").arg(instruct);
                    }
                }
            }
            "voicedesign" => {
                if let Some(instruct) = request.extra.get("instruct") {
                    cmd.arg("--instruct").arg(instruct);
                }
            }
            "base" => {
                if let Some(ref_wav) = request.reference_audio.as_ref() {
                    cmd.arg("--ref-wav").arg(ref_wav);
                    if let Some(ref_text) = request.extra.get("ref_text") {
                        // Write ref_text to a temp file because --ref-text expects a path
                        let temp = std::env::temp_dir().join(format!("qwen_ref_text_{}.txt", std::process::id()));
                        std::fs::write(&temp, ref_text)?;
                        cmd.arg("--ref-text").arg(&temp);
                    }
                }
            }
            _ => {}
        }

        // Sampling parameters (from extra)
        if let Some(v) = request.extra.get("temp") {
            cmd.arg("--temp").arg(v);
        }
        if let Some(v) = request.extra.get("top_k") {
            cmd.arg("--top-k").arg(v);
        }
        if let Some(v) = request.extra.get("top_p") {
            cmd.arg("--top-p").arg(v);
        }
        if let Some(v) = request.extra.get("rep_pen") {
            cmd.arg("--rep-pen").arg(v);
        }
        if let Some(v) = request.extra.get("seed") {
            if v != "-1" {
                cmd.arg("--seed").arg(v);
            }
        }
        if let Some(v) = request.extra.get("max_new") {
            cmd.arg("--max-new").arg(v);
        }

        // Text goes to stdin
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        Ok(cmd)
    }
}

pub fn synthesize_book(
    plugin: &QwenPlugin,
    epub_path: &Path,
    output_dir: &Path,
    max_words: usize,
    max_chars: usize,
    ffmpeg: &Path,
    voice: Option<&str>,
    language: Option<&str>,
    reference_audio: Option<&str>,
    extra: &std::collections::HashMap<String, String>,
    mut progress: Option<Box<dyn FnMut(&str) + Send>>,
) -> Result<usize> {
    if let Some(cb) = progress.as_deref_mut() {
        cb("Reading EPUB...");
    }
    let chapters = crate::input::extract_chapters_from(epub_path)?;
    let total_chapters = chapters.len();
    std::fs::create_dir_all(output_dir)?;
    let recovery_path = output_dir.join("failed_chunks.json");

    if let Some(cb) = progress.as_deref_mut() {
        cb(&format!("Extracted {total_chapters} chapters"));
    }

    let mut recovery_state = recovery::RecoveryState::load(output_dir).unwrap_or_default();
    recovery_state.set_meta(recovery::RecoveryMeta {
        engine_id: Some(plugin.variant_name.clone()),
        reference_audio: reference_audio.map(|s| s.to_string()),
        voice: voice.map(|s| s.to_string()),
        language: language.map(|s| s.to_string()),
        extra: extra.clone(),
        generated_at: Some(recovery::now_stamp()),
    });
    // Persist immediately so the retry commands can resolve the engine even
    // if the very first chapter fails or is stopped.
    let _ = recovery_state.save(output_dir);
    let mut done_count = 0usize;
    let mut failed_count = 0usize;

    for (i, chapter) in chapters.iter().enumerate() {
        if crate::commands::is_stop_requested() {
            if let Some(cb) = progress.as_deref_mut() {
                cb("STOP requested — saving recovery state and exiting.");
            }
            break;
        }

        let chapter_dir = output_dir.join(crate::utils::sanitize_filename(&chapter.title));
        std::fs::create_dir_all(&chapter_dir)?;

        let chunks = chunker::chunk_text(&chapter.text, max_words, max_chars);
        if let Some(cb) = progress.as_deref_mut() {
            cb(&format!(
                "Chapter {}/{}: {} ({} chunks)",
                i + 1,
                total_chapters,
                chapter.title,
                chunks.len()
            ));
        }

        let mut wavs = Vec::with_capacity(chunks.len());
        for (j, chunk_text) in chunks.iter().enumerate() {
            if crate::commands::is_stop_requested() {
                if let Some(cb) = progress.as_deref_mut() {
                    cb("STOP requested — saving recovery state and exiting.");
                }
                break;
            }
            let wav_path = chapter_dir.join(format!("chunk_{:04}.wav", j + 1));
            if recovery_state.is_done(&chapter.title, j) && wav_path.exists() {
                wavs.push(wav_path);
                continue;
            }

            let request = SynthesizeRequest {
                text: chunk_text.clone(),
                output_path: wav_path.to_string_lossy().to_string(),
                reference_audio: reference_audio.map(|s| s.to_string()),
                language: language.map(|s| s.to_string()),
                voice: voice.map(|s| s.to_string()),
                extra: extra.clone(),
            };

            match synthesize_chunk(plugin, &request) {
                Ok(()) => {
                    recovery_state.mark_done(&chapter.title, j);
                    wavs.push(wav_path);
                    done_count += 1;
                }
                Err(e) => {
                    failed_count += 1;
                    warn!("chunk {}/{} failed: {e:#}", j + 1, chunks.len());
                    if let Some(cb) = progress.as_deref_mut() {
                        cb(&format!(
                            "WARN: chunk {}/{} failed: {}",
                            j + 1,
                            chunks.len(),
                            e.to_string().lines().next().unwrap_or(&e.to_string())
                        ));
                    }
                    recovery_state.mark_failed(&chapter.title, j, chunk_text, &format!("{e:#}"));
                }
            }
        }

        if !wavs.is_empty() {
            let mp3_path = output_dir.join(format!(
                "{}.mp3",
                crate::utils::sanitize_filename(&chapter.title)
            ));
            if let Err(e) = merger::merge_wavs_to_mp3(&wavs, &mp3_path, ffmpeg) {
                warn!("merge failed for {}: {e:#}", chapter.title);
                if let Some(cb) = progress.as_deref_mut() {
                    cb(&format!("ERROR: merge failed for {}: {}", chapter.title, e));
                }
            }
        }

        let _ = std::fs::write(
            &recovery_path,
            serde_json::to_string_pretty(&recovery_state).unwrap_or_else(|_| "{}".to_string()),
        );
        if let Some(cb) = progress.as_deref_mut() {
            cb(&format!("Chapter {}/{} done", i + 1, total_chapters));
        }
    }

    if let Some(cb) = progress.as_deref_mut() {
        cb(&format!(
            "Done: {done_count} chunks synthesized, {failed_count} failed across {total_chapters} chapters"
        ));
    }
    Ok(done_count)
}

fn synthesize_chunk(plugin: &QwenPlugin, request: &SynthesizeRequest) -> Result<()> {
    let output_path = Path::new(&request.output_path);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let talker = plugin.resolve_talker_gguf()?;
    let tokenizer = plugin.resolve_tokenizer_gguf()?;

    let mut cmd = plugin.build_command(&talker, &tokenizer, request, output_path)?;

    info!(
        "qwen-tts command: {:?} (variant: {})",
        cmd,
        plugin.variant_name
    );

    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn qwen-tts: {}", QwenPlugin::find_qwen_tts_binary().map(|p| p.display().to_string()).unwrap_or_default()))?;

    // Write text to stdin
    if let Some(stdin) = child.stdin.take() {
        use std::io::Write;
        let mut stdin = stdin;
        stdin.write_all(request.text.as_bytes())
            .with_context(|| "failed to write text to qwen-tts stdin")?;
    }

    let output = child.wait_with_output()
        .with_context(|| "failed to wait for qwen-tts")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "qwen-tts exited with status {}: {}",
            output.status,
            stderr.lines().last().unwrap_or(&stderr.to_string())
        );
    }

    if !output_path.exists() {
        anyhow::bail!("qwen-tts exited successfully but output file {} was not created", output_path.display());
    }

    Ok(())
}

#[async_trait]
impl BaseTTSPlugin for QwenPlugin {
    fn name(&self) -> &str {
        &self.variant_name
    }

    fn plugin_type(&self) -> &str {
        "external_process"
    }

    fn is_installed(&self) -> bool {
        let talker_ok = self.resolve_talker_gguf().is_ok();
        let tokenizer_ok = self.resolve_tokenizer_gguf().is_ok();
        talker_ok && tokenizer_ok
    }

    async fn load_model(&self, model_id: &str) -> Result<EngineHandle> {
        let talker = self.resolve_talker_gguf()?;
        let tokenizer = self.resolve_tokenizer_gguf()?;
        info!("loading Qwen3-TTS: {}", self.variant_name);
        info!("talker: {}", talker.display());
        info!("tokenizer: {}", tokenizer.display());

        // Verify the qwen-tts binary exists
        Self::find_qwen_tts_binary()
            .with_context(|| "qwen-tts binary not found")?;

        *self.inner.lock().unwrap() = Some(());
        *self.current.lock().unwrap() = Some(model_id.to_string());

        Ok(EngineHandle {
            engine_id: self.variant_name.clone(),
            model_id: model_id.to_string(),
        })
    }

    async fn synthesize(&self, _handle: &EngineHandle, request: &SynthesizeRequest) -> Result<()> {
        let output_path = Path::new(&request.output_path).to_path_buf();
        let text = request.text.clone();

        let talker = self.resolve_talker_gguf()?;
        let tokenizer = self.resolve_tokenizer_gguf()?;
        let mut cmd = self.build_command(&talker, &tokenizer, request, &output_path)?;

        info!("qwen-tts command: {:?}", cmd);

        let mut child = cmd.spawn()
            .with_context(|| "failed to spawn qwen-tts")?;

        if let Some(stdin) = child.stdin.take() {
            use std::io::Write;
            let mut stdin = stdin;
            stdin.write_all(text.as_bytes())?;
        }

        let output = child.wait_with_output()
            .with_context(|| "failed to wait for qwen-tts")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "qwen-tts exited with status {}: {}",
                output.status,
                stderr.lines().last().unwrap_or(&stderr.to_string())
            );
        }

        if !output_path.exists() {
            anyhow::bail!("qwen-tts exited successfully but output file was not created");
        }

        Ok(())
    }

    async fn unload(&self, _handle: &EngineHandle) -> Result<()> {
        *self.inner.lock().unwrap() = None;
        *self.current.lock().unwrap() = None;
        info!("Qwen3-TTS engine unloaded");
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}