#!/usr/bin/env bash
# Build the Kotlin/Wasm leap-year fixture via Gradle.
#
# Requires:
#   - JDK 17+ (Kotlin Multiplatform's wasmJs() target needs it)
#   - Gradle 8+ in PATH

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# compileProductionExecutableKotlinWasmJsOptimize produces the
# optimised .wasm file under build/compileSync/wasmJs/main/
# productionExecutable/optimized/. nodejs target is sufficient
# for our needs (no browser bundling required).
gradle --no-daemon compileProductionExecutableKotlinWasmJsOptimize

# Locate the produced wasm. Path varies by Gradle/Kotlin versions.
BUILT=$(find build -name 'leap-year*.wasm' -type f | head -1)
[ -f "$BUILT" ] || { echo "Gradle did not produce a wasm artifact under build/" >&2; exit 1; }
cp "$BUILT" leap.wasm

echo "built: $SCRIPT_DIR/leap.wasm ($(wc -c < leap.wasm) bytes)"
