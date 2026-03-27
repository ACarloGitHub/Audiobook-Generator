# audiobook_generator/setup_helpers/plugin_utils.py
import json
import os
import sys
import threading

# Lock per l'accesso thread-safe al file JSON
_registry_lock = threading.Lock()

def _get_registry_path():
    """Trova il percorso corretto di plugin_registry.json."""
    # Questo percorso deve essere relativo alla root del progetto
    # Assumiamo che questo script sia in audiobook_generator/setup_helpers
    script_dir = os.path.dirname(__file__)
    project_root = os.path.abspath(os.path.join(script_dir, '..', '..'))
    registry_path = os.path.join(project_root, "audiobook_generator", "plugins", "plugin_registry.json")
    return registry_path if os.path.exists(registry_path) else None

def update_plugin_registry(plugin_name, installed=True):
    """Aggiorna plugin_registry.json in modo thread-safe."""
    with _registry_lock:
        registry_path = _get_registry_path()
        if not registry_path:
            print("ERRORE: File plugin_registry.json non trovato.")
            return False
        
        try:
            with open(registry_path, 'r+', encoding='utf-8') as f:
                registry = json.load(f)
                
                updated = False
                for plugin_info in registry:
                    if plugin_info.get("name") == plugin_name:
                        if plugin_info.get("installed") != installed:
                            plugin_info["installed"] = installed
                            updated = True
                            print(f"Registry: '{plugin_name}' impostato a installed={installed}.")
                        break
                
                if updated:
                    f.seek(0)
                    json.dump(registry, f, indent=2, ensure_ascii=False)
                    f.truncate()
            return True
        except (IOError, json.JSONDecodeError) as e:
            print(f"ERRORE durante l'aggiornamento del plugin registry: {e}")
            return False

def update_plugin_registry_with_lock(plugin_name, installed=True):
    """Aggiorna plugin_registry.json con lock esclusivo per evitare corruzione (usa file lock)."""
    registry_path = _get_registry_path()
    if not registry_path:
        print(f"ERRORE: Non trovo plugin_registry.json")
        return False
    
    lock_file = registry_path + ".lock"
    lock_fd = None
    
    try:
        if sys.platform == "win32":
            import msvcrt
            lock_fd = open(lock_file, 'w')
            msvcrt.locking(lock_fd.fileno(), msvcrt.LK_LOCK, 1)
        else:
            import fcntl
            lock_fd = open(lock_file, 'w')
            fcntl.flock(lock_fd.fileno(), fcntl.LOCK_EX)
        
        with open(registry_path, 'r', encoding='utf-8') as f:
            registry = json.load(f)
        
        updated = False
        for plugin_info in registry:
            if plugin_info["name"] == plugin_name:
                plugin_info["installed"] = installed
                updated = True
                break
        
        if not updated:
            print(f"ATTENZIONE: Plugin '{plugin_name}' non trovato nel registry.")
            return False
        
        with open(registry_path, 'w', encoding='utf-8') as f:
            json.dump(registry, f, indent=2, ensure_ascii=False)
        
        print(f"Plugin registry aggiornato per '{plugin_name}' (installed={installed}).")
        return True
    
    except Exception as e:
        print(f"ERRORE durante l'aggiornamento del plugin registry: {e}")
        return False
    
    finally:
        if lock_fd:
            if sys.platform == "win32":
                import msvcrt
                try:
                    msvcrt.locking(lock_fd.fileno(), msvcrt.LK_UNLCK, 1)
                    lock_fd.close()
                except:
                    pass
            else:
                import fcntl
                try:
                    fcntl.flock(lock_fd.fileno(), fcntl.LOCK_UN)
                    lock_fd.close()
                except:
                    pass
            try:
                os.remove(lock_file)
            except:
                pass