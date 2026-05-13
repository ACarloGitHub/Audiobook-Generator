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
Error Recovery tab for managing audio synthesis errors.
"""

import gradio as gr
import os
import json
import logging
from typing import List, Dict, Optional, Tuple

logger = logging.getLogger(__name__)

# Internal module imports
try:
    from audiobook_generator import plugin_manager
    from audiobook_generator import config
    from audiobook_generator import ffmpeg_wrapper
    from audiobook_generator import epub_processor
    HAS_BACKEND = True
except ImportError as e:
    HAS_BACKEND = False
    logging.warning(f"Backend modules not available: {e}")

import threading

# Stop flag for recovery control (shared with app_gradio.py, thread-safe)
stop_event_recovery = threading.Event()

def set_stop_flag_recovery():
    """Set the stop flag for recovery"""
    stop_event_recovery.set()
    logging.info("Recovery stop flag set")
    return "Recovery process stopping..."

def reset_stop_flag_recovery():
    """Reset the stop flag for recovery"""
    stop_event_recovery.clear()
    logging.info("Recovery stop flag reset")
    return "Recovery stop flag reset"

def check_stop_flag_recovery():
    """Check if the recovery stop flag is active"""
    return stop_event_recovery.is_set()

# Paths
GENERATED_AUDIOBOOKS_DIR = "Generated_Audiobooks"
INTERMEDIATE_CHUNKS_DIR = "Intermediate_Audio_Chunks"

def scan_audiobooks_with_errors() -> List[str]:
    """Scans Generated_Audiobooks/ and returns list of audiobooks with errors."""
    books_with_errors = []
    if not os.path.exists(GENERATED_AUDIOBOOKS_DIR):
        logging.info(f"Directory {GENERATED_AUDIOBOOKS_DIR} not found.")
        return books_with_errors
    
    logging.info(f"Scanning {GENERATED_AUDIOBOOKS_DIR} for audiobooks with errors...")
    
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
                            logging.info(f"Found audiobook with errors: {item}")
                        else:
                            logging.info(f"Audiobook {item} has failed_chunks.json but chapters_with_errors is empty")
                except Exception as e:
                    logging.error(f"Error reading {failed_chunks_path}: {e}")
            else:
                logging.debug(f"Audiobook {item} has no failed_chunks.json")
        else:
            logging.debug(f"Ignored file (not a directory): {item}")
    
    logging.info(f"Scan completed. Found {len(books_with_errors)} audiobooks with errors: {books_with_errors}")
    return books_with_errors

def load_failed_chunks_json(book_name: str) -> Optional[Dict]:
    """Load the failed_chunks.json file for an audiobook."""
    if not book_name:
        return None
    json_path = os.path.join(GENERATED_AUDIOBOOKS_DIR, book_name, "failed_chunks.json")
    if not os.path.exists(json_path):
        return None
    try:
        with open(json_path, 'r', encoding='utf-8') as f:
            return json.load(f)
    except Exception as e:
        logger.error("Failed to load failed_chunks.json for '%s': %s", book_name, e)
        return None

def get_chapters_with_errors(book_name: str) -> List[str]:
    """Return list of chapters with errors for an audiobook."""
    data = load_failed_chunks_json(book_name)
    if not data:
        return []
    chapters = data.get("chapters_with_errors", {})
    return list(chapters.keys())

def get_failed_chunks_for_chapter(book_name: str, chapter_name: str) -> List[int]:
    """Return list of failed chunk indices for a chapter."""
    data = load_failed_chunks_json(book_name)
    if not data:
        return []
    chapters = data.get("chapters_with_errors", {})
    return chapters.get(chapter_name, [])

def update_chapters_dropdown(book_name: str) -> gr.update:
    """Update chapters dropdown based on selected audiobook."""
    if not book_name:
        return gr.update(choices=[], value=None)
    chapters = get_chapters_with_errors(book_name)
    return gr.update(choices=chapters, value=chapters[0] if chapters else None)

def update_chunks_checkbox(book_name: str, chapter_name: str) -> gr.update:
    """Update chunks checkbox based on selected chapter."""
    if not book_name or not chapter_name:
        return gr.update(choices=[], value=[])
    chunk_indices = get_failed_chunks_for_chapter(book_name, chapter_name)
    
    if not chunk_indices:
        # Chapter with no failed chunks (all regenerated successfully)
        # Show info message instead of empty list
        choices = ["✅ All chunks regenerated successfully. Press 'Merge All Chunks' to generate the full chapter."]
        return gr.update(choices=choices, value=[], interactive=False, label="Chapter Status")
    else:
        # Create labels "Chunk 1", "Chunk 3", etc.
        choices = [f"Chunk {idx}" for idx in chunk_indices]
        return gr.update(choices=choices, value=choices, interactive=True, label="Failed Chunks")

def update_editing_dropdown(book_name: str, chapter_name: str) -> gr.update:
    """Update dropdown for manual editing."""
    if not book_name or not chapter_name:
        return gr.update(choices=[], value=None)
    chunk_indices = get_failed_chunks_for_chapter(book_name, chapter_name)
    
    if not chunk_indices:
        # Chapter with no failed chunks (all regenerated successfully)
        choices = ["No failed chunks available for editing"]
        return gr.update(choices=choices, value=choices[0], interactive=False)
    else:
        choices = [f"Chunk {idx}" for idx in chunk_indices]
        return gr.update(choices=choices, value=choices[0] if choices else None, interactive=True)

def retry_synthesis_real(book_name: str, chapter_name: str, selected_chunks: List[str]) -> Tuple[str, bool]:
    """Retry synthesis for selected failed chunks."""
    if not HAS_BACKEND:
        return "Backend modules not available. Cannot retry synthesis.", False
    
    reset_stop_flag_recovery()
    
    if not book_name or not chapter_name or not selected_chunks:
        return "Select an audiobook, chapter, and chunk.", False
    
    # Load error data
    data = load_failed_chunks_json(book_name)
    if not data:
        return f"No error data found for '{book_name}'.", False
    
    # Extract chunk indices (e.g. "Chunk 3" -> 3)
    chunk_indices = []
    for chunk_label in selected_chunks:
        try:
            idx = int(chunk_label.replace("Chunk ", ""))
            chunk_indices.append(idx)
        except ValueError:
            logging.warning(f"Invalid chunk format: {chunk_label}")
    
    if not chunk_indices:
        return "No valid chunks selected.", False
    
    # Verify that the chunks are actually failed
    chapters_with_errors = data.get("chapters_with_errors", {})
    actual_failed = chapters_with_errors.get(chapter_name, [])
    valid_indices = [idx for idx in chunk_indices if idx in actual_failed]
    
    if not valid_indices:
        return f"None of the selected chunks have failed in chapter '{chapter_name}'.", False
    
    # Load original text
    failed_texts = data.get("failed_chunks_text", {}).get(chapter_name, {})
    
    # Determine original TTS model and parameters
    model_used = data.get("model_used", "XTTSv2")
    language = data.get("language", "it")
    
    # Load ALL saved parameters
    tts_params = data.get("tts_params", {})
    technical_voice_id = data.get("technical_voice_id")
    proc_opts = data.get("proc_opts", {})
    
    # Paths
    book_chunk_dir = os.path.join(INTERMEDIATE_CHUNKS_DIR, book_name, chapter_name)
    os.makedirs(book_chunk_dir, exist_ok=True)
    
    success_count = 0
    failed_count = 0
    error_messages = []
    
    # Load model once for all chunks (performance)
    model_instance = None
    try:
        model_instance = plugin_manager.plugin_manager.load_model(model_used, language_code=language if model_used == "Kokoro" else None)
        
        if not model_instance:
            error_messages.append(f"Cannot load model '{model_used}'.")
            failed_count = len(valid_indices)
            result_msg = f"Error: model '{model_used}' not available."
            return result_msg, False
    except Exception as e:
        error_messages.append(f"Error loading model: {str(e)}")
        failed_count = len(valid_indices)
        result_msg = f"Error loading model: {str(e)}"
        return result_msg, False
    
    for idx in valid_indices:
        if check_stop_flag_recovery():
            logging.info("Recovery stop flag detected, interrupting retry.")
            error_messages.append("Interrupted by user.")
            break
        chunk_text = failed_texts.get(str(idx))
        if not chunk_text:
            error_messages.append(f"Chunk {idx}: Original text not found.")
            failed_count += 1
            continue
        
        chunk_filename = f"chunk_{idx:04d}.wav"
        chunk_path = os.path.join(book_chunk_dir, chunk_filename)
        
        try:
            logging.info(f"Retrying synthesis for chunk {idx} with model {model_used}...")
            
            # Prepare parameters for synthesis
            all_params = {}
            
            # Add model-specific parameters
            if model_used == "XTTSv2":
                all_params.update({
                    "language": language,
                    "speaker_wav": technical_voice_id,
                    "use_tts_splitting": True,
                    "sentence_separator": proc_opts.get("sentence_separator", ".")
                })
                # Add TTS parameters if present
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
                # Add all VibeVoice parameters
                for key in ["temperature", "top_p", "cfg_scale", "diffusion_steps", 
                           "voice_speed_factor", "use_sampling", "seed"]:
                    if key in tts_params:
                        all_params[key] = tts_params[key]
                        
            elif model_used.startswith("Qwen3-TTS"):
                # For Qwen3-TTS, use saved parameters
                if "qwen_mode" in tts_params:
                    all_params["qwen_mode"] = tts_params["qwen_mode"]
                if "qwen_params" in tts_params:
                    all_params["qwen_params"] = tts_params["qwen_params"]
                # Add language for compatibility
                all_params["language"] = language
            
            # Call TTS plugin manager with all parameters
            success = plugin_manager.plugin_manager.synthesize(
                model_name=model_used,
                text=chunk_text,
                output_path=chunk_path,
                model_instance=model_instance,
                **all_params
            )
            
            if success:
                # Update JSON: remove chunk from error list
                if idx in chapters_with_errors.get(chapter_name, []):
                    chapters_with_errors[chapter_name].remove(idx)
                success_count += 1
                logging.info(f"Chunk {idx} regenerated successfully.")
            else:
                error_messages.append(f"Chunk {idx}: Synthesis failed.")
                failed_count += 1
                
        except Exception as e:
            error_messages.append(f"Chunk {idx}: Error - {str(e)}")
            failed_count += 1
            logging.error(f"Error during retry of chunk {idx}: {e}")
    
    # Save updated JSON if there are successes
    if success_count > 0:
        data["chapters_with_errors"] = chapters_with_errors
        # DO NOT remove the chapter even if it has no more errors
        # The chapter must remain in the list so the user can press "Merge All Chunks"
        # The chapter will be removed only after the user presses "Merge All Chunks" and the merge succeeds
        
        json_path = os.path.join(GENERATED_AUDIOBOOKS_DIR, book_name, "failed_chunks.json")
        try:
            with open(json_path, 'w', encoding='utf-8') as f:
                json.dump(data, f, indent=2, ensure_ascii=False)
            logging.info(f"JSON file updated: {json_path}")
        except Exception as e:
            logging.error(f"Error saving JSON: {e}")
    
    # Build result message
    result_msg = f"Result: {success_count} succeeded, {failed_count} failed."
    if error_messages:
        result_msg += "\nErrors: " + "; ".join(error_messages[:3])  # Show only first 3 errors
    
    success = failed_count == 0
    return result_msg, success

def merge_all_chunks_real(book_name: str, chapter_name: str) -> Tuple[str, bool]:
    """Merge all audio chunks of a chapter and update the error JSON."""
    if not HAS_BACKEND:
        return "Backend modules not available. Cannot merge chunks.", False
    
    if not book_name or not chapter_name:
        return "Select an audiobook and chapter.", False
    
    # Paths
    book_chunk_dir = os.path.join(INTERMEDIATE_CHUNKS_DIR, book_name, chapter_name)
    if not os.path.exists(book_chunk_dir):
        return f"Chunk directory not found: {book_chunk_dir}", False
    
    # Search for chunk files
    chunk_files = []
    for f in os.listdir(book_chunk_dir):
        if f.startswith("chunk_") and f.endswith(".wav"):
            chunk_files.append(os.path.join(book_chunk_dir, f))
    
    if not chunk_files:
        return f"No chunk files found in {book_chunk_dir}", False
    
    # Sort chunks by number
    def safe_chunk_sort(filepath):
        try:
            return int(os.path.basename(filepath).split('_')[1].split('.')[0])
        except (ValueError, IndexError):
            logger.warning("Unexpected chunk filename format: %s", filepath)
            return float('inf')
    
    chunk_files.sort(key=safe_chunk_sort)
    # Filter out files that could not be parsed
    chunk_files = [f for f in chunk_files if safe_chunk_sort(f) != float('inf')]
    
    # MP3 output path
    output_dir = os.path.join(GENERATED_AUDIOBOOKS_DIR, book_name)
    os.makedirs(output_dir, exist_ok=True)
    output_path = os.path.join(output_dir, f"{chapter_name}.mp3")
    
    try:
        # Use ffmpeg wrapper to merge
        success = ffmpeg_wrapper.merge_audio_files_ffmpeg(
            book_chunk_dir, 
            output_path,
            config.DEFAULT_FFMPEG_EXE if hasattr(config, 'DEFAULT_FFMPEG_EXE') else 'ffmpeg'
        )
        
        if success:
            # Update JSON: remove chapter from error list
            data = load_failed_chunks_json(book_name)
            if data and "chapters_with_errors" in data:
                chapters_with_errors = data.get("chapters_with_errors", {})
                if chapter_name in chapters_with_errors:
                    del chapters_with_errors[chapter_name]
                    logging.info(f"Chapter '{chapter_name}' removed from error list after successful merge.")
                    
                    # If chapters_with_errors is empty, delete the JSON file
                    if not chapters_with_errors:
                        json_path = os.path.join(GENERATED_AUDIOBOOKS_DIR, book_name, "failed_chunks.json")
                        try:
                            os.remove(json_path)
                            logging.info(f"JSON file deleted because no more chapters with errors: {json_path}")
                        except Exception as e:
                            logging.error(f"Error deleting JSON file: {e}")
                    else:
                        # Save updated JSON
                        data["chapters_with_errors"] = chapters_with_errors
                        json_path = os.path.join(GENERATED_AUDIOBOOKS_DIR, book_name, "failed_chunks.json")
                        try:
                            with open(json_path, 'w', encoding='utf-8') as f:
                                json.dump(data, f, indent=2, ensure_ascii=False)
                            logging.info(f"JSON file updated after merge: {json_path}")
                        except Exception as e:
                            logging.error(f"Error saving JSON: {e}")
            
            return f"Merge completed: {len(chunk_files)} chunks → {output_path}", True
        else:
            return "Merge failed (ffmpeg error).", False
    except Exception as e:
        logging.error(f"Error during merge: {e}")
        return f"Error during merge: {str(e)}", False

def split_chunk_real(chunk_text: str) -> Tuple[str, bool]:
    """Split a text chunk into smaller parts."""
    if not chunk_text or not chunk_text.strip():
        return "Chunk text is empty.", False
    
    try:
        # Use epub_processor to split
        sentences = epub_processor.split_into_sentences(chunk_text)
        
        if len(sentences) <= 1:
            return "The text has only one sentence, it cannot be split.", False
        
        # Create split preview
        preview = f"Split into {len(sentences)} sentences:\n"
        for i, sent in enumerate(sentences[:3], 1):  # Show first 3
            preview += f"{i}. {sent[:50]}...\n"
        if len(sentences) > 3:
            preview += f"... and {len(sentences) - 3} more sentences."
        
        return preview, True
    except Exception as e:
        logging.error(f"Error during chunk split: {e}")
        return f"Error during split: {str(e)}", False

def manual_generate_real(chunk_text: str) -> Tuple[str, bool]:
    """Generate audio manually from text."""
    if not chunk_text or not chunk_text.strip():
        return "Chunk text is empty.", False
    
    # Placeholder - to be implemented with UI for model/parameter selection
    return "Manual generation feature is in development. It will be implemented in the next phase.", True

# Placeholder functions kept for compatibility
def retry_synthesis_placeholder(book_name: str, chapter_name: str, selected_chunks: List[str]):
    """Placeholder for retrying synthesis."""
    result, _ = retry_synthesis_real(book_name, chapter_name, selected_chunks)
    return result

def merge_chunks_placeholder(book_name: str, chapter_name: str):
    """Placeholder to merge all chunks."""
    result, _ = merge_all_chunks_real(book_name, chapter_name)
    return result

def split_chunk_placeholder(chunk_text: str):
    """Placeholder to split a chunk."""
    result, _ = split_chunk_real(chunk_text)
    return result

def manual_generate_placeholder(chunk_text: str):
    """Placeholder for manual generation."""
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
        
        # --- Events ---
        # Update chapters when audiobook changes
        book_dropdown.change(
            fn=update_chapters_dropdown,
            inputs=[book_dropdown],
            outputs=[chapter_dropdown]
        )
        
        # Update chunks checkbox when chapter changes
        chapter_dropdown.change(
            fn=update_chunks_checkbox,
            inputs=[book_dropdown, chapter_dropdown],
            outputs=[chunks_checkbox]
        )
        
        # Update editing dropdown when chapter changes
        chapter_dropdown.change(
            fn=update_editing_dropdown,
            inputs=[book_dropdown, chapter_dropdown],
            outputs=[edit_chunk_dropdown]
        )
        
        # Refresh function
        def refresh_books():
            return gr.update(choices=scan_audiobooks_with_errors())
        
        refresh_button.click(
            fn=refresh_books,
            inputs=[],
            outputs=[book_dropdown]
        )
        
        # Real functions with output
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
        
        # Callback for recovery stop button
        stop_recovery_button.click(
            fn=set_stop_flag_recovery,
            inputs=[],
            outputs=[result_textbox]
        )
    
    return tab