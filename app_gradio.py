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

import gradio as gr
import os
import sys
import time
import traceback
import shutil
import re
import logging
from typing import Optional, Any, Dict, List
from datetime import datetime

# Aggiungi sox/bin al PATH per evitare errori di soundfile
sox_bin = os.path.join(os.getcwd(), "sox", "bin")
if os.path.exists(sox_bin):
    os.environ["PATH"] = sox_bin + os.pathsep + os.environ["PATH"]
    sys.path.insert(0, sox_bin)  # non necessario, ma per sicurezza

# Import necessary functions from our modules
from audiobook_generator import config
from audiobook_generator import utils
from audiobook_generator import tts_handler
from audiobook_generator import epub_processor
from audiobook_generator import ffmpeg_wrapper
from audiobook_generator import plugin_manager

# Importa modulo per gestione dipendenze
try:
    from audiobook_generator.gradio_UI import dependencies_tab
    HAS_DEPENDENCIES_TAB = True
except ImportError:
    HAS_DEPENDENCIES_TAB = False
    print("⚠️ Modulo dependencies_tab.py non trovato. Il tab System/Dependencies non sarà disponibile.")

# Importa modulo per recovery errori
try:
    from audiobook_generator.gradio_UI import recovery_tab
    HAS_RECOVERY_TAB = True
except ImportError:
    HAS_RECOVERY_TAB = False
    print("⚠️ Modulo recovery_tab.py non trovato. Il tab Recovery Errori non sarà disponibile.")

# Importa modulo per download modelli
try:
    from audiobook_generator.gradio_UI import models_tab
    HAS_MODELS_TAB = True
except ImportError:
    HAS_MODELS_TAB = False
    print("⚠️ Modulo models_tab.py non trovato. Il tab Download Modelli non sarà disponibile.")

# +++ NUOVE IMPORTAZIONI PER I TAB MODULARI +++
from audiobook_generator.gradio_UI.configuration_tab import create_configuration_tab
from audiobook_generator.gradio_UI.epub_options_tab import create_epub_options_tab
from audiobook_generator.gradio_UI.generate_tab import create_generate_tab
from audiobook_generator.gradio_UI.demo_test_tab import create_demo_test_tab


# Esegui verifica dipendenze all'avvio
if HAS_DEPENDENCIES_TAB:
    print("\n" + "="*60)
    print("Verifica dipendenze di sistema...")
    deps_status = dependencies_tab.check_external_dependencies()
    print(f"FFmpeg: {'Presente' if deps_status['ffmpeg']['present'] else 'Assente'}")
    print(f"SoX: {'Presente' if deps_status['sox']['present'] else 'Assente'}")
    print("="*60 + "\n")
else:
    print("\n" + "="*60)
    print("⚠️ Verifica dipendenze non disponibile (modulo mancante)")
    print("="*60 + "\n")

# --- Global settings or constants for Gradio ---
# Flag per controllo stop
STOP_FLAG = False

def set_stop_flag():
    """Imposta il flag di stop a True"""
    global STOP_FLAG
    STOP_FLAG = True
    logging.info("Stop flag impostato a True")
    return "Processo in arresto..."

def reset_stop_flag():
    """Resetta il flag di stop a False"""
    global STOP_FLAG
    STOP_FLAG = False
    logging.info("Stop flag resettato a False")
    return "Stop flag resettato"

def check_stop_flag():
    """Controlla se il flag di stop è True"""
    global STOP_FLAG
    return STOP_FLAG

# Rendi la lista dei modelli dinamica
if config.USE_PLUGIN_ARCHITECTURE:
    # Carica i modelli abilitati dal registro dei plugin
    raw_models = plugin_manager.plugin_manager.list_available_models()
    
    # Filtra: se ci sono modelli VibeVoice specifici, rimuovi "VibeVoice" generico
    vibevoice_specific_models = [m for m in raw_models if m.startswith("VibeVoice-")]
    if vibevoice_specific_models and "VibeVoice" in raw_models:
        raw_models = [m for m in raw_models if m != "VibeVoice"]
    
    # Aggiungi indicazione "(da scaricare)" per modelli Qwen3-TTS mancanti
    TTS_MODELS = []
    for model in raw_models:
        if model.startswith("Qwen3-TTS-"):
            # Estrai dimensione e tipo (es: "0.6B-Base", "1.7B-CustomVoice")
            parts = model.split("-")
            if len(parts) >= 4:
                size_type = f"{parts[2]}-{parts[3]}"  # "0.6B-Base"
            else:
                size_type = f"{parts[2]}"  # Fallback
            
            model_key = f"Qwen3-TTS-{size_type}"
            
            # Cerca in MODEL_ASSETS — usa "path" (non "dest") percorso diretto
            model_path = None
            for asset_key, assets in config.MODEL_ASSETS.items():
                if asset_key == model_key:
                    for asset in assets:
                        # MODEL_ASSETS Qwen usa "path", non "dest"
                        if "path" in asset:
                            model_path = asset["path"]
                            break
                    break
            
            # Costruisci percorso assoluto
            if model_path:
                abs_dest = os.path.join(config.BASE_PROJECT_DIR, model_path)
            else:
                abs_dest = None
            
            if abs_dest and os.path.exists(abs_dest):
                TTS_MODELS.append(model)  # modello presente
            else:
                TTS_MODELS.append(f"{model} (da scaricare)")
        elif model.startswith("VibeVoice-"):
            # Verifica filesystem real-time tramite MODEL_ASSETS (NON il JSON statico)
            vv_path = None
            for asset_key, assets in config.MODEL_ASSETS.items():
                if asset_key == model:
                    for asset in assets:
                        if "path" in asset:
                            vv_path = asset["path"]
                            break
                    break
            if vv_path:
                vv_abs = os.path.join(config.BASE_PROJECT_DIR, vv_path)
                # Verifica che la cartella ESISTA e contenga file essenziali
                if os.path.exists(vv_abs) and os.listdir(vv_abs):
                    TTS_MODELS.append(model)
                else:
                    TTS_MODELS.append(f"{model} (da scaricare)")
            else:
                TTS_MODELS.append(model)  # non trovato in MODEL_ASSETS, passa comunque
        else:
            TTS_MODELS.append(model)
    print(f"INFO: Modalità Plugin ATTIVA. Modelli caricati: {TTS_MODELS}")
else:
    # Mantieni la lista statica per retrocompatibilità
    TTS_MODELS = ["XTTSv2", "Kokoro", "VibeVoice"]
    print("INFO: Modalità Plugin DISATTIVATA. Utilizzo della lista modelli statica.")

# --- Language definitions per model ---
MODEL_LANGUAGES = {
    "XTTSv2": ["en", "es", "fr", "de", "it", "pt", "pl", "tr", "ru", "nl", "cs", "ar", "zh-cn", "ja", "hu", "ko", "hi"],
    "Kokoro": sorted(list(config.AVAILABLE_KOKORO_MODELS.keys())),
    "VibeVoice": ["en", "es", "fr", "de", "it", "pt", "pl", "ru", "zh-cn", "ja"],
    "VibeVoice-1.5B": ["en", "es", "fr", "de", "it", "pt", "pl", "ru", "zh-cn", "ja"],
    "VibeVoice-7B": ["en", "es", "fr", "de", "it", "pt", "pl", "ru", "zh-cn", "ja"],
    "VibeVoice-Realtime-0.5B": ["en", "es", "fr", "de", "it", "pt", "pl", "ru", "zh-cn", "ja"],
    "Qwen3-TTS-0.6B": ["en", "es", "fr", "de", "it", "pt", "pl", "ru", "zh-cn", "ja"],
    "Qwen3-TTS-1.7B": ["en", "es", "fr", "de", "it", "pt", "pl", "ru", "zh-cn", "ja"],
}
INITIAL_LANGUAGES = MODEL_LANGUAGES.get(TTS_MODELS[0], ["en", "it"])

# --- Constants for UI ---
CHUNKING_STRATEGIES = ["Word Count Approx", "Character Limit"]
DEFAULT_CHUNKING_STRATEGY = CHUNKING_STRATEGIES[0] if not config.DEFAULT_USE_CHAR_LIMIT_CHUNKING else CHUNKING_STRATEGIES[1]
SENTENCE_SEPARATOR_OPTIONS = [
    ("Standard Period (.)", "."), ("Pipe (|)", "|"), ("Semicolon (;)", ";"),
    ("Silence Tag (<sil>)", "<sil>"), ("Pause Tag ([PAUSE])", "[PAUSE]"), ("Underscore (_)", "_"),
]
DEFAULT_SEPARATOR_DISPLAY = next((name for name, value in SENTENCE_SEPARATOR_OPTIONS if value == config.DEFAULT_SENTENCE_SEPARATOR), ".")

# --- Logging Setup ---
def setup_file_logger(log_dir: str, base_filename: str = "generation") -> tuple[logging.Logger, Optional[str]]:
    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    log_filename = f"{base_filename}_{timestamp}.log"
    log_path = os.path.join(log_dir, log_filename)
    logger = logging.getLogger(log_filename)
    if logger.hasHandlers():
        logger.handlers.clear()
    logger.setLevel(logging.INFO)
    try:
        os.makedirs(log_dir, exist_ok=True)
        file_handler = logging.FileHandler(log_path, mode='w', encoding='utf-8')
        formatter = logging.Formatter('%(asctime)s - %(levelname)s - %(message)s')
        file_handler.setFormatter(formatter)
        logger.addHandler(file_handler)
        logger.info("--- Log Start ---")
        return logger, log_path
    except Exception as e:
        print(f"  ERROR: Failed to set up file logger at {log_path}: {e}")
        return logging.getLogger('dummy'), None

