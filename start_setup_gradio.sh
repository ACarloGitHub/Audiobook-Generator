#!/bin/bash
# ========================================
# Audiobook Generator - Setup Launcher
# ========================================
# Logic:
# 1. Verify Python 3.11
# 2. Create setup_venv/ if it doesn't exist (or is corrupted)
# 3. Install gradio dependencies (with retry + high timeout for slow lines)
# 4. Launch setup_gradio.py from inside setup_venv
# ========================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# === CONFIG ===
MAX_RETRIES=3
PIP_TIMEOUT=300  # 5 minutes for slow connections
PIP_DELAY=10      # seconds between retries

echo "========================================"
echo "Audiobook Generator - Setup Manager"
echo "========================================"
echo ""

# === Step 1: Verify Python 3.11 ===
PYTHON_CMD=""
if command -v python3.11 &> /dev/null; then
    PYTHON_CMD="python3.11"
elif command -v python3 &> /dev/null; then
    PYTHON_CMD="python3"
elif command -v python &> /dev/null; then
    PYTHON_CMD="python"
else
    echo -e "${RED}❌ ERROR: Python not found!${NC}"
    echo "   Install Python 3.11 or newer."
    exit 1
fi

PYTHON_VERSION=$($PYTHON_CMD -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")')
echo -e "${GREEN}✓${NC} Python found: $($PYTHON_CMD --version)"
echo ""

# === Step 2: Create or fix setup_venv ===
SETUP_VENV_DIR="setup_venv"

venv_python() {
    if [ -f "$SETUP_VENV_DIR/bin/python" ]; then
        echo "$SETUP_VENV_DIR/bin/python"
    elif [ -f "$SETUP_VENV_DIR/Scripts/python.exe" ]; then
        echo "$SETUP_VENV_DIR/Scripts/python.exe"
    else
        echo ""
    fi
}

venv_pip() {
    if [ -f "$SETUP_VENV_DIR/bin/pip" ]; then
        echo "$SETUP_VENV_DIR/bin/pip"
    elif [ -f "$SETUP_VENV_DIR/Scripts/pip.exe" ]; then
        echo "$SETUP_VENV_DIR/Scripts/pip.exe"
    else
        echo ""
    fi
}

cleanup_venv() {
    echo -e "${YELLOW}🗑️  Removing corrupted venv...${NC}"
    rm -rf "$SETUP_VENV_DIR"
    echo -e "${GREEN}✓${NC} Corrupted venv removed."
}

# Check if venv exists and is valid
VENV_PYTHON=$(venv_python)
VENV_PIP=$(venv_pip)

if [ -d "$SETUP_VENV_DIR" ]; then
    if [ -n "$VENV_PYTHON" ] && [ -x "$VENV_PYTHON" ]; then
        # Test if venv works
        if $VENV_PYTHON -c "import sys; sys.exit(0)" 2>/dev/null; then
            echo -e "${GREEN}✓${NC} setup_venv already exists and is valid"
        else
            echo -e "${YELLOW}⚠️${NC} setup_venv found but appears corrupted"
            cleanup_venv
        fi
    else
        echo -e "${YELLOW}⚠️${NC} setup_venv found but Python not executable"
        cleanup_venv
    fi
fi

# Create venv if needed
if [ ! -d "$SETUP_VENV_DIR" ]; then
    echo -e "${YELLOW}🐍 Creating setup_venv...${NC}"
    echo "   Path: $SETUP_VENV_DIR"
    
    if ! $PYTHON_CMD -m venv "$SETUP_VENV_DIR"; then
        echo -e "${RED}❌ ERROR: Failed to create venv${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✓${NC} setup_venv created!"
fi
echo ""

# === Step 3: Install gradio dependencies (with retry + timeout) ===
echo -e "${YELLOW}📦 Installing gradio dependencies...${NC}"
echo "   (Timeout: ${PIP_TIMEOUT}s per attempt, up to ${MAX_RETRIES} retries)"

VENV_PYTHON=$(venv_python)
VENV_PIP=$(venv_pip)

if [ -z "$VENV_PIP" ]; then
    echo -e "${RED}❌ ERROR: pip not found in venv!${NC}"
    cleanup_venv
    exit 1
fi

# Retry loop for pip install
RETRY_COUNT=0
PIP_SUCCESS=0

while [ $RETRY_COUNT -lt $MAX_RETRIES ]; do
    RETRY_COUNT=$((RETRY_COUNT + 1))
    
    echo ""
    echo -e "${YELLOW}   Attempt $RETRY_COUNT of $MAX_RETRIES${NC}"
    
    # Upgrade pip first (with timeout)
    if ! "$VENV_PIP" install --timeout "$PIP_TIMEOUT" --upgrade pip -q 2>&1; then
        echo -e "${RED}   ❌ pip upgrade failed${NC}"
        if [ $RETRY_COUNT -lt $MAX_RETRIES ]; then
            echo -e "${YELLOW}   ⏳ Retrying in ${PIP_DELAY}s...${NC}"
            sleep $PIP_DELAY
        fi
        continue
    fi
    
    # Install wheel
    "$VENV_PIP" install --timeout "$PIP_TIMEOUT" wheel -q 2>&1 || true
    
    # Install gradio and requests (the key dependencies)
    if "$VENV_PIP" install --timeout "$PIP_TIMEOUT" gradio requests -q 2>&1; then
        PIP_SUCCESS=1
        break
    else
        echo -e "${RED}   ❌ Installation failed${NC}"
        if [ $RETRY_COUNT -lt $MAX_RETRIES ]; then
            echo -e "${YELLOW}   ⏳ Retrying in ${PIP_DELAY}s...${NC}"
            sleep $PIP_DELAY
        fi
    fi
done

if [ $PIP_SUCCESS -eq 0 ]; then
    echo ""
    echo -e "${RED}❌ ERROR: Failed to install gradio after ${MAX_RETRIES} attempts${NC}"
    echo ""
    echo "   ⚠️  Your internet connection may be too slow."
    echo "   Try:"
    echo "     1. Run this script again — it will resume from where it left off"
    echo "     2. Increase PIP_TIMEOUT in this script (currently: ${PIP_TIMEOUT}s)"
    echo "     3. Manually: cd setup_venv && bin/pip install gradio requests"
    exit 1
fi

# Verify gradio is installed
if ! "$VENV_PYTHON" -c "import gradio" 2>/dev/null; then
    echo -e "${RED}❌ ERROR: Gradio installation verification failed!${NC}"
    echo "   Try manually: $VENV_PIP install gradio requests"
    exit 1
fi
echo -e "${GREEN}✓${NC} Gradio installed successfully!"
echo ""

# === Step 4: Launch setup_gradio.py from inside setup_venv ===
echo -e "${YELLOW}🚀 Starting Setup Gradio...${NC}"
echo ""

# Create setup_logs if it doesn't exist
mkdir -p setup_logs

LOG_FILE="setup_logs/setup_gradio_$(date +%Y%m%d_%H%M%S).log"

echo "   Log file: $LOG_FILE"
echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  Open http://localhost:7860 in your browser${NC}"
echo -e "${GREEN}  Press CTRL+C to stop${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""

# Launch setup_gradio.py with logging
exec "$VENV_PYTHON" setup/setup_gradio.py 2>&1 | tee "$LOG_FILE"
