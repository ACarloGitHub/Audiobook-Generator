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

# appGradioTab/demo_test_tab.py
import gradio as gr

def create_demo_test_tab():
    """Creates and returns the demo and test tab components."""
    with gr.TabItem("4. Demo & Test"):
        demo_text_input = gr.Textbox(label="Text", lines=3)
        demo_generate_button = gr.Button("Generate Demo", variant="secondary")
        demo_status_textbox = gr.Textbox(label="Status")
        demo_audio_output = gr.Audio(label="Output", visible=False)
        test_file_button = gr.Button("Run Test File Generation")
        test_status_textbox = gr.Textbox(label="Test Status", lines=8)
        test_output_audio_player = gr.Audio(label="Test Audio", visible=False)
        
    return (
        demo_text_input, demo_generate_button, demo_status_textbox, demo_audio_output,
        test_file_button, test_status_textbox, test_output_audio_player
    )