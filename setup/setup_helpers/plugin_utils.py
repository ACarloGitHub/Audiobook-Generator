# audiobook_generator/setup_helpers/plugin_utils.py
import json
import os
import sys
import threading

# Lock for thread-safe access to the JSON file
_registry_lock = threading.Lock()

def _get_registry_path():
    """Finds the correct path for plugin_registry.json."""
    # This path must be relative to the project root
    # We assume this script is in audiobook_generator/setup_helpers
    script_dir = os.path.dirname(__file__)
    project_root = os.path.abspath(os.path.join(script_dir, '..', '..'))
    registry_path = os.path.join(project_root, "audiobook_generator", "plugins", "plugin_registry.json")
    return registry_path if os.path.exists(registry_path) else None

def update_plugin_registry(plugin_name, installed=True):
    """
    Updates plugin_registry.json. The 'installed' field has been removed
    because installation status is determined dynamically
    via filesystem (see models_tab.py check_model_installed).
    This function retains the signature for compatibility but is essentially
    a no-op: it only verifies that the plugin exists in the registry.
    """
    with _registry_lock:
        registry_path = _get_registry_path()
        if not registry_path:
            print("ERROR: plugin_registry.json file not found.")
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
                    print(f"WARNING: Plugin '{plugin_name}' not found in registry.")
                
                if changed:
                    with open(registry_path, 'w', encoding='utf-8') as f_write:
                        json.dump(registry, f_write, indent=2, ensure_ascii=False)
                        f_write.write('\n')

            return True
        except (IOError, json.JSONDecodeError) as e:
            print(f"ERROR updating plugin registry: {e}")
            return False

def update_plugin_registry_with_lock(plugin_name, installed=True):
    """
    Updates plugin_registry.json with exclusive lock.
    The 'installed' field has been removed because status is determined
    dynamically via filesystem. This function retains the signature for
    compatibility but only removes any residual 'installed' fields.
    """
    registry_path = _get_registry_path()
    if not registry_path:
        print(f"ERROR: Cannot find plugin_registry.json")
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
            print(f"WARNING: Plugin '{plugin_name}' not found in registry.")
            return False
        
        if changed:
            with open(registry_path, 'w', encoding='utf-8') as f:
                json.dump(registry, f, indent=2, ensure_ascii=False)
                f.write('\n')
            print(f"Plugin registry cleaned: removed residual 'installed' field.")
        
        return True
    
    except Exception as e:
        print(f"ERROR updating plugin registry: {e}")
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