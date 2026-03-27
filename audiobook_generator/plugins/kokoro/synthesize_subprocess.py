# ATTENZIONE: Questo script è autonomo e non deve importare nulla dal progetto principale.
import sys
import json
import soundfile as sf
import numpy as np
import os
import logging

# --- Configurazione Cache HuggingFace per Portabilità ---
# Imposta la cache dei modelli HuggingFace nella directory del progetto
# Usa percorso relativo per garantire portabilità multipiattaforma
# IMPORTANTE: Tutte le variabili HF devono essere settate PRIMA di importare kokoro
HF_CACHE_DIR = os.path.join('audiobook_generator', 'tts_models', 'kokoro', 'models')
os.environ['HF_HOME'] = HF_CACHE_DIR
os.environ['HF_CACHE_HOME'] = HF_CACHE_DIR
os.environ['HF_MODULES_CACHE'] = HF_CACHE_DIR
os.environ['TRANSFORMERS_CACHE'] = HF_CACHE_DIR
os.environ['HF_DATASETS_CACHE'] = HF_CACHE_DIR

# Setup di un logger dedicato per il subprocess
log_dir = os.path.join('audiobook_generator', 'Logs')
os.makedirs(log_dir, exist_ok=True)
log_file = os.path.join(log_dir, 'kokoro_subprocess.log')
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
        logging.info(f"Ricevuto job Kokoro: {input_data}")
        
        text = input_data['text']
        output_path = input_data['output_path']
        voice_id = input_data.get('voice_id')
        speed = float(input_data.get('speed', 1.0))
        language_code = input_data.get('language_code', 'en')
        
        logging.info(f"Parametri Kokoro: voice_id={voice_id}, speed={speed}, language_code={language_code}")
        
        # Log dettagliato GPU
        logging.info(f"Torch version: {torch.__version__}")
        logging.info(f"CUDA available: {torch.cuda.is_available()}")
        if torch.cuda.is_available():
            logging.info(f"CUDA device count: {torch.cuda.device_count()}")
            for i in range(torch.cuda.device_count()):
                logging.info(f"  Device {i}: {torch.cuda.get_device_name(i)}")
        else:
            logging.warning("CUDA non disponibile. Il modello verrà caricato su CPU.")
        
        # Importa kokoro.pipeline (dipendenza del venv isolato)
        import kokoro.pipeline as kp
        
        # Mappatura language_code -> lang_code usato da Kokoro
        lang_map = {
            'en': 'a',
            'it': 'i',
            'fr': 'f',
            'ja': 'j',
            'zh-cn': 'z',
        }
        kokoro_lang = lang_map.get(language_code)
        if not kokoro_lang:
            error_msg = f"Lingua non supportata da Kokoro: {language_code}"
            logging.error(error_msg)
            send_response({"status": "error", "message": error_msg})
            return
        
        # Caricamento pipeline con device appropriato
        device = 'cuda' if torch.cuda.is_available() else 'cpu'
        logging.info(f"Caricamento pipeline Kokoro per lingua '{language_code}' su device '{device}'...")
        
        pipeline = kp.KPipeline(lang_code=kokoro_lang, device=device)
        logging.info(f"Pipeline Kokoro caricata con successo.")
        
        # Sintesi
        logging.info(f"Sintesi testo: '{text[:80]}...'")
        gen = pipeline(text, voice=voice_id, speed=speed)
        
        # Iteriamo su tutti i risultati per concatenare l'audio
        audio_chunks = []
        for result in gen:
            # Prendi l'audio da result.audio (o result.output.audio)
            audio_tensor = result.audio if hasattr(result, 'audio') else result.output.audio
            audio_chunks.append(audio_tensor.cpu().numpy())
        
        if not audio_chunks:
            error_msg = "Nessun risultato generato dal pipeline Kokoro."
            logging.error(error_msg)
            send_response({"status": "error", "message": error_msg})
            return
        
        # Concatenazione lungo l'asse temporale (asse 0)
        audio_array = np.concatenate(audio_chunks, axis=0)
        # Sampling rate fisso a 24000 (Kokoro usa 24kHz)
        sampling_rate = 24000
        
        # Crea directory se non esiste
        output_dir = os.path.dirname(output_path)
        if output_dir:  # Se c'è una directory, creala
            os.makedirs(output_dir, exist_ok=True)
        sf.write(output_path, audio_array, sampling_rate)
        
        # Verifica output
        if os.path.exists(output_path) and os.path.getsize(output_path) > 1024:
            logging.info(f"File generato con successo: {output_path}")
            # Ripristina stdout originale prima di inviare la risposta JSON
            sys.stdout = original_stdout
            send_response({"status": "ok", "file": output_path, "message": "Sintesi Kokoro completata con successo."})
        else:
            error_msg = "File di output non creato o vuoto."
            logging.error(error_msg)
            # Ripristina stdout originale prima di inviare la risposta JSON
            sys.stdout = original_stdout
            send_response({"status": "error", "message": error_msg})
            
    except ImportError as e:
        error_msg = f"ImportError: {e}. Assicurati che kokoro sia installato nel venv isolato."
        logging.error(error_msg)
        # Ripristina stdout originale prima di inviare la risposta JSON
        sys.stdout = original_stdout
        send_response({"status": "error", "message": error_msg})
    except Exception as e:
        error_msg = f"Errore nel subprocess Kokoro: {e}"
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