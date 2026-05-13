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
    """
    Aggiorna plugin_registry.json. Il campo 'installed' è stato rimosso
    perché lo stato di installazione viene determinato dinamicamente
    tramite filesystem (vedi models_tab.py check_model_installed).
    Questa funzione mantiene la firma per compatibilità ma è sostanzialmente
    un no-op: verifica solo che il plugin sia presente nel registry.
    """
    with _registry_lock:
        registry_path = _get_registry_path()
        if not registry_path:
            print("ERRORE: File plugin_registry.json non trovato.")
            return False
        
        try:
            with open(registry_path, 'r', encoding='utf-8') as f:
                registry = json.load(f)
                
                # Remove stale "installed" field if present
                changed = False
                for plugin_info in registry:
                    if "installed" in plugin_info:
                        del plugin_info["installed"]
                        changed = True

                # Verify plugin exists in registry
                found = any(plugin_info.get("name") == plugin_name for plugin_info in registry)
                if not found:
                    print(f"ATTENZIONE: Plugin '{plugin_name}' non trovato nel registry.")
                
                if changed:
                    with open(registry_path, 'w', encoding='utf-8') as f_write:
                        json.dump(registry, f_write, indent=2, ensure_ascii=False)
                        f_write.write('\n')

            return True
        except (IOError, json.JSONDecodeError) as e:
            print(f"ERRORE durante l'aggiornamento del plugin registry: {e}")
            return False

def update_plugin_registry_with_lock(plugin_name, installed=True):
    """
    Aggiorna plugin_registry.json con lock esclusivo.
    Il campo 'installed' è stato rimosso perché lo stato viene determinato
    dinamicamente tramite filesystem. Questa funzione mantiene la firma per
    compatibilità ma rimuove solo eventuali campi 'installed' residui.
    """
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
        
        # Remove stale "installed" field and verify plugin exists
        changed = False
        found = False
        for plugin_info in registry:
            if "installed" in plugin_info:
                del plugin_info["installed"]
                changed = True
            if plugin_info.get("name") == plugin_name:
                found = True
        
        if not found:
            print(f"ATTENZIONE: Plugin '{plugin_name}' non trovato nel registry.")
            return False
        
        if changed:
            with open(registry_path, 'w', encoding='utf-8') as f:
                json.dump(registry, f, indent=2, ensure_ascii=False)
                f.write('\n')
            print(f"Plugin registry pulito: rimosso campo 'installed' residuo.")
        
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