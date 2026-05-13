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

"""
Module for managing external dependencies (FFmpeg/SoX) in the Gradio interface.
This module is designed to be imported in app_gradio.py to keep the code modular.
"""

import gradio as gr
import logging
import os
import sys
import subprocess
import shutil
import time
from typing import Dict, Any, Tuple

logger = logging.getLogger(__name__)

# Import project configurations
try:
    from audiobook_generator import config
except ImportError:
    # Fallback for direct imports
    import sys
    sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    from audiobook_generator import config

# Import setup functions
try:
    from setup import setup_helpers
    HAS_SETUP_HELPERS = True
except ImportError:
    HAS_SETUP_HELPERS = False


def check_external_dependencies() -> Dict[str, Any]:
    """Check for FFmpeg and SoX availability on the system."""
    dependencies_status = {
        "ffmpeg": {
            "present": False,
            "path": None,
            "message": "",
            "version": None
        },
        "sox": {
            "present": False,
            "path": None,
            "message": "",
            "version": None
        }
    }
    
    # Check FFmpeg
    ffmpeg_path = shutil.which("ffmpeg")
    if ffmpeg_path:
        dependencies_status["ffmpeg"]["present"] = True
        dependencies_status["ffmpeg"]["path"] = ffmpeg_path
        dependencies_status["ffmpeg"]["message"] = "✅ FFmpeg found in system PATH"
        # Try to get version
        try:
            result = subprocess.run([ffmpeg_path, "-version"], capture_output=True, text=True, timeout=5)
            if result.returncode == 0:
                first_line = result.stdout.split('\n')[0] if result.stdout else ""
                dependencies_status["ffmpeg"]["version"] = first_line[:50]
        except Exception:
            pass
    else:
        # Check if FFmpeg is in the local project folder
        local_ffmpeg = config.DEFAULT_FFMPEG_EXE
        if os.path.exists(local_ffmpeg):
            dependencies_status["ffmpeg"]["present"] = True
            dependencies_status["ffmpeg"]["path"] = local_ffmpeg
            dependencies_status["ffmpeg"]["message"] = "✅ FFmpeg found in local project folder"
        else:
            dependencies_status["ffmpeg"]["message"] = "❌ FFmpeg not found. The app will use an alternative (slower) method to merge audio files."
    
    # Check SoX
    sox_path = shutil.which("sox")
    if sox_path:
        dependencies_status["sox"]["present"] = True
        dependencies_status["sox"]["path"] = sox_path
        dependencies_status["sox"]["message"] = "✅ SoX found in system PATH"
    else:
        # Check if SoX is in the local project folder
        local_sox = os.path.join(os.getcwd(), "sox", "bin", "sox.exe" if os.name == 'nt' else "sox")
        if os.path.exists(local_sox):
            dependencies_status["sox"]["present"] = True
            dependencies_status["sox"]["path"] = local_sox
            dependencies_status["sox"]["message"] = "✅ SoX found in local project folder"
        else:
            dependencies_status["sox"]["message"] = "⚠️ SoX not found. Some audio features may not be available."
    
    return dependencies_status


def get_dependencies_status_message() -> str:
    """Returns a human-readable status message for the user."""
    deps = check_external_dependencies()
    
    messages = []
    messages.append("### 📦 System Dependencies Status")
    messages.append("")
    
    # FFmpeg
    ffmpeg_msg = deps["ffmpeg"]["message"]
    if deps["ffmpeg"]["version"]:
        ffmpeg_msg += f" ({deps['ffmpeg']['version']})"
    messages.append(f"- **FFmpeg**: {ffmpeg_msg}")
    
    # SoX
    messages.append(f"- **SoX**: {deps['sox']['message']}")
    
    messages.append("")
    messages.append("💡 **Note**: If FFmpeg is not available, the app will use an alternative (slower) method to merge audio files.")
    
    return "\n".join(messages)


def install_ffmpeg_wrapper() -> Tuple[str, bool]:
    """Wrapper for FFmpeg installation via setup/helpers.py."""
    if not HAS_SETUP_HELPERS:
        return "❌ Cannot install FFmpeg: setup/helpers.py module not found.", False
    
    try:
        # Run setup function
        success = setup_helpers.setup_ffmpeg()
        if success:
            return "✅ FFmpeg installed successfully!", True
        else:
            return "⚠️ FFmpeg installation completed with warnings. Verify manually.", False
    except Exception as e:
        return f"❌ Error during FFmpeg installation: {str(e)}", False


