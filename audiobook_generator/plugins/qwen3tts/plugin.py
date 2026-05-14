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

import logging
from audiobook_generator.base_subprocess_plugin import BaseSubprocessPlugin
from audiobook_generator import config
from audiobook_generator.payload_types import Qwen3TTSPayload

logger = logging.getLogger(__name__)


class Qwen3TTSPlugin(BaseSubprocessPlugin):

    def _get_python_executable(self) -> str:
        return config.QWEN3TTS_PYTHON_EXECUTABLE

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