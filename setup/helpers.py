"""
Wrapper di retrocompatibilità.
Re-exporta tutte le funzioni da setup.setup_helpers.
I file che facevano 'from helpers import ...' continuano a funzionare.
"""
import sys
import os

# Aggiungi la root del progetto a sys.path (la directory che contiene setup/)
# così 'setup.setup_helpers' viene trovato come package
_project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if _project_root not in sys.path:
    sys.path.insert(0, _project_root)

from setup.setup_helpers import (
    # system_utils
    run_command, command_exists, get_python_executable, clone_repo,
    check_venv_integrity, remove_directory,
    # environment_utils
    detect_gpu, detect_nvidia_gpu, detect_apple_silicon, detect_recommended_cuda,
    check_pytorch_cuda, check_pytorch_cuda_all_venvs, install_pytorch,
    # download_utils
    download_file, extract_archive, download_with_huggingface_hub,
    download_with_huggingface_hub_legacy, install_and_download_with_legacy_hf,
    detect_xet_repository,
    # model_downloader
    download_qwen3tts_model, download_qwen3tts_tokenizer,
    download_vibevoice_model_multiple, download_vibevoice_realtime_voices,
    download_vibevoice_repo_microsoft, download_vibevoice_vvembed,
    download_xttsv2_model, download_kokoro_model,
    # plugin_utils
    update_plugin_registry, update_plugin_registry_with_lock,
    # dependency_setup
    setup_ffmpeg, setup_sox,
    # user_prompts
    yes_no_prompt, choice_prompt,
)
