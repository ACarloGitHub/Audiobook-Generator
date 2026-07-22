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

    let mut cmd = Command::new(ffmpeg);
    crate::utils::hide_console_window(&mut cmd);
    let status = cmd
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

    let mut cmd = Command::new(ffmpeg);
    crate::utils::hide_console_window(&mut cmd);
    let status = cmd
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

/// Sort key for chunk WAV files: `chunk_0007.wav` sorts as (7, 0) and
/// `chunk_0007_part02.wav` as (7, 2), so a base chunk always comes before
/// its split parts and parts come in numeric order.
fn chunk_sort_key(path: &Path) -> Option<(u64, u64)> {
    let stem = path.file_stem()?.to_string_lossy();
    let rest = stem.strip_prefix("chunk_")?;
    let (idx_part, part_part) = match rest.split_once("_part") {
        Some((a, b)) => (a, b),
        None => (rest, "0"),
    };
    let idx: u64 = idx_part.parse().ok()?;
    let part: u64 = part_part.parse().ok()?;
    Some((idx, part))
}

/// Collect every chunk WAV in `chapter_dir`, including split-part variants
/// (`chunk_0007_part01.wav`), ordered as: `chunk_0007.wav`,
/// `chunk_0007_part01.wav`, `chunk_0007_part02.wav`, `chunk_0008.wav`, ...
///
/// When a chunk was split into parts, only the parts are kept (the base
/// chunk WAV, if still on disk from a previous attempt, is superseded by
/// its parts and would duplicate the audio).
pub fn collect_chapter_wavs(chapter_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(chapter_dir) else {
        return Vec::new();
    };
    let mut keyed: Vec<((u64, u64), PathBuf)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .map(|ext| ext.eq_ignore_ascii_case("wav"))
                .unwrap_or(false)
        })
        .filter_map(|p| chunk_sort_key(&p).map(|k| (k, p)))
        .collect();
    keyed.sort_by_key(|(k, _)| *k);

    // Group by chunk index: if any `_partNN` variant exists for an index,
    // drop the base chunk for that index.
    let mut split_indices: std::collections::HashSet<u64> = std::collections::HashSet::new();
    for ((idx, part), _) in &keyed {
        if *part > 0 {
            split_indices.insert(*idx);
        }
    }
    keyed
        .into_iter()
        .filter(|((idx, part), _)| *part > 0 || !split_indices.contains(idx))
        .map(|(_, p)| p)
        .collect()
}

/// Locate the ffmpeg binary. Order of preference:
/// 1. `FFMPEG` env var
/// 2. ffmpeg bundled in the installer (`resources/ffmpeg/ffmpeg[.exe]`,
///    with legacy per-user/dev fallbacks handled by the sidecars module)
/// 3. `./ffmpeg/bin/ffmpeg` next to the project root
/// 4. ffmpeg on PATH
pub fn find_ffmpeg() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("FFMPEG") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Ok(pb);
        }
    }
    let exe_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    if let Some(p) = crate::sidecars::sidecar_binary("ffmpeg", exe_name) {
        return Ok(p);
    }
    let local = std::env::current_dir()?
        .join("ffmpeg")
        .join("bin")
        .join(exe_name);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn write_wavs(dir: &Path, names: &[&str]) {
        for n in names {
            std::fs::write(dir.join(n), b"RIFF").unwrap();
        }
    }

    #[test]
    fn collect_wavs_orders_base_before_parts_and_parts_numerically() {
        let dir = std::env::temp_dir().join(format!("merger_test_{}_a", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        write_wavs(
            &dir,
            &[
                "chunk_0001.wav",
                "chunk_0002_part02.wav",
                "chunk_0002_part01.wav",
                "chunk_0002_part10.wav",
                "chunk_0003.wav",
            ],
        );
        let wavs = collect_chapter_wavs(&dir);
        let names: Vec<String> = wavs
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            vec![
                "chunk_0001.wav",
                "chunk_0002_part01.wav",
                "chunk_0002_part02.wav",
                "chunk_0002_part10.wav",
                "chunk_0003.wav",
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_wavs_base_chunk_superseded_by_parts() {
        let dir = std::env::temp_dir().join(format!("merger_test_{}_b", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        write_wavs(
            &dir,
            &[
                "chunk_0001.wav",
                "chunk_0002.wav",
                "chunk_0002_part01.wav",
                "chunk_0002_part02.wav",
            ],
        );
        let wavs = collect_chapter_wavs(&dir);
        let names: Vec<String> = wavs
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        // chunk_0002.wav is dropped: its parts replace it.
        assert_eq!(
            names,
            vec![
                "chunk_0001.wav",
                "chunk_0002_part01.wav",
                "chunk_0002_part02.wav",
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_wavs_ignores_non_chunk_files() {
        let dir = std::env::temp_dir().join(format!("merger_test_{}_c", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        write_wavs(&dir, &["chunk_0001.wav", "notes.wav", "chunk_abc.wav"]);
        std::fs::write(dir.join("chunk_0002.txt"), b"x").unwrap();
        let wavs = collect_chapter_wavs(&dir);
        assert_eq!(wavs.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
