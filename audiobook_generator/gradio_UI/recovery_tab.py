"""
Tab Recovery Errori per gestire errori di sintesi audio.
"""

import gradio as gr
import os
import json
import time
import logging
import shutil
from typing import List, Dict, Optional, Tuple
from datetime import datetime

# Import moduli interni
try:
    from audiobook_generator import tts_handler
    from audiobook_generator import config
    from audiobook_generator import ffmpeg_wrapper
    from audiobook_generator import epub_processor
    HAS_BACKEND = True
except ImportError as e:
    HAS_BACKEND = False
    logging.warning(f"Backend modules not available: {e}")

# Flag per controllo stop (condiviso con app_gradio.py)
STOP_FLAG_RECOVERY = False

def set_stop_flag_recovery():
    """Imposta il flag di stop a True per recovery"""
    global STOP_FLAG_RECOVERY
    STOP_FLAG_RECOVERY = True
    logging.info("Stop flag recovery impostato a True")
    return "Processo recovery in arresto..."

def reset_stop_flag_recovery():
    """Resetta il flag di stop a False per recovery"""
    global STOP_FLAG_RECOVERY
    STOP_FLAG_RECOVERY = False
    logging.info("Stop flag recovery resettato a False")
    return "Stop flag recovery resettato"

def check_stop_flag_recovery():
    """Controlla se il flag di stop è True per recovery"""
    global STOP_FLAG_RECOVERY
    return STOP_FLAG_RECOVERY

# Percorsi
GENERATED_AUDIOBOOKS_DIR = "Generated_Audiobooks"
INTERMEDIATE_CHUNKS_DIR = "Intermediate_Audio_Chunks"

def scan_audiobooks_with_errors() -> List[str]:
    """Scansiona Generated_Audiobooks/ e restituisce lista audiolibri con errori."""
    books_with_errors = []
    if not os.path.exists(GENERATED_AUDIOBOOKS_DIR):
        logging.info(f"Directory {GENERATED_AUDIOBOOKS_DIR} non trovata.")
        return books_with_errors
    
    logging.info(f"Scansione di {GENERATED_AUDIOBOOKS_DIR} per audiolibri con errori...")
    
    for item in os.listdir(GENERATED_AUDIOBOOKS_DIR):
        book_dir = os.path.join(GENERATED_AUDIOBOOKS_DIR, item)
        if os.path.isdir(book_dir):
            failed_chunks_path = os.path.join(book_dir, "failed_chunks.json")
            if os.path.exists(failed_chunks_path):
                try:
                    with open(failed_chunks_path, 'r', encoding='utf-8') as f:
                        data = json.load(f)
                        if data.get("chapters_with_errors"):
                            books_with_errors.append(item)
                            logging.info(f"Trovato audiolibro con errori: {item}")
                        else:
                            logging.info(f"Audiolibro {item} ha failed_chunks.json ma chapters_with_errors è vuoto")
                except Exception as e:
                    logging.error(f"Errore lettura {failed_chunks_path}: {e}")
            else:
                logging.debug(f"Audiolibro {item} non ha failed_chunks.json")
        else:
            logging.debug(f"Ignorato file (non directory): {item}")
    
    logging.info(f"Scansione completata. Trovati {len(books_with_errors)} audiolibri con errori: {books_with_errors}")
    return books_with_errors

def load_failed_chunks_json(book_name: str) -> Optional[Dict]:
    """Carica il file failed_chunks.json per un audiolibro."""
    if not book_name:
        return None
    json_path = os.path.join(GENERATED_AUDIOBOOKS_DIR, book_name, "failed_chunks.json")
    if not os.path.exists(json_path):
        return None
    try:
        with open(json_path, 'r', encoding='utf-8') as f:
            return json.load(f)
    except:
        return None

def get_chapters_with_errors(book_name: str) -> List[str]:
    """Restituisce lista capitoli con errori per un audiolibro."""
    data = load_failed_chunks_json(book_name)
    if not data:
        return []
    chapters = data.get("chapters_with_errors", {})
    return list(chapters.keys())

