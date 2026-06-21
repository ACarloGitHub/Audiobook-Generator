use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use hound::{SampleFormat, WavSpec};
use kokoro_en::{KokoroTts, Voice};
use tracing::info;

/// Thin wrapper around the `kokoro-en` library so the rest of the CLI
/// sees a synchronous `synthesize(text, output_wav)` method.
///
/// On first run, the wrapper points kokoro-en at the on-disk model and
/// voices directories; the user is expected to have downloaded them
/// from `https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX`
/// (the README has the curl/git-lfs commands).
pub struct KokoroEngine {
    inner: KokoroTts,
    sample_rate: u32,
    voice: String,
}

impl KokoroEngine {
    /// Load the Kokoro ONNX model from a directory of voices.
    pub fn load(model_dir: &Path, voices_dir: &Path, voice: &str) -> Result<Self> {
        // kokoro-en takes the path to a single model.onnx file plus the
        // voices directory. We pick the smallest quantised variant that
        // exists, falling back to the full-precision model.
        let model_path = pick_model_file(model_dir)
            .with_context(|| format!("no model.onnx found in {}", model_dir.display()))?;
        info!("loading Kokoro ONNX from {}", model_path.display());
        info!("loading voices from {}", voices_dir.display());

        let inner = futures::executor::block_on(KokoroTts::new(
            model_path.to_str().context("model path is not valid UTF-8")?,
            voices_dir.to_str().context("voices dir is not valid UTF-8")?,
        ))
        .context("failed to construct KokoroTts")?;

        Ok(Self {
            inner,
            sample_rate: 24_000,
            voice: voice.to_string(),
        })
    }

    pub fn voice(&self) -> &str {
        &self.voice
    }

    pub fn synthesize(&self, text: &str, output_wav: &Path) -> Result<()> {
        if let Some(parent) = output_wav.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create chunk output dir {}", parent.display())
            })?;
        }

        let voice = Voice::new(&self.voice);
        let (samples, took) = futures::executor::block_on(self.inner.synth(text, voice))
            .with_context(|| format!("Kokoro synthesis failed for text: {:.40}...", text))?;

        write_wav(output_wav, self.sample_rate, &samples)?;
        tracing::debug!(
            "wrote {} samples in {took:?} to {}",
            samples.len(),
            output_wav.display()
        );
        Ok(())
    }
}

fn pick_model_file(model_dir: &Path) -> Option<PathBuf> {
    for name in ["model_quantized.onnx", "model_q8f16.onnx", "model.onnx"] {
        let p = model_dir.join(name);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn write_wav(path: &Path, sample_rate: u32, samples: &[f32]) -> Result<()> {
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .with_context(|| format!("failed to create WAV at {}", path.display()))?;
    for &s in samples {
        let clipped = s.clamp(-1.0, 1.0);
        let int = (clipped * i16::MAX as f32) as i16;
        writer.write_sample(int)?;
    }
    writer.finalize()?;
    Ok(())
}
