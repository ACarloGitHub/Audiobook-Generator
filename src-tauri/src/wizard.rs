use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

const LLAMACPP_PINNED_VERSION: &str = "b9756";

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

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub id: String,
    pub name: String,
    pub phase: String,
    pub bytes: u64,
    pub total: u64,
    pub speed_bps: u64,
    pub eta_seconds: Option<f64>,
    pub error: Option<String>,
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

fn command_exists(cmd: &str) -> bool {
    which::which(cmd).is_ok()
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

fn platform_string() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

fn arch_string() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86_64") {
        "x64"
    } else {
        "unknown"
    }
}

pub(crate) fn resources_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir error: {}", e))?;
    let dir = base.join("resources");
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("create resources dir: {}", e))?;
    }
    Ok(dir)
}

pub(crate) fn bin_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir error: {}", e))?;
    let dir = base.join("bin");
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("create bin dir: {}", e))?;
    }
    Ok(dir)
}

fn llamacpp_binary_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "llama-server.exe"
    } else {
        "llama-server"
    }
}

fn llamacpp_url_for_variant(variant: &str) -> String {
    let platform = platform_string();
    let arch = arch_string();
    let ver = LLAMACPP_PINNED_VERSION;
    let asset = match (platform, arch, variant) {
        ("windows", "x64", "cpu") => format!("llama-{}-bin-win-cpu-x64.zip", ver),
        ("windows", "x64", "cuda") => format!("llama-{}-bin-win-cuda-12.4-x64.zip", ver),
        ("windows", "x64", "vulkan") => format!("llama-{}-bin-win-vulkan-x64.zip", ver),
        ("windows", "arm64", "cpu") => format!("llama-{}-bin-win-cpu-arm64.zip", ver),
        ("macos", "arm64", _) => format!("llama-{}-bin-macos-arm64.tar.gz", ver),
        ("macos", "x64", _) => format!("llama-{}-bin-macos-x64.tar.gz", ver),
        ("linux", "x64", "cpu") => format!("llama-{}-bin-ubuntu-x64.tar.gz", ver),
        ("linux", "x64", "vulkan") => format!("llama-{}-bin-ubuntu-vulkan-x64.tar.gz", ver),
        ("linux", "arm64", _) => format!("llama-{}-bin-ubuntu-arm64.tar.gz", ver),
        _ => format!("llama-{}-bin-ubuntu-x64.tar.gz", ver),
    };
    format!(
        "https://github.com/ggml-org/llama.cpp/releases/download/{}/{}",
        ver, asset
    )
}

fn llamacpp_cudart_url() -> Option<String> {
    if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        let ver = LLAMACPP_PINNED_VERSION;
        let asset = "cudart-llama-bin-win-cuda-12.4-x64.zip";
        Some(format!(
            "https://github.com/ggml-org/llama.cpp/releases/download/{}/{}",
            ver, asset
        ))
    } else {
        None
    }
}