def get_failed_chunks_for_chapter(book_name: str, chapter_name: str) -> List[int]:
    """Restituisce lista indici chunk falliti per un capitolo."""
    data = load_failed_chunks_json(book_name)
    if not data:
        return []
    chapters = data.get("chapters_with_errors", {})
    return chapters.get(chapter_name, [])

def update_chapters_dropdown(book_name: str) -> gr.update:
    """Aggiorna dropdown capitoli in base all'audiolibro selezionato."""
    if not book_name:
        return gr.update(choices=[], value=None)
    chapters = get_chapters_with_errors(book_name)
    return gr.update(choices=chapters, value=chapters[0] if chapters else None)

def update_chunks_checkbox(book_name: str, chapter_name: str) -> gr.update:
    """Aggiorna checkbox chunk in base al capitolo selezionato."""
    if not book_name or not chapter_name:
        return gr.update(choices=[], value=[])
    chunk_indices = get_failed_chunks_for_chapter(book_name, chapter_name)
    
    if not chunk_indices:
        # Capitolo senza chunk falliti (tutti rigenerati con successo)
        # Mostra messaggio informativo invece di lista vuota
        choices = ["✅ All chunks regenerated successfully. Press 'Merge All Chunks' to generate the full chapter."]
        return gr.update(choices=choices, value=[], interactive=False, label="Chapter Status")
    else:
        # Crea etichette "Chunk 1", "Chunk 3", ecc.
        choices = [f"Chunk {idx}" for idx in chunk_indices]
        return gr.update(choices=choices, value=choices, interactive=True, label="Chunk Falliti")

def update_editing_dropdown(book_name: str, chapter_name: str) -> gr.update:
    """Aggiorna dropdown per editing manuale."""
    if not book_name or not chapter_name:
        return gr.update(choices=[], value=None)
    chunk_indices = get_failed_chunks_for_chapter(book_name, chapter_name)
    
    if not chunk_indices:
        # Capitolo senza chunk falliti (tutti rigenerati con successo)
        choices = ["No failed chunks available for editing"]
        return gr.update(choices=choices, value=choices[0], interactive=False)
    else:
        choices = [f"Chunk {idx}" for idx in chunk_indices]
        return gr.update(choices=choices, value=choices[0] if choices else None, interactive=True)