# --- Funzioni per gestione errori recovery ---
def save_failed_chunks_json(book_name: str, data: dict):
    """Salva il file failed_chunks.json per un audiolibro."""
    import json
    book_dir = os.path.join("Generated_Audiobooks", book_name)
    os.makedirs(book_dir, exist_ok=True)
    json_path = os.path.join(book_dir, "failed_chunks.json")
    try:
        with open(json_path, 'w', encoding='utf-8') as f:
            json.dump(data, f, indent=2, ensure_ascii=False)
        return True
    except Exception as e:
        logging.error(f"Errore salvataggio failed_chunks.json: {e}")
        return False

def load_failed_chunks_json(book_name: str):
    """Carica il file failed_chunks.json per un audiolibro."""
    import json
    json_path = os.path.join("Generated_Audiobooks", book_name, "failed_chunks.json")
    if not os.path.exists(json_path):
        return None
    try:
        with open(json_path, 'r', encoding='utf-8') as f:
            return json.load(f)
    except Exception as e:
        logging.error(f"Errore caricamento failed_chunks.json: {e}")
        return None

# --- Backend Helper Functions ---
def update_language_dropdown(model, current_language):
    base_model = model.split(" (da scaricare)")[0]
    if base_model.startswith("Qwen3-TTS-"):
        parts = base_model.split("-")
        if len(parts) >= 3:
            size = parts[2]
            base_key = f"Qwen3-TTS-{size}"
        else:
            base_key = "Qwen3-TTS-1.7B"
        supported_langs = MODEL_LANGUAGES.get(base_key, [])
    else:
        supported_langs = MODEL_LANGUAGES.get(base_model, [])
    
    new_value = config.DEFAULT_LANGUAGE
    if current_language in supported_langs: 
        new_value = current_language
    elif config.DEFAULT_LANGUAGE not in supported_langs and supported_langs: 
        new_value = supported_langs[0]
    else: 
        new_value = None
    return gr.update(choices=supported_langs, value=new_value)

def update_voice_options_for_model(model, xtts_lang, kokoro_lang, vibevoice_lang):
    """Aggiorna le opzioni voce in base al modello e alla lingua selezionata"""
    file_update = gr.update(visible=False, value=None, label="Upload Reference WAV (.wav)")
    dropdown_update = gr.update(visible=False, choices=[], value=None, label="Select Voice")
    try:
        if model == "XTTSv2":
            file_update = gr.update(visible=True, label="Upload Reference WAV (.wav)")
        elif model == "Kokoro":
            if kokoro_lang:
                voices = tts_handler.get_kokoro_voices(kokoro_lang) or []
                dropdown_update = gr.update(visible=True, label="Select Kokoro Voice", choices=voices, value=None)
        elif model.startswith("VibeVoice") and not model.endswith("Realtime-0.5B"):
            file_update = gr.update(visible=True, label="Upload Reference WAV (.wav)")
    except Exception as e: 
        logging.error(f"ERROR in update_voice_options_for_model: {e}", exc_info=True)
    return file_update, dropdown_update

def update_parameter_visibility(model):
    return (
        gr.update(visible=model == "XTTSv2"),
        gr.update(visible=False),
        gr.update(visible=model == "Kokoro"),
        gr.update(visible=model.startswith("Qwen3-TTS")),
        gr.update(visible=model.startswith("VibeVoice") and not model.endswith("Realtime-0.5B")),
        gr.update(visible=model == "VibeVoice-Realtime-0.5B")
    )

def update_chunking_options_visibility(strategy):
    return gr.update(visible=strategy == CHUNKING_STRATEGIES[0]), gr.update(visible=strategy == CHUNKING_STRATEGIES[1])

def update_qwen_visibility(selected_model):
    visible = selected_model.startswith("Qwen3-TTS")
    return gr.update(visible=visible)

def update_qwen_mode_visibility(selected_mode):
    return (
        gr.update(visible=selected_mode == "Custom Voice"),
        gr.update(visible=selected_mode == "Voice Clone"),
        gr.update(visible=selected_mode == "Voice Design")
    )

def update_qwen_mode_availability(selected_model):
    if not selected_model.startswith("Qwen3-TTS"):
        return gr.update(choices=[], value=None, interactive=False)
    base_dir = os.path.join(config.BASE_PROJECT_DIR, "audiobook_generator/tts_models/qwen3tts")
    # Nomi ufficiali delle cartelle
    has_base_0_6 = os.path.exists(os.path.join(base_dir, "Qwen3-TTS-12Hz-0.6B-Base"))
    has_base_1_7 = os.path.exists(os.path.join(base_dir, "Qwen3-TTS-12Hz-1.7B-Base"))
    has_custom_voice = os.path.exists(os.path.join(base_dir, "Qwen3-TTS-12Hz-1.7B-CustomVoice"))
    has_voice_design = os.path.exists(os.path.join(base_dir, "Qwen3-TTS-12Hz-1.7B-VoiceDesign"))
    all_modes = ["Custom Voice", "Voice Clone", "Voice Design"]
    available_modes = []
    if has_base_0_6 or has_base_1_7: available_modes.append("Voice Clone")
    if has_custom_voice: available_modes.append("Custom Voice")
    if has_voice_design: available_modes.append("Voice Design")
    if not available_modes: return gr.update(choices=all_modes, value=None, interactive=False)
    default_mode = available_modes[0]
    choices_with_hint = []
    for mode in all_modes:
        if mode in available_modes: choices_with_hint.append(mode)
        else:
            if mode == "Custom Voice": choices_with_hint.append(f"Custom Voice (modello non scaricato)")
            elif mode == "Voice Design": choices_with_hint.append(f"Voice Design (modello non scaricato)")
            else: choices_with_hint.append(f"{mode} (modello non scaricato)")
    return gr.update(choices=choices_with_hint, value=default_mode, interactive=True)

def update_qwen_clone_text_visibility(is_fast_mode):
    return gr.update(interactive=not is_fast_mode)

def update_qwen_panel(selected_model):
    if not selected_model.startswith("Qwen3-TTS"):
        return [gr.update(visible=False), gr.update(choices=[], value=None, interactive=False), gr.update(visible=False), gr.update(visible=False), gr.update(visible=False)]
    base_dir = os.path.join(config.BASE_PROJECT_DIR, "audiobook_generator/tts_models/qwen3tts")
    # Nomi ufficiali delle cartelle
    has_base = os.path.exists(os.path.join(base_dir, "Qwen3-TTS-12Hz-0.6B-Base")) or os.path.exists(os.path.join(base_dir, "Qwen3-TTS-12Hz-1.7B-Base"))
    has_custom = os.path.exists(os.path.join(base_dir, "Qwen3-TTS-12Hz-1.7B-CustomVoice"))
    has_design = os.path.exists(os.path.join(base_dir, "Qwen3-TTS-12Hz-1.7B-VoiceDesign"))
    if "Base" in selected_model:
        default_mode = "Voice Clone"
        available_modes = ["Voice Clone"] if has_base else []
    elif "CustomVoice" in selected_model:
        default_mode = "Custom Voice"
        available_modes = ["Custom Voice"] if has_custom else []
    elif "VoiceDesign" in selected_model:
        default_mode = "Voice Design"
        available_modes = ["Voice Design"] if has_design else []
    else:
        available_modes = []
        if has_custom: available_modes.append("Custom Voice")
        if has_base: available_modes.append("Voice Clone")
        if has_design: available_modes.append("Voice Design")
        default_mode = available_modes[0] if available_modes else None
    choices = []
    for mode in ["Custom Voice", "Voice Clone", "Voice Design"]:
        if mode == "Custom Voice" and not has_custom: choices.append("Custom Voice (modello non scaricato)")
        elif mode == "Voice Design" and not has_design: choices.append("Voice Design (modello non scaricato)")
        elif mode == "Voice Clone" and not has_base: choices.append("Voice Clone (modello non scaricato)")
        else: choices.append(mode)
    show_custom = default_mode == "Custom Voice"
    show_clone = default_mode == "Voice Clone"
    show_design = default_mode == "Voice Design"
    return [gr.update(visible=True), gr.update(choices=choices, value=default_mode, interactive=True), gr.update(visible=show_custom), gr.update(visible=show_clone), gr.update(visible=show_design)]

def toggle_select_all_chapters(chapter_list, current_selection):
    if not chapter_list: return gr.update(value=[]), gr.update(value="Seleziona tutto")
    if current_selection and len(current_selection) == len(chapter_list): return gr.update(value=[]), gr.update(value="Seleziona tutto")
    else: return gr.update(value=chapter_list), gr.update(value="Deseleziona tutto")

def invert_chapter_selection(chapter_list, current_selection):
    if not chapter_list: return gr.update(value=[])
    current_set = set(current_selection or [])
    chapter_set = set(chapter_list)
    inverted = list(chapter_set - current_set)
    return gr.update(value=inverted)

