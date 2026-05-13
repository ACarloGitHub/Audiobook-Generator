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
