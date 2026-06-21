//! Engine trait and registry.
//!
//! Each TTS engine (Kokoro, Qwen3-TTS, OuteTTS, NeuTTS Air) implements
//! `Engine`. The `EngineRegistry` is the single source of truth for
//! which engine is currently loaded. The Tauri commands in
//! `commands.rs` are thin shims over this registry.
//!
//! See AudiobookGenerator-Wiki/wiki/concepts/plugin-architecture.md
//! and AudiobookGenerator-Wiki/wiki/concepts/engine-lifecycle.md.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

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

/// The trait every engine implements. Stub for now; Kokoro lands in
/// the next commit when we absorb the prototype's `kokoro.rs`.
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
}

/// Global engine registry. One Engine impl per engine id. The active
/// handle (if any) is recorded here.
#[derive(Default)]
pub struct EngineRegistry {
    inner: Mutex<RegistryInner>,
}

#[derive(Default)]
struct RegistryInner {
    engines: HashMap<String, Arc<dyn Engine>>,
    active: Option<EngineHandle>,
}

impl EngineRegistry {
    pub fn new() -> Self {
        let mut r = Self::default();
        r.register(Arc::new(KokoroEngineStub));
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

// Stubs ------------------------------------------------------------

struct KokoroEngineStub;
struct QwenEngineStub;
struct OuteTtsEngineStub;
struct NeuttsEngineStub;

impl KokoroEngineStub {
    fn info_static() -> EngineInfo {
        EngineInfo {
            id: "kokoro".into(),
            display_name: "Kokoro 82M".into(),
            format: "ONNX".into(),
            voice_cloning: false,
            hardware: vec!["CPU".into(), "CUDA".into()],
            license: "Apache 2.0".into(),
            languages: vec![
                "en".into(), "ja".into(), "zh".into(), "es".into(),
                "fr".into(), "hi".into(), "it".into(), "pt".into(),
            ],
        }
    }
}

impl Engine for KokoroEngineStub {
    fn info(&self) -> &EngineInfo {
        // EngineInfo is cheap to clone, but we keep a static to avoid
        // re-allocating on every call.
        static I: std::sync::OnceLock<Box<EngineInfo>> = std::sync::OnceLock::new();
        I.get_or_init(|| Box::new(Self::info_static()))
    }
    fn is_loaded(&self) -> bool { false }
    fn load(&self, _model_id: &str) -> anyhow::Result<EngineHandle> {
        anyhow::bail!("Kokoro engine not yet implemented; absorbed from prototype in next step")
    }
    fn synthesize(&self, _: &EngineHandle, _: &SynthesizeRequest, _: &Path) -> anyhow::Result<()> {
        anyhow::bail!("Kokoro engine not yet implemented")
    }
    fn unload(&self, _: &EngineHandle) -> anyhow::Result<()> { Ok(()) }
    fn current_vram_bytes(&self) -> Option<u64> { None }
}

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
