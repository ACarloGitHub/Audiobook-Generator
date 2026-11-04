//! Audio8-TTS-Realistic 0.6B engine (audio.cpp sidecar).
//!
//! The model is a DualAR speech generator shipped as a standalone GGUF and
//! served by `audiocpp_cli` from the audio.cpp project (Apache-2.0, ggml, no
//! Python). Voice cloning is the only usable mode for a book: without a
//! reference recording the model invents a new voice on every call.
//!
//! Two engine behaviours are handled explicitly here (measured 2026-11-01):
//!
//! * a reference recording **must** come with its exact transcript, otherwise
//!   the runtime refuses to start;
//! * the runtime has its own text splitter, and that splitter loses text at the
//!   seams. Every call therefore carries one chunk that already fits, and
//!   `text_chunk_size` is set to that chunk length so the runtime never splits.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use async_trait::async_trait;
use tracing::{info, warn};

use crate::base_plugin::{BaseTTSPlugin, EngineHandle, SynthesizeRequest};
use crate::chunker;
use crate::merger;
use crate::plugin_manager::Audio8Paths;
use crate::recovery;

pub struct Audio8Plugin {
    pub paths: Audio8Paths,
    pub variant_name: String,
}

/// Map an English language label from the UI/registry to the ISO code the
/// `audio8_tts` runtime declares (`model_specs/audio8_tts.json`). `None`
/// means "do not send a hint" (autodetect, or a language the runtime's
/// language option does not cover).
fn language_code(label: Option<&str>) -> Option<&'static str> {
    let label = label?.trim().to_ascii_lowercase();
    let code = match label.as_str() {
        "italian" => "it",
        "english" => "en",
        "french" => "fr",
        "german" => "de",
        "spanish" => "es",
        "dutch" => "nl",
        "polish" => "pl",
        "chinese" => "zh",
        "cantonese" => "yue",
        "japanese" => "ja",
        "korean" => "ko",
        _ => return None,
    };
    Some(code)
}

impl Audio8Plugin {
    pub fn new(paths: Audio8Paths, variant_name: &str) -> Self {
        Self {
            paths,
            variant_name: variant_name.to_string(),
        }
    }

