#!/usr/bin/env bash
# Build the C++ leap-year fixture via wasi-sdk clang++.
#
# Defaults to -O0 because that's the level where wasi-sdk's
# wasm-ld preserves DWARF line program addresses. -O1+ hits the
# same upstream wasm-ld DWARF gap as the C wasi-sdk fixture.
# Override with OPT=-O1 to reproduce.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

WASI_SDK_PATH="${WASI_SDK_PATH:-$HOME/.local/opt/wasi-sdk-33.0-arm64-macos}"
if [ ! -d "$WASI_SDK_PATH" ]; then
    echo "wasi-sdk not found at $WASI_SDK_PATH" >&2
    exit 1
fi

CLANGXX="$WASI_SDK_PATH/bin/clang++"
SYSROOT="$WASI_SDK_PATH/share/wasi-sysroot"
OPT="${OPT:--O0}"

"$CLANGXX" --sysroot="$SYSROOT" \
    --target=wasm32-wasip1 \
    -std=c++20 \
    -g "$OPT" \
    leap.cpp -o leap.wasm

echo "built: $SCRIPT_DIR/leap.wasm ($(wc -c < leap.wasm) bytes, opt=$OPT)"
