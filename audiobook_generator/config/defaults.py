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