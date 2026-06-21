use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use tracing::debug;

/// Concatenate a list of WAV files into a single MP3 via an external
/// `ffmpeg` binary. The WAVs are expected to share sample rate and
/// channel count, which is the case in this prototype because every
/// chunk is rendered by the same Kokoro session.
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
        // Resolve to absolute so ffmpeg can find the files regardless
        // of its CWD. Forward slashes; ffmpeg on Windows accepts both.
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

    // Best-effort cleanup of the concat list
    let _ = std::fs::remove_file(&list_path);

    Ok(())
}
