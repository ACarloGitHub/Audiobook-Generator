# appGradioTab/generate_tab.py
import gradio as gr

def create_generate_tab(config):
    """Crea e restituisce i componenti della scheda di generazione."""
    with gr.TabItem("3. Generate"):
        with gr.Row():
            select_all_toggle_btn = gr.Button("Select All", variant="secondary", size="sm")
            invert_selection_btn = gr.Button("Invert", variant="secondary", size="sm")
        
        chapter_selector = gr.CheckboxGroup(label="Chapters")
        
        with gr.Row():
            generate_button = gr.Button("Generate Audiobook", variant="primary")
            stop_generation_button = gr.Button("⏹️ Stop", variant="stop", visible=True)
        
        delete_chunks_checkbox = gr.Checkbox(label="Delete intermediate chunks?", value=config.DEFAULT_CLEANUP_CHUNKS)
        status_textbox = gr.Textbox(label="Progress", lines=10)
        output_audio_player = gr.Audio(label="Audio Sample", visible=False)
        output_logfile_display = gr.Textbox(label="Log File Path", visible=False)
        
    return (
        select_all_toggle_btn, invert_selection_btn, chapter_selector, generate_button,
        stop_generation_button, delete_chunks_checkbox, status_textbox,
        output_audio_player, output_logfile_display
    )