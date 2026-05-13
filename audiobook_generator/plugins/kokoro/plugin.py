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