#!/usr/bin/env bash
# Run the witness-component (wasm32-wasip2) under the wasmtime CLI and check
# that its reporter exports reproduce the committed fixture reports.
#
# For each fixture directory: report-json(run.json) must equal report.json and
# report-text(run.json) must equal report.txt, byte for byte. A malformed
# input must come back as err(...), so a component that answers ok to
# anything fails here too.
#
# Usage: tools/run-witness-component.sh <component.wasm> <fixture-dir>...
# Env:   WASMTIME (default: wasmtime on PATH)
set -euo pipefail

if [ "$#" -lt 2 ]; then
  echo "usage: $0 <component.wasm> <fixture-dir>..." >&2
  exit 2
fi
wasm=$1
shift
wasmtime=${WASMTIME:-wasmtime}
[ -f "$wasm" ] || { echo "component not found: $wasm" >&2; exit 2; }

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# invoke <func> <json-file> <out-file>: call func(string) and decode the
# WAVE result. Prints ok or err on stdout.
invoke() {
  local func=$1 input=$2 out=$3 arg raw
  # The run record as one WAVE string literal. Compact JSON has no control
  # characters, so JSON string escaping (\" and \\) is valid WAVE.
  arg=$(jq -c . "$input" | jq -Rr tojson)
  if grep -q '\\u' <<<"$arg"; then
    echo "$input: needs a \\u escape, which WAVE spells differently" >&2
    return 2
  fi
  raw=$("$wasmtime" run --invoke "$func($arg)" "$wasm")
  case "$raw" in
    ok\(*\)) printf '%s' "${raw#ok(}" | sed '$s/)$//' | jq -r . > "$out"; echo ok ;;
    err\(*\)) printf '%s' "${raw#err(}" | sed '$s/)$//' | jq -r . > "$out"; echo err ;;
    *) echo "$func: unexpected wasmtime output: ${raw:0:200}" >&2; return 1 ;;
  esac
}

fail=0
checked=0
for dir in "$@"; do
  for pair in "report-json:report.json" "report-text:report.txt"; do
    func=${pair%%:*}
    want=$dir/${pair#*:}
    got=$tmp/out
    status=$(invoke "$func" "$dir/run.json" "$got")
    if [ "$status" != ok ]; then
      echo "FAIL $dir $func: returned err: $(head -c 300 "$got")"
      fail=1
    elif ! cmp -s "$got" "$want"; then
      echo "FAIL $dir $func: output differs from $want"
      diff "$want" "$got" | head -20 || true
      fail=1
    else
      echo "ok   $dir $func == $(basename "$want")"
    fi
    checked=$((checked + 1))
  done
done

# Negative control: a run record that is not one must be refused.
echo '{"not":"a run record"}' > "$tmp/bad.json"
status=$(invoke report-json "$tmp/bad.json" "$tmp/bad.out")
if [ "$status" = err ]; then
  echo "ok   malformed input refused: $(head -c 120 "$tmp/bad.out")"
else
  echo "FAIL malformed input was accepted"
  fail=1
fi

echo "$checked export calls checked against fixtures"
[ "$checked" -gt 0 ] || { echo "no fixtures checked" >&2; exit 1; }
exit "$fail"
