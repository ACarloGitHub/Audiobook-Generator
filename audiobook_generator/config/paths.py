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
Path constants for the Audiobook Generator.

Contains base project directories, external executable paths,
and plugin-specific venv/model paths.
"""

import os
from typing import Final

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

# --- Plugin-Specific Paths ---
VIBEVOICE_VENV_DIR: Final[str] = os.path.join(BASE_PROJECT_DIR, "audiobook_generator", "tts_models", "vibevoice", "venv")
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