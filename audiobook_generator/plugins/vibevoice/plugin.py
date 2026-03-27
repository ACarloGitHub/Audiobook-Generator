import subprocess
import json
import os
from audiobook_generator.base_plugin import BaseTTSPlugin
from audiobook_generator import config
from audiobook_generator.model_manager import model_manager  # <-- NUOVO IMPORT

class VibeVoicePlugin(BaseTTSPlugin):
    
    def load_model(self, *args, **kwargs):
        if not os.path.exists(config.VIBEVOICE_PYTHON_EXECUTABLE):
            raise FileNotFoundError("Eseguibile Python di VibeVoice non trovato. Esegui l'installer.")
        
        print(f"Verifica degli asset per {self.name}...")
        if not model_manager.ensure_assets(self.name):
            raise RuntimeError(f"Download degli asset di {self.name} fallito. Controlla i log e la connessione internet.")
        
        return {"status": "ready"}

    def synthesize(self, model_instance: any, text: str, output_path: str, **kwargs) -> bool:
        """
        Lancia il sottoprocesso per la sintesi con VibeVoice e comunica tramite JSON.
        """
        print(f"DEBUG VibeVoice: kwargs ricevuti: {kwargs}")
        speaker_wav = kwargs.get('speaker_wav')
        if not speaker_wav:
            print("ERRORE VibeVoice: 'speaker_wav' non fornito.")
            return False

        script_path = os.path.join(os.path.dirname(__file__), 'synthesize_subprocess.py')
        
        # Estrai parametri con valori di default
        temperature = kwargs.get('temperature', 0.9)
        top_p = kwargs.get('top_p', 0.9)
        cfg_scale = kwargs.get('cfg_scale', 1.3)
        diffusion_steps = kwargs.get('diffusion_steps', 15)
        voice_speed_factor = kwargs.get('voice_speed_factor', 1.0)
        use_sampling = kwargs.get('use_sampling', True)
        seed = kwargs.get('seed')
        
        payload = {
            "text": text,
            "output_path": output_path,
            "speaker_wav": speaker_wav,
            "model_name": self.name,
            "temperature": temperature,
            "top_p": top_p,
            "cfg_scale": cfg_scale,
            "diffusion_steps": diffusion_steps,
            "voice_speed_factor": voice_speed_factor,
            "use_sampling": use_sampling,
            "seed": seed
        }

        try:
            process = subprocess.Popen(
                [config.VIBEVOICE_PYTHON_EXECUTABLE, script_path],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                encoding='utf-8'
            )

            stdout_data, stderr_data = process.communicate(json.dumps(payload), timeout=config.DEFAULT_SUBPROCESS_TIMEOUT) # Timeout configurabile (default 1800s = 30 minuti)

            if process.returncode != 0:
                print(f"ERRORE: Il sottoprocesso VibeVoice ha terminato con codice {process.returncode}.")
                print(f"Stderr: {stderr_data}")
                return False
            
            response = json.loads(stdout_data)
            
            if response.get("status") == "ok":
                print(f"SUCCESSO: VibeVoice ha generato il file: {response.get('file')}")
                return True
            else:
                print(f"ERRORE nella sintesi VibeVoice: {response.get('message')}")
                return False

        except subprocess.TimeoutExpired:
            print("ERRORE: Timeout raggiunto durante la sintesi con VibeVoice.")
            process.kill()
            return False
        except Exception as e:
            print(f"ERRORE imprevisto durante la gestione del sottoprocesso VibeVoice: {e}")
            return False
