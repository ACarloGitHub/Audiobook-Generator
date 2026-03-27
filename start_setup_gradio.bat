@echo off
chcp 65001 >nul
setlocal enabledelayedexpansion

echo ========================================
echo Audiobook Generator - Setup Manager
echo ========================================
echo.

:: Get script directory
set "SCRIPT_DIR=%~dp0"
set "SCRIPT_DIR=%SCRIPT_DIR:~0,-1%"
set "SETUP_VENV_DIR=%SCRIPT_DIR%\setup_venv"

:: === CONFIG ===
set "MAX_RETRIES=3"
set "PIP_TIMEOUT=300"
set "PIP_DELAY=10"

:: === Step 1: Verify Python ===
python --version >nul 2>&1
if errorlevel 1 (
    py --version >nul 2>&1
    if errorlevel 1 (
        echo ERROR: Python not found.
        echo Download: https://www.python.org/downloads/
        pause
        exit /b 1
    )
    set "PYTHON_CMD=py"
) else (
    set "PYTHON_CMD=python"
)

for /f "delims=" %%v in ('%PYTHON_CMD% --version 2^>^&1') do echo Python found: %%v
echo.

:: === Step 2: Check and create setup_venv ===
set "VENV_PYTHON=%SETUP_VENV_DIR%\Scripts\python.exe"
set "VENV_PIP=%SETUP_VENV_DIR%\Scripts\pip.exe"

:: Cleanup function
goto :main

:cleanup_venv
echo.
echo Removing corrupted venv...
if exist "%SETUP_VENV_DIR%" (
    rmdir /s /q "%SETUP_VENV_DIR%"
    echo Done.
)
echo.
goto :eof

:main

:: Check if venv exists
if exist "%SETUP_VENV_DIR%" (
    :: Test if venv is valid
    if exist "%VENV_PYTHON%" (
        "%VENV_PYTHON%" -c "import sys; sys.exit(0)" >nul 2>&1
        if not errorlevel 1 (
            echo setup_venv already exists and is valid.
        ) else (
            echo setup_venv found but appears corrupted.
            call :cleanup_venv
        )
    ) else (
        echo setup_venv found but Python not executable.
        call :cleanup_venv
    )
) else (
    :: Create new venv
    echo Creating setup_venv...
    %PYTHON_CMD% -m venv "%SETUP_VENV_DIR%"
    if errorlevel 1 (
        echo ERROR: Failed to create venv.
        pause
        exit /b 1
    )
    echo setup_venv created!
)
echo.

:: === Step 3: Install gradio and requests (with retry + timeout) ===
echo Installing gradio and requests...
echo (Timeout: %PIP_TIMEOUT%s per attempt, up to %MAX_RETRIES% retries)
echo.

set "RETRY_COUNT=0"
set "PIP_SUCCESS=0"

:retry_install
set /a RETRY_COUNT+=1
echo   Attempt %RETRY_COUNT% of %MAX_RETRIES%

:: Upgrade pip first
"%VENV_PIP%" install --timeout %PIP_TIMEOUT% --upgrade pip >nul 2>&1
if errorlevel 1 (
    echo   pip upgrade failed
    goto :retry_check
)

:: Install wheel
"%VENV_PIP%" install --timeout %PIP_TIMEOUT% wheel >nul 2>&1

:: Install gradio and requests
"%VENV_PIP%" install --timeout %PIP_TIMEOUT% gradio requests >nul 2>&1
if not errorlevel 1 (
    set "PIP_SUCCESS=1"
    goto :install_done
)

:retry_check
if "%PIP_SUCCESS%"=="0" (
    if %RETRY_COUNT% lss %MAX_RETRIES% (
        echo   Retrying in %PIP_DELAY%s...
        ping -n %PIP_DELAY% 127.0.0.1 >nul 2>&1
        goto :retry_install
    )
)

:install_done

if "%PIP_SUCCESS%"=="0" (
    echo.
    echo ERROR: Failed to install gradio after %MAX_RETRIES% attempts.
    echo.
    echo   Your internet connection may be too slow.
    echo   Try:
    echo     1. Run this script again - it will resume from where it left off
    echo     2. Increase PIP_TIMEOUT in this script (currently: %PIP_TIMEOUT%s)
    echo     3. Manually: cd setup_venv ^&^& Scripts\pip install gradio requests
    pause
    exit /b 1
)

:: Verify installation
"%VENV_PYTHON%" -c "import gradio" 2>nul
if errorlevel 1 (
    echo.
    echo ERROR: Gradio installation verification failed.
    pause
    exit /b 1
)
echo Gradio installed successfully!
echo.

:: === Step 4: Launch setup_gradio.py ===
echo ========================================
echo Starting Setup Gradio...
echo ========================================
echo.
echo Open http://localhost:7860 in your browser
echo Press CTRL+C to stop the server
echo.

"%VENV_PYTHON%" "%SCRIPT_DIR%\setup\setup_gradio.py"

pause
