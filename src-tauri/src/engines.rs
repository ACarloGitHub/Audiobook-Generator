//! Engine trait and registry.
//!
//! Each TTS engine (Kokoro, Qwen3-TTS, OuteTTS, NeuTTS Air) implements
//! `Engine`. The `EngineRegistry` is the single source of truth for
//! which engine is currently loaded. The Tauri commands in
//! `commands.rs` are thin shims over this registry.
//!
//! See AudiobookGenerator-Wiki/wiki/concepts/plugin-architecture.md
//! and AudiobookGenerator-Wiki/wiki/concepts/engine-lifecycle.md.

pub mod kokoro;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use kokoro::KokoroEngine;

/// A request to synthesize a chunk of text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesizeRequest {
    pub text: String,
    pub reference_audio: Option<PathBuf>,
    pub language: Option<String>,
    pub voice: Option<String>,
    pub extra: HashMap<String, String>,
}

/// A handle to a loaded engine. The frontend receives this on
/// `load_engine` and passes it back to `synthesize`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineHandle {
    pub engine_id: String,
    pub model_id: String,
}

/// One engine exposed to the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineInfo {
    pub id: String,
    pub display_name: String,
    pub format: String,             // "ONNX" or "GGUF"
    pub voice_cloning: bool,
    pub hardware: Vec<String>,      // e.g. ["CPU", "CUDA", "Vulkan"]
    pub license: String,
    pub languages: Vec<String>,
}

/// Per-engine auto-loaded defaults. Returned by the `engine_defaults`
/// Tauri command so the frontend can pre-fill Configuration and EPUB
/// Options when the user picks an engine. See
/// AudiobookGenerator-Wiki/wiki/concepts/engine-defaults.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineDefaults {
    pub engine_id: String,
    pub chunk_strategy: String,     // "Word Count Approx" | "Character Limit"
    pub chunk_min_words: Option<u32>,
    pub chunk_max_words: Option<u32>,
    pub chunk_max_chars: u32,
    pub chunk_max_chars_by_lang: std::collections::HashMap<String, u32>,
    pub separator: String,          // "." | "|" | ";" | "<sil>" | "[PAUSE]" | "_"
    pub replace_guillemets: bool,
    pub voices: Vec<VoiceDescriptor>,
    pub voice_cloning: bool,
    pub needs_reference_transcript: bool,
    pub supported_languages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceDescriptor {
    pub id: String,
    pub display_name: String,
    pub language: String,
}

