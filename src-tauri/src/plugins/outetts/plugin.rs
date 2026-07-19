use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use tracing::{info, warn};

use crate::base_plugin::{BaseTTSPlugin, EngineHandle, SynthesizeRequest};
use crate::chunker;
use crate::merger;
use crate::recovery;

const OUTE_SERVER_PORT: u16 = 8765;
const DAC_SAMPLE_RATE: u32 = 24000;
const DAC_DECODE_CHUNK: usize = 2048;
const FADE_SAMPLES: usize = 360;

const C1_TOKEN_BASE: i64 = 151669;
const C1_TOKEN_MAX: i64 = 152693;
const C2_TOKEN_BASE: i64 = 152694;
const C2_TOKEN_MAX: i64 = 153718;

#[derive(Debug, Clone, Deserialize)]
struct SpeakerProfile {
    text: String,
    words: Vec<SpeakerWord>,
}

#[derive(Debug, Clone, Deserialize)]
struct SpeakerWord {
    word: String,
    duration: f64,
    c1: Vec<i64>,
    c2: Vec<i64>,
    features: SpeakerFeatures,
}

#[derive(Debug, Clone, Deserialize)]
struct SpeakerFeatures {
    energy: i64,
    spectral_centroid: i64,
    pitch: i64,
}

pub struct OuteTTSPlugin {
    pub variant_name: String,
    pub models_dir: PathBuf,
    server: Mutex<Option<ServerHandle>>,
}

struct ServerHandle {
    child: Child,
}

impl OuteTTSPlugin {
    pub fn new(models_dir: PathBuf, variant_name: &str) -> Self {
        Self {
            variant_name: variant_name.to_string(),
            models_dir,
            server: Mutex::new(None),
        }
    }

    fn backbone_gguf(&self) -> Result<PathBuf> {
        let p = self.models_dir.join(&self.variant_name).join("backbone-Q4_K_M.gguf");
        if p.exists() {
            return Ok(p);
        }
        let p2 = self.models_dir.join(&self.variant_name).join("backbone-Q8_0.gguf");
        if p2.exists() {
            return Ok(p2);
        }
        anyhow::bail!("backbone GGUF not found in {}", self.models_dir.join(&self.variant_name).display());
    }

    fn dac_onnx_path(&self) -> Result<PathBuf> {
        let p = self.models_dir.join("dac").join("decoder.onnx");
        if p.exists() {
            return Ok(p);
        }
        anyhow::bail!("DAC decoder ONNX not found at {}", p.display());
    }

    fn default_speaker_path(&self) -> Result<PathBuf> {
        let p = self.models_dir.join("speakers").join("en-female-1-neutral.json");
        if p.exists() {
            return Ok(p);
        }
        anyhow::bail!("default speaker JSON not found at {}", p.display());
    }

    fn speaker_path_by_name(&self, name: &str) -> Result<PathBuf> {
        let p = self.models_dir.join("speakers").join(format!("{name}.json"));
        if p.exists() {
            return Ok(p);
        }
        anyhow::bail!("speaker JSON '{}' not found at {}", name, p.display());
    }

    fn load_default_speaker(&self) -> Option<SpeakerProfile> {
        match self.default_speaker_path() {
            Ok(p) => match Self::load_speaker(&p) {
                Ok(s) => Some(s),
                Err(e) => {
                    warn!("[outetts] failed to load default speaker: {e:#}");
                    None
                }
            },
            Err(_) => {
                warn!("[outetts] no speaker profile found — generating without speaker reference (lower quality)");
                None
            }
        }
    }

    fn find_llama_server() -> Result<PathBuf> {
        if let Ok(app_data) = std::env::var("APPDATA") {
            let p = PathBuf::from(&app_data)
                .join("com.patata.audiobookgenerator")
                .join("resources")
                .join("llama.cpp")
                .join("llama-server.exe");
            if p.exists() {
                return Ok(p);
            }
        }
        let dev = PathBuf::from("resources").join("llama.cpp").join("llama-server.exe");
        if dev.exists() {
            return Ok(dev);
        }
        anyhow::bail!("llama-server not found")
    }

