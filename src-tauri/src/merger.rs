//! Concatenate a list of WAV files into a single MP3 via ffmpeg.
//!
//! ffmpeg is treated as an external binary; the Tauri shell-out is
//! the only place we depend on a real OS process. The frontend never
//! invokes ffmpeg directly; it goes through the engine's `synthesize`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use tracing::debug;

/// Concatenate the given WAV files into a single MP3 at `out_mp3`,
/// using the ffmpeg binary at `ffmpeg`.
pub fn merge_wavs_to_mp3(wavs: &[PathBuf], out_mp3: &Path, ffmpeg: &Path) -> Result<()> {
    if wavs.is_empty() {
        anyhow::bail!("cannot merge zero WAV files");
    }

    // Build a concat list file so ffmpeg can stream the inputs in order.
    // We always put the list in the same directory as the output MP3 and
    // reference the WAVs by absolute path, so the file works no matter
    // what ffmpeg's working directory is.
    let list_path = out_mp3
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("_concat.txt");

    let mut list_body = String::new();
    for w in wavs {
        let abs = std::fs::canonicalize(w)
            .unwrap_or_else(|_| w.to_path_buf())
            .to_string_lossy()
            .replace('\\', "/");
        list_body.push_str(&format!("file '{}'\n", abs.replace('\'', "'\\''")));
    }
    std::fs::write(&list_path, list_body)
        .with_context(|| format!("failed to write concat list {}", list_path.display()))?;

    debug!("ffmpeg concat list at {}", list_path.display());

    let status = Command::new(ffmpeg)
        .arg("-y")
        .arg("-f")
        .arg("concat")
        .arg("-safe")
        .arg("0")
        .arg("-i")
        .arg(&list_path)
        .arg("-codec:a")
        .arg("libmp3lame")
        .arg("-q:a")
        .arg("2")
        .arg(out_mp3)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .with_context(|| format!("failed to spawn ffmpeg at {}", ffmpeg.display()))?;

    if !status.success() {
        anyhow::bail!("ffmpeg exited with status {status}");
    }

    let _ = std::fs::remove_file(&list_path);

    Ok(())
}

/// Concatenate the given WAV files into a single WAV at `out_wav`,
/// using the ffmpeg binary at `ffmpeg`. Used by the demo which outputs
/// WAV (not MP3).
pub fn merge_wavs_to_wav(wavs: &[PathBuf], out_wav: &Path, ffmpeg: &Path) -> Result<()> {
    if wavs.is_empty() {
        anyhow::bail!("cannot merge zero WAV files");
    }

    let list_path = out_wav
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("_concat_demo.txt");

    let mut list_body = String::new();
    for w in wavs {
        let abs = std::fs::canonicalize(w)
            .unwrap_or_else(|_| w.to_path_buf())
            .to_string_lossy()
            .replace('\\', "/");
        list_body.push_str(&format!("file '{}'\n", abs.replace('\'', "'\\''")));
    }
    std::fs::write(&list_path, list_body)
        .with_context(|| format!("failed to write concat list {}", list_path.display()))?;

    let status = Command::new(ffmpeg)
        .arg("-y")
        .arg("-f")
        .arg("concat")
        .arg("-safe")
        .arg("0")
        .arg("-i")
        .arg(&list_path)
        .arg("-c")
        .arg("copy")
        .arg(out_wav)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .with_context(|| format!("failed to spawn ffmpeg at {}", ffmpeg.display()))?;

    if !status.success() {
        anyhow::bail!("ffmpeg exited with status {status}");
    }

    let _ = std::fs::remove_file(&list_path);
    Ok(())
}

/// Locate the ffmpeg binary. Order of preference:
/// 1. `FFMPEG` env var
/// 2. `./ffmpeg/bin/ffmpeg` next to the project root
/// 3. ffmpeg on PATH
pub fn find_ffmpeg() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("FFMPEG") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Ok(pb);
        }
    }
    // Tauri places sidecars in `src-tauri/sidecars/<name>/`. After
    // bundling, the binary will live next to the .exe; in dev it lives
    // in the project source tree.
    let sidecar = std::env::current_exe()?
        .parent()
        .map(|p| p.join("ffmpeg.exe"))
        .filter(|p| p.exists());
    if let Some(p) = sidecar {
        return Ok(p);
    }
    let local = std::env::current_dir()?
        .join("ffmpeg")
        .join("bin")
        .join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" });
    if local.exists() {
        return Ok(local);
    }
    which("ffmpeg")
}

fn which(name: &str) -> Result<PathBuf> {
    let path = std::env::var_os("PATH").context("PATH not set")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.to_string()
        });
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    anyhow::bail!("ffmpeg not found in PATH; install or set FFMPEG env var")
}
