#!/usr/bin/env python3
"""
Script to download TTS models without user interaction.
Used by app_gradio.py for downloads from the GUI.
"""

import sys
import os
import argparse
from setup.setup_helpers import download_qwen3tts_model, download_vibevoice_model, clone_repo, run_command, command_exists, download_with_huggingface_hub

def download_qwen_model(model_key):
    """Downloads a Qwen3-TTS model based on the key."""
    # Parse model_key: format "0.6B_base", "1.7B_custom_voice", etc.
    if "_" in model_key:
        parts = model_key.split("_", 1)
        version = parts[0]  # "0.6B" or "1.7B"
        model_type = parts[1]  # "base", "custom_voice", "voice_design"
    else:
        print(f"ERROR: Invalid model key format: {model_key}")
        return False
    
    print(f"Downloading Qwen3-TTS model {version} ({model_type})...")
    # Extended timeout of 2 hours for slow downloads
    return download_qwen3tts_model(version, model_type=model_type, idle_timeout=7200)

def download_vibevoice():
    """Downloads the VibeVoice model."""
    print("Downloading VibeVoice model...")
    # VibeVoice has two versions: bf16 and q8. We use bf16 as default.
    return download_vibevoice_model("bf16")

def download_xttsv2():
    """Downloads the XTTSv2 model."""
    print("Downloading XTTSv2 model...")
    model_dir = "audiobook_generator/tts_models/xttsv2"
    repo_id = "coqui/XTTS-v2"
    
    if os.path.exists(model_dir):
        print(f"XTTSv2 model already present in '{model_dir}'. Download skipped.")
        return True
    
    # Uses download_with_huggingface_hub which automatically handles Xet
    # Extended timeout of 2 hours for slow downloads
    return download_with_huggingface_hub(repo_id, model_dir, retries=3)

def download_kokoro():
    """Downloads the Kokoro model."""
    print("Downloading Kokoro model...")
    model_dir = "audiobook_generator/tts_models/kokoro/models"
    repo_id = "hexgrad/Kokoro-82M"
    
    if os.path.exists(model_dir):
        print(f"Kokoro model already present in '{model_dir}'. Download skipped.")
        return True
    
    # Uses download_with_huggingface_hub for consistency
    return download_with_huggingface_hub(repo_id, model_dir, retries=3)

def main():
    parser = argparse.ArgumentParser(description="Download TTS models without interaction")
    parser.add_argument("--model", required=True, help="Type of model to download")
    
    args = parser.parse_args()
    
    # Models -> download functions mapping
    model_handlers = {
        "0.6B_base": lambda: download_qwen_model("0.6B_base"),
        "1.7B_base": lambda: download_qwen_model("1.7B_base"),
        "1.7B_custom_voice": lambda: download_qwen_model("1.7B_custom_voice"),
        "1.7B_voice_design": lambda: download_qwen_model("1.7B_voice_design"),
        "vibevoice": download_vibevoice,
        "xttsv2": download_xttsv2,
        "kokoro": download_kokoro,
    }
    
    if args.model not in model_handlers:
        print(f"ERROR: Model '{args.model}' is not supported.")
        print(f"Supported models: {', '.join(model_handlers.keys())}")
        sys.exit(1)
    
    try:
        success = model_handlers[args.model]()
        if success:
            print(f"Download of model '{args.model}' completed successfully.")
            sys.exit(0)
        else:
            print(f"ERROR: Download of model '{args.model}' failed.")
            sys.exit(1)
    except KeyboardInterrupt:
        print("\nDownload interrupted by user.")
        sys.exit(1)
    except Exception as e:
        print(f"ERROR: Unexpected error during download: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()