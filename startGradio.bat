@echo off
title Audiobook Generator - Starting Graphical Interface
echo ================================================
echo Audiobook Generator - Starting Graphical Interface
echo ================================================
echo.

:: --- Find Virtual Environment ---
:: Supports both .venv\ and venv\ for backward compatibility
set VENV_PATHS=.venv venv
set VENV_ACTIVATE=
set VENV_FOUND=

for %%V in (%VENV_PATHS%) do (
    if exist "%%V\Scripts\activate.bat" (
        set VENV_ACTIVATE=%%V\Scripts\activate.bat
        set VENV_FOUND=%%V
        echo ✓ Found virtual environment at: %%V
        goto :venv_found
    )
)

:venv_found
if "%VENV_ACTIVATE%"=="" (
    echo.
    echo ================================================
    echo ERROR: No virtual environment found
    echo ================================================
    echo.
    echo The Python virtual environment was not found.
    echo.
    echo Searched paths:
    for %%V in (%VENV_PATHS%) do (
        echo   • %%V\Scripts\activate.bat
    )
    echo.
    echo 🔧 Solutions:
    echo   1. Run the installation script first:
    echo        install.bat
    echo.
    echo   2. If already installed, verify the virtual environment exists
    echo        in the current directory.
    echo.
    echo   3. Manually create the virtual environment:
    echo        python -m venv venv
    echo        install.bat
    echo.
    pause
    exit /b 1
)

:: --- Activate Virtual Environment ---
echo Activating virtual environment...
call "%VENV_ACTIVATE%"
if %errorlevel% neq 0 (
    echo.
    echo ================================================
    echo ERROR: Failed to activate virtual environment
    echo ================================================
    echo.
    echo Cannot activate virtual environment at: %VENV_FOUND%
    echo.
    echo 🔧 Solutions:
    echo   1. Recreate the virtual environment:
    echo        rmdir /s /q %VENV_FOUND%
    echo        python -m venv %VENV_FOUND%
    echo        install.bat
    echo.
    echo   2. Verify directory permissions
    echo.
    pause
    exit /b 1
)

echo ✓ Virtual environment activated.
echo.

:: --- Check for Main Script ---
set MAIN_SCRIPT=app_gradio.py
if not exist "%MAIN_SCRIPT%" (
    echo.
    echo ================================================
    echo ERROR: Main script not found
    echo ================================================
    echo.
    echo The file '%MAIN_SCRIPT%' does not exist in the current directory.
    echo.
    echo 🔧 Solutions:
    echo   1. Make sure you are in the correct directory
    echo        cd /d "%~dp0"
    echo.
    echo   2. Verify the project was cloned correctly
    echo.
    pause
    exit /b 1
)

:: --- Check Python Dependencies ---
echo Checking Python dependencies...
python -c "import gradio" >nul 2>&1
if %errorlevel% neq 0 (
    echo.
    echo ⚠️  Warning: Gradio not found in virtual environment
    echo      Reinstall dependencies with: install.bat
    echo.
)

:: --- Run Python GUI Script ---
echo ================================================
echo Starting graphical interface...
echo ================================================
echo.
echo 📢 Instructions:
echo   1. Wait for loading to complete
echo   2. Look for the local URL in the output (e.g.: http://127.0.0.1:7860)
echo   3. Open the URL in your browser
echo   4. Press Ctrl+C to stop the server
echo.
echo Loading in progress...
echo.

python "%MAIN_SCRIPT%"

echo.
echo ================================================
echo Gradio Server Stopped
echo ================================================
pause
exit /b 0
