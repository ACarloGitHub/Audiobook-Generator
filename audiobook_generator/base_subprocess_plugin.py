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

import json
import logging
import os
import subprocess
from typing import Any, Dict, Optional

from .base_plugin import BaseTTSPlugin
from . import config
from .model_manager import model_manager

logger = logging.getLogger(__name__)


class BaseSubprocessPlugin(BaseTTSPlugin):
    """
    Base class for TTS plugins that communicate with a subprocess via JSON on stdin/stdout.

    Subclasses must implement:
        - _build_payload(text, output_path, **kwargs) -> dict
        - script_path property or attribute pointing to the subprocess script
        - python_executable config key

    Subclasses may override:
        - _get_python_executable() -> str
        - _get_script_path() -> str
    """

    def _get_python_executable(self) -> str:
        """Return the Python executable path for this plugin's venv. Must be overridden."""
        raise NotImplementedError

    def _get_script_path(self) -> str:
        """Return the path to the synthesize_subprocess.py script. Default is same directory as plugin."""
        return os.path.join(os.path.dirname(os.path.abspath(__file__)), 'synthesize_subprocess.py')

    def load_model(self, *args, **kwargs):
        """Default load_model: verify venv executable exists and check assets."""
        python_exe = self._get_python_executable()
        if not os.path.exists(python_exe):
            raise FileNotFoundError(f"Python executable not found at {python_exe}. Run the installer.")

        logger.info(f"Verifying venv: {python_exe} found.")

        logger.info(f"Checking assets for {self.name}...")
        if not model_manager.ensure_assets(self.name):
            logger.warning(f"Assets for {self.name} not present. The model may download automatically on first use.")
        else:
            logger.info(f"Assets for {self.name} verified.")

        return {"status": "ready"}

    def _build_payload(self, text: str, output_path: str, **kwargs) -> Dict[str, Any]:
        """Build the JSON payload to send to the subprocess. Must be overridden."""
        raise NotImplementedError

    def synthesize(self, model_instance: Any, text: str, output_path: str, **kwargs) -> bool:
        """
        Launch the subprocess for synthesis and communicate via JSON.

        This method handles:
        - process = None initialization before try
        - cwd=project_dir for correct relative paths
        - if process: process.kill() in except blocks
        - JSON response parsing
        - Logging via the logging module
        """
        script_path = self._get_script_path()
        payload = self._build_payload(text, output_path, **kwargs)

        # Pass timeout to subprocess so it can self-limit if desired
        payload["timeout_seconds"] = config.DEFAULT_SUBPROCESS_TIMEOUT

        process = None
        try:
            project_dir = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

            process = subprocess.Popen(
                [self._get_python_executable(), script_path],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                encoding='utf-8',
                cwd=project_dir
            )

            stdout_data, stderr_data = process.communicate(json.dumps(payload), timeout=config.DEFAULT_SUBPROCESS_TIMEOUT)

            if process.returncode != 0:
                logger.error(f"Subprocess {self.name} exited with code {process.returncode}.")
                logger.error(f"Stderr: {stderr_data}")
                logger.debug(f"Stdout (raw): {stdout_data}")
                return False

            try:
                response = json.loads(stdout_data)
            except json.JSONDecodeError as e:
                logger.error(f"Failed to decode JSON from {self.name} subprocess response.")
                logger.error(f"Stdout (raw): {stdout_data}")
                logger.error(f"Stderr (raw): {stderr_data}")
                logger.error(f"JSONDecodeError: {e}")
                return False

            if response.get("status") == "ok":
                logger.info(f"SUCCESS: {self.name} generated file: {response.get('file')}")
                return True
            else:
                logger.error(f"Error in {self.name} subprocess: {response.get('message')}")
                return False

        except subprocess.TimeoutExpired:
            logger.error(f"Timeout reached during synthesis with {self.name}.")
            if process:
                process.kill()
            return False
        except Exception as e:
            if process:
                process.kill()
            logger.error(f"Unexpected error in {self.name} subprocess: {e}", exc_info=True)
            return False