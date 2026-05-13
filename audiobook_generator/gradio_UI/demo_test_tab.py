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