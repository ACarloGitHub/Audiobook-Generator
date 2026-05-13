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