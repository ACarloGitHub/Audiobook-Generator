from abc import ABC, abstractmethod
from typing import Any, Dict


class BaseTTSPlugin(ABC):
    """Interfaccia base per tutti i plugin TTS."""
    
    def __init__(self, name: str, plugin_type: str):
        self.name = name
        self.type = plugin_type
        self.model_instance = None

    @abstractmethod
    def load_model(self, *args, **kwargs) -> Any:
        """Carica il modello TTS e lo restituisce."""
        pass

    @abstractmethod
    def synthesize(self, model_instance: Any, text: str, output_path: str, **kwargs) -> bool:
        """Sintetizza il testo in un file audio."""
        pass
