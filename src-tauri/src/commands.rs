use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::base_plugin::{EngineHandle, SynthesizeRequest, BaseTTSPlugin};
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

pub fn is_stop_requested() -> bool {
    STOP_FLAG.load(Ordering::SeqCst)
}

#[tauri::command]
pub fn engine_status(pm: State<'_, Arc<PluginManager>>) -> EngineStatus {
    let engines = pm.catalogue();
    EngineStatus {
        active_engine: None,
        active_model: None,
        vram_bytes: None,
        loaded_at: None,
        engines,
        hardware: detect_hardware_real(),
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

/// Get a plugin from the registry, or create one on-the-fly if the model
/// files are on disk but the plugin wasn't registered at startup
/// (e.g. after a model download without app restart).
fn get_or_create_plugin(
    engine_id: &str,
    pm: &PluginManager,
) -> Option<Arc<dyn BaseTTSPlugin>> {
    if let Some(p) = pm.get_plugin(engine_id) {
        return Some(p);
    }
    let qwen_paths = plugin_manager::QwenPaths::from_app_data(pm.app_data_dir());
    let qwen_plugin = crate::plugins::qwen3tts::QwenPlugin::new(qwen_paths, engine_id);
    if qwen_plugin.is_installed() {
        eprintln!("[commands] creating qwen plugin on-the-fly for {}", engine_id);
        return Some(Arc::new(qwen_plugin));
    }
    if engine_id.starts_with("VoxCPM2") {
        let vox_paths = plugin_manager::VoxCpm2Paths::from_app_data(pm.app_data_dir());
        let vox_plugin = crate::plugins::voxcpm2::VoxCpm2Plugin::new(vox_paths, engine_id);
        if vox_plugin.is_installed() {
            eprintln!("[commands] creating voxcpm2 plugin on-the-fly for {}", engine_id);
            return Some(Arc::new(vox_plugin));
        }
    }
    if engine_id.starts_with("OuteTTS") {
        let oute_dir = pm.app_data_dir().join("models").join("outetts");
        let oute_plugin = crate::plugins::outetts::OuteTTSPlugin::new(oute_dir, engine_id);
        if oute_plugin.is_installed() {
            eprintln!("[commands] creating outetts plugin on-the-fly for {}", engine_id);
            return Some(Arc::new(oute_plugin));
        }
    }
    None
}

#[tauri::command]
pub async fn synthesize_demo(
    engine_id: String,
    text: String,
    voice: Option<String>,
    language: Option<String>,
    speed: Option<f32>,
    output_wav: PathBuf,
    extra: Option<std::collections::HashMap<String, String>>,
    reference_audio: Option<String>,
    max_chars: Option<usize>,
    max_words: Option<usize>,
    pm: State<'_, Arc<PluginManager>>,
) -> Result<String, String> {
    let output_wav = resolve_output_path(&output_wav);
    let plugin = get_or_create_plugin(&engine_id, &pm)
        .ok_or_else(|| format!("engine '{}' is not installed or model files missing", engine_id))?;
    let handle = plugin
        .load_model(&engine_id)
        .await
        .map_err(|e| format!("load failed: {e:#}"))?;

    let defaults = plugin_manager::defaults_for(&engine_id);
    let effective_max_chars = max_chars.unwrap_or(defaults.chunk_max_chars as usize);
    let effective_max_words = max_words.unwrap_or(1000);
    let chunks = crate::chunker::chunk_text(&text, effective_max_words, effective_max_chars);
    if chunks.is_empty() {
        return Err("empty text after chunking".to_string());
    }

    let temp_dir = output_wav
        .parent()
        .map(|p| p.join("_demo_chunks"))
        .unwrap_or_else(|| PathBuf::from("_demo_chunks"));
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("create temp dir: {e}"))?;

    let mut chunk_wavs: Vec<PathBuf> = Vec::new();
    for (i, chunk_text) in chunks.iter().enumerate() {
        let chunk_path = temp_dir.join(format!("demo_chunk_{:04}.wav", i + 1));
        let mut req_extra = extra.clone().unwrap_or_default();
        if let Some(s) = speed {
            req_extra.insert("speed".to_string(), s.to_string());
        }
        let request = SynthesizeRequest {
            text: chunk_text.clone(),
            output_path: chunk_path.to_string_lossy().to_string(),
            reference_audio: reference_audio.clone(),
            language: language.clone(),
            voice: voice.clone(),
            extra: req_extra,
        };
        plugin
            .synthesize(&handle, &request)
            .await
            .map_err(|e| format!("demo synthesis failed on chunk {}: {e:#}", i + 1))?;
        chunk_wavs.push(chunk_path);
    }

    let _ = plugin.unload(&handle).await;

    if chunk_wavs.len() == 1 {
        std::fs::rename(&chunk_wavs[0], &output_wav)
            .map_err(|e| format!("rename demo wav: {e}"))?;
    } else {
        let ffmpeg = merger::find_ffmpeg().map_err(|e| format!("find ffmpeg: {e}"))?;
        merger::merge_wavs_to_wav(&chunk_wavs, &output_wav, &ffmpeg)
            .map_err(|e| format!("merge demo chunks: {e}"))?;
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
    let abs = output_wav.canonicalize().unwrap_or(output_wav);
    Ok(abs.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn start_generation(
    engine_id: String,
    voice: Option<String>,
    language: Option<String>,
    speed: Option<f32>,
    epub_path: PathBuf,
    output_dir: PathBuf,
    max_words: usize,
    max_chars: Option<usize>,
    extra: Option<std::collections::HashMap<String, String>>,
    reference_audio: Option<String>,
    pm: State<'_, Arc<PluginManager>>,
    app: AppHandle,
) -> Result<usize, String> {
    let output_dir = resolve_output_path(&output_dir);
    let _ = app.emit("generation-progress", format!("Loading engine {}...", engine_id));

    let plugin = get_or_create_plugin(&engine_id, &pm)
        .ok_or_else(|| format!("engine '{}' is not installed or model files missing", engine_id))?;

    let _handle = plugin
        .load_model(&engine_id)
        .await
        .map_err(|e| format!("load failed: {e:#}"))?;

    let _ = app.emit("generation-progress", format!("Model loaded: {}", engine_id));

    let ffmpeg = merger::find_ffmpeg().map_err(|e| e.to_string())?;
    let epub = epub_path.clone();
    let out = output_dir.clone();
    let app_for_task = app.clone();
    let extra_map = extra.unwrap_or_default();

    STOP_FLAG.store(false, Ordering::SeqCst);

    let max_chars_resolved = max_chars.unwrap_or(800);

    // Qwen3-TTS path
    let qwen_any = plugin.as_any();
    if let Some(qwen_plugin) = qwen_any.downcast_ref::<crate::plugins::qwen3tts::QwenPlugin>() {
        let variant_name = qwen_plugin.variant_name.clone();
        let qwen_paths = qwen_plugin.paths.clone();
        let voice_task = voice.clone();
        let lang_task = language.clone();
        let ref_audio_task = reference_audio.clone();
        let extra_task = extra_map.clone();

        let result = tokio::task::spawn_blocking(move || {
            let q = crate::plugins::qwen3tts::QwenPlugin::new(qwen_paths, &variant_name);
            let cb: Box<dyn FnMut(&str) + Send> = Box::new(move |msg: &str| {
                let _ = app_for_task.emit("generation-progress", msg.to_string());
            });
            crate::plugins::qwen3tts::synthesize_book(
                &q, &epub, &out, max_words, max_chars_resolved, &ffmpeg,
                voice_task.as_deref(),
                lang_task.as_deref(),
                ref_audio_task.as_deref(),
                &extra_task,
                Some(cb),
            )
        })
        .await
        .map_err(|e| format!("synthesis task panicked: {e}"))?;

        let _ = app.emit("generation-complete", ());
        return result.map_err(|e| format!("book synthesis failed: {e:#}"));
    }

    // VoxCPM2 path
    let vox_any = plugin.as_any();
    if let Some(vox_plugin) = vox_any.downcast_ref::<crate::plugins::voxcpm2::VoxCpm2Plugin>() {
        let variant_name = vox_plugin.variant_name.clone();
        let vox_paths = vox_plugin.paths.clone();
        let ref_audio_task = reference_audio.clone();
        let extra_task = extra_map.clone();

        let result = tokio::task::spawn_blocking(move || {
            let p = crate::plugins::voxcpm2::VoxCpm2Plugin::new(vox_paths, &variant_name);
            let cb: Box<dyn FnMut(&str) + Send> = Box::new(move |msg: &str| {
                let _ = app_for_task.emit("generation-progress", msg.to_string());
            });
            crate::plugins::voxcpm2::synthesize_book(
                &p, &epub, &out, max_words, max_chars_resolved, &ffmpeg,
                ref_audio_task.as_deref(),
                &extra_task,
                Some(cb),
            )
        })
        .await
        .map_err(|e| format!("synthesis task panicked: {e}"))?;

        let _ = app.emit("generation-complete", ());
        return result.map_err(|e| format!("book synthesis failed: {e:#}"));
    }

    // OuteTTS path
    let oute_any = plugin.as_any();
    if let Some(oute_plugin) = oute_any.downcast_ref::<crate::plugins::outetts::OuteTTSPlugin>() {
        let models_dir = oute_plugin.models_dir.clone();
        let variant_name = oute_plugin.variant_name.clone();
        let extra_task = extra_map.clone();

        let result = tokio::task::spawn_blocking(move || {
            let p = crate::plugins::outetts::OuteTTSPlugin::new(models_dir, &variant_name);
            let cb: Box<dyn FnMut(&str) + Send> = Box::new(move |msg: &str| {
                let _ = app_for_task.emit("generation-progress", msg.to_string());
            });
            crate::plugins::outetts::synthesize_book(
                &p, &epub, &out, max_words, max_chars_resolved, &ffmpeg,
                &extra_task,
                Some(cb),
            )
        })
        .await
        .map_err(|e| format!("synthesis task panicked: {e}"))?;

        let _ = app.emit("generation-complete", ());
        return result.map_err(|e| format!("book synthesis failed: {e:#}"));
    }

    Err(format!("engine '{}' synthesis not yet implemented", engine_id))
}

fn resolve_output_path(path: &std::path::Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        crate::config::paths::app_data_dir().join(path)
    }
}

#[tauri::command]
pub fn get_default_output_dir(kind: String) -> String {
    match kind.as_str() {
        "demo" => crate::config::paths::demo_output_dir(),
        _ => crate::config::paths::output_base_dir(),
    }
    .to_string_lossy()
    .to_string()
}

fn map_language_to_test_suffix(lang: Option<&str>) -> String {
    let l = match lang {
        Some(l) => l.trim(),
        None => return "en".to_string(),
    };
    if l.is_empty() {
        return "en".to_string();
    }
    let mapping: &[(&str, &str)] = &[
        ("zh-cn", "cn"), ("zh-CN", "cn"), ("chinese", "cn"),
        ("english", "en"), ("german", "de"), ("italian", "it"),
        ("portuguese", "pt"), ("spanish", "es"), ("japanese", "ja"),
        ("korean", "ko"), ("french", "fr"), ("russian", "ru"),
        ("auto", "en"),
        ("it", "it"), ("en", "en"), ("es", "es"), ("fr", "fr"),
        ("de", "de"), ("pt", "pt"), ("pl", "pl"), ("ru", "ru"),
        ("ja", "ja"), ("hu", "hu"), ("ko", "ko"), ("hi", "hi"),
        ("ar", "ar"), ("nl", "nl"), ("cs", "cs"), ("tr", "tr"),
    ];
    let lower = l.to_lowercase();
    for &(key, val) in mapping {
        if lower == key.to_lowercase() {
            return val.to_string();
        }
    }
    if l.contains('-') {
        let parts: Vec<&str> = l.splitn(2, '-').collect();
        for &p in &parts {
            for &(key, val) in mapping {
                if p.eq_ignore_ascii_case(key) {
                    return val.to_string();
                }
            }
        }
    }
    if lower.len() >= 2 {
        let prefix = &lower[..2];
        let prefix_map: &[(&str, &str)] = &[
            ("zh", "cn"), ("cn", "cn"), ("en", "en"), ("it", "it"),
            ("es", "es"), ("fr", "fr"), ("de", "de"), ("pt", "pt"),
            ("pl", "pl"), ("ru", "ru"), ("ja", "ja"), ("ko", "ko"),
        ];
        for &(key, val) in prefix_map {
            if prefix == key {
                return val.to_string();
            }
        }
    }
    "en".to_string()
}

fn find_test_files_dir() -> Option<PathBuf> {
    let candidates = [
        std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("test_files"))),
        std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("..").join("test_files"))),
        std::env::current_dir().ok().map(|d| d.join("test_files")),
        std::env::current_dir().ok().map(|d| d.join("..").join("test_files")),
    ];
    for c in &candidates {
        if let Some(ref p) = c {
            if p.is_dir() {
                return Some(p.clone());
            }
        }
    }
    None
}

