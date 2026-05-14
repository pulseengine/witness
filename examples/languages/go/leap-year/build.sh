#!/usr/bin/env bash
# Build the TinyGo leap-year fixture to wasm with DWARF.
#
# Requires TinyGo 0.31+. Outputs leap.wasm.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

TINYGO="${TINYGO:-tinygo}"

# `wasm-unknown` is TinyGo's minimal wasm target — no wasi runtime,
# tiny binaries. `-opt 1` is the lightest optimisation level that
# still inlines and runs the LLVM optimiser; matches rustc -O1 and
# clang -O1 across the fixture set.
"$TINYGO" build -target wasm-unknown -opt 1 -o leap.wasm leap.go

echo "built: $SCRIPT_DIR/leap.wasm ($(wc -c < leap.wasm) bytes)"
