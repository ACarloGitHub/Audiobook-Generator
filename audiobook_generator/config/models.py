# Copyright (c) 2026 Patata Audiobook Generator
#
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in all
# copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
# SOFTWARE.

"""
TTS model data and asset definitions for the Audiobook Generator.

Contains Kokoro voice models, per-model configuration (char limits,
chunking strategy, notes), and model asset paths for download verification.
"""

from typing import Any, Dict, Final

# --- Kokoro Voice Models ---
AVAILABLE_KOKORO_MODELS: Final[Dict[str, Any]] = {
    "it": {"kokoro_lang_code": "i", "voices": [{"id": "if_sara", "description": "Italian Female (Sara)"}, {"id": "im_nicola", "description": "Italian Male (Nicola)"}]},
    "en": {"kokoro_lang_code": "a", "voices": [{"id": "af_alloy", "description": "English US Female (Alloy)"}, {"id": "am_adam", "description": "English US Male (Adam)"}]},
    "fr": {"kokoro_lang_code": "f", "voices": [{"id": "ff_siwis", "description": "French Female (Siwis)"}]},
    "ja": {"kokoro_lang_code": "j", "voices": [{"id": "jf_alpha", "description": "Japanese Female (Alpha)"}]},
    "zh-cn": {"kokoro_lang_code": "z", "voices": [{"id": "zf_xiaobei", "description": "Chinese Female (Xiaobei)"}]}
}

# --- Model-Specific Constants ---
TTS_MODEL_CONFIG: Final[Dict[str, Dict[str, Any]]] = {
    "XTTSv2": {
        "char_limit_recommended": 250,
        "char_limit_max": 300,
        "separator": "|",
        "replace_guillemets": True,
        "chunking_strategy": "Character Limit",
        "note": "XTTSv2 has an internal token limit of ~400. To avoid errors, especially with non-English languages, it is strongly recommended to stay below 300 characters. The Pipe (|) separator is optimal for XTTSv2.",
        "char_limit_info": "XTTSv2 has an internal limit of ~400 tokens. To avoid errors, especially with non-English languages, it is strongly recommended to stay below 300 characters."
    },
    "Kokoro": {
        "char_limits_by_lang": {
            "it": {"min": 1800, "max": 2300, "note": "Italian: 1800-2300 characters (300-400 words)"},
            "en": {"min": 1800, "max": 2300, "note": "English: 1800-2300 characters (300-400 words)"},
            "fr": {"min": 1800, "max": 2300, "note": "French: 1800-2300 characters (300-400 words)"},
            "ja": {"min": 900, "max": 1100, "note": "Japanese: 900-1100 characters (700-1000 words)"},
            "zh-cn": {"min": 900, "max": 1100, "note": "Chinese: 900-1100 characters (700-1000 words)"}
        },
        "chunking_strategy": "Character Limit",
        "note": "Kokoro has a 512 token limit. Character limits vary by language: for Italian/English 1800-2300 characters, for Chinese/Japanese 900-1100 characters."
    },
    "VibeVoice": {
        "char_limit_recommended": 750,
        "char_limit_max": 20000,  # ~64k tokens correspond to ~20k characters
        "chunking_strategy": "Character Limit",
        "force_char_limit_chunking": True,
        "note": "VibeVoice supports up to 64k tokens (~90 minutes of conversation). A limit of 750 characters ensures better prosody. Can be increased if generation time is not a concern.",
        "time_warning": "VibeVoice is very slow. Longer chunks require more generation time."
    },
    "VibeVoice-1.5B": {
        "char_limit_recommended": 750,
        "char_limit_max": 20000,
        "chunking_strategy": "Character Limit",
        "force_char_limit_chunking": True,
        "note": "VibeVoice-1.5B supports up to 64k tokens (~90 minutes of conversation). A limit of 750 characters ensures better prosody.",
        "time_warning": "VibeVoice-1.5B is very slow. Longer chunks require more generation time."
    },
    "VibeVoice-7B": {
        "char_limit_recommended": 750,
        "char_limit_max": 20000,
        "chunking_strategy": "Character Limit",
        "force_char_limit_chunking": True,
        "note": "VibeVoice-7B supports up to 64k tokens (~90 minutes of conversation). A limit of 750 characters ensures better prosody.",
        "time_warning": "VibeVoice-7B is very slow. Longer chunks require more generation time."
    },
    "VibeVoice-Realtime-0.5B": {
        "char_limit_recommended": 750,
        "char_limit_max": 20000,
        "chunking_strategy": "Character Limit",
        "force_char_limit_chunking": True,
        "note": "VibeVoice-Realtime-0.5B supports up to 64k tokens (~90 minutes of conversation). A limit of 750 characters ensures better prosody.",
        "time_warning": "VibeVoice-Realtime-0.5B is faster than larger versions, but long chunks still require time."
    },
    "Qwen3-TTS-0.6B-Base": {
        "char_limit_recommended": 800,
        "char_limit_max": 1000,
        "chunking_strategy": "Character Limit",
        "force_char_limit_chunking": True,
        "mode": "Voice Clone",
        "note": "Qwen3-TTS-0.6B-Base supports Voice Clone. Optimal limit: 800 characters. Supports sequences up to 1000 characters.",
        "supported_modes": ["Voice Clone"]
    },
    "Qwen3-TTS-1.7B-Base": {
        "char_limit_recommended": 800,
        "char_limit_max": 1000,
        "chunking_strategy": "Character Limit",
        "force_char_limit_chunking": True,
        "mode": "Voice Clone",
        "note": "Qwen3-TTS-1.7B-Base supports Voice Clone. Optimal limit: 800 characters. Supports sequences up to 1000 characters.",
        "supported_modes": ["Voice Clone"]
    },
    "Qwen3-TTS-1.7B-CustomVoice": {
        "char_limit_recommended": 800,
        "char_limit_max": 1000,
        "chunking_strategy": "Character Limit",
        "force_char_limit_chunking": True,
        "mode": "Custom Voice",
        "note": "Qwen3-TTS-1.7B-CustomVoice supports Custom Voice (9 preset voices). Optimal limit: 800 characters.",
        "supported_modes": ["Custom Voice"]
    },
    "Qwen3-TTS-1.7B-VoiceDesign": {
        "char_limit_recommended": 800,
        "char_limit_max": 1000,
        "chunking_strategy": "Character Limit",
        "force_char_limit_chunking": True,
        "mode": "Voice Design",
        "note": "Qwen3-TTS-1.7B-VoiceDesign supports Voice Design (voice description in English). Optimal limit: 800 characters.",
        "supported_modes": ["Voice Design"]
    }
}