def retry_synthesis_real(book_name: str, chapter_name: str, selected_chunks: List[str]) -> Tuple[str, bool]:
    """Riprova sintesi per chunk falliti selezionati."""
    if not HAS_BACKEND:
        return "Backend modules not available. Cannot retry synthesis.", False
    
    if not book_name or not chapter_name or not selected_chunks:
        return "Seleziona audiolibro, capitolo e chunk.", False
    
    # Carica dati errore
    data = load_failed_chunks_json(book_name)
    if not data:
        return f"Nessun dato di errore trovato per '{book_name}'.", False
    
    # Estrai indici chunk (es. "Chunk 3" -> 3)
    chunk_indices = []
    for chunk_label in selected_chunks:
        try:
            idx = int(chunk_label.replace("Chunk ", ""))
            chunk_indices.append(idx)
        except ValueError:
            logging.warning(f"Formato chunk non valido: {chunk_label}")
    
    if not chunk_indices:
        return "Nessun chunk valido selezionato.", False
    
    # Verifica che i chunk siano effettivamente falliti
    chapters_with_errors = data.get("chapters_with_errors", {})
    actual_failed = chapters_with_errors.get(chapter_name, [])
    valid_indices = [idx for idx in chunk_indices if idx in actual_failed]
    
    if not valid_indices:
        return f"Nessuno dei chunk selezionati è fallito nel capitolo '{chapter_name}'.", False
    
    # Carica testo originale
    failed_texts = data.get("failed_chunks_text", {}).get(chapter_name, {})
    
    # Determina modello TTS originale e parametri
    model_used = data.get("model_used", "XTTSv2")
    language = data.get("language", "it")
    
    # Carica TUTTI i parametri salvati
    tts_params = data.get("tts_params", {})
    technical_voice_id = data.get("technical_voice_id")
    proc_opts = data.get("proc_opts", {})
    
    # Percorsi
    book_chunk_dir = os.path.join(INTERMEDIATE_CHUNKS_DIR, book_name, chapter_name)
    os.makedirs(book_chunk_dir, exist_ok=True)
    
    success_count = 0
    failed_count = 0
    error_messages = []
    
    # Carica modello una volta per tutti i chunk (performance)
    model_instance = None
    try:
        # Usa i parametri originali per caricare il modello
        if model_used.startswith("Qwen3-TTS"):
            # Per Qwen3-TTS, usa il plugin manager con i parametri originali
            model_instance = tts_handler.plugin_manager.load_model(model_used)
        elif model_used.startswith("VibeVoice"):
            # Per VibeVoice, usa il plugin manager
            model_instance = tts_handler.plugin_manager.load_model(model_used)
        elif model_used == "XTTSv2":
            model_instance = tts_handler.load_xtts_model()
        elif model_used == "Kokoro":
            model_instance = tts_handler.load_kokoro_model(language)
        else:
            model_instance = tts_handler.plugin_manager.load_model(model_used, language=language)
        
        if not model_instance:
            error_messages.append(f"Impossibile caricare modello '{model_used}'.")
            failed_count = len(valid_indices)
            result_msg = f"Errore: modello '{model_used}' non disponibile."
            return result_msg, False
    except Exception as e:
        error_messages.append(f"Errore caricamento modello: {str(e)}")
        failed_count = len(valid_indices)
        result_msg = f"Errore caricamento modello: {str(e)}"
        return result_msg, False
    
    for idx in valid_indices:
        chunk_text = failed_texts.get(str(idx))
        if not chunk_text:
            error_messages.append(f"Chunk {idx}: Testo originale non trovato.")
            failed_count += 1
            continue
        
        chunk_filename = f"chunk_{idx:04d}.wav"
        chunk_path = os.path.join(book_chunk_dir, chunk_filename)
        
        try:
            logging.info(f"Riprovando sintesi per chunk {idx} con modello {model_used}...")
            
            # Prepara i parametri per la sintesi
            all_params = {}
            
            # Aggiungi parametri specifici per modello
            if model_used == "XTTSv2":
                all_params.update({
                    "language": language,
                    "speaker_wav": technical_voice_id,
                    "use_tts_splitting": True,
                    "sentence_separator": proc_opts.get("sentence_separator", ".")
                })
                # Aggiungi parametri TTS se presenti
                if "temperature" in tts_params:
                    all_params["temperature"] = tts_params["temperature"]
                if "speed" in tts_params:
                    all_params["speed"] = tts_params["speed"]
                if "repetition_penalty" in tts_params:
                    all_params["repetition_penalty"] = tts_params["repetition_penalty"]
                    
            elif model_used == "Kokoro":
                all_params.update({
                    "voice_id": technical_voice_id,
                    "language_code": language
                })
                if "speed" in tts_params:
                    all_params["speed"] = tts_params["speed"]
                    
            elif model_used.startswith("VibeVoice"):
                all_params.update({
                    "language": language,
                    "speaker_wav": technical_voice_id
                })
                # Aggiungi tutti i parametri VibeVoice
                for key in ["temperature", "top_p", "cfg_scale", "diffusion_steps", 
                           "voice_speed_factor", "use_sampling", "seed"]:
                    if key in tts_params:
                        all_params[key] = tts_params[key]
                        
            elif model_used.startswith("Qwen3-TTS"):
                # Per Qwen3-TTS, usa i parametri salvati
                if "qwen_mode" in tts_params:
                    all_params["qwen_mode"] = tts_params["qwen_mode"]
                if "qwen_params" in tts_params:
                    all_params["qwen_params"] = tts_params["qwen_params"]
                # Aggiungi language per compatibilità
                all_params["language"] = language
            
            # Chiamata reale a TTS handler con TUTTI i parametri
            success = tts_handler.synthesize_audio(
                model_name=model_used,
                model_instance=model_instance,
                text=chunk_text,
                output_path=chunk_path,
                **all_params
            )
            
            if success:
                # Aggiorna JSON: rimuovi chunk dalla lista errori
                if idx in chapters_with_errors.get(chapter_name, []):
                    chapters_with_errors[chapter_name].remove(idx)
                success_count += 1
                logging.info(f"Chunk {idx} rigenerato con successo.")
            else:
                error_messages.append(f"Chunk {idx}: Sintesi fallita.")
                failed_count += 1
                
        except Exception as e:
            error_messages.append(f"Chunk {idx}: Errore - {str(e)}")
            failed_count += 1
            logging.error(f"Errore durante retry chunk {idx}: {e}")
    
    # Salva JSON aggiornato se ci sono successi
    if success_count > 0:
        data["chapters_with_errors"] = chapters_with_errors
        # NON rimuovere il capitolo anche se non ha più errori
        # Il capitolo deve rimanere nella lista per permettere all'utente di premere "Unisci tutti i chunk"
        # Il capitolo verrà rimosso solo dopo che l'utente preme "Unisci tutti i chunk" e l'unione ha successo
        
        json_path = os.path.join(GENERATED_AUDIOBOOKS_DIR, book_name, "failed_chunks.json")
        try:
            with open(json_path, 'w', encoding='utf-8') as f:
                json.dump(data, f, indent=2, ensure_ascii=False)
            logging.info(f"File JSON aggiornato: {json_path}")
        except Exception as e:
            logging.error(f"Errore salvataggio JSON: {e}")
    
    # Costruisci messaggio risultato
    result_msg = f"Risultato: {success_count} successi, {failed_count} falliti."
    if error_messages:
        result_msg += "\nErrori: " + "; ".join(error_messages[:3])  # Mostra solo primi 3 errori
    
    success = failed_count == 0
    return result_msg, success