def get_model_specific_config(model_name):
    base_model = model_name.split(" (da scaricare)")[0]
    if base_model in config.TTS_MODEL_CONFIG: return config.TTS_MODEL_CONFIG[base_model]
    if base_model.startswith("VibeVoice"):
        if base_model in config.TTS_MODEL_CONFIG: return config.TTS_MODEL_CONFIG[base_model]
        elif "VibeVoice" in config.TTS_MODEL_CONFIG: return config.TTS_MODEL_CONFIG["VibeVoice"]
    if base_model.startswith("Qwen3-TTS"):
        if base_model in config.TTS_MODEL_CONFIG: return config.TTS_MODEL_CONFIG[base_model]
        if "Base" in base_model and "Qwen3-TTS-1.7B-Base" in config.TTS_MODEL_CONFIG: return config.TTS_MODEL_CONFIG["Qwen3-TTS-1.7B-Base"]
        elif "Base" in base_model and "Qwen3-TTS-0.6B-Base" in config.TTS_MODEL_CONFIG: return config.TTS_MODEL_CONFIG["Qwen3-TTS-0.6B-Base"]
    return {}

def update_model_specific_options(selected_model, xtts_lang, kokoro_lang, vibevoice_lang, current_chunking_strategy, current_max_chars, current_separator, current_replace_guillemets):
    model_config = get_model_specific_config(selected_model)
    updates = []
    if "chunking_strategy" in model_config: updates.append(gr.update(value=model_config["chunking_strategy"]))
    else: updates.append(gr.update())
    new_max_chars = current_max_chars
    if "char_limit_recommended" in model_config: new_max_chars = model_config["char_limit_recommended"]
    elif selected_model == "Kokoro" and "char_limits_by_lang" in model_config:
        current_lang = kokoro_lang if kokoro_lang else config.DEFAULT_LANGUAGE
        lang_limits = model_config["char_limits_by_lang"].get(current_lang, {})
        if "max" in lang_limits: new_max_chars = lang_limits["max"]
        elif "min" in lang_limits: new_max_chars = lang_limits["min"]
    updates.append(gr.update(value=new_max_chars))
    if selected_model == "XTTSv2": updates.append(gr.update(value="Pipe (|)"))
    else: updates.append(gr.update(value="Standard Period (.)"))
    if selected_model == "XTTSv2": updates.append(gr.update(value=True))
    else: updates.append(gr.update(value=False))
    note_text = ""
    if "note" in model_config: note_text = model_config["note"]
    if selected_model.startswith("VibeVoice") and "time_warning" in model_config: note_text += f"\n\n⚠️ {model_config['time_warning']}"
    if selected_model == "Kokoro" and "char_limits_by_lang" in model_config:
        current_lang = kokoro_lang if kokoro_lang else config.DEFAULT_LANGUAGE
        lang_limits = model_config["char_limits_by_lang"].get(current_lang, {})
        if "note" in lang_limits: note_text += f"\n\n📝 {lang_limits['note']}"
        if "max" in lang_limits: note_text += f"\n✅ Limite caratteri automaticamente impostato a {lang_limits['max']} (circa {int(lang_limits['max']/6)}-{int(lang_limits['max']/5)} parole)."
    if selected_model == "XTTSv2": note_text += "\n\nℹ️ Replace Guillemets e Pipe separator sono attivati automaticamente per XTTSv2, che non interpreta bene alcuni simboli."
    updates.append(gr.update(value=note_text, visible=bool(note_text)))
    return tuple(updates)

def update_qwen_mode_for_model(selected_model):
    if not selected_model.startswith("Qwen3-TTS"): return gr.update(value=None, interactive=False, choices=[])
    model_config = get_model_specific_config(selected_model)
    if "mode" in model_config:
        mode = model_config["mode"]
        return gr.update(value=mode, interactive=False, choices=[mode])
    elif "supported_modes" in model_config:
        modes = model_config["supported_modes"]
        return gr.update(value=modes[0] if modes else None, interactive=len(modes) > 1, choices=modes)
    else:
        return gr.update(value="Custom Voice", interactive=False, choices=["Custom Voice", "Voice Clone", "Voice Design"])

def update_language_dropdown_position(model):
    return gr.update(visible=True)

def get_model_status(model_name):
    if model_name.startswith("Qwen3-TTS"):
        base_dir = os.path.join(config.BASE_PROJECT_DIR, "audiobook_generator/tts_models/qwen3tts")
        # Nomi ufficiali delle cartelle
        name_map = {
            "Qwen3-TTS-0.6B-Base": "Qwen3-TTS-12Hz-0.6B-Base",
            "Qwen3-TTS-1.7B-Base": "Qwen3-TTS-12Hz-1.7B-Base",
            "Qwen3-TTS-1.7B-CustomVoice": "Qwen3-TTS-12Hz-1.7B-CustomVoice",
            "Qwen3-TTS-1.7B-VoiceDesign": "Qwen3-TTS-12Hz-1.7B-VoiceDesign",
        }
        folder_name = name_map.get(model_name)
        if not folder_name:
            return f"❌ Modello '{model_name}' non riconosciuto"
        path = os.path.join(base_dir, folder_name)
        if os.path.exists(os.path.join(path, "config.json")): return f"✅ **{model_name}**: Presente"
        else: return f"❌ **{model_name}**: Mancante (clicca 'Scarica' per installare)"
    elif model_name.startswith("VibeVoice"):
        base_dir = "audiobook_generator/tts_models/vibevoice"
        # Path reali (non i nomi dei modelli!)
        if model_name == "VibeVoice-7B": target_dir = os.path.join(base_dir, "7B")
        elif model_name == "VibeVoice-1.5B": target_dir = os.path.join(base_dir, "1.5B")
        elif model_name == "VibeVoice-Realtime-0.5B": target_dir = os.path.join(base_dir, "0.5B")
        else: target_dir = os.path.join(base_dir, "model")
        # Verifica file essenziali: config sempre richiesto, model puo' essere .safetensors O .safetensors.index.json
        if not os.path.exists(os.path.join(target_dir, "config.json")):
            return f"❌ **{model_name}**: Mancante (config.json assente)"
        if not os.path.exists(os.path.join(target_dir, "preprocessor_config.json")):
            return f"❌ **{model_name}**: Mancante (preprocessor_config.json assente)"
        has_model = os.path.exists(os.path.join(target_dir, "model.safetensors")) or \
                    os.path.exists(os.path.join(target_dir, "model.safetensors.index.json"))
        if has_model: return f"✅ **{model_name}**: Presente"
        else: return f"❌ **{model_name}**: Mancante (file modello assente)"
    elif model_name == "XTTSv2":
        base_dir = "audiobook_generator/tts_models/xttsv2"
        if os.path.exists(base_dir): return f"✅ **XTTSv2**: Presente"
        else: return f"❌ **XTTSv2**: Mancante (clicca 'Scarica' per installare)"
    elif model_name == "Kokoro":
        base_dir = "audiobook_generator/tts_models/kokoro/models"
        if os.path.exists(base_dir): return f"✅ **Kokoro**: Presente"
        else: return f"❌ **Kokoro**: Mancante (clicca 'Scarica' per installare)"
    else: return f"ℹ️ **{model_name}**: Stato non disponibile"

def get_all_models_status():
    status_lines = []
    status_lines.append("### 📦 Modelli Qwen3-TTS")
    for model in ["Qwen3-TTS-0.6B-Base", "Qwen3-TTS-1.7B-Base", "Qwen3-TTS-1.7B-CustomVoice", "Qwen3-TTS-1.7B-VoiceDesign"]:
        status_lines.append(get_model_status(model))
    # Tokenizer è incluso dentro ogni directory del modello Qwen (tokenizer_config.json, merges.txt, speech_tokenizer/)
    # NON esiste una cartella tokenizer/ separata — non serve check qui
    status_lines.append("\n### 🎵 Altri Modelli")
    for model in ["VibeVoice-7B", "VibeVoice-1.5B", "VibeVoice-Realtime-0.5B", "XTTSv2", "Kokoro"]:
        status_lines.append(get_model_status(model))
    return "\n".join(status_lines)

def get_qwen_model_status():
    return get_all_models_status()

def map_language_to_test_file(lang_code, model_name):
    if not lang_code: return "en"
    lang_code = str(lang_code).strip()
    mapping = {"zh-cn": "cn", "zh-CN": "cn", "Chinese": "cn", "English": "en", "German": "de", "Italian": "it", "Portuguese": "pt", "Spanish": "es", "Japanese": "ja", "Korean": "ko", "French": "fr", "Russian": "ru", "Auto": "en", "it": "it", "en": "en", "es": "es", "fr": "fr", "de": "de", "pt": "pt", "pl": "pl", "ru": "ru", "ja": "ja", "hu": "hu", "ko": "ko", "hi": "hi", "ar": "ar", "nl": "nl", "cs": "cs", "tr": "tr"}
    if lang_code in mapping: return mapping[lang_code]
    if "-" in lang_code:
        parts = lang_code.split("-")
        if parts[0] in mapping: return mapping[parts[0]]
        if parts[1] in mapping: return mapping[parts[1]]
    if len(lang_code) >= 2:
        prefix = lang_code[:2].lower()
        prefix_map = {"zh": "cn", "cn": "cn", "en": "en", "it": "it", "es": "es", "fr": "fr", "de": "de", "pt": "pt", "pl": "pl", "ru": "ru", "ja": "ja", "ko": "ko"}
        if prefix in prefix_map: return prefix_map[prefix]
    return "en"

def _extract_file_path(file_obj):
    if file_obj is None: return None
    if hasattr(file_obj, 'name'): return file_obj.name
    elif isinstance(file_obj, dict) and 'name' in file_obj: return file_obj['name']
    else: return str(file_obj)