fn is_zip_url(url: &str) -> bool {
    url.to_lowercase().ends_with(".zip")
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
pub fn check_dependencies(app: AppHandle) -> DependencyStatus {
    let app_data = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
    let bin = match bin_dir(&app) {
        Ok(b) => b,
        Err(_) => app_data.join("bin"),
    };
    let res = match resources_dir(&app) {
        Ok(r) => r,
        Err(_) => app_data.join("resources"),
    };

    let ffmpeg_exe = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let ffmpeg_installed = command_exists("ffmpeg") || bin.join(ffmpeg_exe).exists();
    let ffmpeg_path = which::which("ffmpeg")
        .ok()
        .or_else(|| {
            let p = bin.join(ffmpeg_exe);
            if p.exists() { Some(p) } else { None }
        })
        .map(|p| p.to_string_lossy().to_string());

    let llama_exe = llamacpp_binary_name();
    let llama_dir = res.join("llama.cpp");
    let llama_installed = command_exists("llama-server")
        || find_binary_in_dir(&llama_dir, llama_exe).is_some();
    let llama_server_path = which::which("llama-server")
        .ok()
        .or_else(|| find_binary_in_dir(&llama_dir, llama_exe))
        .map(|p| p.to_string_lossy().to_string());

    // ONNX Runtime is a Rust dependency (ort crate); if the app is running it is available.
    let ort_installed = true;

    let cudnn_installed = if cfg!(target_os = "windows") {
        let system32 = PathBuf::from("C:\\Windows\\System32");
        system32.join("cudnn64_9.dll").exists()
            || system32.join("cudnn_ops_infer64_9.dll").exists()
            || res.join("llama.cpp").join("cublas64_12.dll").exists()
            || res.join("llama.cpp").join("cudart64_12.dll").exists()
    } else {
        let lib_paths = ["/usr/lib/x86_64-linux-gnu/libcudnn.so.9", "/usr/local/cuda/lib64/libcudnn.so.9"];
        lib_paths.iter().any(|p| PathBuf::from(p).exists())
    };

    DependencyStatus {
        ffmpeg_installed,
        ffmpeg_path,
        llama_server_installed: llama_installed,
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
            description: "llama-server is the inference engine for GGUF models (Qwen3-TTS, OuteTTS, VoxCPM2).".into(),
            completed: false,
        },
        WizardStep {
            id: "ort".into(),
            title: "ONNX Runtime + cuDNN".into(),
            description: "ONNX Runtime is built into the app (used by OuteTTS). cuDNN is optional for GPU acceleration on NVIDIA.".into(),
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
pub async fn download_ffmpeg(app: AppHandle) -> Result<String, String> {
    if command_exists("ffmpeg") {
        return Ok("FFmpeg is already available on PATH.".into());
    }
    let res = resources_dir(&app)?;
    let bin = bin_dir(&app)?;

    let url = if cfg!(target_os = "windows") {
        "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
    } else {
        return Err("On macOS/Linux, please install FFmpeg via your package manager:\n  macOS: brew install ffmpeg\n  Linux: sudo apt install ffmpeg".into());
    };

    let archive_path = res.join("ffmpeg-download.zip");
    download_to_file_async(&app, "ffmpeg", "FFmpeg", url, &archive_path).await?;

    let target_dir = res.join("ffmpeg");
    if target_dir.exists() {
        let _ = fs::remove_dir_all(&target_dir);
    }
    let _ = fs::create_dir_all(&target_dir);

    app.emit("download-progress", serde_json::json!({
        "id": "ffmpeg", "name": "FFmpeg", "phase": "extracting",
        "bytes": 0, "total": 0, "speed_bps": 0, "eta_seconds": null
    })).ok();

    extract_zip(&archive_path, &target_dir)?;
    let _ = fs::remove_file(&archive_path);

    let ffmpeg_exe = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let ffprobe_exe = if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" };

    let src_bin = find_binary_in_dir(&target_dir, ffmpeg_exe)
        .ok_or_else(|| "Could not find ffmpeg in extracted archive".to_string())?;
    let dest_bin = bin.join(ffmpeg_exe);
    fs::copy(&src_bin, &dest_bin).map_err(|e| format!("Failed to copy ffmpeg: {e}"))?;

    if let Some(src_ffprobe) = find_binary_in_dir(&target_dir, ffprobe_exe) {
        let dest_ffprobe = bin.join(ffprobe_exe);
        fs::copy(&src_ffprobe, &dest_ffprobe).ok();
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dest_bin, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to set permissions: {e}"))?;
    }

    app.emit("download-progress", serde_json::json!({
        "id": "ffmpeg", "name": "FFmpeg", "phase": "done",
        "bytes": 0, "total": 0, "speed_bps": 0, "eta_seconds": null
    })).ok();

    Ok("FFmpeg installed successfully.".into())
}

#[tauri::command]
pub async fn download_llama_server(app: AppHandle) -> Result<String, String> {
    if command_exists("llama-server") {
        return Ok("llama-server is already available on PATH.".into());
    }
    let res = resources_dir(&app)?;

    let variant = if cfg!(target_os = "macos") {
        "metal"
    } else if cfg!(target_os = "windows") && detect_nvidia_smi().is_some() {
        let driver = detect_nvidia_smi().unwrap_or_default();
        let major: u32 = driver.split('.').next().and_then(|s| s.parse().ok()).unwrap_or(0);
        if major >= 525 { "cuda" } else { "vulkan" }
    } else if cfg!(target_os = "linux") && detect_nvidia_smi().is_some() {
        "vulkan"
    } else if cfg!(target_os = "windows") {
        "vulkan"
    } else {
        "cpu"
    };

    let url = llamacpp_url_for_variant(variant);
    let is_zip = is_zip_url(&url);
    let target_dir = res.join("llama.cpp");

    if target_dir.exists() {
        let _ = fs::remove_dir_all(&target_dir);
    }
    let _ = fs::create_dir_all(&target_dir);

    let archive_ext = if is_zip { "llama-server-download.zip" } else { "llama-server-download.tar.gz" };
    let archive_path = res.join(archive_ext);

    let display_name = format!("llama-server ({})", variant);
    download_to_file_async(&app, "llama-server", &display_name, &url, &archive_path).await?;

    app.emit("download-progress", serde_json::json!({
        "id": "llama-server", "name": display_name, "phase": "extracting",
        "bytes": 0, "total": 0, "speed_bps": 0, "eta_seconds": null
    })).ok();

    if is_zip {
        extract_zip(&archive_path, &target_dir)?;
    } else {
        extract_tar_gz(&archive_path, &target_dir)?;
    }
    let _ = fs::remove_file(&archive_path);

    // On Windows with CUDA: download and merge CUDA runtime DLLs
    if variant == "cuda" && cfg!(target_os = "windows") {
        if let Some(cudart_url) = llamacpp_cudart_url() {
            let cudart_archive = res.join("llama-server-cudart.zip");
            download_to_file_async(&app, "llama-server-cudart", "CUDA Runtime DLLs", &cudart_url, &cudart_archive).await?;
            app.emit("download-progress", serde_json::json!({
                "id": "llama-server-cudart", "name": "CUDA Runtime DLLs", "phase": "extracting",
                "bytes": 0, "total": 0, "speed_bps": 0, "eta_seconds": null
            })).ok();
            extract_zip_into(&cudart_archive, &target_dir)?;
            let _ = fs::remove_file(&cudart_archive);
        }
    }

    let exe_name = llamacpp_binary_name();
    let bin = find_binary_in_dir(&target_dir, exe_name)
        .ok_or_else(|| format!("Could not find {} in extracted archive", exe_name))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&bin).map_err(|e| e.to_string())?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&bin, perms).ok();
    }

    // Write variant metadata
    let meta_path = target_dir.join("variant.txt");
    fs::write(&meta_path, variant).map_err(|e| format!("write variant: {}", e))?;

    app.emit("download-progress", serde_json::json!({
        "id": "llama-server", "name": display_name, "phase": "done",
        "bytes": 0, "total": 0, "speed_bps": 0, "eta_seconds": null
    })).ok();

    Ok(format!("llama-server ({}) installed successfully.", variant))
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

// =============================================================================
// Extraction (from AuraWrite pattern: temp dir + atomic rename + path traversal guard)
// =============================================================================

pub(crate) fn extract_zip(zip_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let f = File::open(zip_path).map_err(|e| format!("open zip: {}", e))?;
    let mut archive = zip::ZipArchive::new(f).map_err(|e| format!("read zip: {}", e))?;
    // Extract to a temporary subdirectory first, then atomically move into place.
    let temp_dir = dest_dir.with_file_name(format!(
        "{}.extract-{}",
        dest_dir.file_name().and_then(|n| n.to_str()).unwrap_or("dest"),
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).map_err(|e| format!("create temp dir: {}", e))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("zip entry: {}", e))?;
        let name = entry.name().to_string();
        if name.contains("..") {
            continue;
        }
        let outpath = temp_dir.join(&name);
        if entry.is_dir() {
            fs::create_dir_all(&outpath).ok();
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent).ok();
            }
            // Retry on Windows sharing violations (OS error 32) — antivirus may lock files
            let mut out = None;
            let mut last_err: Option<String> = None;
            for attempt in 0..5 {
                match File::create(&outpath) {
                    Ok(f) => { out = Some(f); break; }
                    Err(e) => {
                        last_err = Some(format!("create {}: {}", outpath.display(), e));
                        std::thread::sleep(Duration::from_millis(150 * (attempt + 1)));
                    }
                }
            }
            let mut out = out.ok_or_else(|| last_err.unwrap_or_else(|| "create failed".to_string()))?;
            io::copy(&mut entry, &mut out).map_err(|e| format!("write: {}", e))?;
        }
    }
    if dest_dir.exists() {
        fs::remove_dir_all(dest_dir).map_err(|e| format!("remove old dest: {}", e))?;
    }
    fs::rename(&temp_dir, dest_dir).map_err(|e| format!("rename temp to dest: {}", e))?;
    Ok(())
}

