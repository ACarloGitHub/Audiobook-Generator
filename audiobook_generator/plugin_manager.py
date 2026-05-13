import json
import importlib
import logging
from typing import Dict, List, Optional, Any
from .base_plugin import BaseTTSPlugin

logger = logging.getLogger(__name__)


class PluginManager:
    def __init__(self, registry_path: str = "audiobook_generator/plugins/plugin_registry.json"):
        self.plugins: Dict[str, BaseTTSPlugin] = {}
        self._load_plugins_from_registry(registry_path)

    def _load_plugins_from_registry(self, registry_path: str):
        try:
            with open(registry_path, 'r', encoding='utf-8') as f:
                registry = json.load(f)
        except FileNotFoundError:
            logger.warning("Plugin registry file not found at '%s'.", registry_path)
            return
        except json.JSONDecodeError as e:
            logger.error("Plugin registry file '%s' contains invalid JSON: %s", registry_path, e)
            return
            
        for plugin_info in registry:
            try:
                module_path, class_name = plugin_info["entry_point"].rsplit(':', 1)
                module = importlib.import_module(module_path)
                plugin_class = getattr(module, class_name)
                self.plugins[plugin_info["name"]] = plugin_class(
                    name=plugin_info["name"],
                    plugin_type=plugin_info["type"]
                )
                logger.info("Plugin '%s' registered successfully.", plugin_info['name'])
            except Exception as e:
                logger.error("Failed to load plugin '%s' (entry_point=%s): %s", plugin_info.get('name', '?'), plugin_info.get('entry_point', '?'), e)
    
    def get_plugin(self, name: str) -> Optional[BaseTTSPlugin]:
        return self.plugins.get(name)

    def list_available_models(self) -> List[str]:
        return list(self.plugins.keys())

    def synthesize(self, model_name: str, text: str, output_path: str, model_instance: Any, **kwargs) -> bool:
        """Centralized synthesis method. Finds the correct plugin and delegates the call."""
        plugin = self.get_plugin(model_name)
        if not plugin:
            logger.error("Plugin '%s' not found for synthesis.", model_name)
            return False
        
        return plugin.synthesize(model_instance, text, output_path, **kwargs)

    def load_model(self, model_name: str, **kwargs) -> Any:
        """Centralized model loading method."""
        plugin = self.get_plugin(model_name)
        if not plugin:
            logger.error("Plugin '%s' not found for model loading.", model_name)
            return None
        return plugin.load_model(**kwargs)


# Global instance (optional, but simplifies access)
plugin_manager = PluginManager()