def _safe_int_conversion(value):
    if value is None or isinstance(value, dict): return None
    try: return int(value)
    except (ValueError, TypeError): return None

def _safe_float_conversion(value, default=None, min_val=None):
    if value is None or isinstance(value, dict): return default
    try:
        result = float(value)
        if min_val is not None and result < min_val: return default
        return result
    except (ValueError, TypeError): return default

def _safe_top_k_conversion(value, default=20):
    if value is None or isinstance(value, dict): return default
    try:
        int_val = int(round(float(value)))
        if int_val < 0: return default
        return int_val
    except (ValueError, TypeError): return default

def _prepare_tts_config(selected_lang, selected_model, xtts_wav_file_obj, piper_kokoro_voice_desc, xtts_temp, xtts_speed_in, xtts_rep_pen, xtts_top_k, xtts_top_p, xtts_length_penalty, xtts_gpt_cond_len, piper_speed_in, piper_noise_scale, piper_noise_scale_w, kokoro_speed_in, vibevoice_temp=None, vibevoice_top_p=None, vibevoice_cfg_scale=None, vibevoice_diffusion_steps=None, vibevoice_speed_factor=None, vibevoice_seed=None, vibevoice_use_sampling=None, vibevoice_top_k=None, vibevoice_realtime_speaker=None, vibevoice_realtime_cfg_scale=None, vibevoice_realtime_ddpm_steps=None, vibevoice_realtime_temperature=None, vibevoice_realtime_top_p=None, vibevoice_realtime_top_k=None, vibevoice_realtime_seed_number=None):
    base_model = selected_model.split(" (da scaricare)")[0]
    technical_voice_id, final_tts_params, error_message = None, {}, None
    try:
        if base_model in ["XTTSv2"] or (base_model.startswith("VibeVoice") and not base_model == "VibeVoice-Realtime-0.5B"):
            if not xtts_wav_file_obj: raise ValueError(f"No reference WAV uploaded for {base_model}.")
            technical_voice_id = _extract_file_path(xtts_wav_file_obj)
            if technical_voice_id is None and hasattr(xtts_wav_file_obj, 'name'): technical_voice_id = xtts_wav_file_obj.name
        elif base_model == "VibeVoice-Realtime-0.5B":
            technical_voice_id = vibevoice_realtime_speaker if vibevoice_realtime_speaker else "it-Spk0_woman"
        elif base_model == "Kokoro":
            if not piper_kokoro_voice_desc: raise ValueError("No Kokoro voice chosen.")
            lang_kokoro_info = config.AVAILABLE_KOKORO_MODELS.get(selected_lang, {})
            technical_voice_id = next((v.get("id") for v in lang_kokoro_info.get("voices", []) if v.get("description") == piper_kokoro_voice_desc), None)
            if not technical_voice_id: raise ValueError(f"Could not find Kokoro tech ID for '{piper_kokoro_voice_desc}'.")
    except Exception as e: 
        error_message = f"Error resolving voice ID: {e}"
    if not error_message:
        try:
            if base_model == "XTTSv2": final_tts_params.update({"temperature": xtts_temp, "speed": xtts_speed_in, "repetition_penalty": xtts_rep_pen, "top_k": xtts_top_k, "top_p": xtts_top_p, "length_penalty": xtts_length_penalty, "gpt_cond_len": xtts_gpt_cond_len})
            elif base_model == "Kokoro": final_tts_params["speed"] = kokoro_speed_in
            elif base_model == "VibeVoice-Realtime-0.5B":
                final_tts_params.update({"cfg_scale": vibevoice_realtime_cfg_scale if vibevoice_realtime_cfg_scale is not None else 1.3, "diffusion_steps": vibevoice_realtime_ddpm_steps if vibevoice_realtime_ddpm_steps is not None else 5, "temperature": vibevoice_realtime_temperature if vibevoice_realtime_temperature is not None else 1.0, "top_p": vibevoice_realtime_top_p if vibevoice_realtime_top_p is not None else 0.9, "top_k": vibevoice_realtime_top_k if vibevoice_realtime_top_k is not None else 0, "seed": vibevoice_realtime_seed_number, "speaker_wav": technical_voice_id})
            elif base_model.startswith("VibeVoice"): 
                final_tts_params.update({"temperature": vibevoice_temp if vibevoice_temp is not None else 0.9, "top_p": vibevoice_top_p if vibevoice_top_p is not None else 0.9, "top_k": vibevoice_top_k if vibevoice_top_k is not None else 0, "cfg_scale": vibevoice_cfg_scale if vibevoice_cfg_scale is not None else 1.3, "diffusion_steps": vibevoice_diffusion_steps if vibevoice_diffusion_steps is not None else 15, "voice_speed_factor": vibevoice_speed_factor if vibevoice_speed_factor is not None else 1.0, "use_sampling": vibevoice_use_sampling if vibevoice_use_sampling is not None else True, "seed": vibevoice_seed})
            elif base_model.startswith("Qwen3-TTS"):
                model_size = "0.6B" if "0.6B" in base_model else "1.7B"
                base_dir = os.path.join(config.BASE_PROJECT_DIR, "audiobook_generator/tts_models/qwen3tts")
                if model_size == "0.6B": model_type = "base"
                else:
                    # Nomi ufficiali delle cartelle
                    if os.path.exists(os.path.join(base_dir, "Qwen3-TTS-12Hz-1.7B-CustomVoice")): model_type = "custom_voice"
                    elif os.path.exists(os.path.join(base_dir, "Qwen3-TTS-12Hz-1.7B-VoiceDesign")): model_type = "voice_design"
                    else: model_type = "base"
                final_tts_params["qwen_model_size"] = model_size
                final_tts_params["qwen_model_type"] = model_type
        except Exception as e: error_message = f"Error setting TTS parameters: {e}"
    return technical_voice_id, final_tts_params, error_message

def _load_tts_model_instance(selected_model, language, technical_voice_id):
    start_time = time.time()
    try:
        base_model = selected_model.split(" (da scaricare)")[0]
        if base_model.startswith("Qwen3-TTS") or base_model.startswith("VibeVoice"):
            model_instance = plugin_manager.plugin_manager.load_model(base_model)
        else:
            loader = {"XTTSv2": tts_handler.load_xtts_model, "Kokoro": lambda: tts_handler.load_kokoro_model(language)}.get(base_model)
            if not loader: raise ValueError(f"Model loading not implemented for: {base_model}")
            model_instance = loader()
        if not model_instance: raise RuntimeError(f"Model loader returned None for {base_model}.")
        logging.info(f"TTS model loaded in {time.time() - start_time:.2f}s.")
        return model_instance, None
    except Exception as e:
        logging.error(f"Error loading TTS model '{selected_model}': {e}", exc_info=True)
        return None, str(e)

