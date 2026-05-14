# audiobook_generator/setup_helpers/environment_utils.py
import subprocess
import sys
import logging
from .system_utils import command_exists, run_command

logger = logging.getLogger(__name__)

def detect_nvidia_gpu():
    """Checks if an NVIDIA GPU is present."""
    return command_exists("nvidia-smi")

def detect_apple_silicon():
    """Checks if we are on macOS with Apple Silicon."""
    if sys.platform != "darwin":
        return False
    try:
        result = subprocess.run(["uname", "-m"], capture_output=True, text=True, check=True)
        return "arm" in result.stdout.strip().lower()
    except (subprocess.CalledProcessError, FileNotFoundError):
        return False

def install_pytorch(pytorch_choice, pip_exe):
    """Installs PyTorch based on the choice."""
    if pytorch_choice == "skip":
        print("PyTorch installation skipped.")
        return True
    
    base_cmd = [pip_exe, "install", "torch", "torchvision", "torchaudio"]
    if pytorch_choice == "cuda121":
        cmd = base_cmd + ["--index-url", "https://download.pytorch.org/whl/cu121"]
    elif pytorch_choice == "cuda118":
        cmd = base_cmd + ["--index-url", "https://download.pytorch.org/whl/cu118"]
    else: # cpu or mps
        cmd = base_cmd

    print(f"Installing PyTorch for {pytorch_choice}...")
    return run_command(cmd)

def detect_recommended_cuda():
    """Determines the recommended CUDA version based on the NVIDIA driver."""
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
    """Checks if PyTorch with CUDA support is installed in a given Python environment."""
    try:
        result = subprocess.run(
            [python_exe, "-c", "import torch; print(torch.cuda.is_available())"],
            capture_output=True, text=True, check=True, timeout=30
        )
        return "True" in result.stdout.strip()
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return False

def detect_gpu():
    """Checks if an NVIDIA GPU is present."""
    try:
        subprocess.check_output("nvidia-smi", shell=True)
        return True
    except (subprocess.CalledProcessError, FileNotFoundError):
        return False

def check_pytorch_cuda_all_venvs():
    """Checks if PyTorch CUDA is installed in TTS model venvs.
    
    Returns:
        - True if all existing venvs have CUDA available
        - False if at least one existing venv does not have CUDA
    Non-existent venvs are ignored (model not yet downloaded).
    """
    import os
    # Only TTS model venvs, not the main AG venv
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
            # Venv does not exist - model not installed, skip
            continue
        
        existing_venvs += 1
        
        if sys.platform == "win32":
            python_exe = os.path.join(venv_path, "Scripts", "python.exe")
        else:
            python_exe = os.path.join(venv_path, "bin", "python")
        
        if not os.path.exists(python_exe):
            # Corrupted venv
            logger.warning("Corrupted venv (missing python): %s", venv_path)
            continue

        try:
            result = subprocess.run(
                [python_exe, "-c", "import torch; print('CUDA_OK' if torch.cuda.is_available() else 'CUDA_NO')"],
                capture_output=True, text=True, timeout=30
            )
            if result.returncode == 0:
                output = result.stdout.strip()
                if output == "CUDA_OK":
                    logger.debug("CUDA OK in %s", venv_path)
                    venvs_with_cuda += 1
                else:
                    logger.debug("CUDA NOT available in %s", venv_path)
            else:
                logger.debug("Error checking CUDA in %s: %s", venv_path, result.stderr.strip())
        except subprocess.TimeoutExpired:
            logger.warning("Timeout verifying CUDA in %s", venv_path)
        except Exception as e:
            logger.warning("Error verifying CUDA in %s: %s", venv_path, e)
    
    # If no existing venvs, return True (no models installed = OK)
    if existing_venvs == 0:
        return True
    
    # If all existing venvs have CUDA, return True
    return venvs_with_cuda == existing_venvs