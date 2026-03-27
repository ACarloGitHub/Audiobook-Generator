# audiobook_generator/setup_helpers/system_utils.py
import subprocess
import sys
import shutil
import os
import threading
import queue
import time

def run_command(command, cwd=None, idle_timeout=1800):
    """Esegue un comando di sistema e stampa l'output in tempo reale."""
    print(f"--- Eseguendo: {' '.join(command)} ---")
    try:
        process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, encoding='utf-8', errors='replace', cwd=cwd)
    except FileNotFoundError:
        print(f"ERRORE: Comando non trovato: {command[0]}. Assicurati che sia installato e nel PATH.")
        return False
    except Exception as e:
        print(f"ERRORE imprevisto durante l'avvio del comando: {e}")
        return False

    output_queue = queue.Queue()
    def output_reader(pipe, queue):
        try:
            for line in iter(pipe.readline, ''):
                queue.put(line)
        finally:
            pipe.close()

    reader_thread = threading.Thread(target=output_reader, args=(process.stdout, output_queue))
    reader_thread.daemon = True
    reader_thread.start()
    
    last_output_time = time.time()
    while True:
        try:
            line = output_queue.get(timeout=1.0)
            print(line.strip())
            last_output_time = time.time()
        except queue.Empty:
            if time.time() - last_output_time > idle_timeout:
                print(f"ATTENZIONE: Timeout di inattività ({idle_timeout}s). Processo terminato.")
                process.terminate()
                try: process.wait(timeout=5)
                except subprocess.TimeoutExpired: process.kill()
                return False
        if process.poll() is not None:
            break

    reader_thread.join(timeout=2)
    return process.returncode == 0

def command_exists(command):
    """Verifica se un comando esiste nel PATH di sistema."""
    return shutil.which(command) is not None

def get_python_executable(version="3.11"):
    """Trova l'eseguibile Python per una data versione."""
    if sys.platform == "win32":
        if command_exists("py"):
            try:
                result = subprocess.run(["py", f"-{version}", "--version"], capture_output=True, text=True, timeout=5)
                if result.returncode == 0:
                    return ["py", f"-{version}"]
            except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
                pass
        if command_exists("python"): return ["python"]
    else:
        for cmd in [f"python{version}", "python3", "python"]:
            if command_exists(cmd):
                return [cmd]
    return None

def clone_repo(repo_url, dest_path, progress=True):
    """Clona un repository Git."""
    if os.path.exists(dest_path):
        print(f"La cartella '{dest_path}' esiste già. Download saltato.")
        return True
    cmd = ["git", "clone", repo_url, dest_path]
    if progress: cmd.insert(2, "--progress")
    return run_command(cmd)

def remove_directory(path):
    """Elimina ricorsivamente una directory se esiste."""
    if os.path.exists(path):
        print(f"Rimozione directory '{path}'...")
        try:
            shutil.rmtree(path, ignore_errors=True)
            print(f"Directory '{path}' rimossa.")
        except Exception as e:
            print(f"ERRORE durante la rimozione di '{path}': {e}")
            return False
    return True

def check_venv_integrity(venv_path):
    """Verifica l'integrità di un ambiente virtuale."""
    if not os.path.exists(venv_path):
        return False
    
    bin_dir = "Scripts" if sys.platform == "win32" else "bin"
    python_exe = os.path.join(venv_path, bin_dir, "python.exe" if sys.platform == "win32" else "python")
    
    if not os.path.exists(python_exe):
        return False
        
    try:
        result = subprocess.run([python_exe, "-c", "import sys; print('VENV_OK')"], check=True, capture_output=True, text=True, timeout=10)
        return "VENV_OK" in result.stdout
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return False