    /// The engine id carries the quantisation ("Audio8-TTS-Realistic Q8_0").
    /// There is no silent fallback to another quant: if the chosen file is
    /// missing the user must download it from the Models panel.
    fn resolve_gguf(&self) -> Result<PathBuf> {
        let file = crate::plugin_manager::audio8_quant_for_engine(&self.variant_name)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "unknown Audio8 engine id '{}'. Expected e.g. 'Audio8-TTS-Realistic Q8_0'.",
                    self.variant_name
                )
            })?;
        let p = self.paths.models_dir.join(&file.filename);
        if p.exists() {
            return Ok(p);
        }
        anyhow::bail!(
            "{} is not downloaded (missing {}). Download it from the Models panel.",
            self.variant_name,
            p.display()
        );
    }

    fn find_binary() -> Result<PathBuf> {
        let exe_name = if cfg!(windows) {
            "audiocpp_cli.exe"
        } else {
            "audiocpp_cli"
        };
        if let Some(p) = crate::sidecars::sidecar_binary("audio8", exe_name) {
            return Ok(p);
        }
        anyhow::bail!(
            "audiocpp_cli binary not found. It ships inside the installer; reinstall the app or, \
             for development, place it in resources/audio8/."
        )
    }

    fn binary_dir() -> Result<PathBuf> {
        Ok(Self::find_binary()?
            .parent()
            .ok_or_else(|| anyhow::anyhow!("cannot get parent of audiocpp_cli binary"))?
            .to_path_buf())
    }

    fn cuda_shared_dir() -> Option<PathBuf> {
        crate::sidecars::sidecar_dir("cuda-shared")
    }

    // NOTE (2026-11-01): the text is passed as-is. audiocpp_cli is built with
    // /utf-8 and receives the command line as UTF-8, unlike voxcpm2-cli which
    // needs the CP1252 round-trip trick: applying that trick here doubled the
    // audio length on an accented sentence (1.9 s vs 4.8 s for the same text),
    // i.e. the model was reading mojibake instead of Italian.
    fn build_command(
        &self,
        gguf: &Path,
        request: &SynthesizeRequest,
        output_path: &Path,
    ) -> Result<std::process::Command> {
        let binary = Self::find_binary()?;
        // GPU-only rule: refuse to run when no GPU backend is visible.
        crate::gpu_guard::ensure_gpu()?;

        let ref_wav = request
            .reference_audio
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Audio8 needs a reference recording: pick a voice sample in Configuration."
                )
            })?;
        let ref_text = request
            .extra
            .get("ref_text")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Audio8 needs the transcript of the reference recording: type the exact words \
                     spoken in the sample."
                )
            })?;

        let mut cmd = std::process::Command::new(&binary);
        crate::utils::hide_console_window(&mut cmd);
        let mut path_dirs = vec![Self::binary_dir()?];
        if let Some(cuda_dir) = Self::cuda_shared_dir() {
            path_dirs.push(cuda_dir);
        }
        crate::sidecars::apply_loader_path(&mut cmd, &path_dirs);

        cmd.arg("--task").arg("clon");
        cmd.arg("--family").arg("audio8_tts");
        cmd.arg("--model").arg(gguf);
        cmd.arg("--backend").arg("cuda");
        cmd.arg("--text").arg(&request.text);
        cmd.arg("--voice-ref").arg(ref_wav);
        cmd.arg("--reference-text").arg(ref_text);
        cmd.arg("--out").arg(output_path);
        cmd.arg("--metrics");

        // Language hint. The runtime expects an ISO code, while the UI and the
        // registry use English labels. Only the codes declared by the
        // audio8_tts model spec are sent; anything else is left to autodetect
        // instead of risking a rejected request.
        if let Some(code) = language_code(request.language.as_deref()) {
            cmd.arg("--language").arg(code);
        }

        // The runtime has its own splitter and loses text at its seams, so the
        // chunk we send is exactly one block for it.
        let chunk_chars = request.text.chars().count().max(1);
        cmd.arg("--request-option")
            .arg(format!("text_chunk_size={chunk_chars}"));

        for (key, flag) in [
            ("temperature", "temperature"),
            ("top_p", "top_p"),
            ("top_k", "top_k"),
            ("max_tokens", "max_tokens"),
        ] {
            if let Some(v) = request.extra.get(key) {
                if !v.trim().is_empty() {
                    cmd.arg("--request-option").arg(format!("{flag}={v}"));
                }
            }
        }
        if let Some(v) = request.extra.get("seed") {
            if v != "-1" && !v.trim().is_empty() {
                cmd.arg("--request-option").arg(format!("seed={v}"));
            }
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        Ok(cmd)
    }
}

