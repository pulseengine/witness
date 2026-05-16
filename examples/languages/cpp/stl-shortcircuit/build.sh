#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

WASI_SDK_PATH="${WASI_SDK_PATH:-$HOME/.local/opt/wasi-sdk-33.0-arm64-macos}"
CLANGXX="$WASI_SDK_PATH/bin/clang++"
SYSROOT="$WASI_SDK_PATH/share/wasi-sysroot"
OPT="${OPT:--O1}"  # -O1 lets STL inline through to the lambdas

"$CLANGXX" --sysroot="$SYSROOT" \
    --target=wasm32-wasip1 \
    -std=c++20 \
    -fno-exceptions \
    -g "$OPT" \
    check.cpp -o check.wasm

echo "built: $SCRIPT_DIR/check.wasm ($(wc -c < check.wasm) bytes, opt=$OPT)"
