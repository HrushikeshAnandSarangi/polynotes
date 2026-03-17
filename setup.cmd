@echo off
setlocal enabledelayedexpansion

cd core

if not exist "whisper.cpp" (
    echo Cloning whisper.cpp...
    git clone https://github.com/ggml-org/whisper.cpp.git
)

cd whisper.cpp\models

echo Downloading whisper English models...
echo.

echo Downloading tiny.en-q5_1 (30 MB) - fastest for English...
call download-ggml-model.cmd tiny.en-q5_1

echo.
echo Downloading base.en-q5_1 (76 MB) - balanced for English...
call download-ggml-model.cmd base.en-q5_1

echo.
echo Downloading whisper Multilingual models...
echo.

echo Downloading tiny-q5_1 (32 MB) - fastest, any language...
call download-ggml-model.cmd tiny-q5_1

echo.
echo Downloading base-q5_1 (60 MB) - balanced, any language...
call download-ggml-model.cmd base-q5_1

echo.
echo ============================================
echo All models downloaded successfully!
echo ============================================
echo.
dir ggml-*.bin 2>nul || echo No model files found
