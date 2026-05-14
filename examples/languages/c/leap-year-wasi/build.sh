#!/usr/bin/env bash
# Build the wasi-sdk C leap-year fixture.
#
# Requires wasi-sdk installed. Default install location matches the
# v0.18 cross-language probe's recommendation:
#   ~/.local/opt/wasi-sdk-33.0-arm64-macos
# Override with WASI_SDK_PATH to point elsewhere.
#
# Outputs leap.wasm in the script's directory.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

WASI_SDK_PATH="${WASI_SDK_PATH:-$HOME/.local/opt/wasi-sdk-33.0-arm64-macos}"
if [ ! -d "$WASI_SDK_PATH" ]; then
    echo "wasi-sdk not found at $WASI_SDK_PATH" >&2
    echo "download from https://github.com/WebAssembly/wasi-sdk/releases" >&2
    exit 1
fi

CLANG="$WASI_SDK_PATH/bin/clang"
SYSROOT="$WASI_SDK_PATH/share/wasi-sysroot"

"$CLANG" --sysroot="$SYSROOT" \
    --target=wasm32-wasip1 \
    -g -O1 \
    leap.c -o leap.wasm

echo "built: $SCRIPT_DIR/leap.wasm ($(wc -c < leap.wasm) bytes)"
