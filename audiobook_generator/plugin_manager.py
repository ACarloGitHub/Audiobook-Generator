import json
import importlib
from typing import Dict, List, Optional
from .base_plugin import BaseTTSPlugin


class PluginManager:
    def __init__(self, registry_path: str = "audiobook_generator/plugins/plugin_registry.json"):
        self.plugins: Dict[str, BaseTTSPlugin] = {}
        self._load_plugins_from_registry(registry_path)

    def _load_plugins_from_registry(self, registry_path: str):
        try:
            with open(registry_path, 'r', encoding='utf-8') as f:
                registry = json.load(f)
            
            for plugin_info in registry:
                # ATTENZIONE: il campo "installed" nel JSON NON è fonte di verità.
                # Lo stato reale viene determinato dinamicamente tramite filesystem.
                # Qui carichiamo TUTTI i plugin definiti, poi la UI verifica se i file esistono.
                try:
                    module_path, class_name = plugin_info["entry_point"].rsplit(':', 1)
                    module = importlib.import_module(module_path)
                    plugin_class = getattr(module, class_name)
                    self.plugins[plugin_info["name"]] = plugin_class(
                        name=plugin_info["name"],
                        plugin_type=plugin_info["type"]
                    )
                    print(f"Plugin '{plugin_info['name']}' registrato con successo.")
                except Exception as e:
                    print(f"ERRORE: Impossibile caricare il plugin '{plugin_info['name']}': {e}")
        except FileNotFoundError:
            print(f"ATTENZIONE: File di registro plugin non trovato in '{registry_path}'.")
    
    def get_plugin(self, name: str) -> Optional[BaseTTSPlugin]:
        return self.plugins.get(name)

    def list_available_models(self) -> List[str]:
        return list(self.plugins.keys())

    def synthesize(self, model_name: str, text: str, output_path: str, model_instance: any, **kwargs) -> bool:
        """
        Metodo centralizzato per la sintesi. Trova il plugin corretto e delega la chiamata.
        """
        plugin = self.get_plugin(model_name)
        if not plugin:
            print(f"ERRORE: Plugin '{model_name}' non trovato per la sintesi.")
            return False
        
        # Passa tutti i kwargs al metodo synthesize del plugin
        return plugin.synthesize(model_instance, text, output_path, **kwargs)

    def load_model(self, model_name: str, **kwargs) -> any:
        """
        Metodo centralizzato per il caricamento del modello.
        """
        plugin = self.get_plugin(model_name)
        if not plugin:
            print(f"ERRORE: Plugin '{model_name}' non trovato per il caricamento.")
            return None
        return plugin.load_model(**kwargs)


# Istanza globale (opzionale, ma semplifica l'accesso)
plugin_manager = PluginManager()