#[tauri::command]
pub fn get_test_epub(language: Option<String>) -> Result<PathBuf, String> {
    let suffix = map_language_to_test_suffix(language.as_deref());
    let dir = find_test_files_dir()
        .ok_or_else(|| "test_files directory not found".to_string())?;

    let primary = dir.join(format!("test_ebook_{}.epub", suffix));
    if primary.exists() {
        return Ok(primary);
    }

    let fallback = dir.join("test_ebook_en.epub");
    if fallback.exists() {
        return Ok(fallback);
    }

    Err(format!(
        "No test EPUB found in {} for lang suffix '{}' or fallback 'en'",
        dir.display(), suffix
    ))
}

#[tauri::command]
pub fn list_mp3s_in_dir(dir: String) -> Result<Vec<String>, String> {
    let abs = std::path::Path::new(&dir)
        .canonicalize()
        .map_err(|e| format!("cannot resolve '{}': {}", dir, e))?;
    if !abs.is_dir() {
        return Err(format!("'{}' is not a directory", abs.display()));
    }
    let mut mp3s: Vec<String> = std::fs::read_dir(&abs)
        .map_err(|e| format!("cannot read '{}': {}", abs.display(), e))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("mp3"))
                .unwrap_or(false)
        })
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();
    mp3s.sort();
    Ok(mp3s)
}

