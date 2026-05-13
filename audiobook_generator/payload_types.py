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
TypedDict definitions for subprocess JSON payloads.

Each plugin sends a JSON payload to its subprocess via stdin. These TypedDicts
document the expected fields, their types, and which are required vs optional.
The BaseSubprocessPlugin also injects a 'timeout_seconds' field automatically.
"""

from typing import Optional, TypedDict


class XTTSv2Payload(TypedDict, total=False):
    """Payload for XTTSv2 subprocess."""
    text: str
    output_path: str
    language: Optional[str]
    speaker_wav: Optional[str]
    temperature: float
    speed: float
    repetition_penalty: float
    use_tts_splitting: bool
    sentence_separator: str
    max_retries: int
    timeout_seconds: int


class KokoroPayload(TypedDict, total=False):
    """Payload for Kokoro subprocess."""
    text: str
    output_path: str
    voice_id: Optional[str]
    speed: float
    language_code: str
    timeout_seconds: int


class VibeVoicePayload(TypedDict, total=False):
    """Payload for VibeVoice subprocess."""
    text: str
    output_path: str
    speaker_wav: str
    model_name: str
    temperature: float
    top_p: float
    cfg_scale: float
    diffusion_steps: int
    voice_speed_factor: float
    use_sampling: bool
    seed: Optional[int]
    timeout_seconds: int


class Qwen3TTSParams(TypedDict, total=False):
    """Nested 'params' dict for Qwen3-TTS payloads."""
    speed: float
    pitch: int
    volume: int
    temperature: float
    top_p: float
    top_k: int
    repetition_penalty: float
    seed: Optional[int]
    speaker: str
    voice: str
    language: str
    instruct: str
    ref_audio: Optional[str]
    ref_text: str
    x_vector_only_mode: bool


class Qwen3TTSPayload(TypedDict, total=False):
    """Payload for Qwen3-TTS subprocess (all variants)."""
    text: str
    output_path: str
    mode: Optional[str]
    params: Qwen3TTSParams
    model_size: str
    model_type: str
    timeout_seconds: int