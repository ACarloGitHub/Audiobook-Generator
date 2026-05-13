# Copyright 2025 Carlo Piras
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# -*- coding: utf-8 -*-
import os
import glob
import logging
import re
import subprocess
import time
from . import config

logger = logging.getLogger(__name__)

def merge_audio_files_ffmpeg(chapter_chunk_dir, final_output_path, ffmpeg_executable_path):
    """
    Merges audio chunk files using FFmpeg with fallback to alternative methods.
    Assumes chunks are in the format specified by config.DEFAULT_AUDIO_FORMAT.
    Converts the final output to MP3 VBR (~192 kbps, qscale:a 2).
    """
    logger.info("Merging audio chunks from: %s", os.path.basename(chapter_chunk_dir))
    logger.info("Saving merged MP3 to: %s", os.path.basename(final_output_path))
    if not os.path.isdir(chapter_chunk_dir):
        logger.error("Chunk directory not found: %s", chapter_chunk_dir)
        return False

    # Use the default audio format from config
    chunk_extension = config.DEFAULT_AUDIO_FORMAT.lower()

    try:
        # Find chunks with the expected extension
        chunk_files_full_path = glob.glob(os.path.join(chapter_chunk_dir, f"chunk_*.{chunk_extension}"))

        if not chunk_files_full_path:
            logger.warning("No audio chunks (*.%s) found starting with 'chunk_' in %s. Cannot merge.", chunk_extension, os.path.basename(chapter_chunk_dir))
            return False

        # Sort chunks numerically
        def get_chunk_num(filepath):
            match = re.search(r'chunk_(\d+)\.', os.path.basename(filepath))
            return int(match.group(1)) if match else float('inf')

        sorted_chunk_files_full = sorted(chunk_files_full_path, key=get_chunk_num)
        sorted_chunk_files_full = [f for f in sorted_chunk_files_full if get_chunk_num(f) != float('inf')]
        sorted_chunk_files = [os.path.basename(f) for f in sorted_chunk_files_full] # Only basenames for mylist.txt

        if not sorted_chunk_files:
            logger.error("Could not extract valid numbers from chunk filenames for sorting.")
            return False

    except Exception as e:
        logger.error("Could not list/sort chunk files in %s: %s", chapter_chunk_dir, e, exc_info=True)
        return False

    logger.info("Found %d chunk files to merge.", len(sorted_chunk_files))
    
    # First try with FFmpeg
    ffmpeg_success = _try_ffmpeg_merge(chapter_chunk_dir, final_output_path, ffmpeg_executable_path, sorted_chunk_files)
    if ffmpeg_success:
        return True
    
    # If FFmpeg fails, try fallback methods
    logger.warning("FFmpeg not available or failed. Trying alternative method...")
    logger.warning("Generation will be slower and quality may be lower.")
    
    # Try with pydub (if available)
    pydub_success = _try_pydub_merge(chapter_chunk_dir, final_output_path, sorted_chunk_files_full)
    if pydub_success:
        return True
    
    # Try with scipy (more basic fallback)
    scipy_success = _try_scipy_merge(chapter_chunk_dir, final_output_path, sorted_chunk_files_full)
    if scipy_success:
        return True
    
    # All methods have failed
    logger.error("All merge methods failed.")
    return False

def _try_ffmpeg_merge(chapter_chunk_dir, final_output_path, ffmpeg_executable_path, sorted_chunk_files):
    """Try merging with FFmpeg."""
    list_filename = os.path.join(chapter_chunk_dir, "mylist.txt")
    
    try:
        # Use utf-8 encoding for the list file
        with open(list_filename, 'w', encoding='utf-8') as f:
            for chunk_file in sorted_chunk_files:
                # Escape single quotes for ffmpeg concat demuxer
                safe_chunk_file = chunk_file.replace("'", "'\\''")
                f.write(f"file '{safe_chunk_file}'\n")
    except IOError as e:
        logger.error("Could not write temporary file list '%s': %s", list_filename, e)
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
        logger.info("Running FFmpeg command...")
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

        if os.path.exists(final_output_path) and os.path.getsize(final_output_path) > 512:
            logger.info("FFmpeg finished successfully (took %.2fs). Merged MP3: %s", merge_time, os.path.basename(final_output_path))
            return True
        else:
            logger.error("Merged file missing or empty after FFmpeg execution, even though FFmpeg reported success.")
            if result.stderr:
                logger.debug("FFmpeg stderr:\n%s", result.stderr)
            return False

    except subprocess.TimeoutExpired:
        logger.error("FFmpeg merge timed out after 10 minutes.")
        return False
    except subprocess.CalledProcessError as e:
        stderr_lines = e.stderr.strip().splitlines() if e.stderr else ["No stderr output."]
        logger.error("FFmpeg failed (code %d):", e.returncode)
        for line in stderr_lines[-15:]:
            logger.error("  %s", line)
        return False
    except FileNotFoundError:
        logger.error("FFmpeg executable not found at: '%s'", ffmpeg_executable_path)
        return False
    except Exception as e:
        logger.error("Unexpected error during FFmpeg merge: %s", e, exc_info=True)
        return False
    finally:
        if os.path.exists(list_filename):
            try:
                os.remove(list_filename)
            except OSError as e:
                logger.debug("Could not remove temporary file %s: %s", list_filename, e)

