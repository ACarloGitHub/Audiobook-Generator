use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::config::models::{self, ModelAsset};
use crate::wizard::download_to_file_async;

/// Files to fetch from `onnx-community/Kokoro-82M-v1.0-ONNX`.
/// The full repo carries voice packs for all 8 languages; we fetch a
/// representative subset (the user's defaults + a few extras) so the
/// download stays under 200 MB.
const KOKORO_HF_REPO: &str = "onnx-community/Kokoro-82M-v1.0-ONNX";
const KOKORO_REQUIRED_FILES: &[&str] = &[
    "onnx/model_quantized.onnx",
    "voices/af_heart.bin",
    "voices/am_adam.bin",
    "voices/bf_emma.bin",
    "voices/ff_siwis.bin",
    "voices/jf_alpha.bin",
    "voices/zf_xiaobei.bin",
    "voices/ef_dora.bin",
    "voices/pf_dora.bin",
    "voices/hf_alpha.bin",
];

#[derive(Debug, Clone, Serialize)]
pub struct ModelDownloadResult {
    pub model_name: String,
    pub installed: bool,
    pub total_bytes: u64,
    pub files: Vec<String>,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelListEntry {
    pub name: String,
    pub engine_id: String,
    pub format: String,
    pub license: String,
    pub size_mb: u32,
    pub installed: bool,
    pub essential_present: bool,
    pub dest: String,
    pub supported: bool,
    pub note: Option<String>,
}

pub fn list_models(app: &AppHandle) -> Vec<ModelListEntry> {
    let assets_map = models::model_assets();
    let mut out = Vec::new();
    for (name, assets) in assets_map.iter() {
        let Some(asset) = assets.first() else { continue; };
        let dest_path = app_models_root(app).join(&asset.dest);
        let essential_present = check_essential(asset, &dest_path);
        let installed = dest_path.exists() && essential_present;
        out.push(ModelListEntry {
            name: name.to_string(),
            engine_id: asset_to_engine_id(name),
            format: asset_format(name),
            license: asset_license(name),
            size_mb: asset_size_mb(name),
            installed,
            essential_present,
            dest: dest_path.to_string_lossy().to_string(),
            supported: is_supported(name),
            note: asset_note(name),
        });
    }
    out
}

pub fn is_model_installed(name: &str, app: &AppHandle) -> bool {
    let Some(asset) = model_asset(name) else { return false; };
    let dest_path = app_models_root(app).join(&asset.dest);
    check_essential(&asset, &dest_path)
}

pub fn remove_model(name: &str, app: &AppHandle) -> Result<(), String> {
    let Some(asset) = model_asset(name) else {
        return Err(format!("unknown model '{name}'"));
    };
    let dest_path = app_models_root(app).join(&asset.dest);
    if dest_path.exists() {
        std::fs::remove_dir_all(&dest_path)
            .map_err(|e| format!("failed to remove {}: {e}", dest_path.display()))?;
    }
    Ok(())
}

/// Download a model by name. Currently only Kokoro is supported;
/// llama-server engines (Qwen3-TTS, VibeVoice, XTTSv2) return an
/// explicit error since their plugins are stubs until phase 12-13.
pub async fn download_model(
    name: &str,
    app: &AppHandle,
) -> Result<ModelDownloadResult, String> {
    let asset = model_asset(name)
        .ok_or_else(|| format!("unknown model '{name}'"))?;

    if !is_supported(name) {
        return Err(format!(
            "model '{name}' is not downloadable yet — its plugin is a stub. \
             Use llama-server engines will be wired in phase 12-13."
        ));
    }

    let dest_root = app_models_root(app).join(&asset.dest);
    std::fs::create_dir_all(&dest_root)
        .map_err(|e| format!("create dest dir: {e}"))?;

    let url_base = format!("https://huggingface.co/{}/resolve/main", asset.url.as_deref().unwrap_or(KOKORO_HF_REPO));
    let files: Vec<String> = KOKORO_REQUIRED_FILES.iter().map(|s| s.to_string()).collect();

    let mut total_bytes: u64 = 0;
    for file in &files {
        let url = format!("{url_base}/{file}");
        let dest = dest_root.join(file);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create dir for {file}: {e}"))?;
        }
        if dest.exists() {
            let _ = app.emit("model-progress", serde_json::json!({
                "model": name, "file": file, "phase": "already_present",
                "bytes": 0, "total": 0, "speed_bps": 0, "eta_seconds": null
            }));
            continue;
        }
        download_to_file_async(&app, &format!("{name}:{file}"), file, &url, &dest).await?;
        let size = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
        total_bytes += size;
    }

    // Verify essential files are now present
    if !check_essential(&asset, &dest_root) {
        return Err(format!(
            "download completed but essential files are missing in {}. \
             Check network and retry.",
            dest_root.display()
        ));
    }

    let _ = app.emit("model-progress", serde_json::json!({
        "model": name, "file": "(done)", "phase": "done",
        "bytes": total_bytes, "total": total_bytes, "speed_bps": 0, "eta_seconds": null
    }));

    Ok(ModelDownloadResult {
        model_name: name.to_string(),
        installed: true,
        total_bytes,
        files,
        path: dest_root.to_string_lossy().to_string(),
    })
}

fn app_models_root(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("models")
}

fn model_asset(name: &str) -> Option<ModelAsset> {
    models::model_assets()
        .get(name)
        .and_then(|v| v.first())
        .cloned()
}

fn check_essential(asset: &ModelAsset, dest_path: &Path) -> bool {
    if !dest_path.exists() {
        return false;
    }
    if let Some(essential) = &asset.essential_files {
        essential.iter().all(|f| dest_path.join(f).exists())
    } else {
        dest_path.exists()
    }
}

fn asset_to_engine_id(name: &str) -> String {
    match name {
        "Kokoro" => "kokoro".into(),
        n if n.starts_with("Qwen3-TTS") => "qwen3tts".into(),
        n if n.starts_with("VibeVoice") => "vibevoice".into(),
        "XTTSv2" => "xttsv2".into(),
        _ => name.to_lowercase(),
    }
}

fn asset_format(name: &str) -> String {
    if name == "Kokoro" {
        "ONNX".into()
    } else {
        "Safetensors".into()
    }
}

fn asset_license(name: &str) -> String {
    match name {
        "Kokoro" => "Apache 2.0".into(),
        n if n.starts_with("Qwen3-TTS") => "Apache 2.0".into(),
        n if n.starts_with("VibeVoice") => "MIT".into(),
        "XTTSv2" => "CPML (non-commercial)".into(),
        _ => "Unknown".into(),
    }
}

fn asset_size_mb(name: &str) -> u32 {
    match name {
        "Kokoro" => 180,
        n if n.starts_with("Qwen3-TTS-0.6B") => 1300,
        n if n.starts_with("Qwen3-TTS-1.7B") => 3600,
        n if n.starts_with("VibeVoice-1.5B") => 3100,
        n if n.starts_with("VibeVoice-7B") => 14500,
        n if n.starts_with("VibeVoice-Realtime") => 1100,
        "XTTSv2" => 2100,
        _ => 1000,
    }
}

fn is_supported(name: &str) -> bool {
    name == "Kokoro"
}

fn asset_note(name: &str) -> Option<String> {
    if is_supported(name) {
        None
    } else {
        Some(format!(
            "'{name}' runs via llama-server and its plugin is a stub until phase 12-13."
        ))
    }
}