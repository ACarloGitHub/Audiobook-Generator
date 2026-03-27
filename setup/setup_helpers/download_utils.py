# audiobook_generator/setup_helpers/download_utils.py
import os
import requests
import zipfile
import tarfile
import time
import traceback
import sys

def download_file(url, dest_folder):
    """Scarica un file da un URL e lo salva in una cartella."""
    try:
        print(f"Scaricando da {url}...")
        response = requests.get(url, stream=True, timeout=30)
        response.raise_for_status()
        filename = os.path.join(dest_folder, url.split('/')[-1])
        with open(filename, 'wb') as f:
            for chunk in response.iter_content(chunk_size=8192):
                f.write(chunk)
        print(f"File salvato come {filename}")
        return filename
    except Exception as e:
        print(f"ERRORE durante il download da {url}: {e}")
        return None

def extract_archive(filepath, dest_folder):
    """Estrae un archivio .zip o .tar.gz in una cartella di destinazione."""
    try:
        print(f"Estraendo {filepath}...")
        if filepath.endswith('.zip'):
            with zipfile.ZipFile(filepath, 'r') as zip_ref:
                zip_ref.extractall(dest_folder)
        elif filepath.endswith('.tar.gz'):
            with tarfile.open(filepath, 'r:gz') as tar_ref:
                tar_ref.extractall(dest_folder)
        else:
            print(f"Formato archivio non supportato per {filepath}")
            return False
        print(f"Archivio estratto in {dest_folder}")
        os.remove(filepath)
        return True
    except Exception as e:
        print(f"ERRORE durante l'estrazione: {e}")
        return False

def detect_xet_repository(repo_id):
    """Rileva se un repository HuggingFace usa Xet (euristica)."""
    xet_orgs = ["microsoft/", "qwen/", "coqui/"]
    return any(repo_id.lower().startswith(org) for org in xet_orgs)

def download_with_huggingface_hub(repo_id, target_dir, retries=3, essential_files=None):
    """Scarica un modello da HuggingFace usando huggingface_hub."""
    print(f"Download tramite huggingface_hub: {repo_id} -> {target_dir}")
    
    for attempt in range(1, retries + 1):
        try:
            from huggingface_hub import snapshot_download
            from huggingface_hub.utils import RepositoryNotFoundError

            print(f"Tentativo {attempt}/{retries}...")
            os.makedirs(target_dir, exist_ok=True)
            
            env = os.environ.copy()
            if detect_xet_repository(repo_id):
                print("Rilevato potenziale repository Xet, disabilito l'integrazione Xet.")
                env["HF_HUB_DISABLE_XET"] = "1"

            snapshot_download(
                repo_id=repo_id,
                local_dir=target_dir,
                local_dir_use_symlinks=False,
                resume_download=True,
                token=os.getenv("HF_TOKEN"), # Usa token se disponibile
                repo_type="model",
                # `os.environ` non è un argomento valido, si usa `os.environ.copy()`
            )

            # Verifica file essenziali
            if essential_files:
                missing_files = [f for f in essential_files if not os.path.exists(os.path.join(target_dir, f))]
                if missing_files:
                    print(f"ATTENZIONE: Mancano file essenziali: {missing_files}")
                    if attempt < retries: continue
                    return False
            
            print(f"Download di {repo_id} completato con successo.")
            return True
            
        except ImportError:
            print("ERRORE: huggingface_hub non installato. Installa con: pip install huggingface-hub")
            return False
        except RepositoryNotFoundError as e:
            print(f"ERRORE: Repository non trovato: {repo_id}. Dettagli: {e}")
            return False
        except Exception as e:
            print(f"ERRORE durante il download (tentativo {attempt}/{retries}): {e}")
            traceback.print_exc()
            if attempt < retries:
                time.sleep(5)
            else:
                return False
    return False

