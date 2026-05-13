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