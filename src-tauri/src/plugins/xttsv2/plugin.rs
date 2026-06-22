use anyhow::{bail, Result};
use async_trait::async_trait;

use crate::base_plugin::{BaseTTSPlugin, EngineHandle, SynthesizeRequest};

pub struct XTTSv2Plugin;

impl XTTSv2Plugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl BaseTTSPlugin for XTTSv2Plugin {
    fn name(&self) -> &str {
        "XTTSv2"
    }

    fn plugin_type(&self) -> &str {
        "llama_server"
    }

    fn is_installed(&self) -> bool {
        false
    }

    async fn load_model(&self, _model_id: &str) -> Result<EngineHandle> {
        bail!("XTTSv2 plugin not yet implemented. Will use llama-server HTTP.")
    }

    async fn synthesize(&self, _handle: &EngineHandle, _request: &SynthesizeRequest) -> Result<()> {
        bail!("XTTSv2 synthesis not yet implemented")
    }

    async fn unload(&self, _handle: &EngineHandle) -> Result<()> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}