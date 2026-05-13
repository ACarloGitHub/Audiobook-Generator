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

"""
Audiobook Generator configuration package.

This package centralizes all configuration constants. It is split into
three modules for clarity:

- config.paths: Project directories and executable paths
- config.defaults: Default settings, timeouts, and chunking parameters
- config.models: TTS model data, voice definitions, and asset paths

All constants are re-exported here for backward compatibility.
Existing imports like ``from audiobook_generator import config`` or
``from audiobook_generator.config import BASE_PROJECT_DIR`` continue to work.
"""

from .paths import *
from .defaults import *
from .models import *