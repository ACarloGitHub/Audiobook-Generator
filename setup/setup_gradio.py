#!/usr/bin/env python3
"""
Audiobook Generator - Setup Manager (Gradio Interface)

Architecture:
  - start_setup_gradio.sh: creates setup_venv/ if it doesn't exist, then launches this script
  - setup_gradio.py: verified to be inside setup_venv, then launches the Gradio interface
  - From inside Gradio: other venvs are created (main venv + models)

Requirement: ALWAYS run via ./start_setup_gradio.sh
"""
import sys
import os
import subprocess
import shutil
import json
import time
from pathlib import Path

# ========================================
# PHASE 1: BOOTSTRAP (STANDARD LIBRARY ONLY)
# ========================================

def get_project_root():
    return Path(__file__).resolve().parent.parent

def get_setup_venv_path():
    return get_project_root() / "setup_venv"

def is_running_in_setup_venv():
    setup_venv = get_setup_venv_path()
    if sys.platform == "win32":
        venv_python = setup_venv / "Scripts" / "python.exe"
    else:
        venv_python = setup_venv / "bin" / "python"
    return venv_python.resolve() == Path(sys.executable).resolve()

def find_python311():
    candidates = []
    if sys.platform == "win32":
        candidates = [
            ["py", "-3.11"],
            ["python3.11"],
            ["python3"],
            ["python"]
        ]
    else:
        candidates = [
            ["python3.11"],
            ["python3"],
            ["python"]
        ]
    
    for cmd in candidates:
        try:
            result = subprocess.run(
                cmd + ["--version"],
                capture_output=True,
                text=True,
                timeout=5
            )
            if result.returncode == 0 and "3.11" in result.stdout:
                ver_check = subprocess.run(
                    cmd + ["-c", "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')"],
                    capture_output=True,
                    text=True,
                    timeout=5
                )
                if ver_check.returncode == 0 and ver_check.stdout.strip() == "3.11":
                    return cmd
        except (FileNotFoundError, subprocess.TimeoutExpired, subprocess.CalledProcessError):
            continue
    return None

def create_setup_venv(python_cmd):
    setup_venv = get_setup_venv_path()
    
    if setup_venv.exists():
        print(f"⚠️  Existing venv found: {setup_venv}")
        print("   Verifying integrity...")
        try:
            if sys.platform == "win32":
                test_py = setup_venv / "Scripts" / "python.exe"
            else:
                test_py = setup_venv / "bin" / "python"
            
            if test_py.exists():
                result = subprocess.run(
                    [str(test_py), "-c", "print('OK')"],
                    capture_output=True,
                    text=True,
                    timeout=5
                )
                if result.returncode == 0 and "OK" in result.stdout:
                    print("✅ Venv valid, skipping creation")
                    return True
            print("⚠️  Corrupted venv, removing it...")
            shutil.rmtree(setup_venv, ignore_errors=True)
        except Exception as e:
            print(f"⚠️  Error verifying venv, removing it: {e}")
            shutil.rmtree(setup_venv, ignore_errors=True)
    
    print(f"🐍 Creating setup_venv at: {setup_venv}")
    cmd = python_cmd + ["-m", "venv", str(setup_venv)]
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"❌ Venv creation failed:\n{result.stderr}")
        return False
    print("✅ Venv created successfully")
    return True

def install_requirements_in_venv():
    setup_venv = get_setup_venv_path()
    if sys.platform == "win32":
        pip_exe = setup_venv / "Scripts" / "pip.exe"
    else:
        pip_exe = setup_venv / "bin" / "pip"
    
    req_file = get_project_root() / "requirements" / "requirements-base.txt"
    if not req_file.exists():
        print(f"❌ requirements-base.txt not found: {req_file}")
        return False
    
    print("📦 Installing base dependencies (gradio, requests, ...) in setup_venv...")
    cmd = [str(pip_exe), "install", "-r", str(req_file), "--no-warn-script-location"]
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"❌ Installation failed:\n{result.stderr}")
        return False
    print("✅ Dependencies installed successfully")
    return True

def restart_in_setup_venv():
    setup_venv = get_setup_venv_path()
    if sys.platform == "win32":
        venv_python = setup_venv / "Scripts" / "python.exe"
    else:
        venv_python = setup_venv / "bin" / "python"
    
    if not venv_python.exists():
        print(f"❌ Python not found at {venv_python}")
        sys.exit(1)
    
    print(f"\n🔄 Restarting inside setup_venv: {venv_python}")
    print("   This is normal - the script is self-configuring\n")
    
    new_args = [str(venv_python)] + sys.argv + ["--bootstrapped"]
    os.execv(str(venv_python), new_args)

