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
use tauri::State;

use crate::engines::kokoro::synthesize_book;
use crate::engines::{EngineHandle, EngineInfo, EngineRegistry, SynthesizeRequest};
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
    let engines = registry.list();
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
pub fn check_recovery(book_dir: PathBuf) -> Result<Option<RecoveryState>, String> {
    recovery::RecoveryState::load(&book_dir)
        .map(Some)
        .or_else(|e| Err(format!("recovery load failed: {e:#}")))
}

/// Synthesize an entire book with the currently loaded Kokoro engine.
/// This is the only "high-level" command the frontend needs: drop an
/// EPUB, click Generate, get MP3s.
///
/// Other engines (Qwen3-TTS, OuteTTS, NeuTTS Air) will get their own
/// top-level helpers as they land. The engine-agnostic plumbing
/// (load / unload / status) is unchanged.
#[tauri::command]
pub async fn start_kokoro_generation(
    handle: EngineHandle,
    epub_path: PathBuf,
    output_dir: PathBuf,
    max_words: usize,
    registry: State<'_, Arc<EngineRegistry>>,
) -> Result<usize, String> {
    let engine = registry
        .get(&handle.engine_id)
        .ok_or_else(|| format!("unknown engine '{}'", handle.engine_id))?;

    // KokoroEngine is the only engine that exposes the book-level
    // helper. Other engines (Qwen3-TTS, OuteTTS, NeuTTS) will get
    // their own top-level Tauri command.
    let kokoro = engine
        .as_kokoro()
        .ok_or_else(|| "this engine does not support book-level synthesis yet".to_string())?;

    let ffmpeg = merger::find_ffmpeg().map_err(|e| e.to_string())?;
    synthesize_book(kokoro, &handle, &epub_path, &output_dir, max_words, &ffmpeg)
        .map_err(|e| format!("book synthesis failed: {e:#}"))
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
