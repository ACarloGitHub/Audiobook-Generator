import sys
import json
import os
import logging
import contextlib
from pathlib import Path
# Importa qwen_tts DOPO aver reindirizzato stdout/stderr per evitare warning su stdout

# Setup di un logger dedicato su file SENZA usare stdout/stderr
# Percorso cross-platform per i log nella cartella audiobook_generator/Logs/
log_dir = os.path.join('audiobook_generator', 'Logs')
os.makedirs(log_dir, exist_ok=True)
log_file = os.path.join(log_dir, 'qwen3_subprocess.log')
logging.basicConfig(
    level=logging.INFO, 
    filename=log_file, 
    filemode='a', 
    format='%(asctime)s - %(message)s',
    force=True  # Sovrascrive eventuali configurazioni precedenti
)

def main():
    try:
        # Importa torch solo quando necessario
        import torch
        
        # Forza modalità offline per Hugging Face
        os.environ["HF_HUB_OFFLINE"] = "1"
        os.environ["TRANSFORMERS_OFFLINE"] = "1"
        
        # Reindirizza stdout e stderr a /dev/null PRIMA di importare qwen_tts per evitare warning
        with open(os.devnull, 'w') as devnull:
            with contextlib.redirect_stdout(devnull), contextlib.redirect_stderr(devnull):
                # Importa qwen_tts e Qwen3TTSModel dentro il contesto silenziato
                from qwen_tts import Qwen3TTSModel
        
        # Aggiungi sox/bin al PATH per evitare errori di soundfile
        sox_bin = os.path.join(os.getcwd(), "sox", "bin")
        if os.path.exists(sox_bin):
            os.environ["PATH"] = sox_bin + os.pathsep + os.environ["PATH"]
        
        # Importa soundfile DOPO aver modificato il PATH
        import soundfile as sf
        
        payload = json.load(sys.stdin)
        logging.info(f"Ricevuto job: {payload}")
        
        text = payload['text']
        output_path = payload['output_path']
        mode = payload.get('mode')
        if mode is None:
            logging.warning("Mode non fornito nel payload, default a 'clone'")
            mode = "clone"
        params = payload.get('params', {})
        model_size = payload.get('model_size', '0.6B')  # Default a 0.6B
        model_type = payload.get('model_type', 'base')  # 'base', 'custom_voice', 'voice_design'

        # Costruisce percorsi relativi alla directory di questo script
        script_dir = Path(__file__).parent.absolute()
        
        # Determina directory modello in base a size e type
        # USA NOMI UFFICIALI: Qwen3-TTS-12Hz-0.6B-Base, Qwen3-TTS-12Hz-1.7B-CustomVoice, ecc.
        # Mappa model_type a folder type name
        if model_type == 'base':
            type_folder = "Base"
        elif model_type == 'custom_voice':
            type_folder = "CustomVoice"
        elif model_type == 'voice_design':
            type_folder = "VoiceDesign"
        else:
            type_folder = model_type
        
        # Costruisci nome cartella ufficiale: Qwen3-TTS-12Hz-{size}-{TypeFolder}
        # Esempi: Qwen3-TTS-12Hz-0.6B-Base, Qwen3-TTS-12Hz-1.7B-VoiceDesign
        model_dir_name = f"Qwen3-TTS-12Hz-{model_size}-{type_folder}"
        
        model_dir = (script_dir / f"../../tts_models/qwen3tts/{model_dir_name}").resolve()
        tokenizer_dir = (script_dir / "../../tts_models/qwen3tts/tokenizer").resolve()
        
        # Converti in stringa POSIX (forward slash) per evitare problemi con backslash Windows
        model_dir = model_dir.as_posix()
        tokenizer_dir = tokenizer_dir.as_posix()
        
        logging.info(f"Model dir: {model_dir}")
        logging.info(f"Tokenizer dir: {tokenizer_dir}")

        # Verifica che la directory del modello esista
        if not os.path.exists(model_dir):
            raise FileNotFoundError(f"Directory del modello non trovata: {model_dir}")
        
        # Silenzia stdout e stderr durante il caricamento del modello per evitare output non JSON
        with open(os.devnull, 'w') as devnull:
            with contextlib.redirect_stdout(devnull), contextlib.redirect_stderr(devnull):
                model = Qwen3TTSModel.from_pretrained(
                    model_dir,
                    device_map="cuda:0" if torch.cuda.is_available() else "cpu",
                    dtype=torch.bfloat16 if torch.cuda.is_available() and torch.cuda.is_bf16_supported() else torch.float16,
                    attn_implementation="eager",
                    local_files_only=True,
                    trust_remote_code=True
                )

        # Parametri comuni a tutte le modalità (basati su documentazione Qwen3-TTS)
        # NOTA: I valori di default devono corrispondere esattamente all'UI (configuration_tab.py)
        common_kwargs = {
            "speed": params.get("speed", 1.0),
            "pitch": params.get("pitch", 0),
            "volume": params.get("volume", 0),
            "temperature": params.get("temperature", 0.7),
            "top_p": params.get("top_p", 0.8),
            "top_k": params.get("top_k", 20),
            "repetition_penalty": params.get("repetition_penalty", 1.1),
            "seed": params.get("seed"),  # può essere None
            # Parametri aggiuntivi dalla documentazione HF Transformers
            "max_new_tokens": 2048,
            "do_sample": True,
        }
        # Rimuovi i parametri None per non passare valori non validi
        common_kwargs = {k: v for k, v in common_kwargs.items() if v is not None}

        # Mappa modalità UI a metodi modello in base al tipo di modello
        # Secondo la documentazione Qwen3-TTS:
        # - Base model (model_type='base'): supporta solo generate_voice_clone (mode='clone')
        # - CustomVoice model (model_type='custom_voice'): supporta solo generate_custom_voice (mode='custom')
        # - VoiceDesign model (model_type='voice_design'): supporta solo generate_voice_design (mode='design')
        
        if model_type == 'custom_voice':
            # Il modello CustomVoice supporta solo generate_custom_voice
            # Parametri obbligatori: text, language, speaker
            # Parametri opzionali: instruct
            # Gestisce sia 'speaker' (nuovo) che 'voice' (vecchio) per backward compatibility
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
            # Il modello VoiceDesign supporta solo generate_voice_design
            # Parametri obbligatori: text, language, instruct
            language = params.get('language', 'Italian')
            instruct = params.get('instruct', '')
            
            # Gestione language: potrebbe essere un numero (indice) da Gradio
            # Converti in stringa e mappa a lingua supportata
            if language is not None:
                language = str(language)
                # Mappa indici numerici UI a lingue supportate (minuscolo per Qwen3-TTS)
                language_map = {
                    '0': 'auto', '1': 'italian', '2': 'english', '3': 'french', 
                    '4': 'german', '5': 'portuguese', '6': 'spanish', '7': 'japanese',
                    '8': 'korean', '9': 'russian', '10': 'chinese'
                }
                if language in language_map:
                    language = language_map[language]
                # Assicura che sia in minuscolo e corrisponda a una lingua supportata
                supported = ['auto', 'chinese', 'english', 'french', 'german', 'italian', 
                            'japanese', 'korean', 'portuguese', 'russian', 'spanish']
                if language.lower() not in supported:
                    logging.warning(f"VoiceDesign: language '{language}' non supportato, default a 'italian'")
                    language = 'italian'
                else:
                    language = language.lower()
            else:
                language = 'italian'
            
            if not instruct:
                logging.warning("VoiceDesign: instruct è vuoto, la qualità potrebbe essere compromessa")
            
            logging.info(f"VoiceDesign: text={text[:50]}..., language={language}, instruct={instruct[:50] if instruct else ''}")
            
            wavs, sr = model.generate_voice_design(
                text=text,
                instruct=instruct,
                language=language,
                **common_kwargs
            )
        else:  # base
            # Il modello Base supporta solo generate_voice_clone
            # L'UI dovrebbe passare mode='clone' per questo modello
            if mode not in ['clone', 'custom', 'design']:
                logging.warning(f"Mode '{mode}' non valido per modello Base, forzato a 'clone'")
                mode = 'clone'
            
            if mode == 'custom':
                raise ValueError("Modalità 'custom' non supportata per modello Base. Usa modello CustomVoice.")
            elif mode == 'design':
                raise ValueError("Modalità 'design' non supportata per modello Base. Usa modello VoiceDesign.")
            
            # Modalità clone (unica supportata per modello Base)
            # Parametri obbligatori: text, language, ref_audio
            # ref_text è obbligatorio solo se x_vector_only_mode=False
            language = params.get('language', 'Italian')
            ref_audio = params.get('ref_audio')
            x_vector_only_mode = params.get('x_vector_only_mode', False)
            
            if not ref_audio:
                raise ValueError("Per la modalità Voice Clone, è necessario fornire un file audio di riferimento (ref_audio).")
            
            logging.info(f"Voice Clone: text={text[:50]}..., language={language}, ref_audio={ref_audio}, x_vector_only_mode={x_vector_only_mode}")
            
            if x_vector_only_mode:
                # Modalità veloce: non richiede ref_text
                wavs, sr = model.generate_voice_clone(
                    text=text,
                    language=language,
                    ref_audio=ref_audio,
                    x_vector_only_mode=True,
                    **common_kwargs
                )
            else:
                # Modalità completa: richiede ref_text
                ref_text = params.get('ref_text', '')
                if not ref_text:
                    raise ValueError("Per la modalità Voice Clone a qualità massima, è necessaria la trascrizione del testo (ref_text).")
                
                wavs, sr = model.generate_voice_clone(
                    text=text,
                    language=language,
                    ref_audio=ref_audio,
                    ref_text=ref_text,
                    x_vector_only_mode=False,
                    **common_kwargs
                )
        
        # Sopprime output stderr di soundfile (errore SoX) durante la scrittura
        with open(os.devnull, 'w') as devnull:
            with contextlib.redirect_stderr(devnull):
                import numpy as np
                import subprocess
                import tempfile
                
                # Converte tensore PyTorch in numpy array
                if torch.is_tensor(wavs):
                    wavs_np = wavs.cpu().numpy()
                else:
                    wavs_np = np.array(wavs)
                
                # DEBUG: Log delle dimensioni per diagnosticare problemi
                logging.info(f"Shape originale audio: {wavs_np.shape}, dtype: {wavs_np.dtype}")
                
                # Rimuovi dimensioni batch singole (es: [1, 24000] -> [24000])
                # o [1, 1, 24000] -> [24000]
                while wavs_np.ndim > 1 and wavs_np.shape[0] == 1:
                    wavs_np = wavs_np.squeeze(0)
                
                # Se risulta 2D con shape [channels, samples], trasponi in [samples, channels]
                # ma se è già 1D, lascialo così
                if wavs_np.ndim == 2 and wavs_np.shape[0] < wavs_np.shape[1]:
                    # Probabilmente è [channels, samples], trasponiamo
                    wavs_np = wavs_np.T
                
                # Assicurati che sia monodimensionale se mono, o bidimensionale corretto se stereo
                if wavs_np.ndim > 2:
                    wavs_np = wavs_np.reshape(-1)
                
                # Normalizzazione robusta
                max_val = np.max(np.abs(wavs_np))
                if max_val > 1.0:
                    logging.warning(f"Normalizzazione audio: valori fuori range (max={max_val:.3f})")
                    wavs_np = wavs_np / max_val
                elif max_val < 0.001:
                    logging.warning(f"Audio quasi silenzioso (max={max_val:.3f})")
                
                # Converte a float32 per compatibilità massima
                wavs_np = wavs_np.astype(np.float32)
                
                # Assicura sample rate sia intero
                sr_int = int(sr) if sr is not None else 24000
                
                # DEBUG: Log finale
                logging.info(f"Audio pronto per salvataggio: shape={wavs_np.shape}, dtype={wavs_np.dtype}, sr={sr_int}")
                
                # METODO 1: FFMPEG (prima scelta - già presente nel progetto)
                try:
                    # Crea file raw PCM temporaneo
                    with tempfile.NamedTemporaryFile(suffix='.raw', delete=False) as tmp_raw:
                        raw_path = tmp_raw.name
                        wavs_np.tofile(raw_path)
                    
                    # Percorso FFmpeg locale
                    ffmpeg_path = os.path.join(os.getcwd(), "ffmpeg", "bin", "ffmpeg.exe")
                    if not os.path.exists(ffmpeg_path):
                        # Fallback a ffmpeg nel PATH
                        ffmpeg_path = "ffmpeg"
                    
                    # Determina canali
                    channels = 1 if wavs_np.ndim == 1 else wavs_np.shape[1]
                    
                    # Comando FFmpeg per convertire raw PCM float32 a WAV PCM s16le
                    cmd = [
                        ffmpeg_path,
                        '-f', 'f32le',          # formato input: float32 little-endian
                        '-ar', str(sr_int),     # sample rate
                        '-ac', str(channels),   # canali
                        '-i', raw_path,         # file input
                        '-c:a', 'pcm_s16le',    # codec audio: PCM signed 16-bit little-endian
                        '-y',                   # sovrascrivi output
                        output_path
                    ]
                    
                    logging.info(f"Esecuzione FFmpeg: {' '.join(cmd)}")
                    result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
                    
                    if result.returncode == 0:
                        # Elimina file raw temporaneo
                        os.unlink(raw_path)
                        logging.info(f"File WAV salvato con FFmpeg: {output_path}, size: {os.path.getsize(output_path)} bytes")
                    else:
                        logging.error(f"FFmpeg fallito: {result.stderr}")
                        raise RuntimeError(f"FFmpeg fallito: {result.stderr}")
                        
                except Exception as e_ffmpeg:
                    logging.warning(f"FFmpeg fallito: {e_ffmpeg}, provo scipy...")
                    
                    # METODO 2: SCIPY (seconda scelta)
                    try:
                        from scipy.io import wavfile
                        # Scipy vuole int16 per PCM, convertiamo da float32 [-1,1] a int16
                        wavs_int16 = (wavs_np * 32767).astype(np.int16)
                        wavfile.write(output_path, sr_int, wavs_int16)
                        logging.info(f"File WAV salvato con scipy: {output_path}")
                    except ImportError:
                        logging.warning("Scipy non disponibile, provo soundfile...")
                        # METODO 3: SOUNDFILE (terza scelta)
                        try:
                            # Se è mono 1D, soundfile è felice
                            # Se è stereo, deve essere shape (samples, channels)
                            sf.write(output_path, wavs_np, sr_int, subtype='PCM_16', format='WAV')
                            logging.info(f"File WAV salvato con soundfile: {output_path}")
                        except Exception as e_sf:
                            # METODO 4: TORCHAUDIO (ultima risorsa)
                            try:
                                import torchaudio
                                wavs_tensor = torch.from_numpy(wavs_np).unsqueeze(0) if wavs_np.ndim == 1 else torch.from_numpy(wavs_np)
                                torchaudio.save(output_path, wavs_tensor, sr_int)
                                logging.info(f"File salvato con torchaudio: {output_path}")
                            except Exception as e_ta:
                                raise RuntimeError(f"Tutti i metodi di salvataggio falliti: FFmpeg({e_ffmpeg}), scipy(ImportError), soundfile({e_sf}), torchaudio({e_ta})")
                    except Exception as e_scipy:
                        logging.warning(f"Scipy fallito: {e_scipy}, provo soundfile...")
                        try:
                            sf.write(output_path, wavs_np, sr_int, subtype='PCM_16', format='WAV')
                            logging.info(f"File WAV salvato con soundfile: {output_path}")
                        except Exception as e_sf:
                            raise RuntimeError(f"Salvataggio fallito dopo FFmpeg e scipy: soundfile({e_sf})")
        
        send_response({"status": "ok", "file": output_path})

    except Exception as e:
        logging.error(f"Errore nel subprocess Qwen3-TTS: {e}", exc_info=True)
        send_response({"status": "error", "message": str(e)})

def send_response(data):
    sys.stdout.write(json.dumps(data) + '\n')
    sys.stdout.flush()

if __name__ == "__main__":
    main()