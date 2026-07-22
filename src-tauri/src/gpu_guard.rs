//! GPU-only guard and GPU memory probing.
//!
//! Audiobook Generator requires a GPU — dedicated or integrated with
//! unified memory (Apple Silicon, AMD AI Max+, etc.). CPU-only machines
//! are not supported: when no GPU backend is visible to ggml, synthesis
//! must fail with a clear error instead of silently falling back to CPU
//! (Carlo's rule, 2026-07-22).
//!
//! All engines run on the same ggml backends as the bundled llama-server,
//! so probing `llama-server --list-devices` covers every engine and every
//! GPU vendor (CUDA, Vulkan, Metal) with one command.

use anyhow::{bail, Result};
use std::process::Command;
use std::sync::OnceLock;

/// One GPU device as reported by ggml (`--list-devices`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct GpuDevice {
    pub backend: String,
    pub name: String,
    pub total_mib: u64,
    pub free_mib: u64,
}

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

/// Live GPU device list with memory figures (not cached: values change
/// while an engine is loaded). Used by the VRAM bar in the UI.
pub fn gpu_devices() -> Result<Vec<GpuDevice>> {
    Ok(parse_devices(&raw_devices_output()?))
}

fn probe() -> Result<()> {
    let text = raw_devices_output()?;
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

fn raw_devices_output() -> Result<String> {
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
    crate::utils::hide_console_window(&mut cmd);
    cmd.arg("--list-devices");
    crate::sidecars::apply_loader_path(&mut cmd, &dirs);
    let output = cmd
        .output()
        .map_err(|e| anyhow::anyhow!("failed to run llama-server --list-devices: {}", e))?;

    Ok(format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

/// Parse lines like:
/// `  CUDA0: NVIDIA GeForce RTX 3090 (24575 MiB, 23335 MiB free)`
/// Output format varies slightly between ggml backends; lines that do
/// not match are skipped.
fn parse_devices(text: &str) -> Vec<GpuDevice> {
    const GPU_BACKENDS: [&str; 6] = ["CUDA", "Vulkan", "Metal", "ROCm", "HIP", "SYCL"];
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(backend) = GPU_BACKENDS.iter().find(|b| line.starts_with(*b)) else {
            continue;
        };
        let Some(colon) = line.find(':') else { continue };
        let rest = line[colon + 1..].trim();
        // Name is everything before the parenthesised memory figures.
        let (name, mem) = match rest.find('(') {
            Some(p) => (rest[..p].trim().to_string(), &rest[p..]),
            None => (rest.to_string(), ""),
        };
        let nums: Vec<u64> = mem
            .split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        let (total_mib, free_mib) = (nums.first().copied().unwrap_or(0), nums.get(1).copied().unwrap_or(0));
        out.push(GpuDevice {
            backend: backend.to_string(),
            name,
            total_mib,
            free_mib,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cuda_line() {
        let text = "Available devices:\n  CUDA0: NVIDIA GeForce RTX 3090 (24575 MiB, 23335 MiB free)\n";
        let devs = parse_devices(text);
        assert_eq!(devs.len(), 1);
        assert_eq!(devs[0].backend, "CUDA");
        assert_eq!(devs[0].name, "NVIDIA GeForce RTX 3090");
        assert_eq!(devs[0].total_mib, 24575);
        assert_eq!(devs[0].free_mib, 23335);
    }
}