def _process_ebook_chapters(epub_filepath, book_final_output_dir, book_chunk_output_dir, model_instance_wrapper, selected_lang, selected_model, technical_voice_id, final_tts_params, final_proc_opts, selected_chapter_keys, update_callback, logger):
    def _send_update(message, level=logging.INFO):
        if update_callback:
            try: yield from update_callback(message, level=level)
            except Exception as cb_err: logger.warning(f"Error in update_callback: {cb_err}")
    
    yield from _send_update("Reading EPUB data...")
    chapters_data = epub_processor.extract_chapters_from_epub(epub_filepath)
    if not chapters_data: raise ValueError("EPUB extraction failed.")
    chapters_data = chapters_data if isinstance(chapters_data, dict) else {"Chapter_01_Fallback": chapters_data}
    actual_keys_to_process = [k for k in selected_chapter_keys if k in chapters_data]
    if not actual_keys_to_process: raise ValueError("None of the selected chapters were found.")
    
    processed_count, failed_chapters, skipped_count, final_mp3s = 0, [], 0, []
    total_chapters = len(actual_keys_to_process)
    yield from _send_update(f"Starting processing for {total_chapters} chapters...")

    failed_chunks_data = {"book_title": os.path.basename(book_final_output_dir), "conversion_date": datetime.now().strftime("%Y-%m-%d"), "total_chapters": total_chapters, "total_chunks": 0, "failed_chunks_count": 0, "chapters_with_errors": {}, "failed_chunks_text": {}, "error_types": {}, "model_used": selected_model.split(" (da scaricare)")[0] if " (da scaricare)" in selected_model else selected_model, "language": selected_lang, "tts_params": final_tts_params, "technical_voice_id": technical_voice_id, "proc_opts": final_proc_opts, "note": "File generato automaticamente dal sistema recovery errori."}
    
    for i, chapter_key in enumerate(actual_keys_to_process):
        yield from _send_update(f"Processing Chapter {i+1}/{total_chapters}: '{chapter_key}'...")
        chapter_text = chapters_data.get(chapter_key, "").strip()
        if not chapter_text:
            skipped_count += 1; logger.warning(f"Chapter '{chapter_key}' is empty. Skipping.")
            continue
        
        chapter_title_sanitized = utils.sanitize_filename(chapter_key)
        chapter_chunk_dir = os.path.join(book_chunk_output_dir, chapter_title_sanitized)
        os.makedirs(chapter_chunk_dir, exist_ok=True)
        
        text_chunks = epub_processor.chunk_chapter_text(chapter_text, **final_proc_opts)
        if not text_chunks:
            skipped_count += 1; logger.warning(f"No text chunks generated for chapter '{chapter_key}'. Skipping.")
            continue
        
        total_chunks = len(text_chunks)
        failed_chunks_data["total_chunks"] += total_chunks
        yield from _send_update(f"Generating {total_chunks} audio chunks for chapter '{chapter_key}'...")
        
        generated_chunk_files, chapter_failed_indices, chapter_failed_texts, chapter_error_types = [], [], {}, []
        
        for j, chunk_text in enumerate(text_chunks):
            yield from _send_update(f"  Synthesizing chunk {j+1}/{total_chunks}...")
            chunk_output_path = os.path.join(chapter_chunk_dir, f"chunk_{j+1:04d}.wav")
            
            extra_params = {}
            if selected_model == "XTTSv2": extra_params.update({"language": selected_lang, "speaker_wav": technical_voice_id, "use_tts_splitting": True, "sentence_separator": final_proc_opts.get("sentence_separator", ".")})
            elif selected_model == "Kokoro": extra_params.update({"voice_id": technical_voice_id, "language_code": selected_lang})
            elif selected_model.startswith("VibeVoice"): extra_params.update({"language": selected_lang, "speaker_wav": technical_voice_id})
            elif selected_model.startswith("Qwen3-TTS"): extra_params.update({"language": selected_lang})
            
            all_params = {**final_tts_params, **extra_params}
            
            if not tts_handler.synthesize_audio(selected_model, model_instance_wrapper, chunk_text, chunk_output_path, **all_params):
                logger.error(f"Failed to synthesize chunk {j+1} of chapter '{chapter_key}'.")
                chapter_failed_indices.append(j+1); chapter_failed_texts[str(j+1)] = chunk_text; chapter_error_types.append("synthesis_failed")
            else:
                generated_chunk_files.append(chunk_output_path)

        if chapter_failed_indices:
            failed_chunks_data["chapters_with_errors"][chapter_title_sanitized] = chapter_failed_indices
            failed_chunks_data["failed_chunks_text"][chapter_title_sanitized] = chapter_failed_texts
            failed_chunks_data["error_types"][chapter_title_sanitized] = chapter_error_types
            failed_chunks_data["failed_chunks_count"] += len(chapter_failed_indices)
        
        if generated_chunk_files:
            yield from _send_update(f"Merging {len(generated_chunk_files)} chunks for chapter '{chapter_key}'...")
            final_mp3_path = os.path.join(book_final_output_dir, f"{chapter_title_sanitized}.mp3")
            if ffmpeg_wrapper.merge_audio_files_ffmpeg(chapter_chunk_dir, final_mp3_path, config.DEFAULT_FFMPEG_EXE):
                processed_count += 1; final_mp3s.append(final_mp3_path)
            else: failed_chapters.append(chapter_key)
        else: failed_chapters.append(chapter_key)
    
    if failed_chunks_data["failed_chunks_count"] > 0:
        yield from _send_update(f"Saving failed chunks data ({failed_chunks_data['failed_chunks_count']} chunks)...")
        save_failed_chunks_json(failed_chunks_data["book_title"], failed_chunks_data)
        logger.info(f"Saved failed_chunks.json with {failed_chunks_data['failed_chunks_count']} failed chunks.")
            
    final_message = f"Processing complete. Success: {processed_count}, Failed: {len(failed_chapters)}, Skipped: {skipped_count}"
    if failed_chunks_data["failed_chunks_count"] > 0: final_message += f"\n⚠️ {failed_chunks_data['failed_chunks_count']} chunk falliti salvati per recovery."
    return final_message, final_mp3s

def run_demo_gradio(demo_text, selected_model, xtts_wav_file, piper_kokoro_voice, xtts_temp, xtts_speed, xtts_rep_pen, piper_speed, piper_noise_scale, piper_noise_scale_w, kokoro_speed, replace_guillemets_demo, separator_dropdown, qwen_mode_radio, qwen_custom_voice_dropdown, qwen_custom_language_dropdown, qwen_custom_instruct_textbox, qwen_clone_ref_audio, qwen_clone_ref_text, qwen_clone_fast_mode_checkbox, qwen_clone_language_dropdown, qwen_design_instruct_textbox, qwen_design_language_dropdown, qwen_speed_slider, qwen_pitch_slider, qwen_volume_slider, qwen_temperature_slider, qwen_top_p_slider, qwen_top_k_slider, qwen_repetition_penalty_slider, qwen_seed_number, vibevoice_temp, vibevoice_top_p, vibevoice_cfg_scale, vibevoice_diffusion_steps, vibevoice_speed_factor, vibevoice_seed, vibevoice_use_sampling, vibevoice_top_k, xtts_lang, kokoro_lang, vibevoice_lang, vibevoice_realtime_speaker, vibevoice_realtime_cfg_scale, vibevoice_realtime_ddpm_steps, vibevoice_realtime_seed, vibevoice_realtime_temperature, vibevoice_realtime_top_p, vibevoice_realtime_top_k, xtts_top_k_slider, xtts_top_p_slider, xtts_length_penalty_slider, xtts_gpt_cond_len_slider):
    yield "Starting demo...", gr.update(value=None, visible=False)
    try:
        if not demo_text or not demo_text.strip(): raise ValueError("Please enter some text.")
        
        selected_lang = config.DEFAULT_LANGUAGE
        if selected_model == "XTTSv2" and xtts_lang: selected_lang = xtts_lang
        elif selected_model == "Kokoro" and kokoro_lang: selected_lang = kokoro_lang
        elif selected_model.startswith("VibeVoice") and vibevoice_lang: selected_lang = vibevoice_lang
        
        technical_voice_id, final_tts_params, error = _prepare_tts_config(selected_lang, selected_model, xtts_wav_file, piper_kokoro_voice, xtts_temp, xtts_speed, xtts_rep_pen, xtts_top_k_slider, xtts_top_p_slider, xtts_length_penalty_slider, xtts_gpt_cond_len_slider, piper_speed, piper_noise_scale, piper_noise_scale_w, kokoro_speed, vibevoice_temp, vibevoice_top_p, vibevoice_cfg_scale, vibevoice_diffusion_steps, vibevoice_speed_factor, vibevoice_seed, vibevoice_use_sampling, vibevoice_top_k, vibevoice_realtime_speaker, vibevoice_realtime_cfg_scale, vibevoice_realtime_ddpm_steps, vibevoice_realtime_temperature, vibevoice_realtime_top_p, vibevoice_realtime_top_k, vibevoice_realtime_seed)
        if error: raise ValueError(error)
        
        if selected_model.startswith("Qwen3-TTS"):
            final_tts_params["qwen_active_tab"] = "Voce Singola"
            final_tts_params["qwen_mode"] = {"Custom Voice": "custom", "Voice Clone": "clone", "Voice Design": "design"}.get(qwen_mode_radio)
            if final_tts_params["qwen_mode"] is None: final_tts_params["qwen_mode"] = "custom"
            qwen_params = {"temperature": qwen_temperature_slider, "top_p": qwen_top_p_slider, "top_k": _safe_top_k_conversion(qwen_top_k_slider), "repetition_penalty": _safe_float_conversion(qwen_repetition_penalty_slider, default=1.1, min_val=0.001), "seed": _safe_int_conversion(qwen_seed_number), "speed": qwen_speed_slider, "pitch": qwen_pitch_slider, "volume": qwen_volume_slider, "model_size": final_tts_params.get("qwen_model_size", "0.6B"), "model_type": final_tts_params.get("qwen_model_type", "base")}
            if final_tts_params["qwen_mode"] == "custom":
                qwen_params.update({"speaker": qwen_custom_voice_dropdown, "language": qwen_custom_language_dropdown, "instruct": qwen_custom_instruct_textbox})
            elif final_tts_params["qwen_mode"] == "clone":
                if not qwen_clone_ref_audio: raise ValueError("Per la modalità Voice Clone, è necessario caricare un file audio di riferimento.")
                ref_audio_path = _extract_file_path(qwen_clone_ref_audio)
                fast_mode = bool(qwen_clone_fast_mode_checkbox)
                qwen_params.update({"ref_audio": ref_audio_path, "x_vector_only_mode": fast_mode})
                if not fast_mode:
                    if not qwen_clone_ref_text: raise ValueError("Per la modalità Clone a qualità massima, è necessaria la trascrizione del testo.")
                    qwen_params["ref_text"] = qwen_clone_ref_text
            elif final_tts_params["qwen_mode"] == "design":
                language_val = qwen_design_language_dropdown
                qwen_params.update({"instruct": qwen_design_instruct_textbox, "language": str(language_val) if language_val is not None else "Italian"})
                final_tts_params["qwen_model_type"] = "voice_design"
            final_tts_params["qwen_params"] = qwen_params
        
        model_instance, error = _load_tts_model_instance(selected_model, selected_lang, technical_voice_id)
        if error: raise RuntimeError(f"Model loading failed: {error}")

        processed_text = utils.replace_guillemets_text(demo_text.strip()) if replace_guillemets_demo else demo_text.strip()
        demo_output_path = os.path.join(config.DEMO_OUTPUT_DIR, f"demo_{int(time.time())}.wav")
        sep_value = next((val for name, val in SENTENCE_SEPARATOR_OPTIONS if name == separator_dropdown), ".")
        
        extra_params = {}
        if selected_model == "XTTSv2": extra_params.update({"language": selected_lang, "speaker_wav": technical_voice_id, "use_tts_splitting": False, "sentence_separator": sep_value})
        elif selected_model == "Kokoro": extra_params.update({"voice_id": technical_voice_id, "language_code": selected_lang})
        elif selected_model.startswith("VibeVoice"): extra_params.update({"language": selected_lang, "speaker_wav": technical_voice_id})
        elif selected_model.startswith("Qwen3-TTS"): extra_params.update({"language": selected_lang, "speaker_wav": technical_voice_id})
        
        all_params = {**extra_params, **final_tts_params}
        start_time = time.time()
        success = tts_handler.synthesize_audio(selected_model, model_instance, processed_text, demo_output_path, **all_params)
        duration = time.time() - start_time

        if success: yield f"Demo generated in {duration:.2f}s.", gr.Audio(value=demo_output_path, label="Demo Output", visible=True)
        else: raise RuntimeError("Synthesis failed.")
    except Exception as e:
        logging.error(f"FATAL ERROR during demo: {e}", exc_info=True)
        yield f"ERROR: {e}", gr.update(value=None, visible=False)

