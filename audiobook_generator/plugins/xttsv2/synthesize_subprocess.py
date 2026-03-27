# ATTENZIONE: Questo script è autonomo e non deve importare nulla dal progetto principale.
import sys
import json
import os
import logging
import time

# --- Configurazione Cache HuggingFace per Portabilità ---
# Imposta la cache dei modelli HuggingFace nella directory del progetto
# Usa percorso relativo per garantire portabilità multipiattaforma
HF_CACHE_DIR = os.path.join('audiobook_generator', 'tts_models', 'xttsv2')
os.environ['HF_HOME'] = HF_CACHE_DIR
os.environ['TRANSFORMERS_CACHE'] = HF_CACHE_DIR
os.environ['HF_DATASETS_CACHE'] = HF_CACHE_DIR

# Setup di un logger dedicato per il subprocess
log_dir = os.path.join('audiobook_generator', 'Logs')
os.makedirs(log_dir, exist_ok=True)
log_file = os.path.join(log_dir, 'xttsv2_subprocess.log')
logging.basicConfig(level=logging.INFO, filename=log_file, filemode='a',
                    format='%(asctime)s - %(levelname)s - %(message)s')

logging.info(f"Cache HuggingFace configurata su: {HF_CACHE_DIR}")

def main():
    # --- Reindirizzamento stdout a stderr per evitare output non-JSON ---
    # Salva il riferimento originale a stdout
    original_stdout = sys.stdout
    
    # Crea un redirector che scrive su stderr
    class StdoutToStderr:
        def write(self, text):
            sys.stderr.write(text)
            sys.stderr.flush()
        def flush(self):
            sys.stderr.flush()
    
    # Reindirizza stdout a stderr
    sys.stdout = StdoutToStderr()
    
    try:
        # Importa torch solo quando necessario (per logging GPU)
        import torch
        
        # 1. Legge l'input JSON da stdin
        input_data = json.load(sys.stdin)
        logging.info(f"Ricevuto job XTTSv2: {input_data}")
        
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
        
        logging.info(f"Parametri XTTSv2: language={language}, speaker_wav={speaker_wav}, temperature={temperature}, speed={speed}")
        
        # Log dettagliato GPU
        logging.info(f"Torch version: {torch.__version__}")
        logging.info(f"CUDA available: {torch.cuda.is_available()}")
        if torch.cuda.is_available():
            logging.info(f"CUDA device count: {torch.cuda.device_count()}")
            for i in range(torch.cuda.device_count()):
                logging.info(f"  Device {i}: {torch.cuda.get_device_name(i)}")
        else:
            logging.warning("CUDA non disponibile. Il modello verrà caricato su CPU.")
        
        # Importa TTS (dipendenza del venv isolato)
        try:
            from TTS.api import TTS
        except ImportError as e:
            error_msg = f"ImportError: {e}. Assicurati che TTS sia installato nel venv isolato."
            logging.error(error_msg)
            sys.stdout = original_stdout
            send_response({"status": "error", "message": error_msg})
            return
        
        # Caricamento modello con device appropriato
        device = 'cuda' if torch.cuda.is_available() else 'cpu'
        logging.info(f"Caricamento modello XTTSv2 su device '{device}'...")
        
        # Funzione per caricare il modello XTTSv2 da path locale
        def load_xtts_model():
            # base: AudiobookGenerator_CARTELLA DI LAVORO (4 livelli up da synthesize_subprocess.py)
            base_dir = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(__file__))))
            model_dir = os.path.join(base_dir, 'audiobook_generator', 'tts_models', 'xttsv2')
            config_path = os.path.join(model_dir, 'config.json')
            
            logging.info(f"Caricamento modello XTTSv2 da directory: {model_dir}")
            logging.info(f"  - config: {config_path}")
            logging.info(f"  - model (dir): {model_dir}")
            
            try:
                # Passa la DIRECTORY a model_path — Coqui fa os.path.join(model_path, "model.pth") internamente
                return TTS(model_path=model_dir, config_path=config_path, gpu=(device == 'cuda'))
            except Exception as e:
                logging.error(f"Errore nel caricamento modello XTTSv2: {e}")
                raise
        
        try:
            model = load_xtts_model()
            logging.info(f"Modello XTTSv2 caricato con successo.")
            logging.info(f"is_multi_speaker={getattr(model, 'is_multi_speaker', '?')}, speakers={getattr(model, 'speakers', '?')}")
        except Exception as e:
            error_msg = f"ERRORE nel caricamento modello XTTSv2: {e}"
            logging.error(error_msg, exc_info=True)
            sys.stdout = original_stdout
            send_response({"status": "error", "message": error_msg})
            return
        
        # Preprocess text: sostituisce separatore se diverso da "."
        processed_text = text.replace(".", sentence_separator) if sentence_separator != "." else text
        
        # Tentativi di sintesi con retry
        success = False
        for attempt in range(max_retries):
            try:
                logging.info(f"Sintesi chunk (tentativo {attempt+1}/{max_retries}): '{processed_text[:80]}...'")
                
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
                    logging.warning(f"File di output non creato dopo il tentativo {attempt+1}.")
            
            except Exception as e:
                logging.error(f"Sintesi XTTSv2 fallita (tentativo {attempt+1}/{max_retries}): {e}", exc_info=True)
                if attempt < max_retries - 1:
                    wait_time = 2 ** attempt
                    logging.info(f"Attesa di {wait_time} secondi prima del prossimo tentativo...")
                    time.sleep(wait_time)
                else:
                    logging.error(f"Tutti i {max_retries} tentativi sono falliti.")
        
        # Verifica output
        if success and os.path.exists(output_path) and os.path.getsize(output_path) > 1024:
            logging.info(f"File generato con successo: {output_path}")
            # Ripristina stdout originale prima di inviare la risposta JSON
            sys.stdout = original_stdout
            send_response({"status": "ok", "file": output_path, "message": "Sintesi XTTSv2 completata con successo."})
        else:
            error_msg = "File di output non creato o vuoto dopo tutti i tentativi."
            logging.error(error_msg)
            # Ripristina stdout originale prima di inviare la risposta JSON
            sys.stdout = original_stdout
            send_response({"status": "error", "message": error_msg})
            
    except Exception as e:
        error_msg = f"Errore nel subprocess XTTSv2: {e}"
        logging.error(error_msg, exc_info=True)
        # Ripristina stdout originale prima di inviare la risposta JSON
        sys.stdout = original_stdout
        send_response({"status": "error", "message": error_msg})
    finally:
        # Assicurati che stdout sia ripristinato anche in caso di eccezioni non catturate
        sys.stdout = original_stdout

def send_response(data):
    """Invia una risposta JSON a stdout."""
    sys.stdout.write(json.dumps(data) + '\n')
    sys.stdout.flush()

if __name__ == "__main__":
    main()