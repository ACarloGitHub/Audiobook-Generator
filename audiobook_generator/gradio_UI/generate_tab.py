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

# appGradioTab/generate_tab.py
import gradio as gr

def create_generate_tab(config):
    """Creates and returns the generation tab components."""
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