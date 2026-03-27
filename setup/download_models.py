#!/usr/bin/env python3
"""
Script per scaricare modelli TTS senza interazione utente.
Utilizzato da app_gradio.py per download dalla GUI.
"""

import sys
import os
import argparse
from setup.setup_helpers import download_qwen3tts_model, download_vibevoice_model, clone_repo, run_command, command_exists, download_with_huggingface_hub

def download_qwen_model(model_key):
    """Scarica un modello Qwen3-TTS in base alla chiave."""
    # Parse model_key: formato "0.6B_base", "1.7B_custom_voice", ecc.
    if "_" in model_key:
        parts = model_key.split("_", 1)
        version = parts[0]  # "0.6B" o "1.7B"
        model_type = parts[1]  # "base", "custom_voice", "voice_design"
    else:
        print(f"ERRORE: Formato chiave modello non valido: {model_key}")
        return False
    
    print(f"Download modello Qwen3-TTS {version} ({model_type})...")
    # Timeout esteso a 2 ore per download lenti
    return download_qwen3tts_model(version, model_type=model_type, idle_timeout=7200)

def download_vibevoice():
    """Scarica il modello VibeVoice."""
    print("Download modello VibeVoice...")
    # VibeVoice ha due versioni: bf16 e q8. Usiamo bf16 come default.
    return download_vibevoice_model("bf16")

def download_xttsv2():
    """Scarica il modello XTTSv2."""
    print("Download modello XTTSv2...")
    model_dir = "audiobook_generator/tts_models/xttsv2"
    repo_id = "coqui/XTTS-v2"
    
    if os.path.exists(model_dir):
        print(f"Il modello XTTSv2 è già presente in '{model_dir}'. Download saltato.")
        return True
    
    # Usa download_with_huggingface_hub che gestisce automaticamente Xet
    # Timeout esteso a 2 ore per download lenti
    return download_with_huggingface_hub(repo_id, model_dir, retries=3)

def download_kokoro():
    """Scarica il modello Kokoro."""
    print("Download modello Kokoro...")
    model_dir = "audiobook_generator/tts_models/kokoro/models"
    repo_id = "hexgrad/Kokoro-82M"
    
    if os.path.exists(model_dir):
        print(f"Il modello Kokoro è già presente in '{model_dir}'. Download saltato.")
        return True
    
    # Usa download_with_huggingface_hub per coerenza
    return download_with_huggingface_hub(repo_id, model_dir, retries=3)

def main():
    parser = argparse.ArgumentParser(description="Download modelli TTS senza interazione")
    parser.add_argument("--model", required=True, help="Tipo di modello da scaricare")
    
    args = parser.parse_args()
    
    # Mappa modelli -> funzioni di download
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
        print(f"ERRORE: Modello '{args.model}' non supportato.")
        print(f"Modelli supportati: {', '.join(model_handlers.keys())}")
        sys.exit(1)
    
    try:
        success = model_handlers[args.model]()
        if success:
            print(f"Download modello '{args.model}' completato con successo.")
            sys.exit(0)
        else:
            print(f"ERRORE: Download modello '{args.model}' fallito.")
            sys.exit(1)
    except KeyboardInterrupt:
        print("\nDownload interrotto dall'utente.")
        sys.exit(1)
    except Exception as e:
        print(f"ERRORE imprevisto durante il download: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()