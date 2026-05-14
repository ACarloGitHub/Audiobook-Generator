# Copyright (c) 2026 Patata Audiobook Generator
#
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in all
# copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
# SOFTWARE.

import sys
import json
import os
import logging
import contextlib
from pathlib import Path

# Setup of a dedicated file logger WITHOUT using stdout/stderr
log_dir = os.path.join('audiobook_generator', 'Logs')
os.makedirs(log_dir, exist_ok=True)
log_file = os.path.join(log_dir, 'qwen3_subprocess.log')
logging.basicConfig(
    level=logging.INFO, 
    filename=log_file, 
    filemode='a', 
    format='%(asctime)s - %(message)s',
    force=True
)

class StdoutToStderr:
    """Redirect stdout to stderr to keep the JSON channel on stdout clean."""
    def write(self, text):
        sys.stderr.write(text)
        sys.stderr.flush()
    def flush(self):
        sys.stderr.flush()

def main():
    # Save original stdout for JSON responses
    original_stdout = sys.stdout
    
    # Redirect all stdout to stderr to prevent library output from corrupting JSON responses
    sys.stdout = StdoutToStderr()
    
    try:
        # Import torch and Qwen3TTSModel (stdout already redirected to stderr)
        import torch
        
        # Force offline mode for Hugging Face
        os.environ["HF_HUB_OFFLINE"] = "1"
        os.environ["TRANSFORMERS_OFFLINE"] = "1"
        
        from qwen_tts import Qwen3TTSModel
        
        # Add sox/bin to PATH to avoid soundfile errors
        sox_bin = os.path.join(os.getcwd(), "sox", "bin")
        if os.path.exists(sox_bin):
            os.environ["PATH"] = sox_bin + os.pathsep + os.environ["PATH"]
        
        # Import soundfile AFTER modifying the PATH
        import soundfile as sf
        
        payload = json.load(sys.stdin)
        logging.info(f"Received job: {payload}")
        
        text = payload['text']
        text = ' '.join(text.replace('\r\n', '\n').replace('\r', '\n').split())
        output_path = payload['output_path']
        mode = payload.get('mode')
        if mode is None:
            logging.warning("Mode not provided in payload, defaulting to 'clone'")
            mode = "clone"
        params = payload.get('params', {})
        model_size = payload.get('model_size', '0.6B')  # Default a 0.6B
        model_type = payload.get('model_type', 'base')  # 'base', 'custom_voice', 'voice_design'

        # Costruisci percorsi relativi alla directory di questo script
        script_dir = Path(__file__).parent.absolute()
        
        # Determina directory modello in base a size e type
        # USE OFFICIAL NAMES: Qwen3-TTS-12Hz-0.6B-Base, Qwen3-TTS-12Hz-1.7B-CustomVoice, etc.
        # Map model_type to folder type name
        if model_type == 'base':
            type_folder = "Base"
        elif model_type == 'custom_voice':
            type_folder = "CustomVoice"
        elif model_type == 'voice_design':
            type_folder = "VoiceDesign"
        else:
            type_folder = model_type
        
        # Build official folder name: Qwen3-TTS-12Hz-{size}-{TypeFolder}
        # Examples: Qwen3-TTS-12Hz-0.6B-Base, Qwen3-TTS-12Hz-1.7B-VoiceDesign
        model_dir_name = f"Qwen3-TTS-12Hz-{model_size}-{type_folder}"
        
        model_dir = (script_dir / f"../../tts_models/qwen3tts/{model_dir_name}").resolve()
        tokenizer_dir = (script_dir / "../../tts_models/qwen3tts/tokenizer").resolve()
        
        # Convert to POSIX path (forward slash) to avoid Windows backslash issues
        model_dir = model_dir.as_posix()
        tokenizer_dir = tokenizer_dir.as_posix()
        
        logging.info(f"Model dir: {model_dir}")
        logging.info(f"Tokenizer dir: {tokenizer_dir}")

        # Verify that the model directory exists
        if not os.path.exists(model_dir):
            raise FileNotFoundError(f"Model directory not found: {model_dir}")
        
        # Suppress stdout during model loading to avoid corrupting JSON channel
        # stderr is NOT suppressed: legitimate errors and warnings remain visible
        with open(os.devnull, 'w') as devnull:
            with contextlib.redirect_stdout(devnull):
                model = Qwen3TTSModel.from_pretrained(
                    model_dir,
                    device_map="cuda:0" if torch.cuda.is_available() else "cpu",
                    dtype=torch.bfloat16 if torch.cuda.is_available() and torch.cuda.is_bf16_supported() else torch.float16,
                    attn_implementation="eager",
                    local_files_only=True,
                    trust_remote_code=True
                )

        # Common parameters for all modes (based on Qwen3-TTS documentation)
        # NOTE: Default values must match exactly the UI (configuration_tab.py)
        common_kwargs = {
            "speed": params.get("speed", 1.0),
            "pitch": params.get("pitch", 0),
            "volume": params.get("volume", 0),
            "temperature": params.get("temperature", 0.7),
            "top_p": params.get("top_p", 0.8),
            "top_k": params.get("top_k", 20),
            "repetition_penalty": params.get("repetition_penalty", 1.1),
            "seed": params.get("seed"),  # can be None
            # Additional parameters from HF Transformers documentation
            "max_new_tokens": 2048,
            "do_sample": True,
        }
        # Remove None parameters to avoid passing invalid values
        common_kwargs = {k: v for k, v in common_kwargs.items() if v is not None}

        # Map UI modes to model methods based on model type
        # According to Qwen3-TTS documentation:
        # - Base model (model_type='base'): supports only generate_voice_clone (mode='clone')
        # - CustomVoice model (model_type='custom_voice'): supports only generate_custom_voice (mode='custom')
        # - VoiceDesign model (model_type='voice_design'): supports only generate_voice_design (mode='design')
        
        if model_type == 'custom_voice':
            # The CustomVoice model only supports generate_custom_voice
            # Required parameters: text, language, speaker
            # Optional parameters: instruct
            # Handle both 'speaker' (new) and 'voice' (old) for backward compatibility
            speaker = params.get('speaker', params.get('voice', 'Serena'))
            language = params.get('language', 'Italian')
            instruct = params.get('instruct', '')
            
            logging.info(f"CustomVoice: text={text[:50]}..., speaker={speaker}, language={language}, instruct={instruct[:50] if instruct else ''}")
            
            wavs, sr = model.generate_custom_voice(
                text=text,
                speaker=speaker,
                language=language,
                instruct=instruct,
                **common_kwargs
            )
        elif model_type == 'voice_design':
            # The VoiceDesign model only supports generate_voice_design
            # Required parameters: text, language, instruct
            language = params.get('language', 'Italian')
            instruct = params.get('instruct', '')
            
            # Handle language: could be a number (index) from Gradio
            # Convert to string and map to supported language
            if language is not None:
                language = str(language)
                # Map numeric UI indices to supported languages (lowercase for Qwen3-TTS)
                language_map = {
                    '0': 'auto', '1': 'italian', '2': 'english', '3': 'french', 
                    '4': 'german', '5': 'portuguese', '6': 'spanish', '7': 'japanese',
                    '8': 'korean', '9': 'russian', '10': 'chinese'
                }
                if language in language_map:
                    language = language_map[language]
                # Ensure it is lowercase and matches a supported language
                supported = ['auto', 'chinese', 'english', 'french', 'german', 'italian', 
                            'japanese', 'korean', 'portuguese', 'russian', 'spanish']
                if language.lower() not in supported:
                    logging.warning(f"VoiceDesign: language '{language}' not supported, defaulting to 'italian'")
                    language = 'italian'
                else:
                    language = language.lower()
            else:
                language = 'italian'
            
            if not instruct:
                logging.warning("VoiceDesign: instruct is empty, quality may be compromised")
            
            logging.info(f"VoiceDesign: text={text[:50]}..., language={language}, instruct={instruct[:50] if instruct else ''}")
            
            wavs, sr = model.generate_voice_design(
                text=text,
                instruct=instruct,
                language=language,
                **common_kwargs
            )
        else:  # base
            # The Base model only supports generate_voice_clone
            # The UI should pass mode='clone' for this model
            if mode not in ['clone', 'custom', 'design']:
                logging.warning(f"Mode '{mode}' not valid for Base model, forcing to 'clone'")
                mode = 'clone'
            
            if mode == 'custom':
                raise ValueError("'custom' mode not supported for Base model. Use CustomVoice model.")
            elif mode == 'design':
                raise ValueError("'design' mode not supported for Base model. Use VoiceDesign model.")
            
            # Clone mode (only one supported for Base model)
            # Required parameters: text, language, ref_audio
            # ref_text is only required when x_vector_only_mode=False
            language = params.get('language', 'Italian')
            ref_audio = params.get('ref_audio')
            x_vector_only_mode = params.get('x_vector_only_mode', False)
            
            if not ref_audio:
                raise ValueError("For Voice Clone mode, a reference audio file (ref_audio) must be provided.")
            
            logging.info(f"Voice Clone: text={text[:50]}..., language={language}, ref_audio={ref_audio}, x_vector_only_mode={x_vector_only_mode}")
            
            if x_vector_only_mode:
                # Fast mode: does not require ref_text
                wavs, sr = model.generate_voice_clone(
                    text=text,
                    language=language,
                    ref_audio=ref_audio,
                    x_vector_only_mode=True,
                    **common_kwargs
                )
            else:
                # Full quality mode: requires ref_text
                ref_text = params.get('ref_text', '')
                if not ref_text:
                    raise ValueError("For maximum quality Voice Clone mode, the text transcription (ref_text) is required.")
                
                wavs, sr = model.generate_voice_clone(
                    text=text,
                    language=language,
                    ref_audio=ref_audio,
                    ref_text=ref_text,
                    x_vector_only_mode=False,
                    **common_kwargs
                )
        
        # Suppress soundfile stderr output (SoX error) during write
        with open(os.devnull, 'w') as devnull:
            with contextlib.redirect_stderr(devnull):
                import numpy as np
                import subprocess
                import tempfile
                
                # Convert PyTorch tensor to numpy array
                if torch.is_tensor(wavs):
                    wavs_np = wavs.cpu().numpy()
                else:
                    wavs_np = np.array(wavs)
                
                # DEBUG: Log dimensions to diagnose issues
                logging.info(f"Original audio shape: {wavs_np.shape}, dtype: {wavs_np.dtype}")
                
                # Remove single batch dimensions (e.g.: [1, 24000] -> [24000])
                # or [1, 1, 24000] -> [24000]
                while wavs_np.ndim > 1 and wavs_np.shape[0] == 1:
                    wavs_np = wavs_np.squeeze(0)
                
                # If 2D with shape [channels, samples], transpose to [samples, channels]
                # but if already 1D, leave as is
                if wavs_np.ndim == 2 and wavs_np.shape[0] < wavs_np.shape[1]:
                    # Probably [channels, samples], let's transpose
                    wavs_np = wavs_np.T
                
                # Ensure it is 1D if mono, or correctly 2D if stereo
                if wavs_np.ndim > 2:
                    wavs_np = wavs_np.reshape(-1)
                
                # Robust normalization
                max_val = np.max(np.abs(wavs_np))
                if max_val > 1.0:
                    logging.warning(f"Audio normalization: values out of range (max={max_val:.3f})")
                    wavs_np = wavs_np / max_val
                elif max_val < 0.001:
                    logging.warning(f"Audio nearly silent (max={max_val:.3f})")
                
                # Convert to float32 for maximum compatibility
                wavs_np = wavs_np.astype(np.float32)
                
                # Ensure sample rate is an integer
                sr_int = int(sr) if sr is not None else 24000
                
                # DEBUG: Final log
                logging.info(f"Audio ready for saving: shape={wavs_np.shape}, dtype={wavs_np.dtype}, sr={sr_int}")
                
                # METHOD 1: FFMPEG (first choice - already present in the project)
                raw_path = None
                try:
                    # Create temporary raw PCM file
                    with tempfile.NamedTemporaryFile(suffix='.raw', delete=False) as tmp_raw:
                        raw_path = tmp_raw.name
                        wavs_np.tofile(raw_path)
                    
                    # Local FFmpeg path
                    ffmpeg_path = os.path.join(os.getcwd(), "ffmpeg", "bin", "ffmpeg.exe")
                    if not os.path.exists(ffmpeg_path):
                        # Fallback to ffmpeg in PATH
                        ffmpeg_path = "ffmpeg"
                    
                    # Determine channels
                    channels = 1 if wavs_np.ndim == 1 else wavs_np.shape[1]
                    
                    # FFmpeg command to convert raw PCM float32 to WAV PCM s16le
                    cmd = [
                        ffmpeg_path,
                        '-f', 'f32le',          # input format: float32 little-endian
                        '-ar', str(sr_int),     # sample rate
                        '-ac', str(channels),   # channels
                        '-i', raw_path,         # input file
                        '-c:a', 'pcm_s16le',    # audio codec: PCM signed 16-bit little-endian
                        '-y',                   # overwrite output
                        output_path
                    ]
                    
                    logging.info(f"Running FFmpeg: {' '.join(cmd)}")
                    result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
                    
                    if result.returncode == 0:
                        logging.info(f"WAV file saved with FFmpeg: {output_path}, size: {os.path.getsize(output_path)} bytes")
                    else:
                        logging.error(f"FFmpeg failed: {result.stderr}")
                        raise RuntimeError(f"FFmpeg failed: {result.stderr}")
                        
                except Exception as e_ffmpeg:
                    logging.warning(f"FFmpeg failed: {e_ffmpeg}, trying scipy...")
                    
                    # METHOD 2: SCIPY (second choice)
                    try:
                        from scipy.io import wavfile
                        # Scipy requires int16 for PCM, convert from float32 [-1,1] to int16
                        wavs_int16 = (wavs_np * 32767).astype(np.int16)
                        wavfile.write(output_path, sr_int, wavs_int16)
                        logging.info(f"WAV file saved with scipy: {output_path}")
                    except ImportError:
                        logging.warning("Scipy not available, trying soundfile...")
                        # METHOD 3: SOUNDFILE (third choice)
                        try:
                            # If mono 1D, soundfile is happy
                            # If stereo, it must be shape (samples, channels)
                            sf.write(output_path, wavs_np, sr_int, subtype='PCM_16', format='WAV')
                            logging.info(f"WAV file saved with soundfile: {output_path}")
                        except Exception as e_sf:
                            # METHOD 4: TORCHAUDIO (last resort)
                            try:
                                import torchaudio
                                wavs_tensor = torch.from_numpy(wavs_np).unsqueeze(0) if wavs_np.ndim == 1 else torch.from_numpy(wavs_np)
                                torchaudio.save(output_path, wavs_tensor, sr_int)
                                logging.info(f"File saved with torchaudio: {output_path}")
                            except Exception as e_ta:
                                raise RuntimeError(f"All save methods failed: FFmpeg({e_ffmpeg}), scipy(ImportError), soundfile({e_sf}), torchaudio({e_ta})")
                    except Exception as e_scipy:
                        logging.warning(f"Scipy failed: {e_scipy}, trying soundfile...")
                        try:
                            sf.write(output_path, wavs_np, sr_int, subtype='PCM_16', format='WAV')
                            logging.info(f"WAV file saved with soundfile: {output_path}")
                        except Exception as e_sf:
                            raise RuntimeError(f"Save failed after FFmpeg and scipy: soundfile({e_sf})")
                finally:
                    # Always clean up the temporary raw file
                    if raw_path and os.path.exists(raw_path):
                        try:
                            os.unlink(raw_path)
                        except OSError:
                            pass
        
        # Restore stdout before sending JSON response
        sys.stdout = original_stdout
        send_response({"status": "ok", "file": output_path})

    except Exception as e:
        logging.error(f"Error in Qwen3-TTS subprocess: {e}", exc_info=True)
        sys.stdout = original_stdout
        send_response({"status": "error", "message": str(e)})
    finally:
        sys.stdout = original_stdout

def send_response(data):
    """Send a JSON response to stdout."""
    sys.stdout.write(json.dumps(data) + '\n')
    sys.stdout.flush()

if __name__ == "__main__":
    main()