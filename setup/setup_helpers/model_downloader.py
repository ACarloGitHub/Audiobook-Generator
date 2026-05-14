# audiobook_generator/setup_helpers/model_downloader.py
import os
import shutil

from .system_utils import command_exists, run_command, clone_repo
from .download_utils import download_with_huggingface_hub, download_file, extract_archive
from .plugin_utils import update_plugin_registry, update_plugin_registry_with_lock

# Absolute path to project root — derived from this file's location.
# model_downloader.py is at: <project_root>/setup/setup_helpers/model_downloader.py
# so project root is 3 levels up.
_PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

def _proj(*parts):
    """Return an absolute path under the project root, regardless of cwd."""
    return os.path.join(_PROJECT_ROOT, *parts)

def download_kokoro_model(idle_timeout=1800):
    """Downloads the Kokoro-82M model from HuggingFace."""
    repo_id = "hexgrad/Kokoro-82M"
    target_dir = _proj("audiobook_generator", "tts_models", "kokoro", "models")

    if os.path.exists(os.path.join(target_dir, "kokoro-v1_0.pth")):
        print("Kokoro model already present. Download skipped.")
        update_plugin_registry("Kokoro", installed=True)
        return True

    print("Downloading Kokoro model...")
    if download_with_huggingface_hub(repo_id, target_dir, essential_files=["config.json", "kokoro-v1_0.pth"]):
        update_plugin_registry("Kokoro", installed=True)
        return True
    else:
        print("ERROR: Kokoro download failed.")
        return False

def download_xttsv2_model(idle_timeout=1800):
    """Downloads the XTTS-v2 model from HuggingFace."""
    repo_id = "coqui/XTTS-v2"
    target_dir = _proj("audiobook_generator", "tts_models", "xttsv2")

    if os.path.exists(os.path.join(target_dir, "model.pth")):
        print("XTTSv2 model already present. Download skipped.")
        update_plugin_registry("XTTSv2", installed=True)
        return True
        
    print("Downloading XTTSv2 model...")
    if download_with_huggingface_hub(repo_id, target_dir, essential_files=["config.json", "model.pth", "dvae.pth"]):
        update_plugin_registry("XTTSv2", installed=True)
        return True
    else:
        print("ERROR: XTTSv2 download failed.")
        return False

def _check_safetensors_shards(model_dir):
    """Checks that all safetensors shard files referenced in model.safetensors.index.json exist.
    
    Returns a list of missing shard filenames, or an empty list if all are present.
    If model.safetensors.index.json does not exist, returns an empty list (no shards to check).
    """
    import json as _json
    index_path = os.path.join(model_dir, "model.safetensors.index.json")
    if not os.path.exists(index_path):
        # Single-file model (e.g. 0.5B) or no index — nothing to check
        return []
    try:
        with open(index_path, 'r', encoding='utf-8') as f:
            index_data = _json.load(f)
        metadata = index_data.get("metadata", {})
        total_size = metadata.get("total_size", 0)
        weight_map = index_data.get("weight_map", {})
        # Collect unique shard filenames from the weight_map
        shard_files = set()
        for tensor_name, filename in weight_map.items():
            shard_files.add(filename)
        missing = []
        for shard_file in sorted(shard_files):
            if not os.path.exists(os.path.join(model_dir, shard_file)):
                missing.append(shard_file)
        return missing
    except Exception as e:
        print(f"Warning: could not parse model.safetensors.index.json: {e}")
        return []


