# Copyright 2025 Carlo Piras
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""
Modulo per la gestione del download dei modelli TTS nell'interfaccia Gradio.
Questo modulo è progettato per essere importato in app_gradio.py per mantenere il codice modulare.
"""

import gradio as gr
import os
import sys
import json
import time
import threading
import queue
from typing import Dict, Any, List, Tuple, Optional
import logging

# Importa configurazioni dal progetto
try:
    from audiobook_generator import config
    from audiobook_generator import plugin_manager
except ImportError:
    # Fallback per importazioni dirette
    import sys
    sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    from audiobook_generator import config
    from audiobook_generator import plugin_manager

# Importa funzioni di download da setup/helpers.py
try:
    from setup import setup_helpers
    HAS_SETUP_HELPERS = True
except ImportError:
    HAS_SETUP_HELPERS = False
    logging.warning("setup/helpers.py non trovato. Il download dei modelli non sarà disponibile.")

# Flag per controllo stop download
STOP_DOWNLOAD_FLAG = False

def set_stop_download_flag():
    """Imposta il flag di stop a True per download"""
    global STOP_DOWNLOAD_FLAG
    STOP_DOWNLOAD_FLAG = True
    logging.info("Stop flag download impostato a True")
    return "Download in arresto..."

def reset_stop_download_flag():
    """Resetta il flag di stop a False per download"""
    global STOP_DOWNLOAD_FLAG
    STOP_DOWNLOAD_FLAG = False
    logging.info("Stop flag download resettato a False")
    return "Stop flag download resettato"

def check_stop_download_flag():
    """Controlla se il flag di stop è True per download"""
    global STOP_DOWNLOAD_FLAG
    return STOP_DOWNLOAD_FLAG


def get_available_models() -> List[Dict[str, Any]]:
    """Restituisce lista di tutti i modelli disponibili con stato."""
    models = []
    
    if not HAS_SETUP_HELPERS:
        return models
    
    # Modelli Qwen3-TTS
    qwen_models = [
        {
            "name": "Qwen3-TTS-0.6B-Base",
            "display_name": "Qwen3-TTS 0.6B Base",
            "description": "Voice Clone, ~2GB",
            "type": "qwen3tts",
            "version": "0.6B",
            "model_type": "base"
        },
        {
            "name": "Qwen3-TTS-1.7B-Base",
            "display_name": "Qwen3-TTS 1.7B Base",
            "description": "Voice Clone, ~5GB",
            "type": "qwen3tts",
            "version": "1.7B",
            "model_type": "base"
        },
        {
            "name": "Qwen3-TTS-1.7B-CustomVoice",
            "display_name": "Qwen3-TTS 1.7B CustomVoice",
            "description": "49 voci predefinite, ~5GB",
            "type": "qwen3tts",
            "version": "1.7B",
            "model_type": "custom_voice"
        },
        {
            "name": "Qwen3-TTS-1.7B-VoiceDesign",
            "display_name": "Qwen3-TTS 1.7B VoiceDesign",
            "description": "Descrizione testuale, ~5GB",
            "type": "qwen3tts",
            "version": "1.7B",
            "model_type": "voice_design"
        }
    ]
    
    # Modelli VibeVoice
    vibevoice_models = [
        {
            "name": "VibeVoice-1.5B",
            "display_name": "VibeVoice-1.5B",
            "description": "Community: 64K context, ~90 min, 4 speaker, ~12GB",
            "type": "vibevoice",
            "version": "1.5B"
        },
        {
            "name": "VibeVoice-7B",
            "display_name": "VibeVoice-7B",
            "description": "Community: 32K context, ~45 min, 4 speaker, ~25GB",
            "type": "vibevoice",
            "version": "7B"
        },
        {
            "name": "VibeVoice-Realtime-0.5B",
            "display_name": "VibeVoice-Realtime-0.5B",
            "description": "Ufficiale Microsoft: real-time, ~10 min, multilingue, ~3GB",
            "type": "vibevoice",
            "version": "Realtime-0.5B"
        }
    ]
    
    # Altri modelli
    other_models = [
        {
            "name": "XTTSv2",
            "display_name": "XTTSv2",
            "description": "Coqui TTS, ~2GB",
            "type": "xttsv2"
        },
        {
            "name": "Kokoro",
            "display_name": "Kokoro",
            "description": "Kokoro TTS, ~300MB",
            "type": "kokoro"
        }
    ]
    
    # Combina tutti i modelli
    all_models = qwen_models + vibevoice_models + other_models
    
    # Aggiungi stato di installazione
    for model in all_models:
        model["installed"] = check_model_installed(model["name"])
        model["status"] = "✅ Installato" if model["installed"] else "❌ Mancante"
    
    return all_models


def check_model_installed(model_name: str) -> bool:
    """Verifica se un modello specifico è installato."""
    if not HAS_SETUP_HELPERS:
        return False
    
    # Percorsi base
    base_project_dir = config.BASE_PROJECT_DIR if hasattr(config, 'BASE_PROJECT_DIR') else os.getcwd()
    
    if model_name.startswith("Qwen3-TTS"):
        # Struttura: tts_models/qwen3tts/Qwen3-TTS-12Hz-{version}-{type}/ (NO models/ sottocartella)
        base_dir = os.path.join(base_project_dir, "audiobook_generator/tts_models/qwen3tts")
        
        # Mappa nome plugin -> nome cartella (corrisponde al nome HuggingFace senza org)
        folder_map = {
            "Qwen3-TTS-0.6B-Base": "Qwen3-TTS-12Hz-0.6B-Base",
            "Qwen3-TTS-1.7B-Base": "Qwen3-TTS-12Hz-1.7B-Base",
            "Qwen3-TTS-1.7B-CustomVoice": "Qwen3-TTS-12Hz-1.7B-CustomVoice",
            "Qwen3-TTS-1.7B-VoiceDesign": "Qwen3-TTS-12Hz-1.7B-VoiceDesign",
        }
        
        folder_name = folder_map.get(model_name)
        if not folder_name:
            return False
        model_dir = os.path.join(base_dir, folder_name)
        
        # File essenziali per Qwen3-TTS
        essential_files = [
            "config.json",
            "generation_config.json",
            "preprocessor_config.json",
            "tokenizer_config.json",
            "vocab.json",
            "merges.txt"
        ]
        
        if not os.path.exists(model_dir):
            return False
        
        # Verifica file essenziali
        for file in essential_files:
            file_path = os.path.join(model_dir, file)
            if not os.path.exists(file_path):
                return False
        
        return True
    
    elif model_name.startswith("VibeVoice"):
        base_dir = os.path.join(base_project_dir, "audiobook_generator/tts_models/vibevoice")
        
        # Map model name -> folder name (0.5B, 1.5B, 7B - NOT the model name!)
        vibe_map = {
            "VibeVoice-1.5B": "1.5B",
            "VibeVoice-7B": "7B",
            "VibeVoice-Realtime-0.5B": "0.5B",
        }
        
        folder_name = vibe_map.get(model_name)
        if not folder_name:
            return False
        model_dir = os.path.join(base_dir, folder_name)
        
        if not os.path.exists(model_dir):
            return False
        
        # Essential files for VibeVoice
        essential_files = ["config.json", "preprocessor_config.json"]
        # Also need model file - either safetensors or first shard
        has_model = (
            os.path.exists(os.path.join(model_dir, "model.safetensors")) or
            os.path.exists(os.path.join(model_dir, "model-00001-of-00003.safetensors")) or
            os.path.exists(os.path.join(model_dir, "model-00001-of-00010.safetensors"))
        )
        
        has_files = all(os.path.exists(os.path.join(model_dir, f)) for f in essential_files) and has_model
        return has_files
    
    elif model_name == "XTTSv2":
        model_dir = os.path.join(base_project_dir, "audiobook_generator/tts_models/xttsv2")
        if not os.path.exists(model_dir):
            return False
        
        # File essenziali per XTTSv2
        essential_files = [
            "config.json",
            "model.pth",
            "dvae.pth"
        ]
        
        for file in essential_files:
            file_path = os.path.join(model_dir, file)
            if not os.path.exists(file_path):
                return False
        
        return True
    
    elif model_name == "Kokoro":
        # Kokoro uses HuggingFace cache structure
        # models/hub/models--hexgrad--Kokoro-82M/snapshots/<hash>/
        base_dir = os.path.join(base_project_dir, "audiobook_generator/tts_models/kokoro/models")
        hub_dir = os.path.join(base_dir, "hub")
        if not os.path.exists(hub_dir):
            return False
        
        # Find the snapshot folder (hash folder)
        snapshots_parent = None
        for item in os.listdir(hub_dir):
            if item.startswith("models--hexgrad--Kokoro-82M"):
                snapshots_parent = os.path.join(hub_dir, item)
                break
        
        if not snapshots_parent or not os.path.exists(snapshots_parent):
            return False
        
        # Find actual snapshot folder
        snapshots_dir = None
        for item in os.listdir(snapshots_parent):
            if item == "snapshots" or os.path.isdir(os.path.join(snapshots_parent, item)):
                snapshots_dir = os.path.join(snapshots_parent, "snapshots") if item == "snapshots" else os.path.join(snapshots_parent, item)
                break
        
        if not snapshots_dir or not os.path.exists(snapshots_dir):
            return False
        
        # Find the actual snapshot (hash folder)
        snapshot_folders = [d for d in os.listdir(snapshots_dir) if os.path.isdir(os.path.join(snapshots_dir, d))]
        if not snapshot_folders:
            return False
        
        actual_snapshot = os.path.join(snapshots_dir, snapshot_folders[0])
        
        # Essential files for Kokoro
        essential_files = ["config.json", "kokoro-v1_0.pth"]
        
        return all(os.path.exists(os.path.join(actual_snapshot, f)) for f in essential_files)
    
    return False


def get_models_status_message() -> str:
    """Restituisce un messaggio di stato leggibile per l'utente."""
    models = get_available_models()
    
    if not models:
        return "### 📦 Stato Modelli TTS\n\n⚠️ Impossibile caricare informazioni sui modelli."
    
    messages = []
    messages.append("### 📦 Stato Modelli TTS")
    messages.append("")
    
    # Raggruppa per tipo
    qwen_models = [m for m in models if m["type"] == "qwen3tts"]
    vibevoice_models = [m for m in models if m["type"] == "vibevoice"]
    other_models = [m for m in models if m["type"] in ["xttsv2", "kokoro"]]
    
    if qwen_models:
        messages.append("#### 🚀 Modelli Qwen3-TTS")
        for model in qwen_models:
            status_icon = "✅" if model["installed"] else "❌"
            messages.append(f"- {status_icon} **{model['display_name']}**: {model['description']}")
        messages.append("")
    
    if vibevoice_models:
        messages.append("#### 🎵 Modelli VibeVoice")
        for model in vibevoice_models:
            status_icon = "✅" if model["installed"] else "❌"
            messages.append(f"- {status_icon} **{model['display_name']}**: {model['description']}")
        messages.append("")
    
    if other_models:
        messages.append("#### 📚 Altri Modelli")
        for model in other_models:
            status_icon = "✅" if model["installed"] else "❌"
            messages.append(f"- {status_icon} **{model['display_name']}**: {model['description']}")
        messages.append("")
    
    messages.append("💡 **Nota**: I modelli marcati con ❌ non sono installati. Selezionali e clicca 'Scarica Modelli Selezionati' per installarli.")
    
    return "\n".join(messages)


