#!/usr/bin/env bash
# Run the wasm32-wasip2 witness CLI (built with --no-default-features, no
# `runtime`) under the wasmtime CLI, with the current directory preopened.
# Paths given to witness must be relative to the current directory.
#
# Env: WITNESS_WASM (required) the witness.wasm to run
#      WASMTIME     (default: wasmtime on PATH)
set -euo pipefail
: "${WITNESS_WASM:?set WITNESS_WASM to the wasm32-wasip2 witness.wasm}"
exec "${WASMTIME:-wasmtime}" run --dir=. "$WITNESS_WASM" "$@"