def install_sox_wrapper() -> Tuple[str, bool]:
    """Wrapper for SoX installation via setup/helpers.py."""
    if not HAS_SETUP_HELPERS:
        return "❌ Cannot install SoX: setup/helpers.py module not found.", False
    
    try:
        # Run setup function
        success = setup_helpers.setup_sox()
        if success:
            return "✅ SoX installed successfully!", True
        else:
            return "⚠️ SoX installation completed with warnings. Verify manually.", False
    except Exception as e:
        return f"❌ Error during SoX installation: {str(e)}", False


def refresh_dependencies_status() -> Tuple[str, str, str]:
    """Refresh dependency status and return updated messages."""
    deps = check_external_dependencies()
    
    # Create status message
    status_message = get_dependencies_status_message()
    
    # Determine status for FFmpeg and SoX (for button colors)
    ffmpeg_status = "✅ Present" if deps["ffmpeg"]["present"] else "❌ Missing"
    sox_status = "✅ Present" if deps["sox"]["present"] else "⚠️ Missing"
    
    return status_message, ffmpeg_status, sox_status


def create_dependencies_tab() -> gr.TabItem:
    """Creates the System Dependencies tab."""
    with gr.TabItem("6. System Dependencies") as tab:
        gr.Markdown("## 🛠️ System Dependencies Management")
        gr.Markdown("This panel allows you to verify and install the external dependencies required for optimal application operation.")
        
        # Initial state
        initial_status = get_dependencies_status_message()
        deps = check_external_dependencies()
        ffmpeg_initial = "✅ Present" if deps["ffmpeg"]["present"] else "❌ Missing"
        sox_initial = "✅ Present" if deps["sox"]["present"] else "⚠️ Missing"
        
        # UI Components
        status_display = gr.Markdown(value=initial_status, label="Current Status")
        
        with gr.Row():
            ffmpeg_status_display = gr.Textbox(value=ffmpeg_initial, label="FFmpeg Status", interactive=False)
            sox_status_display = gr.Textbox(value=sox_initial, label="SoX Status", interactive=False)
        
        with gr.Row():
            install_ffmpeg_btn = gr.Button("📥 Install/Reinstall FFmpeg", variant="secondary")
            install_sox_btn = gr.Button("📥 Install/Reinstall SoX", variant="secondary")
            refresh_btn = gr.Button("🔄 Update Status", variant="secondary")
        
        install_log = gr.Textbox(label="Installation Log", lines=5, interactive=False, placeholder="Installation details will appear here...")
        
        gr.Markdown("### 📚 Manual Instructions")
        with gr.Accordion("Manual Installation Instructions", open=False):
            gr.Markdown("""
            #### FFmpeg
            - **Windows**: Download from [ffmpeg.org](https://ffmpeg.org/download.html) and add to PATH
            - **Linux**: `sudo apt-get install ffmpeg` (Debian/Ubuntu) or `sudo dnf install ffmpeg` (Fedora)
            - **macOS**: `brew install ffmpeg`
            
            #### SoX
            - **Windows**: Download from [sourceforge.net/projects/sox](https://sourceforge.net/projects/sox/) and add to PATH
            - **Linux**: `sudo apt-get install sox` (Debian/Ubuntu) or `sudo dnf install sox` (Fedora)
            - **macOS**: `brew install sox`
            
            #### Verify Installation
            After installation, restart the application or update the status with the button above.
            """)
        
        # Event handlers
        def on_install_ffmpeg():
            message, success = install_ffmpeg_wrapper()
            # Update status after installation
            status_msg, ffmpeg_status, sox_status = refresh_dependencies_status()
            return message, status_msg, ffmpeg_status, sox_status
        
        def on_install_sox():
            message, success = install_sox_wrapper()
            # Update status after installation
            status_msg, ffmpeg_status, sox_status = refresh_dependencies_status()
            return message, status_msg, ffmpeg_status, sox_status
        
        def on_refresh():
            status_msg, ffmpeg_status, sox_status = refresh_dependencies_status()
            return status_msg, ffmpeg_status, sox_status, ""
        
        # Event bindings
        install_ffmpeg_btn.click(
            fn=on_install_ffmpeg,
            outputs=[install_log, status_display, ffmpeg_status_display, sox_status_display]
        )
        
        install_sox_btn.click(
            fn=on_install_sox,
            outputs=[install_log, status_display, ffmpeg_status_display, sox_status_display]
        )
        
        refresh_btn.click(
            fn=on_refresh,
            outputs=[status_display, ffmpeg_status_display, sox_status_display, install_log]
        )
    
    return tab


if __name__ == "__main__":
    # Module test
    print("Testing dependencies_tab.py module")
    print("=" * 50)
    
    deps = check_external_dependencies()
    print(f"FFmpeg: {deps['ffmpeg']['message']}")
    print(f"SoX: {deps['sox']['message']}")
    
    print("\nStatus message:")
    print(get_dependencies_status_message())