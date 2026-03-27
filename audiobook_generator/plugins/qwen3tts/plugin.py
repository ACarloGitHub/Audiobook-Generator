import subprocess
import json
import os
from audiobook_generator.base_plugin import BaseTTSPlugin
from audiobook_generator import config
from audiobook_generator.model_manager import model_manager  # <-- NUOVO IMPORT

class Qwen3TTSPlugin(BaseTTSPlugin):
    def load_model(self, *args, **kwargs):
        if not os.path.exists(config.QWEN3TTS_PYTHON_EXECUTABLE):
            raise FileNotFoundError("Eseguibile Python di Qwen3-TTS non trovato. Esegui l'installer.")
        
        print("Verifica degli asset per Qwen3-TTS...")
        # NOTA: Qui gestiamo la scelta del modello, anche se per ora è semplice
        if not model_manager.ensure_assets("Qwen3-TTS"): # Potremmo passare qui la dimensione del modello in futuro
            raise RuntimeError("Download degli asset di Qwen3-TTS fallito.")
            
        return {"status": "ready"}

    def synthesize(self, model_instance: any, text: str, output_path: str, **kwargs) -> bool:
        script_path = os.path.join(os.path.dirname(__file__), 'synthesize_subprocess.py')
        
        # Determina model_type in base alla modalità (custom -> custom_voice, clone -> base, design -> voice_design)
        mode = kwargs.get("qwen_mode")
        if mode is None:
            # Default a clone se non specificato
            mode = "clone"
            print(f"WARNING: qwen_mode non fornito, default a '{mode}'")
        if mode == "custom":
            model_type = "custom_voice"
        elif mode == "clone":
            model_type = "base"
        elif mode == "design":
            model_type = "voice_design"
        else:
            model_type = "base"  # fallback
            print(f"WARNING: modalità '{mode}' non riconosciuta, uso model_type='base'")
        
        # Model size: default 0.6B per base, 1.7B per custom_voice e voice_design
        # Possibile override tramite qwen_params
        params = kwargs.get("qwen_params", {})
        model_size = params.get("model_size", "0.6B")
        if model_type in ("custom_voice", "voice_design"):
            model_size = "1.7B"  # forzato
        
        payload = {
            "text": text,
            "output_path": output_path,
            "mode": mode,
            "params": params,
            "model_size": model_size,
            "model_type": model_type
        }
        print(f"DEBUG Qwen3TTSPlugin: payload mode={mode}, model_type={model_type}, model_size={model_size}")
        print(f"DEBUG Qwen3TTSPlugin: full payload = {json.dumps(payload, indent=2)}")

        try:
            # Avvia il sottoprocesso catturando stdout e stderr come bytes
            process = subprocess.Popen(
                [config.QWEN3TTS_PYTHON_EXECUTABLE, script_path],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                bufsize=1  # Line buffered
            )
            
            # Invia il payload JSON a stdin
            stdin_data = json.dumps(payload).encode('utf-8')
            stdout_bytes, stderr_bytes = process.communicate(stdin_data, timeout=config.DEFAULT_SUBPROCESS_TIMEOUT)
            
            # Decodifica stderr con gestione errori per logging
            stderr_text = ""
            if stderr_bytes:
                try:
                    stderr_text = stderr_bytes.decode('utf-8', errors='replace')
                except Exception as decode_err:
                    stderr_text = f"[Errore decodifica stderr: {decode_err}] {stderr_bytes[:200]}"
            
            # Log stderr se presente (anche se returncode == 0, potrebbe contenere warning)
            if stderr_text.strip():
                print(f"INFO Subprocess Qwen3-TTS (Stderr): {stderr_text}")
            
            if process.returncode != 0:
                print(f"ERRORE Subprocess Qwen3-TTS (Exit code: {process.returncode}): {stderr_text}")
                return False
            
            # Decodifica stdout con gestione errori
            stdout_text = ""
            try:
                stdout_text = stdout_bytes.decode('utf-8', errors='replace')
            except Exception as decode_err:
                print(f"ERRORE decodifica stdout: {decode_err}, stdout bytes: {stdout_bytes[:200]}")
                return False
            
            # Tenta di parsare JSON
            try:
                response = json.loads(stdout_text)
            except json.JSONDecodeError as e:
                print(f"ERRORE parsing JSON da stdout: {e}")
                print(f"Stdout ricevuto (primi 500 caratteri): {stdout_text[:500]}")
                return False
            
            if response.get("status") == "ok":
                return True
            else:
                print(f"ERRORE nel subprocess Qwen3-TTS: {response.get('message', 'Unknown error')}")
                return False
                
        except subprocess.TimeoutExpired:
            print("ERRORE: Timeout raggiunto durante la sintesi con Qwen3-TTS.")
            if process:
                process.kill()
            return False
        except Exception as e:
            print(f"ERRORE imprevisto durante la gestione del subprocess Qwen3-TTS: {e}")
            return False
