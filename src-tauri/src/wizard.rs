use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::sidecars;

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
pub struct GpuEntry {
    pub vendor: String,
    pub model: String,
    pub vram_bytes: u64,
    pub backend: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HardwareSummary {
    pub os: String,
    pub arch: String,
    pub gpus: Vec<GpuEntry>,
}

pub fn detect_hardware_blocking() -> HardwareSummary {
    let hw = detect_hardware_impl();
    let gpus = if let (Some(vendor), Some(model), Some(vram)) =
        (hw.gpu_vendor.clone(), hw.gpu_model.clone(), hw.gpu_vram_bytes)
    {
        vec![GpuEntry {
            vendor,
            model,
            vram_bytes: vram,
            backend: hw.recommended_backend.clone(),
        }]
    } else {
        Vec::new()
    };
    HardwareSummary {
        os: hw.os,
        arch: hw.arch,
        gpus,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DependencyStatus {
    pub ffmpeg_installed: bool,
    pub ffmpeg_path: Option<String>,
    pub llama_server_installed: bool,
    pub llama_server_path: Option<String>,
    pub qwentts_installed: bool,
    pub qwentts_path: Option<String>,
    pub voxcpm2_installed: bool,
    pub voxcpm2_path: Option<String>,
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

fn silent_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}

fn detect_nvidia_smi() -> Option<String> {
    let output = silent_command("nvidia-smi")
        .args(["--query-gpu=driver_version", "--format=csv,noheader"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let version = String::from_utf8_lossy(&output.stdout);
    Some(version.trim().to_string())
}

fn detect_gpu_model() -> Option<String> {
    let output = silent_command("nvidia-smi")
        .args(["--query-gpu=name", "--format=csv,noheader"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&output.stdout);
    Some(name.trim().to_string())
}

fn detect_gpu_vram() -> Option<u64> {
    let output = silent_command("nvidia-smi")
        .args(["--query-gpu=memory.total", "--format=csv,noheader,nounits"])
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
        "Metal".to_string()
    } else {
        if detect_nvidia_smi().is_some() {
            "CUDA".to_string()
        } else {
            "CPU".to_string()
        }
    }
}

#[tauri::command]
pub fn detect_hardware() -> HardwareInfo {
    detect_hardware_impl()
}

fn detect_hardware_impl() -> HardwareInfo {
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
pub fn check_dependencies(_app: AppHandle) -> DependencyStatus {
    let ffmpeg_exe = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    // ffmpeg ships inside the installer (resources/ffmpeg); PATH and the
    // legacy per-user bin dir are kept as fallbacks for dev setups.
    let ffmpeg_path = which::which("ffmpeg")
        .ok()
        .or_else(|| sidecars::sidecar_binary("ffmpeg", ffmpeg_exe))
        .or_else(|| {
            let p = crate::config::paths::default_ffmpeg_exe();
            if p.exists() { Some(p) } else { None }
        });
    let ffmpeg_installed = ffmpeg_path.is_some();

    let llama_exe = if cfg!(windows) { "llama-server.exe" } else { "llama-server" };
    let llama_server_path = which::which("llama-server")
        .ok()
        .or_else(|| sidecars::sidecar_binary("llama.cpp", llama_exe));
    let llama_installed = llama_server_path.is_some();

    let qwen_exe = if cfg!(windows) { "qwen-tts.exe" } else { "qwen-tts" };
    let qwentts_path = sidecars::sidecar_binary("qwentts", qwen_exe);
    let qwentts_installed = qwentts_path.is_some();

    let vox_exe = if cfg!(windows) { "voxcpm2-cli.exe" } else { "voxcpm2-cli" };
    let voxcpm2_path = sidecars::sidecar_binary("voxcpm2", vox_exe);
    let voxcpm2_installed = voxcpm2_path.is_some();

    // ONNX Runtime is a Rust dependency (ort crate); if the app is running it is available.
    let ort_installed = true;

    let llama_dir = sidecars::sidecar_dir("llama.cpp");
    let cudnn_installed = if cfg!(target_os = "windows") {
        let system32 = PathBuf::from("C:\\Windows\\System32");
        system32.join("cudnn64_9.dll").exists()
            || system32.join("cudnn_ops_infer64_9.dll").exists()
            || llama_dir
                .as_ref()
                .map(|d| d.join("cublas64_12.dll").exists() || d.join("cudart64_12.dll").exists())
                .unwrap_or(false)
    } else {
        let lib_paths = ["/usr/lib/x86_64-linux-gnu/libcudnn.so.9", "/usr/local/cuda/lib64/libcudnn.so.9"];
        lib_paths.iter().any(|p| PathBuf::from(p).exists())
    };

    DependencyStatus {
        ffmpeg_installed,
        ffmpeg_path: ffmpeg_path.map(|p| p.to_string_lossy().to_string()),
        llama_server_installed: llama_installed,
        llama_server_path: llama_server_path.map(|p| p.to_string_lossy().to_string()),
        qwentts_installed,
        qwentts_path: qwentts_path.map(|p| p.to_string_lossy().to_string()),
        voxcpm2_installed,
        voxcpm2_path: voxcpm2_path.map(|p| p.to_string_lossy().to_string()),
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
            description: "Everything needed to run is bundled in the installer. This wizard checks your hardware and guides you to the first model download.".into(),
            completed: false,
        },
        WizardStep {
            id: "hardware".into(),
            title: "Hardware Detection".into(),
            description: "Detecting your GPU and recommended compute backend.".into(),
            completed: false,
        },
        WizardStep {
            id: "components".into(),
            title: "Bundled Components".into(),
            description: "Verifying the engine binaries shipped with the installer (ffmpeg, llama-server, qwen-tts, voxcpm2-cli).".into(),
            completed: false,
        },
        WizardStep {
            id: "done".into(),
            title: "Setup Complete".into(),
            description: "All components are in place. You can now download TTS models from the Models panel.".into(),
            completed: false,
        },
    ]
}

#[tauri::command]
pub fn is_wizard_done(app: AppHandle) -> bool {
    let app_data = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
    app_data.join(".wizard_done").exists()
}

#[tauri::command]
pub fn mark_wizard_done(app: AppHandle) -> Result<(), String> {
    let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::write(app_data.join(".wizard_done"), "done").map_err(|e| e.to_string())
}

// =============================================================================
// Download with resume + structured progress (from AuraWrite pattern)
// =============================================================================

pub(crate) async fn download_to_file_async(
    app: &AppHandle,
    id: &str,
    name: &str,
    url: &str,
    dest: &Path,
) -> Result<u64, String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create dir: {}", e))?;
    }

    let part_file = dest.with_extension(format!(
        "{}.part",
        dest.extension().map_or(String::new(), |e| e.to_string_lossy().to_string())
    ));

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("http client: {}", e))?;

    // Check for partial download to resume
    let partial_size: u64 = if part_file.exists() {
        fs::metadata(&part_file).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    if partial_size > 0 {
        let _ = app.emit("download-progress", serde_json::json!({
            "id": id, "name": name, "phase": "resuming",
            "bytes": partial_size, "total": 0, "speed_bps": 0, "eta_seconds": null
        }));
        let resp = client
            .get(url)
            .header("Accept-Encoding", "identity")
            .header("Range", format!("bytes={}-", partial_size))
            .send()
            .await
            .map_err(|e| {
                let _ = fs::remove_file(&part_file);
                format!("resume request failed: {}", e)
            })?;

        if resp.status() == reqwest::StatusCode::PARTIAL_CONTENT {
            let f = fs::OpenOptions::new().append(true).open(&part_file)
                .map_err(|e| format!("open partial file for append: {}", e))?;
            return download_stream_to_file(
                app, id, name, resp, &part_file, dest,
                partial_size, Some(f),
            ).await;
        } else if resp.status().is_success() {
            let _ = fs::remove_file(&part_file);
            let f = File::create(&part_file)
                .map_err(|e| format!("create partial file: {}", e))?;
            return download_stream_to_file(
                app, id, name, resp, &part_file, dest,
                0, Some(f),
            ).await;
        } else {
            let _ = fs::remove_file(&part_file);
            let _ = app.emit("download-progress", serde_json::json!({
                "id": id, "name": name, "phase": "error",
                "error": format!("HTTP {} for {}", resp.status(), url),
                "bytes": 0, "total": 0, "speed_bps": 0, "eta_seconds": null
            }));
            return Err(format!("HTTP {} for {}", resp.status(), url));
        }
    }

    // Fresh download
    let _ = app.emit("download-progress", serde_json::json!({
        "id": id, "name": name, "phase": "downloading",
        "bytes": 0, "total": 0, "speed_bps": 0, "eta_seconds": null
    }));
    let resp = client
        .get(url)
        .header("Accept-Encoding", "identity")
        .send()
        .await
        .map_err(|e| format!("download failed: {}", e))?;
    if !resp.status().is_success() {
        let _ = app.emit("download-progress", serde_json::json!({
            "id": id, "name": name, "phase": "error",
            "error": format!("HTTP {} for {}", resp.status(), url),
            "bytes": 0, "total": 0, "speed_bps": 0, "eta_seconds": null
        }));
        return Err(format!("HTTP {} for {}", resp.status(), url));
    }

    let f = File::create(&part_file)
        .map_err(|e| format!("create partial file: {}", e))?;
    download_stream_to_file(
        app, id, name, resp, &part_file, dest,
        0, Some(f),
    ).await
}

async fn download_stream_to_file(
    app: &AppHandle,
    id: &str,
    name: &str,
    resp: reqwest::Response,
    part_file: &Path,
    dest: &Path,
    resume_from: u64,
    file_handle: Option<File>,
) -> Result<u64, String> {
    use futures::StreamExt;

    let total = resume_from + resp.content_length().unwrap_or(0);
    let start = std::time::Instant::now();
    let mut last_emit = std::time::Instant::now();

    let _ = app.emit("download-progress", serde_json::json!({
        "id": id, "name": name,
        "phase": if resume_from > 0 { "resuming" } else { "downloading" },
        "bytes": resume_from, "total": total, "speed_bps": 0, "eta_seconds": null
    }));

    let mut f = file_handle.ok_or_else(|| "no file handle".to_string())?;
    let mut downloaded: u64 = resume_from;
    let mut download_error: Option<String> = None;
    let mut stream = resp.bytes_stream();

    loop {
        let chunk = match tokio::time::timeout(Duration::from_secs(120), stream.next()).await {
            Ok(Some(Ok(c))) => c,
            Ok(Some(Err(e))) => {
                download_error = Some(format!("stream error: {}", e));
                break;
            }
            Ok(None) => break,
            Err(_) => {
                download_error = Some(
                    "download timed out (no data received for 120 seconds). The partial file is kept for resume.".to_string()
                );
                break;
            }
        };
        if let Err(e) = f.write_all(&chunk) {
            download_error = Some(format!("write error: {}", e));
            break;
        }
        downloaded += chunk.len() as u64;

        // Flush every 1MB
        if downloaded % (1024 * 1024) < chunk.len() as u64 {
            let _ = f.flush();
        }

        if last_emit.elapsed() >= Duration::from_millis(200) {
            let elapsed = start.elapsed().as_secs_f64();
            let effective_downloaded = downloaded - resume_from;
            let speed_bps = if elapsed > 0.0 { (effective_downloaded as f64 / elapsed) as u64 } else { 0 };
            let eta = if total > downloaded && speed_bps > 0 {
                ((total - downloaded) as f64 / speed_bps as f64).max(0.0)
            } else {
                -1.0
            };
            let eta_value = if eta < 0.0 { serde_json::Value::Null } else { serde_json::json!(eta) };
            let _ = app.emit("download-progress", serde_json::json!({
                "id": id, "name": name,
                "phase": if resume_from > 0 { "resuming" } else { "downloading" },
                "bytes": downloaded, "total": total, "speed_bps": speed_bps, "eta_seconds": eta_value
            }));
            last_emit = std::time::Instant::now();
        }
    }

    if let Some(err) = download_error {
        let _ = f.flush();
        drop(f);
        return Err(format!("download interrupted: {}", err));
    }

    if let Err(e) = f.flush() {
        drop(f);
        let _ = fs::remove_file(part_file);
        return Err(format!("flush error: {}", e));
    }
    drop(f);

    let part_size = fs::metadata(part_file).map(|m| m.len()).unwrap_or(0);
    if part_size == 0 {
        let _ = fs::remove_file(part_file);
        return Err("Download produced an empty file (server returned 0 bytes).".to_string());
    }
    if total > resume_from && part_size < total {
        // Incomplete — .part file kept for resume
        return Err(format!(
            "Incomplete download: got {} bytes, expected {} bytes. The partial file is kept for resume.",
            part_size, total
        ));
    }

    // Atomic rename: .part → final destination
    if dest.exists() {
        let _ = fs::remove_file(dest);
    }
    fs::rename(part_file, dest)
        .map_err(|e| format!("rename .part to final: {}", e))?;

    let _ = app.emit("download-progress", serde_json::json!({
        "id": id, "name": name, "phase": "done",
        "bytes": part_size, "total": total, "speed_bps": 0, "eta_seconds": null
    }));
    Ok(part_size)
}

