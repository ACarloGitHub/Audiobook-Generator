use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::wizard::{download_to_file_async, extract_zip, resources_dir};

/// Default quantization for downloads.
const DEFAULT_QUANT: &str = "Q4_K_M";

/// The engine registry JSON (verified download links + parameters).
/// Loaded at compile time so it is always in sync with the binary.
const ENGINE_REGISTRY: &str = include_str!("../engine_registry.json");

/// Parse only the qwen3tts engine from the registry JSON.
/// Other engines (OuteTTS, Chatterbox, etc.) have different
/// shared_files structures and would break strict deserialization.
/// By parsing as Value first and extracting only qwen3tts, we
/// isolate ourselves from incompatible engine definitions.
fn parse_qwen_engine() -> Result<EngineDef, String> {
    let raw: serde_json::Value = serde_json::from_str(ENGINE_REGISTRY)
        .map_err(|e| format!("parse engine_registry.json: {e}"))?;
    let qwen_val = raw
        .get("engines")
        .and_then(|e| e.get("qwen3tts"))
        .ok_or_else(|| "qwen3tts not found in engine_registry.json".to_string())?;
    serde_json::from_value(qwen_val.clone())
        .map_err(|e| format!("parse qwen3tts engine definition: {e}"))
}

/// Parse the outetts engine from the registry JSON.
/// OuteTTS has a different shared_files structure (DAC ONNX, not GGUF quants).
fn parse_oute_engine() -> Result<OuteEngineDef, String> {
    let raw: serde_json::Value = serde_json::from_str(ENGINE_REGISTRY)
        .map_err(|e| format!("parse engine_registry.json: {e}"))?;
    let oute_val = raw
        .get("engines")
        .and_then(|e| e.get("outetts"))
        .ok_or_else(|| "outetts not found in engine_registry.json".to_string())?;
    serde_json::from_value(oute_val.clone())
        .map_err(|e| format!("parse outetts engine definition: {e}"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OuteEngineDef {
    variants: Vec<OuteVariantDef>,
    shared_files: Vec<OuteSharedFileDef>,
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OuteVariantDef {
    name: String,
    files: Vec<FileDef>,
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OuteSharedFileDef {
    name: String,
    files: Vec<OuteNamedFile>,
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OuteNamedFile {
    name: String,
    filename: String,
    url: String,
    size_mb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EngineDef {
    variants: Vec<VariantDef>,
    shared_files: Vec<SharedFileDef>,
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VariantDef {
    name: String,
    gguf_repo: String,
    files: Vec<FileDef>,
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SharedFileDef {
    name: String,
    filename_template: String,
    quants: std::collections::HashMap<String, QuantInfo>,
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileDef {
    name: String,
    filename_template: String,
    quants: std::collections::HashMap<String, QuantInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QuantInfo {
    size_mb: u32,
    url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuntimeDownload {
    dest_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuntimeVariant {
    url: String,
    dest_dir: String,
    size_mb: u32,
}

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

/// Short filename for a talker GGUF in a given quantization.
/// E.g. quant="Q4_K_M" → "talker-Q4_K_M.gguf"
fn talker_filename(quant: &str) -> String {
    format!("talker-{}.gguf", quant)
}

/// Short filename for the tokenizer GGUF in a given quantization.
fn tokenizer_filename(quant: &str) -> String {
    format!("tokenizer-{}.gguf", quant)
}

/// Directory where a specific variant's talker GGUF lives.
fn variant_dir(app: &AppHandle, variant_name: &str) -> PathBuf {
    app_models_root(app)
        .join("qwen3tts")
        .join(variant_name)
}

/// Directory where the shared tokenizer GGUF lives.
fn tokenizer_dir(app: &AppHandle) -> PathBuf {
    app_models_root(app)
        .join("qwen3tts")
        .join("tokenizer")
}

/// Directory where an OuteTTS variant's backbone GGUF lives.
fn oute_variant_dir(app: &AppHandle, variant_name: &str) -> PathBuf {
    app_models_root(app)
        .join("outetts")
        .join(variant_name)
}

/// Directory where the shared DAC ONNX decoder lives.
fn oute_dac_dir(app: &AppHandle) -> PathBuf {
    app_models_root(app)
        .join("outetts")
        .join("dac")
}

/// Check if the qwen-tts binary is installed in resources/qwentts/.
pub fn is_runtime_installed(app: &AppHandle) -> bool {
    if let Ok(res) = resources_dir(app) {
        let exe_name = if cfg!(windows) {
            "qwen-tts.exe"
        } else {
            "qwen-tts"
        };
        return res.join("qwentts").join(exe_name).exists();
    }
    false
}

/// Download and extract the qwen-tts binary if not already present.
async fn ensure_runtime(app: &AppHandle) -> Result<(), String> {
    if is_runtime_installed(app) {
        return Ok(());
    }

    let qwen = parse_qwen_engine()?;

    // Find the right runtime download for this platform
    let runtime_key = if cfg!(target_os = "windows") {
        "windows_cuda"
    } else if cfg!(target_os = "linux") {
        "linux_cuda"
    } else if cfg!(target_os = "macos") {
        "macos_metal"
    } else {
        return Err("unsupported platform for qwen-tts runtime".into());
    };

    let runtime = qwen
        ._extra
        .get("runtime_download")
        .and_then(|rd| rd.get(runtime_key))
        .ok_or_else(|| format!("no runtime download configured for {}", runtime_key))?;

    let url = runtime
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "runtime URL missing".to_string())?;

    let _ = app.emit("model-progress", serde_json::json!({
        "model": "qwen-tts-runtime", "file": "qwen-tts-runtime.zip",
        "phase": "downloading", "bytes": 0, "total": 0,
        "speed_bps": 0, "eta_seconds": null
    }));

    let res = resources_dir(app)?;
    let dest = res.join("qwentts");
    std::fs::create_dir_all(&dest)
        .map_err(|e| format!("create qwentts dir: {e}"))?;

    let zip_dest = res.join("qwen-tts-runtime.zip");

    // Handle file:// URLs (local testing) vs HTTP URLs
    if url.starts_with("file://") {
        let local_path = url.strip_prefix("file:///").unwrap_or(&url[7..]);
        let local_path = if local_path.starts_with('/') {
            local_path.to_string()
        } else {
            local_path.replace('/', "\\")
        };
        std::fs::copy(&local_path, &zip_dest)
            .map_err(|e| format!("copy local runtime: {e}"))?;
        let _ = app.emit("model-progress", serde_json::json!({
            "model": "qwen-tts-runtime", "file": "qwen-tts-runtime.zip",
            "phase": "done", "bytes": 0, "total": 0,
            "speed_bps": 0, "eta_seconds": null
        }));
    } else {
        download_to_file_async(
            app,
            "qwen-tts-runtime",
            "qwen-tts-runtime.zip",
            url,
            &zip_dest,
        )
        .await?;
    }

    let _ = app.emit("model-progress", serde_json::json!({
        "model": "qwen-tts-runtime", "file": "qwen-tts-runtime.zip",
        "phase": "extracting", "bytes": 0, "total": 0,
        "speed_bps": 0, "eta_seconds": null
    }));

    extract_zip(&zip_dest, &dest)?;
    let _ = std::fs::remove_file(&zip_dest);

    let _ = app.emit("model-progress", serde_json::json!({
        "model": "qwen-tts-runtime", "file": "qwen-tts-runtime.zip",
        "phase": "done", "bytes": 0, "total": 0,
        "speed_bps": 0, "eta_seconds": null
    }));

    Ok(())
}

pub fn list_models(app: &AppHandle) -> Vec<ModelListEntry> {
    let mut out = Vec::new();

    let qwen = match parse_qwen_engine() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[model_manager] {e}");
            return out;
        }
    };

    for variant in &qwen.variants {
        let vdir = variant_dir(app, &variant.name);
        let tdir = tokenizer_dir(app);

        let talker_name = talker_filename(DEFAULT_QUANT);
        let tok_name = tokenizer_filename(DEFAULT_QUANT);

        let essential_present =
            vdir.join(&talker_name).exists() && tdir.join(&tok_name).exists();
        let installed = essential_present;

        let size_mb = variant
            .files
            .first()
            .and_then(|f| f.quants.get(DEFAULT_QUANT))
            .map(|q| q.size_mb)
            .unwrap_or(0)
            + qwen
                .shared_files
                .first()
                .and_then(|f| f.quants.get(DEFAULT_QUANT))
                .map(|q| q.size_mb)
                .unwrap_or(0);

        out.push(ModelListEntry {
            name: variant.name.clone(),
            engine_id: "qwen3tts".into(),
            format: "GGUF".into(),
            license: "Apache 2.0".into(),
            size_mb,
            installed,
            essential_present,
            dest: vdir.to_string_lossy().to_string(),
            supported: true,
            note: None,
        });
    }

    if let Ok(oute) = parse_oute_engine() {
        for variant in &oute.variants {
            let vdir = oute_variant_dir(app, &variant.name);
            let ddir = oute_dac_dir(app);

            let backbone_name = format!("backbone-{}.gguf", DEFAULT_QUANT);
            let dac_name = "decoder.onnx";

            let essential_present =
                vdir.join(&backbone_name).exists() && ddir.join(dac_name).exists();

            let backbone_size = variant
                .files
                .first()
                .and_then(|f| f.quants.get(DEFAULT_QUANT))
                .map(|q| q.size_mb)
                .unwrap_or(0);
            let dac_size = oute
                .shared_files
                .first()
                .and_then(|sf| sf.files.first())
                .map(|f| f.size_mb)
                .unwrap_or(0);

            out.push(ModelListEntry {
                name: variant.name.clone(),
                engine_id: "outetts".into(),
                format: "GGUF + ONNX".into(),
                license: "Apache 2.0".into(),
                size_mb: backbone_size + dac_size,
                installed: essential_present,
                essential_present,
                dest: vdir.to_string_lossy().to_string(),
                supported: true,
                note: None,
            });
        }
    }

    out
}

pub fn is_model_installed(name: &str, app: &AppHandle) -> bool {
    list_models(app)
        .iter()
        .any(|m| m.name == name && m.installed)
}

pub fn remove_model(name: &str, app: &AppHandle) -> Result<(), String> {
    let dest_path = variant_dir(app, name);
    if dest_path.exists() {
        std::fs::remove_dir_all(&dest_path)
            .map_err(|e| format!("failed to remove {}: {e}", dest_path.display()))?;
    }
    let oute_path = oute_variant_dir(app, name);
    if oute_path.exists() {
        std::fs::remove_dir_all(&oute_path)
            .map_err(|e| format!("failed to remove {}: {e}", oute_path.display()))?;
    }
    let _ = app.emit("engine-status-changed", ());
    Ok(())
}

pub async fn download_model(
    name: &str,
    app: &AppHandle,
) -> Result<ModelDownloadResult, String> {
    if let Ok(oute) = parse_oute_engine() {
        if oute.variants.iter().any(|v| v.name == name) {
            return download_outetts_model(name, &oute, app).await;
        }
    }

    let qwen = parse_qwen_engine()?;

    let variant = qwen
        .variants
        .iter()
        .find(|v| v.name == name)
        .ok_or_else(|| format!("variant '{}' not found in registry", name))?;

    // Step 1: ensure the qwen-tts binary is installed
    ensure_runtime(app).await?;

    // Step 2: download model files
    let dest_root = variant_dir(app, &variant.name);
    std::fs::create_dir_all(&dest_root)
        .map_err(|e| format!("create dest dir: {e}"))?;

    let mut total_bytes: u64 = 0;
    let mut files: Vec<String> = Vec::new();

    // Download talker GGUF
    if let Some(talker_file) = variant.files.first() {
        if let Some(quant) = talker_file.quants.get(DEFAULT_QUANT) {
            let local_name = talker_filename(DEFAULT_QUANT);
            let dest = dest_root.join(&local_name);
            if dest.exists() {
                let _ = app.emit("model-progress", serde_json::json!({
                    "model": name, "file": local_name, "phase": "already_present",
                    "bytes": 0, "total": 0, "speed_bps": 0, "eta_seconds": null
                }));
            } else {
                download_to_file_async(
                    app,
                    &format!("{}:{}", name, local_name),
                    &local_name,
                    &quant.url,
                    &dest,
                )
                .await?;
            }
            let size = std::fs::metadata(&dest)
                .map(|m| m.len())
                .unwrap_or(0);
            total_bytes += size;
            files.push(local_name);
        }
    }

    // Download shared tokenizer
    if let Some(tokenizer_file) = qwen.shared_files.first() {
        if let Some(quant) = tokenizer_file.quants.get(DEFAULT_QUANT) {
            let tdir = tokenizer_dir(app);
            std::fs::create_dir_all(&tdir)
                .map_err(|e| format!("create tokenizer dir: {e}"))?;
            let local_name = tokenizer_filename(DEFAULT_QUANT);
            let dest = tdir.join(&local_name);
            if dest.exists() {
                let _ = app.emit("model-progress", serde_json::json!({
                    "model": name, "file": local_name, "phase": "already_present",
                    "bytes": 0, "total": 0, "speed_bps": 0, "eta_seconds": null
                }));
            } else {
                download_to_file_async(
                    app,
                    &format!("{}:tokenizer", name),
                    &local_name,
                    &quant.url,
                    &dest,
                )
                .await?;
            }
            let size = std::fs::metadata(&dest)
                .map(|m| m.len())
                .unwrap_or(0);
            total_bytes += size;
            files.push(local_name);
        }
    }

    let _ = app.emit("engine-status-changed", ());

    Ok(ModelDownloadResult {
        model_name: name.to_string(),
        installed: true,
        total_bytes,
        files,
        path: dest_root.to_string_lossy().to_string(),
    })
}

/// Download backbone GGUF + DAC ONNX for an OuteTTS variant.
async fn download_outetts_model(
    name: &str,
    oute: &OuteEngineDef,
    app: &AppHandle,
) -> Result<ModelDownloadResult, String> {
    let variant = oute
        .variants
        .iter()
        .find(|v| v.name == name)
        .ok_or_else(|| format!("variant '{}' not found in outetts registry", name))?;

    let dest_root = oute_variant_dir(app, &variant.name);
    std::fs::create_dir_all(&dest_root)
        .map_err(|e| format!("create dest dir: {e}"))?;

    let mut total_bytes: u64 = 0;
    let mut files: Vec<String> = Vec::new();

    if let Some(backbone_file) = variant.files.first() {
        if let Some(quant) = backbone_file.quants.get(DEFAULT_QUANT) {
            let local_name = format!("backbone-{}.gguf", DEFAULT_QUANT);
            let dest = dest_root.join(&local_name);
            if dest.exists() {
                let _ = app.emit("model-progress", serde_json::json!({
                    "model": name, "file": local_name, "phase": "already_present",
                    "bytes": 0, "total": 0, "speed_bps": 0, "eta_seconds": null
                }));
            } else {
                download_to_file_async(
                    app,
                    &format!("{}:{}", name, local_name),
                    &local_name,
                    &quant.url,
                    &dest,
                )
                .await?;
            }
            let size = std::fs::metadata(&dest)
                .map(|m| m.len())
                .unwrap_or(0);
            total_bytes += size;
            files.push(local_name);
        }
    }

    for shared in &oute.shared_files {
        let subdir = match shared.name.as_str() {
            "dac_codec" => "dac",
            "default_speaker" => "speakers",
            other => other,
        };
        let dest_dir = app_models_root(app).join("outetts").join(subdir);
        std::fs::create_dir_all(&dest_dir)
            .map_err(|e| format!("create {} dir: {e}", subdir))?;

        for file in &shared.files {
            let local_name = &file.filename;
            let dest = dest_dir.join(local_name);
            if dest.exists() {
                let _ = app.emit("model-progress", serde_json::json!({
                    "model": name, "file": local_name, "phase": "already_present",
                    "bytes": 0, "total": 0, "speed_bps": 0, "eta_seconds": null
                }));
            } else {
                download_to_file_async(
                    app,
                    &format!("{}:{}", name, local_name),
                    local_name,
                    &file.url,
                    &dest,
                )
                .await?;
            }
            let size = std::fs::metadata(&dest)
                .map(|m| m.len())
                .unwrap_or(0);
            total_bytes += size;
            files.push(local_name.clone());
        }
    }

    let _ = app.emit("engine-status-changed", ());

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
