//! Centralized resolution of bundled sidecar binaries.
//!
//! All engine binaries (qwen-tts, voxcpm2-cli, llama-server, ffmpeg) ship
//! INSIDE the installer as Tauri bundle resources, so the app works offline
//! out of the box. The lookup order is:
//!
//! 1. Bundled resource dir next to the installed executable
//!    (`<install>/resources/<name>` on Windows/Linux,
//!    `<App>.app/Contents/Resources/resources/<name>` on macOS).
//! 2. Legacy per-user data dir (`%APPDATA%\com.patata.audiobookgenerator\
//!    resources\<name>` on Windows, `~/.local/share/...` on Linux) for
//!    backward compatibility with dev installs that downloaded sidecars at
//!    runtime.
//! 3. Development fallback: `resources/<name>` relative to the current
//!    working directory.
//!
//! Only GGUF model weights are ever downloaded at runtime (Models panel).

use std::path::PathBuf;

/// Candidate directories for a sidecar named `name`, in priority order.
pub fn sidecar_dir_candidates(name: &str) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();

    // 1. Bundled resources, next to the installed executable.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            // Windows (NSIS/MSI) and Linux: resources sit next to the binary.
            dirs.push(exe_dir.join("resources").join(name));
            // macOS: <App>.app/Contents/MacOS/<exe> → Contents/Resources.
            if let Some(contents) = exe_dir.parent() {
                dirs.push(contents.join("Resources").join("resources").join(name));
            }
        }
    }

    // 2. Legacy per-user app data dir (dev installs / backward compat).
    if let Ok(app_data) = std::env::var("APPDATA") {
        dirs.push(
            PathBuf::from(app_data)
                .join("com.patata.audiobookgenerator")
                .join("resources")
                .join(name),
        );
    }
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(
            PathBuf::from(home)
                .join(".local/share/com.patata.audiobookgenerator")
                .join("resources")
                .join(name),
        );
    }

    // 3. Development fallback: relative path from the working directory.
    dirs.push(PathBuf::from("resources").join(name));

    dirs
}

/// First existing directory for sidecar `name`, in priority order.
pub fn sidecar_dir(name: &str) -> Option<PathBuf> {
    sidecar_dir_candidates(name).into_iter().find(|p| p.exists())
}

/// Resolve a sidecar binary (e.g. `sidecar_binary("qwentts", "qwen-tts.exe")`).
pub fn sidecar_binary(name: &str, exe_name: &str) -> Option<PathBuf> {
    sidecar_dir_candidates(name)
        .into_iter()
        .map(|d| d.join(exe_name))
        .find(|p| p.exists())
}