fn run_command(mut cmd: std::process::Command, context: &str) -> Result<String> {
    let output = cmd
        .output()
        .with_context(|| format!("failed to spawn audiocpp_cli for {context}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        let last = stderr
            .lines()
            .chain(stdout.lines())
            .filter(|l| !l.trim().is_empty())
            .last()
            .unwrap_or("no output");
        anyhow::bail!("audiocpp_cli failed for {context}: {last}");
    }
    Ok(format!("{stdout}\n{stderr}"))
}

#[async_trait]
impl BaseTTSPlugin for Audio8Plugin {
    fn name(&self) -> &str {
        &self.variant_name
    }

    fn plugin_type(&self) -> &str {
        "external_process"
    }

    fn is_installed(&self) -> bool {
        self.resolve_gguf().is_ok() && Self::find_binary().is_ok()
    }

    async fn load_model(&self, model_id: &str) -> Result<EngineHandle> {
        // Nothing to load: audiocpp_cli starts, synthesizes and exits.
        Ok(EngineHandle {
            engine_id: self.variant_name.clone(),
            model_id: model_id.to_string(),
        })
    }

    async fn synthesize(&self, _handle: &EngineHandle, request: &SynthesizeRequest) -> Result<()> {
        let gguf = self.resolve_gguf()?;
        let output_path = PathBuf::from(&request.output_path);
        let cmd = self.build_command(&gguf, request, &output_path)?;
        run_command(cmd, &request.output_path)?;
        if !output_path.exists() {
            anyhow::bail!(
                "audiocpp_cli reported success but produced no file at {}",
                output_path.display()
            );
        }
        Ok(())
    }

    async fn unload(&self, _handle: &EngineHandle) -> Result<()> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Audio8Plugin {
    fn synthesize_chunk(&self, request: &SynthesizeRequest) -> Result<()> {
        let gguf = self.resolve_gguf()?;
        let output_path = PathBuf::from(&request.output_path);
        let cmd = self.build_command(&gguf, request, &output_path)?;
        let log = run_command(cmd, &request.output_path)?;
        if !output_path.exists() {
            anyhow::bail!(
                "audiocpp_cli reported success but produced no file at {}",
                output_path.display()
            );
        }
        if let Some(rtf) = log.lines().find(|l| l.contains("metrics.rtf=")) {
            info!("{}", rtf.trim());
        }
        Ok(())
    }
}

pub fn synthesize_book(
    plugin: &Audio8Plugin,
    epub_path: &Path,
    output_dir: &Path,
    max_words: usize,
    max_chars: usize,
    ffmpeg: Option<&Path>,
    reference_audio: Option<&str>,
    language: Option<&str>,
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
    // Reference-quality preflight. Audio8 copies the pace and the pauses of
    // the reference recording, so a long or pause-riddled sample is the first
    // suspect when the narration stalls after a few words. The recording also
    // shares the 2048-token context with the text.
    if let Some(path) = reference_audio {
        if let Some(info) = crate::audio_check::reference_info(Path::new(path)) {
            let mut messages: Vec<String> = Vec::new();
            if info.duration_seconds > 30.0 {
                messages.push(format!(
                    "reference is {:.1}s: Audio8 copies its pace and shares the 2048-token context, so keep it within 10-30s (~30 words)",
                    info.duration_seconds
                ));
            }
            if info.longest_silence_seconds > 1.5 {
                messages.push(format!(
                    "reference contains a {:.1}s pause: the model imitates it, which is the usual cause of long pauses after a few words",
                    info.longest_silence_seconds
                ));
            }
            let words = extra
                .get("ref_text")
                .map(|t| t.split_whitespace().count())
                .unwrap_or(0);
            if words == 0 {
                messages.push("reference transcript is empty (the runtime requires it)".to_string());
            } else if words > 30 {
                messages.push(format!(
                    "reference transcript has {words} words: keep the sample around 20-30 words for a stable clone"
                ));
            } else {
                let words_per_second = words as f64 / info.duration_seconds.max(0.1);
                if !(0.8..=5.0).contains(&words_per_second) {
                    messages.push(format!(
                        "reference transcript looks mismatched: {words} words in {:.1}s ({words_per_second:.1} words/s)",
                        info.duration_seconds
                    ));
                }
            }
            for message in messages {
                warn!("[audio8] {message}");
                if let Some(cb) = progress.as_deref_mut() {
                    cb(&format!("WARN: {message}"));
                }
            }
        }
    }
    let chapters = crate::input::extract_chapters_from(epub_path)?;
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

    // One seed per book, so a re-run of the same book is reproducible.
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

    if let Some(cb) = progress.as_deref_mut() {
        cb(&format!("Extracted {total_chapters} chapters"));
    }

    let mut recovery_state = recovery::RecoveryState::load(output_dir).unwrap_or_default();

    let chunking_changed = recovery_state.reset_if_chunking_changed();
    recovery_state.set_meta(recovery::RecoveryMeta {
        engine_id: Some(plugin.variant_name.clone()),
        reference_audio: reference_audio.map(|s| s.to_string()),
        voice: None,
        language: language.map(|s| s.to_string()),
        extra: extra.clone(),
        generated_at: Some(recovery::now_stamp()),
        chunker_version: recovery::CURRENT_CHUNKER_VERSION,
    });
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
                voice: None,
                extra: extra.clone(),
            };

            match plugin.synthesize_chunk(&request) {
                Ok(()) => {
                    let suspicion = crate::audio_check::wav_duration_seconds(&wav_path)
                        .and_then(|secs| rate_guard.observe(chunk_text.chars().count(), secs));
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
            let merged = merger::merge_chapter(&wavs, output_dir, &base, ffmpeg)?;
            if let Some(cb) = progress.as_deref_mut() {
                cb(&format!(
                    "Merged {} chunk(s) into {}",
                    wavs.len(),
                    merged.display()
                ));
            }
        }

        recovery_state.save(output_dir)?;
    }

    if let Some(cb) = progress.as_deref_mut() {
        cb(&format!(
            "Done. {done_count} chunk(s) converted, {failed_count} failed."
        ));
    }
    Ok(done_count)
}
