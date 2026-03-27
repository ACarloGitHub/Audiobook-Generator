# audiobook_generator/plugins/mock/plugin.py
from audiobook_generator.base_plugin import BaseTTSPlugin


class MockPlugin(BaseTTSPlugin):
    def load_model(self, *args, **kwargs):
        return "mock_model"

    def synthesize(self, model, text, path, **kwargs):
        print(f"Sintesi mock per '{text}' in '{path}'")
        return True
