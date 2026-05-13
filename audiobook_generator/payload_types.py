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