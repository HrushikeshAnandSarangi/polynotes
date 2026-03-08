#!/bin/bash
set -e

cd core

# Clone whisper.cpp if not already present
if [ ! -d "whisper.cpp" ]; then
    git clone https://github.com/ggml-org/whisper.cpp.git
fi

cd whisper.cpp
bash models/download-ggml-model.sh base.q5_1

