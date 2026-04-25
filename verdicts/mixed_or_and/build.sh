#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"
TARGET="${TARGET:-wasm32-wasip2}"
cargo build --release --target "$TARGET"
BUILT="target/${TARGET}/release/verdict_mixed_or_and.wasm"
[ -f "$BUILT" ] || { echo "build did not produce $BUILT" >&2; exit 1; }
cp "$BUILT" "$SCRIPT_DIR/verdict_mixed_or_and.wasm"
echo "built: $SCRIPT_DIR/verdict_mixed_or_and.wasm ($(wc -c < "$SCRIPT_DIR/verdict_mixed_or_and.wasm") bytes)"
