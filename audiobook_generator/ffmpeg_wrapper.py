# -*- coding: utf-8 -*-
import os
import glob
import re
import subprocess
import time
import traceback
from . import config # Import from the current package

def merge_audio_files_ffmpeg(chapter_chunk_dir, final_output_path, ffmpeg_executable_path):
    """
    Merges audio chunk files using FFmpeg with fallback to alternative methods.
    Assumes chunks are in the format specified by config.DEFAULT_AUDIO_FORMAT.
    Converts the final output to MP3 VBR (~192 kbps, qscale:a 2).
    """
    print(f"  Merging audio chunks from: {os.path.basename(chapter_chunk_dir)}")
    print(f"  Saving merged MP3 to: {os.path.basename(final_output_path)}")
    if not os.path.isdir(chapter_chunk_dir):
        print(f"    ERROR: Chunk directory not found: {chapter_chunk_dir}")
        return False

    # Use the default audio format from config
    chunk_extension = config.DEFAULT_AUDIO_FORMAT.lower()

    try:
        # Find chunks with the expected extension
        chunk_files_full_path = glob.glob(os.path.join(chapter_chunk_dir, f"chunk_*.{chunk_extension}"))

        if not chunk_files_full_path:
            print(f"    WARNING: No audio chunks (*.{chunk_extension}) found starting with 'chunk_' in {os.path.basename(chapter_chunk_dir)}. Cannot merge.")
            return False # Or True? If no chunks to merge, it's not a merge error

        # Sort chunks numerically
        def get_chunk_num(filepath):
            match = re.search(r'chunk_(\d+)\.', os.path.basename(filepath))
            return int(match.group(1)) if match else float('inf')

        sorted_chunk_files_full = sorted(chunk_files_full_path, key=get_chunk_num)
        sorted_chunk_files_full = [f for f in sorted_chunk_files_full if get_chunk_num(f) != float('inf')]
        sorted_chunk_files = [os.path.basename(f) for f in sorted_chunk_files_full] # Only basenames for mylist.txt

        if not sorted_chunk_files:
            print(f"    ERROR: Could not extract valid numbers from chunk filenames for sorting.")
            return False

    except Exception as e:
        print(f"    ERROR: Could not list/sort chunk files in {chapter_chunk_dir}: {e}")
        traceback.print_exc()
        return False

    print(f"    Found {len(sorted_chunk_files)} chunk files to merge.")
    
    # Prima prova con FFmpeg
    ffmpeg_success = _try_ffmpeg_merge(chapter_chunk_dir, final_output_path, ffmpeg_executable_path, sorted_chunk_files)
    if ffmpeg_success:
        return True
    
    # Se FFmpeg fallisce, prova con fallback
    print(f"    ⚠️ FFmpeg non disponibile o fallito. Tentativo con metodo alternativo...")
    print(f"    ⚠️ La generazione sarà più lenta e la qualità potrebbe essere inferiore.")
    
    # Prova con pydub (se disponibile)
    pydub_success = _try_pydub_merge(chapter_chunk_dir, final_output_path, sorted_chunk_files_full)
    if pydub_success:
        return True
    
    # Prova con scipy (fallback più basilare)
    scipy_success = _try_scipy_merge(chapter_chunk_dir, final_output_path, sorted_chunk_files_full)
    if scipy_success:
        return True
    
    # Tutti i metodi hanno fallito
    print(f"    ❌ Tutti i metodi di merge hanno fallito.")
    return False

