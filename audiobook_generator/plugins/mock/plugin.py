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

# audiobook_generator/plugins/mock/plugin.py
import logging
from audiobook_generator.base_plugin import BaseTTSPlugin

logger = logging.getLogger(__name__)


class MockPlugin(BaseTTSPlugin):
    def load_model(self, *args, **kwargs):
        return "mock_model"

    def synthesize(self, model, text, path, **kwargs):
        logger.info("Mock synthesis for '%s' in '%s'", text[:50], path)
        return True
