//! GPU-only guard.
//!
//! Audiobook Generator requires a GPU — dedicated or integrated with
//! unified memory (Apple Silicon, AMD AI Max+, etc.). CPU-only machines
//! are not supported: when no GPU backend is visible to ggml, synthesis
//! must fail with a clear error instead of silently falling back to CPU
//! (Carlo's rule, 2026-07-22).
//!
//! All engines run on the same ggml backends as the bundled llama-server,
//! so probing `llama-server --list-devices` once covers every engine.

use anyhow::{bail, Result};
use std::process::Command;
use std::sync::OnceLock;

/// Verify that at least one GPU backend device is visible.
///
/// The probe result is cached for the whole app run: hardware does not
/// change while the app is open, and re-probing would add latency to
/// every synthesis call.
pub fn ensure_gpu() -> Result<()> {
    static RESULT: OnceLock<std::result::Result<(), String>> = OnceLock::new();
    match RESULT.get_or_init(|| probe().map_err(|e| e.to_string())) {
        Ok(()) => Ok(()),
        Err(msg) => bail!(msg.clone()),
    }
}

fn probe() -> Result<()> {
    let exe_name = if cfg!(windows) {
        "llama-server.exe"
    } else {
        "llama-server"
    };
    let binary = crate::sidecars::sidecar_binary("llama.cpp", exe_name)
        .ok_or_else(|| anyhow::anyhow!("llama-server binary not found; cannot verify GPU"))?;

    let mut dirs = Vec::new();
    if let Some(dir) = binary.parent() {
        dirs.push(dir.to_path_buf());
    }
    if let Some(cuda_dir) = crate::sidecars::sidecar_dir("cuda-shared") {
        dirs.push(cuda_dir);
    }

    let mut cmd = Command::new(&binary);
    cmd.arg("--list-devices");
    crate::sidecars::apply_loader_path(&mut cmd, &dirs);
    let output = cmd
        .output()
        .map_err(|e| anyhow::anyhow!("failed to run llama-server --list-devices: {}", e))?;

    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // ggml prints one line per GPU device, prefixed by the backend name
    // (e.g. "CUDA0: NVIDIA GeForce RTX 3090", "Vulkan0: ...", "Metal: ...").
    const GPU_BACKENDS: [&str; 6] = ["CUDA", "Vulkan", "Metal", "ROCm", "HIP", "SYCL"];
    if GPU_BACKENDS.iter().any(|b| text.contains(b)) {
        Ok(())
    } else {
        bail!(
            "No compatible GPU detected. Audiobook Generator requires a GPU \
             (dedicated, or integrated with unified memory such as Apple Silicon \
             or AMD AI Max). CPU-only machines are not supported."
        )
    }
}
