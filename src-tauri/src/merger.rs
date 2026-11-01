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

/// Concatenate PCM WAV files into a single WAV without any external tool.
///
/// Fallback used when ffmpeg is not available: every chunk produced by an
/// engine has the same format (one engine per book), so concatenating the
/// samples is enough. The result is lossless but much larger than MP3.
pub fn merge_wavs_pure(wavs: &[PathBuf], out_wav: &Path) -> Result<()> {
    if wavs.is_empty() {
        anyhow::bail!("cannot merge zero WAV files");
    }

    let first = hound::WavReader::open(&wavs[0])
        .with_context(|| format!("open first chunk {}", wavs[0].display()))?;
    let spec = first.spec();
    drop(first);

    let mut writer = hound::WavWriter::create(out_wav, spec)
        .with_context(|| format!("create {}", out_wav.display()))?;
    let mut samples_written: u64 = 0;

    for path in wavs {
        let mut reader =
            hound::WavReader::open(path).with_context(|| format!("open chunk {}", path.display()))?;
        let chunk = reader.spec();
        if chunk.channels != spec.channels
            || chunk.sample_rate != spec.sample_rate
            || chunk.bits_per_sample != spec.bits_per_sample
            || chunk.sample_format != spec.sample_format
        {
            anyhow::bail!(
                "chunk {} has a different audio format ({} Hz, {} ch, {}-bit) from the first one \
                 ({} Hz, {} ch, {}-bit); cannot merge without ffmpeg",
                path.display(),
                chunk.sample_rate,
                chunk.channels,
                chunk.bits_per_sample,
                spec.sample_rate,
                spec.channels,
                spec.bits_per_sample
            );
        }
        match spec.sample_format {
            hound::SampleFormat::Int => {
                for sample in reader.samples::<i32>() {
                    writer.write_sample(sample?)?;
                    samples_written += 1;
                }
            }
            hound::SampleFormat::Float => {
                for sample in reader.samples::<f32>() {
                    writer.write_sample(sample?)?;
                    samples_written += 1;
                }
            }
        }
    }

    writer.finalize().context("finalize merged WAV")?;
    debug!(
        "merged {} chunk(s), {} samples at {} Hz into {}",
        wavs.len(),
        samples_written,
        spec.sample_rate,
        out_wav.display()
    );
    Ok(())
}

/// Merge a chapter's chunks into the best container available.
///
/// With ffmpeg the chapter becomes an MP3 (small, what users expect); without
/// it the chunks are concatenated into a WAV in pure Rust, so a book is never
/// lost just because the encoder is missing.
pub fn merge_chapter(
    wavs: &[PathBuf],
    out_dir: &Path,
    base_name: &str,
    ffmpeg: Option<&Path>,
) -> Result<PathBuf> {
    match ffmpeg {
        Some(ffmpeg) => {
            let mp3 = out_dir.join(format!("{base_name}.mp3"));
            merge_wavs_to_mp3(wavs, &mp3, ffmpeg)?;
            Ok(mp3)
        }
        None => {
            let wav = out_dir.join(format!("{base_name}.wav"));
            merge_wavs_pure(wavs, &wav)?;
            Ok(wav)
        }
    }
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

    fn write_pcm_wav(path: &Path, samples: &[i16]) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for s in samples {
            w.write_sample(*s).unwrap();
        }
        w.finalize().unwrap();
    }

    #[test]
    fn pure_merge_concatenates_samples() {
        let dir = std::env::temp_dir().join(format!("merger_test_{}_pure", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        write_pcm_wav(&dir.join("a.wav"), &[1, 2, 3]);
        write_pcm_wav(&dir.join("b.wav"), &[4, 5]);
        let out = dir.join("merged.wav");

        merge_wavs_pure(&[dir.join("a.wav"), dir.join("b.wav")], &out).unwrap();

        let mut r = hound::WavReader::open(&out).unwrap();
        let samples: Vec<i16> = r.samples::<i16>().map(|s| s.unwrap()).collect();
        assert_eq!(samples, vec![1, 2, 3, 4, 5]);
        assert_eq!(r.spec().sample_rate, 16_000);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pure_merge_rejects_mismatched_formats() {
        let dir = std::env::temp_dir().join(format!("merger_test_{}_mismatch", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        write_pcm_wav(&dir.join("a.wav"), &[1, 2]);
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 44_100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(dir.join("b.wav"), spec).unwrap();
        w.write_sample(3i16).unwrap();
        w.finalize().unwrap();

        let err = merge_wavs_pure(&[dir.join("a.wav"), dir.join("b.wav")], &dir.join("out.wav"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("different audio format"), "error: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_chapter_falls_back_to_wav_without_ffmpeg() {
        let dir = std::env::temp_dir().join(format!("merger_test_{}_chapter", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        write_pcm_wav(&dir.join("c1.wav"), &[7, 8]);

        let out = merge_chapter(&[dir.join("c1.wav")], &dir, "chapter", None).unwrap();
        assert_eq!(out.extension().unwrap(), "wav");
        assert!(out.exists());
        let _ = std::fs::remove_dir_all(&dir);
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
