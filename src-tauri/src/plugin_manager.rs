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
        for entry in &self.registry {
            let models_dir = self.app_data_dir.join("models").join(&entry.engine_id);
            if models_dir.exists() {
                eprintln!("[PluginManager] found installed engine: {}", entry.name);
            }
        }
    }

    pub fn list_available_models(&self) -> Vec<String> {
        self.registry.iter().map(|e| e.name.clone()).collect()
    }

    pub fn get_plugin(&self, name: &str) -> Option<Arc<dyn BaseTTSPlugin>> {
        self.plugins.get(name).cloned()
    }

    pub fn catalogue(&self) -> Vec<EngineInfo> {
        let mut out = Vec::new();
        let kokoro_installed = {
            let kokoro_models = self.app_data_dir.join("models").join("kokoro").join("models");
            let kokoro_voices = self.app_data_dir.join("models").join("kokoro").join("voices");
            let model_ok = ["model_quantized.onnx", "model_q8f16.onnx", "model.onnx"]
                .iter()
                .any(|n| kokoro_models.join(n).exists());
            let voices_ok = kokoro_voices.join("af_heart.bin").exists();
            model_ok && voices_ok
        };

        if kokoro_installed {
            let kokoro_voices = models::available_kokoro_models();
            let mut all_voices: Vec<VoiceDescriptor> = Vec::new();
            let mut all_langs: Vec<String> = Vec::new();
            for (lang, entry) in &kokoro_voices {
                all_langs.push(lang.to_string());
                for v in &entry.voices {
                    all_voices.push(VoiceDescriptor {
                        id: v.id.clone(),
                        display_name: v.description.clone(),
                        language: lang.to_string(),
                    });
                }
            }
            out.push(EngineInfo {
                id: "kokoro".into(),
                display_name: "Kokoro 82M".into(),
                format: "ONNX".into(),
                voice_cloning: false,
                hardware: vec!["CPU".into(), "CUDA".into()],
                license: "Apache 2.0".into(),
                languages: all_langs,
                installed: true,
                size_mb: 92,
                voices: all_voices,
            });
        }

        out
    }

    pub fn app_data_dir(&self) -> &PathBuf {
        &self.app_data_dir
    }

    pub fn models_dir(&self) -> PathBuf {
        self.app_data_dir.join("models")
    }
}

pub fn defaults_for(engine_id: &str) -> EngineDefaults {
    let configs = models::tts_model_config();
    let kokoro_models = models::available_kokoro_models();

    match engine_id {
        "kokoro" => {
            let char_limits = configs.get("Kokoro").and_then(|c| c.char_limits_by_lang.as_ref());
            let mut by_lang: HashMap<String, u32> = HashMap::new();
            if let Some(limits) = char_limits {
                for (lang, range) in limits {
                    by_lang.insert(lang.clone(), range.max as u32);
                }
            }
            let mut all_voices: Vec<VoiceDescriptor> = Vec::new();
            let mut all_langs: Vec<String> = Vec::new();
            for (lang, entry) in &kokoro_models {
                all_langs.push(lang.to_string());
                for v in &entry.voices {
                    all_voices.push(VoiceDescriptor {
                        id: v.id.clone(),
                        display_name: v.description.clone(),
                        language: lang.to_string(),
                    });
                }
            }
            EngineDefaults {
                engine_id: "kokoro".into(),
                chunk_strategy: "Character Limit".into(),
                chunk_min_words: None,
                chunk_max_words: None,
                chunk_max_chars: 2300,
                chunk_max_chars_by_lang: by_lang,
                separator: ".".into(),
                replace_guillemets: false,
                voice_cloning: false,
                needs_reference_transcript: false,
                supported_languages: all_langs,
                voices: all_voices,
            }
        }
        "qwen3tts" => EngineDefaults {
            engine_id: "qwen3tts".into(),
            chunk_strategy: "Character Limit".into(),
            chunk_min_words: None,
            chunk_max_words: None,
            chunk_max_chars: 800,
            chunk_max_chars_by_lang: HashMap::new(),
            separator: ".".into(),
            replace_guillemets: false,
            voice_cloning: true,
            needs_reference_transcript: true,
            supported_languages: vec![
                "Auto".into(), "Chinese".into(), "English".into(), "German".into(),
                "Italian".into(), "Portuguese".into(), "Spanish".into(), "Japanese".into(),
                "Korean".into(), "French".into(), "Russian".into(),
            ],
            voices: vec![
                VoiceDescriptor { id: "Vivian".into(), display_name: "Vivian".into(), language: "Auto".into() },
                VoiceDescriptor { id: "Serena".into(), display_name: "Serena".into(), language: "Auto".into() },
                VoiceDescriptor { id: "Uncle_Fu".into(), display_name: "Uncle Fu".into(), language: "Auto".into() },
            ],
        },
        "vibevoice" => EngineDefaults {
            engine_id: "vibevoice".into(),
            chunk_strategy: "Character Limit".into(),
            chunk_min_words: None,
            chunk_max_words: None,
            chunk_max_chars: 750,
            chunk_max_chars_by_lang: HashMap::new(),
            separator: ".".into(),
            replace_guillemets: false,
            voice_cloning: true,
            needs_reference_transcript: false,
            supported_languages: vec!["en".into()],
            voices: vec![],
        },
        "xttsv2" => EngineDefaults {
            engine_id: "xttsv2".into(),
            chunk_strategy: "Character Limit".into(),
            chunk_min_words: None,
            chunk_max_words: None,
            chunk_max_chars: 250,
            chunk_max_chars_by_lang: HashMap::new(),
            separator: "|".into(),
            replace_guillemets: true,
            voice_cloning: true,
            needs_reference_transcript: true,
            supported_languages: vec!["en".into(), "it".into(), "fr".into(), "de".into(), "es".into()],
            voices: vec![],
        },
        _ => EngineDefaults {
            engine_id: engine_id.into(),
            chunk_strategy: "Character Limit".into(),
            chunk_min_words: None,
            chunk_max_words: None,
            chunk_max_chars: 500,
            chunk_max_chars_by_lang: HashMap::new(),
            separator: ".".into(),
            replace_guillemets: false,
            voice_cloning: false,
            needs_reference_transcript: false,
            supported_languages: vec!["en".into()],
            voices: vec![],
        },
    }
}