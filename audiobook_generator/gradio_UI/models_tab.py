# Copyright (c) 2026 Patata Audiobook Generator
#
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in all
# copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
# SOFTWARE.

"""
Module for managing TTS model downloads in the Gradio interface.
This module is designed to be imported in app_gradio.py to keep the code modular.
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

logger = logging.getLogger(__name__)

# Import configurations from project
try:
    from audiobook_generator import config
    from audiobook_generator import plugin_manager
except ImportError:
    # Fallback for direct imports
    import sys
    sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    from audiobook_generator import config
    from audiobook_generator import plugin_manager

# Import download functions from setup/helpers.py
try:
    from setup import setup_helpers
    HAS_SETUP_HELPERS = True
except ImportError:
    HAS_SETUP_HELPERS = False
    logging.warning("setup/helpers.py not found. Model download will not be available.")

# Flag for download stop control
STOP_DOWNLOAD_FLAG = False

def set_stop_download_flag():
    """Set the stop flag to True for downloads"""
    global STOP_DOWNLOAD_FLAG
    STOP_DOWNLOAD_FLAG = True
    logging.info("Download stop flag set to True")
    return "Download stopping..."

def reset_stop_download_flag():
    """Reset the stop flag to False for downloads"""
    global STOP_DOWNLOAD_FLAG
    STOP_DOWNLOAD_FLAG = False
    logging.info("Download stop flag reset to False")
    return "Download stop flag reset"

def check_stop_download_flag():
    """Check if the stop flag is True for downloads"""
    global STOP_DOWNLOAD_FLAG
    return STOP_DOWNLOAD_FLAG


def get_available_models() -> List[Dict[str, Any]]:
    """Returns list of all available models with status."""
    models = []
    
    if not HAS_SETUP_HELPERS:
        return models
    
    # Qwen3-TTS models
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
            "description": "49 predefined voices, ~5GB",
            "type": "qwen3tts",
            "version": "1.7B",
            "model_type": "custom_voice"
        },
        {
            "name": "Qwen3-TTS-1.7B-VoiceDesign",
            "display_name": "Qwen3-TTS 1.7B VoiceDesign",
            "description": "Text description, ~5GB",
            "type": "qwen3tts",
            "version": "1.7B",
            "model_type": "voice_design"
        }
    ]
    
    # VibeVoice models
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
            "description": "Official Microsoft: real-time, ~10 min, multilingual, ~3GB",
            "type": "vibevoice",
            "version": "Realtime-0.5B"
        }
    ]
    
    # Other models
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
    
    # Combine all models
    all_models = qwen_models + vibevoice_models + other_models
    
    # Add installation status
    for model in all_models:
        model["installed"] = check_model_installed(model["name"])
        model["status"] = "✅ Installed" if model["installed"] else "❌ Missing"
    
    return all_models


def check_model_installed(model_name: str) -> bool:
    """Verifies if a specific model is installed."""
    if not HAS_SETUP_HELPERS:
        return False
    
    base_project_dir = config.BASE_PROJECT_DIR if hasattr(config, 'BASE_PROJECT_DIR') else os.getcwd()
    
    try:
        if model_name.startswith("Qwen3-TTS"):
            base_dir = os.path.join(base_project_dir, "audiobook_generator/tts_models/qwen3tts")
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
            essential_files = [
                "config.json", "generation_config.json",
                "preprocessor_config.json", "tokenizer_config.json",
                "vocab.json", "merges.txt"
            ]
            if not os.path.exists(model_dir):
                return False
            for file in essential_files:
                if not os.path.exists(os.path.join(model_dir, file)):
                    return False
            return True
        
        elif model_name.startswith("VibeVoice"):
            base_dir = os.path.join(base_project_dir, "audiobook_generator/tts_models/vibevoice")
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
            essential_files = ["config.json", "preprocessor_config.json"]
            has_model = (
                os.path.exists(os.path.join(model_dir, "model.safetensors")) or
                os.path.exists(os.path.join(model_dir, "model-00001-of-00003.safetensors")) or
                os.path.exists(os.path.join(model_dir, "model-00001-of-00010.safetensors"))
            )
            return all(os.path.exists(os.path.join(model_dir, f)) for f in essential_files) and has_model
        
        elif model_name == "XTTSv2":
            model_dir = os.path.join(base_project_dir, "audiobook_generator/tts_models/xttsv2")
            if not os.path.exists(model_dir):
                return False
            essential_files = ["config.json", "model.pth", "dvae.pth"]
            for file in essential_files:
                if not os.path.exists(os.path.join(model_dir, file)):
                    return False
            return True
        
        elif model_name == "Kokoro":
            base_dir = os.path.join(base_project_dir, "audiobook_generator/tts_models/kokoro/models")
            hub_dir = os.path.join(base_dir, "hub")
            if not os.path.exists(hub_dir):
                return False
            snapshots_parent = None
            for item in os.listdir(hub_dir):
                if item.startswith("models--hexgrad--Kokoro-82M"):
                    snapshots_parent = os.path.join(hub_dir, item)
                    break
            if not snapshots_parent or not os.path.exists(snapshots_parent):
                return False
            snapshots_dir = None
            for item in os.listdir(snapshots_parent):
                if item == "snapshots" or os.path.isdir(os.path.join(snapshots_parent, item)):
                    snapshots_dir = os.path.join(snapshots_parent, "snapshots") if item == "snapshots" else os.path.join(snapshots_parent, item)
                    break
            if not snapshots_dir or not os.path.exists(snapshots_dir):
                return False
            snapshot_folders = [d for d in os.listdir(snapshots_dir) if os.path.isdir(os.path.join(snapshots_dir, d))]
            if not snapshot_folders:
                return False
            actual_snapshot = os.path.join(snapshots_dir, snapshot_folders[0])
            essential_files = ["config.json", "kokoro-v1_0.pth"]
            return all(os.path.exists(os.path.join(actual_snapshot, f)) for f in essential_files)
        
        return False
    except OSError as e:
        logger.warning("Error checking model installation for '%s': %s", model_name, e)
        return False


def get_models_status_message() -> str:
    """Returns a user-readable status message."""
    models = get_available_models()
    
    if not models:
        return "### 📦 TTS Models Status\n\n⚠️ Unable to load model information."
    
    messages = []
    messages.append("### 📦 TTS Models Status")
    messages.append("")
    
    # Group by type
    qwen_models = [m for m in models if m["type"] == "qwen3tts"]
    vibevoice_models = [m for m in models if m["type"] == "vibevoice"]
    other_models = [m for m in models if m["type"] in ["xttsv2", "kokoro"]]
    
    if qwen_models:
        messages.append("#### 🚀 Qwen3-TTS Models")
        for model in qwen_models:
            status_icon = "✅" if model["installed"] else "❌"
            messages.append(f"- {status_icon} **{model['display_name']}**: {model['description']}")
        messages.append("")
    
    if vibevoice_models:
        messages.append("#### 🎵 VibeVoice Models")
        for model in vibevoice_models:
            status_icon = "✅" if model["installed"] else "❌"
            messages.append(f"- {status_icon} **{model['display_name']}**: {model['description']}")
        messages.append("")
    
    if other_models:
        messages.append("#### 📚 Other Models")
        for model in other_models:
            status_icon = "✅" if model["installed"] else "❌"
            messages.append(f"- {status_icon} **{model['display_name']}**: {model['description']}")
        messages.append("")
    
    messages.append("💡 **Note**: Models marked with ❌ are not installed. Select them and click 'Download Selected Models' to install them.")
    
    return "\n".join(messages)


def download_model_wrapper(model_info: Dict[str, Any]) -> Tuple[str, bool]:
    """Wrapper for downloading a single model."""
    global STOP_DOWNLOAD_FLAG
    
    if not HAS_SETUP_HELPERS:
        return "❌ Cannot download models: setup/helpers.py module not found.", False
    
    model_name = model_info["name"]
    model_type = model_info.get("type", "")
    
    try:
        logging.info(f"Starting model download: {model_name}")
        
        if model_type == "qwen3tts":
            version = model_info.get("version", "0.6B")
            model_type_param = model_info.get("model_type", "base")
            
            # Map model_type for helpers compatibility
            if model_type_param == "custom_voice":
                model_type_param = "custom_voice"
            elif model_type_param == "voice_design":
                model_type_param = "voice_design"
            else:
                model_type_param = "base"
            
            success = setup_helpers.download_qwen3tts_model(
                version_choice=version,
                model_type=model_type_param,
                idle_timeout=7200  # 2 hour timeout
            )
            
        elif model_type == "vibevoice":
            version = model_info.get("version", "1.5B")
            success = setup_helpers.download_vibevoice_model_multiple(
                version_choice=version,
                idle_timeout=7200  # 2 hour timeout
            )
            
        elif model_name == "XTTSv2":
            success = setup_helpers.download_xttsv2_model(idle_timeout=7200)
            
        elif model_name == "Kokoro":
            success = setup_helpers.download_kokoro_model(idle_timeout=7200)
            
        else:
            return f"❌ Unsupported model type: {model_type}", False
        
        if success:
            # The plugin registry no longer has the "installed" field
            # Status is determined dynamically via filesystem
            
            return f"✅ Model {model_name} downloaded successfully!", True
        else:
            return f"❌ Model {model_name} download failed.", False
            
    except Exception as e:
        logging.error(f"Error during model download {model_name}: {e}")
        return f"❌ Error during model download {model_name}: {str(e)}", False


def download_selected_models(selected_models: List[str], progress_callback=None) -> Tuple[str, bool]:
    """Downloads selected models."""
    global STOP_DOWNLOAD_FLAG
    
    if not selected_models:
        return "❌ No models selected.", False
    
    # Reset stop flag
    reset_stop_download_flag()
    
    # Get information about selected models
    all_models = get_available_models()
    models_to_download = []
    
    for model_name in selected_models:
        model_info = next((m for m in all_models if m["name"] == model_name), None)
        if model_info and not model_info["installed"]:
            models_to_download.append(model_info)
    
    if not models_to_download:
        return "✅ All selected models are already installed.", True
    
    total_models = len(models_to_download)
    success_count = 0
    failed_count = 0
    messages = []
    
    for i, model_info in enumerate(models_to_download, 1):
        if check_stop_download_flag():
            messages.append("⏹️ Download interrupted by user.")
            break
        
        # Update progress
        if progress_callback:
            progress_callback(f"Downloading {i}/{total_models}: {model_info['display_name']}")
        
        # Download model
        message, success = download_model_wrapper(model_info)
        messages.append(message)
        
        if success:
            success_count += 1
        else:
            failed_count += 1
        
        # Brief pause between downloads
        time.sleep(1)
    
    # Build final message
    final_message = f"Download completed: {success_count} succeeded, {failed_count} failed.\n\n"
    final_message += "\n".join(messages)
    
    overall_success = failed_count == 0
    return final_message, overall_success


def refresh_models_status() -> Tuple[str, List[str], str]:
    """Refreshes model status and returns updated information."""
    models = get_available_models()
    status_message = get_models_status_message()
    
    # Create model name list for checkboxes
    model_names = [model["name"] for model in models]
    
    # Create status string for display
    status_display = f"Available models: {len(models)}\n"
    installed_count = sum(1 for model in models if model["installed"])
    status_display += f"Installed: {installed_count}, Missing: {len(models) - installed_count}"
    
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
        
        # UI management functions
        def on_refresh():
            """Refresh model status."""
            status_msg, model_names, status_disp = refresh_models_status()
            
        # Get missing models for pre-selected checkboxes
            models = get_available_models()
            missing_models = [model["name"] for model in models if not model["installed"]]
            
            return (
                status_msg,
                gr.update(choices=model_names, value=missing_models),
                status_disp,
                ""
            )
        
        def on_select_all():
            """Select all missing models."""
            models = get_available_models()
            missing_models = [model["name"] for model in models if not model["installed"]]
            return gr.update(value=missing_models)
        
        def on_deselect_all():
            """Deselect all models."""
            return gr.update(value=[])
        
        def on_download(selected_models):
            """Start download of selected models."""
            if not selected_models:
                return "❌ No models selected.", "", gr.update(visible=False)
            
            # Reset stop flag
            reset_stop_download_flag()
            
            # Function for progress updates
            def progress_callback(message):
                nonlocal progress_text
                progress_text = message
            
            progress_text = ""
            
            # Execute download
            message, success = download_selected_models(selected_models, progress_callback)
            
            # Update status after download
            status_msg, model_names, status_disp = refresh_models_status()
            
            return (
                message,
                status_msg,
                gr.update(value=progress_text, visible=bool(progress_text))
            )
        
        # Event connections
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
    # Module test
    print("Testing models_tab.py module")
    print("=" * 50)
    
    models = get_available_models()
    print(f"Models found: {len(models)}")
    
    for model in models:
        print(f"- {model['name']}: {model['status']}")
    
    print("\nStatus message:")
    print(get_models_status_message())
