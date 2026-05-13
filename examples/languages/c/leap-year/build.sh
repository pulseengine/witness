#!/usr/bin/env bash
# Build the C leap-year fixture to wasm32 with DWARF debug info.
#
# Requires LLVM 14+ with wasm32 backend and wasm-ld. On macOS via
# homebrew: `brew install llvm lld` and pass CLANG / LD overrides
# if your system clang doesn't have the wasm32 target enabled.
#
# Outputs leap.wasm in the script's directory.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

CLANG="${CLANG:-clang}"
WASM_LD="${WASM_LD:-wasm-ld}"

# On macOS with homebrew LLVM, the default clang on PATH may be
# Apple clang which doesn't include the wasm32 backend. Detect
# and route via homebrew if available.
if [ "$CLANG" = "clang" ] && [ -x /opt/homebrew/opt/llvm/bin/clang ]; then
    CLANG=/opt/homebrew/opt/llvm/bin/clang
    WASM_LD=/opt/homebrew/opt/lld/bin/wasm-ld
fi

"$CLANG" --target=wasm32-unknown-unknown \
    -nostdlib \
    -g -O1 \
    -fuse-ld="$WASM_LD" \
    -Wl,--no-entry \
    -Wl,--export-dynamic \
    -Wl,--allow-undefined \
    leap.c -o leap.wasm

echo "built: $SCRIPT_DIR/leap.wasm ($(wc -c < leap.wasm) bytes)"
