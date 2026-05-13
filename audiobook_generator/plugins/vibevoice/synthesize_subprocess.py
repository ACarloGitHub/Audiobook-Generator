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

# WARNING: This script is standalone and must not import anything from the main project.
import sys
import json
import numpy as np
from scipy.io.wavfile import write as write_wav
import os
import logging

# Setup of a dedicated logger for the subprocess
log_dir = os.path.join('audiobook_generator', 'Logs')
os.makedirs(log_dir, exist_ok=True)
log_file = os.path.join(log_dir, 'vibevoice_subprocess.log')
logging.basicConfig(level=logging.INFO, filename=log_file, filemode='a',
                    format='%(asctime)s - %(levelname)s - %(message)s')

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
        # Import torch only when needed
        import torch
        
        # 1. Read JSON input from stdin
        input_data = json.load(sys.stdin)
        logging.info(f"Received job: {input_data}")
        
        text = input_data['text']
        output_path = input_data['output_path']
        speaker_wav = input_data['speaker_wav']
        model_name = input_data.get('model_name', 'VibeVoice')
        
        # Extract generation parameters with default values
        temperature = input_data.get('temperature', 0.9)
        top_p = input_data.get('top_p', 0.9)
        cfg_scale = input_data.get('cfg_scale', 1.3)
        diffusion_steps = input_data.get('diffusion_steps', 15)
        voice_speed_factor = input_data.get('voice_speed_factor', 1.0)
        use_sampling = input_data.get('use_sampling', True)
        seed = input_data.get('seed')
        
        logging.info(f"Generation parameters: temperature={temperature}, top_p={top_p}, cfg_scale={cfg_scale}, "
                     f"diffusion_steps={diffusion_steps}, voice_speed_factor={voice_speed_factor}, "
                     f"use_sampling={use_sampling}, seed={seed}")

        # --- Local asset loading logic as per technical report ---
        # Local path definitions
        base_project_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', '..'))
        
        # Determine model directory based on name
        if model_name == 'VibeVoice-7B':
            model_subdir = '7B'
        elif model_name == 'VibeVoice-Realtime-0.5B':
            model_subdir = '0.5B'
        else:
            model_subdir = '1.5B'  # fallback for compatibility
        
        # Structure: vibevoice/{1.5B,7B,0.5B}/ for models
        # repo_community/vibevoice for code (streaming + inference)
        vibevoice_model_dir = os.path.join(base_project_dir, 'audiobook_generator', 'tts_models', 'vibevoice', model_subdir)
        
        # Qwen2.5-1.5B tokenizer: separate folder tts_models/vibevoice/tokenizer/
        # (NOT inside the model — the model on HF does not include tokenizer files)
        vibevoice_tokenizer_dir = os.path.join(base_project_dir, 'audiobook_generator', 'tts_models', 'vibevoice', 'tokenizer')
        
        # Source code: repo_community/vibevoice for Realtime-0.5B (streaming), repo/vibevoice for 1.5B/7B
        # VibeVoiceStreamingForConditionalGenerationInference is in repo_community (community repo with streaming classes)
        # VibeVoiceForConditionalGenerationInference (non-streaming) and streaming are both in repo_community
        if model_name == 'VibeVoice-Realtime-0.5B':
            vibevoice_code_base = os.path.join(base_project_dir, 'audiobook_generator', 'tts_models', 'vibevoice', 'repo_community')
        else:
            # 1.5B/7B: use repo_community (contains both modeling_vibevoice.py and modeling_vibevoice_inference.py)
            vibevoice_code_base = os.path.join(base_project_dir, 'audiobook_generator', 'tts_models', 'vibevoice', 'repo_community')
        
        vibevoice_code_dir = os.path.join(vibevoice_code_base, 'vibevoice')
        
        logging.info(f"Loading model: {model_name} -> {model_subdir}")
        
        # Detailed GPU logging
        logging.info(f"Torch version: {torch.__version__}")
        logging.info(f"CUDA available: {torch.cuda.is_available()}")
        if torch.cuda.is_available():
            logging.info(f"CUDA device count: {torch.cuda.device_count()}")
            for i in range(torch.cuda.device_count()):
                logging.info(f"  Device {i}: {torch.cuda.get_device_name(i)}")
                logging.info(f"    Memory allocated: {torch.cuda.memory_allocated(i) / 1024**2:.2f} MB")
                logging.info(f"    Memory reserved: {torch.cuda.memory_reserved(i) / 1024**2:.2f} MB")
        else:
            logging.warning("CUDA not available. The model will be loaded on CPU.")

        # Manipulate sys.path to import VibeVoice source code
        original_sys_path = sys.path.copy()
        try:
            # Add both paths for different import patterns
            if vibevoice_code_base not in sys.path:
                sys.path.insert(0, vibevoice_code_base)
            if vibevoice_code_dir not in sys.path:
                sys.path.insert(0, vibevoice_code_dir)
            
            use_gpu = torch.cuda.is_available()
            device_map = "cuda:0" if use_gpu else "cpu"
            torch_dtype = torch.bfloat16 if use_gpu else torch.float32
            
            if model_name == 'VibeVoice-Realtime-0.5B':
                # =====================================================================
                # STREAMING PATH (Realtime-0.5B)
                # =====================================================================
                import copy
                from modular.modeling_vibevoice_streaming_inference import VibeVoiceStreamingForConditionalGenerationInference
                from processor.vibevoice_streaming_processor import VibeVoiceStreamingProcessor
                
                logging.info("Streaming import completed. Loading streaming processor...")
                
                # Load streaming processor
                processor = VibeVoiceStreamingProcessor.from_pretrained(vibevoice_model_dir)
                logging.info(f"Streaming processor loaded: {processor.__class__.__name__}")
                
                logging.info("Loading streaming model...")
                model = VibeVoiceStreamingForConditionalGenerationInference.from_pretrained(
                    vibevoice_model_dir,
                    device_map=device_map,
                    torch_dtype=torch_dtype,
                )
                model.set_ddpm_inference_steps(num_steps=diffusion_steps if diffusion_steps is not None else 5)
                logging.info(f"Streaming model loaded on: {model.device}")
                
            else:
                # =====================================================================
                # NON-STREAMING PATH (1.5B / 7B)
                # =====================================================================
                from modular.modeling_vibevoice_inference import VibeVoiceForConditionalGenerationInference
                from processor.vibevoice_processor import VibeVoiceProcessor
                
                logging.info("Import completed. Loading processor...")
                
                logging.info("Loading VibeVoiceProcessor from model directory...")
                processor = VibeVoiceProcessor.from_pretrained(
                    vibevoice_model_dir,
                    language_model_pretrained_name=vibevoice_tokenizer_dir,
                    local_files_only=True,
                    trust_remote_code=False
                )
                logging.info(f"Processor loaded: {processor.__class__.__name__}")
                
                tokenizer = processor.tokenizer
                if tokenizer is None:
                    raise RuntimeError("FATAL: Tokenizer is None after loading processor.")
                logging.info(f"Tokenizer wrapper loaded: {tokenizer.__class__.__name__}")
                
                if tokenizer.bos_token_id is None:
                    logging.warning(f"bos_token_id is None, set to eos_token_id: {tokenizer.eos_token_id}")
                    tokenizer.bos_token_id = tokenizer.eos_token_id
                
                logging.info("Processor loaded. Loading model...")
                
                from modular.configuration_vibevoice import VibeVoiceConfig
                config = VibeVoiceConfig.from_pretrained(vibevoice_model_dir)
                
                model = VibeVoiceForConditionalGenerationInference.from_pretrained(
                    vibevoice_model_dir,
                    config=config,
                    device_map=device_map,
                    torch_dtype=torch_dtype,
                )
            
            
            logging.info(f"Model loaded on device: {model.device}")
            
            # =====================================================================
            # INPUT PREPARATION AND GENERATION
            # =====================================================================
            if model_name == 'VibeVoice-Realtime-0.5B':
                # -----------------------------------------------------------------
                # STREAMING: Realtime-0.5B uses voice embedding (.pt) with streaming API
                # -----------------------------------------------------------------
                # 25 preset voices: en (6), de (2), fr (2), it (2), jp (2), kr (2), nl (2), pl (2), pt (2), sp (2), in (1)
                speaker_preset_voices = [
                    "Carter", "Davis", "Emma", "Frank", "Grace", "Mike",  # English
                    "de-Spk0_man", "de-Spk1_woman",  # German
                    "fr-Spk0_man", "fr-Spk1_woman",  # French
                    "it-Spk0_woman", "it-Spk1_man",  # Italian
                    "jp-Spk0_man", "jp-Spk1_woman",  # Japanese
                    "kr-Spk0_woman", "kr-Spk1_man",  # Korean
                    "nl-Spk0_man", "nl-Spk1_woman",  # Dutch
                    "pl-Spk0_man", "pl-Spk1_woman",  # Polish
                    "pt-Spk0_woman", "pt-Spk1_man",  # Portuguese
                    "sp-Spk0_woman", "sp-Spk1_man",  # Spanish
                    "in-Samuel_man"  # Hindi
                ]
                
                if speaker_wav in speaker_preset_voices:
                    # Preset voices (.pt) are in reference_voices/vibevoice RealTime/
                    voices_base = os.path.join(base_project_dir, 'audiobook_generator', 'tts_models', 'vibevoice', 'reference_voices', 'vibevoice RealTime')
                    voice_path = os.path.join(voices_base, f"{speaker_wav}.pt")
                else:
                    # Voice file: supports .pt (voice embedding) or .wav (load and process)
                    voice_path = speaker_wav
                
                logging.info(f"Loading voice embedding: {voice_path}")
                if not os.path.exists(voice_path):
                    raise FileNotFoundError(f"Voice file not found: {voice_path}")
                
                # Load voice embedding with weights_only=False for .pt files
                if voice_path.endswith('.pt'):
                    all_prefilled_outputs = torch.load(voice_path, map_location=model.device, weights_only=False)
                else:
                    # For wav files, use the processor to generate voice embedding
                    import soundfile as sf
                    wav_audio, sr = sf.read(voice_path)
                    if sr != 24000:
                        import torchaudio
                        wav_audio = torchaudio.functional.resample(torch.from_numpy(wav_audio), sr, 24000).numpy()
                    # Convert to tensor and pass to processor
                    all_prefilled_outputs = processor.prepare_input_for_speech(
                        torch.from_numpy(wav_audio).float(), 
                        target_sample_rate=24000
                    )
                
                # Prepare input with streaming API
                inputs = processor.process_input_with_cached_prompt(
                    text=text,
                    cached_prompt=all_prefilled_outputs,
                    padding=True,
                    return_tensors="pt",
                    return_attention_mask=True,
                )
                
                # Move tensors to device
                for k, v in inputs.items():
                    if torch.is_tensor(v):
                        inputs[k] = v.to(model.device)
                
                # Streaming generation
                generate_kwargs = {
                    **inputs,
                    "cfg_scale": cfg_scale if cfg_scale is not None else 1.5,
                    "tokenizer": processor.tokenizer,
                    "generation_config": {"do_sample": use_sampling},
                    "verbose": True,
                    "all_prefilled_outputs": copy.deepcopy(all_prefilled_outputs) if all_prefilled_outputs is not None else None,
                }
                if seed is not None:
                    generate_kwargs["seed"] = seed
                
                logging.info(f"Streaming generation with parameters: cfg_scale={generate_kwargs['cfg_scale']}")
                with torch.no_grad():
                    output = model.generate(**generate_kwargs)
                
                # Extract streaming audio
                if not output.speech_outputs or output.speech_outputs[0] is None:
                    raise RuntimeError("No audio output generated (streaming).")
                
                audio_tensor = output.speech_outputs[0].cpu().detach()
                if audio_tensor.dtype == torch.bfloat16:
                    audio_tensor = audio_tensor.float()
                audio_numpy = audio_tensor.numpy().squeeze()
                
                # Normalize and convert to int16
                if audio_numpy.dtype == np.float32 or audio_numpy.dtype == np.float64:
                    audio_numpy = (audio_numpy * 32767).astype(np.int16)
                
                sampling_rate = 24000
                os.makedirs(os.path.dirname(output_path), exist_ok=True)
                write_wav(output_path, sampling_rate, audio_numpy)
                
            else:
                # -----------------------------------------------------------------
                # NON-STREAMING: 1.5B / 7B
                # -----------------------------------------------------------------
                tokenizer = processor.tokenizer
                logging.info(f"Tokenizer configured from processor: {tokenizer.__class__.__name__}")
                
                if tokenizer.bos_token_id is None:
                    tokenizer.bos_token_id = tokenizer.eos_token_id
                
                model.tokenizer = tokenizer
                
                # Non-streaming input preparation
                inputs = processor(
                    text=[f"Speaker 1: {text}"],
                    voice_samples=[[speaker_wav]],
                    return_tensors="pt"
                )
                
                processed_inputs = {}
                for k, v in inputs.items():
                    if isinstance(v, list):
                        processed_inputs[k] = [t.to(model.device) if hasattr(t, 'to') else t for t in v]
                    elif hasattr(v, 'to'):
                        processed_inputs[k] = v.to(model.device)
                    else:
                        processed_inputs[k] = v
                inputs = processed_inputs
                
                generate_kwargs = {
                    **inputs,
                    "do_sample": use_sampling,
                    "tokenizer": tokenizer,
                    "temperature": temperature,
                    "top_p": top_p,
                    "cfg_scale": cfg_scale if cfg_scale is not None else 1.3,
                    "diffusion_steps": diffusion_steps if diffusion_steps is not None else 15,
                    "voice_speed_factor": voice_speed_factor,
                }
                if seed is not None:
                    generate_kwargs["seed"] = seed
                
                logging.info(f"Generation with parameters: {generate_kwargs}")
                with torch.no_grad():
                    output = model.generate(**generate_kwargs)
                
                speech_outputs = output.speech_outputs
                if not speech_outputs:
                    raise RuntimeError("No audio output generated.")
                
                audio_tensor = speech_outputs[0]
                if isinstance(audio_tensor, (list, tuple)):
                    audio_tensor = audio_tensor[0]
                if isinstance(audio_tensor, (list, tuple)):
                    audio_tensor = audio_tensor[0]
                
                audio_tensor = audio_tensor.cpu().detach()
                if audio_tensor.dtype == torch.bfloat16:
                    audio_tensor = audio_tensor.float()
                audio_numpy = audio_tensor.numpy().squeeze().astype(np.float32)
                
                if np.issubdtype(audio_numpy.dtype, np.floating):
                    audio_numpy = (audio_numpy * 32767).astype(np.int16)
                
                sampling_rate = 24000
                os.makedirs(os.path.dirname(output_path), exist_ok=True)
                write_wav(output_path, sampling_rate, audio_numpy)
                
        finally:
            # Restore original sys.path
            sys.path = original_sys_path
        
        # Verify output
        if os.path.exists(output_path) and os.path.getsize(output_path) > 1024:
            sys.stdout = original_stdout
            send_response({"status": "ok", "file": output_path, "message": "Synthesis completed successfully."})
        else:
            raise RuntimeError("Output file not created or empty.")

    except Exception as e:
        logging.error(f"Error in VibeVoice subprocess: {e}", exc_info=True)
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
