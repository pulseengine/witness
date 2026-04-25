#!/usr/bin/env bash
# Build every verdict in the suite to wasm32-wasip2.
#
# Each verdict is a standalone crate with its own build.sh; this is the
# orchestrator. CI calls this before running the verdict-suite step in
# the compliance workflow.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

VERDICTS=(
    leap_year
    range_overlap
    triangle
    state_guard
    mixed_or_and
    safety_envelope
    parser_dispatch
)

for v in "${VERDICTS[@]}"; do
    if [ ! -d "$v" ]; then
        echo "warn: verdict directory '$v' does not exist; skipping" >&2
        continue
    fi
    echo "=== building $v ==="
    (cd "$v" && ./build.sh)
done

echo "=== all verdicts built ==="
ls -la "$SCRIPT_DIR"/*/*.wasm 2>/dev/null || echo "no .wasm outputs found"
