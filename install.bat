@echo off
setlocal

set "SCRIPT_DIR=%~dp0"
set "PLUGIN=squelchbox.dll"
set "STANDALONE=squelchbox-standalone.exe"

if not exist "%SCRIPT_DIR%%PLUGIN%" (
    echo Error: %PLUGIN% not found in %SCRIPT_DIR%
    echo Make sure install.bat is in the same folder as the built binaries.
    exit /b 1
)

echo Installing SquelchBox plugins...

:: VST3
set "VST3_DIR=%CommonProgramFiles%\VST3\squelchbox.vst3\Contents\x86_64-win"
if not exist "%VST3_DIR%" mkdir "%VST3_DIR%"
copy /y "%SCRIPT_DIR%%PLUGIN%" "%VST3_DIR%\squelchbox.vst3" >nul
echo   VST3 -^> %VST3_DIR%\squelchbox.vst3

:: CLAP
set "CLAP_DIR=%CommonProgramFiles%\CLAP"
if not exist "%CLAP_DIR%" mkdir "%CLAP_DIR%"
copy /y "%SCRIPT_DIR%%PLUGIN%" "%CLAP_DIR%\squelchbox.clap" >nul
echo   CLAP -^> %CLAP_DIR%\squelchbox.clap

:: Standalone
if exist "%SCRIPT_DIR%%STANDALONE%" (
    set "BIN_DIR=%LocalAppData%\SquelchBox"
    if not exist "%BIN_DIR%" mkdir "%BIN_DIR%"
    copy /y "%SCRIPT_DIR%%STANDALONE%" "%BIN_DIR%\squelchbox-standalone.exe" >nul
    echo   Standalone   -^> %BIN_DIR%\squelchbox-standalone.exe
    (
        echo @echo off
        echo REM Launcher with safe WASAPI buffer size. Do not run squelchbox-standalone.exe directly.
        echo set "DIR=%%~dp0"
        echo "%%DIR%%squelchbox-standalone.exe" --period-size 2048 %%*
    ) > "%BIN_DIR%\SquelchBox.bat"
    echo   Launcher     -^> %BIN_DIR%\SquelchBox.bat
)

echo.
echo Done! Rescan plugins in your DAW to find SquelchBox.
echo Note: Windows may require running this as Administrator for the VST3/CLAP paths.
echo Note: To run the standalone app, use SquelchBox.bat (not squelchbox-standalone.exe directly).
echo       The launcher passes --period-size 2048 to avoid a WASAPI buffer size mismatch.
pause
