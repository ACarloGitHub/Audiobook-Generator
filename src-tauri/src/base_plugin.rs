use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::any::Any;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineHandle {
    pub engine_id: String,
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesizeRequest {
    pub text: String,
    pub output_path: String,
    pub reference_audio: Option<String>,
    pub language: Option<String>,
    pub voice: Option<String>,
    pub extra: std::collections::HashMap<String, String>,
}

#[async_trait]
pub trait BaseTTSPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn plugin_type(&self) -> &str;
    fn is_installed(&self) -> bool;

    async fn load_model(&self, model_id: &str) -> Result<EngineHandle>;
    async fn synthesize(&self, handle: &EngineHandle, request: &SynthesizeRequest) -> Result<()>;
    async fn unload(&self, handle: &EngineHandle) -> Result<()>;

    fn as_any(&self) -> &dyn Any;
}