def download_model_wrapper(model_info: Dict[str, Any]) -> Tuple[str, bool]:
    """Wrapper per download di un singolo modello."""
    global STOP_DOWNLOAD_FLAG
    
    if not HAS_SETUP_HELPERS:
        return "❌ Impossibile scaricare modelli: modulo setup/helpers.py non trovato.", False
    
    model_name = model_info["name"]
    model_type = model_info.get("type", "")
    
    try:
        logging.info(f"Inizio download modello: {model_name}")
        
        if model_type == "qwen3tts":
            version = model_info.get("version", "0.6B")
            model_type_param = model_info.get("model_type", "base")
            
            # Mappa model_type per compatibilità con helpers
            if model_type_param == "custom_voice":
                model_type_param = "custom_voice"
            elif model_type_param == "voice_design":
                model_type_param = "voice_design"
            else:
                model_type_param = "base"
            
            success = setup_helpers.download_qwen3tts_model(
                version_choice=version,
                model_type=model_type_param,
                idle_timeout=7200  # 2 ore timeout
            )
            
        elif model_type == "vibevoice":
            version = model_info.get("version", "1.5B")
            success = setup_helpers.download_vibevoice_model_multiple(
                version_choice=version,
                idle_timeout=7200  # 2 ore timeout
            )
            
        elif model_name == "XTTSv2":
            success = setup_helpers.download_xttsv2_model(idle_timeout=7200)
            
        elif model_name == "Kokoro":
            success = setup_helpers.download_kokoro_model(idle_timeout=7200)
            
        else:
            return f"❌ Tipo modello non supportato: {model_type}", False
        
        if success:
            # Aggiorna plugin registry se necessario
            if model_type in ["qwen3tts", "vibevoice", "xttsv2", "kokoro"]:
                try:
                    plugin_name = model_name
                    setup_helpers.update_plugin_registry(plugin_name, installed=True)
                except Exception as e:
                    logging.warning(f"Impossibile aggiornare plugin registry: {e}")
            
            return f"✅ Modello {model_name} scaricato con successo!", True
        else:
            return f"❌ Download modello {model_name} fallito.", False
            
    except Exception as e:
        logging.error(f"Errore durante download modello {model_name}: {e}")
        return f"❌ Errore durante download modello {model_name}: {str(e)}", False


