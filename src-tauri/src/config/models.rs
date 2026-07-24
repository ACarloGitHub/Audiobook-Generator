use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsModelConfig {
    pub char_limit_recommended: Option<usize>,
    pub char_limit_max: Option<usize>,
    pub char_limits_by_lang: Option<HashMap<String, CharLimitRange>>,
    pub separator: String,
    pub replace_guillemets: bool,
    pub chunking_strategy: String,
    pub force_char_limit_chunking: Option<bool>,
    pub mode: Option<String>,
    pub supported_modes: Option<Vec<String>>,
    pub note: Option<String>,
    pub time_warning: Option<String>,
    pub voice_cloning: Option<bool>,
    pub needs_reference_transcript: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharLimitRange {
    pub min: usize,
    pub max: usize,
}

pub fn tts_model_config() -> HashMap<&'static str, TtsModelConfig> {
    let mut m = HashMap::new();

    // Qwen3-TTS — all 5 variants share the same chunking config.
    // Source: backup config/models.py lines 93-128 + verified online
    // (https://huggingface.co/Qwen/Qwen3-TTS-12Hz-1.7B-Base)
    let qwen_base = TtsModelConfig {
        char_limit_recommended: Some(800),
        char_limit_max: Some(1000),
        char_limits_by_lang: None,
        separator: ".".into(),
        replace_guillemets: false,
        chunking_strategy: "Character Limit".into(),
        force_char_limit_chunking: Some(true),
        mode: Some("Voice Clone".into()),
        supported_modes: Some(vec!["Voice Clone".into()]),
        note: Some("Qwen3-TTS Base: zero-shot voice clone from 3-20s reference audio.".into()),
        time_warning: None,
        voice_cloning: Some(true),
        needs_reference_transcript: Some(false),
    };
    m.insert("Qwen3-TTS-12Hz-0.6B-Base", qwen_base);

    // 1.7B models: reduced char limits (700/850) to avoid
    // "decode would overflow cache (4097 > 4096)" on borderline chunks.
    let qwen_base_17b = TtsModelConfig {
        char_limit_recommended: Some(700),
        char_limit_max: Some(850),
        ..qwen_base.clone()
    };
    m.insert("Qwen3-TTS-12Hz-1.7B-Base", qwen_base_17b);

    m.insert(
        "Qwen3-TTS-12Hz-0.6B-CustomVoice",
        TtsModelConfig {
            mode: Some("Custom Voice".into()),
            supported_modes: Some(vec!["Custom Voice".into()]),
            note: Some("Qwen3-TTS CustomVoice: 9 preset voices with instruct control.".into()),
            char_limit_recommended: Some(800),
            char_limit_max: Some(1000),
            char_limits_by_lang: None,
            separator: ".".into(),
            replace_guillemets: false,
            chunking_strategy: "Character Limit".into(),
            force_char_limit_chunking: Some(true),
            time_warning: None,
            voice_cloning: Some(false),
            needs_reference_transcript: Some(false),
        },
    );
    m.insert(
        "Qwen3-TTS-12Hz-1.7B-CustomVoice",
        TtsModelConfig {
            mode: Some("Custom Voice".into()),
            supported_modes: Some(vec!["Custom Voice".into()]),
            note: Some("Qwen3-TTS CustomVoice: 9 preset voices with instruct control, higher quality.".into()),
            char_limit_recommended: Some(700),
            char_limit_max: Some(850),
            char_limits_by_lang: None,
            separator: ".".into(),
            replace_guillemets: false,
            chunking_strategy: "Character Limit".into(),
            force_char_limit_chunking: Some(true),
            time_warning: None,
            voice_cloning: Some(false),
            needs_reference_transcript: Some(false),
        },
    );

    m.insert(
        "Qwen3-TTS-12Hz-1.7B-VoiceDesign",
        TtsModelConfig {
            mode: Some("Voice Design".into()),
            supported_modes: Some(vec!["Voice Design".into()]),
            note: Some("Qwen3-TTS VoiceDesign: generate voice from natural-language description.".into()),
            char_limit_recommended: Some(700),
            char_limit_max: Some(850),
            char_limits_by_lang: None,
            separator: ".".into(),
            replace_guillemets: false,
            chunking_strategy: "Character Limit".into(),
            force_char_limit_chunking: Some(true),
            time_warning: None,
            voice_cloning: Some(false),
            needs_reference_transcript: Some(false),
        },
    );

    m.insert(
        "VoxCPM2",
        TtsModelConfig {
            mode: Some("Voice Design".into()),
            supported_modes: Some(vec![
                "Voice Design".into(),
                "Controllable Cloning".into(),
                "Ultimate Cloning".into(),
            ]),
            note: Some("VoxCPM2 2B: Voice Design, Controllable Cloning and Ultimate Cloning, 30 languages, 48kHz output.".into()),
            // Verified 2026-07-19 via voxcpm2-cli tests: the stop predictor
            // misfires on chunks above ~450 chars (early stop mid-text, all
            // seeds, all voice modes). Chunks of ~400 chars complete reliably.
            char_limit_recommended: Some(400),
            char_limit_max: Some(450),
            char_limits_by_lang: None,
            separator: ".".into(),
            replace_guillemets: false,
            chunking_strategy: "Character Limit".into(),
            force_char_limit_chunking: Some(true),
            time_warning: None,
            voice_cloning: Some(true),
            needs_reference_transcript: Some(false),
        },
    );

    m
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelAsset {
    pub name: String,
    pub dest: String,
    #[serde(rename = "type")]
    pub asset_type: String,
    pub url: Option<String>,
    pub essential_files: Option<Vec<String>>,
}

pub fn model_assets() -> std::collections::HashMap<String, Vec<ModelAsset>> {
    let mut m = std::collections::HashMap::new();

    // Qwen3-TTS — all variants download from Serveurperso/Qwen3-TTS-GGUF.
    // The tokenizer is shared across all variants.
    // Source: engine_registry.json (verified 2026-06-30)
    let qwen_variants: &[(&str, &str)] = &[
        ("Qwen3-TTS-12Hz-0.6B-Base", "qwen3tts/Qwen3-TTS-12Hz-0.6B-Base"),
        ("Qwen3-TTS-12Hz-0.6B-CustomVoice", "qwen3tts/Qwen3-TTS-12Hz-0.6B-CustomVoice"),
        ("Qwen3-TTS-12Hz-1.7B-Base", "qwen3tts/Qwen3-TTS-12Hz-1.7B-Base"),
        ("Qwen3-TTS-12Hz-1.7B-CustomVoice", "qwen3tts/Qwen3-TTS-12Hz-1.7B-CustomVoice"),
        ("Qwen3-TTS-12Hz-1.7B-VoiceDesign", "qwen3tts/Qwen3-TTS-12Hz-1.7B-VoiceDesign"),
    ];

    for (name, dest) in qwen_variants {
        m.insert(
            name.to_string(),
            vec![
                ModelAsset {
                    name: format!("{} talker", name),
                    dest: dest.to_string(),
                    asset_type: "huggingface".into(),
                    url: Some("Serveurperso/Qwen3-TTS-GGUF".into()),
                    essential_files: Some(vec![
                        "talker-Q8_0.gguf".into(),
                    ]),
                },
                ModelAsset {
                    name: "Qwen3-TTS-Tokenizer-12Hz (shared)".into(),
                    dest: "qwen3tts/tokenizer".into(),
                    asset_type: "huggingface".into(),
                    url: Some("Serveurperso/Qwen3-TTS-GGUF".into()),
                    essential_files: Some(vec![
                        "tokenizer-Q8_0.gguf".into(),
                    ]),
                },
            ],
        );
    }

    m
}