#!/usr/bin/env bash
# Build the Zig leap-year fixture to wasm32-freestanding.
#
# Requires Zig 0.13+ in PATH. Outputs leap.wasm.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

ZIG="${ZIG:-zig}"

"$ZIG" build-exe \
    -target wasm32-freestanding \
    -fno-entry \
    -rdynamic \
    -OReleaseSafe \
    leap.zig

# zig build-exe writes to leap.wasm by default.
[ -f leap.wasm ] || { echo "zig build did not produce leap.wasm" >&2; exit 1; }
echo "built: $SCRIPT_DIR/leap.wasm ($(wc -c < leap.wasm) bytes)"
