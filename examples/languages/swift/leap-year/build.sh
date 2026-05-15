#!/usr/bin/env bash
# Build the Swift leap-year fixture via SwiftWasm SDK.
#
# Requires:
#   - Apple Swift 6.x toolchain (built-in on macOS)
#   - SwiftWasm 6.3 SDK installed via:
#       swift sdk install \
#         https://github.com/swiftwasm/swift/releases/download/swift-wasm-6.3-RELEASE/swift-wasm-6.3-RELEASE-wasm32-unknown-wasip1.artifactbundle.zip \
#         --checksum 6704d137e532f1ac31eafedd80658f9ee61239f2b6291216a02da32361ea9dcb
#
# Default OPT=-Onone to keep DWARF intact; pass OPT=-O to reproduce
# the wasm-ld DWARF gap.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

SDK_ID="${SWIFTWASM_SDK_ID:-6.3-RELEASE-wasm32-unknown-wasip1}"
OPT="${OPT:--Onone}"

# SwiftPM owns SDK selection (swiftc CLI doesn't know about
# installed SDKs). Build via `swift build --swift-sdk <id>` and
# copy the resulting wasm.
swift build \
    --swift-sdk "$SDK_ID" \
    --configuration release \
    -Xswiftc -g \
    -Xswiftc "$OPT"

BUILT=".build/${SDK_ID#*-}/release/leap.wasm"
# Some SwiftPM versions emit under .build/wasm32-unknown-wasip1/release/.
if [ ! -f "$BUILT" ]; then
    BUILT=$(find .build -maxdepth 3 -name leap.wasm -type f | head -1)
fi
[ -f "$BUILT" ] || { echo "swift build did not produce leap.wasm" >&2; exit 1; }
cp "$BUILT" leap.wasm

echo "built: $SCRIPT_DIR/leap.wasm ($(wc -c < leap.wasm) bytes, opt=$OPT)"
