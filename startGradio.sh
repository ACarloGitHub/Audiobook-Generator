#!/bin/bash
echo "================================================"
echo "Audiobook Generator - Starting Graphical Interface"
echo "================================================"
echo ""

# --- Find Virtual Environment ---
# Supports both .venv/ and venv/ for backward compatibility
VENV_PATHS=(".venv" "venv")
VENV_ACTIVATE=""
VENV_FOUND=""

for VENV_PATH in "${VENV_PATHS[@]}"; do
    ACTIVATE_SCRIPT="$VENV_PATH/bin/activate"
    if [ -f "$ACTIVATE_SCRIPT" ]; then
        VENV_ACTIVATE="$ACTIVATE_SCRIPT"
        VENV_FOUND="$VENV_PATH"
        echo "✓ Virtual environment found at: $VENV_PATH"
        break
    fi
done

if [ -z "$VENV_ACTIVATE" ]; then
    echo ""
    echo "================================================"
    echo "ERROR: No virtual environment found"
    echo "================================================"
    echo ""
    echo "Python virtual environment was not found."
    echo ""
    echo "Searched paths:"
    for VENV_PATH in "${VENV_PATHS[@]}"; do
        echo "  • $VENV_PATH/bin/activate"
    done
    echo ""
    echo "🔧 Solutions:"
    echo "  1. Create the virtual environment:"
    echo "     python3.11 -m venv venv"
    echo "     source venv/bin/activate"
    echo "     pip install -r requirements/requirements.txt"
    echo ""
    echo "  2. If you already installed, verify the virtual environment exists"
    echo "     in the current directory."
    echo ""
    echo "  3. Manually create the virtual environment:"
    echo "     python3.11 -m venv venv"
    echo ""
    exit 1
fi

# --- Activate Virtual Environment ---
echo "Activating virtual environment..."
if ! source "$VENV_ACTIVATE"; then
    echo ""
    echo "================================================"
    echo "ERROR: Failed to activate virtual environment"
    echo "================================================"
    echo ""
    echo "Cannot activate virtual environment at: $VENV_FOUND"
    echo ""
    echo "🔧 Solutions:"
    echo "  1. Recreate the virtual environment:"
    echo "     rm -rf $VENV_FOUND"
    echo "     python3.11 -m venv $VENV_FOUND"
    echo "     ./install.sh"
    echo ""
    echo "  2. Verify directory permissions:"
    echo "     ls -la $VENV_FOUND/"
    echo ""
    exit 1
fi

echo "✓ Virtual environment activated."
echo ""

# --- Check for Main GUI Script ---
MAIN_SCRIPT="app_gradio.py"
if [ ! -f "$MAIN_SCRIPT" ]; then
    echo ""
    echo "================================================"
    echo "ERROR: Main script not found"
    echo "================================================"
    echo ""
    echo "File '$MAIN_SCRIPT' does not exist in the current directory."
    echo ""
    echo "🔧 Solutions:"
    echo "  1. Make sure you are in the correct directory:"
    echo "     pwd"
    echo ""
    echo "  2. Verify the project was cloned correctly"
    echo ""
    exit 1
fi

# --- Check Python Dependencies ---
echo "Checking Python dependencies..."
if ! python -c "import gradio" 2>/dev/null; then
    echo ""
    echo "⚠️  Warning: Gradio not found in virtual environment"
    echo "     Reinstall dependencies with:"
    echo "     pip install -r requirements/requirements.txt"
    echo ""
fi

# --- Run Python GUI Script ---
echo "================================================"
echo "Starting graphical interface..."
echo "================================================"
echo ""
echo "📢 Instructions:"
echo "  1. Wait for loading to complete"
echo "  2. Look for the local URL in the output (e.g.: http://127.0.0.1:7860)"
echo "  3. Open the URL in your browser"
echo "  4. Press Ctrl+C to stop the server"
echo ""
echo "Loading in progress..."

python "$MAIN_SCRIPT"

# Deactivate automatically
deactivate 2>/dev/null || true
echo ""
echo "================================================"
echo "Gradio Server Stopped"
echo "================================================"
exit 0
