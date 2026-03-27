import subprocess
import json
import os
import logging
from audiobook_generator.base_plugin import BaseTTSPlugin
from audiobook_generator import config
from audiobook_generator.model_manager import model_manager  # Import per asset management

logger = logging.getLogger(__name__)

class KokoroPlugin(BaseTTSPlugin):
    
    def load_model(self, *args, **kwargs):
        """Verifica che il venv isolato di Kokoro esista e che gli asset siano pronti."""
        if not os.path.exists(config.KOKORO_PYTHON_EXECUTABLE):
            raise FileNotFoundError(f"Eseguibile Python di Kokoro non trovato in {config.KOKORO_PYTHON_EXECUTABLE}. Esegui l'installer.")
        
        logger.info(f"Verifica venv Kokoro: {config.KOKORO_PYTHON_EXECUTABLE} trovato.")
        
        # Verifica asset tramite model_manager (come VibeVoice/Qwen)
        logger.info(f"Verifica asset per {self.name}...")
        if not model_manager.ensure_assets(self.name):
            logger.warning(f"Asset per {self.name} non presenti. Kokoro scaricherà automaticamente i modelli alla prima sintesi.")
            # Non blocchiamo, Kokoro può scaricare automaticamente
        else:
            logger.info(f"Asset per {self.name} verificati.")
        
        return {"status": "ready"}

    def synthesize(self, model_instance: any, text: str, output_path: str, **kwargs) -> bool:
        """
        Lancia il sottoprocesso per la sintesi con Kokoro e comunica tramite JSON.
        """
        logger.info(f"Sintesi Kokoro: '{text[:80]}...'")
        
        # Estrai parametri con valori di default
        voice_id = kwargs.get('voice_id')
        speed = float(kwargs.get('speed', 1.0))
        language_code = kwargs.get('language_code', 'en')
        
        if not voice_id:
            logger.error("ERRORE Kokoro: 'voice_id' non fornito.")
            return False

        script_path = os.path.join(os.path.dirname(__file__), 'synthesize_subprocess.py')
        
        payload = {
            "text": text,
            "output_path": output_path,
            "voice_id": voice_id,
            "speed": speed,
            "language_code": language_code
        }

        try:
            # Esegui dalla directory del progetto per evitare problemi di percorsi
            project_dir = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
            
            process = subprocess.Popen(
                [config.KOKORO_PYTHON_EXECUTABLE, script_path],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                encoding='utf-8',
                cwd=project_dir
            )

            stdout_data, stderr_data = process.communicate(json.dumps(payload), timeout=300)  # Timeout di 5 minuti

            if process.returncode != 0:
                logger.error(f"ERRORE: Il sottoprocesso Kokoro ha terminato con codice {process.returncode}.")
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
                logger.info(f"SUCCESSO: Kokoro ha generato il file: {response.get('file')}")
                return True
            else:
                logger.error(f"ERRORE nella sintesi Kokoro: {response.get('message')}")
                return False

        except subprocess.TimeoutExpired:
            logger.error("ERRORE: Timeout raggiunto durante la sintesi con Kokoro.")
            process.kill()
            return False
        except Exception as e:
            logger.error(f"ERRORE imprevisto durante la gestione del sottoprocesso Kokoro: {e}", exc_info=True)
            return False
