#!/bin/bash
set -e

cd core

# Clone whisper.cpp if not already present
if [ ! -d "whisper.cpp" ]; then
    git clone https://github.com/ggml-org/whisper.cpp.git
fi

cd whisper.cpp/models

# Download English-only quantized models
echo "Downloading whisper English models..."

bash download-ggml-model.sh tiny.en-q5_1
bash download-ggml-model.sh base.en-q5_1

# Download Multilingual quantized models
echo ""
echo "Downloading whisper Multilingual models..."

bash download-ggml-model.sh tiny-q5_1
bash download-ggml-model.sh base-q5_1

echo ""
echo "All models downloaded successfully!"
echo ""
echo "Available models:"
ls -lh ggml-*.bin 2>/dev/null || echo "No model files found"
