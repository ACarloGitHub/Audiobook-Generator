# audiobook_generator/setup_helpers/environment_utils.py
import subprocess
import sys
from .system_utils import command_exists, run_command

def detect_nvidia_gpu():
    """Verifica se è presente una GPU NVIDIA."""
    return command_exists("nvidia-smi")

def detect_apple_silicon():
    """Verifica se siamo su macOS con Apple Silicon."""
    if sys.platform != "darwin":
        return False
    try:
        result = subprocess.run(["uname", "-m"], capture_output=True, text=True, check=True)
        return "arm" in result.stdout.strip().lower()
    except (subprocess.CalledProcessError, FileNotFoundError):
        return False

def install_pytorch(pytorch_choice, pip_exe):
    """Installa PyTorch in base alla scelta."""
    if pytorch_choice == "skip":
        print("Installazione PyTorch saltata.")
        return True
    
    base_cmd = [pip_exe, "install", "torch", "torchvision", "torchaudio"]
    if pytorch_choice == "cuda121":
        cmd = base_cmd + ["--index-url", "https://download.pytorch.org/whl/cu121"]
    elif pytorch_choice == "cuda118":
        cmd = base_cmd + ["--index-url", "https://download.pytorch.org/whl/cu118"]
    else: # cpu or mps
        cmd = base_cmd

    print(f"Installazione PyTorch per {pytorch_choice}...")
    return run_command(cmd)

def detect_recommended_cuda():
    """Determina la versione CUDA consigliata basata sul driver NVIDIA."""
    if not command_exists("nvidia-smi"):
        return None
    try:
        output = subprocess.check_output("nvidia-smi --query-gpu=driver_version --format=csv,noheader", shell=True, text=True)
        major_driver_version = int(output.strip().split('.')[0])
        if major_driver_version >= 525:
            return 'cuda121'
        else:
            return 'cuda118'
    except (subprocess.CalledProcessError, FileNotFoundError, ValueError, IndexError):
        return None

def check_pytorch_cuda(python_exe):
    """Verifica se PyTorch con supporto CUDA è installato in un dato ambiente Python."""
    try:
        result = subprocess.run(
            [python_exe, "-c", "import torch; print(torch.cuda.is_available())"],
            capture_output=True, text=True, check=True, timeout=30
        )
        return "True" in result.stdout.strip()
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return False

def detect_gpu():
    """Verifica se è presente una GPU NVIDIA."""
    try:
        subprocess.check_output("nvidia-smi", shell=True)
        return True
    except (subprocess.CalledProcessError, FileNotFoundError):
        return False

def check_pytorch_cuda_all_venvs():
    """Verifica se PyTorch CUDA è installato nei venv dei modelli TTS.
    
    Ritorna:
        - True se tutti i venv esistenti hanno CUDA disponibile
        - False se almeno un venv esistente non ha CUDA
    I venv non esistenti vengono ignorati (non ancora scaricati).
    """
    import os
    # Solo venv dei modelli TTS, non il venv principale di AG
    venv_paths = [
        os.path.join("audiobook_generator", "tts_models", "vibevoice", "venv"),
        os.path.join("audiobook_generator", "tts_models", "qwen3tts", "venv"),
        os.path.join("audiobook_generator", "tts_models", "kokoro", "venv"),
        os.path.join("audiobook_generator", "tts_models", "xttsv2", "venv"),
    ]
    existing_venvs = 0
    venvs_with_cuda = 0
    
    for venv_path in venv_paths:
        if not os.path.exists(venv_path):
            # Venv non esiste - modello non installato, ignora
            continue
        
        existing_venvs += 1
        
        if sys.platform == "win32":
            python_exe = os.path.join(venv_path, "Scripts", "python.exe")
        else:
            python_exe = os.path.join(venv_path, "bin", "python")
        
        if not os.path.exists(python_exe):
            # Venv corrotto
            print(f"⚠ Venv corrotto (manca python): {venv_path}")
            continue
        
        try:
            result = subprocess.run(
                [python_exe, "-c", "import torch; print('CUDA_OK' if torch.cuda.is_available() else 'CUDA_NO')"],
                capture_output=True, text=True, timeout=30
            )
            if result.returncode == 0:
                output = result.stdout.strip()
                if output == "CUDA_OK":
                    print(f"✓ CUDA in {venv_path}")
                    venvs_with_cuda += 1
                else:
                    print(f"✗ CUDA NON disponibile in {venv_path}")
            else:
                print(f"✗ Errore in {venv_path}: {result.stderr.strip()}")
        except subprocess.TimeoutExpired:
            print(f"⚠ Timeout verifica CUDA in {venv_path}")
        except Exception as e:
            print(f"✗ Errore in {venv_path}: {e}")
    
    # Se non ci sono venv esistenti, ritorna True (nessun modello installato = OK)
    if existing_venvs == 0:
        return True
    
    # Se tutti i venv esistenti hanno CUDA, ritorna True
    return venvs_with_cuda == existing_venvs