def run_generation(selected_model, xtts_wav_file, piper_kokoro_voice, xtts_temp, xtts_speed, xtts_rep_pen, piper_speed, piper_noise_scale, piper_noise_scale_w, kokoro_speed, epub_file_obj, audiobook_title_in, replace_guillemets, chunking_strategy, separator_dropdown, min_words, max_words, max_chars, delete_chunks, selected_chapter_keys, qwen_mode_radio, qwen_custom_voice_dropdown, qwen_custom_language_dropdown, qwen_custom_instruct_textbox, qwen_clone_ref_audio, qwen_clone_ref_text, qwen_clone_fast_mode_checkbox, qwen_clone_language_dropdown, qwen_design_instruct_textbox, qwen_design_language_dropdown, qwen_speed_slider, qwen_pitch_slider, qwen_volume_slider, qwen_temperature_slider, qwen_top_p_slider, qwen_top_k_slider, qwen_repetition_penalty_slider, qwen_seed_number, shared_state, xtts_lang, kokoro_lang, vibevoice_lang, vibevoice_temp_slider, vibevoice_top_p_slider, vibevoice_cfg_scale_slider, vibevoice_diffusion_steps_slider, vibevoice_speed_factor_slider, vibevoice_seed_number, vibevoice_use_sampling_checkbox, vibevoice_top_k_slider, vibevoice_realtime_speaker_dropdown, vibevoice_realtime_cfg_scale_slider, vibevoice_realtime_ddpm_steps_slider, vibevoice_realtime_seed_number, vibevoice_realtime_temperature_slider, vibevoice_realtime_top_p_slider, vibevoice_realtime_top_k_slider, xtts_top_k_slider, xtts_top_p_slider, xtts_length_penalty_slider, xtts_gpt_cond_len_slider):
    status_log, logger, log_file_path = [], logging.getLogger('dummy'), None
    def _update_status(message, level=logging.INFO):
        status_log.append(message)
        if logger.name != 'dummy': logger.log(level, message)
        yield "\n".join(status_log), gr.update(), gr.update()

    yield from _update_status("Starting generation process...")
    if HAS_DEPENDENCIES_TAB:
        deps_status = dependencies_tab.check_external_dependencies()
        ffmpeg_status = "✅ Presente" if deps_status["ffmpeg"]["present"] else "❌ Mancante"
        sox_status = "✅ Presente" if deps_status["sox"]["present"] else "⚠️ Mancante"
        yield from _update_status(f"📦 Stato dipendenze: FFmpeg: {ffmpeg_status}, SoX: {sox_status}")
        if not deps_status["ffmpeg"]["present"]: yield from _update_status("⚠️ FFmpeg non trovato. L'app userà un metodo alternativo (più lento) per unire i file audio.")
        if not deps_status["sox"]["present"]: yield from _update_status("⚠️ SoX non trovato. Alcune funzionalità audio potrebbero non essere disponibili.")
    else:
        yield from _update_status("ℹ️ Verifica dipendenze non disponibile (modulo mancante)")
    try:
        selected_lang = config.DEFAULT_LANGUAGE
        if selected_model == "XTTSv2" and xtts_lang: selected_lang = xtts_lang
        elif selected_model == "Kokoro" and kokoro_lang: selected_lang = kokoro_lang
        elif selected_model.startswith("VibeVoice") and vibevoice_lang: selected_lang = vibevoice_lang
        
        if not all([selected_model, epub_file_obj, selected_chapter_keys]): raise ValueError("Missing one or more required inputs.")
        
        technical_voice_id, final_tts_params, error = _prepare_tts_config(selected_lang, selected_model, xtts_wav_file, piper_kokoro_voice, xtts_temp, xtts_speed, xtts_rep_pen, xtts_top_k_slider, xtts_top_p_slider, xtts_length_penalty_slider, xtts_gpt_cond_len_slider, piper_speed, piper_noise_scale, piper_noise_scale_w, kokoro_speed, vibevoice_temp_slider, vibevoice_top_p_slider, vibevoice_cfg_scale_slider, vibevoice_diffusion_steps_slider, vibevoice_speed_factor_slider, vibevoice_seed_number, vibevoice_use_sampling_checkbox, vibevoice_top_k_slider, vibevoice_realtime_speaker_dropdown, vibevoice_realtime_cfg_scale_slider, vibevoice_realtime_ddpm_steps_slider, vibevoice_realtime_temperature_slider, vibevoice_realtime_top_p_slider, vibevoice_realtime_top_k_slider, vibevoice_realtime_seed_number)
        if error: raise ValueError(error)
        
        if selected_model.startswith("Qwen3-TTS"):
            final_tts_params["qwen_active_tab"] = "Voce Singola"
            final_tts_params["qwen_mode"] = {"Custom Voice": "custom", "Voice Clone": "clone", "Voice Design": "design"}.get(qwen_mode_radio)
            qwen_params = {"temperature": qwen_temperature_slider, "top_p": qwen_top_p_slider, "top_k": _safe_top_k_conversion(qwen_top_k_slider), "repetition_penalty": _safe_float_conversion(qwen_repetition_penalty_slider, default=1.1, min_val=0.001), "seed": _safe_int_conversion(qwen_seed_number), "speed": qwen_speed_slider, "pitch": qwen_pitch_slider, "volume": qwen_volume_slider, "model_size": final_tts_params.get("qwen_model_size", "0.6B"), "model_type": final_tts_params.get("qwen_model_type", "base")}
            if final_tts_params["qwen_mode"] == "custom":
                qwen_params.update({"speaker": qwen_custom_voice_dropdown, "language": qwen_custom_language_dropdown, "instruct": qwen_custom_instruct_textbox})
            elif final_tts_params["qwen_mode"] == "clone":
                if not qwen_clone_ref_audio: raise ValueError("Per la modalità Voice Clone, è necessario caricare un file audio di riferimento.")
                ref_audio_path = _extract_file_path(qwen_clone_ref_audio)
                fast_mode = bool(qwen_clone_fast_mode_checkbox)
                qwen_params.update({"ref_audio": ref_audio_path, "x_vector_only_mode": fast_mode})
                if not fast_mode:
                    if not qwen_clone_ref_text: raise ValueError("Per la modalità Clone a qualità massima, è necessaria la trascrizione del testo.")
                    qwen_params["ref_text"] = qwen_clone_ref_text
            elif final_tts_params["qwen_mode"] == "design":
                language_val = qwen_design_language_dropdown
                qwen_params.update({"instruct": qwen_design_instruct_textbox, "language": str(language_val) if language_val is not None else "Italian"})
                final_tts_params["qwen_model_type"] = "voice_design"
            final_tts_params["qwen_params"] = qwen_params
        
        model_instance, error = _load_tts_model_instance(selected_model, selected_lang, technical_voice_id)
        if error: raise RuntimeError(f"Model loading failed: {error}")
        
        audiobook_title = utils.sanitize_filename(audiobook_title_in.strip() or os.path.splitext(os.path.basename(epub_file_obj.name))[0])
        book_final_output_dir = os.path.join(config.OUTPUT_BASE_DIR, audiobook_title)
        book_chunk_output_dir = os.path.join(config.CHUNK_OUTPUT_BASE_DIR, audiobook_title)
        os.makedirs(book_final_output_dir, exist_ok=True); os.makedirs(book_chunk_output_dir, exist_ok=True)
        logger, log_file_path = setup_file_logger(book_final_output_dir)

        sep_value = next((val for name, val in SENTENCE_SEPARATOR_OPTIONS if name == separator_dropdown), ".")
        proc_opts = {"use_char_limit_chunking": chunking_strategy == CHUNKING_STRATEGIES[1], "max_chars_per_chunk": max_chars, "min_words_approx": min_words, "max_words_approx": max_words, "sentence_separator": sep_value, "replace_guillemets": replace_guillemets}

        if selected_model.startswith("VibeVoice"):
            logger.info(f"Modello {selected_model} selezionato. Forzatura di una strategia di chunking a testo lungo.")
            proc_opts.update({"use_char_limit_chunking": True, "max_chars_per_chunk": 750, "min_words_approx": 0, "max_words_approx": 0})
        if selected_model.startswith("Qwen3-TTS"):
            logger.info("Modello Qwen3-TTS selezionato. Configurazione ottimale per Qwen3-TTS.")
            proc_opts.update({"use_char_limit_chunking": True, "max_chars_per_chunk": 800, "min_words_approx": 0, "max_words_approx": 0})

        final_message, final_mp3s = yield from _process_ebook_chapters(epub_file_obj.name, book_final_output_dir, book_chunk_output_dir, model_instance, selected_lang, selected_model, technical_voice_id, final_tts_params, proc_opts, selected_chapter_keys, _update_status, logger)
        status_log.append(final_message)

        has_failed_chunks = os.path.exists(os.path.join(book_final_output_dir, "failed_chunks.json"))
        if delete_chunks and os.path.isdir(book_chunk_output_dir):
            if has_failed_chunks:
                yield from _update_status("⚠️ Chunk falliti rilevati. Directory chunk preservata per recovery.")
                status_log.append("Cleanup skipped (failed chunks present).")
            else:
                yield from _update_status("Cleaning up intermediate chunk files...")
                shutil.rmtree(book_chunk_output_dir)
                status_log.append("Cleanup complete.")

        first_mp3 = final_mp3s[0] if final_mp3s else None
        yield "\n".join(status_log), gr.Audio(value=first_mp3, label="First Chapter", visible=bool(first_mp3)), gr.Textbox(value=log_file_path, visible=True)

    except Exception as e:
        error_msg = f"FATAL ERROR: {e}"
        logging.error(error_msg, exc_info=True)
        if logger.name != 'dummy': logger.error(error_msg, exc_info=True)
        status_log.append(error_msg)
        yield "\n".join(status_log), gr.update(visible=False), gr.Textbox(value=log_file_path, label="Log File (Error)", visible=True)