def _try_ffmpeg_merge(chapter_chunk_dir, final_output_path, ffmpeg_executable_path, sorted_chunk_files):
    """Tenta il merge con FFmpeg."""
    list_filename = os.path.join(chapter_chunk_dir, "mylist.txt")
    
    try:
        # Use utf-8 encoding for the list file
        with open(list_filename, 'w', encoding='utf-8') as f:
            for chunk_file in sorted_chunk_files:
                # Escape single quotes for ffmpeg concat demuxer
                safe_chunk_file = chunk_file.replace("'", "'\\''")
                f.write(f"file '{safe_chunk_file}'\n")
    except IOError as e:
        print(f"   ERROR: Could not write temporary file list '{list_filename}': {e}")
        return False

    # Ensure the final output directory exists
    os.makedirs(os.path.dirname(final_output_path), exist_ok=True)

    # FFmpeg command
    ffmpeg_command = [
        ffmpeg_executable_path,
        '-y',                   # Overwrite output without asking
        '-f', 'concat',         # Use concat demuxer
        '-safe', '0',           # Allow simple filenames in list file
        '-i', list_filename,    # Input list file
        '-codec:a', 'libmp3lame',# MP3 codec
        '-qscale:a', '2',       # VBR quality (approx 192kbps)
        final_output_path       # Output file
    ]

    try:
        print(f"    Running FFmpeg command...")
        start_merge = time.time()
        result = subprocess.run(
            ffmpeg_command,
            cwd=chapter_chunk_dir, # Run in chunk dir
            check=True,            # Error if FFmpeg fails
            capture_output=True,   # Capture output
            text=True,             # Output as text
            encoding='utf-8',      # Use UTF-8
            errors='ignore',       # Ignore ffmpeg output decoding errors
            timeout=600            # 10 minute timeout
        )
        merge_time = time.time() - start_merge

        if os.path.exists(final_output_path) and os.path.getsize(final_output_path) > 512: # Basic check
            print(f"    ✅ FFmpeg finished successfully (took {merge_time:.2f}s). Merged MP3: {os.path.basename(final_output_path)}")
            return True
        else:
            print(f"    ERROR: Merged file missing or empty after FFmpeg execution, even though FFmpeg reported success.")
            if result.stderr: print(f"    FFmpeg stderr:\n{result.stderr}")
            return False

    except subprocess.TimeoutExpired:
        print(f"    ERROR: FFmpeg merge timed out after 10 minutes.")
        return False
    except subprocess.CalledProcessError as e:
        stderr_lines = e.stderr.strip().splitlines() if e.stderr else ["No stderr output."]
        print(f"    ERROR running FFmpeg (code {e.returncode}):")
        # Print last stderr lines
        for line in stderr_lines[-15:]:
             print(f"      {line}")
        return False
    except FileNotFoundError:
        # This error now means the specified ffmpeg executable path was not found
        print(f"    ERROR: FFmpeg executable not found at: '{ffmpeg_executable_path}'")
        return False
    except Exception as e:
        print(f"    Unexpected error during FFmpeg merge: {e}")
        traceback.print_exc()
        return False
    finally:
        # Clean up mylist.txt
        if os.path.exists(list_filename):
            try:
                os.remove(list_filename)
            except OSError:
                pass # Ignore if it cannot be removed

def _try_pydub_merge(chapter_chunk_dir, final_output_path, sorted_chunk_files_full):
    """Tenta il merge con pydub (se disponibile)."""
    try:
        from pydub import AudioSegment
        print(f"    Tentativo merge con pydub...")
        
        # Carica il primo file per inizializzare
        combined = AudioSegment.from_file(sorted_chunk_files_full[0])
        
        # Aggiungi gli altri file
        for chunk_file in sorted_chunk_files_full[1:]:
            audio = AudioSegment.from_file(chunk_file)
            combined = combined + audio
        
        # Esporta come MP3
        combined.export(final_output_path, format="mp3", bitrate="192k")
        
        if os.path.exists(final_output_path) and os.path.getsize(final_output_path) > 512:
            print(f"    ✅ Merge con pydub completato con successo.")
            return True
        else:
            print(f"    ERROR: Merge con pydub fallito (file non creato).")
            return False
            
    except ImportError:
        print(f"    INFO: pydub non disponibile. Passo al metodo successivo.")
        return False
    except Exception as e:
        print(f"    ERROR durante merge con pydub: {e}")
        traceback.print_exc()
        return False

