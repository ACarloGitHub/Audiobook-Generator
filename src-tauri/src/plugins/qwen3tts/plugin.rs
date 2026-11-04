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

/// Boolean engine flags are carried as strings in `extra`; accept the usual
/// spellings so the CLI, the GUI and the MCP server agree.
fn extra_flag(extra: &std::collections::HashMap<String, String>, key: &str) -> bool {
    extra
        .get(key)
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on"))
        .unwrap_or(false)
}

/// Canonical `--lang` label accepted by qwen-tts. The runtime knows exactly
/// these labels (plus `auto`); anything else makes it exit, so unknown
/// labels fall back to autodetect.
fn qwen_language(label: Option<&str>) -> &'static str {
    match label.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("italian") => "Italian",
        Some("english") => "English",
        Some("french") => "French",
        Some("german") => "German",
        Some("spanish") => "Spanish",
        Some("portuguese") => "Portuguese",
        Some("russian") => "Russian",
        Some("japanese") => "Japanese",
        Some("korean") => "Korean",
        Some("chinese") => "Chinese",
        _ => "auto",
    }
}

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

        // Language. qwen-tts matches the label against the GGUF language
        // table and exits with "unknown language" on a label it does not
        // know; an unknown label therefore degrades to autodetect instead of
        // failing the chunk.
        cmd.arg("--lang").arg(qwen_language(request.language.as_deref()));

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
        // Sub-talker (Code Predictor) sampling: separate stack, official
        // defaults 0.9 / 50 / 1.0.
        if let Some(v) = request.extra.get("sub_temp") {
            cmd.arg("--sub-temp").arg(v);
        }
        if let Some(v) = request.extra.get("sub_top_k") {
            cmd.arg("--sub-top-k").arg(v);
        }
        if let Some(v) = request.extra.get("sub_top_p") {
            cmd.arg("--sub-top-p").arg(v);
        }
        // Codec decode window: a chunk longer than this is decoded in
        // several passes (each seam leaves a small residual), and the window
        // is also a VRAM knob.
        if let Some(v) = request.extra.get("codec_chunk_dur") {
            cmd.arg("--codec-chunk-dur").arg(v);
        }
        if let Some(v) = request.extra.get("codec_left_dur") {
            cmd.arg("--codec-left-dur").arg(v);
        }
        if extra_flag(&request.extra, "greedy") {
            cmd.arg("--greedy");
        }
        if extra_flag(&request.extra, "no_fa") {
            cmd.arg("--no-fa");
        }
        if extra_flag(&request.extra, "clamp_fp16") {
            cmd.arg("--clamp-fp16");
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
    ffmpeg: Option<&Path>,
    voice: Option<&str>,
    language: Option<&str>,
    reference_audio: Option<&str>,
    only: Option<&[String]>,
    extra: &std::collections::HashMap<String, String>,
    mut progress: Option<Box<dyn FnMut(&str) + Send>>,
) -> Result<usize> {
    if let Some(cb) = progress.as_deref_mut() {
        cb("Reading EPUB...");
    }
    if ffmpeg.is_none() {
        if let Some(cb) = progress.as_deref_mut() {
            cb("ffmpeg not found — chapters will be saved as WAV with the fallback merger.");
        }
    }
    let chapters = crate::input::extract_chapters_from(epub_path)?;
    // Keep only the chapters selected in the Generate panel (None/empty = all).
    let chapters: Vec<_> = match only {
        Some(titles) if !titles.is_empty() => chapters
            .into_iter()
            .filter(|c| titles.iter().any(|t| t == &c.title))
            .collect(),
        _ => chapters,
    };
    if chapters.is_empty() {
        anyhow::bail!("no chapters left after applying the chapter selection");
    }
    let total_chapters = chapters.len();
    // Reference preflight: Qwen clones with the x-vector of the clip and, when
    // a transcript is given, switches to ICL (reference text + codes). A
    // mismatched transcript or a clip with long pauses degrades every chunk.
    if let Some(path) = reference_audio {
        if let Some(info) = crate::audio_check::reference_info(Path::new(path)) {
            let mut messages: Vec<String> = Vec::new();
            if info.duration_seconds < 3.0 || info.duration_seconds > 20.0 {
                messages.push(format!(
                    "reference is {:.1}s: Qwen voice cloning is designed for 3-20s clips",
                    info.duration_seconds
                ));
            }
            if info.longest_silence_seconds > 1.5 {
                messages.push(format!(
                    "reference contains a {:.1}s pause: the model imitates the sample's delivery",
                    info.longest_silence_seconds
                ));
            }
            if let Some(text) = extra.get("ref_text") {
                let words = text.split_whitespace().count();
                if words > 0 {
                    let rate = words as f64 / info.duration_seconds.max(0.1);
                    if !(0.5..=6.0).contains(&rate) {
                        messages.push(format!(
                            "reference transcript looks mismatched: {words} words in {:.1}s ({rate:.1} words/s); the transcript is used verbatim in ICL mode, so it must be exact",
                            info.duration_seconds
                        ));
                    }
                }
            }
            for message in messages {
                warn!("[qwen3tts] {message}");
                if let Some(cb) = progress.as_deref_mut() {
                    cb(&format!("WARN: {message}"));
                }
            }
        }
    }
    std::fs::create_dir_all(output_dir)?;
    let recovery_path = output_dir.join("failed_chunks.json");

    if let Some(cb) = progress.as_deref_mut() {
        cb(&format!("Extracted {total_chapters} chapters"));
    }

    let mut recovery_state = recovery::RecoveryState::load(output_dir).unwrap_or_default();

    let chunking_changed = recovery_state.reset_if_chunking_changed();
    recovery_state.set_meta(recovery::RecoveryMeta {
        engine_id: Some(plugin.variant_name.clone()),
        reference_audio: reference_audio.map(|s| s.to_string()),
        voice: voice.map(|s| s.to_string()),
        language: language.map(|s| s.to_string()),
        extra: extra.clone(),
        generated_at: Some(recovery::now_stamp()),
        chunker_version: recovery::CURRENT_CHUNKER_VERSION,
    });
    // Persist immediately so the retry commands can resolve the engine even
    // if the very first chapter fails or is stopped.
    let _ = recovery_state.save(output_dir);
    if chunking_changed {
        if let Some(cb) = progress.as_deref_mut() {
            cb("Text extraction/chunking changed: previous chunk records were discarded, the book will be re-synthesized.");
        }
    }
    let mut done_count = 0usize;
    let mut failed_count = 0usize;
    let mut rate_guard = crate::audio_check::RateGuard::new();

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
                if let Some(secs) = crate::audio_check::wav_duration_seconds(&wav_path) {
                    let _ = rate_guard.observe(chunk_text.chars().count(), secs);
                }
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
                    let suspicion = crate::audio_check::wav_duration_seconds(&wav_path)
                        .and_then(|secs| {
                            rate_guard.observe(chunk_text.chars().count(), secs)
                        });
                    if let Some(reason) = suspicion {
                        failed_count += 1;
                        warn!("chunk {}/{} looks incomplete: {reason}", j + 1, chunks.len());
                        if let Some(cb) = progress.as_deref_mut() {
                            cb(&format!(
                                "WARN: chunk {}/{} looks incomplete ({reason}). Retry it in Error Recovery.",
                                j + 1,
                                chunks.len()
                            ));
                        }
                        recovery_state.mark_failed(&chapter.title, j, chunk_text, &reason);
                    } else {
                        recovery_state.mark_done(&chapter.title, j);
                        wavs.push(wav_path);
                        done_count += 1;
                    }
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

        let chapter_failed = recovery_state
            .failed
            .get(&chapter.title)
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        if chapter_failed {
            let n = recovery_state
                .failed
                .get(&chapter.title)
                .map(|v| v.len())
                .unwrap_or(0);
            if let Some(cb) = progress.as_deref_mut() {
                cb(&format!(
                    "Merge skipped for '{}': {n} chunk(s) failed. Retry them in Error Recovery, then use Merge All Chunks.",
                    chapter.title
                ));
            }
        } else if !wavs.is_empty() {
            let base = crate::utils::sanitize_filename(&chapter.title);
            match merger::merge_chapter(&wavs, output_dir, &base, ffmpeg) {
                Ok(path) => {
                    if let Some(cb) = progress.as_deref_mut() {
                        cb(&format!(
                            "Merged {} chunk(s) into {}",
                            wavs.len(),
                            path.display()
                        ));
                    }
                }
                Err(e) => {
                    warn!("merge failed for {}: {e:#}", chapter.title);
                    if let Some(cb) = progress.as_deref_mut() {
                        cb(&format!("ERROR: merge failed for {}: {}", chapter.title, e));
                    }
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
    crate::job_object::assign_child(&child);
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
        anyhow::bail!(
            "qwen-tts exited with status {}: {}",
            output.status,
            crate::utils::process_error_detail(&output.stdout, &output.stderr)
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
        crate::job_object::assign_child(&child);
        if let Some(stdin) = child.stdin.take() {
            use std::io::Write;
            let mut stdin = stdin;
            stdin.write_all(text.as_bytes())?;
        }

        let output = child.wait_with_output()
            .with_context(|| "failed to wait for qwen-tts")?;

        if !output.status.success() {
            anyhow::bail!(
                "qwen-tts exited with status {}: {}",
                output.status,
                crate::utils::process_error_detail(&output.stdout, &output.stderr)
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_language_labels_and_falls_back_to_auto() {
        assert_eq!(qwen_language(Some("Italian")), "Italian");
        assert_eq!(qwen_language(Some("italian")), "Italian");
        assert_eq!(qwen_language(Some("Auto")), "auto");
        assert_eq!(qwen_language(None), "auto");
        // A label the GGUF does not declare must not reach the runtime.
        assert_eq!(qwen_language(Some("Dutch")), "auto");
    }

    #[test]
    fn parses_boolean_flags() {
        let mut extra = std::collections::HashMap::new();
        assert!(!extra_flag(&extra, "greedy"));
        extra.insert("greedy".to_string(), "true".to_string());
        assert!(extra_flag(&extra, "greedy"));
        extra.insert("greedy".to_string(), "off".to_string());
        assert!(!extra_flag(&extra, "greedy"));
    }
}
