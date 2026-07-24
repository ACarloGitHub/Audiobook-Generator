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
use std::process::{Command, Stdio};
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
    // On Windows the ggml probe reports the static WDDM budget, which never
    // moves (Carlo's bug, 2026-07-22). Performance counters give the real,
    // Task-Manager-like usage; fall back to the probe if they fail.
    #[cfg(windows)]
    if let Some(devs) = windows_counters_devices() {
        return Ok(devs);
    }
    Ok(parse_devices(&raw_devices_output()?))
}

/// Real GPU memory usage on Windows from performance counters
/// ("\GPU Process Memory(*)\Dedicated Usage", summed across processes).
/// The device name/total come from the ggml probe, read once.
#[cfg(windows)]
fn windows_counters_devices() -> Option<Vec<GpuDevice>> {
    static BASE: OnceLock<Option<GpuDevice>> = OnceLock::new();
    let base = BASE
        .get_or_init(|| {
            parse_devices(&raw_devices_output().ok()?)
                .into_iter()
                .find(|d| d.total_mib > 0)
        })
        .clone()?;

    let mut cmd = std::process::Command::new("powershell");
    crate::utils::hide_console_window(&mut cmd);
    let child = cmd
        .args([
            "-NoProfile",
            "-Command",
            "((Get-Counter '\\GPU Process Memory(*)\\Dedicated Usage' -ErrorAction SilentlyContinue).CounterSamples | Measure-Object -Property CookedValue -Sum).Sum",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    crate::job_object::assign_child(&child);
    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let used_bytes: f64 = text.trim().parse().ok()?;
    let used_mib = (used_bytes / 1_048_576.0) as u64;
    Some(vec![GpuDevice {
        backend: base.backend,
        name: base.name,
        total_mib: base.total_mib,
        free_mib: base.total_mib.saturating_sub(used_mib),
    }])
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
    let child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to run llama-server --list-devices: {}", e))?;
    crate::job_object::assign_child(&child);
    let output = child
        .wait_with_output()
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
