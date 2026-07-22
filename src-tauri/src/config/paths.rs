use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::RwLock;

static APP_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// User-chosen storage folder for heavy payloads (models, engines).
/// `None` means the default: everything lives in `app_data_dir`.
/// Persisted in `<app_data>/settings.json` (the settings file itself
/// stays in the fixed app data dir; only the GBs move).
static STORAGE_OVERRIDE: RwLock<Option<PathBuf>> = RwLock::new(None);

pub fn set_app_data_dir(path: PathBuf) {
    let _ = APP_DATA_DIR.set(path);
}

pub fn app_data_dir() -> PathBuf {
    APP_DATA_DIR.get().cloned().unwrap_or_else(|| {
        std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|p| p.join("com.patata.audiobookgenerator"))
            .unwrap_or_else(|_| {
                dirs::data_dir().unwrap_or_else(|| PathBuf::from("."))
            })
    })
}

fn settings_path() -> PathBuf {
    app_data_dir().join("settings.json")
}

/// Base folder for heavy payloads: the user override when set, otherwise
/// the app data dir.
pub fn storage_dir() -> PathBuf {
    STORAGE_OVERRIDE
        .read()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_else(app_data_dir)
}

/// Read `<app_data>/settings.json` and apply the stored storage override.
/// Called once at startup, after `set_app_data_dir`.
pub fn load_storage_override() {
    let path = settings_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        eprintln!("[paths] settings.json is not valid JSON, ignoring");
        return;
    };
    if let Some(dir) = json.get("storage_dir").and_then(|v| v.as_str()) {
        let dir = dir.trim();
        if !dir.is_empty() {
            eprintln!("[paths] storage override from settings: {}", dir);
            if let Ok(mut g) = STORAGE_OVERRIDE.write() {
                *g = Some(PathBuf::from(dir));
            }
        }
    }
}

/// Persist (or clear) the storage override in settings.json and apply it.
pub fn save_storage_override(dir: Option<PathBuf>) -> std::io::Result<()> {
    let settings = match &dir {
        Some(d) => serde_json::json!({ "storage_dir": d.to_string_lossy() }),
        None => serde_json::json!({}),
    };
    if let Ok(mut g) = STORAGE_OVERRIDE.write() {
        *g = dir;
    }
    std::fs::write(settings_path(), settings.to_string())
}

pub fn models_dir() -> PathBuf {
    storage_dir().join("models")
}

pub fn ffmpeg_dir() -> PathBuf {
    app_data_dir().join("bin")
}

pub fn reference_voices_dir() -> PathBuf {
    app_data_dir().join("Reference_Voices")
}

pub fn output_base_dir() -> PathBuf {
    app_data_dir().join("Generated_Audiobooks")
}

pub fn chunk_output_base_dir() -> PathBuf {
    app_data_dir().join("Intermediate_Audio_Chunks")
}

pub fn demo_output_dir() -> PathBuf {
    app_data_dir().join("Demo_Outputs")
}

pub fn tts_models_dir() -> PathBuf {
    app_data_dir().join("models")
}

pub fn kokoro_models_dir() -> PathBuf {
    models_dir().join("kokoro").join("models")
}

pub fn kokoro_voices_dir() -> PathBuf {
    models_dir().join("kokoro").join("voices")
}

pub fn qwen3tts_models_dir() -> PathBuf {
    models_dir().join("qwen3tts")
}

pub fn vibevoice_models_dir() -> PathBuf {
    models_dir().join("vibevoice")
}

pub fn xttsv2_models_dir() -> PathBuf {
    models_dir().join("xttsv2")
}

pub fn default_ffmpeg_exe() -> PathBuf {
    if cfg!(windows) {
        ffmpeg_dir().join("ffmpeg.exe")
    } else {
        ffmpeg_dir().join("ffmpeg")
    }
}