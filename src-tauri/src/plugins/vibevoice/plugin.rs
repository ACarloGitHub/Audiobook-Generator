use anyhow::{bail, Result};
use async_trait::async_trait;

use crate::base_plugin::{BaseTTSPlugin, EngineHandle, SynthesizeRequest};

pub struct VibeVoicePlugin {
    pub model_name: String,
}

impl VibeVoicePlugin {
    pub fn new(model_name: &str) -> Self {
        Self {
            model_name: model_name.to_string(),
        }
    }
}

#[async_trait]
impl BaseTTSPlugin for VibeVoicePlugin {
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
        bail!("VibeVoice plugin ({}) not yet implemented. Will use llama-server HTTP.", self.model_name)
    }

    async fn synthesize(&self, _handle: &EngineHandle, _request: &SynthesizeRequest) -> Result<()> {
        bail!("VibeVoice synthesis not yet implemented")
    }

    async fn unload(&self, _handle: &EngineHandle) -> Result<()> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}