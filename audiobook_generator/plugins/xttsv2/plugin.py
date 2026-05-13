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
from audiobook_generator.payload_types import XTTSv2Payload

logger = logging.getLogger(__name__)


class XTTSv2Plugin(BaseSubprocessPlugin):

    def _get_python_executable(self) -> str:
        return config.XTTSV2_PYTHON_EXECUTABLE

    def _build_payload(self, text: str, output_path: str, **kwargs) -> XTTSv2Payload:
        language = kwargs.get('language')
        speaker_wav = kwargs.get('speaker_wav')
        temperature = float(kwargs.get('temperature', 0.75))
        speed = float(kwargs.get('speed', 1.0))
        repetition_penalty = float(kwargs.get('repetition_penalty', 2.0))
        use_tts_splitting = kwargs.get('use_tts_splitting', True)
        sentence_separator = kwargs.get('sentence_separator', ".")
        max_retries = int(kwargs.get('max_retries', 3))

        return {
            "text": text,
            "output_path": output_path,
            "language": language,
            "speaker_wav": speaker_wav,
            "temperature": temperature,
            "speed": speed,
            "repetition_penalty": repetition_penalty,
            "use_tts_splitting": use_tts_splitting,
            "sentence_separator": sentence_separator,
            "max_retries": max_retries
        }