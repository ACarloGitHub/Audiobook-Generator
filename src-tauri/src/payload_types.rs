use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KokoroPayload {
    pub text: String,
    pub output_path: String,
    pub voice_id: Option<String>,
    pub speed: f64,
    pub language_code: String,
    pub timeout_seconds: u64,
}

impl Default for KokoroPayload {
    fn default() -> Self {
        Self {
            text: String::new(),
            output_path: String::new(),
            voice_id: None,
            speed: 1.0,
            language_code: "en".into(),
            timeout_seconds: 1800,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XTTSv2Payload {
    pub text: String,
    pub output_path: String,
    pub language: Option<String>,
    pub speaker_wav: Option<String>,
    pub temperature: f64,
    pub speed: f64,
    pub repetition_penalty: f64,
    pub use_tts_splitting: bool,
    pub sentence_separator: String,
    pub max_retries: u32,
    pub timeout_seconds: u64,
}

impl Default for XTTSv2Payload {
    fn default() -> Self {
        Self {
            text: String::new(),
            output_path: String::new(),
            language: None,
            speaker_wav: None,
            temperature: 0.75,
            speed: 1.0,
            repetition_penalty: 2.0,
            use_tts_splitting: true,
            sentence_separator: ".".into(),
            max_retries: 3,
            timeout_seconds: 1800,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibeVoicePayload {
    pub text: String,
    pub output_path: String,
    pub speaker_wav: String,
    pub model_name: String,
    pub temperature: f64,
    pub top_p: f64,
    pub cfg_scale: f64,
    pub diffusion_steps: u32,
    pub voice_speed_factor: f64,
    pub use_sampling: bool,
    pub seed: Option<u64>,
    pub timeout_seconds: u64,
}

impl VibeVoicePayload {
    pub fn new(text: &str, output_path: &str, speaker_wav: &str, model_name: &str) -> Self {
        Self {
            text: text.into(),
            output_path: output_path.into(),
            speaker_wav: speaker_wav.into(),
            model_name: model_name.into(),
            temperature: 0.9,
            top_p: 0.9,
            cfg_scale: 1.3,
            diffusion_steps: 15,
            voice_speed_factor: 1.0,
            use_sampling: true,
            seed: None,
            timeout_seconds: 1800,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Qwen3TTSParams {
    pub speed: f64,
    pub pitch: i32,
    pub volume: i32,
    pub temperature: f64,
    pub top_p: f64,
    pub top_k: i32,
    pub repetition_penalty: f64,
    pub seed: Option<u64>,
    pub speaker: String,
    pub voice: String,
    pub language: String,
    pub instruct: String,
    pub ref_audio: Option<String>,
    pub ref_text: String,
    pub x_vector_only_mode: bool,
}

impl Default for Qwen3TTSParams {
    fn default() -> Self {
        Self {
            speed: 1.0,
            pitch: 0,
            volume: 0,
            temperature: 1.0,
            top_p: 0.95,
            top_k: 50,
            repetition_penalty: 1.0,
            seed: None,
            speaker: String::new(),
            voice: String::new(),
            language: String::new(),
            instruct: String::new(),
            ref_audio: None,
            ref_text: String::new(),
            x_vector_only_mode: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Qwen3TTSPayload {
    pub text: String,
    pub output_path: String,
    pub mode: Option<String>,
    pub params: Qwen3TTSParams,
    pub model_size: String,
    pub model_type: String,
    pub timeout_seconds: u64,
}

impl Qwen3TTSPayload {
    pub fn new(text: &str, output_path: &str, mode: &str, model_size: &str, model_type: &str) -> Self {
        Self {
            text: text.into(),
            output_path: output_path.into(),
            mode: Some(mode.into()),
            params: Qwen3TTSParams::default(),
            model_size: model_size.into(),
            model_type: model_type.into(),
            timeout_seconds: 1800,
        }
    }
}