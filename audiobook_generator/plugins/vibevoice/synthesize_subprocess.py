# ATTENZIONE: Questo script è autonomo e non deve importare nulla dal progetto principale.
import sys
import json
import numpy as np
from scipy.io.wavfile import write as write_wav
import os
import logging

# Setup di un logger dedicato per il subprocess
# Percorso cross-platform per i log nella cartella audiobook_generator/Logs/
log_dir = os.path.join('audiobook_generator', 'Logs')
os.makedirs(log_dir, exist_ok=True)
log_file = os.path.join(log_dir, 'vibevoice_subprocess.log')
logging.basicConfig(level=logging.INFO, filename=log_file, filemode='a',
                    format='%(asctime)s - %(levelname)s - %(message)s')

def main():
    try:
        # Importa torch solo quando necessario
        import torch
        
        # 1. Legge l'input JSON da stdin
        input_data = json.load(sys.stdin)
        logging.info(f"Ricevuto job: {input_data}")
        
        text = input_data['text']
        output_path = input_data['output_path']
        speaker_wav = input_data['speaker_wav']
        model_name = input_data.get('model_name', 'VibeVoice')
        
        # Estrai parametri di generazione con valori di default
        temperature = input_data.get('temperature', 0.9)
        top_p = input_data.get('top_p', 0.9)
        cfg_scale = input_data.get('cfg_scale', 1.3)
        diffusion_steps = input_data.get('diffusion_steps', 15)
        voice_speed_factor = input_data.get('voice_speed_factor', 1.0)
        use_sampling = input_data.get('use_sampling', True)
        seed = input_data.get('seed')
        
        logging.info(f"Parametri generazione: temperature={temperature}, top_p={top_p}, cfg_scale={cfg_scale}, "
                     f"diffusion_steps={diffusion_steps}, voice_speed_factor={voice_speed_factor}, "
                     f"use_sampling={use_sampling}, seed={seed}")

        # --- Logica di caricamento da asset locali come da report tecnico ---
        # Definizione percorsi locali
        base_project_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', '..'))
        
        # Determina directory modello in base al nome
        if model_name == 'VibeVoice-7B':
            model_subdir = '7B'
        elif model_name == 'VibeVoice-Realtime-0.5B':
            model_subdir = '0.5B'
        else:
            model_subdir = '1.5B'  # fallback per compatibilità
        
        # Struttura: vibevoice/{1.5B,7B,0.5B}/
        # Struttura: vibevoice/{1.5B,7B,0.5B}/ per i modelli
        # repo_community/vibevoice per codice (streaming + inference)
        vibevoice_model_dir = os.path.join(base_project_dir, 'audiobook_generator', 'tts_models', 'vibevoice', model_subdir)
        
        # Tokenizer Qwen2.5-1.5B: cartella separata tts_models/vibevoice/tokenizer/
        # (NON è dentro il modello — il modello su HF non include i file del tokenizer)
        vibevoice_tokenizer_dir = os.path.join(base_project_dir, 'audiobook_generator', 'tts_models', 'vibevoice', 'tokenizer')
        
        # Codice sorgente: repo_community/vibevoice per Realtime-0.5B (streaming), repo/vibevoice per 1.5B/7B
        # VibeVoiceStreamingForConditionalGenerationInference è in repo_community (community repo con classi streaming)
        # VibeVoiceForConditionalGenerationInference (non-streaming) e streaming sono entrambi in repo_community
        if model_name == 'VibeVoice-Realtime-0.5B':
            vibevoice_code_base = os.path.join(base_project_dir, 'audiobook_generator', 'tts_models', 'vibevoice', 'repo_community')
        else:
            # 1.5B/7B: usa repo_community (contiene sia modeling_vibevoice.py che modeling_vibevoice_inference.py)
            vibevoice_code_base = os.path.join(base_project_dir, 'audiobook_generator', 'tts_models', 'vibevoice', 'repo_community')
        
        vibevoice_code_dir = os.path.join(vibevoice_code_base, 'vibevoice')
        
        logging.info(f"Caricamento modello: {model_name} -> {model_subdir}")
        
        # Log dettagliato GPU
        logging.info(f"Torch version: {torch.__version__}")
        logging.info(f"CUDA available: {torch.cuda.is_available()}")
        if torch.cuda.is_available():
            logging.info(f"CUDA device count: {torch.cuda.device_count()}")
            for i in range(torch.cuda.device_count()):
                logging.info(f"  Device {i}: {torch.cuda.get_device_name(i)}")
                logging.info(f"    Memory allocated: {torch.cuda.memory_allocated(i) / 1024**2:.2f} MB")
                logging.info(f"    Memory reserved: {torch.cuda.memory_reserved(i) / 1024**2:.2f} MB")
        else:
            logging.warning("CUDA non disponibile. Il modello verrà caricato su CPU.")

        # Manipolazione sys.path per importare il codice sorgente di VibeVoice
        original_sys_path = sys.path.copy()
        try:
            # Aggiungi entrambi i path per i diversi pattern di import
            if vibevoice_code_base not in sys.path:
                sys.path.insert(0, vibevoice_code_base)
            if vibevoice_code_dir not in sys.path:
                sys.path.insert(0, vibevoice_code_dir)
            
            use_gpu = torch.cuda.is_available()
            device_map = "cuda:0" if use_gpu else "cpu"
            torch_dtype = torch.bfloat16 if use_gpu else torch.float32
            
            if model_name == 'VibeVoice-Realtime-0.5B':
                # =====================================================================
                # PATH STREAMING (Realtime-0.5B)
                # =====================================================================
                import copy
                from modular.modeling_vibevoice_streaming_inference import VibeVoiceStreamingForConditionalGenerationInference
                from processor.vibevoice_streaming_processor import VibeVoiceStreamingProcessor
                
                logging.info("Import streaming completato. Caricamento processor streaming...")
                
                # Carica processor streaming
                processor = VibeVoiceStreamingProcessor.from_pretrained(vibevoice_model_dir)
                logging.info(f"Processor streaming caricato: {processor.__class__.__name__}")
                
                logging.info("Caricamento modello streaming...")
                model = VibeVoiceStreamingForConditionalGenerationInference.from_pretrained(
                    vibevoice_model_dir,
                    device_map=device_map,
                    torch_dtype=torch_dtype,
                )
                model.set_ddpm_inference_steps(num_steps=diffusion_steps if diffusion_steps is not None else 5)
                logging.info(f"Modello streaming caricato su: {model.device}")
                
            else:
                # =====================================================================
                # PATH NON-STREAMING (1.5B / 7B)
                # =====================================================================
                from modular.modeling_vibevoice_inference import VibeVoiceForConditionalGenerationInference
                from processor.vibevoice_processor import VibeVoiceProcessor
                
                logging.info("Import completato. Caricamento processore...")
                
                logging.info("Loading VibeVoiceProcessor from model directory...")
                processor = VibeVoiceProcessor.from_pretrained(
                    vibevoice_model_dir,
                    language_model_pretrained_name=vibevoice_tokenizer_dir,
                    local_files_only=True,
                    trust_remote_code=False
                )
                logging.info(f"Processor caricato: {processor.__class__.__name__}")
                
                tokenizer = processor.tokenizer
                if tokenizer is None:
                    raise RuntimeError("FATAL: Tokenizer is None after loading processor.")
                logging.info(f"Tokenizer wrapper caricato: {tokenizer.__class__.__name__}")
                
                if tokenizer.bos_token_id is None:
                    logging.warning(f"bos_token_id è None, impostato a eos_token_id: {tokenizer.eos_token_id}")
                    tokenizer.bos_token_id = tokenizer.eos_token_id
                
                logging.info("Processore caricato. Caricamento modello...")
                
                from modular.configuration_vibevoice import VibeVoiceConfig
                config = VibeVoiceConfig.from_pretrained(vibevoice_model_dir)
                
                model = VibeVoiceForConditionalGenerationInference.from_pretrained(
                    vibevoice_model_dir,
                    config=config,
                    device_map=device_map,
                    torch_dtype=torch_dtype,
                )
            
            
            logging.info(f"Modello caricato su dispositivo: {model.device}")
            
            # =====================================================================
            # PREPARAZIONE INPUT E GENERAZIONE
            # =====================================================================
            if model_name == 'VibeVoice-Realtime-0.5B':
                # -----------------------------------------------------------------
                # STREAMING: Realtime-0.5B usa voice embedding (.pt) con API streaming
                # -----------------------------------------------------------------
                # 25 voci preset: en (6), de (2), fr (2), it (2), jp (2), kr (2), nl (2), pl (2), pt (2), sp (2), in (1)
                speaker_preset_voices = [
                    "Carter", "Davis", "Emma", "Frank", "Grace", "Mike",  # Inglese
                    "de-Spk0_man", "de-Spk1_woman",  # Tedesco
                    "fr-Spk0_man", "fr-Spk1_woman",  # Francese
                    "it-Spk0_woman", "it-Spk1_man",  # Italiano
                    "jp-Spk0_man", "jp-Spk1_woman",  # Giapponese
                    "kr-Spk0_woman", "kr-Spk1_man",  # Coreano
                    "nl-Spk0_man", "nl-Spk1_woman",  # Olandese
                    "pl-Spk0_man", "pl-Spk1_woman",  # Polacco
                    "pt-Spk0_woman", "pt-Spk1_man",  # Portoghese
                    "sp-Spk0_woman", "sp-Spk1_man",  # Spagnolo
                    "in-Samuel_man"  # Hindi
                ]
                
                if speaker_wav in speaker_preset_voices:
                    # Le voci preset (.pt) sono in reference_voices/vibevoice RealTime/
                    voices_base = os.path.join(base_project_dir, 'audiobook_generator', 'tts_models', 'vibevoice', 'reference_voices', 'vibevoice RealTime')
                    voice_path = os.path.join(voices_base, f"{speaker_wav}.pt")
                else:
                    # Voce file: supporta .pt (voice embedding) o .wav (carica e processa)
                    voice_path = speaker_wav
                
                logging.info(f"Caricamento voice embedding: {voice_path}")
                if not os.path.exists(voice_path):
                    raise FileNotFoundError(f"Voice file non trovato: {voice_path}")
                
                # Carica voice embedding con weights_only=False per file .pt
                if voice_path.endswith('.pt'):
                    all_prefilled_outputs = torch.load(voice_path, map_location=model.device, weights_only=False)
                else:
                    # Per file wav, usa il processor per generare voice embedding
                    import soundfile as sf
                    wav_audio, sr = sf.read(voice_path)
                    if sr != 24000:
                        import torchaudio
                        wav_audio = torchaudio.functional.resample(torch.from_numpy(wav_audio), sr, 24000).numpy()
                    # Converte in tensore e passa al processor
                    all_prefilled_outputs = processor.prepare_input_for_speech(
                        torch.from_numpy(wav_audio).float(), 
                        target_sample_rate=24000
                    )
                
                # Prepara input con API streaming
                inputs = processor.process_input_with_cached_prompt(
                    text=text,
                    cached_prompt=all_prefilled_outputs,
                    padding=True,
                    return_tensors="pt",
                    return_attention_mask=True,
                )
                
                # Sposta tensori sul device
                for k, v in inputs.items():
                    if torch.is_tensor(v):
                        inputs[k] = v.to(model.device)
                
                # Generazione streaming
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
                
                logging.info(f"Generazione streaming con parametri: cfg_scale={generate_kwargs['cfg_scale']}")
                with torch.no_grad():
                    output = model.generate(**generate_kwargs)
                
                # Estrazione audio streaming
                if not output.speech_outputs or output.speech_outputs[0] is None:
                    raise RuntimeError("Nessun output audio generato (streaming).")
                
                audio_tensor = output.speech_outputs[0].cpu().detach()
                if audio_tensor.dtype == torch.bfloat16:
                    audio_tensor = audio_tensor.float()
                audio_numpy = audio_tensor.numpy().squeeze()
                
                # Normalizza e converti a int16
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
                logging.info(f"Tokenizer configurato dal processore: {tokenizer.__class__.__name__}")
                
                if tokenizer.bos_token_id is None:
                    tokenizer.bos_token_id = tokenizer.eos_token_id
                
                model.tokenizer = tokenizer
                
                # Preparazione input non-streaming
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
                
                logging.info(f"Generazione con parametri: {generate_kwargs}")
                with torch.no_grad():
                    output = model.generate(**generate_kwargs)
                
                speech_outputs = output.speech_outputs
                if not speech_outputs:
                    raise RuntimeError("Nessun output audio generato.")
                
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
            # Ripristino sys.path originale
            sys.path = original_sys_path
        
        # Verifica output
        if os.path.exists(output_path) and os.path.getsize(output_path) > 1024:
            send_response({"status": "ok", "file": output_path, "message": "Sintesi completata con successo."})
        else:
            raise RuntimeError("File di output non creato o vuoto.")

    except Exception as e:
        logging.error(f"Errore nel subprocess VibeVoice: {e}", exc_info=True)
        send_response({"status": "error", "message": str(e)})

def send_response(data):
    """Invia una risposta JSON a stdout."""
    sys.stdout.write(json.dumps(data) + '\n')
    sys.stdout.flush()

if __name__ == "__main__":
    main()
