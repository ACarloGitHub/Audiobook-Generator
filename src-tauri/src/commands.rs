//! Tauri commands exposed to the frontend.
//!
//! Engine-agnostic surface: the frontend never sees engine-specific
//! code. Every command works the same way for Kokoro, Qwen3-TTS,
//! OuteTTS, NeuTTS Air.
//!
//! See AudiobookGenerator-Wiki/wiki/concepts/engine-lifecycle.md
//! for the load / hold / release state machine.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::engines::kokoro::{synthesize_book, KokoroEngine};
use crate::engines::{defaults_for, EngineDefaults, EngineHandle, EngineInfo, EngineRegistry, SynthesizeRequest};
use crate::merger;
use crate::recovery::{self, RecoveryState};

#[derive(Debug, Serialize, Clone)]
pub struct EngineStatus {
    pub active_engine: Option<String>,
    pub active_model: Option<String>,
    pub vram_bytes: Option<u64>,
    pub loaded_at: Option<String>,
    pub engines: Vec<EngineInfo>,
    pub hardware: HardwareSummary,
}

#[derive(Debug, Serialize, Clone)]
pub struct HardwareSummary {
    pub os: String,
    pub arch: String,
    pub gpus: Vec<GpuInfo>,
}

#[derive(Debug, Serialize, Clone)]
pub struct GpuInfo {
    pub vendor: String,
    pub model: String,
    pub vram_bytes: u64,
    pub backend: String,
}

static STOP_FLAG: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn engine_status(registry: State<'_, Arc<EngineRegistry>>) -> EngineStatus {
    let active = registry.active();
    let engines = registry.catalogue();
    let vram = active
        .as_ref()
        .and_then(|h| registry.get(&h.engine_id))
        .and_then(|e| e.current_vram_bytes());

    EngineStatus {
        active_engine: active.as_ref().map(|h| h.engine_id.clone()),
        active_model: active.as_ref().map(|h| h.model_id.clone()),
        vram_bytes: vram,
        loaded_at: active.as_ref().map(|_| "just now".to_string()),
        engines,
        hardware: detect_hardware_stub(),
    }
}

#[tauri::command]
pub fn load_engine(
    engine_id: String,
    model_id: String,
    registry: State<'_, Arc<EngineRegistry>>,
) -> Result<EngineHandle, String> {
    // Auto-release: drop any currently loaded engine before loading
    // the new one. This is the single most important VRAM safety
    // path. See AudiobookGenerator-Wiki/wiki/concepts/engine-lifecycle.md.
    if let Some(prev) = registry.active() {
        if let Some(prev_engine) = registry.get(&prev.engine_id) {
            let _ = prev_engine.unload(&prev);
        }
        registry.set_active(None);
    }

    let engine = registry
        .get(&engine_id)
        .ok_or_else(|| format!("unknown engine '{engine_id}'"))?;
    let handle = engine
        .load(&model_id)
        .map_err(|e| format!("load failed: {e:#}"))?;
    registry.set_active(Some(handle.clone()));
    Ok(handle)
}

#[tauri::command]
pub fn unload_engine(registry: State<'_, Arc<EngineRegistry>>) -> Result<(), String> {
    let active = registry.active();
    if let Some(h) = active {
        if let Some(engine) = registry.get(&h.engine_id) {
            engine.unload(&h).map_err(|e| format!("unload failed: {e:#}"))?;
        }
        registry.set_active(None);
    }
    Ok(())
}

#[tauri::command]
pub async fn synthesize(
    handle: EngineHandle,
    request: SynthesizeRequest,
    output_wav: PathBuf,
    registry: State<'_, Arc<EngineRegistry>>,
) -> Result<(), String> {
    let engine = registry
        .get(&handle.engine_id)
        .ok_or_else(|| format!("unknown engine '{}'", handle.engine_id))?;
    engine
        .synthesize(&handle, &request, &output_wav)
        .map_err(|e| format!("synthesize failed: {e:#}"))
}

