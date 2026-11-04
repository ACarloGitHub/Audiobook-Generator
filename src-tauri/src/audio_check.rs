//! Plausibility check between a chunk's text length and the audio it produced.
//!
//! Small autoregressive TTS engines fail silently in two ways: they stop early
//! (the tail of the sentence is never spoken) or they loop (the same words are
//! repeated). Both show up as an audio length that does not match the number of
//! characters sent, so a cheap duration check catches what otherwise reaches the
//! finished audiobook unnoticed.
//!
//! The check is deliberately conservative: it flags only a chunk whose speaking
//! rate is far outside the range measured for the same job, plus a couple of
//! absolute bounds for the first chunks of a job, where there is no history yet.
//! A flagged chunk is reported as failed, so it ends up in the Error Recovery
//! panel where it can be retried, instead of being merged into the book.

use std::path::Path;

/// Fastest plausible narrative speech, in characters per second. Above this the
/// audio is too short for the text: something was dropped.
pub const MAX_CHARS_PER_SECOND: f64 = 35.0;
/// Slowest plausible narrative speech. Below this the audio is far longer than
/// the text: the model looped or padded.
pub const MIN_CHARS_PER_SECOND: f64 = 4.0;
/// How far a chunk may drift from the job's median rate before being flagged.
pub const RELATIVE_TOLERANCE: f64 = 0.30;
/// Number of chunks needed before the median test becomes meaningful.
pub const MIN_SAMPLES_FOR_MEDIAN: usize = 4;
/// Very short chunks (a closing line, a chapter tail) have a naturally noisy
/// rate: comparing them to the job median produces false positives, so only
/// the absolute bounds apply below this length.
pub const MIN_CHARS_FOR_MEDIAN_COMPARISON: usize = 80;

/// Tracks the speaking rate of the chunks generated for one job and reports the
/// ones that do not look like the text they came from.
#[derive(Debug, Default)]
pub struct RateGuard {
    rates: Vec<f64>,
}

impl RateGuard {
    pub fn new() -> Self {
        Self { rates: Vec::new() }
    }