def _try_scipy_merge(chapter_chunk_dir, final_output_path, sorted_chunk_files_full):
    """Tenta il merge con scipy.io.wavfile (solo per file WAV)."""
    try:
        import scipy.io.wavfile as wavfile
        import numpy as np
        import shutil
        print(f"    Tentativo merge con scipy (solo WAV)...")
        
        # Verifica che tutti i file siano WAV
        if not all(f.lower().endswith('.wav') for f in sorted_chunk_files_full):
            print(f"    ERROR: scipy merge supporta solo file WAV.")
            return False
        
        # Leggi il primo file per ottenere sample rate
        sample_rate, first_data = wavfile.read(sorted_chunk_files_full[0])
        all_data = [first_data]
        
        # Leggi gli altri file
        for chunk_file in sorted_chunk_files_full[1:]:
            sr, data = wavfile.read(chunk_file)
            if sr != sample_rate:
                print(f"    WARNING: Sample rate mismatch ({sr} vs {sample_rate}). Tentativo di risoluzione...")
                # Prova a ricampionare (semplice)
                if sr > sample_rate:
                    data = data[::sr//sample_rate]
                else:
                    # Ripetizione semplice (non ideale)
                    data = np.repeat(data, sample_rate//sr)
            
            all_data.append(data)
        
        # Concatena tutti i dati
        combined_data = np.concatenate(all_data)
        
        # Salva come WAV (scipy non supporta MP3 direttamente)
        wav_output = final_output_path.replace('.mp3', '.wav')
        wavfile.write(wav_output, sample_rate, combined_data)
        
        # Converti in MP3 con ffmpeg se disponibile, altrimenti lascia come WAV
        if os.path.exists(wav_output) and os.path.getsize(wav_output) > 512:
            print(f"    ✅ Merge con scipy completato. File salvato come WAV: {os.path.basename(wav_output)}")
            
            # Prova a convertire in MP3 se possibile
            try:
                ffmpeg_path = shutil.which("ffmpeg")
                if ffmpeg_path:
                    cmd = [ffmpeg_path, '-y', '-i', wav_output, '-codec:a', 'libmp3lame', '-qscale:a', '2', final_output_path]
                    subprocess.run(cmd, check=True, capture_output=True)
                    os.remove(wav_output)
                    print(f"    Convertito in MP3.")
                else:
                    # Rinomina il file WAV come output finale
                    os.rename(wav_output, final_output_path.replace('.mp3', '_fallback.wav'))
                    print(f"    ⚠️ File salvato come WAV (fallback).")
            except:
                # Rinomina comunque
                os.rename(wav_output, final_output_path.replace('.mp3', '_fallback.wav'))
            
            return True
        else:
            print(f"    ERROR: Merge con scipy fallito (file non creato).")
            return False
            
    except ImportError:
        print(f"    INFO: scipy non disponibile. Passo al metodo successivo.")
        return False
    except Exception as e:
        print(f"    ERROR durante merge con scipy: {e}")
        traceback.print_exc()
        return False

def concatenate_audio_files(file_list, output_path, ffmpeg_executable_path):
    """Concatena una lista di file audio in un unico file MP3."""
    print(f"Concatenando {len(file_list)} file audio in {os.path.basename(output_path)}")
    
    # Crea un file di lista temporaneo
    list_filename = os.path.join(os.path.dirname(output_path), "concat_list.txt")
    with open(list_filename, 'w', encoding='utf-8') as f:
        for audio_file in file_list:
            safe_path = audio_file.replace("'", "'\\''")
            f.write(f"file '{safe_path}'\n")

    ffmpeg_command = [
        ffmpeg_executable_path, '-y', '-f', 'concat', '-safe', '0',
        '-i', list_filename, '-codec:a', 'libmp3lame', '-qscale:a', '2', output_path
    ]

    try:
        subprocess.run(ffmpeg_command, check=True, capture_output=True, text=True, encoding='utf-8', errors='ignore')
        os.remove(list_filename)
        return True
    except Exception as e:
        print(f"ERRORE durante la concatenazione con FFmpeg: {e}")
        if hasattr(e, 'stderr'):
            print(f"FFmpeg stderr: {e.stderr}")
        return False
