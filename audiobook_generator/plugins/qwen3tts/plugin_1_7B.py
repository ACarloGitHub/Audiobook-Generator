import subprocess
import json
import os
from audiobook_generator.base_plugin import BaseTTSPlugin
from audiobook_generator import config
from audiobook_generator.model_manager import model_manager

class Qwen3TTS_1_7B_Plugin(BaseTTSPlugin):
    def load_model(self, *args, **kwargs):
        if not os.path.exists(config.QWEN3TTS_PYTHON_EXECUTABLE):
            raise FileNotFoundError("Eseguibile Python di Qwen3-TTS non trovato. Esegui l'installer.")
        
        print("Verifica degli asset per Qwen3-TTS 1.7B...")
        if not model_manager.ensure_assets("Qwen3-TTS-1.7B"):
            raise RuntimeError("Download degli asset di Qwen3-TTS 1.7B fallito.")
            
        return {"status": "ready"}

    def synthesize(self, model_instance: any, text: str, output_path: str, **kwargs) -> bool:
        script_path = os.path.join(os.path.dirname(__file__), 'synthesize_subprocess.py')
        
        payload = {
            "text": text,
            "output_path": output_path,
            "mode": kwargs.get("qwen_mode"),
            "params": kwargs.get("qwen_params", {}),
            "model_size": "1.7B"
        }

        try:
            process = subprocess.Popen(
                [config.QWEN3TTS_PYTHON_EXECUTABLE, script_path],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                bufsize=1
            )
            
            stdin_data = json.dumps(payload).encode('utf-8')
            stdout_bytes, stderr_bytes = process.communicate(stdin_data, timeout=300)
            
            stderr_text = ""
            if stderr_bytes:
                try:
                    stderr_text = stderr_bytes.decode('utf-8', errors='replace')
                except Exception as decode_err:
                    stderr_text = f"[Errore decodifica stderr: {decode_err}] {stderr_bytes[:200]}"
            
            if stderr_text.strip():
                print(f"INFO Subprocess Qwen3-TTS 1.7B (Stderr): {stderr_text}")
            
            if process.returncode != 0:
                print(f"ERRORE Subprocess Qwen3-TTS 1.7B (Exit code: {process.returncode}): {stderr_text}")
                return False
            
            stdout_text = ""
            try:
                stdout_text = stdout_bytes.decode('utf-8', errors='replace')
            except Exception as decode_err:
                print(f"ERRORE decodifica stdout: {decode_err}, stdout bytes: {stdout_bytes[:200]}")
                return False
            
            try:
                response = json.loads(stdout_text)
            except json.JSONDecodeError as e:
                print(f"ERRORE parsing JSON da stdout: {e}")
                print(f"Stdout ricevuto (primi 500 caratteri): {stdout_text[:500]}")
                return False
            
            if response.get("status") == "ok":
                return True
            else:
                print(f"ERRORE nel subprocess Qwen3-TTS 1.7B: {response.get('message', 'Unknown error')}")
                return False
                
        except subprocess.TimeoutExpired:
            print("ERRORE: Timeout raggiunto durante la sintesi con Qwen3-TTS 1.7B.")
            if process:
                process.kill()
            return False
        except Exception as e:
            print(f"ERRORE imprevisto durante la gestione del subprocess Qwen3-TTS 1.7B: {e}")
            return False