pub fn defaults_for(engine_id: &str) -> EngineDefaults {
    match engine_id {
        "kokoro" => EngineDefaults {
            engine_id: "kokoro".into(),
            chunk_strategy: "Character Limit".into(),
            chunk_min_words: None,
            chunk_max_words: None,
            chunk_max_chars: 2300,
            chunk_max_chars_by_lang: [
                ("it".into(), 2300),
                ("en".into(), 2300),
                ("fr".into(), 2300),
                ("ja".into(), 1100),
                ("zh-cn".into(), 1100),
            ]
            .into_iter()
            .collect(),
            separator: ".".into(),
            replace_guillemets: false,
            voice_cloning: false,
            needs_reference_transcript: false,
            supported_languages: vec![
                "it".into(),
                "en".into(),
                "fr".into(),
                "ja".into(),
                "zh-cn".into(),
            ],
            voices: vec![
                VoiceDescriptor { id: "if_sara".into(), display_name: "Italian Female (Sara)".into(), language: "it".into() },
                VoiceDescriptor { id: "im_nicola".into(), display_name: "Italian Male (Nicola)".into(), language: "it".into() },
                VoiceDescriptor { id: "af_alloy".into(), display_name: "English US Female (Alloy)".into(), language: "en".into() },
                VoiceDescriptor { id: "am_adam".into(), display_name: "English US Male (Adam)".into(), language: "en".into() },
                VoiceDescriptor { id: "ff_siwis".into(), display_name: "French Female (Siwis)".into(), language: "fr".into() },
                VoiceDescriptor { id: "jf_alpha".into(), display_name: "Japanese Female (Alpha)".into(), language: "ja".into() },
                VoiceDescriptor { id: "zf_xiaobei".into(), display_name: "Chinese Female (Xiaobei)".into(), language: "zh-cn".into() },
            ],
        },
        "qwen3-tts" => EngineDefaults {
            engine_id: "qwen3-tts".into(),
            chunk_strategy: "Character Limit".into(),
            chunk_min_words: None,
            chunk_max_words: None,
            chunk_max_chars: 800,
            chunk_max_chars_by_lang: std::collections::HashMap::new(),
            separator: ".".into(),
            replace_guillemets: false,
            voice_cloning: true,
            needs_reference_transcript: true,
            supported_languages: vec![
                "Auto".into(),
                "Chinese".into(),
                "English".into(),
                "German".into(),
                "Italian".into(),
                "Portuguese".into(),
                "Spanish".into(),
                "Japanese".into(),
                "Korean".into(),
                "French".into(),
                "Russian".into(),
            ],
            voices: vec![
                VoiceDescriptor { id: "Vivian".into(), display_name: "Vivian".into(), language: "Auto".into() },
                VoiceDescriptor { id: "Serena".into(), display_name: "Serena".into(), language: "Auto".into() },
                VoiceDescriptor { id: "Uncle_Fu".into(), display_name: "Uncle Fu".into(), language: "Auto".into() },
                VoiceDescriptor { id: "Dylan".into(), display_name: "Dylan".into(), language: "Auto".into() },
                VoiceDescriptor { id: "Eric".into(), display_name: "Eric".into(), language: "Auto".into() },
                VoiceDescriptor { id: "Ryan".into(), display_name: "Ryan".into(), language: "Auto".into() },
                VoiceDescriptor { id: "Aiden".into(), display_name: "Aiden".into(), language: "Auto".into() },
                VoiceDescriptor { id: "Ono_Anna".into(), display_name: "Ono Anna".into(), language: "Auto".into() },
                VoiceDescriptor { id: "Sohee".into(), display_name: "Sohee".into(), language: "Auto".into() },
            ],
        },
        "outetts" => EngineDefaults {
            engine_id: "outetts".into(),
            chunk_strategy: "Character Limit".into(),
            chunk_min_words: None,
            chunk_max_words: None,
            chunk_max_chars: 500,
            chunk_max_chars_by_lang: std::collections::HashMap::new(),
            separator: ".".into(),
            replace_guillemets: false,
            voice_cloning: true,
            needs_reference_transcript: true,
            supported_languages: vec![
                "en".into(), "ar".into(), "zh".into(), "nl".into(),
                "fr".into(), "de".into(), "it".into(), "ja".into(),
                "ko".into(), "lt".into(), "ru".into(), "es".into(),
            ],
            voices: vec![],
        },
        "neutts-air" => EngineDefaults {
            engine_id: "neutts-air".into(),
            chunk_strategy: "Character Limit".into(),
            chunk_min_words: None,
            chunk_max_words: None,
            chunk_max_chars: 250,
            chunk_max_chars_by_lang: std::collections::HashMap::new(),
            separator: ".".into(),
            replace_guillemets: false,
            voice_cloning: true,
            needs_reference_transcript: false,
            supported_languages: vec!["en".into()],
            voices: vec![],
        },
        _ => EngineDefaults {
            engine_id: engine_id.into(),
            chunk_strategy: "Character Limit".into(),
            chunk_min_words: None,
            chunk_max_words: None,
            chunk_max_chars: 500,
            chunk_max_chars_by_lang: std::collections::HashMap::new(),
            separator: ".".into(),
            replace_guillemets: false,
            voice_cloning: false,
            needs_reference_transcript: false,
            supported_languages: vec!["en".into()],
            voices: vec![],
        },
    }
}

/// The trait every engine implements.
pub trait Engine: Send + Sync {
    fn info(&self) -> &EngineInfo;
    fn is_loaded(&self) -> bool;
    fn load(&self, model_id: &str) -> anyhow::Result<EngineHandle>;
    fn synthesize(
        &self,
        handle: &EngineHandle,
        request: &SynthesizeRequest,
        output_wav: &Path,
    ) -> anyhow::Result<()>;
    fn unload(&self, handle: &EngineHandle) -> anyhow::Result<()>;
    fn current_vram_bytes(&self) -> Option<u64>;
    /// Book-level synthesis entry point. Default is `None`; engines
    /// that have a top-level "process this EPUB" command override
    /// this. The frontend dispatches on the engine id and calls the
    /// matching `start_*_generation` Tauri command.
    fn as_kokoro(&self) -> Option<&kokoro::KokoroEngine> {
        None
    }
}

/// Global engine registry. One Engine impl per engine id. The active
/// handle (if any) is recorded here.
#[derive(Clone)]
pub struct EngineRegistry {
    inner: Arc<Mutex<RegistryInner>>,
}

struct RegistryInner {
    engines: HashMap<String, Arc<dyn Engine>>,
    active: Option<EngineHandle>,
}

impl EngineRegistry {
    pub fn new() -> Self {
        let mut r = Self {
            inner: Arc::new(Mutex::new(RegistryInner {
                engines: HashMap::new(),
                active: None,
            })),
        };

        // Kokoro: the only engine with a real implementation so far.
        // The paths default to `<app_data>/kokoro/{models,voices}`.
        // The First-Run Wizard populates these directories.
        let paths = KokoroEngine::default_data_paths();
        let kokoro = Arc::new(KokoroEngine::new(paths, "af_heart"));
        r.register(kokoro);

        // Stubs for the other engines. Each carries the real
        // `EngineInfo` so the Models panel can render the catalogue
        // accurately; they all bail on `load()` until the next
        // plugin lands.
        r.register(Arc::new(QwenEngineStub));
        r.register(Arc::new(OuteTtsEngineStub));
        r.register(Arc::new(NeuttsEngineStub));

        r
    }

    pub fn register(&mut self, engine: Arc<dyn Engine>) {
        let id = engine.info().id.to_string();
        self.inner.lock().unwrap().engines.insert(id, engine);
    }

