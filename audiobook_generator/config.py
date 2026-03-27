# Copyright 2025 Carlo Piras
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""
This module centralizes all configuration constants for the Audiobook Generator.
It defines file paths, default parameters, and other constants used throughout the application.
"""

import os
from typing import Final, Dict, Any

# --- Base Project Paths ---
BASE_PROJECT_DIR: Final[str] = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# --- Main Application Directories ---
FFMPEG_DIR: Final[str] = os.path.join(BASE_PROJECT_DIR, "ffmpeg", "bin")
REFERENCE_VOICES_DIR: Final[str] = os.path.join(BASE_PROJECT_DIR, "Reference_Voices")
EBOOK_SOURCE_DIR: Final[str] = os.path.join(BASE_PROJECT_DIR, "Ebooks_to_Convert")
OUTPUT_BASE_DIR: Final[str] = os.path.join(BASE_PROJECT_DIR, "Generated_Audiobooks")
CHUNK_OUTPUT_BASE_DIR: Final[str] = os.path.join(BASE_PROJECT_DIR, "Intermediate_Audio_Chunks")
DEMO_OUTPUT_DIR: Final[str] = os.path.join(BASE_PROJECT_DIR, "Demo_Outputs")
TEST_FILES_DIR: Final[str] = os.path.join(BASE_PROJECT_DIR, "test_files")
TTS_MODELS_DIR: Final[str] = os.path.join(BASE_PROJECT_DIR, "audiobook_generator", "tts_models")


# --- External Executable Paths ---
DEFAULT_FFMPEG_EXE: Final[str] = os.path.join(FFMPEG_DIR, "ffmpeg.exe") if os.name == 'nt' else os.path.join(FFMPEG_DIR, "ffmpeg")
DEFAULT_ESPEAK_EXE: Final[str] = "espeak-ng"

# --- Default Application Settings ---
DEFAULT_CLEANUP_CHUNKS: Final[bool] = False
DEFAULT_LANGUAGE: Final[str] = "en"
DEFAULT_AUDIO_FORMAT: Final[str] = "wav"
DEFAULT_SUBPROCESS_TIMEOUT: Final[int] = 1800  # 30 minuti (300 secondi = 5 minuti era troppo breve)

# --- Timeout Configuration for TTS Synthesis ---
TTS_TIMEOUT_SECONDS: Final[int] = 1800  # 30 minuti per sintesi normale
TTS_IDLE_TIMEOUT_SECONDS: Final[int] = 3600  # 1 ora per idle timeout
TTS_RECOVERY_TIMEOUT_SECONDS: Final[int] = 3600  # 1 ora per recovery
DEFAULT_TTS_NOISE_SCALE: Final[float] = 0.667
DEFAULT_TTS_NOISE_SCALE_W: Final[float] = 0.8

# --- Default TTS Model Parameters ---
DEFAULT_TTS_TEMPERATURE: Final[float] = 0.70
DEFAULT_TTS_SPEED: Final[float] = 1.0
DEFAULT_TTS_REPETITION_PENALTY: Final[float] = 2.0

# --- Default Text Chunking Parameters ---
DEFAULT_REPLACE_GUILLEMETS: Final[bool] = True
DEFAULT_SENTENCE_SEPARATOR: Final[str] = "."
DEFAULT_USE_CHAR_LIMIT_CHUNKING: Final[bool] = True  # Use character limit chunking by default to avoid XTTSv2 token limit errors
DEFAULT_MAX_CHARS_PER_CHUNK: Final[int] = 250  # Balanced limit to avoid token errors while preserving sentence integrity
DEFAULT_MIN_WORDS_APPROX: Final[int] = 400
DEFAULT_MAX_WORDS_APPROX: Final[int] = 800

# --- TTS Model Data (Moved from tts_handler.py to prevent circular imports) ---

AVAILABLE_KOKORO_MODELS: Final[Dict[str, Any]] = {
    "it": {"kokoro_lang_code": "i", "voices": [{"id": "if_sara", "description": "Italian Female (Sara)"}, {"id": "im_nicola", "description": "Italian Male (Nicola)"}]},
    "en": {"kokoro_lang_code": "a", "voices": [{"id": "af_alloy", "description": "English US Female (Alloy)"}, {"id": "am_adam", "description": "English US Male (Adam)"}]},
    "fr": {"kokoro_lang_code": "f", "voices": [{"id": "ff_siwis", "description": "French Female (Siwis)"}]},
    "ja": {"kokoro_lang_code": "j", "voices": [{"id": "jf_alpha", "description": "Japanese Female (Alpha)"}]},
    "zh-cn": {"kokoro_lang_code": "z", "voices": [{"id": "zf_xiaobei", "description": "Chinese Female (Xiaobei)"}]}
}

# --- Model-Specific Constants ---
# Centralizes model-specific limits and recommendations.
TTS_MODEL_CONFIG: Final[Dict[str, Dict[str, Any]]] = {
    "XTTSv2": {
        "char_limit_recommended": 250,
        "char_limit_max": 300,
        "separator": "|",
        "replace_guillemets": True,
        "chunking_strategy": "Character Limit",
        "note": "XTTSv2 ha un limite interno di ~400 token. Per evitare errori, specialmente con lingue non inglesi, si raccomanda di restare sotto i 300 caratteri. Il separatore Pipe (|) è ottimale per XTTSv2.",
        "char_limit_info": "XTTSv2 has an internal limit of ~400 tokens. To avoid errors, especially with non-English languages, it is strongly recommended to stay below 300 characters."
    },
    "Kokoro": {
        "char_limits_by_lang": {
            "it": {"min": 1800, "max": 2300, "note": "Italiano: 1800-2300 caratteri (300-400 parole)"},
            "en": {"min": 1800, "max": 2300, "note": "Inglese: 1800-2300 caratteri (300-400 parole)"},
            "fr": {"min": 1800, "max": 2300, "note": "Francese: 1800-2300 caratteri (300-400 parole)"},
            "ja": {"min": 900, "max": 1100, "note": "Giapponese: 900-1100 caratteri (700-1000 parole)"},
            "zh-cn": {"min": 900, "max": 1100, "note": "Cinese: 900-1100 caratteri (700-1000 parole)"}
        },
        "chunking_strategy": "Character Limit",
        "note": "Kokoro ha un limite di 512 token. Il limite di caratteri varia in base alla lingua: per italiano/inglese 1800-2300 caratteri, per cinese/giapponese 900-1100 caratteri."
    },
    "VibeVoice": {
        "char_limit_recommended": 750,
        "char_limit_max": 20000,  # ~64k token corrispondono a ~20k caratteri
        "chunking_strategy": "Character Limit",
        "note": "VibeVoice supporta fino a 64k token (~90 minuti di conversazione). Un limite di 750 caratteri garantisce una prosodia migliore. Si può aumentare se non si vuole aspettare troppo per la generazione.",
        "time_warning": "VibeVoice è molto lento. Chunk più lunghi richiedono più tempo di generazione."
    },
    "VibeVoice-1.5B": {
        "char_limit_recommended": 750,
        "char_limit_max": 20000,
        "chunking_strategy": "Character Limit",
        "note": "VibeVoice-1.5B supporta fino a 64k token (~90 minuti di conversazione). Un limite di 750 caratteri garantisce una prosodia migliore.",
        "time_warning": "VibeVoice-1.5B è molto lento. Chunk più lunghi richiedono più tempo di generazione."
    },
    "VibeVoice-7B": {
        "char_limit_recommended": 750,
        "char_limit_max": 20000,
        "chunking_strategy": "Character Limit",
        "note": "VibeVoice-7B supporta fino a 64k token (~90 minuti di conversazione). Un limite di 750 caratteri garantisce una prosodia migliore.",
        "time_warning": "VibeVoice-7B è molto lento. Chunk più lunghi richiedono più tempo di generazione."
    },
    "VibeVoice-Realtime-0.5B": {
        "char_limit_recommended": 750,
        "char_limit_max": 20000,
        "chunking_strategy": "Character Limit",
        "note": "VibeVoice-Realtime-0.5B supporta fino a 64k token (~90 minuti di conversazione). Un limite di 750 caratteri garantisce una prosodia migliore.",
        "time_warning": "VibeVoice-Realtime-0.5B è più veloce delle versioni più grandi, ma chunk lunghi richiedono comunque tempo."
    },
    "Qwen3-TTS-0.6B-Base": {
        "char_limit_recommended": 600,
        "char_limit_max": 1000,
        "chunking_strategy": "Character Limit",
        "mode": "Voice Clone",
        "note": "Qwen3-TTS-0.6B-Base supporta Voice Clone. Limite ottimale: 600 caratteri (85-100 parole). Supporta sequenze lunghe fino a 1000 caratteri.",
        "supported_modes": ["Voice Clone"]
    },
    "Qwen3-TTS-1.7B-Base": {
        "char_limit_recommended": 600,
        "char_limit_max": 1000,
        "chunking_strategy": "Character Limit",
        "mode": "Voice Clone",
        "note": "Qwen3-TTS-1.7B-Base supporta Voice Clone. Limite ottimale: 600 caratteri (85-100 parole). Supporta sequenze lunghe fino a 1000 caratteri.",
        "supported_modes": ["Voice Clone"]
    },
    "Qwen3-TTS-1.7B-CustomVoice": {
        "char_limit_recommended": 600,
        "char_limit_max": 1000,
        "chunking_strategy": "Character Limit",
        "mode": "Custom Voice",
        "note": "Qwen3-TTS-1.7B-CustomVoice supporta Custom Voice (9 voci predefinite). Limite ottimale: 600 caratteri (85-100 parole).",
        "supported_modes": ["Custom Voice"]
    },
    "Qwen3-TTS-1.7B-VoiceDesign": {
        "char_limit_recommended": 600,
        "char_limit_max": 1000,
        "chunking_strategy": "Character Limit",
        "mode": "Voice Design",
        "note": "Qwen3-TTS-1.7B-VoiceDesign supporta Voice Design (descrizione vocale in inglese). Limite ottimale: 600 caratteri (85-100 parole).",
        "supported_modes": ["Voice Design"]
    }
}

# --- Miscellaneous Constants ---
MIN_PYTHON_VERSION: Final[tuple[int, int]] = (3, 9)
RECOMMENDED_PYTHON_VERSION: Final[tuple[int, int]] = (3, 11)
MAX_PYTHON_VERSION_WARN: Final[tuple[int, int]] = (3, 12)

# --- Feature Flags ---
USE_PLUGIN_ARCHITECTURE: Final[bool] = True

# --- Plugin-Specific Paths ---
VIBEVOICE_VENV_DIR: Final[str] = os.path.join(BASE_PROJECT_DIR, "audiobook_generator", "tts_models", "vibevoice", "venv")
# Determina il percorso corretto dell'eseguibile Python all'interno del venv per compatibilità cross-platform
_vibevoice_python_executable = "python.exe" if os.name == 'nt' else "python"
_vibevoice_bin_dir = "Scripts" if os.name == 'nt' else "bin"
VIBEVOICE_PYTHON_EXECUTABLE: Final[str] = os.path.join(VIBEVOICE_VENV_DIR, _vibevoice_bin_dir, _vibevoice_python_executable)

QWEN3TTS_VENV_DIR: Final[str] = os.path.join(BASE_PROJECT_DIR, "audiobook_generator", "tts_models", "qwen3tts", "venv")
_qwen3tts_python_executable = "python.exe" if os.name == 'nt' else "python"
_qwen3tts_bin_dir = "Scripts" if os.name == 'nt' else "bin"
QWEN3TTS_PYTHON_EXECUTABLE: Final[str] = os.path.join(QWEN3TTS_VENV_DIR, _qwen3tts_bin_dir, _qwen3tts_python_executable)

# --- Kokoro Plugin Paths ---
KOKORO_VENV_DIR: Final[str] = os.path.join(BASE_PROJECT_DIR, "audiobook_generator", "tts_models", "kokoro", "venv")
_kokoro_python_executable = "python.exe" if os.name == 'nt' else "python"
_kokoro_bin_dir = "Scripts" if os.name == 'nt' else "bin"
KOKORO_PYTHON_EXECUTABLE: Final[str] = os.path.join(KOKORO_VENV_DIR, _kokoro_bin_dir, _kokoro_python_executable)
KOKORO_MODELS_DIR: Final[str] = os.path.join(TTS_MODELS_DIR, "kokoro", "models")

# --- XTTSv2 Plugin Paths ---
XTTSV2_VENV_DIR: Final[str] = os.path.join(BASE_PROJECT_DIR, "audiobook_generator", "tts_models", "xttsv2", "venv")
_xttsv2_python_executable = "python.exe" if os.name == 'nt' else "python"
_xttsv2_bin_dir = "Scripts" if os.name == 'nt' else "bin"
XTTSV2_PYTHON_EXECUTABLE: Final[str] = os.path.join(XTTSV2_VENV_DIR, _xttsv2_bin_dir, _xttsv2_python_executable)
XTTSV2_MODELS_DIR: Final[str] = os.path.join(TTS_MODELS_DIR, "xttsv2")

# --- Model Asset Management ---
MODEL_ASSETS: Final[Dict[str, Any]] = {
    # URL HuggingFace verificati (21/03/2026):
    #   1.5B:   microsoft/VibeVoice-1.5B (≈3B parametri)
    #   7B:     vibevoice/VibeVoice-7B (community, ≈7B parametri)
    #   Realtime-0.5B: microsoft/VibeVoice-Realtime-0.5B
    # URL NON esistenti: VibeVoice-1.5B-full, VibeVoice-7B-low-vram
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
        # Struttura: qwen3tts/Qwen3-TTS-12Hz-{version}-{type}/ (NO models/ sottocartella)
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
    # Aggiungere qui altri modelli in futuro (es. Step-Audio)
}
