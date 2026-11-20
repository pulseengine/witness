#!/usr/bin/env bash
# Regenerate the `witness recheck` fixture and its planted controls (FEAT-045).
#
# Usage, from anywhere:
#   WITNESS=/abs/path/to/witness tests/fixtures/recheck/regen.sh
#
# Inputs kept across regenerations: suite/<verdict>/source.wasm (copied from
# the verdict-suite CI evidence, built by rustc for wasm32-unknown-unknown).
# Everything else under suite/ is rebuilt the way verdicts/run-suite.sh builds
# a verdict, signed with a fresh Ed25519 key whose public half is written to
# expected-signer.pub (outside the bundle: the trust anchor) and whose secret
# half is deleted on exit. Then INVENTORY.sha256 is written, and the five
# ci-controls/recheck-* plants are rebuilt from the new fixture.
set -euo pipefail

FIX="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$FIX/../../.." && pwd)"
WITNESS="${WITNESS:-$ROOT/target/release/witness}"
case "$WITNESS" in /*) ;; *) WITNESS="$ROOT/$WITNESS" ;; esac
[ -x "$WITNESS" ] || { echo "error: witness not executable at $WITNESS" >&2; exit 1; }

SUITE="$FIX/suite"
REL="tests/fixtures/recheck/suite"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# --- the fixture -----------------------------------------------------------
cd "$SUITE"
find . -type f ! -name source.wasm -exec rm -f {} +
"$WITNESS" keygen --secret "$TMP/expected.sk" --public "$FIX/expected-signer.pub" > /dev/null
cp "$FIX/expected-signer.pub" verifying-key.pub

for v in leap_year:4 state_machine:27; do
    name="${v%:*}"
    rows="${v#*:}"
    "$WITNESS" instrument "$name/source.wasm" -o "$name/instrumented.wasm" > /dev/null
    cp "$name/instrumented.wasm.witness.json" "$name/manifest.json"
    args=()
    for ((i = 0; i < rows; i++)); do args+=(--invoke "run_row_$i"); done
    "$WITNESS" run "$name/instrumented.wasm" "${args[@]}" -o "$name/run.json" > /dev/null
    "$WITNESS" report --input "$name/run.json" --format mcdc > "$name/report.txt"
    "$WITNESS" report --input "$name/run.json" --format mcdc-json > "$name/report.json"
    "$WITNESS" predicate --run "$name/run.json" --module "$name/instrumented.wasm" \
        -o "$name/predicate.json" > /dev/null
    "$WITNESS" attest --predicate "$name/predicate.json" --secret-key "$TMP/expected.sk" \
        --key-id "witness-recheck-fixture/$name" -o "$name/signed.dsse.json" > /dev/null
done
"$WITNESS" recheck --write-inventory . > /dev/null

# --- the plants ------------------------------------------------------------
CTL="$ROOT/ci-controls"
rm -rf "$CTL"/recheck-*
inventory_of() { # $1 = a modified copy of the suite; prints its fresh inventory path
    "$WITNESS" recheck --write-inventory "$1" > /dev/null
    echo "$1/INVENTORY.sha256"
}

# 1. deleted run.json
mkdir -p "$CTL/recheck-deleted-run-json"
echo "$REL/state_machine/run.json" > "$CTL/recheck-deleted-run-json/delete"

# 2. one flipped byte in the instrumented module (bit-flip control)
P="$CTL/recheck-flipped-module-byte/$REL/state_machine"
mkdir -p "$P"
cp state_machine/instrumented.wasm "$P/instrumented.wasm"
off=$(( $(wc -c < "$P/instrumented.wasm") / 2 ))
byte=$(od -An -tu1 -j "$off" -N1 "$P/instrumented.wasm" | tr -d ' ')
printf "$(printf '\\%03o' $(( byte ^ 1 )))" \
    | dd of="$P/instrumented.wasm" bs=1 seek="$off" conv=notrunc 2> /dev/null
cmp -s state_machine/instrumented.wasm "$P/instrumented.wasm" && { echo "error: byte flip did nothing" >&2; exit 1; }

# 3. one condition edited from gap to proved, overall counts and inventory
#    re-computed: only re-derivation from run.json can catch it.
cp -R "$SUITE" "$TMP/gap"
awk '
    !s && /"status": "gap"/ { sub(/"status": "gap"/, "\"status\": \"proved\""); s = 1 }
    !g && /"conditions_gap": / { n = $2 + 0; sub(/[0-9]+/, n - 1); g = 1 }
    !p && /"conditions_proved": / { n = $2 + 0; sub(/[0-9]+/, n + 1); p = 1 }
    { print }
    END { if (!(s && g && p)) exit 1 }
' state_machine/report.json > "$TMP/gap/state_machine/report.json"
P="$CTL/recheck-gap-edited-to-proved/$REL"
mkdir -p "$P/state_machine"
cp "$TMP/gap/state_machine/report.json" "$P/state_machine/report.json"
cp "$(inventory_of "$TMP/gap")" "$P/INVENTORY.sha256"

# 4. an inventory line for a file that is not in the bundle
P="$CTL/recheck-inventory-absent-file/$REL"
mkdir -p "$P"
{
    cat INVENTORY.sha256
    printf '%s  %s\n' "$(shasum -a 256 state_machine/report.json | cut -d' ' -f1)" \
        "state_machine/coverage-addendum.json"
} | LC_ALL=C sort -k2,2 > "$P/INVENTORY.sha256"

# 5. the envelope re-signed with a fresh key; the in-bundle verifying key and
#    the inventory updated to match, so only the expected-key check fails.
cp -R "$SUITE" "$TMP/resign"
"$WITNESS" keygen --secret "$TMP/fresh.sk" --public "$TMP/resign/verifying-key.pub" > /dev/null
"$WITNESS" attest --predicate "$TMP/resign/state_machine/predicate.json" \
    --secret-key "$TMP/fresh.sk" --key-id "witness-recheck-fixture/state_machine" \
    -o "$TMP/resign/state_machine/signed.dsse.json" > /dev/null
P="$CTL/recheck-resigned-fresh-key/$REL"
mkdir -p "$P/state_machine"
cp "$TMP/resign/state_machine/signed.dsse.json" "$P/state_machine/signed.dsse.json"
cp "$TMP/resign/verifying-key.pub" "$P/verifying-key.pub"
cp "$(inventory_of "$TMP/resign")" "$P/INVENTORY.sha256"

echo "regenerated $REL and ci-controls/recheck-*"
