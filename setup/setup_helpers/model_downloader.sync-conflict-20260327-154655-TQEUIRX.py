# audiobook_generator/setup_helpers/model_downloader.py
import os
import shutil

from .system_utils import command_exists, run_command, clone_repo
from .download_utils import download_with_huggingface_hub, download_file, extract_archive
from .plugin_utils import update_plugin_registry, update_plugin_registry_with_lock

def download_kokoro_model(idle_timeout=1800):
    """Scarica il modello Kokoro-82M da HuggingFace."""
    repo_id = "hexgrad/Kokoro-82M"
    target_dir = "audiobook_generator/tts_models/kokoro/models"
    
    if os.path.exists(os.path.join(target_dir, "kokoro-v1_0.pth")):
        print("Modello Kokoro già presente. Download saltato.")
        update_plugin_registry("Kokoro", installed=True)
        return True

    print("Download modello Kokoro...")
    if download_with_huggingface_hub(repo_id, target_dir, essential_files=["config.json", "kokoro-v1_0.pth"]):
        update_plugin_registry("Kokoro", installed=True)
        return True
    else:
        print("ERRORE: Download di Kokoro fallito.")
        return False

def download_xttsv2_model(idle_timeout=1800):
    """Scarica il modello XTTS-v2 da HuggingFace."""
    repo_id = "coqui/XTTS-v2"
    target_dir = "audiobook_generator/tts_models/xttsv2"

    if os.path.exists(os.path.join(target_dir, "model.pth")):
        print("Modello XTTSv2 già presente. Download saltato.")
        update_plugin_registry("XTTSv2", installed=True)
        return True
        
    print("Download modello XTTSv2...")
    if download_with_huggingface_hub(repo_id, target_dir, essential_files=["config.json", "model.pth", "dvae.pth"]):
        update_plugin_registry("XTTSv2", installed=True)
        return True
    else:
        print("ERRORE: Download di XTTSv2 fallito.")
        return False

