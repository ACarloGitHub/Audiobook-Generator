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

# WARNING: This script is standalone and must not import anything from the main project.
import sys
import json
import os
import logging
import time

# --- HuggingFace Cache Configuration for Portability ---
# Set HuggingFace model cache in the project directory
# Use relative path to ensure cross-platform portability
HF_CACHE_DIR = os.path.join('audiobook_generator', 'tts_models', 'xttsv2')
os.environ['HF_HOME'] = HF_CACHE_DIR
os.environ['TRANSFORMERS_CACHE'] = HF_CACHE_DIR
os.environ['HF_DATASETS_CACHE'] = HF_CACHE_DIR

# Setup of a dedicated logger for the subprocess
log_dir = os.path.join('audiobook_generator', 'Logs')
os.makedirs(log_dir, exist_ok=True)
log_file = os.path.join(log_dir, 'xttsv2_subprocess.log')
logging.basicConfig(level=logging.INFO, filename=log_file, filemode='a',
                    format='%(asctime)s - %(levelname)s - %(message)s')

logging.info(f"HuggingFace cache configured on: {HF_CACHE_DIR}")

def main():
    # --- Redirect stdout to stderr to avoid non-JSON output ---
    # Save original stdout reference
    original_stdout = sys.stdout
    
    # Create a redirector that writes to stderr
    class StdoutToStderr:
        def write(self, text):
            sys.stderr.write(text)
            sys.stderr.flush()
        def flush(self):
            sys.stderr.flush()
    
    # Redirect stdout to stderr
    sys.stdout = StdoutToStderr()
    
    try:
        # Import torch only when needed (for GPU logging)
        import torch
        
        # 1. Read JSON input from stdin
        input_data = json.load(sys.stdin)
        logging.info(f"Received XTTSv2 job: {input_data}")
        
        text = input_data['text']
        output_path = input_data['output_path']
        language = input_data.get('language')
        speaker_wav = input_data.get('speaker_wav')
        temperature = float(input_data.get('temperature', 0.75))
        speed = float(input_data.get('speed', 1.0))
        repetition_penalty = float(input_data.get('repetition_penalty', 2.0))
        use_tts_splitting = input_data.get('use_tts_splitting', True)
        sentence_separator = input_data.get('sentence_separator', ".")
        max_retries = int(input_data.get('max_retries', 3))
        
        logging.info(f"XTTSv2 parameters: language={language}, speaker_wav={speaker_wav}, temperature={temperature}, speed={speed}")
        
        # Detailed GPU logging
        logging.info(f"Torch version: {torch.__version__}")
        logging.info(f"CUDA available: {torch.cuda.is_available()}")
        if torch.cuda.is_available():
            logging.info(f"CUDA device count: {torch.cuda.device_count()}")
            for i in range(torch.cuda.device_count()):
                logging.info(f"  Device {i}: {torch.cuda.get_device_name(i)}")
        else:
            logging.warning("CUDA not available. The model will be loaded on CPU.")
        
        # Import TTS (isolated venv dependency)
        try:
            from TTS.api import TTS
        except ImportError as e:
            error_msg = f"ImportError: {e}. Make sure TTS is installed in the isolated venv."
            logging.error(error_msg)
            sys.stdout = original_stdout
            send_response({"status": "error", "message": error_msg})
            return
        
        # Load model with appropriate device
        device = 'cuda' if torch.cuda.is_available() else 'cpu'
        logging.info(f"Loading XTTSv2 model on device '{device}'...")
        
        # Function to load XTTSv2 model from local path
        def load_xtts_model():
            # base: AudiobookGenerator_WORKING_DIR (4 levels up from synthesize_subprocess.py)
            base_dir = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(__file__))))
            model_dir = os.path.join(base_dir, 'audiobook_generator', 'tts_models', 'xttsv2')
            config_path = os.path.join(model_dir, 'config.json')
            
            logging.info(f"Loading XTTSv2 model from directory: {model_dir}")
            logging.info(f"  - config: {config_path}")
            logging.info(f"  - model (dir): {model_dir}")
            
            try:
                # Pass the DIRECTORY to model_path — Coqui does os.path.join(model_path, "model.pth") internally
                return TTS(model_path=model_dir, config_path=config_path, gpu=(device == 'cuda'))
            except Exception as e:
                logging.error(f"Error loading XTTSv2 model: {e}")
                raise
        
        try:
            model = load_xtts_model()
            logging.info(f"XTTSv2 model loaded successfully.")
            logging.info(f"is_multi_speaker={getattr(model, 'is_multi_speaker', '?')}, speakers={getattr(model, 'speakers', '?')}")
        except Exception as e:
            error_msg = f"ERROR loading XTTSv2 model: {e}"
            logging.error(error_msg, exc_info=True)
            sys.stdout = original_stdout
            send_response({"status": "error", "message": error_msg})
            return
        
        # Preprocess text: replace separator if different from "."
        processed_text = text.replace(".", sentence_separator) if sentence_separator != "." else text
        
        # Synthesis attempts with retry
        success = False
        for attempt in range(max_retries):
            try:
                logging.info(f"Synthesizing chunk (attempt {attempt+1}/{max_retries}): '{processed_text[:80]}...'")
                
                model.tts_to_file(
                    text=processed_text,
                    file_path=output_path,
                    speaker_wav=speaker_wav,
                    language=language,
                    split_sentences=use_tts_splitting,
                    speed=speed,
                    temperature=temperature,
                    repetition_penalty=repetition_penalty
                )
                
                if os.path.exists(output_path):
                    success = True
                    break
                else:
                    logging.warning(f"Output file not created after attempt {attempt+1}.")
            
            except Exception as e:
                logging.error(f"XTTSv2 synthesis failed (attempt {attempt+1}/{max_retries}): {e}", exc_info=True)
                if attempt < max_retries - 1:
                    wait_time = 2 ** attempt
                    logging.info(f"Waiting {wait_time} seconds before next attempt...")
                    time.sleep(wait_time)
                else:
                    logging.error(f"All {max_retries} attempts failed.")
        
        # Verify output
        if success and os.path.exists(output_path) and os.path.getsize(output_path) > 1024:
            logging.info(f"File generated successfully: {output_path}")
            # Restore original stdout before sending JSON response
            sys.stdout = original_stdout
            send_response({"status": "ok", "file": output_path, "message": "XTTSv2 synthesis completed successfully."})
        else:
            error_msg = "Output file not created or empty after all attempts."
            logging.error(error_msg)
            # Restore original stdout before sending JSON response
            sys.stdout = original_stdout
            send_response({"status": "error", "message": error_msg})
            
    except Exception as e:
        error_msg = f"Error in XTTSv2 subprocess: {e}"
        logging.error(error_msg, exc_info=True)
        # Restore original stdout before sending JSON response
        sys.stdout = original_stdout
        send_response({"status": "error", "message": error_msg})
    finally:
        # Ensure stdout is restored even for uncaught exceptions
        sys.stdout = original_stdout

def send_response(data):
    """Send a JSON response to stdout."""
    sys.stdout.write(json.dumps(data) + '\n')
    sys.stdout.flush()

if __name__ == "__main__":
    main()