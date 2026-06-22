use std::path::PathBuf;
use std::sync::OnceLock;

static APP_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

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

pub fn models_dir() -> PathBuf {
    app_data_dir().join("models")
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