def merge_all_chunks_real(book_name: str, chapter_name: str) -> Tuple[str, bool]:
    """Unisce tutti i chunk audio di un capitolo e aggiorna il JSON errori."""
    if not HAS_BACKEND:
        return "Backend modules not available. Cannot merge chunks.", False
    
    if not book_name or not chapter_name:
        return "Seleziona audiolibro e capitolo.", False
    
    # Percorsi
    book_chunk_dir = os.path.join(INTERMEDIATE_CHUNKS_DIR, book_name, chapter_name)
    if not os.path.exists(book_chunk_dir):
        return f"Directory chunk non trovata: {book_chunk_dir}", False
    
    # Cerca file chunk
    chunk_files = []
    for f in os.listdir(book_chunk_dir):
        if f.startswith("chunk_") and f.endswith(".wav"):
            chunk_files.append(os.path.join(book_chunk_dir, f))
    
    if not chunk_files:
        return f"Nessun file chunk trovato in {book_chunk_dir}", False
    
    # Ordina chunk per numero
    chunk_files.sort(key=lambda x: int(os.path.basename(x).split('_')[1].split('.')[0]))
    
    # Percorso output MP3
    output_dir = os.path.join(GENERATED_AUDIOBOOKS_DIR, book_name)
    os.makedirs(output_dir, exist_ok=True)
    output_path = os.path.join(output_dir, f"{chapter_name}.mp3")
    
    try:
        # Usa ffmpeg wrapper per unire
        success = ffmpeg_wrapper.merge_audio_files_ffmpeg(
            book_chunk_dir, 
            output_path,
            config.DEFAULT_FFMPEG_EXE if hasattr(config, 'DEFAULT_FFMPEG_EXE') else 'ffmpeg'
        )
        
        if success:
            # Aggiorna JSON: rimuovi capitolo dalla lista errori
            data = load_failed_chunks_json(book_name)
            if data and "chapters_with_errors" in data:
                chapters_with_errors = data.get("chapters_with_errors", {})
                if chapter_name in chapters_with_errors:
                    del chapters_with_errors[chapter_name]
                    logging.info(f"Capitolo '{chapter_name}' rimosso dalla lista errori dopo unione riuscita.")
                    
                    # Se chapters_with_errors è vuoto, elimina il file JSON
                    if not chapters_with_errors:
                        json_path = os.path.join(GENERATED_AUDIOBOOKS_DIR, book_name, "failed_chunks.json")
                        try:
                            os.remove(json_path)
                            logging.info(f"File JSON eliminato perché non ci sono più capitoli con errori: {json_path}")
                        except Exception as e:
                            logging.error(f"Errore eliminazione file JSON: {e}")
                    else:
                        # Salva JSON aggiornato
                        data["chapters_with_errors"] = chapters_with_errors
                        json_path = os.path.join(GENERATED_AUDIOBOOKS_DIR, book_name, "failed_chunks.json")
                        try:
                            with open(json_path, 'w', encoding='utf-8') as f:
                                json.dump(data, f, indent=2, ensure_ascii=False)
                            logging.info(f"File JSON aggiornato dopo unione: {json_path}")
                        except Exception as e:
                            logging.error(f"Errore salvataggio JSON: {e}")
            
            return f"Unione completata: {len(chunk_files)} chunk → {output_path}", True
        else:
            return "Unione fallita (ffmpeg error).", False
    except Exception as e:
        logging.error(f"Errore durante merge: {e}")
        return f"Errore durante unione: {str(e)}", False

