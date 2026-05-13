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

from abc import ABC, abstractmethod
from typing import Any, Dict


class BaseTTSPlugin(ABC):
    """Base interface for all TTS plugins."""
    
    def __init__(self, name: str, plugin_type: str):
        self.name = name
        self.type = plugin_type
        self.model_instance = None

    @abstractmethod
    def load_model(self, *args, **kwargs) -> Any:
        """Load the TTS model and return it."""
        pass

    @abstractmethod
    def synthesize(self, model_instance: Any, text: str, output_path: str, **kwargs) -> bool:
        """Synthesizes text into an audio file."""
        pass
