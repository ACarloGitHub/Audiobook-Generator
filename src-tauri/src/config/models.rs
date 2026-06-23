use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KokoroVoice {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KokoroLangEntry {
    pub kokoro_lang_code: String,
    pub voices: Vec<KokoroVoice>,
}

pub fn available_kokoro_models() -> HashMap<&'static str, KokoroLangEntry> {
    let mut m = HashMap::new();
    m.insert(
        "it",
        KokoroLangEntry {
            kokoro_lang_code: "i".into(),
            voices: vec![
                KokoroVoice { id: "if_sara".into(), description: "Italian Female (Sara)".into() },
                KokoroVoice { id: "im_nicola".into(), description: "Italian Male (Nicola)".into() },
            ],
        },
    );
    m.insert(
        "en",
        KokoroLangEntry {
            kokoro_lang_code: "a".into(),
            voices: vec![
                KokoroVoice { id: "af_alloy".into(), description: "English US Female (Alloy)".into() },
                KokoroVoice { id: "am_adam".into(), description: "English US Male (Adam)".into() },
                KokoroVoice { id: "af_heart".into(), description: "English US Female (Heart)".into() },
                KokoroVoice { id: "am_michael".into(), description: "English US Male (Michael)".into() },
            ],
        },
    );
    m.insert(
        "fr",
        KokoroLangEntry {
            kokoro_lang_code: "f".into(),
            voices: vec![
                KokoroVoice { id: "ff_siwis".into(), description: "French Female (Siwis)".into() },
                KokoroVoice { id: "fm_denis".into(), description: "French Male (Denis)".into() },
            ],
        },
    );
    m.insert(
        "ja",
        KokoroLangEntry {
            kokoro_lang_code: "j".into(),
            voices: vec![
                KokoroVoice { id: "jf_alpha".into(), description: "Japanese Female (Alpha)".into() },
                KokoroVoice { id: "jm_gong".into(), description: "Japanese Male (Gong)".into() },
            ],
        },
    );
    m.insert(
        "zh-cn",
        KokoroLangEntry {
            kokoro_lang_code: "z".into(),
            voices: vec![
                KokoroVoice { id: "zf_xiaobei".into(), description: "Chinese Female (Xiaobei)".into() },
                KokoroVoice { id: "zm_yunxi".into(), description: "Chinese Male (Yunxi)".into() },
            ],
        },
    );
    m.insert(
        "es",
        KokoroLangEntry {
            kokoro_lang_code: "e".into(),
            voices: vec![
                KokoroVoice { id: "ef_dora".into(), description: "Spanish Female (Dora)".into() },
                KokoroVoice { id: "em_alex".into(), description: "Spanish Male (Alex)".into() },
            ],
        },
    );
    m.insert(
        "pt",
        KokoroLangEntry {
            kokoro_lang_code: "p".into(),
            voices: vec![
                KokoroVoice { id: "pf_dora".into(), description: "Portuguese Female (Dora)".into() },
                KokoroVoice { id: "pm_alex".into(), description: "Portuguese Male (Alex)".into() },
            ],
        },
    );
    m.insert(
        "hi",
        KokoroLangEntry {
            kokoro_lang_code: "h".into(),
            voices: vec![
                KokoroVoice { id: "hf_alpha".into(), description: "Hindi Female (Alpha)".into() },
                KokoroVoice { id: "hm_beta".into(), description: "Hindi Male (Beta)".into() },
            ],
        },
    );
    m
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharLimitRange {
    pub min: usize,
    pub max: usize,
}

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

pub fn tts_model_config() -> HashMap<&'static str, TtsModelConfig> {
    let mut m = HashMap::new();

    m.insert(
        "XTTSv2",
        TtsModelConfig {
            char_limit_recommended: Some(250),
            char_limit_max: Some(300),
            char_limits_by_lang: None,
            separator: "|".into(),
            replace_guillemets: true,
            chunking_strategy: "Character Limit".into(),
            force_char_limit_chunking: None,
            mode: None,
            supported_modes: None,
            note: Some("XTTSv2 uses a pipe separator and recommends keeping chunks under 250 characters for best quality.".into()),
            time_warning: None,
            voice_cloning: Some(true),
            needs_reference_transcript: Some(true),
        },
    );

    let mut kokoro_langs = HashMap::new();
    kokoro_langs.insert("it".into(), CharLimitRange { min: 1800, max: 2300 });
    kokoro_langs.insert("en".into(), CharLimitRange { min: 1800, max: 2300 });
    kokoro_langs.insert("fr".into(), CharLimitRange { min: 1800, max: 2300 });
    kokoro_langs.insert("ja".into(), CharLimitRange { min: 900, max: 1100 });
    kokoro_langs.insert("zh-cn".into(), CharLimitRange { min: 900, max: 1100 });
    m.insert(
        "Kokoro",
        TtsModelConfig {
            char_limit_recommended: None,
            char_limit_max: None,
            char_limits_by_lang: Some(kokoro_langs),
            separator: ".".into(),
            replace_guillemets: false,
            chunking_strategy: "Character Limit".into(),
            force_char_limit_chunking: None,
            mode: None,
            supported_modes: None,
            note: Some("Kokoro supports 8 languages. Use the character limit for your language: 1800-2300 for Latin scripts, 900-1100 for CJK.".into()),
            time_warning: None,
            voice_cloning: Some(false),
            needs_reference_transcript: Some(false),
        },
    );

    let vibevoice_config = TtsModelConfig {
        char_limit_recommended: Some(750),
        char_limit_max: Some(20000),
        char_limits_by_lang: None,
        separator: ".".into(),
        replace_guillemets: false,
        chunking_strategy: "Character Limit".into(),
        force_char_limit_chunking: Some(true),
        mode: None,
        supported_modes: None,
        note: Some("VibeVoice requires a reference WAV for voice cloning.".into()),
        time_warning: Some("Large texts may take several minutes per chunk.".into()),
        voice_cloning: Some(true),
        needs_reference_transcript: Some(false),
    };
    m.insert("VibeVoice-1.5B", vibevoice_config.clone());
    m.insert("VibeVoice-7B", vibevoice_config.clone());
    m.insert(
        "VibeVoice-Realtime-0.5B",
        TtsModelConfig {
            time_warning: Some("Faster than larger VibeVoice models but lower quality.".into()),
            ..vibevoice_config
        },
    );

    let qwen3_base_config = TtsModelConfig {
        char_limit_recommended: Some(800),
        char_limit_max: Some(1000),
        char_limits_by_lang: None,
        separator: ".".into(),
        replace_guillemets: false,
        chunking_strategy: "Character Limit".into(),
        force_char_limit_chunking: Some(true),
        mode: Some("Voice Clone".into()),
        supported_modes: Some(vec!["Voice Clone".into()]),
        note: None,
        time_warning: None,
        voice_cloning: Some(true),
        needs_reference_transcript: Some(false),
    };
    m.insert("Qwen3-TTS-0.6B-Base", qwen3_base_config.clone());
    m.insert("Qwen3-TTS-1.7B-Base", qwen3_base_config);

    m.insert(
        "Qwen3-TTS-1.7B-CustomVoice",
        TtsModelConfig {
            char_limit_recommended: Some(800),
            char_limit_max: Some(1000),
            char_limits_by_lang: None,
            separator: ".".into(),
            replace_guillemets: false,
            chunking_strategy: "Character Limit".into(),
            force_char_limit_chunking: Some(true),
            mode: Some("Custom Voice".into()),
            supported_modes: Some(vec!["Custom Voice".into()]),
            note: None,
            time_warning: None,
            voice_cloning: Some(true),
            needs_reference_transcript: Some(false),
        },
    );

    m.insert(
        "Qwen3-TTS-1.7B-VoiceDesign",
        TtsModelConfig {
            char_limit_recommended: Some(800),
            char_limit_max: Some(1000),
            char_limits_by_lang: None,
            separator: ".".into(),
            replace_guillemets: false,
            chunking_strategy: "Character Limit".into(),
            force_char_limit_chunking: Some(true),
            mode: Some("Voice Design".into()),
            supported_modes: Some(vec!["Voice Design".into()]),
            note: None,
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

pub fn model_assets() -> HashMap<&'static str, Vec<ModelAsset>> {
    let mut m = HashMap::new();

    m.insert(
        "Kokoro",
        vec![ModelAsset {
            name: "Kokoro-82M-ONNX".into(),
            dest: "kokoro".into(),
            asset_type: "huggingface".into(),
            url: Some("onnx-community/Kokoro-82M-v1.0-ONNX".into()),
            essential_files: Some(vec![
                "models/model_quantized.onnx".into(),
                "voices/af_heart.bin".into(),
            ]),
        }],
    );

    m.insert(
        "Qwen3-TTS-0.6B-Base",
        vec![ModelAsset {
            name: "Qwen3-TTS-0.6B-Base".into(),
            dest: "qwen3tts/Qwen3-TTS-12Hz-0.6B-Base".into(),
            asset_type: "huggingface".into(),
            url: Some("Qwen/Qwen3-TTS-12Hz-0.6B-Base".into()),
            essential_files: Some(vec![
                "config.json".into(),
                "model.safetensors".into(),
            ]),
        }],
    );

    m.insert(
        "Qwen3-TTS-1.7B-Base",
        vec![ModelAsset {
            name: "Qwen3-TTS-1.7B-Base".into(),
            dest: "qwen3tts/Qwen3-TTS-12Hz-1.7B-Base".into(),
            asset_type: "huggingface".into(),
            url: Some("Qwen/Qwen3-TTS-12Hz-1.7B-Base".into()),
            essential_files: Some(vec![
                "config.json".into(),
                "model.safetensors".into(),
            ]),
        }],
    );

    m.insert(
        "Qwen3-TTS-1.7B-CustomVoice",
        vec![ModelAsset {
            name: "Qwen3-TTS-1.7B-CustomVoice".into(),
            dest: "qwen3tts/Qwen3-TTS-12Hz-1.7B-CustomVoice".into(),
            asset_type: "huggingface".into(),
            url: Some("Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice".into()),
            essential_files: Some(vec![
                "config.json".into(),
                "model.safetensors".into(),
            ]),
        }],
    );

    m.insert(
        "Qwen3-TTS-1.7B-VoiceDesign",
        vec![ModelAsset {
            name: "Qwen3-TTS-1.7B-VoiceDesign".into(),
            dest: "qwen3tts/Qwen3-TTS-12Hz-1.7B-VoiceDesign".into(),
            asset_type: "huggingface".into(),
            url: Some("Qwen/Qwen3-TTS-12Hz-1.7B-VoiceDesign".into()),
            essential_files: Some(vec![
                "config.json".into(),
                "model.safetensors".into(),
            ]),
        }],
    );

    m.insert(
        "VibeVoice-1.5B",
        vec![ModelAsset {
            name: "VibeVoice-1.5B".into(),
            dest: "vibevoice/1.5B".into(),
            asset_type: "huggingface".into(),
            url: Some("vibevoice/VibeVoice-1.5B".into()),
            essential_files: Some(vec![
                "config.json".into(),
                "preprocessor_config.json".into(),
                "model.safetensors".into(),
            ]),
        }],
    );

    m.insert(
        "VibeVoice-7B",
        vec![ModelAsset {
            name: "VibeVoice-7B".into(),
            dest: "vibevoice/7B".into(),
            asset_type: "huggingface".into(),
            url: Some("vibevoice/VibeVoice-7B".into()),
            essential_files: Some(vec![
                "config.json".into(),
                "preprocessor_config.json".into(),
            ]),
        }],
    );

    m.insert(
        "VibeVoice-Realtime-0.5B",
        vec![ModelAsset {
            name: "VibeVoice-Realtime-0.5B".into(),
            dest: "vibevoice/0.5B".into(),
            asset_type: "huggingface".into(),
            url: Some("microsoft/VibeVoice-Realtime-0.5B".into()),
            essential_files: Some(vec![
                "config.json".into(),
                "preprocessor_config.json".into(),
            ]),
        }],
    );

    m.insert(
        "XTTSv2",
        vec![ModelAsset {
            name: "XTTSv2".into(),
            dest: "xttsv2".into(),
            asset_type: "huggingface".into(),
            url: Some("coqui/XTTS-v2".into()),
            essential_files: Some(vec![
                "config.json".into(),
                "model.pth".into(),
                "dvae.pth".into(),
            ]),
        }],
    );

    m
}