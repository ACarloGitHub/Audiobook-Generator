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

# appGradioTab/epub_options_tab.py
import gradio as gr

def create_epub_options_tab(config, SENTENCE_SEPARATOR_OPTIONS, DEFAULT_SEPARATOR_DISPLAY, CHUNKING_STRATEGIES, DEFAULT_CHUNKING_STRATEGY):
    """Create and return the EPUB & Options tab components."""
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
            max_chars_number = gr.Number(label="Max Chars", value=config.DEFAULT_MAX_CHARS_PER_CHUNK, minimum=100, step=50, precision=0)
        
        model_info_note = gr.Markdown(visible=False)
        
    return (
        epub_upload, audiobook_title_textbox, epub_load_notification, replace_guillemets_checkbox,
        separator_dropdown, chunking_strategy_radio, word_count_group, min_words_number,
        max_words_number, char_limit_group, max_chars_number, model_info_note
    )