def download_vibevoice_tokenizer(idle_timeout=300):
    """Downloads the Qwen2.5-1.5B tokenizer for VibeVoice 1.5B and 7B.
    
    The Qwen tokenizer is NOT included in the VibeVoice models on HuggingFace.
    It must be downloaded separately from Qwen/Qwen2.5-1.5B.
    
    The tokenizer is saved in tts_models/vibevoice/tokenizer/
    with files: tokenizer.json, tokenizer_config.json, merges.txt, vocab.json
    
    The synthesis code loads the tokenizer from the local directory:
        tokenizer = VibeVoiceTextTokenizerFast.from_pretrained(vibevoice_tokenizer_dir, ...)
    
    Additionally, since the VibeVoice 7B preprocessor_config.json references
    "Qwen/Qwen2.5-7B" as language_model_pretrained_name, and Qwen2.5-1.5B and
    Qwen2.5-7B share the same tokenizer, we create a HuggingFace cache entry
    for Qwen2.5-7B by copying the Qwen2.5-1.5B cache structure.
    
    Uses a lock file to prevent race conditions when two downloads start
    concurrently (e.g. "Download All VibeVoice").
    """
    import time
    
    tokenizer_dir = _proj("audiobook_generator", "tts_models", "vibevoice", "tokenizer")
    
    # Required files for the Qwen tokenizer
    tokenizer_files = ["tokenizer.json", "tokenizer_config.json", "merges.txt", "vocab.json"]
    
    # Determine HF cache directory
    try:
        from huggingface_hub import constants
        hf_cache = constants.HUGGINGFACE_HUB_CACHE
    except ImportError:
        hf_cache = os.path.expanduser("~/.cache/huggingface/hub")
    qwen_7b_cache = os.path.join(hf_cache, "models--Qwen--Qwen2.5-7B")
    lock_file = qwen_7b_cache + ".lock"
    
    # ============================================================
    # LOCK: prevent race condition if two downloads start concurrently
    # (e.g. "Download All VibeVoice" triggers both 1.5B and 7B)
    # ============================================================
    waited = 0
    wait_interval = 1.0
    while True:
        try:
            # O_EXCL makes creation atomic: fails if file already exists
            lock_fd = os.open(lock_file, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
            os.write(lock_fd, str(os.getpid()).encode())
            os.close(lock_fd)
            break  # Lock acquired
        except FileExistsError:
            # Lock already exists — wait and retry
            if waited >= idle_timeout:
                print(f"TIMEOUT: lock file still present after {idle_timeout}s. Likely crash of a previous process.")
                return False
            time.sleep(wait_interval)
            waited += wait_interval
            wait_interval = min(wait_interval * 1.5, 10)  # backoff max 10s
    
    try:
        # Verify ONLY AFTER acquiring the lock (no other process is writing)
        tokenizer_present = all(os.path.exists(os.path.join(tokenizer_dir, f)) for f in tokenizer_files)
        cache_7b_present = os.path.exists(qwen_7b_cache)
        
        if tokenizer_present and cache_7b_present:
            print("Qwen2.5-1.5B tokenizer already present and Qwen2.5-7B cache exists. Nothing to do.")
            return True
        
        # If local tokenizer is missing, download it
        if not tokenizer_present:
            print("Downloading Qwen2.5-1.5B tokenizer...")
            try:
                from huggingface_hub import hf_hub_download
            except ImportError:
                print("ERROR: huggingface_hub not available.")
                return False
            
            os.makedirs(tokenizer_dir, exist_ok=True)
            
            repo_id = "Qwen/Qwen2.5-1.5B"
            for filename in tokenizer_files:
                try:
                    local_path = hf_hub_download(repo_id, filename)
                    dest_path = os.path.join(tokenizer_dir, filename)
                    if os.path.abspath(local_path) != os.path.abspath(dest_path):
                        shutil.copy2(local_path, dest_path)
                    print(f"  {filename} -> {dest_path}")
                except Exception as e:
                    print(f"ERROR downloading {filename}: {e}")
                    return False
            print("Qwen2.5-1.5B tokenizer downloaded successfully.")
        
        # ============================================================
        # FIX: The VibeVoice code for 7B looks for "Qwen/Qwen2.5-7B" in the cache.
        # Since Qwen2.5-1.5B and Qwen2.5-7B use the SAME tokenizer,
        # we create a cache entry for Qwen2.5-7B by copying the entire cache structure.
        # ============================================================
        if not cache_7b_present:
            qwen_1_5b_cache = os.path.join(hf_cache, "models--Qwen--Qwen2.5-1.5B")
            if os.path.exists(qwen_1_5b_cache):
                print("Creating cache for Qwen2.5-7B (same tokenizer as Qwen2.5-1.5B)...")
                try:
                    if os.path.exists(qwen_7b_cache):
                        shutil.rmtree(qwen_7b_cache)
                    shutil.copytree(qwen_1_5b_cache, qwen_7b_cache)
                    print("  Qwen2.5-7B cache created successfully (full structure).")
                except Exception as e:
                    print(f"  Error during cache copy: {e}")
            else:
                print("WARNING: Qwen2.5-1.5B cache not found, cannot create 7B cache.")
        
        return True
    finally:
        # Release the lock
        if os.path.exists(lock_file):
            os.remove(lock_file)


def download_vibevoice_model_multiple(version_choice, idle_timeout=1800):
    """Downloads the VibeVoice model based on the choice.
    
    Verified HuggingFace URLs (03/21/2026):
    - 1.5B:   microsoft/VibeVoice-1.5B
    - 7B:     vibevoice/VibeVoice-7B
    - Realtime-0.5B: microsoft/VibeVoice-Realtime-0.5B
    
    Non-existent URLs (removed):
    - vibevoice/VibeVoice-1.5B-full     → 404
    - vibevoice/VibeVoice-7B-low-vram   → 404
    
    Note: The 1.5B and 7B models also require the Qwen2.5-1.5B tokenizer
    (downloaded separately by download_vibevoice_tokenizer()).
    """
    model_dir = _proj("audiobook_generator", "tts_models", "vibevoice")
    # Version -> (HuggingFace repo, local folder) mapping
    # Struttura: {1.5B, 7B, 0.5B}/
    repo_map = {
        "1.5B": ("vibevoice/VibeVoice-1.5B", "1.5B"),
        "7B": ("vibevoice/VibeVoice-7B", "7B"),
        "Realtime-0.5B": ("microsoft/VibeVoice-Realtime-0.5B", "0.5B"),
    }
    plugin_name_map = {
        "1.5B": "VibeVoice-1.5B",
        "7B": "VibeVoice-7B",
        "Realtime-0.5B": "VibeVoice-Realtime-0.5B",
    }
    repo_info = repo_map.get(version_choice)
    plugin_name = plugin_name_map.get(version_choice)
    if not repo_info or not plugin_name:
        print(f"ERROR: Unsupported version: {version_choice}")
        return False
    repo, folder = repo_info
    # Struttura: {1.5B,7B,0.5B}/
    target_dir = os.path.join(model_dir, folder)
    print(f"Downloading VibeVoice-{version_choice} model...")
    print(f"Target: {target_dir}")
    if os.path.exists(target_dir):
        # Essential config files (present in all versions)
        essential_files = ["config.json", "preprocessor_config.json"]
        # Sharded models (1.5B, 7B) have index file; single-file models (0.5B) have model.safetensors
        if os.path.exists(os.path.join(target_dir, "model.safetensors.index.json")):
            essential_files.append("model.safetensors.index.json")
        elif os.path.exists(os.path.join(target_dir, "model.safetensors")):
            essential_files.append("model.safetensors")
        missing_files = []
        for file in essential_files:
            file_path = os.path.join(target_dir, file)
            if not os.path.exists(file_path):
                missing_files.append(file)
        # Also check that all safetensors shard files are present
        missing_shards = _check_safetensors_shards(target_dir)
        if missing_shards:
            missing_files.extend(missing_shards)
        if not missing_files:
            print(f"VibeVoice-{version_choice} model already present and complete in '{target_dir}'. Download skipped.")
            update_plugin_registry_with_lock(plugin_name, installed=True)
            # Download the correct source code based on the model
            if version_choice == "Realtime-0.5B":
                download_vibevoice_repo_microsoft()
                download_vibevoice_vvembed()  # Required for streaming classes
                download_vibevoice_realtime_voices()  # Downloads voice embeddings
            else:
                download_vibevoice_vvembed()
                download_vibevoice_tokenizer()  # Qwen2.5-1.5B tokenizer for 1.5B and 7B
            return True
        else:
            print(f"VibeVoice-{version_choice} model exists but {len(missing_files)} files are missing: {missing_files}")
            print("Proceeding with download to complete the model...")
    else:
        print(f"VibeVoice-{version_choice} model not present in '{target_dir}'. Proceeding with download...")
    print("Attempt 1: Download via huggingface_hub (Python)...")
    success = download_with_huggingface_hub(repo, target_dir)
    if not success and command_exists("hf"):
        print("Attempt 2: Download via hf CLI...")
        cmd = ["hf", "download", repo, "--local-dir", target_dir]
        env = os.environ.copy()
        env["PYTHONIOENCODING"] = "utf-8"
        success = run_command(cmd, idle_timeout=idle_timeout)
    if not success and command_exists("huggingface-cli"):
        print("Attempt 3: Download via huggingface-cli (deprecated)...")
        cmd = ["huggingface-cli", "download", repo, "--local-dir", target_dir]
        env = os.environ.copy()
        env["PYTHONIOENCODING"] = "utf-8"
        success = run_command(cmd, idle_timeout=idle_timeout)
    if success:
        update_plugin_registry_with_lock(plugin_name, installed=True)
        # Download the correct source code based on the model
        if version_choice == "Realtime-0.5B":
            download_vibevoice_repo_microsoft()
            download_vibevoice_vvembed()  # Required for streaming classes
            download_vibevoice_realtime_voices()  # Downloads voice embeddings
        else:
            download_vibevoice_vvembed()
            download_vibevoice_tokenizer()  # Qwen2.5-1.5B tokenizer for 1.5B and 7B
    else:
        print(f"ERROR: All download methods for VibeVoice-{version_choice} failed.")
    return success


def download_vibevoice_vvembed():
    """Downloads the VibeVoice source code (repo_community) required for 1.5B/7B models.
    
    Structure (matches Test_TTS_Autonomi):
    - repo_community/ for community models (1.5B, 7B)
    - repo/ for Microsoft Realtime model
    """
    import requests
    import zipfile
    repo_community_dir = _proj("audiobook_generator", "tts_models", "vibevoice", "repo_community")
    repo_url = "https://github.com/vibevoice-community/VibeVoice"
    temp_dir = _proj("audiobook_generator", "tts_models", "vibevoice", "repo_community_temp")
    
    if os.path.exists(repo_community_dir):
        essential_files = [
            "vibevoice/modular/modeling_vibevoice_inference.py",
            "vibevoice/processor/vibevoice_processor.py",
            "vibevoice/schedule/__init__.py"
        ]
        missing_files = []
        for file in essential_files:
            file_path = os.path.join(repo_community_dir, file)
            if not os.path.exists(file_path):
                missing_files.append(file)
        if not missing_files:
            print(f"repo_community folder already present and complete in '{repo_community_dir}'. Download skipped.")
            return True
        else:
            print(f"repo_community folder exists but {len(missing_files)} essential files are missing: {missing_files}")
            print("Proceeding with download to complete...")
    else:
        print(f"repo_community folder not present in '{repo_community_dir}'. Proceeding with download...")
    
    # Attempt 1: Git clone (optional, not required)
    print(f"Attempting to clone repository {repo_url}...")
    git_success = False
    if command_exists("git"):
        git_success = clone_repo(repo_url, temp_dir)
    
    if git_success:
        print("Git clone completed. Copying required folders...")
        vibevoice_subdir = os.path.join(temp_dir, "vibevoice")
        if os.path.exists(vibevoice_subdir) and os.path.isdir(vibevoice_subdir):
            print(f"Found 'vibevoice' subdirectory in cloned repository")
            if os.path.exists(repo_community_dir):
                shutil.rmtree(repo_community_dir, ignore_errors=True)
            os.makedirs(os.path.dirname(repo_community_dir), exist_ok=True)
            shutil.copytree(vibevoice_subdir, os.path.join(repo_community_dir, "vibevoice"))
            print(f"  Copied vibevoice/ -> {os.path.join(repo_community_dir, 'vibevoice')}")
        else:
            print(f"WARNING: 'vibevoice' subdirectory not found, using root")
            if os.path.exists(repo_community_dir):
                shutil.rmtree(repo_community_dir, ignore_errors=True)
            os.makedirs(os.path.dirname(repo_community_dir), exist_ok=True)
            shutil.copytree(temp_dir, repo_community_dir, dirs_exist_ok=True)
        
        shutil.rmtree(temp_dir, ignore_errors=True)
        return True
    else:
        print("Git clone failed or git not available. Trying ZIP download...")
    
    # Attempt 2: ZIP download (primary fallback)
    print("Attempting ZIP download...")
    zip_url = "https://github.com/vibevoice-community/VibeVoice/archive/refs/heads/main.zip"
    zip_temp_dir = temp_dir + "_zip"
    
    try:
        if os.path.exists(zip_temp_dir):
            shutil.rmtree(zip_temp_dir, ignore_errors=True)
        os.makedirs(zip_temp_dir, exist_ok=True)
        
        zip_path = os.path.join(zip_temp_dir, "vibevoice.zip")
        print(f"Downloading ZIP from {zip_url}...")
        response = requests.get(zip_url, stream=True)
        response.raise_for_status()
        
        with open(zip_path, 'wb') as f:
            for chunk in response.iter_content(chunk_size=8192):
                f.write(chunk)
        
        print(f"Extracting ZIP...")
        with zipfile.ZipFile(zip_path, 'r') as zip_ref:
            zip_ref.extractall(zip_temp_dir)
        
        root_items = [d for d in os.listdir(zip_temp_dir) if os.path.isdir(os.path.join(zip_temp_dir, d))]
        if len(root_items) == 1:
            actual_root = os.path.join(zip_temp_dir, root_items[0])
            print(f"Found root folder: {root_items[0]}")
        else:
            actual_root = zip_temp_dir
        
        vibevoice_subdir = os.path.join(actual_root, "vibevoice")
        if os.path.exists(vibevoice_subdir) and os.path.isdir(vibevoice_subdir):
            if os.path.exists(repo_community_dir):
                shutil.rmtree(repo_community_dir, ignore_errors=True)
            os.makedirs(os.path.dirname(repo_community_dir), exist_ok=True)
            shutil.copytree(vibevoice_subdir, os.path.join(repo_community_dir, "vibevoice"))
            print(f"  Copied vibevoice/ -> {os.path.join(repo_community_dir, 'vibevoice')}")
        else:
            if os.path.exists(repo_community_dir):
                shutil.rmtree(repo_community_dir, ignore_errors=True)
            os.makedirs(os.path.dirname(repo_community_dir), exist_ok=True)
            shutil.copytree(actual_root, repo_community_dir, dirs_exist_ok=True)
        
        shutil.rmtree(zip_temp_dir, ignore_errors=True)
        return True
            
    except Exception as e:
        print(f"ERROR during ZIP download: {e}")
        if os.path.exists(zip_temp_dir):
            shutil.rmtree(zip_temp_dir, ignore_errors=True)
        return False


def download_vibevoice_repo_microsoft():
    """Downloads the Microsoft source code for VibeVoice Realtime.
    
    Structure (matches Test_TTS_Autonomi):
    - repo/ for Microsoft Realtime model
    """
    import requests
    import zipfile
    repo_dir = _proj("audiobook_generator", "tts_models", "vibevoice", "repo")
    repo_url = "https://github.com/microsoft/VibeVoice.git"
    temp_dir = _proj("audiobook_generator", "tts_models", "vibevoice", "repo_temp")
    
    if os.path.exists(repo_dir):
        essential_files = [
            "vibevoice/modular/modeling_vibevoice_inference.py",
            "vibevoice/processor/vibevoice_processor.py",
        ]
        missing_files = []
        for file in essential_files:
            file_path = os.path.join(repo_dir, file)
            if not os.path.exists(file_path):
                missing_files.append(file)
        if not missing_files:
            print(f"repo folder already present and complete in '{repo_dir}'. Download skipped.")
            return True
        else:
            print(f"repo folder exists but {len(missing_files)} essential files are missing: {missing_files}")
            print("Proceeding with download to complete...")
    else:
        print(f"repo folder not present in '{repo_dir}'. Proceeding with download...")
    
    print(f"Attempting to clone Microsoft repository...")
    git_success = False
    if command_exists("git"):
        git_success = clone_repo(repo_url, temp_dir)
    
    if git_success:
        print("Git clone completed. Copying required folders...")
        vibevoice_subdir = os.path.join(temp_dir, "vibevoice")
        demo_subdir = os.path.join(temp_dir, "demo")
        
        if os.path.exists(vibevoice_subdir) and os.path.isdir(vibevoice_subdir):
            if os.path.exists(repo_dir):
                shutil.rmtree(repo_dir, ignore_errors=True)
            os.makedirs(os.path.dirname(repo_dir), exist_ok=True)
            shutil.copytree(vibevoice_subdir, os.path.join(repo_dir, "vibevoice"))
            print(f"  Copied vibevoice/ -> {os.path.join(repo_dir, 'vibevoice')}")
            
            if os.path.exists(demo_subdir) and os.path.isdir(demo_subdir):
                demo_target = os.path.join(os.path.dirname(repo_dir), "repo", "demo")
                shutil.copytree(demo_subdir, demo_target, dirs_exist_ok=True)
                print(f"  Copied demo/ -> {demo_target}")
        else:
            if os.path.exists(repo_dir):
                shutil.rmtree(repo_dir, ignore_errors=True)
            os.makedirs(os.path.dirname(repo_dir), exist_ok=True)
            shutil.copytree(temp_dir, repo_dir, dirs_exist_ok=True)
        
        shutil.rmtree(temp_dir, ignore_errors=True)
        return True
    else:
        print("Git clone failed. Trying ZIP download...")
    
    zip_url = "https://github.com/microsoft/VibeVoice/archive/refs/heads/main.zip"
    zip_temp_dir = temp_dir + "_zip"
    
    try:
        if os.path.exists(zip_temp_dir):
            shutil.rmtree(zip_temp_dir, ignore_errors=True)
        os.makedirs(zip_temp_dir, exist_ok=True)
        
        zip_path = os.path.join(zip_temp_dir, "repo.zip")
        print(f"Downloading ZIP from {zip_url}...")
        response = requests.get(zip_url, stream=True)
        response.raise_for_status()
        
        with open(zip_path, 'wb') as f:
            for chunk in response.iter_content(chunk_size=8192):
                f.write(chunk)
        
        print("Extracting ZIP...")
        with zipfile.ZipFile(zip_path, 'r') as zip_ref:
            zip_ref.extractall(zip_temp_dir)
        
        root_items = [d for d in os.listdir(zip_temp_dir) if os.path.isdir(os.path.join(zip_temp_dir, d))]
        if len(root_items) == 1:
            actual_root = os.path.join(zip_temp_dir, root_items[0])
        else:
            actual_root = zip_temp_dir
        
        vibevoice_subdir = os.path.join(actual_root, "vibevoice")
        demo_subdir = os.path.join(actual_root, "demo")
        if os.path.exists(vibevoice_subdir):
            if os.path.exists(repo_dir):
                shutil.rmtree(repo_dir, ignore_errors=True)
            os.makedirs(os.path.dirname(repo_dir), exist_ok=True)
            shutil.copytree(vibevoice_subdir, os.path.join(repo_dir, "vibevoice"))
            print(f"  Copied vibevoice/ -> {os.path.join(repo_dir, 'vibevoice')}")
            
            if os.path.exists(demo_subdir):
                demo_target = os.path.join(os.path.dirname(repo_dir), "repo", "demo")
                shutil.copytree(demo_subdir, demo_target, dirs_exist_ok=True)
                print(f"  Copied demo/ -> {demo_target}")
        else:
            if os.path.exists(repo_dir):
                shutil.rmtree(repo_dir, ignore_errors=True)
            os.makedirs(os.path.dirname(repo_dir), exist_ok=True)
            shutil.copytree(actual_root, repo_dir, dirs_exist_ok=True)
        
        shutil.rmtree(zip_temp_dir, ignore_errors=True)
        return True
        
    except Exception as e:
        print(f"ERROR during Microsoft repo download: {e}")
        if os.path.exists(zip_temp_dir):
            shutil.rmtree(zip_temp_dir, ignore_errors=True)
        return False


def download_vibevoice_realtime_voices():
    """Downloads voice embeddings for VibeVoice Realtime from the Microsoft repo.
    
    The voices are in demo/voices/streaming_model/ in the Microsoft repo.
    """
    import requests
    source_dir = _proj("audiobook_generator", "tts_models", "vibevoice", "repo")
    voices_source = os.path.join(source_dir, "demo", "voices", "streaming_model")
    voices_target = _proj("audiobook_generator", "tts_models", "vibevoice", "reference_voices", "vibevoice RealTime")
    
    os.makedirs(voices_target, exist_ok=True)
    
    if os.path.exists(voices_source):
        print(f"Voice embeddings found in Microsoft repo: {voices_source}")
        for filename in os.listdir(voices_source):
            src = os.path.join(voices_source, filename)
            dst = os.path.join(voices_target, filename)
            if not os.path.exists(dst):
                shutil.copy2(src, dst)
                print(f"  Copied: {filename}")
        print(f"Voice embeddings copied to: {voices_target}")
        return True
    else:
        print(f"WARNING: Voice embeddings not found in {voices_source}")
        print("The Microsoft repo may not contain voice embeddings.")
        return False

def download_qwen3tts_model(version_choice, model_type="base", idle_timeout=1800):
    """Downloads a specific version of the Qwen3-TTS model."""
    repo_map = {
        ("0.6B", "base"): "Qwen/Qwen3-TTS-12Hz-0.6B-Base",
        ("1.7B", "base"): "Qwen/Qwen3-TTS-12Hz-1.7B-Base",
        ("1.7B", "custom_voice"): "Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice",
        ("1.7B", "voice_design"): "Qwen/Qwen3-TTS-12Hz-1.7B-VoiceDesign",
    }
    key = (version_choice, model_type)
    if key not in repo_map:
        print(f"ERROR: Unsupported Qwen3-TTS combination: {key}")
        return False

    repo_id = repo_map[key]
    # Official folder name: Qwen3-TTS-12Hz-0.6B-Base, Qwen3-TTS-12Hz-1.7B-VoiceDesign, etc.
    # voice_design must be VoiceDesign (not Voice_design)
    if model_type == "voice_design":
        type_folder = "VoiceDesign"
    elif model_type == "custom_voice":
        type_folder = "CustomVoice"
    else:
        type_folder = model_type.capitalize()  # base -> Base
    folder_name = f"Qwen3-TTS-12Hz-{version_choice}-{type_folder}"
    plugin_name = f"Qwen3-TTS-{version_choice}-{type_folder}"
    target_dir = _proj("audiobook_generator", "tts_models", "qwen3tts", folder_name)

    if os.path.exists(os.path.join(target_dir, "config.json")):
        print(f"{plugin_name} model already present. Download skipped.")
        update_plugin_registry(plugin_name, installed=True)
        return True
    
    print(f"Downloading {plugin_name} model...")
    success = download_with_huggingface_hub(repo_id, target_dir, essential_files=["config.json", "generation_config.json"])
    
    if success:
        update_plugin_registry(plugin_name, installed=True)
        print(f"Installation of {plugin_name} completed.")
    else:
        print(f"ERROR: Installation of {plugin_name} failed.")
        
    return success

def download_qwen3tts_tokenizer(idle_timeout=1800):
    """Downloads the tokenizer for Qwen3-TTS."""
    repo_id = "Qwen/Qwen3-TTS-12Hz-0.6B-Base" # Tokenizer is the same for all
    target_dir = _proj("audiobook_generator", "tts_models", "qwen3tts", "tokenizer")
    
    if os.path.exists(os.path.join(target_dir, "tokenizer_config.json")):
        print("Qwen3-TTS tokenizer already present.")
        return True

    print("Downloading tokenizer for Qwen3-TTS...")
    return download_with_huggingface_hub(
        repo_id, 
        target_dir, 
        essential_files=["tokenizer_config.json", "vocab.json", "merges.txt"]
    )