def split_chunk_real(chunk_text: str) -> Tuple[str, bool]:
    """Suddivide un chunk di testo in parti più piccole."""
    if not chunk_text or not chunk_text.strip():
        return "Testo del chunk vuoto.", False
    
    try:
        # Usa epub_processor per suddividere
        sentences = epub_processor.split_text_into_sentences(chunk_text)
        
        if len(sentences) <= 1:
            return "Il testo ha solo una frase, non può essere suddiviso.", False
        
        # Crea anteprima suddivisione
        preview = f"Suddiviso in {len(sentences)} frasi:\n"
        for i, sent in enumerate(sentences[:3], 1):  # Mostra prime 3
            preview += f"{i}. {sent[:50]}...\n"
        if len(sentences) > 3:
            preview += f"... e altre {len(sentences) - 3} frasi."
        
        return preview, True
    except Exception as e:
        logging.error(f"Errore durante split chunk: {e}")
        return f"Errore durante suddivisione: {str(e)}", False

def manual_generate_real(chunk_text: str) -> Tuple[str, bool]:
    """Genera audio manualmente da testo."""
    if not chunk_text or not chunk_text.strip():
        return "Testo del chunk vuoto.", False
    
    # Placeholder - da implementare con UI per selezione modello/parametri
    return "Funzionalità di generazione manuale in sviluppo. Verrà implementata nella prossima fase.", True

# Funzioni placeholder mantenute per compatibilità
def retry_synthesis_placeholder(book_name: str, chapter_name: str, selected_chunks: List[str]):
    """Placeholder per ripetere sintesi."""
    result, _ = retry_synthesis_real(book_name, chapter_name, selected_chunks)
    return result

def merge_chunks_placeholder(book_name: str, chapter_name: str):
    """Placeholder per unire tutti i chunk."""
    result, _ = merge_all_chunks_real(book_name, chapter_name)
    return result

def split_chunk_placeholder(chunk_text: str):
    """Placeholder per suddividere chunk."""
    result, _ = split_chunk_real(chunk_text)
    return result

def manual_generate_placeholder(chunk_text: str):
    """Placeholder per generazione manuale."""
    result, _ = manual_generate_real(chunk_text)
    return result

