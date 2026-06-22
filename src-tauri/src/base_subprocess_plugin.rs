use anyhow::{bail, Result};
use async_trait::async_trait;
use std::path::PathBuf;

use crate::base_plugin::{BaseTTSPlugin, EngineHandle, SynthesizeRequest};

pub struct BaseSubprocessPlugin {
    pub name: String,
    pub plugin_type: String,
    pub model_dir: PathBuf,
}

impl BaseSubprocessPlugin {
    pub fn get_python_executable(&self) -> PathBuf {
        let venv_dir = self.model_dir.join("venv");
        if cfg!(windows) {
            venv_dir.join("Scripts").join("python.exe")
        } else {
            venv_dir.join("bin").join("python")
        }
    }

    pub fn get_script_path(&self) -> PathBuf {
        self.model_dir.join("synthesize_subprocess.py")
    }

    pub fn check_venv_integrity(&self) -> bool {
        let exe = self.get_python_executable();
        exe.exists()
    }
}

#[async_trait]
impl BaseTTSPlugin for BaseSubprocessPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn plugin_type(&self) -> &str {
        &self.plugin_type
    }

    fn is_installed(&self) -> bool {
        self.check_venv_integrity()
    }

    async fn load_model(&self, _model_id: &str) -> Result<EngineHandle> {
        if !self.check_venv_integrity() {
            bail!("venv integrity check failed for {}", self.name);
        }
        Ok(EngineHandle {
            engine_id: self.name.clone(),
            model_id: _model_id.to_string(),
        })
    }

    async fn synthesize(&self, handle: &EngineHandle, request: &SynthesizeRequest) -> Result<()> {
        let script = self.get_script_path();
        let python = self.get_python_executable();
        if !python.exists() {
            bail!("python executable not found: {}", python.display());
        }
        if !script.exists() {
            bail!("subprocess script not found: {}", script.display());
        }
        bail!("BaseSubprocessPlugin::synthesize is a placeholder. Use the specific engine plugin.");
    }

    async fn unload(&self, _handle: &EngineHandle) -> Result<()> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}