def download_selected_models(selected_models: List[str], progress_callback=None) -> Tuple[str, bool]:
    """Scarica i modelli selezionati."""
    global STOP_DOWNLOAD_FLAG
    
    if not selected_models:
        return "❌ Nessun modello selezionato.", False
    
    # Reset stop flag
    reset_stop_download_flag()
    
    # Ottieni informazioni sui modelli selezionati
    all_models = get_available_models()
    models_to_download = []
    
    for model_name in selected_models:
        model_info = next((m for m in all_models if m["name"] == model_name), None)
        if model_info and not model_info["installed"]:
            models_to_download.append(model_info)
    
    if not models_to_download:
        return "✅ Tutti i modelli selezionati sono già installati.", True
    
    total_models = len(models_to_download)
    success_count = 0
    failed_count = 0
    messages = []
    
    for i, model_info in enumerate(models_to_download, 1):
        if check_stop_download_flag():
            messages.append("⏹️ Download interrotto dall'utente.")
            break
        
        # Aggiorna progresso
        if progress_callback:
            progress_callback(f"Scaricando {i}/{total_models}: {model_info['display_name']}")
        
        # Download modello
        message, success = download_model_wrapper(model_info)
        messages.append(message)
        
        if success:
            success_count += 1
        else:
            failed_count += 1
        
        # Piccola pausa tra download
        time.sleep(1)
    
    # Costruisci messaggio finale
    final_message = f"Download completato: {success_count} successi, {failed_count} falliti.\n\n"
    final_message += "\n".join(messages)
    
    overall_success = failed_count == 0
    return final_message, overall_success