    /// Record one synthesized chunk. Returns `Some(reason)` when the audio and
    /// the text length do not plausibly match.
    pub fn observe(&mut self, chars: usize, seconds: f64) -> Option<String> {
        if chars == 0 || !seconds.is_finite() || seconds <= 0.0 {
            return None;
        }
        let rate = chars as f64 / seconds;
        let median = self.median();
        let reason = if rate > MAX_CHARS_PER_SECOND {
            Some(format!(
                "audio far shorter than the text: {chars} chars in {seconds:.1}s ({rate:.0} chars/s)"
            ))
        } else if rate < MIN_CHARS_PER_SECOND {
            Some(format!(
                "audio far longer than the text: {chars} chars in {seconds:.1}s ({rate:.1} chars/s)"
            ))
        } else if chars >= MIN_CHARS_FOR_MEDIAN_COMPARISON {
            if let Some(median) = median {
                if rate > median * (1.0 + RELATIVE_TOLERANCE) {
                    Some(format!(
                        "audio shorter than the rest of the job: {chars} chars in {seconds:.1}s \
                         ({rate:.1} chars/s vs {median:.1} typical)"
                    ))
                } else if rate < median * (1.0 - RELATIVE_TOLERANCE) {
                    Some(format!(
                        "audio longer than the rest of the job: {chars} chars in {seconds:.1}s \
                         ({rate:.1} chars/s vs {median:.1} typical)"
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        self.rates.push(rate);
        reason
    }

    /// Median of the rates observed so far, once there are enough of them.
    fn median(&self) -> Option<f64> {
        if self.rates.len() < MIN_SAMPLES_FOR_MEDIAN {
            return None;
        }
        let mut sorted = self.rates.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = sorted.len() / 2;
        Some(if sorted.len() % 2 == 0 {
            (sorted[mid - 1] + sorted[mid]) / 2.0
        } else {
            sorted[mid]
        })
    }
}

/// Duration of a PCM WAV file, read from its RIFF header.
///
/// Returns `None` for anything that is not a readable PCM WAV; the caller treats
/// that as "no opinion" and keeps the chunk.
pub fn wav_duration_seconds(path: &Path) -> Option<f64> {
    let data = std::fs::read(path).ok()?;
    if data.len() < 44 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return None;
    }
    let mut pos = 12usize;
    let mut byte_rate = 0u32;
    let mut data_len = 0u32;
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let size = u32::from_le_bytes([
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ]) as usize;
        if id == b"fmt " && pos + 8 + 16 <= data.len() {
            let b = pos + 8;
            byte_rate = u32::from_le_bytes([data[b + 8], data[b + 9], data[b + 10], data[b + 11]]);
        } else if id == b"data" {
            data_len = size as u32;
            break;
        }
        pos += 8 + size + (size % 2);
    }
    if byte_rate == 0 || data_len == 0 {
        return None;
    }
    Some(data_len as f64 / byte_rate as f64)
}

/// Summary of a reference recording, used to warn about samples that the
/// models will imitate (pace, pauses) or that eat the engine context.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReferenceInfo {
    pub duration_seconds: f64,
    pub sample_rate: u32,
    pub channels: u16,
    /// Absolute peak, normalised to 0..1.
    pub peak: f32,
    /// Longest run below 1% of the peak (≈ -40 dB), in seconds.
    pub longest_silence_seconds: f64,
    /// Share of the recording that sits below the silence threshold.
    pub silence_ratio: f64,
}

/// Read a reference WAV and report duration, peak and the longest pause.
/// Returns `None` when the file is not a readable PCM WAV.
pub fn reference_info(path: &Path) -> Option<ReferenceInfo> {
    let mut reader = hound::WavReader::open(path).ok()?;
    let spec = reader.spec();
    if spec.sample_rate == 0 {
        return None;
    }
    let mut samples: Vec<f32> = Vec::new();
    match spec.sample_format {
        hound::SampleFormat::Int => {
            let max = (1i64 << spec.bits_per_sample.saturating_sub(1)) as f32;
            for sample in reader.samples::<i32>() {
                let v = sample.ok()? as f32;
                samples.push(if max > 0.0 { v / max } else { 0.0 });
            }
        }
        hound::SampleFormat::Float => {
            for sample in reader.samples::<f32>() {
                samples.push(sample.ok()?);
            }
        }
    }
    if samples.is_empty() {
        return None;
    }
    let channels = spec.channels.max(1) as f64;
    let peak = samples.iter().fold(0f32, |acc, s| acc.max(s.abs()));
    let threshold = (peak * 0.01).max(0.002);
    let mut longest = 0usize;
    let mut run = 0usize;
    let mut silent = 0usize;
    for sample in &samples {
        if sample.abs() < threshold {
            run += 1;
            silent += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let frames = samples.len() as f64 / channels;
    Some(ReferenceInfo {
        duration_seconds: frames / spec.sample_rate as f64,
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        peak,
        longest_silence_seconds: longest as f64 / channels / spec.sample_rate as f64,
        silence_ratio: silent as f64 / samples.len() as f64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_plausible_chunk() {
        let mut guard = RateGuard::new();
        assert!(guard.observe(448, 30.0).is_none());
    }

    #[test]
    fn flags_audio_that_is_far_too_short() {
        let mut guard = RateGuard::new();
        let reason = guard.observe(448, 10.0).expect("expected a flag");
        assert!(reason.contains("shorter"), "reason: {reason}");
    }

    #[test]
    fn flags_audio_that_is_far_too_long() {
        let mut guard = RateGuard::new();
        let reason = guard.observe(60, 40.0).expect("expected a flag");
        assert!(reason.contains("longer"), "reason: {reason}");
    }

    #[test]
    fn short_chunks_skip_the_median_comparison() {
        let mut guard = RateGuard::new();
        for _ in 0..5 {
            assert!(guard.observe(300, 20.0).is_none());
        }
        // 30 chars in 2.6 s is a normal closing line: the noisy short rate
        // must not be compared with the 15 chars/s median.
        assert!(guard.observe(30, 2.6).is_none());
        // The absolute bound still applies: 30 chars in 0.5 s is impossible.
        assert!(guard.observe(30, 0.5).is_some());
    }

    #[test]
    fn spots_a_chunk_outside_the_job_median() {
        let mut guard = RateGuard::new();
        // Five consistent chunks at ~15 chars/s.
        for _ in 0..5 {
            assert!(guard.observe(300, 20.0).is_none());
        }
        // A chunk with 30% of the text missing: 300 chars in 14 s = 21 chars/s.
        let reason = guard
            .observe(300, 14.0)
            .expect("expected the outlier to be flagged");
        assert!(reason.contains("rest of the job"), "reason: {reason}");
    }

    #[test]
    fn does_not_flag_before_enough_samples() {
        let mut guard = RateGuard::new();
        assert!(guard.observe(300, 20.0).is_none());
        assert!(guard.observe(300, 14.0).is_none());
    }

    #[test]
    fn ignores_empty_or_broken_input() {
        let mut guard = RateGuard::new();
        assert!(guard.observe(0, 10.0).is_none());
        assert!(guard.observe(100, 0.0).is_none());
        assert!(guard.observe(100, f64::NAN).is_none());
    }

    #[test]
    fn reads_duration_from_a_wav_header() {
        // 8 kHz, 16-bit mono, one second of silence.
        let path = std::env::temp_dir().join("abg_audio_check_1s.wav");
        let samples: u32 = 8000;
        let data_len = samples * 2;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
        bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
        bytes.extend_from_slice(&8000u32.to_le_bytes());
        bytes.extend_from_slice(&16000u32.to_le_bytes()); // byte rate
        bytes.extend_from_slice(&2u16.to_le_bytes()); // block align
        bytes.extend_from_slice(&16u16.to_le_bytes()); // bits
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        bytes.extend(std::iter::repeat(0u8).take(data_len as usize));
        std::fs::write(&path, &bytes).unwrap();
        let seconds = wav_duration_seconds(&path).expect("duration");
        let _ = std::fs::remove_file(&path);
        assert!((seconds - 1.0).abs() < 1e-6, "got {seconds}");
    }

    #[test]
    fn rejects_a_non_wav_file() {
        let path = std::env::temp_dir().join("abg_audio_check_not_wav.bin");
        std::fs::write(&path, b"not a wav file at all").unwrap();
        let seconds = wav_duration_seconds(&path);
        let _ = std::fs::remove_file(&path);
        assert!(seconds.is_none());
    }

    #[test]
    fn reference_info_measures_duration_and_longest_pause() {
        let path = std::env::temp_dir().join("abg_reference_info.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        // 0.5 s tone, 1.0 s silence, 0.5 s tone = 2.0 s total.
        for i in 0..4000 {
            let v = if i % 2 == 0 { 8000i16 } else { -8000 };
            writer.write_sample(v).unwrap();
        }
        for _ in 0..8000 {
            writer.write_sample(0i16).unwrap();
        }
        for i in 0..4000 {
            let v = if i % 2 == 0 { 8000i16 } else { -8000 };
            writer.write_sample(v).unwrap();
        }
        writer.finalize().unwrap();

        let info = reference_info(&path).expect("reference info");
        let _ = std::fs::remove_file(&path);
        assert!((info.duration_seconds - 2.0).abs() < 0.01, "{:?}", info);
        assert!(
            (info.longest_silence_seconds - 1.0).abs() < 0.01,
            "longest pause: {}",
            info.longest_silence_seconds
        );
        assert!(info.peak > 0.2, "peak: {}", info.peak);
    }
}