    fn binary_dir() -> Result<PathBuf> {
        Ok(Self::find_llama_server()?
            .parent()
            .ok_or_else(|| anyhow::anyhow!("cannot get parent"))?
            .to_path_buf())
    }

    fn ensure_server_running(&self) -> Result<()> {
        self.ensure_server_running_with_ctx(8192)
    }

    fn ensure_server_running_with_ctx(&self, ctx_size: u32) -> Result<()> {
        let mut guard = self.server.lock().unwrap();
        if guard.is_some() {
            return Ok(());
        }

        let binary = Self::find_llama_server()?;
        let model = self.backbone_gguf()?;

        info!("[outetts] starting llama-server: {} -m {} (ctx-size={})", binary.display(), model.display(), ctx_size);

        let mut path_env = Self::binary_dir()?.to_string_lossy().to_string();
        if let Ok(existing) = std::env::var("PATH") {
            path_env = format!("{};{}", path_env, existing);
        }

        let child = Command::new(&binary)
            .arg("-m").arg(&model)
            .arg("--port").arg(OUTE_SERVER_PORT.to_string())
            .arg("--host").arg("127.0.0.1")
            .arg("-ngl").arg("999")
            .arg("--ctx-size").arg(ctx_size.to_string())
            .arg("--no-webui")
            .env("PATH", &path_env)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to spawn llama-server")?;

        *guard = Some(ServerHandle { child });

        drop(guard);

        if !Self::wait_for_server(Duration::from_secs(60)) {
            self.stop_server();
            anyhow::bail!("llama-server did not become ready within 60s");
        }

        info!("[outetts] llama-server ready on port {}", OUTE_SERVER_PORT);
        Ok(())
    }

    fn stop_server(&self) {
        let mut guard = self.server.lock().unwrap();
        if let Some(mut handle) = guard.take() {
            let _ = handle.child.kill();
            let _ = handle.child.wait();
        }
    }

