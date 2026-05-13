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

import os
import logging
from typing import Any
from audiobook_generator.base_subprocess_plugin import BaseSubprocessPlugin
from audiobook_generator import config
from audiobook_generator.payload_types import VibeVoicePayload

logger = logging.getLogger(__name__)


class VibeVoicePlugin(BaseSubprocessPlugin):

    def _get_python_executable(self) -> str:
        return config.VIBEVOICE_PYTHON_EXECUTABLE

    def synthesize(self, model_instance: Any, text: str, output_path: str, **kwargs) -> bool:
        """
        Override to add speaker_wav validation before delegating to base class.
        """
        speaker_wav = kwargs.get('speaker_wav')
        if not speaker_wav:
            logger.error("VibeVoice: 'speaker_wav' not provided.")
            return False

        return super().synthesize(model_instance, text, output_path, **kwargs)

    def _build_payload(self, text: str, output_path: str, **kwargs) -> VibeVoicePayload:
        speaker_wav = kwargs.get('speaker_wav')
        temperature = kwargs.get('temperature', 0.9)
        top_p = kwargs.get('top_p', 0.9)
        cfg_scale = kwargs.get('cfg_scale', 1.3)
        diffusion_steps = kwargs.get('diffusion_steps', 15)
        voice_speed_factor = kwargs.get('voice_speed_factor', 1.0)
        use_sampling = kwargs.get('use_sampling', True)
        seed = kwargs.get('seed')

        return {
            "text": text,
            "output_path": output_path,
            "speaker_wav": speaker_wav,
            "model_name": self.name,
            "temperature": temperature,
            "top_p": top_p,
            "cfg_scale": cfg_scale,
            "diffusion_steps": diffusion_steps,
            "voice_speed_factor": voice_speed_factor,
            "use_sampling": use_sampling,
            "seed": seed
        }