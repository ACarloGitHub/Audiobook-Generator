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

"""
Modulo per la gestione delle dipendenze esterne (FFmpeg/SoX) nell'interfaccia Gradio.
Questo modulo è progettato per essere importato in app_gradio.py per mantenere il codice modulare.
"""

import gradio as gr
import os
import sys
import subprocess
import shutil
import time
from typing import Dict, Any, Tuple

# Importa configurazioni dal progetto
try:
    from audiobook_generator import config
except ImportError:
    # Fallback per importazioni dirette
    import sys
    sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    from audiobook_generator import config

# Importa funzioni di setup
try:
    from setup import setup_helpers
    HAS_SETUP_HELPERS = True
except ImportError:
    HAS_SETUP_HELPERS = False


def check_external_dependencies() -> Dict[str, Any]:
    """Verifica la presenza di FFmpeg e SoX nel sistema."""
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
    
    # Verifica FFmpeg
    ffmpeg_path = shutil.which("ffmpeg")
    if ffmpeg_path:
        dependencies_status["ffmpeg"]["present"] = True
        dependencies_status["ffmpeg"]["path"] = ffmpeg_path
        dependencies_status["ffmpeg"]["message"] = "✅ FFmpeg trovato nel PATH di sistema"
        # Prova a ottenere versione
        try:
            result = subprocess.run([ffmpeg_path, "-version"], capture_output=True, text=True, timeout=5)
            if result.returncode == 0:
                first_line = result.stdout.split('\n')[0] if result.stdout else ""
                dependencies_status["ffmpeg"]["version"] = first_line[:50]  # Limita lunghezza
        except:
            pass
    else:
        # Controlla se FFmpeg è nella cartella locale del progetto
        local_ffmpeg = config.DEFAULT_FFMPEG_EXE
        if os.path.exists(local_ffmpeg):
            dependencies_status["ffmpeg"]["present"] = True
            dependencies_status["ffmpeg"]["path"] = local_ffmpeg
            dependencies_status["ffmpeg"]["message"] = "✅ FFmpeg trovato nella cartella locale del progetto"
        else:
            dependencies_status["ffmpeg"]["message"] = "❌ FFmpeg not found. The app will use an alternative (slower) method to merge audio files."
    
    # Verifica SoX
    sox_path = shutil.which("sox")
    if sox_path:
        dependencies_status["sox"]["present"] = True
        dependencies_status["sox"]["path"] = sox_path
        dependencies_status["sox"]["message"] = "✅ SoX trovato nel PATH di sistema"
    else:
        # Controlla se SoX è nella cartella locale del progetto
        local_sox = os.path.join(os.getcwd(), "sox", "bin", "sox.exe" if os.name == 'nt' else "sox")
        if os.path.exists(local_sox):
            dependencies_status["sox"]["present"] = True
            dependencies_status["sox"]["path"] = local_sox
            dependencies_status["sox"]["message"] = "✅ SoX trovato nella cartella locale del progetto"
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
        # Esegui la funzione di setup
        success = setup_helpers.setup_ffmpeg()
        if success:
            return "✅ FFmpeg installato con successo!", True
        else:
            return "⚠️ Installazione FFmpeg completata con avvisi. Verifica manualmente.", False
    except Exception as e:
        return f"❌ Errore durante l'installazione di FFmpeg: {str(e)}", False


def install_sox_wrapper() -> Tuple[str, bool]:
    """Wrapper for SoX installation via setup/helpers.py."""
    if not HAS_SETUP_HELPERS:
        return "❌ Cannot install SoX: setup/helpers.py module not found.", False
    
    try:
        # Esegui la funzione di setup
        success = setup_helpers.setup_sox()
        if success:
            return "✅ SoX installato con successo!", True
        else:
            return "⚠️ Installazione SoX completata con avvisi. Verifica manualmente.", False
    except Exception as e:
        return f"❌ Errore durante l'installazione di SoX: {str(e)}", False


def refresh_dependencies_status() -> Tuple[str, str, str]:
    """Aggiorna lo stato delle dipendenze e restituisce messaggi aggiornati."""
    deps = check_external_dependencies()
    
    # Crea messaggio di stato
    status_message = get_dependencies_status_message()
    
    # Determina stato per FFmpeg e SoX (per colore pulsanti)
    ffmpeg_status = "✅ Presente" if deps["ffmpeg"]["present"] else "❌ Mancante"
    sox_status = "✅ Presente" if deps["sox"]["present"] else "⚠️ Mancante"
    
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
            # Aggiorna stato dopo installazione
            status_msg, ffmpeg_status, sox_status = refresh_dependencies_status()
            return message, status_msg, ffmpeg_status, sox_status
        
        def on_install_sox():
            message, success = install_sox_wrapper()
            # Aggiorna stato dopo installazione
            status_msg, ffmpeg_status, sox_status = refresh_dependencies_status()
            return message, status_msg, ffmpeg_status, sox_status
        
        def on_refresh():
            status_msg, ffmpeg_status, sox_status = refresh_dependencies_status()
            return status_msg, ffmpeg_status, sox_status, ""
        
        # Collegamenti eventi
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
    # Test del modulo
    print("Test modulo dependencies_tab.py")
    print("=" * 50)
    
    deps = check_external_dependencies()
    print(f"FFmpeg: {deps['ffmpeg']['message']}")
    print(f"SoX: {deps['sox']['message']}")
    
    print("\nMessaggio di stato:")
    print(get_dependencies_status_message())