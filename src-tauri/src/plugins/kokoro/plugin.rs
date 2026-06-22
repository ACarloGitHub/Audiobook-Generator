use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use async_trait::async_trait;
use hound::{SampleFormat, WavSpec};
use kokoro_en::{KokoroTts, Voice};
use tracing::{debug, info, warn};

use crate::base_plugin::{BaseTTSPlugin, EngineHandle, SynthesizeRequest};
use crate::chunker;
use crate::merger;
use crate::recovery;

pub struct KokoroPlugin {
    pub paths: KokoroPaths,
    pub voice: String,
    inner: Mutex<Option<KokoroTts>>,
    current: Mutex<Option<String>>,
}

#[derive(Debug, Clone)]
pub struct KokoroPaths {
    pub model_dir: PathBuf,
    pub voices_dir: PathBuf,
}

impl KokoroPaths {
    pub fn from_app_data(app_data: &Path) -> Self {
        Self {
            model_dir: app_data.join("models").join("kokoro").join("models"),
            voices_dir: app_data.join("models").join("kokoro").join("voices"),
        }
    }
}

impl KokoroPlugin {
    pub fn new(paths: KokoroPaths, voice: &str) -> Self {
        Self {
            paths,
            voice: voice.to_string(),
            inner: Mutex::new(None),
            current: Mutex::new(None),
        }
    }

    fn resolve_model(&self) -> Result<PathBuf> {
        for name in ["model_quantized.onnx", "model_q8f16.onnx", "model.onnx"] {
            let p = self.paths.model_dir.join(name);
            if p.exists() {
                return Ok(p);
            }
        }
        anyhow::bail!(
            "no Kokoro model found in {}. Download from the Models panel.",
            self.paths.model_dir.display()
        );
    }
}

pub fn synthesize_book(
    plugin: &KokoroPlugin,
    epub_path: &Path,
    output_dir: &Path,
    max_words: usize,
    ffmpeg: &Path,
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
        let chapter_dir = output_dir.join(crate::utils::sanitize_filename(&chapter.title));
        std::fs::create_dir_all(&chapter_dir)?;

        let chunks = chunker::chunk_text(&chapter.text, max_words);
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
            let wav_path = chapter_dir.join(format!("chunk_{:04}.wav", j + 1));
            if recovery_state.is_done(&chapter.title, j) && wav_path.exists() {
                wavs.push(wav_path);
                continue;
            }
            match synthesize_chunk(plugin, chunk_text, &wav_path) {
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
                            short_err(&format!("{e:#}"))
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
                    cb(&format!(
                        "ERROR: merge failed for {}: {}",
                        chapter.title,
                        short_err(&format!("{e:#}"))
                    ));
                }
            }
        }

        let _ = std::fs::write(
            &recovery_path,
            serde_json::to_string_pretty(&recovery_state).unwrap_or_else(|_| "{}".to_string()),
        );
        info!("chapter {}/{} done", i + 1, chapters.len());
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

fn synthesize_chunk(plugin: &KokoroPlugin, text: &str, output_path: &Path) -> Result<()> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!("failed to create chunk output dir {}", parent.display())
        })?;
    }

    let mut guard = plugin.inner.lock().unwrap();
    let tts = guard
        .as_mut()
        .context("Kokoro engine not loaded; call load_model() first")?;

    let voice = Voice::new(&plugin.voice);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime for synthesis")?;
    let (samples, _took) = rt
        .block_on(tts.synth(text, voice))
        .with_context(|| format!("Kokoro synthesis failed for text: {:.40}...", text))?;
    drop(rt);
    drop(guard);

    write_wav(output_path, 24_000, &samples)?;
    debug!(
        "wrote {} samples to {}",
        samples.len(),
        output_path.display()
    );
    Ok(())
}

#[async_trait]
impl BaseTTSPlugin for KokoroPlugin {
    fn name(&self) -> &str {
        "kokoro"
    }

    fn plugin_type(&self) -> &str {
        "in_process"
    }

    fn is_installed(&self) -> bool {
        let model_ok = ["model_quantized.onnx", "model_q8f16.onnx", "model.onnx"]
            .iter()
            .any(|n| self.paths.model_dir.join(n).exists());
        let voices_ok = self.paths.voices_dir.join("af_heart.bin").exists();
        model_ok && voices_ok
    }

    async fn load_model(&self, model_id: &str) -> Result<EngineHandle> {
        let model_path = self.resolve_model().context("Kokoro model not installed")?;
        info!("loading Kokoro ONNX from {}", model_path.display());
        info!("loading voices from {}", self.paths.voices_dir.display());

        prepend_cuda_to_path();

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("failed to build tokio runtime for KokoroTts::new")?;
        let tts = rt
            .block_on(KokoroTts::new(
                model_path.to_str().context("non-UTF-8 model path")?,
                self.paths.voices_dir.to_str().context("non-UTF-8 voices dir")?,
            ))
            .context("failed to construct KokoroTts")?;
        drop(rt);

        *self.inner.lock().unwrap() = Some(tts);
        *self.current.lock().unwrap() = Some(model_id.to_string());

        Ok(EngineHandle {
            engine_id: "kokoro".into(),
            model_id: model_id.to_string(),
        })
    }

    async fn synthesize(&self, handle: &EngineHandle, request: &SynthesizeRequest) -> Result<()> {
        let output_path = Path::new(&request.output_path);
        synthesize_chunk(self, &request.text, output_path)
    }

    async fn unload(&self, _handle: &EngineHandle) -> Result<()> {
        *self.inner.lock().unwrap() = None;
        *self.current.lock().unwrap() = None;
        info!("Kokoro engine unloaded");
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn write_wav(path: &Path, sample_rate: u32, samples: &[f32]) -> Result<()> {
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .with_context(|| format!("failed to create WAV at {}", path.display()))?;
    for &s in samples {
        let clipped = s.clamp(-1.0, 1.0);
        let int = (clipped * i16::MAX as f32) as i16;
        writer.write_sample(int)?;
    }
    writer.finalize()?;
    Ok(())
}

fn prepend_cuda_to_path() {
    if !cfg!(windows) {
        return;
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.to_path_buf());
            candidates.push(parent.join("sidecars").join("cuda"));
            candidates.push(parent.join("sidecars"));
            if let Some(workspace_root) = parent.parent().and_then(|p| p.parent()) {
                candidates.push(workspace_root.join("src-tauri").join("sidecars").join("cuda"));
                candidates.push(workspace_root.join("src-tauri").join("sidecars"));
            }
        }
    }
    candidates.retain(|p| p.is_dir());
    if candidates.is_empty() {
        return;
    }
    let mut joined = std::env::join_paths(&candidates).unwrap_or_default();
    if let Some(existing) = std::env::var_os("PATH") {
        joined.push(existing);
    }
    std::env::set_var("PATH", joined);
    debug!("prepended to PATH: {:?}", candidates);
}

fn short_err(s: &str) -> String {
    let one_line = s.lines().next().unwrap_or(s);
    if one_line.len() > 200 {
        format!("{}...", &one_line[..200])
    } else {
        one_line.to_string()
    }
}