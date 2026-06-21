//! Kokoro-82M TTS engine.
//!
//! Wraps `kokoro-en` (which itself wraps ONNX Runtime in-process via
//! the `ort` crate) and exposes it through the engine-agnostic
//! `Engine` trait. See `engines.rs`.
//!
//! The model file and voice packs are expected under the per-user data
//! directory. The download pipeline (First-Run Wizard, Models panel)
//! populates that directory before the user clicks Generate.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use hound::{SampleFormat, WavSpec};
use kokoro_en::{KokoroTts, Voice};
use tracing::{debug, info, warn};

use crate::chunker;
use crate::engines::{Engine, EngineHandle, EngineInfo, SynthesizeRequest};
use crate::merger;

/// Default path to the Kokoro data directory.
///
/// In a Tauri build, this resolves to `<app_data>/kokoro/`. The
/// helper below uses Tauri's `AppHandle::path().app_data_dir()` to
/// locate it; for now we let the caller pass the directory in via
/// `KokoroEngine::with_data_dir` so the engine stays independent of
/// the Tauri runtime and is testable from plain Rust binaries
/// (e.g. a future CLI front-end).
#[derive(Debug, Clone)]
pub struct KokoroPaths {
    pub model_dir: PathBuf,
    pub voices_dir: PathBuf,
}

impl KokoroPaths {
    pub fn from_data_root(root: &Path) -> Self {
        Self {
            model_dir: root.join("kokoro").join("models"),
            voices_dir: root.join("kokoro").join("voices"),
        }
    }
}

pub struct KokoroEngine {
    paths: KokoroPaths,
    /// The inner `KokoroTts` is built lazily on first `load`. It is
    /// wrapped in a `Mutex` because `KokoroTts` is `!Sync` (it holds
    /// ONNX Runtime session state).
    inner: Mutex<Option<KokoroTts>>,
    /// The currently loaded model id, kept so `current_vram_bytes` and
    /// `info` can report it.
    current: Mutex<Option<String>>,
    /// Holds the model id; `EngineHandle.model_id` is a `String`, so we
    /// just need to keep it around for the status command.
    voice: String,
}

impl KokoroEngine {
    pub fn new(paths: KokoroPaths, voice: &str) -> Self {
        Self {
            paths,
            inner: Mutex::new(None),
            current: Mutex::new(None),
            voice: voice.to_string(),
        }
    }

    /// Default data paths for the engine. The First-Run Wizard places
    /// the model and voice packs in these directories.
    ///
    /// For now we resolve them under the current working directory's
    /// `kokoro/{models,voices}/` so the engine works in dev without a
    /// Tauri runtime. The wizard will redirect the paths to the real
    /// per-user data dir when it lands.
    pub fn default_data_paths() -> KokoroPaths {
        KokoroPaths::from_data_root(&std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    fn resolve_model(&self) -> Result<PathBuf> {
        for name in ["model_quantized.onnx", "model_q8f16.onnx", "model.onnx"] {
            let p = self.paths.model_dir.join(name);
            if p.exists() {
                return Ok(p);
            }
        }
        anyhow::bail!(
            "no Kokoro model found in {}. Run the First-Run Wizard to download it.",
            self.paths.model_dir.display()
        );
    }
}

impl Engine for KokoroEngine {
    fn info(&self) -> &EngineInfo {
        static I: std::sync::OnceLock<Box<EngineInfo>> = std::sync::OnceLock::new();
        I.get_or_init(|| {
            Box::new(EngineInfo {
                id: "kokoro".into(),
                display_name: "Kokoro 82M".into(),
                format: "ONNX".into(),
                voice_cloning: false,
                hardware: vec!["CPU".into(), "CUDA".into()],
                license: "Apache 2.0".into(),
                languages: vec![
                    "en".into(),
                    "ja".into(),
                    "zh".into(),
                    "es".into(),
                    "fr".into(),
                    "hi".into(),
                    "it".into(),
                    "pt".into(),
                ],
            })
        })
    }

    fn is_loaded(&self) -> bool {
        self.inner.lock().unwrap().is_some()
    }

    fn load(&self, model_id: &str) -> Result<EngineHandle> {
        let model_path = self
            .resolve_model()
            .context("Kokoro model not installed")?;
        info!("loading Kokoro ONNX from {}", model_path.display());
        info!("loading voices from {}", self.paths.voices_dir.display());

        // Make sure the CUDA / cuDNN runtime DLLs are findable on
        // PATH before we hand off to ort. We look in three places:
        //   1. `<project>/src-tauri/sidecars/cuda/`  (dev)
        //   2. `<exe-dir>/sidecars/cuda/`             (bundled installer)
        //   3. `<exe-dir>/`                           (Tauri's default)
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

    fn synthesize(
        &self,
        handle: &EngineHandle,
        request: &SynthesizeRequest,
        output_wav: &Path,
    ) -> Result<()> {
        if let Some(parent) = output_wav.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create chunk output dir {}", parent.display())
            })?;
        }

        // The inner `KokoroTts` must be held across the async call.
        let mut guard = self.inner.lock().unwrap();
        let tts = guard
            .as_mut()
            .context("Kokoro engine not loaded; call load() first")?;
        if self.current.lock().unwrap().as_deref() != Some(&handle.model_id) {
            anyhow::bail!("Kokoro engine is loaded with a different model id");
        }

        let voice = Voice::new(&self.voice);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("failed to build tokoro runtime for synthesis")?;
        let (samples, took) = rt
            .block_on(tts.synth(&request.text, voice))
            .with_context(|| format!("Kokoro synthesis failed for text: {:.40}...", request.text))?;
        drop(rt);

        write_wav(output_wav, 24_000, &samples)?;
        tracing::debug!(
            "wrote {} samples in {took:?} to {}",
            samples.len(),
            output_wav.display()
        );
        Ok(())
    }

