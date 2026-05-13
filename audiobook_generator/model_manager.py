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
import subprocess
from . import config

logger = logging.getLogger(__name__)

def run_command_git(command):
    """Execute a git command and handle output."""
    try:
        logger.info("Running: %s", ' '.join(command))
        subprocess.run(command, check=True, capture_output=True, text=True, encoding='utf-8')
        return True
    except FileNotFoundError:
        logger.error("'git' is not installed or not in PATH.")
        return False
    except subprocess.CalledProcessError as e:
        logger.error("git command failed:\n%s", e.stderr)
        return False

class ModelManager:
    def ensure_assets(self, model_name: str) -> bool:
        if model_name not in config.MODEL_ASSETS:
            return True 

        assets = config.MODEL_ASSETS[model_name]
        for asset in assets:
            relative_path = asset.get("dest") or asset.get("path")
            dest_path = os.path.join(config.BASE_PROJECT_DIR, relative_path) if relative_path else None
            
            if self._check_asset_exists(asset):
                logger.info("Asset for '%s' found locally: %s", model_name, dest_path)
                continue
            
            if asset.get("type") == "local":
                logger.error("Local asset missing for '%s': %s", model_name, dest_path)
                return False
            
            logger.warning("Asset missing in '%s'. Attempting download...", dest_path)
            if not self._download_asset(asset):
                logger.error("Download failed for asset from %s.", asset.get('url', 'N/A'))
                return False
        
        logger.info("All assets for '%s' are ready.", model_name)
        return True

    def _check_asset_exists(self, asset_info: dict) -> bool:
        relative_path = asset_info.get("dest") or asset_info.get("path")
        dest_path = os.path.join(config.BASE_PROJECT_DIR, relative_path) if relative_path else None
        if not dest_path or not os.path.exists(dest_path):
            return False
        
        # For VibeVoice models, verify essential files
        if "VibeVoice" in dest_path:
            essential_files = ["config.json", "preprocessor_config.json"]
            for file in essential_files:
                if not os.path.exists(os.path.join(dest_path, file)):
                    return False
            # Verify that at least model.safetensors or model.safetensors.index.json exists
            model_file = os.path.join(dest_path, "model.safetensors")
            model_index = os.path.join(dest_path, "model.safetensors.index.json")
            if not os.path.exists(model_file) and not os.path.exists(model_index):
                return False
        
        return True

    def _download_asset(self, asset_info: dict) -> bool:
        asset_type = asset_info.get("type", "git")
        relative_path = asset_info.get("dest") or asset_info.get("path")
        dest_path = os.path.join(config.BASE_PROJECT_DIR, relative_path) if relative_path else None
        if asset_type == "git":
            return run_command_git(["git", "clone", asset_info["url"], dest_path])
        elif asset_type == "local":
            # For local assets, just verify the path exists (already checked in _check_asset_exists)
            return True
        logger.warning("Unknown asset type '%s' for %s.", asset_type, dest_path)
        return False

model_manager = ModelManager()