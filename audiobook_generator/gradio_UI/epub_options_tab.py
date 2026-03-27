# appGradioTab/epub_options_tab.py
import gradio as gr

def create_epub_options_tab(config, SENTENCE_SEPARATOR_OPTIONS, DEFAULT_SEPARATOR_DISPLAY, CHUNKING_STRATEGIES, DEFAULT_CHUNKING_STRATEGY):
    """Crea e restituisce i componenti della scheda EPUB e opzioni."""
    with gr.TabItem("2. EPUB & Options"):
        epub_upload = gr.File(label="Upload EPUB", file_types=[".epub"])
        audiobook_title_textbox = gr.Textbox(label="Audiobook Title", placeholder="Enter title or leave blank")
        epub_load_notification = gr.Markdown(visible=False)
        replace_guillemets_checkbox = gr.Checkbox(label="Replace Guillemets (« »)", value=config.DEFAULT_REPLACE_GUILLEMETS)
        separator_dropdown = gr.Dropdown(label="Sentence Separator", choices=[o[0] for o in SENTENCE_SEPARATOR_OPTIONS], value=DEFAULT_SEPARATOR_DISPLAY)
        chunking_strategy_radio = gr.Radio(label="Chunking Strategy", choices=CHUNKING_STRATEGIES, value=DEFAULT_CHUNKING_STRATEGY)
        with gr.Group(visible=(DEFAULT_CHUNKING_STRATEGY == CHUNKING_STRATEGIES[0])) as word_count_group:
            min_words_number = gr.Number(label="Min Words", value=config.DEFAULT_MIN_WORDS_APPROX, minimum=10, step=10, precision=0)
            max_words_number = gr.Number(label="Max Words", value=config.DEFAULT_MAX_WORDS_APPROX, minimum=50, step=10, precision=0)
        with gr.Group(visible=(DEFAULT_CHUNKING_STRATEGY == CHUNKING_STRATEGIES[1])) as char_limit_group:
            max_chars_number = gr.Number(label="Max Chars", value=config.TTS_MODEL_CONFIG.get("XTTSv2", {}).get("char_limit_recommended", 300), minimum=100, step=50, precision=0)
        
        model_info_note = gr.Markdown(visible=False)
        
    return (
        epub_upload, audiobook_title_textbox, epub_load_notification, replace_guillemets_checkbox,
        separator_dropdown, chunking_strategy_radio, word_count_group, min_words_number,
        max_words_number, char_limit_group, max_chars_number, model_info_note
    )