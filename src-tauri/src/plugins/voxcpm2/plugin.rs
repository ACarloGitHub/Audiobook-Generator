use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use async_trait::async_trait;
use tracing::{info, warn};

use crate::base_plugin::{BaseTTSPlugin, EngineHandle, SynthesizeRequest};
use crate::chunker;
use crate::merger;
use crate::plugin_manager::VoxCpm2Paths;
use crate::recovery;

pub struct VoxCpm2Plugin {
    pub paths: VoxCpm2Paths,
    pub variant_name: String,
    inner: Mutex<Option<()>>,
    current: Mutex<Option<String>>,
}

impl VoxCpm2Plugin {
    pub fn new(paths: VoxCpm2Paths, variant_name: &str) -> Self {
        Self {
            paths,
            variant_name: variant_name.to_string(),
            inner: Mutex::new(None),
            current: Mutex::new(None),
        }
    }

    fn resolve_base_lm_gguf(&self) -> Result<PathBuf> {
        // The engine id IS the quant selection (e.g. "VoxCPM2 Q8_0" or
        // "VoxCPM2 F16", as chosen in Configuration / the Models panel).
        // No automatic fallback to another quant: if the chosen file is
        // missing, the user must download it explicitly.
        let quant_file = crate::plugin_manager::voxcpm2_quant_for_engine(&self.variant_name)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "unknown VoxCPM2 engine id '{}'. Expected e.g. 'VoxCPM2 Q8_0'.",
                    self.variant_name
                )
            })?;
        let p = self
            .paths
            .models_dir
            .join(&quant_file.base_name)
            .join(&quant_file.filename);
        if p.exists() {
            return Ok(p);
        }
        anyhow::bail!(
            "{} is not downloaded (missing {}). Download it from the Models panel.",
            self.variant_name,
            p.display()
        );
    }

    fn resolve_acoustic_gguf(&self) -> Result<PathBuf> {
        let p = self
            .paths
            .acoustic_dir
            .join("VoxCPM2-Acoustic-F16.gguf");
        if p.exists() {
            return Ok(p);
        }
        anyhow::bail!(
            "no VoxCPM2 Acoustic GGUF found in {}. Download from the Models panel.",
            self.paths.acoustic_dir.display()
        );
    }

    fn find_voxcpm2_binary() -> Result<PathBuf> {
        let exe_name = if cfg!(windows) {
            "voxcpm2-cli.exe"
        } else {
            "voxcpm2-cli"
        };
        if let Some(p) = crate::sidecars::sidecar_binary("voxcpm2", exe_name) {
            return Ok(p);
        }
        anyhow::bail!(
            "voxcpm2-cli binary not found. It ships inside the installer; reinstall the app or, for development, place it in resources/voxcpm2/."
        )
    }

    /// Get the directory containing the voxcpm2-cli binary (for DLL resolution).
    fn binary_dir() -> Result<PathBuf> {
        Ok(Self::find_voxcpm2_binary()?
            .parent()
            .ok_or_else(|| anyhow::anyhow!("cannot get parent of voxcpm2-cli binary"))?
            .to_path_buf())
    }

    /// Get the shared CUDA runtime DLL directory (single copy used by all engines).
    fn cuda_shared_dir() -> Option<PathBuf> {
        crate::sidecars::sidecar_dir("cuda-shared")
    }

    /// Re-encode a UTF-8 string so it survives the Windows ANSI (CP1252)
    /// argv conversion performed by the MSVC CRT in the child process.
    ///
    /// voxcpm2-cli uses plain `main(argc, argv)`: on Windows the CRT converts
    /// the UTF-16 command line to the system ANSI code page (CP1252), but the
    /// tokenizer expects UTF-8, so accented characters would be corrupted.
    /// We map every UTF-8 byte to the Unicode character whose CP1252 encoding
    /// is exactly that byte, so the CRT conversion reconstructs the original
    /// UTF-8 byte sequence. Bytes 0x81/0x8D/0x8F/0x90/0x9D (undefined in
    /// CP1252) map to the C1 control characters, which Windows round-trips.
    /// Verified end-to-end on 2026-07-19 with the compiled CLI.
    #[cfg(windows)]
    fn to_ansi_argv(text: &str) -> String {
        text.bytes()
            .map(|b| match b {
                0x80 => '\u{20AC}',
                0x82 => '\u{201A}',
                0x83 => '\u{0192}',
                0x84 => '\u{201E}',
                0x85 => '\u{2026}',
                0x86 => '\u{2020}',
                0x87 => '\u{2021}',
                0x88 => '\u{02C6}',
                0x89 => '\u{2030}',
                0x8A => '\u{0160}',
                0x8B => '\u{2039}',
                0x8C => '\u{0152}',
                0x8E => '\u{017D}',
                0x91 => '\u{2018}',
                0x92 => '\u{2019}',
                0x93 => '\u{201C}',
                0x94 => '\u{201D}',
                0x95 => '\u{2022}',
                0x96 => '\u{2013}',
                0x97 => '\u{2014}',
                0x98 => '\u{02DC}',
                0x99 => '\u{2122}',
                0x9A => '\u{0161}',
                0x9B => '\u{203A}',
                0x9C => '\u{0153}',
                0x9E => '\u{017E}',
                0x9F => '\u{0178}',
                // ASCII, Latin-1 supplement and C1 round-trip bytes
                _ => b as char,
            })
            .collect()
    }

    #[cfg(not(windows))]
    fn to_ansi_argv(text: &str) -> String {
        // On Linux/macOS argv is passed as raw bytes: UTF-8 survives as-is.
        text.to_string()
    }

    fn build_command(
        &self,
        base_lm: &Path,
        acoustic: &Path,
        request: &SynthesizeRequest,
        output_path: &Path,
    ) -> Result<std::process::Command> {
        let binary = Self::find_voxcpm2_binary()?;
        // GPU-only rule: refuse to run when no GPU backend is visible
        // (never fall back to CPU silently).
        crate::gpu_guard::ensure_gpu()?;
        let mut cmd = std::process::Command::new(&binary);
        crate::utils::hide_console_window(&mut cmd);

        // Loader path needs: the voxcpm2-cli binary dir (ggml libs), the
        // shared CUDA runtime dir (Windows only), and the llama.cpp dir
        // (llama libraries used by voxcpm2-cli). Windows uses PATH; Linux
        // LD_LIBRARY_PATH; macOS DYLD_LIBRARY_PATH.
        let mut path_dirs = vec![Self::binary_dir()?];
        if let Some(cuda_dir) = Self::cuda_shared_dir() {
            path_dirs.push(cuda_dir);
        }
        if let Some(llama_dir) = crate::sidecars::sidecar_dir("llama.cpp") {
            path_dirs.push(llama_dir);
        }
        crate::sidecars::apply_loader_path(&mut cmd, &path_dirs);

        // Voice design / style control: prefix "(description)" to the text.
        let voice_description = request
            .extra
            .get("voice_description")
            .map(|s| s.trim())
            .unwrap_or("");
        let full_text = if voice_description.is_empty() {
            request.text.clone()
        } else {
            format!("({}){}", voice_description, request.text)
        };

        cmd.arg("-t").arg(Self::to_ansi_argv(&full_text));
        cmd.arg("-o").arg(output_path);

        // Voice cloning modes. Ultimate cloning takes priority when the
        // reference transcript is provided. In Voice Design mode, never
        // clone: ignore any reference audio, even a stale path passed by
        // mistake (Carlo's bug, 2026-07-22).
        let design_mode = request
            .extra
            .get("voice_mode")
            .map(|m| m.trim() == "design")
            .unwrap_or(false);
        if !design_mode {
            if let Some(ref_wav) = request.reference_audio.as_ref() {
                let prompt_text = request
                    .extra
                    .get("prompt_text")
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default();
                if !prompt_text.is_empty() {
                    cmd.arg("--prompt-wav").arg(ref_wav);
                    cmd.arg("--prompt-text").arg(Self::to_ansi_argv(&prompt_text));
                } else {
                    cmd.arg("-r").arg(ref_wav);
                }
            }
        }

        // Sampling parameters (from extra). NOTE: --temperature was removed
        // on 2026-07-23 (owner decision): it is undocumented upstream, so the
        // engine always runs with its runtime default (1.0).
        if let Some(v) = request.extra.get("cfg") {
            cmd.arg("--cfg").arg(v);
        }
        if let Some(v) = request.extra.get("timesteps") {
            cmd.arg("--timesteps").arg(v);
        }
        if let Some(v) = request.extra.get("steps") {
            cmd.arg("--steps").arg(v);
        }
        if let Some(v) = request.extra.get("seed") {
            if v != "-1" && !v.is_empty() {
                cmd.arg("--seed").arg(v);
            }
        }

        cmd.arg(base_lm);
        cmd.arg(acoustic);

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        Ok(cmd)
    }
}

