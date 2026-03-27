import os
import subprocess
from . import config

def run_command_git(command):
    """Esegue un comando git e gestisce l'output."""
    try:
        print(f"--- Eseguendo: {' '.join(command)} ---")
        subprocess.run(command, check=True, capture_output=True, text=True, encoding='utf-8')
        return True
    except FileNotFoundError:
        print("ERRORE: 'git' non è installato o non è nel PATH.")
        return False
    except subprocess.CalledProcessError as e:
        print(f"ERRORE durante l'esecuzione di git:\n{e.stderr}")
        return False

class ModelManager:
    def ensure_assets(self, model_name: str) -> bool:
        if model_name not in config.MODEL_ASSETS:
            return True 

        assets = config.MODEL_ASSETS[model_name]
        for asset in assets:
            # Supporta sia 'dest' che 'path' per compatibilità
            relative_path = asset.get("dest") or asset.get("path")
            # Risolvi il percorso rispetto alla directory del progetto
            dest_path = os.path.join(config.BASE_PROJECT_DIR, relative_path) if relative_path else None
            
            # Verifica se l'asset esiste già
            if self._check_asset_exists(asset):
                print(f"INFO: Asset per '{model_name}' trovato in locale: {dest_path}")
                continue
            
            # Se non esiste e il tipo è "local", errore
            if asset.get("type") == "local":
                print(f"ERRORE: Asset locale mancante per '{model_name}': {dest_path}")
                return False
            
            print(f"ATTENZIONE: Asset mancante in '{dest_path}'. Tentativo di download...")
            if not self._download_asset(asset):
                print(f"ERRORE: Download fallito per l'asset da {asset.get('url', 'N/A')}.")
                return False
        
        print(f"Tutti gli asset per '{model_name}' sono pronti.")
        return True

    def _check_asset_exists(self, asset_info: dict) -> bool:
        relative_path = asset_info.get("dest") or asset_info.get("path")
        dest_path = os.path.join(config.BASE_PROJECT_DIR, relative_path) if relative_path else None
        if not dest_path or not os.path.exists(dest_path):
            return False
        
        # Per modelli VibeVoice, verifica file essenziali
        if "VibeVoice" in dest_path:
            essential_files = ["config.json", "preprocessor_config.json"]
            for file in essential_files:
                if not os.path.exists(os.path.join(dest_path, file)):
                    return False
            # Verifica che esista almeno model.safetensors o model.safetensors.index.json
            model_file = os.path.join(dest_path, "model.safetensors")
            model_index = os.path.join(dest_path, "model.safetensors.index.json")
            if not os.path.exists(model_file) and not os.path.exists(model_index):
                return False
        
        return True

    def _download_asset(self, asset_info: dict) -> bool:
        asset_type = asset_info.get("type", "git")
        relative_path = asset_info.get("dest") or asset_info.get("path")
        dest_path = os.path.join(config.BASE_PROJECT_DIR, relative_path) if relative_path else None
        if asset_type == "git":
            return run_command_git(["git", "clone", asset_info["url"], dest_path])
        elif asset_type == "local":
            # Per asset locali, basta che il percorso esista (già verificato in _check_asset_exists)
            return True
        return False

model_manager = ModelManager()