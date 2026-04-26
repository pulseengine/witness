#!/usr/bin/env bash
# Run every verdict in the suite end-to-end and write per-verdict evidence
# to the supplied output directory.
#
# Usage:
#   ./run-suite.sh [output-dir]
#
# Default output-dir: compliance/verdict-evidence (relative to repo root).
#
# Each verdict's evidence directory contains:
#   source.wasm            — pre-instrumentation Wasm
#   instrumented.wasm      — post-instrumentation Wasm
#   manifest.json          — branch manifest with reconstructed decisions
#   run.json               — run record (per-row condition vectors)
#   report.txt             — human-readable MC/DC report
#   report.json            — machine-readable MC/DC report
#   predicate.json         — unwrapped in-toto Statement
#   lcov.info / overview.txt — LCOV (when DWARF present)
#
# Two failure modes are tolerated and reported as zero-decision results:
#   - rustc optimised the predicate to bitwise/inline arithmetic
#     (range_overlap, mixed_or_and).
#   - DWARF reconstruction declined to group br_ifs (very rare with
#     v0.6.2's adjacent-line clustering).
#
# A genuine pipeline failure (build, instrument, or run errors) exits
# non-zero so CI catches regressions.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

WITNESS="${WITNESS:-target/release/witness}"
OUT_DIR="${1:-compliance/verdict-evidence}"

if [ ! -x "$WITNESS" ]; then
    echo "error: witness binary not found at '$WITNESS'." >&2
    echo "  hint: cargo build --release -p witness" >&2
    exit 1
fi

# (verdict-name : invoke-row-count) pairs in suite-natural order.
VERDICTS=(
    leap_year:4
    range_overlap:3
    triangle:4
    state_guard:5
    mixed_or_and:5
    safety_envelope:6
    parser_dispatch:6
)

mkdir -p "$OUT_DIR"
SUMMARY="$OUT_DIR/SUMMARY.txt"
echo "witness verdict suite — $(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$SUMMARY"
echo "" >> "$SUMMARY"
printf "%-20s %-10s %-12s %-15s\n" "verdict" "branches" "decisions" "full-mcdc" >> "$SUMMARY"
printf "%-20s %-10s %-12s %-15s\n" "-------" "--------" "---------" "---------" >> "$SUMMARY"

OVERALL_FAIL=0
for v in "${VERDICTS[@]}"; do
    name="${v%:*}"
    rows="${v#*:}"
    src="verdicts/$name"
    out="$OUT_DIR/$name"
    mkdir -p "$out"

    if [ ! -d "$src" ]; then
        echo "  skip: verdict directory '$src' missing" >&2
        continue
    fi

    # Build (wasm32-unknown-unknown — produces a core module walrus can
    # rewrite, vs wasm32-wasip2 which produces a Component).
    (cd "$src" && TARGET=wasm32-unknown-unknown ./build.sh) > "$out/build.log" 2>&1 || {
        echo "FAIL build $name" | tee -a "$SUMMARY"
        OVERALL_FAIL=1
        continue
    }
    cp "$src/verdict_$name.wasm" "$out/source.wasm"

    # Instrument
    "$WITNESS" instrument "$out/source.wasm" -o "$out/instrumented.wasm" 2> "$out/instrument.log" || {
        echo "FAIL instrument $name" | tee -a "$SUMMARY"
        OVERALL_FAIL=1
        continue
    }
    cp "$out/instrumented.wasm.witness.json" "$out/manifest.json"

    # Run all rows
    invoke_args=()
    for ((i=0; i<rows; i++)); do
        invoke_args+=("--invoke" "run_row_$i")
    done
    "$WITNESS" run "$out/instrumented.wasm" "${invoke_args[@]}" -o "$out/run.json" 2> "$out/run.log" || {
        echo "FAIL run $name" | tee -a "$SUMMARY"
        OVERALL_FAIL=1
        continue
    }

    # Reports (text + JSON)
    "$WITNESS" report --input "$out/run.json" --format mcdc > "$out/report.txt" 2>&1 || true
    "$WITNESS" report --input "$out/run.json" --format mcdc-json > "$out/report.json" 2>&1 || true

    # Predicate (unsigned in-toto Statement). Signing the predicate
    # requires a release-time DSSE key; v0.6.4 pulls that into this
    # action.
    "$WITNESS" predicate --run "$out/run.json" --module "$out/instrumented.wasm" -o "$out/predicate.json" 2> "$out/predicate.log" || true

    # LCOV (best-effort — fails harmlessly on zero-branch verdicts).
    "$WITNESS" lcov \
        --run "$out/run.json" \
        --manifest "$out/manifest.json" \
        -o "$out/lcov.info" \
        --overview "$out/overview.txt" \
        > "$out/lcov.log" 2>&1 || true

    # Suite summary stats from the JSON report.
    branches=$(python3 -c "import json; d=json.load(open('$out/run.json')); print(len(d.get('branches', [])))" 2>/dev/null || echo "?")
    decisions=$(python3 -c "import json; d=json.load(open('$out/run.json')); print(len(d.get('decisions', [])))" 2>/dev/null || echo "?")
    full=$(python3 -c "import json; d=json.load(open('$out/report.json', 'r')); print(d['overall']['decisions_full_mcdc'])" 2>/dev/null || echo "?")
    total=$(python3 -c "import json; d=json.load(open('$out/report.json', 'r')); print(d['overall']['decisions_total'])" 2>/dev/null || echo "?")

    printf "%-20s %-10s %-12s %-15s\n" "$name" "$branches" "$decisions" "$full/$total" >> "$SUMMARY"
done

echo "" >> "$SUMMARY"
echo "Detail per verdict:" >> "$SUMMARY"
for v in "${VERDICTS[@]}"; do
    name="${v%:*}"
    out="$OUT_DIR/$name"
    if [ -f "$out/report.txt" ]; then
        echo "" >> "$SUMMARY"
        echo "=== $name ===" >> "$SUMMARY"
        cat "$out/report.txt" >> "$SUMMARY"
    fi
done

echo "" >> "$SUMMARY"
echo "Generated $(date -u +%Y-%m-%dT%H:%M:%SZ) by witness $($WITNESS --version 2>&1 | head -1)" >> "$SUMMARY"

cat "$SUMMARY"

exit $OVERALL_FAIL