fn detect_hardware_real() -> HardwareSummary {
    let hw = crate::wizard::detect_hardware_blocking();
    HardwareSummary {
        os: hw.os,
        arch: hw.arch,
        gpus: hw
            .gpus
            .into_iter()
            .map(|g| GpuInfo {
                vendor: g.vendor,
                model: g.model,
                vram_bytes: g.vram_bytes,
                backend: g.backend,
            })
            .collect(),
    }
}

#[tauri::command]
pub fn list_models(app: AppHandle) -> Vec<crate::model_manager::ModelListEntry> {
    crate::model_manager::list_models(&app)
}

#[tauri::command]
pub fn is_model_installed(name: String, app: AppHandle) -> bool {
    crate::model_manager::is_model_installed(&name, &app)
}

#[tauri::command]
pub async fn download_model(
    name: String,
    app: AppHandle,
) -> Result<crate::model_manager::ModelDownloadResult, String> {
    let result = crate::model_manager::download_model(&name, &app).await?;
    // Note: the plugin manager holds an Arc<PluginManager> in app state
    // and cannot be refreshed from inside an async command (would need
    // a Mutex). The next engine_status call after download will rescan
    // the disk and re-register the Kokoro plugin automatically.
    Ok(result)
}

#[tauri::command]
pub fn remove_model(
    name: String,
    app: AppHandle,
) -> Result<(), String> {
    crate::model_manager::remove_model(&name, &app)
}