def refresh_models_status() -> Tuple[str, List[str], str]:
    """Aggiorna lo stato dei modelli e restituisce informazioni aggiornate."""
    models = get_available_models()
    status_message = get_models_status_message()
    
    # Crea lista di nomi modello per checkbox
    model_names = [model["name"] for model in models]
    
    # Crea stringa di stato per display
    status_display = f"Modelli disponibili: {len(models)}\n"
    installed_count = sum(1 for model in models if model["installed"])
    status_display += f"Installati: {installed_count}, Mancanti: {len(models) - installed_count}"
    
    return status_message, model_names, status_display


def create_models_tab() -> gr.TabItem:
    """Creates the TTS Models Management tab."""
    with gr.TabItem("7. Download TTS Models") as tab:
        gr.Markdown("## 📥 TTS Models Management")
        gr.Markdown("This panel allows you to verify and download the TTS models required for the application to work.")
        
        # Initial state
        initial_status = get_models_status_message()
        models = get_available_models()
        initial_model_names = [model["name"] for model in models]
        installed_count = sum(1 for model in models if model["installed"])
        initial_status_display = f"Available models: {len(models)}\nInstalled: {installed_count}, Missing: {len(models) - installed_count}"
        
        # UI Components
        status_display = gr.Markdown(value=initial_status, label="Models Status")
        
        with gr.Row():
            status_summary = gr.Textbox(value=initial_status_display, label="Summary", interactive=False)
            refresh_btn = gr.Button("🔄 Update Status", variant="secondary")
        
        # Checkbox for model selection
        gr.Markdown("### 📋 Select Models to Download")
        models_checkbox = gr.CheckboxGroup(
            label="Missing Models",
            choices=initial_model_names,
            value=[model["name"] for model in models if not model["installed"]],
            interactive=True
        )
        
        # Action buttons
        with gr.Row():
            download_btn = gr.Button("📥 Download Selected Models", variant="primary")
            stop_download_btn = gr.Button("⏹️ Stop Download", variant="stop", visible=True)
            select_all_btn = gr.Button("✓ Select All Missing", variant="secondary")
            deselect_all_btn = gr.Button("✗ Deselect All", variant="secondary")
        
        # Download log
        download_log = gr.Textbox(
            label="Download Log",
            lines=8,
            interactive=False,
            placeholder="Download details will appear here..."
        )
        
        # Progress bar
        download_progress = gr.Textbox(
            label="Progress",
            visible=False,
            interactive=False
        )
        
        gr.Markdown("### 📚 Models Information")
        with gr.Accordion("Models Details", open=False):
            gr.Markdown("""
            #### Qwen3-TTS
            - **0.6B Base**: Base model for voice cloning (2GB)
            - **1.7B Base**: Advanced model for voice cloning (5GB)
            - **1.7B CustomVoice**: 49 predefined voices (5GB)
            - **1.7B VoiceDesign**: Generate voices from text description (5GB)
            
            #### VibeVoice
            - **1.5B**: Community model, 64K context, ~90 min, 4 speakers (12GB)
            - **7B**: Community model, 32K context, ~45 min, 4 speakers (25GB)
            - **Realtime-0.5B**: Official Microsoft model, real-time, multilingual (3GB)
            
            #### Other Models
            - **XTTSv2**: Coqui TTS, multilingual, voice cloning (2GB)
            - **Kokoro**: Lightweight TTS, multilingual (300MB)
            
            **Note**: Downloads may take a long time depending on your internet connection.
            """)
        
        # Funzioni per gestione UI
        def on_refresh():
            """Aggiorna stato modelli."""
            status_msg, model_names, status_disp = refresh_models_status()
            
            # Ottieni modelli mancanti per checkbox pre-selezionati
            models = get_available_models()
            missing_models = [model["name"] for model in models if not model["installed"]]
            
            return (
                status_msg,
                gr.update(choices=model_names, value=missing_models),
                status_disp,
                ""
            )
        
        def on_select_all():
            """Seleziona tutti i modelli mancanti."""
            models = get_available_models()
            missing_models = [model["name"] for model in models if not model["installed"]]
            return gr.update(value=missing_models)
        
        def on_deselect_all():
            """Deseleziona tutti i modelli."""
            return gr.update(value=[])
        
        def on_download(selected_models):
            """Avvia download modelli selezionati."""
            if not selected_models:
                return "❌ Nessun modello selezionato.", "", gr.update(visible=False)
            
            # Reset stop flag
            reset_stop_download_flag()
            
            # Funzione per aggiornamento progresso
            def progress_callback(message):
                nonlocal progress_text
                progress_text = message
            
            progress_text = ""
            
            # Esegui download
            message, success = download_selected_models(selected_models, progress_callback)
            
            # Aggiorna stato dopo download
            status_msg, model_names, status_disp = refresh_models_status()
            
            return (
                message,
                status_msg,
                gr.update(value=progress_text, visible=bool(progress_text))
            )
        
        # Collegamenti eventi
        refresh_btn.click(
            fn=on_refresh,
            outputs=[status_display, models_checkbox, status_summary, download_log]
        )
        
        select_all_btn.click(
            fn=on_select_all,
            outputs=[models_checkbox]
        )
        
        deselect_all_btn.click(
            fn=on_deselect_all,
            outputs=[models_checkbox]
        )
        
        download_btn.click(
            fn=on_download,
            inputs=[models_checkbox],
            outputs=[download_log, status_display, download_progress]
        )
        
        stop_download_btn.click(
            fn=set_stop_download_flag,
            outputs=[download_log]
        )
    
    return tab


if __name__ == "__main__":
    # Test del modulo
    print("Test modulo models_tab.py")
    print("=" * 50)
    
    models = get_available_models()
    print(f"Modelli trovati: {len(models)}")
    
    for model in models:
        print(f"- {model['name']}: {model['status']}")
    
    print("\nMessaggio di stato:")
    print(get_models_status_message())