    pub fn list(&self) -> Vec<EngineInfo> {
        let g = self.inner.lock().unwrap();
        let mut out: Vec<EngineInfo> =
            g.engines.values().map(|e| e.info().clone()).collect();
        out.sort_by(|a, b| a.display_name.cmp(&b.display_name));
        out
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn Engine>> {
        self.inner.lock().unwrap().engines.get(id).cloned()
    }

    pub fn active(&self) -> Option<EngineHandle> {
        self.inner.lock().unwrap().active.clone()
    }

    pub fn set_active(&self, handle: Option<EngineHandle>) {
        self.inner.lock().unwrap().active = handle;
    }
}

impl Default for EngineRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Stubs for engines that are not yet implemented -------------

struct QwenEngineStub;
struct OuteTtsEngineStub;
struct NeuttsEngineStub;

impl QwenEngineStub {
    fn info_static() -> EngineInfo {
        EngineInfo {
            id: "qwen3-tts".into(),
            display_name: "Qwen3-TTS".into(),
            format: "GGUF".into(),
            voice_cloning: true,
            hardware: vec!["CPU".into(), "CUDA".into(), "Vulkan".into()],
            license: "Apache 2.0".into(),
            languages: vec![
                "zh".into(), "en".into(), "ja".into(), "ko".into(),
                "de".into(), "fr".into(), "ru".into(), "pt".into(),
                "es".into(), "it".into(),
            ],
        }
    }
}

impl Engine for QwenEngineStub {
    fn info(&self) -> &EngineInfo {
        static I: std::sync::OnceLock<Box<EngineInfo>> = std::sync::OnceLock::new();
        I.get_or_init(|| Box::new(Self::info_static()))
    }
    fn is_loaded(&self) -> bool { false }
    fn load(&self, _model_id: &str) -> anyhow::Result<EngineHandle> {
        anyhow::bail!("Qwen3-TTS engine not yet implemented")
    }
    fn synthesize(&self, _: &EngineHandle, _: &SynthesizeRequest, _: &Path) -> anyhow::Result<()> {
        anyhow::bail!("Qwen3-TTS engine not yet implemented")
    }
    fn unload(&self, _: &EngineHandle) -> anyhow::Result<()> { Ok(()) }
    fn current_vram_bytes(&self) -> Option<u64> { None }
}

impl OuteTtsEngineStub {
    fn info_static() -> EngineInfo {
        EngineInfo {
            id: "outetts".into(),
            display_name: "OuteTTS 1.0".into(),
            format: "GGUF".into(),
            voice_cloning: true,
            hardware: vec!["CPU".into(), "CUDA".into()],
            license: "CC-BY-NC-SA-4.0".into(),
            languages: vec![
                "en".into(), "ar".into(), "zh".into(), "nl".into(),
                "fr".into(), "de".into(), "it".into(), "ja".into(),
                "ko".into(), "lt".into(), "ru".into(), "es".into(),
            ],
        }
    }
}

impl Engine for OuteTtsEngineStub {
    fn info(&self) -> &EngineInfo {
        static I: std::sync::OnceLock<Box<EngineInfo>> = std::sync::OnceLock::new();
        I.get_or_init(|| Box::new(Self::info_static()))
    }
    fn is_loaded(&self) -> bool { false }
    fn load(&self, _model_id: &str) -> anyhow::Result<EngineHandle> {
        anyhow::bail!("OuteTTS engine not yet implemented")
    }
    fn synthesize(&self, _: &EngineHandle, _: &SynthesizeRequest, _: &Path) -> anyhow::Result<()> {
        anyhow::bail!("OuteTTS engine not yet implemented")
    }
    fn unload(&self, _: &EngineHandle) -> anyhow::Result<()> { Ok(()) }
    fn current_vram_bytes(&self) -> Option<u64> { None }
}

impl NeuttsEngineStub {
    fn info_static() -> EngineInfo {
        EngineInfo {
            id: "neutts-air".into(),
            display_name: "NeuTTS Air".into(),
            format: "GGUF".into(),
            voice_cloning: true,
            hardware: vec!["CPU".into()],
            license: "Apache 2.0".into(),
            languages: vec!["en".into()],
        }
    }
}

impl Engine for NeuttsEngineStub {
    fn info(&self) -> &EngineInfo {
        static I: std::sync::OnceLock<Box<EngineInfo>> = std::sync::OnceLock::new();
        I.get_or_init(|| Box::new(Self::info_static()))
    }
    fn is_loaded(&self) -> bool { false }
    fn load(&self, _model_id: &str) -> anyhow::Result<EngineHandle> {
        anyhow::bail!("NeuTTS Air engine not yet implemented")
    }
    fn synthesize(&self, _: &EngineHandle, _: &SynthesizeRequest, _: &Path) -> anyhow::Result<()> {
        anyhow::bail!("NeuTTS Air engine not yet implemented")
    }
    fn unload(&self, _: &EngineHandle) -> anyhow::Result<()> { Ok(()) }
    fn current_vram_bytes(&self) -> Option<u64> { None }
}