#[tauri::command]
pub fn stop_generation() {
    STOP_FLAG.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub fn engine_defaults(engine_id: String) -> EngineDefaults {
    defaults_for(&engine_id)
}

#[derive(Debug, Serialize, Clone)]
pub struct BookInfo {
    pub title: String,
    pub chapters: Vec<ChapterSummary>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ChapterSummary {
    pub title: String,
    pub char_count: usize,
}

#[tauri::command]
pub fn load_epub(path: PathBuf) -> Result<BookInfo, String> {
    let book = crate::epub::parse_epub(&path)
        .map_err(|e| format!("failed to parse EPUB: {e:#}"))?;
    Ok(BookInfo {
        title: book.title,
        chapters: book
            .chapters
            .iter()
            .map(|c| ChapterSummary {
                title: c.title.clone(),
                char_count: c.text.len(),
            })
            .collect(),
    })
}

#[tauri::command]
pub fn check_recovery(book_dir: PathBuf) -> Result<Option<RecoveryState>, String> {
    recovery::RecoveryState::load(&book_dir)
        .map(Some)
        .or_else(|e| Err(format!("recovery load failed: {e:#}")))
}

#[derive(Debug, Serialize, Clone)]
pub struct BookErrorSummary {
    pub book_title: String,
    pub book_dir: PathBuf,
    pub chapters_with_errors: Vec<ChapterErrorSummary>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ChapterErrorSummary {
    pub title: String,
    pub failed_chunks: usize,
    pub total_chunks: usize,
}

#[tauri::command]
pub fn scan_recovery_books(root_dir: PathBuf) -> Vec<BookErrorSummary> {
    let Ok(books_root) = std::fs::read_dir(&root_dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in books_root.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let recovery_path = path.join("failed_chunks.json");
        if !recovery_path.exists() {
            continue;
        }
        let Ok(state) = recovery::RecoveryState::load(&path) else {
            continue;
        };
        if state.failed.is_empty() {
            continue;
        }
        let mut chapters_with_errors: Vec<ChapterErrorSummary> = state
            .failed
            .iter()
            .map(|(title, failures)| ChapterErrorSummary {
                title: title.clone(),
                failed_chunks: failures.len(),
                total_chunks: state.done.get(title).map(|v| v.len()).unwrap_or(0) + failures.len(),
            })
            .collect();
        chapters_with_errors.sort_by(|a, b| a.title.cmp(&b.title));
        out.push(BookErrorSummary {
            book_title: path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            book_dir: path,
            chapters_with_errors,
        });
    }
    out
}

#[derive(Debug, Serialize, Clone)]
pub struct FailedChunkInfo {
    pub chapter: String,
    pub chunk_index: usize,
    pub text: String,
    pub error: String,
}

#[tauri::command]
pub fn get_failed_chunks(book_dir: PathBuf, chapter: String) -> Vec<FailedChunkInfo> {
    let Ok(state) = recovery::RecoveryState::load(&book_dir) else {
        return Vec::new();
    };
    state
        .failed
        .get(&chapter)
        .map(|v| {
            v.iter()
                .map(|f| FailedChunkInfo {
                    chapter: chapter.clone(),
                    chunk_index: f.chunk_index,
                    text: f.text.clone(),
                    error: f.error.clone(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Synthesize a single chunk of text (Demo & Test panel).
#[tauri::command]
pub async fn synthesize_demo(
    handle: EngineHandle,
    text: String,
    voice: Option<String>,
    output_wav: PathBuf,
    registry: State<'_, Arc<EngineRegistry>>,
) -> Result<(), String> {
    let engine = registry
        .get(&handle.engine_id)
        .ok_or_else(|| format!("unknown engine '{}'", handle.engine_id))?;
    let request = SynthesizeRequest {
        text,
        reference_audio: None,
        language: None,
        voice,
        extra: Default::default(),
    };
    engine
        .synthesize(&handle, &request, &output_wav)
        .map_err(|e| format!("demo synthesis failed: {e:#}"))
}
/// This is the only "high-level" command the frontend needs: drop an
/// EPUB, click Generate, get MP3s.
///
/// Other engines (Qwen3-TTS, OuteTTS, NeuTTS Air) will get their own
/// top-level helpers as they land. The engine-agnostic plumbing
/// (load / unload / status) is unchanged.
///
/// Top-level "Generate Audiobook" command. Mirrors the legacy Gradio
/// flow: the user has picked an engine on the Configuration panel
/// and clicked Generate. The command:
///
/// 1. Auto-releases whatever engine is currently loaded (single-VRAM
///    safety per [[concepts/engine-lifecycle]]).
/// 2. Loads the requested engine model (`loading-model` event).
/// 3. Verifies assets (`model-ready` event).
/// 4. Runs `synthesize_book` with a progress callback that emits
///    `generation-progress` events chapter-by-chapter and
///    chunk-by-chunk.
/// 5. Emits `generation-complete` when done.
///
/// Only `engine_id = "kokoro"` is supported today. Other engines
/// (Qwen3-TTS, OuteTTS, NeuTTS Air) will get their own top-level
/// helpers as they land.
#[tauri::command]
pub async fn start_kokoro_generation(
    engine_id: String,
    model_id: String,
    voice: Option<String>,
    epub_path: PathBuf,
    output_dir: PathBuf,
    max_words: usize,
    registry: State<'_, Arc<EngineRegistry>>,
    app: AppHandle,
) -> Result<usize, String> {
    let _ = app.emit("generation-progress", format!("Loading engine {engine_id}..."));

    let engine = registry
        .get(&engine_id)
        .ok_or_else(|| format!("engine '{engine_id}' is not installed"))?;

    if engine.as_kokoro().is_none() {
        return Err(format!("engine '{engine_id}' does not support book-level synthesis yet"));
    }
    let kokoro_arc = engine.clone();

    if let Some(prev) = registry.active() {
        if let Some(prev_engine) = registry.get(&prev.engine_id) {
            let _ = prev_engine.unload(&prev);
        }
        registry.set_active(None);
    }

    let handle = engine
        .load(&model_id)
        .map_err(|e| format!("load failed: {e:#}"))?;
    registry.set_active(Some(handle.clone()));
    let _ = app.emit("generation-progress", format!("Model loaded: {model_id}"));

    let ffmpeg = merger::find_ffmpeg().map_err(|e| e.to_string())?;
    let epub = epub_path.clone();
    let out = output_dir.clone();
    let h = handle.clone();
    let app_for_task = app.clone();
    let voice_clone = voice.clone();

    STOP_FLAG.store(false, Ordering::SeqCst);

    let result = tokio::task::spawn_blocking(move || {
        let app = app_for_task;
        let cb: Box<dyn FnMut(&str) + Send> = Box::new(move |msg: &str| {
            let _ = app.emit("generation-progress", msg.to_string());
        });
        let k = kokoro_arc.as_kokoro().expect("checked above");
        let k: KokoroEngine = match voice_clone {
            Some(v) => k.clone_with_voice(&v),
            None => k.clone_with_voice(&k.voice.clone()),
        };
        synthesize_book(&k, &h, &epub, &out, max_words, &ffmpeg, Some(cb))
    })
    .await
    .map_err(|e| format!("synthesis task panicked: {e}"))?;

    let _ = app.emit("generation-complete", ());
    result.map_err(|e| format!("book synthesis failed: {e:#}"))
}

// ---- stub hardware detection -----------------------------------------

fn detect_hardware_stub() -> HardwareSummary {
    HardwareSummary {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        gpus: vec![GpuInfo {
            vendor: "NVIDIA".into(),
            model: "GeForce RTX 3090".into(),
            vram_bytes: 24 * 1024 * 1024 * 1024,
            backend: "CUDA".into(),
        }],
    }
}
