use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::base_plugin::BaseTTSPlugin;
use crate::config::models;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRegistryEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub plugin_type: String,
    pub engine_id: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineInfo {
    pub id: String,
    pub display_name: String,
    pub format: String,
    pub voice_cloning: bool,
    pub hardware: Vec<String>,
    pub license: String,
    pub languages: Vec<String>,
    pub installed: bool,
    pub size_mb: u32,
    pub voices: Vec<VoiceDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceDescriptor {
    pub id: String,
    pub display_name: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineDefaults {
    pub engine_id: String,
    pub chunk_strategy: String,
    pub chunk_min_words: Option<u32>,
    pub chunk_max_words: Option<u32>,
    pub chunk_max_chars: u32,
    pub chunk_max_chars_by_lang: HashMap<String, u32>,
    pub separator: String,
    pub replace_guillemets: bool,
    pub voice_cloning: bool,
    pub needs_reference_transcript: bool,
    pub supported_languages: Vec<String>,
    pub voices: Vec<VoiceDescriptor>,
    pub generation: serde_json::Map<String, serde_json::Value>,
}

pub struct PluginManager {
    plugins: HashMap<String, Arc<dyn BaseTTSPlugin>>,
    registry: Vec<PluginRegistryEntry>,
    app_data_dir: PathBuf,
}

impl PluginManager {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let registry = Self::load_registry();
        let mut pm = Self {
            plugins: HashMap::new(),
            registry,
            app_data_dir,
        };
        pm.discover_installed_engines();
        pm
    }

    fn load_registry() -> Vec<PluginRegistryEntry> {
        let json = include_str!("../plugins/plugin_registry.json");
        serde_json::from_str(json).unwrap_or_else(|e| {
            eprintln!("[PluginManager] failed to parse plugin_registry.json: {e}");
            Vec::new()
        })
    }

    fn discover_installed_engines(&mut self) {
        // Qwen3-TTS (external process: llama.cpp + codec.cpp)
        // Register when the model files are on disk.
        let qwen_paths = QwenPaths::from_app_data(&crate::config::paths::storage_dir());
        for variant in &self.registry {
            if variant.engine_id == "qwen3tts" {
                let plugin = crate::plugins::qwen3tts::QwenPlugin::new(
                    qwen_paths.clone(),
                    &variant.name,
                );
                if plugin.is_installed() {
                    eprintln!(
                        "[PluginManager] registering {} (model files present)",
                        variant.name
                    );
                    self.plugins
                        .insert(variant.name.clone(), Arc::new(plugin));
                }
            }
        }

        // OuteTTS (llama-server backbone + DAC ONNX decoder)
        let oute_models_dir = crate::config::paths::models_dir().join("outetts");
        for variant in &self.registry {
            if variant.engine_id == "outetts" {
                let plugin = crate::plugins::outetts::OuteTTSPlugin::new(
                    oute_models_dir.clone(),
                    &variant.name,
                );
                if plugin.is_installed() {
                    eprintln!(
                        "[PluginManager] registering {} (model files present)",
                        variant.name
                    );
                    self.plugins
                        .insert(variant.name.clone(), Arc::new(plugin));
                }
            }
        }
        // VoxCPM2 (external process: voxcpm2-cli sidecar)
        let vox_paths = VoxCpm2Paths::from_app_data(&crate::config::paths::storage_dir());
        for variant in &self.registry {
            if variant.engine_id == "voxcpm2" {
                let plugin = crate::plugins::voxcpm2::VoxCpm2Plugin::new(
                    vox_paths.clone(),
                    &variant.name,
                );
                if plugin.is_installed() {
                    eprintln!(
                        "[PluginManager] registering {} (model files present)",
                        variant.name
                    );
                    self.plugins
                        .insert(variant.name.clone(), Arc::new(plugin));
                }
            }
        }
    }

    pub fn list_available_models(&self) -> Vec<String> {
        self.registry.iter().map(|e| e.name.clone()).collect()
    }

    pub fn get_plugin(&self, name: &str) -> Option<Arc<dyn BaseTTSPlugin>> {
        self.plugins.get(name).cloned()
    }

    pub fn refresh_installed(&mut self) {
        self.plugins.clear();
        self.discover_installed_engines();
    }

    pub fn catalogue(&self) -> Vec<EngineInfo> {
        let mut out = Vec::new();

        let configs = models::tts_model_config();
        let qwen_voices = qwen_preset_voices();

        let models_base = crate::config::paths::models_dir().join("qwen3tts");
        let tokenizer_path = models_base
            .join("tokenizer")
            .join("tokenizer-Q4_K_M.gguf");

        for entry in &self.registry {
            if entry.engine_id != "qwen3tts" {
                continue;
            }

            let variant_dir = models_base.join(&entry.name);
            let talker_exists = ["talker-Q4_K_M.gguf", "talker-Q8_0.gguf", "talker-BF16.gguf"]
                .iter()
                .any(|f| variant_dir.join(f).exists());
            let installed = talker_exists && tokenizer_path.exists();

            let _config = configs.get(entry.name.as_str());
            let size_mb = match entry.name.as_str() {
                n if n.contains("0.6B") => 629 + 255,
                n if n.contains("1.7B") => 1220 + 255,
                _ => 884,
            };
            let (display_name, mode_label) = parse_variant_name(&entry.name);

            let voices = if entry.name.contains("CustomVoice") {
                qwen_voices
                    .iter()
                    .map(|(id, desc)| VoiceDescriptor {
                        id: id.to_string(),
                        display_name: desc.to_string(),
                        language: "Auto".to_string(),
                    })
                    .collect()
            } else {
                Vec::new()
            };

            out.push(EngineInfo {
                id: entry.name.clone(),
                display_name: display_name,
                format: "GGUF".into(),
                voice_cloning: entry.name.contains("Base"),
                hardware: vec!["CPU".into(), "CUDA".into(), "Vulkan".into()],
                license: "Apache 2.0".into(),
                languages: vec![
                    "Auto".into(), "Chinese".into(), "English".into(), "German".into(),
                    "Italian".into(), "Portuguese".into(), "Spanish".into(), "Japanese".into(),
                    "Korean".into(), "French".into(), "Russian".into(),
                ],
                installed,
                size_mb,
                voices,
            });

            let _ = mode_label;
        }

        for entry in &self.registry {
            if entry.engine_id != "outetts" {
                continue;
            }

            let oute_base = crate::config::paths::models_dir().join("outetts");
            let variant_dir = oute_base.join(&entry.name);
            let backbone_exists = ["backbone-Q4_K_M.gguf", "backbone-Q8_0.gguf"]
                .iter()
                .any(|f| variant_dir.join(f).exists());
            let dac_exists = oute_base.join("dac").join("decoder.onnx").exists();
            let installed = backbone_exists && dac_exists;

            out.push(EngineInfo {
                id: entry.name.clone(),
                display_name: "OuteTTS 0.6B".into(),
                format: "GGUF + ONNX".into(),
                voice_cloning: true,
                hardware: vec!["CPU".into(), "CUDA".into(), "Vulkan".into()],
                license: "Apache 2.0".into(),
                languages: vec![
                    "Auto".into(), "English".into(), "Chinese".into(), "Dutch".into(),
                    "French".into(), "German".into(), "Hungarian".into(), "Italian".into(),
                    "Japanese".into(), "Korean".into(), "Latvian".into(), "Polish".into(),
                    "Russian".into(), "Spanish".into(),
                ],
                installed,
                size_mb: 402 + 100,
                voices: Vec::new(),
            });
        }

        for entry in &self.registry {
            if entry.engine_id != "voxcpm2" {
                continue;
            }

            let vox_base = crate::config::paths::models_dir().join("voxcpm2");
            let Some(quant_file) = voxcpm2_quant_for_engine(&entry.name) else {
                continue;
            };
            let (acoustic_name, acoustic_size) =
                voxcpm2_acoustic_file().unwrap_or_else(|| (String::new(), 0));
            let base_lm_exists = vox_base
                .join(&quant_file.base_name)
                .join(&quant_file.filename)
                .exists();
            let acoustic_exists = vox_base.join("acoustic").join(&acoustic_name).exists();
            let installed = base_lm_exists && acoustic_exists;

            out.push(EngineInfo {
                id: entry.name.clone(),
                display_name: format!("VoxCPM2 2B {}", quant_file.quant),
                format: "GGUF".into(),
                voice_cloning: true,
                hardware: vec!["CPU".into(), "CUDA".into()],
                license: "Apache 2.0".into(),
                languages: voxcpm2_languages(),
                installed,
                size_mb: quant_file.size_mb + acoustic_size,
                voices: Vec::new(),
            });
        }

        out
    }

    pub fn app_data_dir(&self) -> &PathBuf {
        &self.app_data_dir
    }
}

const ENGINE_REGISTRY_JSON: &str = include_str!("../engine_registry.json");

fn read_generation_params(engine_id: &str) -> serde_json::Map<String, serde_json::Value> {
    let engine_key = if engine_id.starts_with("Qwen3-TTS") {
        "qwen3tts"
    } else if engine_id.starts_with("OuteTTS") {
        "outetts"
    } else if engine_id.starts_with("VoxCPM2") {
        "voxcpm2"
    } else {
        return serde_json::Map::new();
    };

    let Ok(raw) = serde_json::from_str::<serde_json::Value>(ENGINE_REGISTRY_JSON) else {
        return serde_json::Map::new();
    };

    let Some(engine) = raw.get("engines").and_then(|e| e.get(engine_key)) else {
        return serde_json::Map::new();
    };

    let mut params = engine
        .get("parameters")
        .and_then(|p| p.as_object())
        .cloned()
        .unwrap_or_default();

    if let Some(ctx_size) = engine.get("ctx_size") {
        params.insert("ctx_size".to_string(), ctx_size.clone());
    } else if let Some(ctx) = engine.get("context_window") {
        params.insert("ctx_size".to_string(), ctx.clone());
    }

    params
}

fn read_outetts_char_limit() -> u32 {
    let Ok(raw) = serde_json::from_str::<serde_json::Value>(ENGINE_REGISTRY_JSON) else {
        return 350;
    };
    let Some(engine) = raw.get("engines").and_then(|e| e.get("outetts")) else {
        return 350;
    };
    let Some(variant) = engine
        .get("variants")
        .and_then(|v| v.as_array())
        .and_then(|v| v.first())
    else {
        return 350;
    };
    variant
        .get("char_limit_recommended")
        .and_then(|c| c.as_u64())
        .unwrap_or(350) as u32
}

fn read_voxcpm2_char_limit() -> u32 {
    let Ok(raw) = serde_json::from_str::<serde_json::Value>(ENGINE_REGISTRY_JSON) else {
        return 800;
    };
    let Some(engine) = raw.get("engines").and_then(|e| e.get("voxcpm2")) else {
        return 800;
    };
    let Some(variant) = engine
        .get("variants")
        .and_then(|v| v.as_array())
        .and_then(|v| v.first())
    else {
        return 800;
    };
    variant
        .get("char_limit_recommended")
        .and_then(|c| c.as_u64())
        .unwrap_or(800) as u32
}

fn voxcpm2_languages() -> Vec<String> {
    let mut langs = vec!["Auto".to_string()];
    let Ok(raw) = serde_json::from_str::<serde_json::Value>(ENGINE_REGISTRY_JSON) else {
        return langs;
    };
    if let Some(arr) = raw
        .get("engines")
        .and_then(|e| e.get("voxcpm2"))
        .and_then(|e| e.get("languages"))
        .and_then(|l| l.as_array())
    {
        for l in arr {
            if let Some(s) = l.as_str() {
                langs.push(s.to_string());
            }
        }
    }
    langs
}

/// One downloadable BaseLM quantization of the voxcpm2 engine,
/// parsed data-driven from engine_registry.json.
#[derive(Debug, Clone)]
pub struct VoxQuantFile {
    pub base_name: String,
    pub quant: String,
    pub filename: String,
    pub size_mb: u32,
}

/// All voxcpm2 BaseLM quant files from engine_registry.json,
/// sorted by size ascending (smallest = legacy default quant).
pub fn voxcpm2_registry_files() -> Vec<VoxQuantFile> {
    let Ok(raw) = serde_json::from_str::<serde_json::Value>(ENGINE_REGISTRY_JSON) else {
        return Vec::new();
    };
    let Some(variant) = raw
        .get("engines")
        .and_then(|e| e.get("voxcpm2"))
        .and_then(|e| e.get("variants"))
        .and_then(|v| v.as_array())
        .and_then(|v| v.first())
    else {
        return Vec::new();
    };
    let Some(base_name) = variant.get("name").and_then(|n| n.as_str()) else {
        return Vec::new();
    };
    let Some(file) = variant.get("files").and_then(|f| f.as_array()).and_then(|f| f.first()) else {
        return Vec::new();
    };
    let Some(template) = file.get("filename_template").and_then(|t| t.as_str()) else {
        return Vec::new();
    };
    let Some(quants) = file.get("quants").and_then(|q| q.as_object()) else {
        return Vec::new();
    };
    let mut out: Vec<VoxQuantFile> = quants
        .iter()
        .map(|(quant, info)| VoxQuantFile {
            base_name: base_name.to_string(),
            quant: quant.clone(),
            filename: template.replace("{quant}", quant),
            size_mb: info
                .get("size_mb")
                .and_then(|s| s.as_u64())
                .unwrap_or(0) as u32,
        })
        .collect();
    out.sort_by_key(|f| f.size_mb);
    out
}

/// Map a voxcpm2 engine id (e.g. "VoxCPM2 Q8_0") to its BaseLM quant file.
/// The bare variant name ("VoxCPM2", saved before per-quant selection
/// existed) maps to the smallest quant, which was the only one available
/// at that time.
pub fn voxcpm2_quant_for_engine(engine_id: &str) -> Option<VoxQuantFile> {
    let files = voxcpm2_registry_files();
    if let Some(f) = files
        .iter()
        .find(|f| format!("{} {}", f.base_name, f.quant) == engine_id)
    {
        return Some(f.clone());
    }
    if files.first().map(|f| f.base_name.as_str()) == Some(engine_id) {
        return files.into_iter().next();
    }
    None
}

/// Shared voxcpm2 Acoustic file (filename, size_mb) from engine_registry.json.
pub fn voxcpm2_acoustic_file() -> Option<(String, u32)> {
    let raw = serde_json::from_str::<serde_json::Value>(ENGINE_REGISTRY_JSON).ok()?;
    let file = raw
        .get("engines")?
        .get("voxcpm2")?
        .get("shared_files")?
        .as_array()?
        .first()?
        .get("files")?
        .as_array()?
        .first()?;
    let filename = file.get("filename")?.as_str()?.to_string();
    let size_mb = file.get("size_mb")?.as_u64()? as u32;
    Some((filename, size_mb))
}

pub fn defaults_for(engine_id: &str) -> EngineDefaults {
    let configs = models::tts_model_config();
    let generation = read_generation_params(engine_id);

    if engine_id.starts_with("VoxCPM2") {
        return EngineDefaults {
            engine_id: engine_id.into(),
            chunk_strategy: "Character Limit".into(),
            chunk_min_words: None,
            chunk_max_words: None,
            chunk_max_chars: read_voxcpm2_char_limit(),
            chunk_max_chars_by_lang: HashMap::new(),
            separator: ".".into(),
            replace_guillemets: false,
            voice_cloning: true,
            needs_reference_transcript: false,
            supported_languages: voxcpm2_languages(),
            voices: Vec::new(),
            generation,
        };
    }

    if engine_id.starts_with("OuteTTS") {
        return EngineDefaults {
            engine_id: engine_id.into(),
            chunk_strategy: "Character Limit".into(),
            chunk_min_words: None,
            chunk_max_words: None,
            chunk_max_chars: read_outetts_char_limit(),
            chunk_max_chars_by_lang: HashMap::new(),
            separator: ".".into(),
            replace_guillemets: false,
            voice_cloning: true,
            needs_reference_transcript: false,
            supported_languages: vec![
                "Auto".into(), "English".into(), "Chinese".into(), "Dutch".into(),
                "French".into(), "German".into(), "Hungarian".into(), "Italian".into(),
                "Japanese".into(), "Korean".into(), "Latvian".into(), "Polish".into(),
                "Russian".into(), "Spanish".into(),
            ],
            voices: Vec::new(),
            generation,
        };
    }

    let config = configs.get(engine_id);
    let chunk_max_chars = config
        .and_then(|c| c.char_limit_recommended)
        .unwrap_or(800) as u32;

    let separator = config.map(|c| c.separator.clone()).unwrap_or_else(|| ".".into());
    let replace_guillemets = config.map(|c| c.replace_guillemets).unwrap_or(false);
    let voice_cloning = config.and_then(|c| c.voice_cloning).unwrap_or(false);
    let needs_ref = config.and_then(|c| c.needs_reference_transcript).unwrap_or(false);

    let qwen_voices = qwen_preset_voices();
    let voices: Vec<VoiceDescriptor> = qwen_voices
        .iter()
        .map(|(id, desc)| VoiceDescriptor {
            id: id.to_string(),
            display_name: desc.to_string(),
            language: "Auto".to_string(),
        })
        .collect();

    EngineDefaults {
        engine_id: engine_id.into(),
        chunk_strategy: "Character Limit".into(),
        chunk_min_words: None,
        chunk_max_words: None,
        chunk_max_chars,
        chunk_max_chars_by_lang: HashMap::new(),
        separator,
        replace_guillemets,
        voice_cloning,
        needs_reference_transcript: needs_ref,
        supported_languages: vec![
            "Auto".into(), "Chinese".into(), "English".into(), "German".into(),
            "Italian".into(), "Portuguese".into(), "Spanish".into(), "Japanese".into(),
            "Korean".into(), "French".into(), "Russian".into(),
        ],
        voices,
        generation,
    }
}

fn parse_variant_name(name: &str) -> (String, Option<String>) {
    // "Qwen3-TTS-12Hz-0.6B-CustomVoice" -> ("Qwen3-TTS 0.6B Custom Voice", Some("Custom Voice"))
    let parts: Vec<&str> = name.split('-').collect();
    if parts.len() >= 5 {
        let size = parts[3]; // "0.6B" or "1.7B"
        let mode = parts[4..].join(" "); // "CustomVoice" or "Base" or "VoiceDesign"
        let mode_label = match mode.as_str() {
            "Base" => "Voice Clone",
            "CustomVoice" => "Custom Voice",
            "VoiceDesign" => "Voice Design",
            _ => &mode,
        };
        (format!("Qwen3-TTS {} {}", size, mode_label), Some(mode_label.to_string()))
    } else {
        (name.to_string(), None)
    }
}

fn qwen_preset_voices() -> Vec<(&'static str, &'static str)> {
    // Source: backup configuration_tab.py:98
    vec![
        ("Vivian", "Vivian"),
        ("Serena", "Serena"),
        ("Uncle_Fu", "Uncle Fu"),
        ("Dylan", "Dylan"),
        ("Eric", "Eric"),
        ("Ryan", "Ryan"),
        ("Aiden", "Aiden"),
        ("Ono_Anna", "Ono Anna"),
        ("Sohee", "Sohee"),
    ]
}

#[derive(Debug, Clone)]
pub struct VoxCpm2Paths {
    pub models_dir: PathBuf,
    pub acoustic_dir: PathBuf,
}

impl VoxCpm2Paths {
    pub fn from_app_data(app_data: &std::path::Path) -> Self {
        let base = app_data.join("models").join("voxcpm2");
        Self {
            models_dir: base.clone(),
            acoustic_dir: base.join("acoustic"),
        }
    }
}
#[derive(Debug, Clone)]
pub struct QwenPaths {
    pub models_dir: PathBuf,
    pub tokenizer_dir: PathBuf,
}

impl QwenPaths {
    pub fn from_app_data(app_data: &std::path::Path) -> Self {
        let base = app_data.join("models").join("qwen3tts");
        Self {
            models_dir: base.clone(),
            tokenizer_dir: base.join("tokenizer"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vox_quant_for_engine_maps_per_quant_ids() {
        let f = voxcpm2_quant_for_engine("VoxCPM2 Q8_0").expect("Q8_0");
        assert_eq!(f.quant, "Q8_0");
        assert_eq!(f.base_name, "VoxCPM2");
        assert_eq!(f.filename, "VoxCPM2-BaseLM-Q8_0.gguf");
        assert_eq!(f.size_mb, 1647);

        let f = voxcpm2_quant_for_engine("VoxCPM2 F16").expect("F16");
        assert_eq!(f.quant, "F16");
        assert_eq!(f.filename, "VoxCPM2-BaseLM-F16.gguf");
        assert_eq!(f.size_mb, 3097);
    }

    #[test]
    fn vox_quant_for_engine_bare_id_maps_to_smallest_quant() {
        // Legacy engine ids saved before per-quant selection existed.
        let f = voxcpm2_quant_for_engine("VoxCPM2").expect("legacy bare id");
        assert_eq!(f.quant, "Q8_0");
    }

    #[test]
    fn vox_quant_for_engine_rejects_unknown_ids() {
        assert!(voxcpm2_quant_for_engine("VoxCPM2 Q4_K_M").is_none());
        assert!(voxcpm2_quant_for_engine("Qwen3-TTS-12Hz-0.6B-Base").is_none());
    }

    #[test]
    fn vox_acoustic_file_comes_from_registry() {
        let (filename, size_mb) = voxcpm2_acoustic_file().expect("acoustic in registry");
        assert_eq!(filename, "VoxCPM2-Acoustic-F16.gguf");
        assert_eq!(size_mb, 1740);
    }
}