def download_vibevoice_tokenizer(idle_timeout=300):
    """Scarica il tokenizer Qwen2.5-1.5B per VibeVoice 1.5B e 7B.
    
    Il tokenizer Qwen NON è incluso nei modelli VibeVoice su HuggingFace.
    Deve essere scaricato separatamente da Qwen/Qwen2.5-1.5B.
    
    Il tokenizer viene salvato in tts_models/vibevoice/tokenizer/
    con i file: tokenizer.json, tokenizer_config.json, merges.txt, vocab.json
    
    Il codice di sintesi usa:
        processor = VibeVoiceProcessor.from_pretrained(
            vibevoice_model_dir,
            language_model_pretrained_name=vibevoice_tokenizer_dir,
            ...
        )
    """
    tokenizer_dir = os.path.join("audiobook_generator", "tts_models", "vibevoice", "tokenizer")
    
    # File necessari per il tokenizer Qwen
    tokenizer_files = ["tokenizer.json", "tokenizer_config.json", "merges.txt", "vocab.json"]
    
    # Determina la cache HF
    try:
        from huggingface_hub import constants
        hf_cache = constants.HUGGINGFACE_HUB_CACHE
    except ImportError:
        hf_cache = os.path.expanduser("~/.cache/huggingface/hub")
    qwen_7b_cache = os.path.join(hf_cache, "models--Qwen--Qwen2.5-7B")
    lock_file = qwen_7b_cache + ".lock"
    
    # ============================================================
    # LOCK: evita race condition se due download partono insieme
    # Il secondo processo aspetta che il primo finisca
    # ============================================================
    import time
    waited = 0
    wait_interval = 1.0
    while True:
        try:
            # O_EXCL rende la creazione atomica: fallisce se il file esiste già
            lock_fd = os.open(lock_file, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
            os.write(lock_fd, str(os.getpid()).encode())
            os.close(lock_fd)
            break  # Lock acquisito
        except FileExistsError:
            # Lock già esistente - aspetta e riprova
            if waited >= idle_timeout:
                print(f"TIMEOUT: lock file ancora presente dopo {idle_timeout}s. Probabile crash di un processo precedente.")
                return False
            time.sleep(wait_interval)
            waited += wait_interval
            wait_interval = min(wait_interval * 1.5, 10)  # backoff max 10s

    try:
        # Verifica SOLO DOPO aver acquisito il lock (nessun altro sta scrivendo)
        tokenizer_present = all(os.path.exists(os.path.join(tokenizer_dir, f)) for f in tokenizer_files)
        cache_7b_present = os.path.exists(qwen_7b_cache)

        if tokenizer_present and cache_7b_present:
            print("Tokenizer Qwen2.5-1.5B già presente e cache Qwen2.5-7B esistente. Niente da fare.")
            return True

        # Se manca il tokenizer locale, scaricalo
        if not tokenizer_present:
            print("Download tokenizer Qwen2.5-1.5B...")
            try:
                from huggingface_hub import hf_hub_download
            except ImportError:
                print("ERRORE: huggingface_hub non disponibile.")
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
                    print(f"ERRORE download {filename}: {e}")
                    return False
            print("Tokenizer Qwen2.5-1.5B scaricato con successo.")

        # ============================================================
        # FIX: Il codice VibeVoice per 7B cerca "Qwen/Qwen2.5-7B" nella cache.
        # Dato che Qwen2.5-1.5B e Qwen2.5-7B usano lo STESSO tokenizer,
        # creiamo un cache entry per Qwen2.5-7B copiando l'intera struttura cache.
        # ============================================================
        if not cache_7b_present:
            qwen_1_5b_cache = os.path.join(hf_cache, "models--Qwen--Qwen2.5-1.5B")
            if os.path.exists(qwen_1_5b_cache):
                print("Creazione cache per Qwen2.5-7B (stesso tokenizer di Qwen2.5-1.5B)...")
                import subprocess
                try:
                    result = subprocess.run(
                        ['cp', '-r', qwen_1_5b_cache + '/.', qwen_7b_cache],
                        capture_output=True, text=True
                    )
                    if result.returncode == 0:
                        print("  Cache Qwen2.5-7B creata con successo (struttura completa).")
                    else:
                        print(f"  Errore copia cache: {result.stderr}")
                except Exception as e:
                    print(f"  Errore durante copia cache: {e}")
            else:
                print("WARNING: Cache Qwen2.5-1.5B non trovata, impossibile creare cache 7B.")

        return True
    finally:
        # Rilascia il lock
        if os.path.exists(lock_file):
            os.remove(lock_file)


def download_vibevoice_model_multiple(version_choice, idle_timeout=1800):
    """Scarica il modello VibeVoice in base alla scelta.
    
    URL HuggingFace verificati (21/03/2026):
    - 1.5B:   microsoft/VibeVoice-1.5B
    - 7B:     vibevoice/VibeVoice-7B
    - Realtime-0.5B: microsoft/VibeVoice-Realtime-0.5B
    
    URL NON esistenti (rimossi):
    - vibevoice/VibeVoice-1.5B-full     → 404
    - vibevoice/VibeVoice-7B-low-vram   → 404
    
    Nota: I modelli 1.5B e 7B richiedono anche il tokenizer Qwen2.5-1.5B
    (scaricato separatamente da download_vibevoice_tokenizer()).
    """
    model_dir = "audiobook_generator/tts_models/vibevoice"
    # Mappa versione -> (repo HuggingFace, cartella locale)
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
        print(f"ERRORE: Versione non supportata: {version_choice}")
        return False
    repo, folder = repo_info
    # Struttura: {1.5B,7B,0.5B}/
    target_dir = os.path.join(model_dir, folder)
    print(f"Download modello VibeVoice-{version_choice}...")
    print(f"Target: {target_dir}")
    if os.path.exists(target_dir):
        essential_files = ["config.json", "preprocessor_config.json", "model.safetensors.index.json"]
        missing_files = []
        for file in essential_files:
            file_path = os.path.join(target_dir, file)
            if not os.path.exists(file_path):
                missing_files.append(file)
        if not missing_files:
            print(f"Il modello VibeVoice-{version_choice} è già presente e completo in '{target_dir}'. Download saltato.")
            update_plugin_registry_with_lock(plugin_name, installed=True)
            # Scarica il codice sorgente corretto in base al modello
            if version_choice == "Realtime-0.5B":
                download_vibevoice_repo_microsoft()
                download_vibevoice_vvembed()  # Necessario per classi streaming
                download_vibevoice_realtime_voices()  # Scarica voice embeddings
            else:
                download_vibevoice_vvembed()
                download_vibevoice_tokenizer()  # Tokenizer Qwen2.5-1.5B per 1.5B e 7B
            return True
        else:
            print(f"Il modello VibeVoice-{version_choice} esiste ma mancano {len(missing_files)} file essenziali: {missing_files}")
            print("Procedo con il download per completare il modello...")
    else:
        print(f"Il modello VibeVoice-{version_choice} non è presente in '{target_dir}'. Procedo con il download...")
    print("Tentativo 1: Download tramite huggingface_hub (Python)...")
    success = download_with_huggingface_hub(repo, target_dir)
    if not success and command_exists("hf"):
        print("Tentativo 2: Download tramite hf CLI...")
        cmd = ["hf", "download", repo, "--local-dir", target_dir]
        env = os.environ.copy()
        env["PYTHONIOENCODING"] = "utf-8"
        success = run_command(cmd, idle_timeout=idle_timeout)
    if not success and command_exists("huggingface-cli"):
        print("Tentativo 3: Download tramite huggingface-cli (deprecato)...")
        cmd = ["huggingface-cli", "download", repo, "--local-dir", target_dir]
        env = os.environ.copy()
        env["PYTHONIOENCODING"] = "utf-8"
        success = run_command(cmd, idle_timeout=idle_timeout)
    if success:
        update_plugin_registry_with_lock(plugin_name, installed=True)
        # Scarica il codice sorgente corretto in base al modello
        if version_choice == "Realtime-0.5B":
            download_vibevoice_repo_microsoft()
            download_vibevoice_vvembed()  # Necessario per classi streaming
            download_vibevoice_realtime_voices()  # Scarica voice embeddings
        else:
            download_vibevoice_vvembed()
            download_vibevoice_tokenizer()  # Tokenizer Qwen2.5-1.5B per 1.5B e 7B
    else:
        print(f"ERRORE: Tutti i metodi di download per VibeVoice-{version_choice} hanno fallito.")
    return success


def download_vibevoice_vvembed():
    """Scarica il codice sorgente VibeVoice (repo_community) necessario per i modelli 1.5B/7B.
    
    Struttura (matcha Test_TTS_Autonomi):
    - repo_community/ per modelli community (1.5B, 7B)
    - repo/ per modello Microsoft Realtime
    """
    import requests
    import zipfile
    repo_community_dir = "audiobook_generator/tts_models/vibevoice/repo_community"
    repo_url = "https://github.com/vibevoice-community/VibeVoice"
    temp_dir = "audiobook_generator/tts_models/vibevoice/repo_community_temp"
    
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
            print(f"La cartella repo_community è già presente e completa in '{repo_community_dir}'. Download saltato.")
            return True
        else:
            print(f"La cartella repo_community esiste ma mancano {len(missing_files)} file essenziali: {missing_files}")
            print("Procedo con il download per completare...")
    else:
        print(f"La cartella repo_community non è presente in '{repo_community_dir}'. Procedo con il download...")
    
    # Tentativo 1: Clone Git (opzionale, non obbligatorio)
    print(f"Tentativo clone repository {repo_url}...")
    git_success = False
    if command_exists("git"):
        git_success = clone_repo(repo_url, temp_dir)
    
    if git_success:
        print("Clone Git completato. Copia cartelle necessarie...")
        vibevoice_subdir = os.path.join(temp_dir, "vibevoice")
        if os.path.exists(vibevoice_subdir) and os.path.isdir(vibevoice_subdir):
            print(f"Trovata sottocartella 'vibevoice' in repository clonato")
            if os.path.exists(repo_community_dir):
                shutil.rmtree(repo_community_dir, ignore_errors=True)
            os.makedirs(os.path.dirname(repo_community_dir), exist_ok=True)
            shutil.copytree(vibevoice_subdir, os.path.join(repo_community_dir, "vibevoice"))
            print(f"  Copiato vibevoice/ -> {os.path.join(repo_community_dir, 'vibevoice')}")
        else:
            print(f"ATTENZIONE: Sottocartella 'vibevoice' non trovata, uso root")
            if os.path.exists(repo_community_dir):
                shutil.rmtree(repo_community_dir, ignore_errors=True)
            os.makedirs(os.path.dirname(repo_community_dir), exist_ok=True)
            shutil.copytree(temp_dir, repo_community_dir, dirs_exist_ok=True)
        
        shutil.rmtree(temp_dir, ignore_errors=True)
        return True
    else:
        print("Clone Git fallito o git non disponibile. Provo con download ZIP...")
    
    # Tentativo 2: Download ZIP (fallback principale)
    print("Tentativo download ZIP...")
    zip_url = "https://github.com/vibevoice-community/VibeVoice/archive/refs/heads/main.zip"
    zip_temp_dir = temp_dir + "_zip"
    
    try:
        if os.path.exists(zip_temp_dir):
            shutil.rmtree(zip_temp_dir, ignore_errors=True)
        os.makedirs(zip_temp_dir, exist_ok=True)
        
        zip_path = os.path.join(zip_temp_dir, "vibevoice.zip")
        print(f"Download ZIP da {zip_url}...")
        response = requests.get(zip_url, stream=True)
        response.raise_for_status()
        
        with open(zip_path, 'wb') as f:
            for chunk in response.iter_content(chunk_size=8192):
                f.write(chunk)
        
        print(f"Estrazione ZIP...")
        with zipfile.ZipFile(zip_path, 'r') as zip_ref:
            zip_ref.extractall(zip_temp_dir)
        
        root_items = [d for d in os.listdir(zip_temp_dir) if os.path.isdir(os.path.join(zip_temp_dir, d))]
        if len(root_items) == 1:
            actual_root = os.path.join(zip_temp_dir, root_items[0])
            print(f"Trovata cartella root: {root_items[0]}")
        else:
            actual_root = zip_temp_dir
        
        vibevoice_subdir = os.path.join(actual_root, "vibevoice")
        if os.path.exists(vibevoice_subdir) and os.path.isdir(vibevoice_subdir):
            if os.path.exists(repo_community_dir):
                shutil.rmtree(repo_community_dir, ignore_errors=True)
            os.makedirs(os.path.dirname(repo_community_dir), exist_ok=True)
            shutil.copytree(vibevoice_subdir, os.path.join(repo_community_dir, "vibevoice"))
            print(f"  Copiato vibevoice/ -> {os.path.join(repo_community_dir, 'vibevoice')}")
        else:
            if os.path.exists(repo_community_dir):
                shutil.rmtree(repo_community_dir, ignore_errors=True)
            os.makedirs(os.path.dirname(repo_community_dir), exist_ok=True)
            shutil.copytree(actual_root, repo_community_dir, dirs_exist_ok=True)
        
        shutil.rmtree(zip_temp_dir, ignore_errors=True)
        return True
            
    except Exception as e:
        print(f"ERRORE durante download ZIP: {e}")
        if os.path.exists(zip_temp_dir):
            shutil.rmtree(zip_temp_dir, ignore_errors=True)
        return False


def download_vibevoice_repo_microsoft():
    """Scarica il codice sorgente Microsoft per VibeVoice Realtime.
    
    Struttura (matcha Test_TTS_Autonomi):
    - repo/ per modello Microsoft Realtime
    """
    import requests
    import zipfile
    repo_dir = "audiobook_generator/tts_models/vibevoice/repo"
    repo_url = "https://github.com/microsoft/VibeVoice.git"
    temp_dir = "audiobook_generator/tts_models/vibevoice/repo_temp"
    
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
            print(f"La cartella repo è già presente e completa in '{repo_dir}'. Download saltato.")
            return True
        else:
            print(f"La cartella repo esiste ma mancano {len(missing_files)} file essenziali: {missing_files}")
            print("Procedo con il download per completare...")
    else:
        print(f"La cartella repo non è presente in '{repo_dir}'. Procedo con il download...")
    
    print(f"Tentativo clone repository Microsoft...")
    git_success = False
    if command_exists("git"):
        git_success = clone_repo(repo_url, temp_dir)
    
    if git_success:
        print("Clone Git completato. Copia cartelle necessarie...")
        vibevoice_subdir = os.path.join(temp_dir, "vibevoice")
        demo_subdir = os.path.join(temp_dir, "demo")
        
        if os.path.exists(vibevoice_subdir) and os.path.isdir(vibevoice_subdir):
            if os.path.exists(repo_dir):
                shutil.rmtree(repo_dir, ignore_errors=True)
            os.makedirs(os.path.dirname(repo_dir), exist_ok=True)
            shutil.copytree(vibevoice_subdir, os.path.join(repo_dir, "vibevoice"))
            print(f"  Copiato vibevoice/ -> {os.path.join(repo_dir, 'vibevoice')}")
            
            if os.path.exists(demo_subdir) and os.path.isdir(demo_subdir):
                demo_target = os.path.join(os.path.dirname(repo_dir), "repo", "demo")
                shutil.copytree(demo_subdir, demo_target, dirs_exist_ok=True)
                print(f"  Copiato demo/ -> {demo_target}")
        else:
            if os.path.exists(repo_dir):
                shutil.rmtree(repo_dir, ignore_errors=True)
            os.makedirs(os.path.dirname(repo_dir), exist_ok=True)
            shutil.copytree(temp_dir, repo_dir, dirs_exist_ok=True)
        
        shutil.rmtree(temp_dir, ignore_errors=True)
        return True
    else:
        print("Clone Git fallito. Provo con download ZIP...")
    
    zip_url = "https://github.com/microsoft/VibeVoice/archive/refs/heads/main.zip"
    zip_temp_dir = temp_dir + "_zip"
    
    try:
        if os.path.exists(zip_temp_dir):
            shutil.rmtree(zip_temp_dir, ignore_errors=True)
        os.makedirs(zip_temp_dir, exist_ok=True)
        
        zip_path = os.path.join(zip_temp_dir, "repo.zip")
        print(f"Download ZIP da {zip_url}...")
        response = requests.get(zip_url, stream=True)
        response.raise_for_status()
        
        with open(zip_path, 'wb') as f:
            for chunk in response.iter_content(chunk_size=8192):
                f.write(chunk)
        
        print("Estrazione ZIP...")
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
            print(f"  Copiato vibevoice/ -> {os.path.join(repo_dir, 'vibevoice')}")
            
            if os.path.exists(demo_subdir):
                demo_target = os.path.join(os.path.dirname(repo_dir), "repo", "demo")
                shutil.copytree(demo_subdir, demo_target, dirs_exist_ok=True)
                print(f"  Copiato demo/ -> {demo_target}")
        else:
            if os.path.exists(repo_dir):
                shutil.rmtree(repo_dir, ignore_errors=True)
            os.makedirs(os.path.dirname(repo_dir), exist_ok=True)
            shutil.copytree(actual_root, repo_dir, dirs_exist_ok=True)
        
        shutil.rmtree(zip_temp_dir, ignore_errors=True)
        return True
        
    except Exception as e:
        print(f"ERRORE durante download repo Microsoft: {e}")
        if os.path.exists(zip_temp_dir):
            shutil.rmtree(zip_temp_dir, ignore_errors=True)
        return False


def download_vibevoice_realtime_voices():
    """Scarica le voice embeddings per VibeVoice Realtime dal repo Microsoft.
    
    Le voci sono in demo/voices/streaming_model/ nel repo Microsoft.
    """
    import requests
    source_dir = "audiobook_generator/tts_models/vibevoice/repo"
    voices_source = os.path.join(source_dir, "demo", "voices", "streaming_model")
    voices_target = "audiobook_generator/tts_models/vibevoice/reference_voices/vibevoice RealTime"
    
    os.makedirs(voices_target, exist_ok=True)
    
    if os.path.exists(voices_source):
        print(f"Voice embeddings trovate in repo Microsoft: {voices_source}")
        for filename in os.listdir(voices_source):
            src = os.path.join(voices_source, filename)
            dst = os.path.join(voices_target, filename)
            if not os.path.exists(dst):
                shutil.copy2(src, dst)
                print(f"  Copiato: {filename}")
        print(f"Voice embeddings copiate in: {voices_target}")
        return True
    else:
        print(f"ATTENZIONE: Voice embeddings non trovate in {voices_source}")
        print("Il repo Microsoft potrebbe non contenere le voice embeddings.")
        return False

def download_qwen3tts_model(version_choice, model_type="base", idle_timeout=1800):
    """Scarica una versione specifica del modello Qwen3-TTS."""
    repo_map = {
        ("0.6B", "base"): "Qwen/Qwen3-TTS-12Hz-0.6B-Base",
        ("1.7B", "base"): "Qwen/Qwen3-TTS-12Hz-1.7B-Base",
        ("1.7B", "custom_voice"): "Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice",
        ("1.7B", "voice_design"): "Qwen/Qwen3-TTS-12Hz-1.7B-VoiceDesign",
    }
    key = (version_choice, model_type)
    if key not in repo_map:
        print(f"ERRORE: Combinazione Qwen3-TTS non supportata: {key}")
        return False

    repo_id = repo_map[key]
    # Nome cartella ufficiale: Qwen3-TTS-12Hz-0.6B-Base, Qwen3-TTS-12Hz-1.7B-VoiceDesign, ecc.
    # voice_design deve essere VoiceDesign (non Voice_design)
    if model_type == "voice_design":
        type_folder = "VoiceDesign"
    elif model_type == "custom_voice":
        type_folder = "CustomVoice"
    else:
        type_folder = model_type.capitalize()  # base -> Base
    folder_name = f"Qwen3-TTS-12Hz-{version_choice}-{type_folder}"
    plugin_name = f"Qwen3-TTS-{version_choice}-{type_folder}"
    target_dir = f"audiobook_generator/tts_models/qwen3tts/{folder_name}"

    if os.path.exists(os.path.join(target_dir, "config.json")):
        print(f"Modello {plugin_name} già presente. Download saltato.")
        update_plugin_registry(plugin_name, installed=True)
        return True
    
    print(f"Download modello {plugin_name}...")
    success = download_with_huggingface_hub(repo_id, target_dir, essential_files=["config.json", "generation_config.json"])
    
    if success:
        update_plugin_registry(plugin_name, installed=True)
        print(f"Installazione di {plugin_name} completata.")
    else:
        print(f"ERRORE: Installazione di {plugin_name} fallita.")
        
    return success

def download_qwen3tts_tokenizer(idle_timeout=1800):
    """Scarica il tokenizer per Qwen3-TTS."""
    repo_id = "Qwen/Qwen3-TTS-12Hz-0.6B-Base" # Il tokenizer è lo stesso per tutti
    target_dir = "audiobook_generator/tts_models/qwen3tts/tokenizer"
    
    if os.path.exists(os.path.join(target_dir, "tokenizer_config.json")):
        print("Tokenizer Qwen3-TTS già presente.")
        return True

    print("Download tokenizer per Qwen3-TTS...")
    return download_with_huggingface_hub(
        repo_id, 
        target_dir, 
        essential_files=["tokenizer_config.json", "vocab.json", "merges.txt"]
    )