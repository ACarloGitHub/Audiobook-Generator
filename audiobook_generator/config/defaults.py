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
Default configuration constants for the Audiobook Generator.

Contains default application settings, timeout configuration,
TTS parameters, and text chunking parameters.
"""

from typing import Final

# --- Default Application Settings ---
DEFAULT_CLEANUP_CHUNKS: Final[bool] = False
DEFAULT_LANGUAGE: Final[str] = "en"
DEFAULT_AUDIO_FORMAT: Final[str] = "wav"
DEFAULT_SUBPROCESS_TIMEOUT: Final[int] = 1800  # 30 minutes

# --- Timeout Configuration for TTS Synthesis ---
TTS_TIMEOUT_SECONDS: Final[int] = 1800  # 30 minutes for normal synthesis
TTS_IDLE_TIMEOUT_SECONDS: Final[int] = 3600  # 1 hour for idle timeout
TTS_RECOVERY_TIMEOUT_SECONDS: Final[int] = 3600  # 1 hour for recovery
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

# --- Miscellaneous Constants ---
MIN_PYTHON_VERSION: Final[tuple[int, int]] = (3, 9)
RECOMMENDED_PYTHON_VERSION: Final[tuple[int, int]] = (3, 11)
MAX_PYTHON_VERSION_WARN: Final[tuple[int, int]] = (3, 12)

# --- Feature Flags ---
USE_PLUGIN_ARCHITECTURE: Final[bool] = True