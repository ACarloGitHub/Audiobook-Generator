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
