#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

WASI_SDK_PATH="${WASI_SDK_PATH:-$HOME/.local/opt/wasi-sdk-33.0-arm64-macos}"
CLANGXX="$WASI_SDK_PATH/bin/clang++"
SYSROOT="$WASI_SDK_PATH/share/wasi-sysroot"
OPT="${OPT:--O0}"

"$CLANGXX" --sysroot="$SYSROOT" \
    --target=wasm32-wasip1 \
    -std=c++20 \
    -g "$OPT" \
    shapes.cpp -o shapes.wasm

echo "built: $SCRIPT_DIR/shapes.wasm ($(wc -c < shapes.wasm) bytes, opt=$OPT)"
