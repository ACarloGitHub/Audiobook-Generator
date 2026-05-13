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
from audiobook_generator.payload_types import KokoroPayload

logger = logging.getLogger(__name__)


class KokoroPlugin(BaseSubprocessPlugin):

    def _get_python_executable(self) -> str:
        return config.KOKORO_PYTHON_EXECUTABLE

    def _build_payload(self, text: str, output_path: str, **kwargs) -> KokoroPayload:
        voice_id = kwargs.get('voice_id')
        speed = float(kwargs.get('speed', 1.0))
        language_code = kwargs.get('language_code', 'en')

        if not voice_id:
            logger.error("Kokoro: 'voice_id' not provided.")
            raise ValueError("voice_id is required for Kokoro synthesis")

        return {
            "text": text,
            "output_path": output_path,
            "voice_id": voice_id,
            "speed": speed,
            "language_code": language_code
        }