def _try_pydub_merge(chapter_chunk_dir, final_output_path, sorted_chunk_files_full):
    """Try to merge with pydub (if available)."""
    try:
        from pydub import AudioSegment
        logger.info("Attempting merge with pydub...")
        
        # Load the first file to initialize
        combined = AudioSegment.from_file(sorted_chunk_files_full[0])
        
        # Append the other files
        for chunk_file in sorted_chunk_files_full[1:]:
            audio = AudioSegment.from_file(chunk_file)
            combined = combined + audio
        
        # Export as MP3
        combined.export(final_output_path, format="mp3", bitrate="192k")
        
        if os.path.exists(final_output_path) and os.path.getsize(final_output_path) > 512:
            logger.info("Merge with pydub completed successfully.")
            return True
        else:
            logger.error("Merge with pydub failed (file not created).")
            return False
            
    except ImportError:
        logger.info("pydub not available. Trying next method.")
        return False
    except Exception as e:
        logger.error("Merge with pydub failed: %s", e, exc_info=True)
        return False

def _try_scipy_merge(chapter_chunk_dir, final_output_path, sorted_chunk_files_full):
    """Try to merge with scipy.io.wavfile (WAV files only)."""
    try:
        import scipy.io.wavfile as wavfile
        import numpy as np
        import shutil
        logger.info("Attempting merge with scipy (WAV only)...")
        
        # Verify all files are WAV
        if not all(f.lower().endswith('.wav') for f in sorted_chunk_files_full):
            logger.error("scipy merge only supports WAV files.")
            return False
        
        # Read the first file to get sample rate
        sample_rate, first_data = wavfile.read(sorted_chunk_files_full[0])
        all_data = [first_data]
        
        # Read the other files
        for chunk_file in sorted_chunk_files_full[1:]:
            sr, data = wavfile.read(chunk_file)
            if sr != sample_rate:
                logger.warning("Sample rate mismatch (%d vs %d). Attempting resampling...", sr, sample_rate)
                # Try to resample (simple)
                if sr > sample_rate:
                    data = data[::sr//sample_rate]
                else:
                    # Simple repetition (not ideal)
                    data = np.repeat(data, sample_rate//sr)
            
            all_data.append(data)
        
        # Concatenate all data
        combined_data = np.concatenate(all_data)
        
        # Save as WAV (scipy does not support MP3 directly)
        wav_output = final_output_path.replace('.mp3', '.wav')
        wavfile.write(wav_output, sample_rate, combined_data)
        
        # Convert to MP3 with ffmpeg if available, otherwise leave as WAV
        if os.path.exists(wav_output) and os.path.getsize(wav_output) > 512:
            logger.info("Merge with scipy completed. File saved as WAV: %s", os.path.basename(wav_output))
            
            # Try to convert to MP3 if possible
            try:
                ffmpeg_path = shutil.which("ffmpeg")
                if ffmpeg_path:
                    cmd = [ffmpeg_path, '-y', '-i', wav_output, '-codec:a', 'libmp3lame', '-qscale:a', '2', final_output_path]
                    subprocess.run(cmd, check=True, capture_output=True)
                    os.remove(wav_output)
                    logger.info("Converted to MP3.")
                else:
                    os.rename(wav_output, final_output_path.replace('.mp3', '_fallback.wav'))
                    logger.warning("File saved as WAV (fallback).")
            except Exception:
                os.rename(wav_output, final_output_path.replace('.mp3', '_fallback.wav'))
            
            return True
        else:
            logger.error("Merge with scipy failed (file not created).")
            return False
            
    except ImportError:
        logger.info("scipy not available. Trying next method.")
        return False
    except Exception as e:
        logger.error("Merge with scipy failed: %s", e, exc_info=True)
        return False

def concatenate_audio_files(file_list, output_path, ffmpeg_executable_path):
    """Concatenates a list of audio files into a single MP3 file."""
    logger.info("Concatenating %d audio files into %s", len(file_list), os.path.basename(output_path))
    
    list_filename = os.path.join(os.path.dirname(output_path), "concat_list.txt")
    try:
        with open(list_filename, 'w', encoding='utf-8') as f:
            for audio_file in file_list:
                safe_path = audio_file.replace("'", "'\\''")
                f.write(f"file '{safe_path}'\n")
    except OSError as e:
        logger.error("Could not write temporary file list '%s': %s", list_filename, e)
        return False

    ffmpeg_command = [
        ffmpeg_executable_path, '-y', '-f', 'concat', '-safe', '0',
        '-i', list_filename, '-codec:a', 'libmp3lame', '-qscale:a', '2', output_path
    ]

    try:
        subprocess.run(ffmpeg_command, check=True, capture_output=True, text=True, encoding='utf-8', errors='ignore')
        try:
            os.remove(list_filename)
        except OSError:
            pass
        return True
    except Exception as e:
        logger.error("Error during FFmpeg concatenation: %s", e)
        if hasattr(e, 'stderr'):
            logger.debug("FFmpeg stderr: %s", e.stderr)
        return False