def run_test_generation(selected_model, xtts_wav_file, piper_kokoro_voice, xtts_temp, xtts_speed, xtts_rep_pen, piper_speed, piper_noise_scale, piper_noise_scale_w, kokoro_speed, replace_guillemets, chunking_strategy, separator_dropdown, min_words, max_words, max_chars, shared_state, xtts_lang=None, kokoro_lang=None, vibevoice_lang=None, qwen_custom_language_dropdown="Auto", qwen_clone_language_dropdown="Auto", qwen_design_language_dropdown="Italian"):
    class MockGradioFile:
        def __init__(self, name): self.name = name
    
    selected_lang = config.DEFAULT_LANGUAGE
    if selected_model == "XTTSv2" and xtts_lang: selected_lang = xtts_lang
    elif selected_model == "Kokoro" and kokoro_lang: selected_lang = kokoro_lang
    elif selected_model.startswith("VibeVoice") and vibevoice_lang: selected_lang = vibevoice_lang
    elif selected_model.startswith("Qwen3-TTS"): selected_lang = qwen_custom_language_dropdown
    
    lang_suffix = map_language_to_test_file(selected_lang, selected_model)
    test_epub_path = os.path.join(config.TEST_FILES_DIR, f"test_ebook_{lang_suffix}.epub")
    
    if not os.path.exists(test_epub_path):
        test_epub_path = os.path.join(config.TEST_FILES_DIR, "test_ebook_en.epub")
        if not os.path.exists(test_epub_path):
            yield f"ERROR: Test file not found for lang '{selected_lang}' (mapped to '{lang_suffix}') and fallback 'en'.", gr.update(visible=False)
            return
        else:
            logging.warning(f"Test file for lang '{selected_lang}' not found, using fallback 'en'")

    test_epub_obj = MockGradioFile(test_epub_path)
    test_chapters = epub_processor.extract_chapters_from_epub(test_epub_path)
    test_chapter_keys = list(test_chapters.keys()) if isinstance(test_chapters, dict) else ["Chapter_01_Fallback"]
    
    # Default values for missing Qwen and VibeVoice params
    qwen_mode_radio, qwen_custom_voice_dropdown, qwen_custom_instruct_textbox, qwen_clone_ref_audio, qwen_clone_ref_text, qwen_clone_fast_mode_checkbox, qwen_design_instruct_textbox, qwen_speed_slider, qwen_pitch_slider, qwen_volume_slider, qwen_temperature_slider, qwen_top_p_slider, qwen_top_k_slider, qwen_repetition_penalty_slider, qwen_seed_number = "Custom Voice", "Serena", "", None, "", False, "", 1.0, 0, 0, 0.7, 0.8, 20, 1.1, None
    vibevoice_temp_slider, vibevoice_top_p_slider, vibevoice_cfg_scale_slider, vibevoice_diffusion_steps_slider, vibevoice_speed_factor_slider, vibevoice_seed_number, vibevoice_use_sampling_checkbox, vibevoice_top_k_slider = 1.0, 0.9, 1.3, 15, 1.0, None, True, 0
    vibevoice_realtime_speaker_dropdown, vibevoice_realtime_cfg_scale_slider, vibevoice_realtime_ddpm_steps_slider, vibevoice_realtime_seed_number, vibevoice_realtime_temperature_slider, vibevoice_realtime_top_p_slider, vibevoice_realtime_top_k_slider = "it-Spk0_woman", 1.3, 5, None, 1.0, 0.9, 0
    xtts_top_k_slider, xtts_top_p_slider, xtts_length_penalty_slider, xtts_gpt_cond_len_slider = 50, 0.85, 1.0, 30
    
    gen = run_generation(selected_model, xtts_wav_file, piper_kokoro_voice, xtts_temp, xtts_speed, xtts_rep_pen, piper_speed, piper_noise_scale, piper_noise_scale_w, kokoro_speed, test_epub_obj, f"TEST_{lang_suffix}_{selected_model}", replace_guillemets, chunking_strategy, separator_dropdown, min_words, max_words, max_chars, False, test_chapter_keys, qwen_mode_radio, qwen_custom_voice_dropdown, qwen_custom_language_dropdown, qwen_custom_instruct_textbox, qwen_clone_ref_audio, qwen_clone_ref_text, qwen_clone_fast_mode_checkbox, qwen_clone_language_dropdown, qwen_design_instruct_textbox, qwen_design_language_dropdown, qwen_speed_slider, qwen_pitch_slider, qwen_volume_slider, qwen_temperature_slider, qwen_top_p_slider, qwen_top_k_slider, qwen_repetition_penalty_slider, qwen_seed_number, shared_state, xtts_lang, kokoro_lang, vibevoice_lang, vibevoice_temp_slider, vibevoice_top_p_slider, vibevoice_cfg_scale_slider, vibevoice_diffusion_steps_slider, vibevoice_speed_factor_slider, vibevoice_seed_number, vibevoice_use_sampling_checkbox, vibevoice_top_k_slider, vibevoice_realtime_speaker_dropdown, vibevoice_realtime_cfg_scale_slider, vibevoice_realtime_ddpm_steps_slider, vibevoice_realtime_seed_number, vibevoice_realtime_temperature_slider, vibevoice_realtime_top_p_slider, vibevoice_realtime_top_k_slider, xtts_top_k_slider, xtts_top_p_slider, xtts_length_penalty_slider, xtts_gpt_cond_len_slider)
    for status, audio, _ in gen:
        yield status, audio

def handle_epub_upload(epub_file_obj):
    if not epub_file_obj: return gr.update(value=""), [], gr.update(value="EPUB removed.", visible=True), gr.update(choices=[], value=[])
    try:
        title = utils.sanitize_filename(os.path.splitext(os.path.basename(epub_file_obj.name))[0])
        chapters = epub_processor.extract_chapters_from_epub(epub_file_obj.name)
        if not chapters: raise ValueError("EPUB extraction failed.")
        chapter_data = chapters if isinstance(chapters, dict) else {"Chapter_01_Fallback": chapters}
        chapter_list = list(chapter_data.keys())
        if not chapter_list: raise ValueError("No chapters found.")
        return gr.update(value=title), chapter_list, gr.update(value=f"Loaded {len(chapter_list)} chapter(s).", visible=True), gr.update(choices=chapter_list, value=chapter_list, interactive=True)
    except Exception as e:
        logging.error(f"ERROR handling EPUB: {e}", exc_info=True)
        return gr.update(value=""), [], gr.update(value=str(e), visible=True), gr.update(choices=[], value=[], label="Error loading chapters!")