    fn wait_for_server(timeout: Duration) -> bool {
        let start = Instant::now();
        let url = format!("http://127.0.0.1:{}/health", OUTE_SERVER_PORT);
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(5)))
            .build()
            .into();
        while start.elapsed() < timeout {
            std::thread::sleep(Duration::from_millis(500));
            if let Ok(resp) = agent.get(&url).call() {
                if resp.status().is_success() {
                    if let Ok(text) = resp.into_body().read_to_string() {
                        if text.contains("\"ok\"") {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn send_completion_streaming(prompt: &str, extra: &std::collections::HashMap<String, String>) -> Result<(Vec<i64>, Vec<i64>)> {
        let url = format!("http://127.0.0.1:{}/completion", OUTE_SERVER_PORT);

        let temperature: f64 = extra.get("temperature")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.4);
        let top_k: i64 = extra.get("top_k")
            .and_then(|v| v.parse().ok())
            .unwrap_or(40);
        let top_p: f64 = extra.get("top_p")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.9);
        let min_p: f64 = extra.get("min_p")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.05);
        let rep_pen: f64 = extra.get("repetition_penalty")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.1);
        let max_tokens: i64 = extra.get("max_tokens")
            .and_then(|v| v.parse().ok())
            .unwrap_or(8192);

        let body = serde_json::json!({
            "prompt": prompt,
            "temperature": temperature,
            "top_k": top_k,
            "top_p": top_p,
            "min_p": min_p,
            "repeat_penalty": rep_pen,
            "repeat_last_n": 64,
            "n_predict": max_tokens,
            "stop": ["<|im_end|>"],
            "stream": true,
        });

        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(300)))
            .build()
            .into();

        let body_str = serde_json::to_string(&body)?;
        let resp = agent
            .post(&url)
            .header("Content-Type", "application/json")
            .send(&body_str)
            .context("failed to send completion request")?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.into_body().read_to_string().unwrap_or_default();
            anyhow::bail!("llama-server returned {}: {}", status, text);
        }

        let reader = resp.into_body().into_reader();
        let buf_reader = std::io::BufReader::new(reader);

        let mut c1 = Vec::new();
        let mut c2 = Vec::new();
        let mut token_count = 0u64;

        use std::io::BufRead;
        for line in buf_reader.lines() {
            let line = line?;
            let line = line.strip_prefix("data: ").unwrap_or(&line);
            if line.is_empty() || line == "[DONE]" {
                continue;
            }
            let json: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            if json.get("stop").and_then(|v| v.as_bool()).unwrap_or(false) {
                break;
            }

            if let Some(tokens) = json.get("tokens").and_then(|v| v.as_array()) {
                for tok in tokens {
                    let tid = tok.as_i64().unwrap_or(-1);
                    if tid >= C1_TOKEN_BASE && tid <= C1_TOKEN_MAX {
                        c1.push(tid - C1_TOKEN_BASE);
                    } else if tid >= C2_TOKEN_BASE && tid <= C2_TOKEN_MAX {
                        c2.push(tid - C2_TOKEN_BASE);
                    }
                    token_count += 1;
                }
            }
        }

        info!("[outetts] streamed {} tokens, extracted {} c1 + {} c2 codec tokens",
              token_count, c1.len(), c2.len());

        Ok((c1, c2))
    }

    fn dac_decode(c1: &[i64], c2: &[i64], onnx_path: &Path) -> Result<Vec<f32>> {
        use ort::session::Session;
        use ort::value::Tensor;

        let min_len = c1.len().min(c2.len());
        if min_len == 0 {
            anyhow::bail!("no codec tokens to decode");
        }

        let mut session = Session::builder()?
            .commit_from_file(onnx_path)
            .with_context(|| format!("failed to load DAC ONNX: {}", onnx_path.display()))?;

        info!("[outetts] DAC ONNX loaded, decoding {} frames in chunks of {}", min_len, DAC_DECODE_CHUNK);

        let mut all_audio: Vec<f32> = Vec::new();

        for start in (0..min_len).step_by(DAC_DECODE_CHUNK) {
            let end = (start + DAC_DECODE_CHUNK).min(min_len);
            let chunk_len = end - start;

            let mut flat = Vec::with_capacity(2 * chunk_len);
            flat.extend_from_slice(&c1[start..end]);
            flat.extend_from_slice(&c2[start..end]);

            let input_tensor = Tensor::from_array(([1, 2, chunk_len], flat))?;
            let outputs = session.run(ort::inputs![input_tensor])?;

            let first = outputs
                .iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("DAC produced no output"))?;
            let extracted = first.1.try_extract_tensor::<f32>()?;
            let mut chunk_audio: Vec<f32> = extracted.1.iter().copied().collect();
            apply_fade_in_place(&mut chunk_audio, start == 0, start + DAC_DECODE_CHUNK >= min_len);
            all_audio.extend(chunk_audio);
        }

        info!("[outetts] DAC decode complete: {} samples ({:.1}s at {}Hz)",
              all_audio.len(),
              all_audio.len() as f32 / DAC_SAMPLE_RATE as f32,
              DAC_SAMPLE_RATE);

        Ok(all_audio)
    }

    fn write_wav(samples: &[f32], path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: DAC_SAMPLE_RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec)?;
        for &s in samples {
            writer.write_sample(s)?;
        }
        writer.finalize()?;
        Ok(())
    }

    fn normalize_text(text: &str) -> String {
        let mut s = text.to_string();

        s = s.replace('\u{2026}', "...");
        s = s.replace('\u{201C}', "\"")
             .replace('\u{201D}', "\"")
             .replace('\u{201E}', "\"")
             .replace('\u{201F}', "\"")
             .replace('\u{00AB}', "\"")
             .replace('\u{00BB}', "\"");
        s = s.replace('\u{2018}', "'")
             .replace('\u{2019}', "'")
             .replace('\u{201A}', "'")
             .replace('\u{201B}', "'")
             .replace('\u{2039}', "'")
             .replace('\u{203A}', "'")
             .replace('\u{0060}', "'")
             .replace('\u{00B4}', "'");
        s = s.replace('\u{2013}', "-")
             .replace('\u{2014}', "-")
             .replace('\u{2010}', "-")
             .replace('\u{2011}', "-")
             .replace('\u{2212}', "-");

        while s.contains("--") {
            s = s.replace("--", "-");
        }

        s = s.replace('\u{00A0}', " ");
        s = s.replace('\u{00AD}', "");
        s = s.replace('\u{200B}', "");
        s = s.replace('\u{200C}', "");
        s = s.replace('\u{200D}', "");
        s = s.replace('\u{FEFF}', "");

        while s.contains("  ") {
            s = s.replace("  ", " ");
        }
        s = s.replace(" .", ".");
        s = s.replace(" ,", ",");
        s = s.replace(" ?", "?");
        s = s.replace(" !", "!");
        s = s.replace(" :", ":");
        s = s.replace(" ;", ";");
        s = s.replace('"', "");
        s.trim().to_string()
    }

    fn build_prompt(text: &str, speaker: Option<&SpeakerProfile>) -> String {
        let normalized = Self::normalize_text(text);

        let full_text = match speaker {
            Some(s) => Self::merge_speaker_text(&normalized, &s.text),
            None => normalized,
        };

        let mut prompt = format!(
            "<|im_start|>\n<|text_start|>{}<|text_end|>\n<|audio_start|>\n",
            full_text
        );

        if let Some(s) = speaker {
            for word in &s.words {
                prompt.push_str(&Self::format_speaker_word(word));
                prompt.push('\n');
            }
            prompt.push_str("<|word_start|>");
        }

        prompt
    }

    fn merge_speaker_text(input_text: &str, speaker_text: &str) -> String {
        let speaker_trimmed = speaker_text.trim();
        let last = speaker_trimmed.chars().last().unwrap_or('.');
        if last == '.' || last == '!' || last == '?' {
            format!("{} {}", speaker_trimmed, input_text.trim())
        } else {
            format!("{}. {}", speaker_trimmed, input_text.trim())
        }
    }

    fn format_speaker_word(word: &SpeakerWord) -> String {
        let mut s = format!(
            "<|word_start|>{}<|features|><|t_{:.2}|><|energy_{}|><|spectral_centroid_{}|><|pitch_{}|><|code|>",
            word.word,
            word.duration,
            word.features.energy,
            word.features.spectral_centroid,
            word.features.pitch
        );

        let min_len = word.c1.len().min(word.c2.len());
        for i in 0..min_len {
            s.push_str(&format!("<|c1_{}|><|c2_{}|>", word.c1[i], word.c2[i]));
        }

        s.push_str("<|word_end|>");
        s
    }

    fn load_speaker(path: &Path) -> Result<SpeakerProfile> {
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read speaker profile: {}", path.display()))?;
        let profile: SpeakerProfile = serde_json::from_str(&data)
            .with_context(|| format!("failed to parse speaker profile: {}", path.display()))?;
        Ok(profile)
    }

    pub fn synthesize_chunk_internal(
        &self,
        text: &str,
        output_path: &Path,
        extra: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        let ctx_size: u32 = extra.get("ctx_size")
            .and_then(|v| v.parse().ok())
            .unwrap_or(8192);
        self.ensure_server_running_with_ctx(ctx_size)?;

        let speaker = if let Some(custom_path) = extra.get("speaker_json") {
            let p = Path::new(custom_path);
            match Self::load_speaker(p) {
                Ok(s) => Some(s),
                Err(e) => {
                    warn!("[outetts] failed to load custom speaker {}: {e:#}", p.display());
                    None
                }
            }
        } else if let Some(name) = extra.get("speaker") {
            match self.speaker_path_by_name(name) {
                Ok(p) => match Self::load_speaker(&p) {
                    Ok(s) => Some(s),
                    Err(e) => {
                        warn!("[outetts] failed to load speaker '{}': {e:#}", name);
                        None
                    }
                },
                Err(e) => {
                    warn!("[outetts] speaker '{}' not found, falling back to default: {e:#}", name);
                    self.load_default_speaker()
                }
            }
        } else {
            self.load_default_speaker()
        };

        let prompt = Self::build_prompt(text, speaker.as_ref());
        info!(
            "[outetts] prompt length: {} chars, speaker: {}, words: {}",
            prompt.len(),
            if speaker.is_some() { "yes" } else { "no" },
            speaker.map(|s| s.words.len()).unwrap_or(0)
        );

        let (c1, c2) = Self::send_completion_streaming(&prompt, extra)?;

        if c1.is_empty() || c2.is_empty() {
            anyhow::bail!("no codec tokens extracted from LLM response");
        }

        let onnx_path = self.dac_onnx_path()?;
        let audio = Self::dac_decode(&c1, &c2, &onnx_path)?;

        Self::write_wav(&audio, output_path)?;
        info!("[outetts] WAV written: {}", output_path.display());

        Ok(())
    }
}

