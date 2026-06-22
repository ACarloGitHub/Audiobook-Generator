use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::base_plugin::{EngineHandle, SynthesizeRequest};
use crate::merger;
use crate::plugin_manager::{self, EngineDefaults, EngineInfo, PluginManager};
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
pub fn engine_status(pm: State<'_, Arc<PluginManager>>) -> EngineStatus {
    let engines = pm.catalogue();
    EngineStatus {
        active_engine: None,
        active_model: None,
        vram_bytes: None,
        loaded_at: None,
        engines,
        hardware: detect_hardware_stub(),
    }
}

#[tauri::command]
pub fn engine_defaults(engine_id: String) -> EngineDefaults {
    plugin_manager::defaults_for(&engine_id)
}

#[tauri::command]
pub fn load_engine(
    engine_id: String,
    model_id: String,
    pm: State<'_, Arc<PluginManager>>,
) -> Result<EngineHandle, String> {
    let plugin = pm
        .get_plugin(&engine_id)
        .ok_or_else(|| format!("unknown engine '{engine_id}'"))?;

    tauri::async_runtime::block_on(plugin.load_model(&model_id))
        .map_err(|e| format!("load failed: {e:#}"))
}

#[tauri::command]
pub async fn unload_engine(
    engine_id: String,
    model_id: String,
    pm: State<'_, Arc<PluginManager>>,
) -> Result<(), String> {
    let handle = EngineHandle {
        engine_id,
        model_id,
    };
    if let Some(plugin) = pm.get_plugin(&handle.engine_id) {
        plugin
            .unload(&handle)
            .await
            .map_err(|e| format!("unload failed: {e:#}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn stop_generation() {
    STOP_FLAG.store(true, Ordering::SeqCst);
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

#[tauri::command]
pub async fn synthesize_demo(
    text: String,
    voice: Option<String>,
    output_wav: PathBuf,
    pm: State<'_, Arc<PluginManager>>,
) -> Result<(), String> {
    let plugin = pm
        .get_plugin("kokoro")
        .ok_or_else(|| "Kokoro engine not available".to_string())?;
    let handle = plugin
        .load_model("kokoro-82M-quantized")
        .await
        .map_err(|e| format!("load failed: {e:#}"))?;
    let request = SynthesizeRequest {
        text,
        output_path: output_wav.to_string_lossy().to_string(),
        reference_audio: None,
        language: None,
        voice,
        extra: Default::default(),
    };
    plugin
        .synthesize(&handle, &request)
        .await
        .map_err(|e| format!("demo synthesis failed: {e:#}"))?;
    let _ = plugin.unload(&handle).await;
    Ok(())
}

#[tauri::command]
pub async fn start_kokoro_generation(
    voice: Option<String>,
    epub_path: PathBuf,
    output_dir: PathBuf,
    max_words: usize,
    pm: State<'_, Arc<PluginManager>>,
    app: AppHandle,
) -> Result<usize, String> {
    let _ = app.emit("generation-progress", "Loading engine kokoro...");

    let plugin = pm
        .get_plugin("kokoro")
        .ok_or_else(|| "Kokoro engine not installed".to_string())?;

    let handle = plugin
        .load_model("kokoro-82M-quantized")
        .await
        .map_err(|e| format!("load failed: {e:#}"))?;

    let _ = app.emit("generation-progress", "Model loaded: kokoro-82M-quantized");

    let ffmpeg = merger::find_ffmpeg().map_err(|e| e.to_string())?;
    let epub = epub_path.clone();
    let out = output_dir.clone();
    let voice_clone = voice.clone();
    let app_for_task = app.clone();

    STOP_FLAG.store(false, Ordering::SeqCst);

    let kokoro_any = plugin.as_any();
    let kokoro_plugin = kokoro_any
        .downcast_ref::<crate::plugins::kokoro::KokoroPlugin>()
        .ok_or_else(|| "internal: kokoro plugin type mismatch".to_string())?;

    let voice_for_task = voice.unwrap_or_else(|| kokoro_plugin.voice.clone());
    let kokoro_paths = kokoro_plugin.paths.clone();

    let result = tokio::task::spawn_blocking(move || {
        let k = crate::plugins::kokoro::KokoroPlugin::new(kokoro_paths, &voice_for_task);
        let cb: Box<dyn FnMut(&str) + Send> = Box::new(move |msg: &str| {
            let _ = app_for_task.emit("generation-progress", msg.to_string());
        });
        crate::plugins::kokoro::synthesize_book(&k, &epub, &out, max_words, &ffmpeg, Some(cb))
    })
    .await
    .map_err(|e| format!("synthesis task panicked: {e}"))?;

    let _ = app.emit("generation-complete", ());
    result.map_err(|e| format!("book synthesis failed: {e:#}"))
}

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