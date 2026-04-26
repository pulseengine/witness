#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"
TARGET="${TARGET:-wasm32-unknown-unknown}"
cargo build --release --target "$TARGET"
BUILT="target/${TARGET}/release/verdict_json_lite.wasm"
[ -f "$BUILT" ] || { echo "build did not produce $BUILT" >&2; exit 1; }
cp "$BUILT" "$SCRIPT_DIR/verdict_json_lite.wasm"
echo "built: $SCRIPT_DIR/verdict_json_lite.wasm ($(wc -c < "$SCRIPT_DIR/verdict_json_lite.wasm") bytes)"