with gr.Blocks(title="Audiobook Generator EVO") as app:
    gr.Markdown("# Audiobook Generator EVO")
    
    shared_state, chapter_list_state = gr.State({}), gr.State([])
    
    with gr.Tabs():
        # --- Crea le schede chiamando le funzioni dai moduli importati ---
        (
            model_radio, xtts_voice_file, xtts_params_group, piper_params_group, kokoro_params_group, 
            qwen_params_group, vibevoice_params_group, vibevoice_realtime_params_group,
            xtts_lang_dropdown, xtts_temp_slider, xtts_speed_slider, xtts_rep_pen_slider, 
            xtts_top_k_slider, xtts_top_p_slider, xtts_length_penalty_slider, xtts_gpt_cond_len_slider,
            piper_speed_slider, piper_noise_scale_slider, piper_noise_scale_w_slider,
            kokoro_lang_dropdown, piper_kokoro_voice_dropdown, kokoro_speed_slider,
            vibevoice_lang_dropdown, vibevoice_temp_slider, vibevoice_cfg_scale_slider, vibevoice_diffusion_steps_slider,
            vibevoice_speed_factor_slider, vibevoice_top_p_slider, vibevoice_top_k_slider, vibevoice_seed_number,
            vibevoice_use_sampling_checkbox, vibevoice_realtime_speaker_dropdown, vibevoice_realtime_cfg_scale_slider,
            vibevoice_realtime_ddpm_steps_slider, vibevoice_realtime_temperature_slider, vibevoice_realtime_top_p_slider,
            vibevoice_realtime_top_k_slider, vibevoice_realtime_seed_number,
            qwen_mode_radio, qwen_custom_group, qwen_clone_group, qwen_design_group,
            qwen_custom_voice_dropdown, qwen_custom_language_dropdown, qwen_custom_instruct_textbox,
            qwen_clone_ref_audio, qwen_clone_ref_text, qwen_clone_fast_mode_checkbox, qwen_clone_language_dropdown,
            qwen_design_instruct_textbox, qwen_design_language_dropdown, qwen_speed_slider,
            qwen_pitch_slider, qwen_volume_slider, qwen_temperature_slider, qwen_top_p_slider,
            qwen_top_k_slider, qwen_repetition_penalty_slider, qwen_seed_number
        ) = create_configuration_tab(TTS_MODELS, MODEL_LANGUAGES, config)

        (
            epub_upload, audiobook_title_textbox, epub_load_notification, replace_guillemets_checkbox,
            separator_dropdown, chunking_strategy_radio, word_count_group, min_words_number,
            max_words_number, char_limit_group, max_chars_number, model_info_note
        ) = create_epub_options_tab(config, SENTENCE_SEPARATOR_OPTIONS, DEFAULT_SEPARATOR_DISPLAY, CHUNKING_STRATEGIES, DEFAULT_CHUNKING_STRATEGY)

        (
            select_all_toggle_btn, invert_selection_btn, chapter_selector, generate_button,
            stop_generation_button, delete_chunks_checkbox, status_textbox,
            output_audio_player, output_logfile_display
        ) = create_generate_tab(config)

        (
            demo_text_input, demo_generate_button, demo_status_textbox, demo_audio_output,
            test_file_button, test_status_textbox, test_output_audio_player
        ) = create_demo_test_tab()
        
        if HAS_RECOVERY_TAB: recovery_tab.create_recovery_tab()
        if HAS_DEPENDENCIES_TAB: dependencies_tab.create_dependencies_tab()
        if HAS_MODELS_TAB: models_tab.create_models_tab()

    app.queue()

    # ### GESTIONE DEGLI EVENTI ###
    model_radio.change(fn=update_parameter_visibility, inputs=[model_radio], outputs=[xtts_params_group, piper_params_group, kokoro_params_group, qwen_params_group, vibevoice_params_group, vibevoice_realtime_params_group])
    chunking_strategy_radio.change(fn=update_chunking_options_visibility, inputs=[chunking_strategy_radio], outputs=[word_count_group, char_limit_group])
    epub_upload.upload(fn=handle_epub_upload, inputs=[epub_upload], outputs=[audiobook_title_textbox, chapter_list_state, epub_load_notification, chapter_selector])

    model_radio.change(fn=update_qwen_panel, inputs=[model_radio], outputs=[qwen_params_group, qwen_mode_radio, qwen_custom_group, qwen_clone_group, qwen_design_group])
    qwen_mode_radio.change(fn=update_qwen_mode_visibility, inputs=[qwen_mode_radio], outputs=[qwen_custom_group, qwen_clone_group, qwen_design_group])
    qwen_clone_fast_mode_checkbox.change(fn=update_qwen_clone_text_visibility, inputs=[qwen_clone_fast_mode_checkbox], outputs=[qwen_clone_ref_text])
    
    select_all_toggle_btn.click(fn=toggle_select_all_chapters, inputs=[chapter_list_state, chapter_selector], outputs=[chapter_selector, select_all_toggle_btn])
    invert_selection_btn.click(fn=invert_chapter_selection, inputs=[chapter_list_state, chapter_selector], outputs=[chapter_selector])
    
    model_radio.change(fn=update_model_specific_options, inputs=[model_radio, xtts_lang_dropdown, kokoro_lang_dropdown, vibevoice_lang_dropdown, chunking_strategy_radio, max_chars_number, separator_dropdown, replace_guillemets_checkbox], outputs=[chunking_strategy_radio, max_chars_number, separator_dropdown, replace_guillemets_checkbox, model_info_note])
    model_radio.change(fn=update_qwen_mode_for_model, inputs=[model_radio], outputs=[qwen_mode_radio])
    
    kokoro_lang_dropdown.change(fn=update_voice_options_for_model, inputs=[model_radio, xtts_lang_dropdown, kokoro_lang_dropdown, vibevoice_lang_dropdown], outputs=[xtts_voice_file, piper_kokoro_voice_dropdown])
    model_radio.change(fn=update_voice_options_for_model, inputs=[model_radio, xtts_lang_dropdown, kokoro_lang_dropdown, vibevoice_lang_dropdown], outputs=[xtts_voice_file, piper_kokoro_voice_dropdown])
    
    demo_inputs = [demo_text_input, model_radio, xtts_voice_file, piper_kokoro_voice_dropdown, xtts_temp_slider, xtts_speed_slider, xtts_rep_pen_slider, piper_speed_slider, piper_noise_scale_slider, piper_noise_scale_w_slider, kokoro_speed_slider, replace_guillemets_checkbox, separator_dropdown, qwen_mode_radio, qwen_custom_voice_dropdown, qwen_custom_language_dropdown, qwen_custom_instruct_textbox, qwen_clone_ref_audio, qwen_clone_ref_text, qwen_clone_fast_mode_checkbox, qwen_clone_language_dropdown, qwen_design_instruct_textbox, qwen_design_language_dropdown, qwen_speed_slider, qwen_pitch_slider, qwen_volume_slider, qwen_temperature_slider, qwen_top_p_slider, qwen_top_k_slider, qwen_repetition_penalty_slider, qwen_seed_number, vibevoice_temp_slider, vibevoice_top_p_slider, vibevoice_cfg_scale_slider, vibevoice_diffusion_steps_slider, vibevoice_speed_factor_slider, vibevoice_seed_number, vibevoice_use_sampling_checkbox, vibevoice_top_k_slider, xtts_lang_dropdown, kokoro_lang_dropdown, vibevoice_lang_dropdown, vibevoice_realtime_speaker_dropdown, vibevoice_realtime_cfg_scale_slider, vibevoice_realtime_ddpm_steps_slider, vibevoice_realtime_seed_number, vibevoice_realtime_temperature_slider, vibevoice_realtime_top_p_slider, vibevoice_realtime_top_k_slider, xtts_top_k_slider, xtts_top_p_slider, xtts_length_penalty_slider, xtts_gpt_cond_len_slider]
    demo_generate_button.click(fn=run_demo_gradio, inputs=demo_inputs, outputs=[demo_status_textbox, demo_audio_output], queue=True)
    
    gen_inputs = [model_radio, xtts_voice_file, piper_kokoro_voice_dropdown, xtts_temp_slider, xtts_speed_slider, xtts_rep_pen_slider, piper_speed_slider, piper_noise_scale_slider, piper_noise_scale_w_slider, kokoro_speed_slider, epub_upload, audiobook_title_textbox, replace_guillemets_checkbox, chunking_strategy_radio, separator_dropdown, min_words_number, max_words_number, max_chars_number, delete_chunks_checkbox, chapter_selector, qwen_mode_radio, qwen_custom_voice_dropdown, qwen_custom_language_dropdown, qwen_custom_instruct_textbox, qwen_clone_ref_audio, qwen_clone_ref_text, qwen_clone_fast_mode_checkbox, qwen_clone_language_dropdown, qwen_design_instruct_textbox, qwen_design_language_dropdown, qwen_speed_slider, qwen_pitch_slider, qwen_volume_slider, qwen_temperature_slider, qwen_top_p_slider, qwen_top_k_slider, qwen_repetition_penalty_slider, qwen_seed_number, shared_state, xtts_lang_dropdown, kokoro_lang_dropdown, vibevoice_lang_dropdown, vibevoice_temp_slider, vibevoice_top_p_slider, vibevoice_cfg_scale_slider, vibevoice_diffusion_steps_slider, vibevoice_speed_factor_slider, vibevoice_seed_number, vibevoice_use_sampling_checkbox, vibevoice_top_k_slider, vibevoice_realtime_speaker_dropdown, vibevoice_realtime_cfg_scale_slider, vibevoice_realtime_ddpm_steps_slider, vibevoice_realtime_seed_number, vibevoice_realtime_temperature_slider, vibevoice_realtime_top_p_slider, vibevoice_realtime_top_k_slider, xtts_top_k_slider, xtts_top_p_slider, xtts_length_penalty_slider, xtts_gpt_cond_len_slider]
    generate_button.click(fn=run_generation, inputs=gen_inputs, outputs=[status_textbox, output_audio_player, output_logfile_display], queue=True)
    
    stop_generation_button.click(fn=set_stop_flag, inputs=[], outputs=[status_textbox])

    test_inputs = [model_radio, xtts_voice_file, piper_kokoro_voice_dropdown, xtts_temp_slider, xtts_speed_slider, xtts_rep_pen_slider, piper_speed_slider, piper_noise_scale_slider, piper_noise_scale_w_slider, kokoro_speed_slider, replace_guillemets_checkbox, chunking_strategy_radio, separator_dropdown, min_words_number, max_words_number, max_chars_number, shared_state, xtts_lang_dropdown, kokoro_lang_dropdown, vibevoice_lang_dropdown, qwen_custom_language_dropdown, qwen_clone_language_dropdown, qwen_design_language_dropdown]
    test_file_button.click(fn=run_test_generation, inputs=test_inputs, outputs=[test_status_textbox, test_output_audio_player], queue=True)

if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
    app.launch()