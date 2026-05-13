import os
import logging
from typing import Any
from audiobook_generator.base_subprocess_plugin import BaseSubprocessPlugin
from audiobook_generator import config
from audiobook_generator.model_manager import model_manager
from audiobook_generator.payload_types import Qwen3TTSPayload

logger = logging.getLogger(__name__)


class Qwen3TTS_1_7B_Plugin(BaseSubprocessPlugin):

    def _get_python_executable(self) -> str:
        return config.QWEN3TTS_PYTHON_EXECUTABLE

    def _build_payload(self, text: str, output_path: str, **kwargs) -> Qwen3TTSPayload:
        return {
            "text": text,
            "output_path": output_path,
            "mode": kwargs.get("qwen_mode"),
            "params": kwargs.get("qwen_params", {}),
            "model_size": "1.7B"
        }