def install_and_download_with_legacy_hf(repo_id, target_dir):
    """Installa huggingface_hub versione legacy (<0.26) e scarica il modello."""
    import subprocess
    print("Installazione huggingface_hub 0.25.0 (legacy, senza Xet)...")
    try:
        result = subprocess.run(
            ["pip", "install", "huggingface_hub==0.25.0", "--quiet"],
            capture_output=True, text=True, timeout=120
        )
        if result.returncode != 0:
            print(f"ERRORE installazione: {result.stderr}")
            return False
        print("Installazione completata.")
    except Exception as e:
        print(f"ERRORE durante installazione huggingface_hub: {e}")
        return False
    
    # Riprova il download
    return download_with_huggingface_hub(repo_id, target_dir)

def download_with_huggingface_hub_legacy(repo_id, target_dir, retries=3, essential_files=None):
    """Scarica un modello da HuggingFace usando versione legacy di huggingface_hub."""
    print(f"Download tramite huggingface_hub legacy (no Xet): {repo_id}")
    for attempt in range(1, retries + 1):
        try:
            try:
                from huggingface_hub import snapshot_download
                from huggingface_hub.utils import RepositoryNotFoundError, EntryNotFoundError
                import huggingface_hub
                from packaging import version as pkg_version
                hf_version = pkg_version.parse(huggingface_hub.__version__)
                target_version = pkg_version.parse("0.26.0")
                if hf_version < target_version:
                    print(f"✓ Versione huggingface_hub {huggingface_hub.__version__} < 0.26.0 (non supporta Xet)")
                else:
                    print(f"⚠ Versione huggingface_hub {huggingface_hub.__version__} >= 0.26.0 (potrebbe usare Xet)")
                    print("Provo comunque con HF_HUB_DISABLE_XET=1...")
                    os.environ["HF_HUB_DISABLE_XET"] = "1"
            except ImportError:
                print("huggingface_hub non installato, provo a installare versione 0.25.0...")
                return install_and_download_with_legacy_hf(repo_id, target_dir)
            
            print(f"Tentativo {attempt}/{retries}...")
            os.makedirs(target_dir, exist_ok=True)
            
            # FORZA il download nella cartella designata
            os.environ["HF_HOME"] = target_dir
            os.environ["HF_CACHE_HOME"] = target_dir
            os.environ["HF_MODULES_CACHE"] = target_dir
            os.environ["TRANSFORMERS_CACHE"] = target_dir
            
            downloaded_path = snapshot_download(
                repo_id=repo_id,
                local_dir=target_dir,
                local_dir_use_symlinks=False,
                resume_download=True,
                force_download=False,
                token=None,
                repo_type="model"
            )
            print(f"Download completato: {downloaded_path}")
            
            check_files = essential_files if essential_files else ["config.json", "preprocessor_config.json"]
            missing_files = [f for f in check_files if not os.path.exists(os.path.join(target_dir, f))]
            
            if missing_files:
                print(f"ATTENZIONE: Mancano file essenziali: {missing_files}")
                if attempt < retries:
                    print("Riprovo...")
                    time.sleep(2)
                    continue
                else:
                    print("ERRORE: Download incompleto dopo tutti i tentativi.")
                    return False
            
            print(f"Download di {repo_id} completato con successo.")
            return True
            
        except (RepositoryNotFoundError, EntryNotFoundError) as e:
            print(f"ERRORE: Repository non trovato: {repo_id}")
            print(f"Dettagli: {e}")
            return False
        except Exception as e:
            print(f"ERRORE durante il download legacy (tentativo {attempt}/{retries}): {e}")
            traceback.print_exc()
            if "xet" in str(e).lower() or "ImportError" in str(type(e).__name__):
                print("Rilevato errore Xet o ImportError, provo con versione legacy...")
                return install_and_download_with_legacy_hf(repo_id, target_dir)
            if attempt < retries:
                print(f"Attendo 5 secondi prima di riprovare...")
                time.sleep(5)
            else:
                print("ERRORE: Download fallito dopo tutti i tentativi.")
                return False
    return False