pub fn synthesize_book(
    plugin: &VoxCpm2Plugin,
    epub_path: &Path,
    output_dir: &Path,
    max_words: usize,
    max_chars: usize,
    ffmpeg: &Path,
    reference_audio: Option<&str>,
    only: Option<&[String]>,
    extra: &std::collections::HashMap<String, String>,
    mut progress: Option<Box<dyn FnMut(&str) + Send>>,
) -> Result<usize> {
    if let Some(cb) = progress.as_deref_mut() {
        cb("Reading EPUB...");
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

    // One seed per book: when the user did not set one, pick it here so
    // every chunk of the same book shares it — otherwise each chunk gets
    // a random seed and the voice changes from chunk to chunk.
    let mut extra_owned = extra.clone();
    let seed_missing = extra_owned
        .get("seed")
        .map(|s| s.trim().is_empty() || s.trim() == "-1")
        .unwrap_or(true);
    if seed_missing {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(42);
        let seed = (nanos ^ (nanos >> 32)) % 9_999_999 + 1;
        extra_owned.insert("seed".to_string(), seed.to_string());
        if let Some(cb) = progress.as_deref_mut() {
            cb(&format!("Auto seed for this book: {seed}"));
        }
    }
    let extra = &extra_owned;
    std::fs::create_dir_all(output_dir)?;
    let recovery_path = output_dir.join("failed_chunks.json");

    if let Some(cb) = progress.as_deref_mut() {
        cb(&format!("Extracted {total_chapters} chapters"));
    }

    let mut recovery_state = recovery::RecoveryState::load(output_dir).unwrap_or_default();
    recovery_state.set_meta(recovery::RecoveryMeta {
        engine_id: Some(plugin.variant_name.clone()),
        reference_audio: reference_audio.map(|s| s.to_string()),
        voice: None,
        language: None,
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
                language: None,
                voice: None,
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

fn synthesize_chunk(plugin: &VoxCpm2Plugin, request: &SynthesizeRequest) -> Result<()> {
    let output_path = Path::new(&request.output_path);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let base_lm = plugin.resolve_base_lm_gguf()?;
    let acoustic = plugin.resolve_acoustic_gguf()?;

    let mut cmd = plugin.build_command(&base_lm, &acoustic, request, output_path)?;

    info!(
        "voxcpm2-cli command: {:?} (variant: {})",
        cmd,
        plugin.variant_name
    );

    let output = cmd
        .output()
        .with_context(|| "failed to spawn voxcpm2-cli")?;

    if !output.status.success() {
        anyhow::bail!(
            "voxcpm2-cli exited with status {}: {}",
            output.status,
            crate::utils::process_error_detail(&output.stdout, &output.stderr)
        );
    }

    if !output_path.exists() {
        anyhow::bail!(
            "voxcpm2-cli exited successfully but output file {} was not created",
            output_path.display()
        );
    }

    Ok(())
}

#[async_trait]
impl BaseTTSPlugin for VoxCpm2Plugin {
    fn name(&self) -> &str {
        &self.variant_name
    }

    fn plugin_type(&self) -> &str {
        "external_process"
    }

    fn is_installed(&self) -> bool {
        self.resolve_base_lm_gguf().is_ok() && self.resolve_acoustic_gguf().is_ok()
    }

    async fn load_model(&self, model_id: &str) -> Result<EngineHandle> {
        let base_lm = self.resolve_base_lm_gguf()?;
        let acoustic = self.resolve_acoustic_gguf()?;
        info!("loading VoxCPM2: {}", self.variant_name);
        info!("base_lm: {}", base_lm.display());
        info!("acoustic: {}", acoustic.display());

        // Verify the voxcpm2-cli binary exists
        Self::find_voxcpm2_binary().with_context(|| "voxcpm2-cli binary not found")?;

        *self.inner.lock().unwrap() = Some(());
        *self.current.lock().unwrap() = Some(model_id.to_string());

        Ok(EngineHandle {
            engine_id: self.variant_name.clone(),
            model_id: model_id.to_string(),
        })
    }

    async fn synthesize(&self, _handle: &EngineHandle, request: &SynthesizeRequest) -> Result<()> {
        synthesize_chunk(self, request)
    }

    async fn unload(&self, _handle: &EngineHandle) -> Result<()> {
        *self.inner.lock().unwrap() = None;
        *self.current.lock().unwrap() = None;
        info!("VoxCPM2 engine unloaded");
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
