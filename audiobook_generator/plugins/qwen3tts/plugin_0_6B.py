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
from audiobook_generator.model_manager import model_manager
from audiobook_generator.payload_types import Qwen3TTSPayload

logger = logging.getLogger(__name__)


class Qwen3TTS_0_6B_Plugin(BaseSubprocessPlugin):

    def _get_python_executable(self) -> str:
        return config.QWEN3TTS_PYTHON_EXECUTABLE

    def _build_payload(self, text: str, output_path: str, **kwargs) -> Qwen3TTSPayload:
        return {
            "text": text,
            "output_path": output_path,
            "mode": kwargs.get("qwen_mode"),
            "params": kwargs.get("qwen_params", {}),
            "model_size": "0.6B"
        }