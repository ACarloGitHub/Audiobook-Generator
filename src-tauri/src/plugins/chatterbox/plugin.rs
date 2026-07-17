use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use async_trait::async_trait;
use tracing::{info, warn};

use crate::base_plugin::{BaseTTSPlugin, EngineHandle, SynthesizeRequest};
use crate::chunker;
use crate::merger;
use crate::recovery;

pub struct ChatterboxPlugin {
    pub variant_name: String,
    pub models_dir: PathBuf,
}

impl ChatterboxPlugin {
    pub fn new(models_dir: PathBuf, variant_name: &str) -> Self {
        Self {
            variant_name: variant_name.to_string(),
            models_dir,
        }
    }

    fn backbone_gguf(&self) -> Result<PathBuf> {
        for name in &[
            "chatterbox-mtl-t3-Q4_K_M.gguf",
            "chatterbox-mtl-t3-Q5_K_M.gguf",
            "chatterbox-mtl-t3-Q6_K.gguf",
            "chatterbox-mtl-t3-Q8_0.gguf",
        ] {
            let p = self.models_dir.join(&self.variant_name).join(name);
            if p.exists() {
                return Ok(p);
            }
        }
        anyhow::bail!(
            "backbone GGUF not found in {}",
            self.models_dir.join(&self.variant_name).display()
        );
    }

    fn codec_gguf(&self) -> Result<PathBuf> {
        for name in &[
            "chatterbox-mtl-codec-Q4_K_M.gguf",
            "chatterbox-mtl-codec-Q5_K_M.gguf",
        ] {
            let p = self.models_dir.join(name);
            if p.exists() {
                return Ok(p);
            }
        }
        anyhow::bail!(
            "codec GGUF not found in {}",
            self.models_dir.display()
        );
    }

    fn find_tts_cli() -> Result<PathBuf> {
        if let Ok(app_data) = std::env::var("APPDATA") {
            let p = PathBuf::from(&app_data)
                .join("com.patata.audiobookgenerator")
                .join("resources")
                .join("codec.cpp")
                .join("tts-cli.exe");
            if p.exists() {
                return Ok(p);
            }
        }
        let dev = PathBuf::from("resources")
            .join("codec.cpp")
            .join("tts-cli.exe");
        if dev.exists() {
            return Ok(dev);
        }
        anyhow::bail!("tts-cli (codec.cpp) not found — install it from the Models panel")
    }

    fn synthesize_chunk_internal(
        &self,
        text: &str,
        output_path: &Path,
        extra: &std::collections::HashMap<String, String>,
        reference_audio: Option<&str>,
    ) -> Result<()> {
        let tts_cli = Self::find_tts_cli()?;
        let codec = self.codec_gguf()?;
        let backbone = self.backbone_gguf()?;

        let mut cmd = Command::new(&tts_cli);
        cmd.arg("synthesize")
            .arg("--model").arg(&codec)
            .arg("--backbone").arg(&backbone)
            .arg("--text").arg(text)
            .arg("--output").arg(output_path)
            .arg("--gpu");

        if let Some(ref_audio) = reference_audio {
            if !ref_audio.is_empty() && Path::new(ref_audio).exists() {
                cmd.arg("--ref-audio").arg(ref_audio);
            }
        }

        let temperature = extra
            .get("temperature")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.8);
        cmd.arg("--temp").arg(temperature.to_string());

        let top_p = extra
            .get("top_p")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(1.0);
        if top_p < 1.0 {
            cmd.arg("--top-p").arg(top_p.to_string());
        }

        let min_p = extra
            .get("min_p")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.05);
        cmd.arg("--min-p").arg(min_p.to_string());

        let rep_penalty = extra
            .get("repetition_penalty")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(2.0);
        cmd.arg("--rep-penalty").arg(rep_penalty.to_string());

        let cfg_weight = extra
            .get("cfg_weight")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.5);
        cmd.arg("--cfg-weight").arg(cfg_weight.to_string());

        let max_new_tokens = extra
            .get("max_new_tokens")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(1000);
        cmd.arg("--max-frames").arg(max_new_tokens.to_string());

        if let Some(seed_str) = extra.get("seed") {
            if let Ok(seed_val) = seed_str.parse::<i32>() {
                if seed_val >= 0 {
                    cmd.arg("--seed").arg(seed_val.to_string());
                }
            }
        }

        info!(
            "[chatterbox] running tts-cli: backbone={}, codec={}",
            backbone.display(),
            codec.display()
        );

        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("failed to spawn tts-cli")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("tts-cli failed: {}", stderr.trim());
        }

        if !output_path.exists() {
            anyhow::bail!(
                "tts-cli completed but output WAV not found at {}",
                output_path.display()
            );
        }

        info!("[chatterbox] WAV written: {}", output_path.display());
        Ok(())
    }
}

pub fn synthesize_book(
    plugin: &ChatterboxPlugin,
    epub_path: &Path,
    output_dir: &Path,
    max_words: usize,
    max_chars: usize,
    ffmpeg: &Path,
    extra: &std::collections::HashMap<String, String>,
    reference_audio: Option<&str>,
    mut progress: Option<Box<dyn FnMut(&str) + Send>>,
) -> Result<usize> {
    if let Some(cb) = progress.as_deref_mut() {
        cb("Reading EPUB...");
    }
    let chapters = crate::epub::extract_chapters(epub_path)?;
    let total_chapters = chapters.len();
    std::fs::create_dir_all(output_dir)?;
    let recovery_path = output_dir.join("failed_chunks.json");

    if let Some(cb) = progress.as_deref_mut() {
        cb(&format!("Extracted {total_chapters} chapters"));
    }

    let mut recovery_state = recovery::RecoveryState::load(output_dir).unwrap_or_default();
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

            match plugin.synthesize_chunk_internal(chunk_text, &wav_path, extra, reference_audio) {
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
            serde_json::to_string_pretty(&recovery_state)
                .unwrap_or_else(|_| "{}".to_string()),
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

#[async_trait]
impl BaseTTSPlugin for ChatterboxPlugin {
    fn name(&self) -> &str {
        &self.variant_name
    }

    fn plugin_type(&self) -> &str {
        "codec_cpp"
    }

    fn is_installed(&self) -> bool {
        self.backbone_gguf().is_ok() && self.codec_gguf().is_ok()
    }

    async fn load_model(&self, model_id: &str) -> Result<EngineHandle> {
        let backbone = self.backbone_gguf()?;
        let codec = self.codec_gguf()?;
        Self::find_tts_cli().context("tts-cli (codec.cpp) not found")?;
        info!(
            "[chatterbox] ready: backbone={}, codec={}",
            backbone.display(),
            codec.display()
        );
        Ok(EngineHandle {
            engine_id: self.variant_name.clone(),
            model_id: model_id.to_string(),
        })
    }

    async fn synthesize(&self, _handle: &EngineHandle, request: &SynthesizeRequest) -> Result<()> {
        let output_path = Path::new(&request.output_path);
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        self.synthesize_chunk_internal(
            &request.text,
            output_path,
            &request.extra,
            request.reference_audio.as_deref(),
        )
    }

    async fn unload(&self, _handle: &EngineHandle) -> Result<()> {
        info!("[chatterbox] unloaded");
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
