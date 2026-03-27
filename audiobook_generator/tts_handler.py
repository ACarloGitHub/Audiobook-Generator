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

import os
import sys
import logging
import soundfile as sf
import numpy as np
from typing import Optional, Any, Dict, List
from scipy.io.wavfile import write as write_wav

# Import project-specific modules
from . import config
from . import utils
from .plugin_manager import plugin_manager

# --- Setup logging ---
logger = logging.getLogger(__name__)



# --- Model Loading Functions ---

def load_xtts_model() -> Optional[Any]:
    """Delega sempre il caricamento di XTTSv2 al Plugin Manager."""
    logger.info("Delegating XTTSv2 model loading to Plugin Manager.")
    return plugin_manager.load_model("XTTSv2")


def load_kokoro_model(language_code: str) -> Optional[Dict[str, Any]]:
    """Delega sempre il caricamento di Kokoro al Plugin Manager."""
    logger.info("Delegating Kokoro model loading to Plugin Manager.")
    return plugin_manager.load_model("Kokoro", language_code=language_code)

def load_vibevoice_model() -> Optional[Dict[str, Any]]:
    """Delega sempre il caricamento di VibeVoice al Plugin Manager."""
    logger.info("Delegating VibeVoice model loading to Plugin Manager.")
    return plugin_manager.load_model("VibeVoice")

def load_qwen3tts_model() -> Optional[Dict[str, Any]]:
    """Delega sempre il caricamento di Qwen3-TTS al Plugin Manager."""
    logger.info("Delegating Qwen3-TTS model loading to Plugin Manager.")
    return plugin_manager.load_model("Qwen3-TTS")


# --- Synthesis Functions ---

def synthesize_audio(model_name: str, model_instance: any, text: str, output_path: str, **kwargs) -> bool:
    """
    Funzione generica che delega la sintesi al Plugin Manager.
    """
    return plugin_manager.synthesize(model_name, text, output_path, model_instance, **kwargs)






# --- UI Helper Functions ---

def get_kokoro_voices(language_code: str) -> List[str]:
    lang_data = config.AVAILABLE_KOKORO_MODELS.get(language_code, {})
    return [voice.get("description", "Unknown Voice") for voice in lang_data.get("voices", [])]
