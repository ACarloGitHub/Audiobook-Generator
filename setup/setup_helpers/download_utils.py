# audiobook_generator/setup_helpers/download_utils.py
import os
import requests
import zipfile
import tarfile
import time
import traceback
import sys

def download_file(url, dest_folder):
    """Downloads a file from a URL and saves it to a folder."""
    try:
        print(f"Downloading from {url}...")
        response = requests.get(url, stream=True, timeout=30)
        response.raise_for_status()
        filename = os.path.join(dest_folder, url.split('/')[-1])
        with open(filename, 'wb') as f:
            for chunk in response.iter_content(chunk_size=8192):
                f.write(chunk)
        print(f"File saved as {filename}")
        return filename
    except Exception as e:
        print(f"ERROR downloading from {url}: {e}")
        return None

def extract_archive(filepath, dest_folder):
    """Extracts a .zip or .tar.gz archive to a destination folder."""
    try:
        print(f"Extracting {filepath}...")
        if filepath.endswith('.zip'):
            with zipfile.ZipFile(filepath, 'r') as zip_ref:
                zip_ref.extractall(dest_folder)
        elif filepath.endswith('.tar.gz'):
            with tarfile.open(filepath, 'r:gz') as tar_ref:
                tar_ref.extractall(dest_folder)
        else:
            print(f"Unsupported archive format for {filepath}")
            return False
        print(f"Archive extracted to {dest_folder}")
        os.remove(filepath)
        return True
    except Exception as e:
        print(f"ERROR during extraction: {e}")
        return False

def detect_xet_repository(repo_id):
    """Detects if a HuggingFace repository uses Xet (heuristic)."""
    xet_orgs = ["microsoft/", "qwen/", "coqui/"]
    return any(repo_id.lower().startswith(org) for org in xet_orgs)

def download_with_huggingface_hub(repo_id, target_dir, retries=3, essential_files=None):
    """Downloads a model from HuggingFace using huggingface_hub."""
    print(f"Downloading via huggingface_hub: {repo_id} -> {target_dir}")
    
    for attempt in range(1, retries + 1):
        try:
            from huggingface_hub import snapshot_download
            from huggingface_hub.utils import RepositoryNotFoundError

            print(f"Attempt {attempt}/{retries}...")
            os.makedirs(target_dir, exist_ok=True)
            
            env = os.environ.copy()
            if detect_xet_repository(repo_id):
                print("Detected potential Xet repository, disabling Xet integration.")
                env["HF_HUB_DISABLE_XET"] = "1"

            snapshot_download(
                repo_id=repo_id,
                local_dir=target_dir,
                local_dir_use_symlinks=False,
                resume_download=True,
                token=os.getenv("HF_TOKEN"), # Use token if available
                repo_type="model",
                # `os.environ` is not a valid argument, use `os.environ.copy()`
            )

            # Verify essential files
            if essential_files:
                missing_files = [f for f in essential_files if not os.path.exists(os.path.join(target_dir, f))]
                if missing_files:
                    print(f"WARNING: Missing essential files: {missing_files}")
                    if attempt < retries: continue
                    return False
            
            print(f"Download of {repo_id} completed successfully.")
            return True
            
        except ImportError:
            print("ERROR: huggingface_hub not installed. Install with: pip install huggingface-hub")
            return False
        except RepositoryNotFoundError as e:
            print(f"ERROR: Repository not found: {repo_id}. Details: {e}")
            return False
        except Exception as e:
            print(f"ERROR during download (attempt {attempt}/{retries}): {e}")
            traceback.print_exc()
            if attempt < retries:
                time.sleep(5)
            else:
                return False
    return False

def install_and_download_with_legacy_hf(repo_id, target_dir):
    """Installs huggingface_hub legacy version (<0.26) and downloads the model."""
    import subprocess
    print("Installing huggingface_hub 0.25.0 (legacy, without Xet)...")
    try:
        result = subprocess.run(
            ["pip", "install", "huggingface_hub==0.25.0", "--quiet"],
            capture_output=True, text=True, timeout=120
        )
        if result.returncode != 0:
            print(f"ERROR installing: {result.stderr}")
            return False
        print("Installation completed.")
    except Exception as e:
        print(f"ERROR installing huggingface_hub: {e}")
        return False
    
    # Retry the download
    return download_with_huggingface_hub(repo_id, target_dir)

def download_with_huggingface_hub_legacy(repo_id, target_dir, retries=3, essential_files=None):
    """Downloads a model from HuggingFace using legacy version of huggingface_hub."""
    print(f"Downloading via legacy huggingface_hub (no Xet): {repo_id}")
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
                    print(f"✓ huggingface_hub version {huggingface_hub.__version__} < 0.26.0 (does not support Xet)")
                else:
                    print(f"⚠ huggingface_hub version {huggingface_hub.__version__} >= 0.26.0 (may use Xet)")
                    print("Trying anyway with HF_HUB_DISABLE_XET=1...")
                    os.environ["HF_HUB_DISABLE_XET"] = "1"
            except ImportError:
                print("huggingface_hub not installed, trying to install version 0.25.0...")
                return install_and_download_with_legacy_hf(repo_id, target_dir)
            
            print(f"Attempt {attempt}/{retries}...")
            os.makedirs(target_dir, exist_ok=True)
            
            # FORCE download into the designated folder
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
            print(f"Download completed: {downloaded_path}")
            
            check_files = essential_files if essential_files else ["config.json", "preprocessor_config.json"]
            missing_files = [f for f in check_files if not os.path.exists(os.path.join(target_dir, f))]
            
            if missing_files:
                print(f"WARNING: Missing essential files: {missing_files}")
                if attempt < retries:
                    print("Retrying...")
                    time.sleep(2)
                    continue
                else:
                    print("ERROR: Incomplete download after all attempts.")
                    return False
            
            print(f"Download of {repo_id} completed successfully.")
            return True
            
        except (RepositoryNotFoundError, EntryNotFoundError) as e:
            print(f"ERROR: Repository not found: {repo_id}")
            print(f"Details: {e}")
            return False
        except Exception as e:
            print(f"ERROR during legacy download (attempt {attempt}/{retries}): {e}")
            traceback.print_exc()
            if "xet" in str(e).lower() or "ImportError" in str(type(e).__name__):
                print("Detected Xet error or ImportError, trying with legacy version...")
                return install_and_download_with_legacy_hf(repo_id, target_dir)
            if attempt < retries:
                print(f"Waiting 5 seconds before retrying...")
                time.sleep(5)
            else:
                print("ERROR: Download failed after all attempts.")
                return False
    return False