use std::path::PathBuf;
use std::process::Command;

use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize)]
pub struct HardwareInfo {
    pub os: String,
    pub arch: String,
    pub cpu_cores: usize,
    pub ram_total_gb: f64,
    pub ram_free_gb: f64,
    pub gpu_vendor: Option<String>,
    pub gpu_model: Option<String>,
    pub gpu_vram_bytes: Option<u64>,
    pub gpu_driver_version: Option<String>,
    pub recommended_backend: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DependencyStatus {
    pub ffmpeg_installed: bool,
    pub ffmpeg_path: Option<String>,
    pub llama_server_installed: bool,
    pub llama_server_path: Option<String>,
    pub ort_installed: bool,
    pub cudnn_installed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WizardStep {
    pub id: String,
    pub title: String,
    pub description: String,
    pub completed: bool,
}

fn command_exists(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}

fn detect_nvidia_smi() -> Option<String> {
    let output = Command::new("nvidia-smi")
        .arg("--query-gpu=driver_version")
        .arg("--format=csv,noheader")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let version = String::from_utf8_lossy(&output.stdout);
    Some(version.trim().to_string())
}

fn detect_gpu_model() -> Option<String> {
    let output = Command::new("nvidia-smi")
        .arg("--query-gpu=name")
        .arg("--format=csv,noheader")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&output.stdout);
    Some(name.trim().to_string())
}

fn detect_gpu_vram() -> Option<u64> {
    let output = Command::new("nvidia-smi")
        .arg("--query-gpu=memory.total")
        .arg("--format=csv,noheader,nounits")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mb: u64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .ok()?;
    Some(mb * 1024 * 1024)
}

fn recommended_backend() -> String {
    if cfg!(target_os = "windows") {
        if detect_nvidia_smi().is_some() {
            let driver = detect_nvidia_smi().unwrap_or_default();
            let major: u32 = driver
                .split('.')
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            if major >= 525 {
                return "CUDA".to_string();
            } else {
                return "Vulkan".to_string();
            }
        }
        "Vulkan".to_string()
    } else if cfg!(target_os = "macos") {
        "Vulkan".to_string()
    } else {
        if detect_nvidia_smi().is_some() {
            "Vulkan".to_string()
        } else {
            "CPU".to_string()
        }
    }
}

#[tauri::command]
pub fn detect_hardware() -> HardwareInfo {
    let os = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    let cpu_cores = num_cpus::get();

    let sys = sysinfo::System::new_all();
    let ram_total_kb = sys.total_memory() / 1024;
    let ram_free_kb = sys.available_memory() / 1024;
    let ram_total_gb = (ram_total_kb as f64) / 1024.0 / 1024.0;
    let ram_free_gb = (ram_free_kb as f64) / 1024.0 / 1024.0;

    let (gpu_vendor, gpu_model, gpu_vram_bytes, gpu_driver_version) =
        if detect_nvidia_smi().is_some() {
            (
                Some("NVIDIA".to_string()),
                detect_gpu_model(),
                detect_gpu_vram(),
                detect_nvidia_smi(),
            )
        } else {
            (None, None, None, None)
        };

    let recommended_backend = recommended_backend();

    HardwareInfo {
        os,
        arch,
        cpu_cores,
        ram_total_gb: (ram_total_gb * 100.0).round() / 100.0,
        ram_free_gb: (ram_free_gb * 100.0).round() / 100.0,
        gpu_vendor,
        gpu_model,
        gpu_vram_bytes,
        gpu_driver_version,
        recommended_backend,
    }
}

#[tauri::command]
pub fn check_dependencies(app: AppHandle) -> DependencyStatus {
    let app_data = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
    let bin_dir = app_data.join("bin");

    let ffmpeg_installed = command_exists("ffmpeg")
        || bin_dir.join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" }).exists();
    let ffmpeg_path = which::which("ffmpeg")
        .ok()
        .or_else(|| {
            let p = bin_dir.join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" });
            if p.exists() { Some(p) } else { None }
        })
        .map(|p| p.to_string_lossy().to_string());

    let llama_server_installed = command_exists("llama-server")
        || bin_dir.join(if cfg!(windows) { "llama-server.exe" } else { "llama-server" }).exists();
    let llama_server_path = which::which("llama-server")
        .ok()
        .or_else(|| {
            let p = bin_dir.join(if cfg!(windows) { "llama-server.exe" } else { "llama-server" });
            if p.exists() { Some(p) } else { None }
        })
        .map(|p| p.to_string_lossy().to_string());

    let ort_installed = {
        let kokoro_models = app_data.join("models").join("kokoro").join("models");
        kokoro_models.join("model_quantized.onnx").exists()
            || kokoro_models.join("model_q8f16.onnx").exists()
            || kokoro_models.join("model.onnx").exists()
    };

    let cudnn_installed = if cfg!(windows) {
        let system32 = PathBuf::from("C:\\Windows\\System32");
        system32.join("cudnn64_9.dll").exists()
            || system32.join("cudnn_ops_infer64_9.dll").exists()
    } else {
        let lib_paths = ["/usr/lib/x86_64-linux-gnu/libcudnn.so.9", "/usr/local/cuda/lib64/libcudnn.so.9"];
        lib_paths.iter().any(|p| PathBuf::from(p).exists())
    };

    DependencyStatus {
        ffmpeg_installed,
        ffmpeg_path,
        llama_server_installed,
        llama_server_path,
        ort_installed,
        cudnn_installed,
    }
}

#[tauri::command]
pub fn get_wizard_steps() -> Vec<WizardStep> {
    vec![
        WizardStep {
            id: "welcome".into(),
            title: "Welcome".into(),
            description: "Audiobook Generator needs a few system components to work. This wizard will help you install them.".into(),
            completed: false,
        },
        WizardStep {
            id: "hardware".into(),
            title: "Hardware Detection".into(),
            description: "Detecting your GPU and recommended compute backend.".into(),
            completed: false,
        },
        WizardStep {
            id: "ffmpeg".into(),
            title: "FFmpeg".into(),
            description: "FFmpeg is required to merge audio chunks into MP3 files.".into(),
            completed: false,
        },
        WizardStep {
            id: "llama_server".into(),
            title: "llama-server".into(),
            description: "llama-server is the inference engine for GGUF models (Qwen3-TTS, OuteTTS, NeuTTS Air, VibeVoice).".into(),
            completed: false,
        },
        WizardStep {
            id: "ort".into(),
            title: "ONNX Runtime + cuDNN".into(),
            description: "ONNX Runtime and cuDNN are required for Kokoro (in-process synthesis via kokoro-en).".into(),
            completed: false,
        },
        WizardStep {
            id: "done".into(),
            title: "Setup Complete".into(),
            description: "All system dependencies are installed. You can now download TTS models from the Models panel.".into(),
            completed: false,
        },
    ]
}

#[tauri::command]
pub fn download_ffmpeg(app: AppHandle) -> Result<String, String> {
    if command_exists("ffmpeg") {
        return Ok("FFmpeg is already available on PATH.".into());
    }
    let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let bin_dir = app_data.join("bin");
    std::fs::create_dir_all(&bin_dir).map_err(|e| e.to_string())?;

    Err("FFmpeg auto-download not yet implemented. Please install FFmpeg manually: https://ffmpeg.org/download.html".into())
}

#[tauri::command]
pub fn download_llama_server(app: AppHandle) -> Result<String, String> {
    if command_exists("llama-server") {
        return Ok("llama-server is already available on PATH.".into());
    }
    let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let bin_dir = app_data.join("bin");
    std::fs::create_dir_all(&bin_dir).map_err(|e| e.to_string())?;

    Err("llama-server auto-download not yet implemented. Please install llama.cpp manually: https://github.com/ggergan/llama.cpp".into())
}

#[tauri::command]
pub fn is_wizard_done(app: AppHandle) -> bool {
    let app_data = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
    app_data.join(".wizard_done").exists()
}

#[tauri::command]
pub fn mark_wizard_done(app: AppHandle) -> Result<(), String> {
    let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::write(app_data.join(".wizard_done"), "done").map_err(|e| e.to_string())
}