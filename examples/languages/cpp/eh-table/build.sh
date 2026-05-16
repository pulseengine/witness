#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

WASI_SDK_PATH="${WASI_SDK_PATH:-$HOME/.local/opt/wasi-sdk-33.0-arm64-macos}"
CLANGXX="$WASI_SDK_PATH/bin/clang++"
SYSROOT="$WASI_SDK_PATH/share/wasi-sysroot"
OPT="${OPT:--O0}"

# wasi-sdk's default libcxx is built without C++ exceptions
# (`__cxa_throw` is undefined). Try the wasm-EH proposal first
# (`-fwasm-exceptions`) — produces native `try`/`catch_all`
# wasm instructions, which is what we want for the br_table
# audit anyway. If that fails (libcxx doesn't have the right
# unwind shims), fall back to the no-exceptions build that
# still exercises the switch table without unwinding.
if "$CLANGXX" --sysroot="$SYSROOT" \
    --target=wasm32-wasip1 \
    -std=c++20 \
    -fwasm-exceptions \
    -mllvm -wasm-enable-eh \
    -g "$OPT" \
    parser.cpp -o parser.wasm 2>/dev/null; then
    echo "built with -fwasm-exceptions"
else
    echo "wasm-EH not supported by this wasi-sdk libcxx; falling back to -fno-exceptions" >&2
    "$CLANGXX" --sysroot="$SYSROOT" \
        --target=wasm32-wasip1 \
        -std=c++20 \
        -fno-exceptions \
        -DPARSER_NO_EH \
        -g "$OPT" \
        parser.cpp -o parser.wasm
fi

echo "built: $SCRIPT_DIR/parser.wasm ($(wc -c < parser.wasm) bytes, opt=$OPT)"
