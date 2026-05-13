# audiobook_generator/setup_helpers/dependency_setup.py
import os
import sys
from .download_utils import download_file, extract_archive
from .system_utils import command_exists

def setup_ffmpeg():
    """Sets up FFmpeg by downloading it if not present (Windows only)."""
    print("\n--- Checking FFmpeg ---")
    if command_exists("ffmpeg"):
        print("FFmpeg is already in the system PATH. OK.")
        return True
    
    if sys.platform == "win32":
        ffmpeg_dir = os.path.abspath("ffmpeg/bin")
        if os.path.exists(os.path.join(ffmpeg_dir, "ffmpeg.exe")):
            print("FFmpeg found in local folder. OK.")
            return True
        
        print("FFmpeg not found. Attempting automatic download for Windows...")
        url = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
        archive_path = download_file(url, ".")
        if archive_path and extract_archive(archive_path, "."):
             # Move files to a cleaner structure
            extracted_folder = next(d for d in os.listdir('.') if d.startswith('ffmpeg-'))
            os.rename(extracted_folder, "ffmpeg")
            print("FFmpeg configured successfully.")
            return True
        print("ERROR: FFmpeg download or extraction failed.")
        return False
    else:
        print("On Linux/macOS, install FFmpeg with your package manager (e.g. `sudo apt install ffmpeg` or `brew install ffmpeg`).")
        return True # Don't block installation

def setup_sox():
    """Sets up SoX by downloading it if not present (Windows only)."""
    print("\n--- Checking SoX ---")
    if command_exists("sox"):
        print("SoX is already in the system PATH. OK.")
        return True
        
    if sys.platform == "win32":
        sox_dir = os.path.abspath("sox")
        if os.path.exists(os.path.join(sox_dir, "sox.exe")):
            print("SoX found in local folder. OK.")
            return True
        
        print("SoX not found. Attempting automatic download for Windows...")
        url = "https://sourceforge.net/projects/sox/files/sox/14.4.2/sox-14.4.2-win32.zip/download"
        archive_path = download_file(url, ".")
        if archive_path and extract_archive(archive_path, "."):
            extracted_folder = next(d for d in os.listdir('.') if d.startswith('sox-'))
            for item in os.listdir(extracted_folder):
                shutil.move(os.path.join(extracted_folder, item), sox_dir)
            shutil.rmtree(extracted_folder)
            print("SoX configured successfully.")
            return True
        print("ERROR: SoX download or extraction failed.")
        return False
    else:
        print("On Linux/macOS, install SoX with your package manager (e.g. `sudo apt install sox` or `brew install sox`).")
        return True # Don't block installation