fn apply_fade_in_place(samples: &mut Vec<f32>, is_first: bool, is_last: bool) {
    let fade_len = FADE_SAMPLES.min(samples.len() / 2);
    if !is_first {
        for i in 0..fade_len {
            let gain = i as f32 / fade_len as f32;
            samples[i] *= gain;
        }
    }
    if !is_last {
        let start = samples.len() - fade_len;
        for i in 0..fade_len {
            let gain = (fade_len - i) as f32 / fade_len as f32;
            samples[start + i] *= gain;
        }
    }
}

pub fn synthesize_book(
    plugin: &OuteTTSPlugin,
    epub_path: &Path,
    output_dir: &Path,
    max_words: usize,
    max_chars: usize,
    ffmpeg: &Path,
    extra: &std::collections::HashMap<String, String>,
    mut progress: Option<Box<dyn FnMut(&str) + Send>>,
) -> Result<usize> {
    if let Some(cb) = progress.as_deref_mut() {
        cb("Reading EPUB...");
    }
    let chapters = crate::epub::extract_chapters(epub_path)?;
    let total_chapters = chapters.len();
    std::fs::create_dir_all(output_dir)?;
    let recovery_path = output_dir.join("failed_chunks.json");

    if let Some(cb) = progress.as_deref_mut() {
        cb(&format!("Extracted {total_chapters} chapters"));
    }

    plugin.ensure_server_running()?;

    let mut recovery_state = recovery::RecoveryState::load(output_dir).unwrap_or_default();
    recovery_state.set_meta(recovery::RecoveryMeta {
        engine_id: Some(plugin.variant_name.clone()),
        reference_audio: None,
        voice: None,
        language: None,
        extra: extra.clone(),
        generated_at: Some(recovery::now_stamp()),
    });
    // Persist immediately so the retry commands can resolve the engine even
    // if the very first chapter fails or is stopped.
    let _ = recovery_state.save(output_dir);
    let mut done_count = 0usize;
    let mut failed_count = 0usize;

    for (i, chapter) in chapters.iter().enumerate() {
        if crate::commands::is_stop_requested() {
            if let Some(cb) = progress.as_deref_mut() {
                cb("STOP requested — saving recovery state and exiting.");
            }
            break;
        }

        let chapter_dir = output_dir.join(crate::utils::sanitize_filename(&chapter.title));
        std::fs::create_dir_all(&chapter_dir)?;

        let chunks = chunker::chunk_text(&chapter.text, max_words, max_chars);
        if let Some(cb) = progress.as_deref_mut() {
            cb(&format!(
                "Chapter {}/{}: {} ({} chunks)",
                i + 1, total_chapters, chapter.title, chunks.len()
            ));
        }

        let mut wavs = Vec::with_capacity(chunks.len());
        for (j, chunk_text) in chunks.iter().enumerate() {
            if crate::commands::is_stop_requested() {
                if let Some(cb) = progress.as_deref_mut() {
                    cb("STOP requested — saving recovery state and exiting.");
                }
                break;
            }
            let wav_path = chapter_dir.join(format!("chunk_{:04}.wav", j + 1));
            if recovery_state.is_done(&chapter.title, j) && wav_path.exists() {
                wavs.push(wav_path);
                continue;
            }

            match plugin.synthesize_chunk_internal(chunk_text, &wav_path, extra) {
                Ok(()) => {
                    recovery_state.mark_done(&chapter.title, j);
                    wavs.push(wav_path);
                    done_count += 1;
                }
                Err(e) => {
                    failed_count += 1;
                    warn!("chunk {}/{} failed: {e:#}", j + 1, chunks.len());
                    if let Some(cb) = progress.as_deref_mut() {
                        cb(&format!("WARN: chunk {}/{} failed: {}", j + 1, chunks.len(),
                            e.to_string().lines().next().unwrap_or(&e.to_string())));
                    }
                    recovery_state.mark_failed(&chapter.title, j, chunk_text, &format!("{e:#}"));
                }
            }
        }

        if !wavs.is_empty() {
            let mp3_path = output_dir.join(format!(
                "{}.mp3", crate::utils::sanitize_filename(&chapter.title)
            ));
            if let Err(e) = merger::merge_wavs_to_mp3(&wavs, &mp3_path, ffmpeg) {
                warn!("merge failed for {}: {e:#}", chapter.title);
                if let Some(cb) = progress.as_deref_mut() {
                    cb(&format!("ERROR: merge failed for {}: {}", chapter.title, e));
                }
            }
        }

        let _ = std::fs::write(
            &recovery_path,
            serde_json::to_string_pretty(&recovery_state).unwrap_or_else(|_| "{}".to_string()),
        );
        if let Some(cb) = progress.as_deref_mut() {
            cb(&format!("Chapter {}/{} done", i + 1, total_chapters));
        }
    }

    plugin.stop_server();

    if let Some(cb) = progress.as_deref_mut() {
        cb(&format!(
            "Done: {done_count} chunks synthesized, {failed_count} failed across {total_chapters} chapters"
        ));
    }
    Ok(done_count)
}

#[async_trait]
impl BaseTTSPlugin for OuteTTSPlugin {
    fn name(&self) -> &str {
        &self.variant_name
    }

    fn plugin_type(&self) -> &str {
        "llama_server"
    }

    fn is_installed(&self) -> bool {
        self.backbone_gguf().is_ok() && self.dac_onnx_path().is_ok()
    }

    async fn load_model(&self, model_id: &str) -> Result<EngineHandle> {
        let backbone = self.backbone_gguf()?;
        let dac = self.dac_onnx_path()?;
        info!("[outetts] loading: backbone={}, dac={}", backbone.display(), dac.display());
        Self::find_llama_server().context("llama-server not found")?;
        self.ensure_server_running()?;
        Ok(EngineHandle {
            engine_id: self.variant_name.clone(),
            model_id: model_id.to_string(),
        })
    }

    async fn synthesize(&self, _handle: &EngineHandle, request: &SynthesizeRequest) -> Result<()> {
        let output_path = Path::new(&request.output_path);
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        self.synthesize_chunk_internal(&request.text, output_path, &request.extra)
    }

    async fn unload(&self, _handle: &EngineHandle) -> Result<()> {
        self.stop_server();
        info!("[outetts] unloaded");
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
