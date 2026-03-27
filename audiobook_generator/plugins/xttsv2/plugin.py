import subprocess
import json
import os
import logging
from audiobook_generator.base_plugin import BaseTTSPlugin
from audiobook_generator import config
from audiobook_generator.model_manager import model_manager  # Import per asset management

logger = logging.getLogger(__name__)

class XTTSv2Plugin(BaseTTSPlugin):
    
    def load_model(self, *args, **kwargs):
        """Verifica che il venv isolato di XTTSv2 esista e che gli asset siano pronti."""
        if not hasattr(config, 'XTTSV2_PYTHON_EXECUTABLE') or not os.path.exists(config.XTTSV2_PYTHON_EXECUTABLE):
            raise FileNotFoundError(
                f"Eseguibile Python di XTTSv2 non trovato in {getattr(config, 'XTTSV2_PYTHON_EXECUTABLE', 'N/D')}. "
                f"Esegui l'installer e seleziona 'Vuoi installare Coqui TTS (XTTSv2)?'."
            )
        
        logger.info(f"Verifica venv XTTSv2: {config.XTTSV2_PYTHON_EXECUTABLE} trovato.")
        
        # Verifica asset tramite model_manager
        logger.info(f"Verifica asset per {self.name}...")
        if not model_manager.ensure_assets(self.name):
            logger.warning(f"Asset per {self.name} non presenti. XTTSv2 scaricherà automaticamente i modelli alla prima sintesi.")
        else:
            logger.info(f"Asset per {self.name} verificati.")
        
        return {"status": "ready"}

    def synthesize(self, model_instance: any, text: str, output_path: str, **kwargs) -> bool:
        """
        Lancia il sottoprocesso per la sintesi con XTTSv2 e comunica tramite JSON.
        """
        logger.info(f"Sintesi XTTSv2: '{text[:80]}...'")
        
        # Estrai parametri con valori di default
        language = kwargs.get('language')
        speaker_wav = kwargs.get('speaker_wav')
        temperature = float(kwargs.get('temperature', 0.75))
        speed = float(kwargs.get('speed', 1.0))
        repetition_penalty = float(kwargs.get('repetition_penalty', 2.0))
        use_tts_splitting = kwargs.get('use_tts_splitting', True)
        sentence_separator = kwargs.get('sentence_separator', ".")
        max_retries = int(kwargs.get('max_retries', 3))

        script_path = os.path.join(os.path.dirname(__file__), 'synthesize_subprocess.py')
        
        payload = {
            "text": text,
            "output_path": output_path,
            "language": language,
            "speaker_wav": speaker_wav,
            "temperature": temperature,
            "speed": speed,
            "repetition_penalty": repetition_penalty,
            "use_tts_splitting": use_tts_splitting,
            "sentence_separator": sentence_separator,
            "max_retries": max_retries
        }

        try:
            process = subprocess.Popen(
                [config.XTTSV2_PYTHON_EXECUTABLE, script_path],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                encoding='utf-8'
            )

            stdout_data, stderr_data = process.communicate(json.dumps(payload), timeout=300)  # Timeout di 5 minuti

            if process.returncode != 0:
                logger.error(f"ERRORE: Il sottoprocesso XTTSv2 ha terminato con codice {process.returncode}.")
                logger.error(f"Stderr: {stderr_data}")
                logger.error(f"Stdout (raw): {stdout_data}")
                return False
            
            # Debug: log raw stdout per diagnosticare errori JSON
            logger.debug(f"Stdout ricevuto (raw): {stdout_data}")
            
            try:
                response = json.loads(stdout_data)
            except json.JSONDecodeError as e:
                logger.error(f"ERRORE: Impossibile decodificare JSON dalla risposta del subprocess.")
                logger.error(f"Stdout (raw): {stdout_data}")
                logger.error(f"Stderr (raw): {stderr_data}")
                logger.error(f"JSONDecodeError: {e}")
                return False
            
            if response.get("status") == "ok":
                logger.info(f"SUCCESSO: XTTSv2 ha generato il file: {response.get('file')}")
                return True
            else:
                logger.error(f"ERRORE nella sintesi XTTSv2: {response.get('message')}")
                return False

        except subprocess.TimeoutExpired:
            logger.error("ERRORE: Timeout raggiunto durante la sintesi con XTTSv2.")
            process.kill()
            return False
        except Exception as e:
            logger.error(f"ERRORE imprevisto durante la gestione del sottoprocesso XTTSv2: {e}", exc_info=True)
            return False