/// Extract a zip archive into dest_dir WITHOUT clearing it first.
/// Used to merge CUDA runtime DLLs into the same directory as the binary.
pub(crate) fn extract_zip_into(zip_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let f = File::open(zip_path).map_err(|e| format!("open zip: {}", e))?;
    let mut archive = zip::ZipArchive::new(f).map_err(|e| format!("read zip: {}", e))?;
    fs::create_dir_all(dest_dir).map_err(|e| format!("create dir: {}", e))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("zip entry: {}", e))?;
        let name = entry.name().to_string();
        if name.contains("..") {
            continue;
        }
        let outpath = dest_dir.join(&name);
        if entry.is_dir() {
            fs::create_dir_all(&outpath).ok();
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent).ok();
            }
            let mut out = None;
            let mut last_err: Option<String> = None;
            for attempt in 0..5 {
                match File::create(&outpath) {
                    Ok(f) => { out = Some(f); break; }
                    Err(e) => {
                        last_err = Some(format!("create {}: {}", outpath.display(), e));
                        std::thread::sleep(Duration::from_millis(150 * (attempt + 1)));
                    }
                }
            }
            let mut out = out.ok_or_else(|| last_err.unwrap_or_else(|| "create failed".to_string()))?;
            io::copy(&mut entry, &mut out).map_err(|e| format!("write: {}", e))?;
        }
    }
    Ok(())
}

pub(crate) fn extract_tar_gz(tar_gz_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let f = File::open(tar_gz_path).map_err(|e| format!("open tar.gz: {}", e))?;
    let gz = flate2::read::GzDecoder::new(f);
    let mut archive = tar::Archive::new(gz);
    let temp_dir = dest_dir.with_file_name(format!(
        "{}.extract-{}",
        dest_dir.file_name().and_then(|n| n.to_str()).unwrap_or("dest"),
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).map_err(|e| format!("create temp dir: {}", e))?;
    archive.unpack(&temp_dir).map_err(|e| format!("unpack tar.gz: {}", e))?;
    if dest_dir.exists() {
        fs::remove_dir_all(dest_dir).map_err(|e| format!("remove old dest: {}", e))?;
    }
    fs::rename(&temp_dir, dest_dir).map_err(|e| format!("rename temp to dest: {}", e))?;
    Ok(())
}

pub(crate) fn find_binary_in_dir(root: &Path, name: &str) -> Option<PathBuf> {
    if !root.exists() {
        return None;
    }
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.is_file() {
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    if file_name == name {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}