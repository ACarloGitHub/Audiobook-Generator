import os
import logging
from typing import Any
from audiobook_generator.base_subprocess_plugin import BaseSubprocessPlugin
from audiobook_generator import config

logger = logging.getLogger(__name__)


class XTTSv2Plugin(BaseSubprocessPlugin):

    def _get_python_executable(self) -> str:
        return config.XTTSV2_PYTHON_EXECUTABLE

    def _build_payload(self, text: str, output_path: str, **kwargs) -> dict:
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