def create_recovery_tab():
    '''Creates the Error Recovery tab.'''
    with gr.TabItem("5. Error Recovery") as tab:
        gr.Markdown("## 🔄 Synthesis Errors Recovery System")
        
        # --- Section 1: Audiobook Selection ---
        with gr.Row():
            book_dropdown = gr.Dropdown(
                label="Audiobooks with Errors",
                choices=scan_audiobooks_with_errors(),
                interactive=True,
                scale=3
            )
            chapter_dropdown = gr.Dropdown(
                label="Chapters with Errors",
                choices=[],
                interactive=True,
                scale=3
            )
            refresh_button = gr.Button("🔄 Refresh", variant="secondary", scale=1)
        
        # --- Section 2: Failed Chunks Management ---
        chunks_checkbox = gr.CheckboxGroup(
            label="Failed Chunks",
            choices=[],
            interactive=True
        )
        
        with gr.Row():
            retry_button = gr.Button("🔄 Retry Synthesis", variant="primary")
            merge_button = gr.Button("🔗 Merge All Chunks", variant="secondary")
            stop_recovery_button = gr.Button("⏹️ Stop", variant="stop", visible=True)
        
        # Textbox for results
        result_textbox = gr.Textbox(
            label="Result",
            interactive=False,
            lines=3,
            visible=False
        )
        
        # --- Section 3: Manual Editing (Accordion) ---
        with gr.Accordion("✏️ Manual Editing", open=False):
            with gr.Row():
                edit_chunk_dropdown = gr.Dropdown(
                    label="Select Chunk for Editing",
                    choices=[],
                    interactive=True
                )
            
            edit_textbox = gr.Textbox(
                label="Chunk Text",
                lines=6,
                interactive=True,
                placeholder="Text of the selected chunk..."
            )
            
            with gr.Row():
                split_button = gr.Button("✂️ Split Chunk", variant="secondary")
                manual_button = gr.Button("🎵 Generate Manually", variant="primary")
        
        # --- Eventi ---
        # Aggiorna capitoli quando cambia audiolibro
        book_dropdown.change(
            fn=update_chapters_dropdown,
            inputs=[book_dropdown],
            outputs=[chapter_dropdown]
        )
        
        # Aggiorna checkbox chunk quando cambia capitolo
        chapter_dropdown.change(
            fn=update_chunks_checkbox,
            inputs=[book_dropdown, chapter_dropdown],
            outputs=[chunks_checkbox]
        )
        
        # Aggiorna dropdown editing quando cambia capitolo
        chapter_dropdown.change(
            fn=update_editing_dropdown,
            inputs=[book_dropdown, chapter_dropdown],
            outputs=[edit_chunk_dropdown]
        )
        
        # Funzione per refresh
        def refresh_books():
            return gr.update(choices=scan_audiobooks_with_errors())
        
        refresh_button.click(
            fn=refresh_books,
            inputs=[],
            outputs=[book_dropdown]
        )
        
        # Funzioni reali con output
        retry_button.click(
            fn=retry_synthesis_placeholder,
            inputs=[book_dropdown, chapter_dropdown, chunks_checkbox],
            outputs=[result_textbox]
        ).then(
            fn=lambda: gr.update(visible=True),
            outputs=[result_textbox]
        )
        
        merge_button.click(
            fn=merge_chunks_placeholder,
            inputs=[book_dropdown, chapter_dropdown],
            outputs=[result_textbox]
        ).then(
            fn=lambda: gr.update(visible=True),
            outputs=[result_textbox]
        )
        
        split_button.click(
            fn=split_chunk_placeholder,
            inputs=[edit_textbox],
            outputs=[result_textbox]
        ).then(
            fn=lambda: gr.update(visible=True),
            outputs=[result_textbox]
        )
        
        manual_button.click(
            fn=manual_generate_placeholder,
            inputs=[edit_textbox],
            outputs=[result_textbox]
        ).then(
            fn=lambda: gr.update(visible=True),
            outputs=[result_textbox]
        )
        
        # Callback per pulsante stop recovery
        stop_recovery_button.click(
            fn=set_stop_flag_recovery,
            inputs=[],
            outputs=[result_textbox]
        )
    
    return tab

# Funzione per compatibilità
def dummy_recovery_function():
    return None