def bootstrap_phase():
    """Verifies that the script is run INSIDE setup_venv.
    The bootstrap (venv creation) is now handled by start_setup_gradio.sh"""
    
    if is_running_in_setup_venv():
        # We're inside setup_venv - all good
        try:
            import gradio
            return
        except ImportError:
            print("⚠️  Gradio not found in setup_venv/")
            print("   Retry: Run ./start_setup_gradio.sh again")
            sys.exit(1)
    
    # We're not inside setup_venv - error!
    print("\n" + "="*60)
    print("❌ ERROR: setup_gradio.py must be run via:")
    print()
    print("   ./start_setup_gradio.sh")
    print()
    print("DO NOT run it directly with 'python setup/setup_gradio.py'")
    print()
    print("The start_setup_gradio.sh script creates setup_venv/ if it doesn't exist")
    print("and then launches this script from inside the venv.")
    print("="*60 + "\n")
    sys.exit(1)

# ========================================
# PHASE 2: APPLICATION (AFTER RESTART)
# ========================================

def application_phase():
    import gradio as gr
    from helpers import (
        run_command, command_exists, get_python_executable,
        download_vibevoice_model_multiple, download_qwen3tts_model,
        download_xttsv2_model, download_kokoro_model,
        setup_ffmpeg, setup_sox, check_venv_integrity,
        detect_gpu, detect_recommended_cuda, check_pytorch_cuda_all_venvs,
        update_plugin_registry_with_lock,
        detect_apple_silicon
    )
    import logging
    from datetime import datetime
    import threading
    import queue
    
    BASE_DIR = Path(__file__).resolve().parent
    LOG_DIR = BASE_DIR / "setup_logs"
    LOG_DIR.mkdir(exist_ok=True)
    log_file = LOG_DIR / f"setup_gradio_{datetime.now().strftime('%Y%m%d_%H%M%S')}.log"
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(levelname)s - %(message)s',
        handlers=[
            logging.FileHandler(log_file, encoding='utf-8'),
            logging.StreamHandler(sys.stdout)
        ]
    )
    logger = logging.getLogger(__name__)
    
    # Global single queue for ALL heavy tasks (venv + download)
    task_queue = queue.Queue()
    task_thread = None
    task_status = {"running": False, "current": None, "message": ""}
    
    def get_plugin_registry():
        registry_path = BASE_DIR.parent / "audiobook_generator" / "plugins" / "plugin_registry.json"
        if not registry_path.exists():
            return []
        try:
            with open(registry_path, 'r', encoding='utf-8') as f:
                return json.load(f)
        except Exception as e:
            logger.error(f"Error reading plugin registry: {e}")
            return []
    
    def get_model_status_display():
        registry = get_plugin_registry()
        vibevoice_models = []
        qwen_models = []
        other_models = []
        for plugin in registry:
            name = plugin["name"]
            if name == "mock":
                continue
            if name.startswith("VibeVoice"):
                vibevoice_models.append(plugin)
            elif name.startswith("Qwen3-TTS"):
                qwen_models.append(plugin)
            else:
                other_models.append(plugin)
        
        lines = []
        if vibevoice_models:
            lines.append("### 🎵 VibeVoice Models")
            for plugin in vibevoice_models:
                model_name = plugin["name"]
                # Verify real filesystem - folder names are 0.5B, 1.5B, 7B
                folder_map = {
                    "VibeVoice-Realtime-0.5B": "0.5B",
                    "VibeVoice-1.5B": "1.5B",
                    "VibeVoice-7B": "7B",
                }
                folder_name = folder_map.get(model_name)
                if not folder_name:
                    lines.append(f"- **{model_name}**: ❌ Unrecognized name")
                    continue
                model_dir = BASE_DIR.parent / "audiobook_generator" / "tts_models" / "vibevoice" / folder_name

                if model_dir.exists():
                    # Essential files: config + model
                    # For sharded models, look for first shard
                    essential_files = ["config.json", "preprocessor_config.json"]
                    # Check if model.safetensors exists (0.5B) or first shard (1.5B, 7B)
                    has_model = (
                        (model_dir / "model.safetensors").exists() or
                        (model_dir / "model-00001-of-00003.safetensors").exists() or
                        (model_dir / "model-00001-of-00010.safetensors").exists()
                    )
                    has_files = all((model_dir / f).exists() for f in essential_files) and has_model
                    status = "✅ Installed (verified)" if has_files else "⚠️ Incomplete"
                else:
                    status = "❌ Missing"
                lines.append(f"- **{model_name}**: {status}")
        if qwen_models:
            lines.append("\n### 🤖 Qwen3-TTS Models")
            for plugin in qwen_models:
                model_name = plugin["name"]
                # Map plugin name -> folder name (matches HuggingFace name without org)
                folder_map = {
                    "Qwen3-TTS-0.6B-Base": "Qwen3-TTS-12Hz-0.6B-Base",
                    "Qwen3-TTS-1.7B-Base": "Qwen3-TTS-12Hz-1.7B-Base",
                    "Qwen3-TTS-1.7B-CustomVoice": "Qwen3-TTS-12Hz-1.7B-CustomVoice",
                    "Qwen3-TTS-1.7B-VoiceDesign": "Qwen3-TTS-12Hz-1.7B-VoiceDesign",
                }
                folder_name = folder_map.get(model_name)
                if not folder_name:
                    lines.append(f"- **{model_name}**: ❌ Unrecognized name")
                    continue
                # Correct path: no extra 'models/' 
                model_dir = BASE_DIR.parent / "audiobook_generator" / "tts_models" / "qwen3tts" / folder_name

                if model_dir.exists():
                    # Essential files: config + model
                    essential_files = ["config.json", "generation_config.json", "preprocessor_config.json"]
                    has_config = all((model_dir / f).exists() for f in essential_files)
                    has_model = (model_dir / "model.safetensors").exists()
                    has_files = has_config and has_model
                    status = "✅ Installed (verified)" if has_files else "⚠️ Incomplete"
                else:
                    status = "❌ Missing"
                lines.append(f"- **{model_name}**: {status}")
        if other_models:
            lines.append("\n### 📦 Other Models")
            for plugin in other_models:
                model_name = plugin["name"]
                status_already_set = False  # Flag to avoid overwrite
                
                if model_name == "XTTSv2":
                    model_dir = BASE_DIR.parent / "audiobook_generator" / "tts_models" / "xttsv2"
                    essential_files = ["config.json", "model.pth", "dvae.pth"]
                elif model_name == "Kokoro":
                    # Kokoro uses HuggingFace cache structure
                    model_dir = BASE_DIR.parent / "audiobook_generator" / "tts_models" / "kokoro" / "models" / "hub" / "models--hexgrad--Kokoro-82M" / "snapshots"
                    # Find snapshot folder (hash)
                    if model_dir.exists():
                        snapshots = list(model_dir.iterdir())
                        if snapshots:
                            snapshot_dir = snapshots[0]  # Takes the first (and only) snapshot
                            essential_files = ["config.json", "kokoro-v1_0.pth"]
                            has_files = all((snapshot_dir / f).exists() for f in essential_files)
                            status = "✅ Installed (verified)" if has_files else "⚠️ Incomplete"
                        else:
                            status = "❌ Missing (no snapshot)"
                    else:
                        status = "❌ Missing"
                    status_already_set = True  # Kokoro already set status
                else:
                    model_dir = None
                    essential_files = []
                
                if not status_already_set:
                    if model_dir and model_dir.exists():
                        has_files = all((model_dir / f).exists() for f in essential_files)
                        status = "✅ Installed (verified)" if has_files else "⚠️ Incomplete"
                    else:
                        status = "❌ Missing"
                
                lines.append(f"- **{model_name}**: {status}")
        return "\n".join(lines)
    
    def get_venv_status():
        venv_paths = [
            ("venv (main)", "venv"),
            ("VibeVoice", "audiobook_generator/tts_models/vibevoice/venv"),
            ("Qwen3-TTS", "audiobook_generator/tts_models/qwen3tts/venv"),
            ("Kokoro", "audiobook_generator/tts_models/kokoro/venv"),
            ("XTTSv2", "audiobook_generator/tts_models/xttsv2/venv")
        ]
        lines = ["### 🐍 Virtual Environments"]
        for name, path in venv_paths:
            full_path = BASE_DIR.parent / path
            if full_path.exists():
                if check_venv_integrity(str(full_path)):
                    lines.append(f"- **{name}**: ✅ Valid ({path})")
                else:
                    lines.append(f"- **{name}**: ⚠️ Corrupted or incomplete ({path})")
            else:
                lines.append(f"- **{name}**: ❌ Not present ({path})")
        
        lines.append("\n### 🎮 GPU Support (CUDA)")
        gpu_detected = detect_gpu()
        if gpu_detected:
            lines.append("- **NVIDIA GPU**: ✅ Detected")
            cuda_recommended = detect_recommended_cuda()
            if cuda_recommended:
                lines.append(f"- **Recommended CUDA**: {cuda_recommended}")
            else:
                lines.append("- **Recommended CUDA**: ⚠️ Unable to determine")
            cuda_status = check_pytorch_cuda_all_venvs()
            if cuda_status is True:
                lines.append("- **PyTorch CUDA**: ✅ Installed in all environments")
            elif cuda_status is False:
                lines.append("- **PyTorch CUDA**: ❌ Missing in some environments")
            else:
                lines.append("- **PyTorch CUDA**: ⚠️ Status indeterminate")
        else:
            lines.append("- **NVIDIA GPU**: ❌ Not detected (CPU mode)")
        return "\n".join(lines)
    
    def get_system_status():
        lines = ["### 🖥️ System and Dependencies"]
        ffmpeg_local = BASE_DIR.parent / "ffmpeg" / "bin" / ("ffmpeg.exe" if sys.platform == "win32" else "ffmpeg")
        if command_exists("ffmpeg") or ffmpeg_local.exists():
            lines.append("- **FFmpeg**: ✅ Present")
        else:
            lines.append("- **FFmpeg**: ❌ Missing")
        
        sox_local = BASE_DIR.parent / "sox" / "bin" / ("sox.exe" if sys.platform == "win32" else "sox")
        if command_exists("sox") or sox_local.exists():
            lines.append("- **SoX**: ✅ Present")
        else:
            lines.append("- **SoX**: ❌ Missing")
        
        python_exe = get_python_executable("3.11")
        if python_exe:
            lines.append("- **Python 3.11**: ✅ Available")
        else:
            lines.append("- **Python 3.11**: ❌ Not found")
        
        lines.append(f"- **System**: {sys.platform}")
        return "\n".join(lines)
    
    def task_worker():
        nonlocal task_status
        while True:
            try:
                task = task_queue.get(timeout=1)
                if task is None:
                    break
                task_type, task_name, func, args, kwargs = task
                task_status["running"] = True
                task_status["current"] = task_name
                task_status["message"] = f"Starting {task_name}..."
                try:
                    result = func(*args, **kwargs)
                    task_status["message"] = f"✅ {task_name} completed!"
                except Exception as e:
                    logger.error(f"Error during {task_name}: {e}")
                    task_status["message"] = f"❌ Error during {task_name}: {str(e)}"
                finally:
                    task_status["running"] = False
                    task_status["current"] = None
                    task_queue.task_done()
                    time.sleep(2)
            except queue.Empty:
                continue
            except Exception as e:
                logger.error(f"Error in task worker: {e}")
                task_status["running"] = False
                task_status["current"] = None
    
    def start_task_thread():
        nonlocal task_thread
        if task_thread is None or not task_thread.is_alive():
            task_thread = threading.Thread(target=task_worker, daemon=True)
            task_thread.start()
            logger.info("Task thread started")
    
    def queue_task(task_type, task_name, func, *args, **kwargs):
        task_queue.put((task_type, task_name, func, args, kwargs))
        start_task_thread()
        return f"{task_name} added to queue"
    
    def get_task_status():
        return (
            task_status["running"],
            task_status["current"] or "",
            task_status["message"]
        )
    
    def update_all_status():
        model_status = get_model_status_display()
        venv_status = get_venv_status()
        system_status = get_system_status()
        task_running, current_task, message = get_task_status()
        status_text = f"## 📊 System Status\n{system_status}\n{venv_status}\n{model_status}"
        if task_running:
            status_text += f"\n## ⏳ Operation in Progress\n**Task**: {current_task}\n**Status**: {message}"
        return status_text
    
    # --- Download functions ---
    def download_vibevoice_15b():
        return queue_task("download", "VibeVoice-1.5B", download_vibevoice_model_multiple, "1.5B", idle_timeout=7200)
    
    def download_vibevoice_7b():
        return queue_task("download", "VibeVoice-7B", download_vibevoice_model_multiple, "7B", idle_timeout=7200)
    
    def download_vibevoice_realtime():
        return queue_task("download", "VibeVoice-Realtime-0.5B", download_vibevoice_model_multiple, "Realtime-0.5B", idle_timeout=7200)
    
    def download_qwen_06b_base():
        return queue_task("download", "Qwen3-TTS-0.6B-Base", download_qwen3tts_model, "0.6B", "base", idle_timeout=7200)
    
    def download_qwen_17b_base():
        return queue_task("download", "Qwen3-TTS-1.7B-Base", download_qwen3tts_model, "1.7B", "base", idle_timeout=7200)
    
    def download_qwen_17b_custom():
        return queue_task("download", "Qwen3-TTS-1.7B-CustomVoice", download_qwen3tts_model, "1.7B", "custom_voice", idle_timeout=7200)
    
    def download_qwen_17b_design():
        return queue_task("download", "Qwen3-TTS-1.7B-VoiceDesign", download_qwen3tts_model, "1.7B", "voice_design", idle_timeout=7200)
    
    def download_xttsv2():
        return queue_task("download", "XTTSv2", download_xttsv2_model, idle_timeout=7200)
    
    def download_kokoro():
        return queue_task("download", "Kokoro", download_kokoro_model, idle_timeout=7200)
    
    def download_all_vibevoice():
        results = []
        results.append(download_vibevoice_15b())
        results.append(download_vibevoice_7b())
        results.append(download_vibevoice_realtime())
        return "\n".join(results)
    
    def download_all_qwen():
        results = []
        results.append(download_qwen_06b_base())
        results.append(download_qwen_17b_base())
        results.append(download_qwen_17b_custom())
        results.append(download_qwen_17b_design())
        return "\n".join(results)
    
    def download_all_models():
        results = []
        results.append(download_all_vibevoice())
        results.append(download_all_qwen())
        results.append(download_xttsv2())
        results.append(download_kokoro())
        return "\n".join(results)
    
    # --- Venv functions ---
    def setup_venv_main(force_recreate=False):
        venv_path = BASE_DIR.parent / "venv"
        if venv_path.exists() and not force_recreate:
            if check_venv_integrity(str(venv_path)):
                return "✅ Main venv already present and valid"
            else:
                try:
                    shutil.rmtree(venv_path)
                except Exception as e:
                    return f"❌ Cannot remove corrupted venv: {e}"
        
        python_exe = get_python_executable("3.11")
        if not python_exe:
            return "❌ Python 3.11 not found"
        
        cmd = python_exe + ["-m", "venv", str(venv_path)]
        success = run_command(cmd, idle_timeout=1800)
        if not success:
            return "❌ Main venv creation failed"
        
        if sys.platform == "win32":
            pip_exe = venv_path / "Scripts" / "pip.exe"
        else:
            pip_exe = venv_path / "bin" / "pip"
        
        run_command([str(pip_exe), "install", "--upgrade", "pip"], idle_timeout=600)
        req_file = BASE_DIR.parent / "requirements" / "requirements-base.txt"
        if req_file.exists():
            run_command([str(pip_exe), "install", "-r", str(req_file)], idle_timeout=1800)
        return "✅ Main venv created (PyTorch NOT installed - install manually if needed)"
    
    def setup_venv_vibevoice(force_recreate=False):
        venv_path = BASE_DIR.parent / "audiobook_generator" / "tts_models" / "vibevoice" / "venv"
        if venv_path.exists() and not force_recreate:
            if check_venv_integrity(str(venv_path)):
                return "✅ VibeVoice venv already present and valid"
            else:
                try:
                    shutil.rmtree(venv_path)
                except Exception as e:
                    return f"❌ Cannot remove corrupted venv: {e}"
        
        python_exe = get_python_executable("3.11")
        if not python_exe:
            return "❌ Python 3.11 not found"
        
        cmd = python_exe + ["-m", "venv", str(venv_path)]
        success = run_command(cmd, idle_timeout=1800)
        if not success:
            return "❌ VibeVoice venv creation failed"
        
        if sys.platform == "win32":
            pip_exe = venv_path / "Scripts" / "pip.exe"
        else:
            pip_exe = venv_path / "bin" / "pip"
        
        run_command([str(pip_exe), "install", "--upgrade", "pip"], idle_timeout=600)
        gpu_detected = detect_gpu()
        if gpu_detected:
            cuda_recommended = detect_recommended_cuda()
            if cuda_recommended == "cuda121":
                run_command([str(pip_exe), "install", "torch", "torchvision", "torchaudio", "--index-url", "https://download.pytorch.org/whl/cu121"], idle_timeout=1800)
            else:
                run_command([str(pip_exe), "install", "torch", "torchvision", "torchaudio", "--index-url", "https://download.pytorch.org/whl/cu118"], idle_timeout=1800)
        else:
            run_command([str(pip_exe), "install", "torch", "torchvision", "torchaudio", "--index-url", "https://download.pytorch.org/whl/cpu"], idle_timeout=1800)
        
        run_command([str(pip_exe), "install", "transformers", "accelerate", "safetensors", "huggingface-hub"], idle_timeout=1800)
        
        # Install VibeVoice-specific requirements
        vibevoice_req = BASE_DIR.parent / "audiobook_generator" / "plugins" / "vibevoice" / "requirements-vibevoice.txt"
        if vibevoice_req.exists():
            run_command([str(pip_exe), "install", "-r", str(vibevoice_req)], idle_timeout=1800)
        
        return "✅ VibeVoice venv configured successfully"
    
    def setup_venv_qwen(force_recreate=False):
        # Venv in model folder, not in .venvs
        venv_path = BASE_DIR.parent / "audiobook_generator" / "tts_models" / "qwen3tts" / "venv"
        if venv_path.exists() and not force_recreate:
            if check_venv_integrity(str(venv_path)):
                return "✅ Qwen3-TTS venv already present and valid"
            else:
                try:
                    shutil.rmtree(venv_path)
                except Exception as e:
                    return f"❌ Cannot remove corrupted venv: {e}"
        
        # Qwen3-TTS requires Python >= 3.12 for full support
        python_exe = get_python_executable("3.12")
        if not python_exe:
            return "❌ Python 3.12 not found (required for Qwen3-TTS)"
        
        cmd = python_exe + ["-m", "venv", str(venv_path)]
        success = run_command(cmd, idle_timeout=1800)
        if not success:
            return "❌ Qwen3-TTS venv creation failed"
        
        if sys.platform == "win32":
            pip_exe = venv_path / "Scripts" / "pip.exe"
        else:
            pip_exe = venv_path / "bin" / "pip"
        
        run_command([str(pip_exe), "install", "--upgrade", "pip"], idle_timeout=600)
        gpu_detected = detect_gpu()
        if gpu_detected:
            cuda_recommended = detect_recommended_cuda()
            if cuda_recommended == "cuda121":
                run_command([str(pip_exe), "install", "torch", "torchvision", "torchaudio", "--index-url", "https://download.pytorch.org/whl/cu121"], idle_timeout=1800)
            else:
                run_command([str(pip_exe), "install", "torch", "torchvision", "torchaudio", "--index-url", "https://download.pytorch.org/whl/cu118"], idle_timeout=1800)
        else:
            run_command([str(pip_exe), "install", "torch", "torchvision", "torchaudio", "--index-url", "https://download.pytorch.org/whl/cpu"], idle_timeout=1800)
        
        run_command([str(pip_exe), "install", "transformers", "accelerate", "safetensors", "huggingface-hub", "soundfile"], idle_timeout=1800)
        
        # Install Qwen3-TTS-specific requirements
        qwen_req = BASE_DIR.parent / "audiobook_generator" / "plugins" / "qwen3tts" / "requirements-qwen3tts.txt"
        if qwen_req.exists():
            run_command([str(pip_exe), "install", "-r", str(qwen_req)], idle_timeout=1800)
        
        return "✅ Qwen3-TTS venv configured successfully"
    
    def setup_venv_kokoro(force_recreate=False):
        venv_path = BASE_DIR.parent / "audiobook_generator" / "tts_models" / "kokoro" / "venv"
        if venv_path.exists() and not force_recreate:
            if check_venv_integrity(str(venv_path)):
                return "✅ Kokoro venv already present and valid"
            else:
                try:
                    shutil.rmtree(venv_path)
                except Exception as e:
                    return f"❌ Cannot remove corrupted venv: {e}"
        
        python_exe = get_python_executable("3.11")
        if not python_exe:
            return "❌ Python 3.11 not found"
        
        cmd = python_exe + ["-m", "venv", str(venv_path)]
        success = run_command(cmd, idle_timeout=1800)
        if not success:
            return "❌ Kokoro venv creation failed"
        
        if sys.platform == "win32":
            pip_exe = venv_path / "Scripts" / "pip.exe"
        else:
            pip_exe = venv_path / "bin" / "pip"
        
        run_command([str(pip_exe), "install", "--upgrade", "pip"], idle_timeout=600)
        
        # Install kokoro - pip will automatically install torch and all dependencies
        # Working test shows pip installs torch 2.10.0+cu128 with CUDA automatically
        run_command([str(pip_exe), "install", "kokoro>=0.9.4", "soundfile"], idle_timeout=3600)
        
        # Install Kokoro-specific requirements
        kokoro_req = BASE_DIR.parent / "audiobook_generator" / "plugins" / "kokoro" / "requirements-kokoro.txt"
        if kokoro_req.exists():
            run_command([str(pip_exe), "install", "-r", str(kokoro_req)], idle_timeout=1800)
        
        return "✅ Kokoro venv configured successfully"
    
    def setup_venv_xttsv2(force_recreate=False):
        venv_path = BASE_DIR.parent / "audiobook_generator" / "tts_models" / "xttsv2" / "venv"
        if venv_path.exists() and not force_recreate:
            if check_venv_integrity(str(venv_path)):
                return "✅ XTTSv2 venv already present and valid"
            else:
                try:
                    shutil.rmtree(venv_path)
                except Exception as e:
                    return f"❌ Cannot remove corrupted venv: {e}"
        
        python_exe = get_python_executable("3.11")
        if not python_exe:
            return "❌ Python 3.11 not found"
        
        cmd = python_exe + ["-m", "venv", str(venv_path)]
        success = run_command(cmd, idle_timeout=1800)
        if not success:
            return "❌ XTTSv2 venv creation failed"
        
        if sys.platform == "win32":
            pip_exe = venv_path / "Scripts" / "pip.exe"
        else:
            pip_exe = venv_path / "bin" / "pip"
        
        run_command([str(pip_exe), "install", "--upgrade", "pip"], idle_timeout=600)
        gpu_detected = detect_gpu()
        if gpu_detected:
            cuda_recommended = detect_recommended_cuda()
            if cuda_recommended == "cuda121":
                run_command([str(pip_exe), "install", "torch", "torchvision", "torchaudio", "--index-url", "https://download.pytorch.org/whl/cu121"], idle_timeout=1800)
            else:
                run_command([str(pip_exe), "install", "torch", "torchvision", "torchaudio", "--index-url", "https://download.pytorch.org/whl/cu118"], idle_timeout=1800)
        else:
            run_command([str(pip_exe), "install", "torch", "torchvision", "torchaudio", "--index-url", "https://download.pytorch.org/whl/cpu"], idle_timeout=1800)
        
        run_command([str(pip_exe), "install", "TTS==0.22.0", "trainer==0.0.36", "coqpit==0.0.17", "transformers==4.36.2"], idle_timeout=1800)
        
        # Install XTTSv2-specific requirements
        xttsv2_req = BASE_DIR.parent / "audiobook_generator" / "plugins" / "xttsv2" / "requirements-xttsv2.txt"
        if xttsv2_req.exists():
            run_command([str(pip_exe), "install", "-r", str(xttsv2_req)], idle_timeout=1800)
        
        return "✅ XTTSv2 venv configured successfully"
    
    def setup_all_venvs(force_recreate=False):
        results = []
        results.append(queue_task("venv", "Main Venv", setup_venv_main, force_recreate))
        results.append(queue_task("venv", "VibeVoice Venv", setup_venv_vibevoice, force_recreate))
        results.append(queue_task("venv", "Qwen3-TTS Venv", setup_venv_qwen, force_recreate))
        results.append(queue_task("venv", "Kokoro Venv", setup_venv_kokoro, force_recreate))
        results.append(queue_task("venv", "XTTSv2 Venv", setup_venv_xttsv2, force_recreate))
        return "\n".join(results)
    
    
    # --- System dependencies functions ---
    def install_ffmpeg():
        success = setup_ffmpeg()
        return "✅ FFmpeg configured successfully" if success else "⚠️ FFmpeg not configured automatically. Install it manually."
    
    def install_sox():
        success = setup_sox()
        return "✅ SoX configured successfully" if success else "⚠️ SoX not configured automatically. Install it manually."
    
    def install_all_deps():
        results = []
        results.append(queue_task("deps", "FFmpeg", install_ffmpeg))
        results.append(queue_task("deps", "SoX", install_sox))
        return "\n".join(results)
    
    # --- Gradio Interface ---
    def create_interface(gr):
        with gr.Blocks(title="Audiobook Generator - Setup Manager") as app:
            gr.Markdown("# 🛠️ Audiobook Generator - Setup Manager")
            gr.Markdown("Graphical interface to manage TTS models and virtual environments setup.")
            
            with gr.Tabs():
                with gr.Tab("📊 System Status"):
                    status_display = gr.Markdown()
                    refresh_btn = gr.Button("🔄 Refresh Status", variant="secondary")
                    refresh_btn.click(fn=update_all_status, inputs=[], outputs=[status_display])
                
                with gr.Tab("⬇️ Model Download"):
                    gr.Markdown("## 🎵 VibeVoice Models")
                    with gr.Row():
                        btn_vibevoice_15b = gr.Button("Download VibeVoice-1.5B", variant="secondary")
                        btn_vibevoice_7b = gr.Button("Download VibeVoice-7B", variant="secondary")
                        btn_vibevoice_realtime = gr.Button("Download VibeVoice-Realtime-0.5B", variant="secondary")
                    btn_all_vibevoice = gr.Button("Download ALL VibeVoice", variant="primary")
                    
                    gr.Markdown("## 🤖 Qwen3-TTS Models")
                    with gr.Row():
                        btn_qwen_06b_base = gr.Button("Download Qwen3-TTS-0.6B-Base", variant="secondary")
                        btn_qwen_17b_base = gr.Button("Download Qwen3-TTS-1.7B-Base", variant="secondary")
                    with gr.Row():
                        btn_qwen_17b_custom = gr.Button("Download Qwen3-TTS-1.7B-CustomVoice", variant="secondary")
                        btn_qwen_17b_design = gr.Button("Download Qwen3-TTS-1.7B-VoiceDesign", variant="secondary")
                    btn_all_qwen = gr.Button("Download ALL Qwen3-TTS", variant="primary")
                    
                    gr.Markdown("## 📦 Other Models")
                    with gr.Row():
                        btn_xttsv2 = gr.Button("Download XTTSv2", variant="secondary")
                        btn_kokoro = gr.Button("Download Kokoro", variant="secondary")
                    
                    gr.Markdown("## 🚀 Complete Download")
                    btn_all_models = gr.Button("Download ALL models", variant="primary", size="lg")
                    download_output = gr.Textbox(label="Download Status", lines=5, interactive=False)
                    
                    btn_vibevoice_15b.click(fn=download_vibevoice_15b, inputs=[], outputs=[download_output])
                    btn_vibevoice_7b.click(fn=download_vibevoice_7b, inputs=[], outputs=[download_output])
                    btn_vibevoice_realtime.click(fn=download_vibevoice_realtime, inputs=[], outputs=[download_output])
                    btn_all_vibevoice.click(fn=download_all_vibevoice, inputs=[], outputs=[download_output])
                    btn_qwen_06b_base.click(fn=download_qwen_06b_base, inputs=[], outputs=[download_output])
                    btn_qwen_17b_base.click(fn=download_qwen_17b_base, inputs=[], outputs=[download_output])
                    btn_qwen_17b_custom.click(fn=download_qwen_17b_custom, inputs=[], outputs=[download_output])
                    btn_qwen_17b_design.click(fn=download_qwen_17b_design, inputs=[], outputs=[download_output])
                    btn_all_qwen.click(fn=download_all_qwen, inputs=[], outputs=[download_output])
                    btn_xttsv2.click(fn=download_xttsv2, inputs=[], outputs=[download_output])
                    btn_kokoro.click(fn=download_kokoro, inputs=[], outputs=[download_output])
                    btn_all_models.click(fn=download_all_models, inputs=[], outputs=[download_output])
                
                with gr.Tab("🐍 Virtual Environments"):
                    gr.Markdown("## Virtual Environments Configuration")
                    gr.Markdown("Each TTS model requires a specific virtual environment with optimized dependencies.")
                    with gr.Row():
                        btn_venv_main = gr.Button("Configure Main Venv", variant="secondary")
                        btn_venv_vibevoice = gr.Button("Configure VibeVoice Venv", variant="secondary")
                        btn_venv_qwen = gr.Button("Configure Qwen3-TTS Venv", variant="secondary")
                    with gr.Row():
                        btn_venv_kokoro = gr.Button("Configure Kokoro Venv", variant="secondary")
                        btn_venv_xttsv2 = gr.Button("Configure XTTSv2 Venv", variant="secondary")
                        btn_all_venvs = gr.Button("Configure ALL Venvs", variant="primary")
                    with gr.Row():
                        force_recreate_toggle = gr.Checkbox(label="Force recreation (ignore existing venvs)", value=False)
                    venv_output = gr.Textbox(label="Configuration Status", lines=5, interactive=False)
                    
                    btn_venv_main.click(fn=lambda f: queue_task("venv", "Main Venv", setup_venv_main, f), inputs=[force_recreate_toggle], outputs=[venv_output])
                    btn_venv_vibevoice.click(fn=lambda f: queue_task("venv", "VibeVoice Venv", setup_venv_vibevoice, f), inputs=[force_recreate_toggle], outputs=[venv_output])
                    btn_venv_qwen.click(fn=lambda f: queue_task("venv", "Qwen3-TTS Venv", setup_venv_qwen, f), inputs=[force_recreate_toggle], outputs=[venv_output])
                    btn_venv_kokoro.click(fn=lambda f: queue_task("venv", "Kokoro Venv", setup_venv_kokoro, f), inputs=[force_recreate_toggle], outputs=[venv_output])
                    btn_venv_xttsv2.click(fn=lambda f: queue_task("venv", "XTTSv2 Venv", setup_venv_xttsv2, f), inputs=[force_recreate_toggle], outputs=[venv_output])
                    btn_all_venvs.click(fn=lambda f: setup_all_venvs(f), inputs=[force_recreate_toggle], outputs=[venv_output])
                
                with gr.Tab("🖥️ System Dependencies"):
                    gr.Markdown("## System Dependencies")
                    gr.Markdown("FFmpeg and SoX are required for audio processing.")
                    with gr.Row():
                        btn_ffmpeg = gr.Button("Install FFmpeg", variant="secondary")
                        btn_sox = gr.Button("Install SoX", variant="secondary")
                        btn_all_deps = gr.Button("Install ALL dependencies", variant="primary")
                    deps_output = gr.Textbox(label="Installation Status", lines=5, interactive=False)
                    
                    btn_ffmpeg.click(fn=lambda: queue_task("deps", "FFmpeg", install_ffmpeg), inputs=[], outputs=[deps_output])
                    btn_sox.click(fn=lambda: queue_task("deps", "SoX", install_sox), inputs=[], outputs=[deps_output])
                    btn_all_deps.click(fn=install_all_deps, inputs=[], outputs=[deps_output])
                
                with gr.Tab("ℹ️ Information"):
                    gr.Markdown("## About Setup")
                    gr.Markdown(r"""
### 🎯 Features
- **Download Models**: Download all supported TTS models
- **Venv Configuration**: Create optimized virtual environments for each model
- **Dependencies Installation**: Automatically configure FFmpeg and SoX
- **Status Monitoring**: View the current state of all components

### 📁 Directory Structure
```
AudiobookGenerator/
├── audiobook_generator/tts_models/    # TTS Models
│   ├── vibevoice/                     # VibeVoice Models
│   │   └── venv/                    # VibeVoice venv
│   ├── qwen3tts/                     # Qwen3-TTS Models
│   │   └── venv/                    # Qwen3-TTS venv
│   ├── xttsv2/                       # XTTSv2 Model
│   │   └── venv/                    # XTTSv2 venv
│   └── kokoro/                       # Kokoro Model
│       └── venv/                    # Kokoro venv
├── venv/                              # Main venv
├── setup_venv/                        # DEDICATED venv for installer (NEVER deleted)
├── ffmpeg/                            # FFmpeg (if downloaded)
├── sox/                               # SoX (if downloaded)
├── setup_logs/                        # Setup logs
└── setup/                             # This script
```

### ⚙️ Usage
1. **First run**: automatic bootstrap → creates `setup_venv/` → restarts → opens interface
2. **Subsequent runs**: skip bootstrap → launch interface directly
3. **Zero conflicts**: each model has its own venv inside its folder → no risk of accidental deletion

### 📝 Notes
- Downloads can take a long time (up to 30+ GB for all models)
- Make sure you have a stable internet connection
- On Windows, automatic FFmpeg/SoX installation may require administrator permissions
- In case of errors, check the logs in `setup_logs/`
""")
            
            app.load(update_all_status, inputs=[], outputs=[status_display])
            return app
    
    print("="*60)
    print("Audiobook Generator - Setup Manager (Application)")
    print("="*60)
    print(f"\n✅ Bootstrap completed - launching Gradio interface...")
    print(f"📝 Logs saved at: {log_file}")
    print("\n🌐 Starting Gradio server at http://localhost:7861")
    print("   Press CTRL+C to stop\n")
    
    app = create_interface(gr)
    try:
        app.launch(
            server_name="127.0.0.1",
            server_port=7861,
            share=False,
            show_error=True,
            quiet=True
        )
    except KeyboardInterrupt:
        print("\n👋 Server stopped by user")
    except Exception as e:
        logger.exception("Error during Gradio startup")
        print(f"\n❌ Error: {e}")
        sys.exit(1)

# ========================================
# ENTRY POINT
# ========================================

if __name__ == "__main__":
    bootstrap_phase()
    application_phase()