# --- Model Asset Management ---
MODEL_ASSETS: Final[Dict[str, Any]] = {
    "VibeVoice-1.5B": [
        {"type": "local", "path": "audiobook_generator/tts_models/vibevoice/1.5B"},
    ],
    "VibeVoice-7B": [
        {"type": "local", "path": "audiobook_generator/tts_models/vibevoice/7B"},
    ],
    "VibeVoice-Realtime-0.5B": [
        {"type": "local", "path": "audiobook_generator/tts_models/vibevoice/0.5B"},
    ],
    "Qwen3-TTS-0.6B-Base": [
        {"type": "local", "path": "audiobook_generator/tts_models/qwen3tts/Qwen3-TTS-12Hz-0.6B-Base"},
    ],
    "Qwen3-TTS-1.7B-Base": [
        {"type": "local", "path": "audiobook_generator/tts_models/qwen3tts/Qwen3-TTS-12Hz-1.7B-Base"},
    ],
    "Qwen3-TTS-1.7B-CustomVoice": [
        {"type": "local", "path": "audiobook_generator/tts_models/qwen3tts/Qwen3-TTS-12Hz-1.7B-CustomVoice"},
    ],
    "Qwen3-TTS-1.7B-VoiceDesign": [
        {"type": "local", "path": "audiobook_generator/tts_models/qwen3tts/Qwen3-TTS-12Hz-1.7B-VoiceDesign"},
    ],
    "Kokoro": [
        {"type": "local", "path": "audiobook_generator/tts_models/kokoro/models/hub/models--hexgrad--Kokoro-82M"}
    ],
    "XTTSv2": [
        {"type": "local", "path": "audiobook_generator/tts_models/xttsv2"}
    ]
}