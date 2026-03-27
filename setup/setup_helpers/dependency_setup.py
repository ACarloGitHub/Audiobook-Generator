# audiobook_generator/setup_helpers/dependency_setup.py
import os
import sys
from .download_utils import download_file, extract_archive
from .system_utils import command_exists

def setup_ffmpeg():
    """Configura FFmpeg scaricandolo se non presente (solo per Windows)."""
    print("\n--- Verifica FFmpeg ---")
    if command_exists("ffmpeg"):
        print("FFmpeg è già nel PATH di sistema. OK.")
        return True
    
    if sys.platform == "win32":
        ffmpeg_dir = os.path.abspath("ffmpeg/bin")
        if os.path.exists(os.path.join(ffmpeg_dir, "ffmpeg.exe")):
            print("FFmpeg trovato nella cartella locale. OK.")
            return True
        
        print("FFmpeg non trovato. Tentativo di download automatico per Windows...")
        url = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
        archive_path = download_file(url, ".")
        if archive_path and extract_archive(archive_path, "."):
             # Sposta i file in una struttura più pulita
            extracted_folder = next(d for d in os.listdir('.') if d.startswith('ffmpeg-'))
            os.rename(extracted_folder, "ffmpeg")
            print("FFmpeg configurato con successo.")
            return True
        print("ERRORE: Download o estrazione di FFmpeg fallita.")
        return False
    else:
        print("Su Linux/macOS, installa FFmpeg con il gestore di pacchetti (es. `sudo apt install ffmpeg` o `brew install ffmpeg`).")
        return True # Non bloccare l'installazione

def setup_sox():
    """Configura SoX scaricandolo se non presente (solo per Windows)."""
    print("\n--- Verifica SoX ---")
    if command_exists("sox"):
        print("SoX è già nel PATH di sistema. OK.")
        return True
        
    if sys.platform == "win32":
        sox_dir = os.path.abspath("sox")
        if os.path.exists(os.path.join(sox_dir, "sox.exe")):
            print("SoX trovato nella cartella locale. OK.")
            return True
        
        print("SoX non trovato. Tentativo di download automatico per Windows...")
        url = "https://sourceforge.net/projects/sox/files/sox/14.4.2/sox-14.4.2-win32.zip/download"
        archive_path = download_file(url, ".")
        if archive_path and extract_archive(archive_path, "."):
            extracted_folder = next(d for d in os.listdir('.') if d.startswith('sox-'))
            for item in os.listdir(extracted_folder):
                shutil.move(os.path.join(extracted_folder, item), sox_dir)
            shutil.rmtree(extracted_folder)
            print("SoX configurato con successo.")
            return True
        print("ERRORE: Download o estrazione di SoX fallita.")
        return False
    else:
        print("Su Linux/macOS, installa SoX con il gestore di pacchetti (es. `sudo apt install sox` o `brew install sox`).")
        return True # Non bloccare l'installazione