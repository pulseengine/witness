#!/usr/bin/env bash
# Build the witness end-to-end test fixture.
#
# Usage:
#   ./build.sh              # build for wasm32-unknown-unknown (default)
#   TARGET=wasm32-wasip1 ./build.sh
#
# Output: a single .wasm file copied to a stable path the integration test
# can find:
#
#   tests/fixtures/sample-rust-crate/sample.wasm
#
# CI calls this before `cargo test --test integration_e2e`. Locally, run
# it once after editing src/lib.rs.
#
# This script does NOT commit the produced .wasm. It is listed in
# tests/fixtures/.gitignore.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

TARGET="${TARGET:-wasm32-unknown-unknown}"
PROFILE="${PROFILE:-release}"

# Verify the target is installed. If not, point the user at `rustup target
# add ...` rather than letting `cargo build` produce a less helpful error.
if ! rustup target list --installed | grep -q "^${TARGET}$"; then
    echo "error: rustup target '${TARGET}' is not installed." >&2
    echo "  hint: rustup target add ${TARGET}" >&2
    exit 1
fi

if [ "$PROFILE" = "release" ]; then
    cargo build --target "$TARGET" --release
    BUILT="target/${TARGET}/release/witness_sample_fixture.wasm"
else
    cargo build --target "$TARGET"
    BUILT="target/${TARGET}/debug/witness_sample_fixture.wasm"
fi

if [ ! -f "$BUILT" ]; then
    echo "error: expected build output at $BUILT but file does not exist." >&2
    echo "  cargo may have placed it elsewhere; inspect the cargo output above." >&2
    exit 1
fi

# Copy to a stable name the integration test reads.
cp "$BUILT" "$SCRIPT_DIR/sample.wasm"
echo "built fixture: $SCRIPT_DIR/sample.wasm ($(wc -c < "$SCRIPT_DIR/sample.wasm") bytes, target=${TARGET}, profile=${PROFILE})"