    fn unload(&self, _handle: &EngineHandle) -> Result<()> {
        // Dropping the `KokoroTts` releases ONNX Runtime session state
        // and the pinned model weights in VRAM. Done.
        let mut guard = self.inner.lock().unwrap();
        *guard = None;
        *self.current.lock().unwrap() = None;
        info!("Kokoro engine unloaded");
        Ok(())
    }

    fn current_vram_bytes(&self) -> Option<u64> {
        // Kokoro 82M is ~330 MB FP32 / ~92 MB int8. We can't read the
        // actual VRAM usage from `KokoroTts` (ort 2.0 RC doesn't expose
        // a "session memory" hook yet). We report the on-disk model
        // size as a conservative lower bound so the UI has a number
        // to display.
        let model = self.resolve_model().ok()?;
        let size = std::fs::metadata(model).ok()?.len();
        Some(size)
    }

    fn as_kokoro(&self) -> Option<&KokoroEngine> {
        Some(self)
    }
}

/// Pick a voice by name (passed in from the UI). Default to
/// `af_heart`. The frontend can override via the engine's
/// "synthesize" payload in a future revision.
impl KokoroEngine {
    pub fn with_voice(mut self, voice: &str) -> Self {
        self.voice = voice.to_string();
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

/// Prepend CUDA / cuDNN DLL search paths to the process PATH so that
/// ort can find `cudart64_12.dll`, `cublas64_12.dll`, `cudnn64_9.dll`,
/// etc. on Windows. This is a no-op if the paths do not exist.
fn prepend_cuda_to_path() {
    if !cfg!(windows) {
        return;
    }
    let mut candidates: Vec<PathBuf> = Vec::new();

    // 1. The directory of the running executable. ort 2.0 RC resolves
    //    `onnxruntime_providers_shared.dll` and `onnxruntime_providers_cuda.dll`
    //    relative to the executable's directory, so the parent of the
    //    binary must be searchable.
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

/// Top-level entry point used by the Tauri command layer to
/// synthesize an entire book: parse the EPUB, chunk, synthesize each
/// chunk, merge with ffmpeg, write `failed_chunks.json` for recovery.
///
/// This is the only function the frontend needs to know about. It
/// coordinates the engine, the chunker, the merger, and the recovery
/// state.
pub fn synthesize_book(
    engine: &KokoroEngine,
    handle: &EngineHandle,
    epub_path: &Path,
    output_dir: &Path,
    max_words: usize,
    ffmpeg: &Path,
) -> Result<usize> {
    let chapters = crate::epub::extract_chapters(epub_path)?;
    std::fs::create_dir_all(output_dir)?;
    let recovery_path = output_dir.join("failed_chunks.json");

    let mut recovery = crate::recovery::RecoveryState::load(output_dir).unwrap_or_default();
    let mut done_count = 0usize;

    for (i, chapter) in chapters.iter().enumerate() {
        let chapter_dir = output_dir.join(sanitize_filename(&chapter.title));
        std::fs::create_dir_all(&chapter_dir)?;

        let chunks = chunker::chunk_text(&chapter.text, max_words);
        let mut wavs = Vec::with_capacity(chunks.len());
        for (j, chunk_text) in chunks.iter().enumerate() {
            let wav_path = chapter_dir.join(format!("chunk_{:04}.wav", j + 1));
            if recovery.is_done(&chapter.title, j) && wav_path.exists() {
                wavs.push(wav_path);
                continue;
            }
            let request = SynthesizeRequest {
                text: chunk_text.clone(),
                reference_audio: None,
                language: None,
                voice: Some(engine.voice.clone()),
                extra: Default::default(),
            };
            match engine.synthesize(handle, &request, &wav_path) {
                Ok(()) => {
                    recovery.mark_done(&chapter.title, j);
                    wavs.push(wav_path);
                    done_count += 1;
                }
                Err(e) => {
                    warn!("chunk {}/{} failed: {e:#}", j + 1, chunks.len());
                    let mut failed_map = std::collections::HashMap::new();
                    failed_map.insert(
                        chapter.title.clone(),
                        vec![crate::recovery::FailedChunk {
                            chunk_index: j,
                            text: chunk_text.clone(),
                            error: format!("{e:#}"),
                        }],
                    );
                    // Persist via the recovery module's writer hook.
                    let _ = std::fs::write(
                        &recovery_path,
                        serde_json::to_string_pretty(&recovery)
                            .unwrap_or_else(|_| "{}".to_string()),
                    );
                    let _ = failed_map;
                }
            }
        }
        if !wavs.is_empty() {
            let mp3_path = output_dir.join(format!("{}.mp3", sanitize_filename(&chapter.title)));
            merger::merge_wavs_to_mp3(&wavs, &mp3_path, ffmpeg)?;
        }
        // Persist recovery after every chapter.
        let _ = std::fs::write(
            &recovery_path,
            serde_json::to_string_pretty(&recovery).unwrap_or_else(|_| "{}".to_string()),
        );
        info!("chapter {}/{} done", i + 1, chapters.len());
    }
    Ok(done_count)
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .replace(' ', "_")
        .to_lowercase()
}
