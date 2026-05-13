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


class Qwen3TTSPlugin(BaseSubprocessPlugin):

    def _get_python_executable(self) -> str:
        return config.QWEN3TTS_PYTHON_EXECUTABLE

    def load_model(self, *args, **kwargs):
        if not os.path.exists(config.QWEN3TTS_PYTHON_EXECUTABLE):
            raise FileNotFoundError(f"Python executable for {self.name} not found. Run the installer.")

        logger.info(f"Checking assets for {self.name}...")
        if not model_manager.ensure_assets(self.name):
            raise RuntimeError(f"Asset download for {self.name} failed.")

        return {"status": "ready"}

    def _build_payload(self, text: str, output_path: str, **kwargs) -> Qwen3TTSPayload:
        mode = kwargs.get("qwen_mode")
        if mode is None:
            mode = "clone"
            logger.warning(f"qwen_mode not provided, defaulting to '{mode}'")
        if mode == "custom":
            model_type = "custom_voice"
        elif mode == "clone":
            model_type = "base"
        elif mode == "design":
            model_type = "voice_design"
        else:
            model_type = "base"
            logger.warning(f"Unrecognized mode '{mode}', using model_type='base'")

        params = kwargs.get("qwen_params", {})
        model_size = params.get("model_size", "0.6B")
        if model_type in ("custom_voice", "voice_design"):
            model_size = "1.7B"

        return {
            "text": text,
            "output_path": output_path,
            "mode": mode,
            "params": params,
            "model_size": model_size,
            "model_type": model_type
        }