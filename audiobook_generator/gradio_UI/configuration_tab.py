# audiobook_generator/gradio_UI/configuration_tab.py
import gradio as gr

def create_configuration_tab(TTS_MODELS, MODEL_LANGUAGES, config):
    """Crea e restituisce i componenti della scheda di configurazione."""
    with gr.TabItem("1. Configuration"):
        gr.Markdown("## TTS Engine and Voice")
        with gr.Row():
            model_radio = gr.Radio(label="TTS Model", choices=TTS_MODELS, value=TTS_MODELS[0], interactive=True)

        with gr.Row():
            xtts_voice_file = gr.File(label="Upload Reference WAV (.wav)", file_types=[".wav"], visible=(TTS_MODELS[0] == "XTTSv2" or TTS_MODELS[0].startswith("VibeVoice")))

        with gr.Group(visible=(TTS_MODELS[0] == "XTTSv2")) as xtts_params_group:
            xtts_lang_dropdown = gr.Dropdown(label="Language", choices=MODEL_LANGUAGES.get("XTTSv2", ["en", "it"]), value=config.DEFAULT_LANGUAGE, interactive=True)
            xtts_temp_slider = gr.Slider(0.3, 1.0, value=0.75, step=0.05, label="Temperature", interactive=True, info="Voice creativity: lower = more monotone, higher = more varied")
            xtts_speed_slider = gr.Slider(0.5, 2.0, value=1.0, step=0.1, label="Speed", interactive=True, info="Speech speed: <1 = slower, >1 = faster")
            xtts_rep_pen_slider = gr.Slider(1.0, 10.0, value=2.0, step=0.5, label="Repetition Penalty", interactive=True, info="Repetition penalty: higher values reduce repetitions")
            xtts_top_k_slider = gr.Slider(1, 100, value=50, step=1, label="Top-K", interactive=True, info="Top-k sampling: limits voice choices (lower = more predictable)")
            xtts_top_p_slider = gr.Slider(0.1, 1.0, value=0.85, step=0.05, label="Top-P", interactive=True, info="Nucleus sampling: controls acoustic vocabulary diversity")
            xtts_length_penalty_slider = gr.Slider(0.5, 2.0, value=1.0, step=0.1, label="Length Penalty", interactive=True, info="Length penalty: <1 = shorter output, >1 = longer output")
            xtts_gpt_cond_len_slider = gr.Slider(1, 60, value=30, step=1, label="GPT Conditioning Length", interactive=True, info="GPT conditioning length: controls how much context the model uses")

        with gr.Group(visible=(TTS_MODELS[0] == "Piper")) as piper_params_group:
            piper_speed_slider = gr.Slider(0.1, 5.0, value=config.DEFAULT_TTS_SPEED, step=0.1, label="Speed", interactive=True)
            piper_noise_scale_slider = gr.Slider(0.0, 2.0, value=config.DEFAULT_TTS_NOISE_SCALE, step=0.05, label="Noise Scale", interactive=True)
            piper_noise_scale_w_slider = gr.Slider(0.0, 2.0, value=config.DEFAULT_TTS_NOISE_SCALE_W, step=0.05, label="Noise Scale W", interactive=True)

        with gr.Group(visible=(TTS_MODELS[0] == "Kokoro")) as kokoro_params_group:
            kokoro_lang_dropdown = gr.Dropdown(label="Language", choices=MODEL_LANGUAGES.get("Kokoro", ["it", "en"]), value=config.DEFAULT_LANGUAGE, interactive=True)
            piper_kokoro_voice_dropdown = gr.Dropdown(label="Select Kokoro Voice", choices=[], visible=False)
            kokoro_speed_slider = gr.Slider(0.5, 2.0, value=1.0, step=0.1, label="Speed", interactive=True, info="Speech speed: <1 = slower, >1 = faster")

        with gr.Group(visible=False) as vibevoice_params_group:
            gr.Markdown("### 🎵 VibeVoice Parameters (1.5B/7B)")
            vibevoice_lang_dropdown = gr.Dropdown(label="Language", choices=MODEL_LANGUAGES.get("VibeVoice", ["en", "it"]), value=config.DEFAULT_LANGUAGE, interactive=True)
            with gr.Row():
                vibevoice_temp_slider = gr.Slider(0.0, 2.0, value=1.0, step=0.05, label="Temperature", interactive=True, info="Controls randomness: lower = more monotone, higher = more varied")
                vibevoice_cfg_scale_slider = gr.Slider(1.0, 2.0, value=1.3, step=0.05, label="CFG Scale", interactive=True, info="How strictly the voice follows the text: higher = more faithful")
            with gr.Row():
                vibevoice_diffusion_steps_slider = gr.Slider(1, 50, value=15, step=1, label="Diffusion Steps", interactive=True, info="Quality vs speed: more steps = better quality but slower")
                vibevoice_speed_factor_slider = gr.Slider(0.5, 2.0, value=1.0, step=0.1, label="Voice Speed Factor", interactive=True, info="Speech speed: <1 = slower, >1 = faster")
            with gr.Accordion("Advanced Parameters", open=False):
                with gr.Row():
                    vibevoice_top_p_slider = gr.Slider(0.0, 1.0, value=0.9, step=0.05, label="Top-P", interactive=True, info="Nucleus sampling: controls acoustic vocabulary diversity")
                    vibevoice_top_k_slider = gr.Slider(0, 100, value=0, step=1, label="Top-K", interactive=True, info="Top-k sampling: 0 = disabled, >0 = limits voice choices")
                with gr.Row():
                    vibevoice_seed_number = gr.Number(label="Seed (optional)", value=None, interactive=True, info="For reproducible results: leave empty for random")
                vibevoice_use_sampling_checkbox = gr.Checkbox(label="Use Sampling", value=True, interactive=True, info="Use stochastic sampling: if disabled, more deterministic")

        with gr.Group(visible=False) as vibevoice_realtime_params_group:
            gr.Markdown("### 🎵 VibeVoice-Realtime Parameters (0.5B)")
            vibevoice_realtime_speaker_dropdown = gr.Dropdown(
                label="Preset Voice",
                choices=[
                    "de-Spk0_man", "de-Spk1_woman", "fr-Spk0_man", "fr-Spk1_woman", "it-Spk0_woman", "it-Spk1_man", "jp-Spk0_man", "jp-Spk1_woman",
                    "kr-Spk0_woman", "kr-Spk1_man", "nl-Spk0_man", "nl-Spk1_woman", "pl-Spk0_man", "pl-Spk1_woman", "pt-Spk0_woman", "pt-Spk1_man",
                    "sp-Spk0_woman", "sp-Spk1_woman", "in-Samuel_man"
                ],
                value="it-Spk0_woman", interactive=True, info="Available voices for Realtime-0.5B"
            )
            with gr.Row():
                vibevoice_realtime_cfg_scale_slider = gr.Slider(1.0, 2.0, value=1.3, step=0.1, label="CFG Scale", interactive=True, info="Classifier-Free Guidance: higher = more conditioned")
                vibevoice_realtime_ddpm_steps_slider = gr.Slider(1, 50, value=5, step=1, label="DDPM Steps", interactive=True, info="Denoising steps: more steps = better quality but slower")
            with gr.Row():
                vibevoice_realtime_temperature_slider = gr.Slider(0.0, 2.0, value=1.0, step=0.1, label="Temperature", interactive=True, info="Sampling temperature: 0 = deterministic, >0 = variation")
                vibevoice_realtime_top_p_slider = gr.Slider(0.0, 1.0, value=0.9, step=0.05, label="Top-P", interactive=True, info="Nucleus sampling")
            with gr.Row():
                vibevoice_realtime_top_k_slider = gr.Slider(0, 100, value=0, step=1, label="Top-K", interactive=True, info="Top-k sampling: 0 = disabled, >0 = limits voice choices")
                vibevoice_realtime_seed_number = gr.Number(label="Seed (optional)", value=None, interactive=True, info="For reproducible results: leave empty for random")

        with gr.Group(visible=False) as qwen_params_group:
            with gr.Tabs() as qwen_tabs:
                with gr.Tab("Single Voice"):
                    gr.Markdown("### 🚀 Qwen3-TTS Parameters (Single Voice)")
                    qwen_mode_radio = gr.Radio(label="Generation Mode", choices=["Custom Voice", "Voice Clone", "Voice Design"], value="Custom Voice")
                    with gr.Group(visible=True) as qwen_custom_group:
                        qwen_custom_voice_dropdown = gr.Dropdown(label="Preset Voice (9 available)", choices=["Vivian", "Serena", "Uncle_Fu", "Dylan", "Eric", "Ryan", "Aiden", "Ono_Anna", "Sohee"], value="Serena")
                        qwen_custom_language_dropdown = gr.Dropdown(label="Language", choices=["Auto", "Chinese", "English", "German", "Italian", "Portuguese", "Spanish", "Japanese", "Korean", "French", "Russian"], value="Auto")
                        qwen_custom_instruct_textbox = gr.Textbox(label="Additional Instructions (optional)", placeholder="Examples: Speak slowly. With excitement and energy. In a sad tone.")
                        qwen_custom_warning = gr.Markdown("**Note:** The CustomVoice (1.7B) model has not been downloaded...", visible=False)
                    with gr.Group(visible=False) as qwen_clone_group:
                        qwen_clone_ref_audio = gr.File(label="Reference Audio (3-20s, .wav)", file_types=[".wav"])
                        qwen_clone_ref_text = gr.Textbox(label="Exact Audio Transcription", lines=2)
                        qwen_clone_fast_mode_checkbox = gr.Checkbox(label="Fast Mode (x_vector_only_mode)", value=False, info="Fast mode: does not require transcription but lower quality")
                        qwen_clone_language_dropdown = gr.Dropdown(label="Language", choices=["Auto", "Chinese", "English", "German", "Italian", "Portuguese", "Spanish", "Japanese", "Korean", "French", "Russian"], value="Auto", interactive=True)
                        qwen_clone_warning = gr.Markdown("**Note:** The Base (0.6B/1.7B) model has not been downloaded...", visible=False)
                    with gr.Group(visible=False) as qwen_design_group:
                        qwen_design_instruct_textbox = gr.Textbox(label="Voice Description (in English)", lines=3, placeholder="Example: A calm middle-aged male announcer...")
                        qwen_design_language_dropdown = gr.Dropdown(label="Language", choices=["Chinese", "English", "German", "Italian", "Portuguese", "Spanish", "Japanese", "Korean", "French", "Russian"], value="Italian")
                        qwen_design_warning = gr.Markdown("**Note:** The VoiceDesign (1.7B) model has not been downloaded...", visible=False)
                    with gr.Accordion("Advanced Settings (for all modes)", open=False):
                        with gr.Row():
                            qwen_speed_slider = gr.Slider(label="Speed", minimum=0.5, maximum=2.0, value=1.0, step=0.1)
                            qwen_pitch_slider = gr.Slider(label="Pitch", minimum=-10, maximum=10, value=0, step=1)
                            qwen_volume_slider = gr.Slider(label="Volume (dB)", minimum=-20, maximum=20, value=0, step=1)
                        with gr.Row():
                            qwen_temperature_slider = gr.Slider(label="Temperature", minimum=0.0, maximum=1.0, value=0.7, step=0.05)
                            qwen_top_p_slider = gr.Slider(label="Top_P", minimum=0.0, maximum=1.0, value=0.8, step=0.05)
                            qwen_top_k_slider = gr.Slider(label="Top_K", minimum=0, maximum=100, value=20, step=1)
                        with gr.Row():
                            qwen_repetition_penalty_slider = gr.Slider(label="Repetition Penalty", minimum=1.0, maximum=2.0, value=1.1, step=0.05)
                            qwen_seed_number = gr.Number(label="Seed (leave empty for random)", value=None)

        # Ritorna tutti i componenti in modo che possano essere referenziati nel file principale
        return (
            model_radio, xtts_voice_file, xtts_params_group, piper_params_group, kokoro_params_group,
            qwen_params_group, vibevoice_params_group, vibevoice_realtime_params_group,
            xtts_lang_dropdown, xtts_temp_slider, xtts_speed_slider, xtts_rep_pen_slider,
            xtts_top_k_slider, xtts_top_p_slider, xtts_length_penalty_slider, xtts_gpt_cond_len_slider,
            piper_speed_slider, piper_noise_scale_slider, piper_noise_scale_w_slider,
            kokoro_lang_dropdown, piper_kokoro_voice_dropdown, kokoro_speed_slider,
            vibevoice_lang_dropdown, vibevoice_temp_slider, vibevoice_cfg_scale_slider, vibevoice_diffusion_steps_slider,
            vibevoice_speed_factor_slider, vibevoice_top_p_slider, vibevoice_top_k_slider, vibevoice_seed_number,
            vibevoice_use_sampling_checkbox, vibevoice_realtime_speaker_dropdown, vibevoice_realtime_cfg_scale_slider,
            vibevoice_realtime_ddpm_steps_slider, vibevoice_realtime_temperature_slider, vibevoice_realtime_top_p_slider,
            vibevoice_realtime_top_k_slider, vibevoice_realtime_seed_number,
            qwen_mode_radio, qwen_custom_group, qwen_clone_group, qwen_design_group,
            qwen_custom_voice_dropdown, qwen_custom_language_dropdown, qwen_custom_instruct_textbox,
            qwen_clone_ref_audio, qwen_clone_ref_text, qwen_clone_fast_mode_checkbox, qwen_clone_language_dropdown,
            qwen_design_instruct_textbox, qwen_design_language_dropdown, qwen_speed_slider,
            qwen_pitch_slider, qwen_volume_slider, qwen_temperature_slider, qwen_top_p_slider,
            qwen_top_k_slider, qwen_repetition_penalty_slider, qwen_seed_number
        )
