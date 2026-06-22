use anyhow::{bail, Result};
use async_trait::async_trait;

use crate::base_plugin::{BaseTTSPlugin, EngineHandle, SynthesizeRequest};

pub struct Qwen3TTSPlugin {
    pub model_name: String,
    pub model_size: String,
    pub model_type: String,
}

impl Qwen3TTSPlugin {
    pub fn new(model_name: &str, model_size: &str, model_type: &str) -> Self {
        Self {
            model_name: model_name.to_string(),
            model_size: model_size.to_string(),
            model_type: model_type.to_string(),
        }
    }
}

#[async_trait]
impl BaseTTSPlugin for Qwen3TTSPlugin {
    fn name(&self) -> &str {
        &self.model_name
    }

    fn plugin_type(&self) -> &str {
        "llama_server"
    }

    fn is_installed(&self) -> bool {
        false
    }

    async fn load_model(&self, _model_id: &str) -> Result<EngineHandle> {
        bail!("Qwen3-TTS plugin ({}) not yet implemented. Will use llama-server HTTP.", self.model_name)
    }

    async fn synthesize(&self, _handle: &EngineHandle, _request: &SynthesizeRequest) -> Result<()> {
        bail!("Qwen3-TTS synthesis not yet implemented")
    }

    async fn unload(&self, _handle: &EngineHandle) -> Result<()> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}