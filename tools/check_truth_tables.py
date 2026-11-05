#!/usr/bin/env python3
"""check_truth_tables — compare witness's MC/DC reports to verdicts/*/TRUTH-TABLE.md.

Issue #350, step 1.  Stdlib only, deterministic.

Usage:
  check_truth_tables.py [--evidence DIR] [--verdicts DIR]   check real evidence
  check_truth_tables.py --self-test                         fixtures + negative controls

Exit: 0 = every table matches, 1 = mismatch (per-verdict, per-condition diff
printed), 2 = a table could not be parsed / evidence missing (never skipped).

What "matches" means (decision-tier tables)
-------------------------------------------
The table's machine-readable JSON block is authoritative; its markdown
`### cN: row X vs row Y` / `Pair: row X vs row Y` prose is cross-checked
against it.  For the single decision the table describes, witness's
`report.json` must have:
  1. exactly one decision                       (decision count)
  2. the same number of conditions              (condition count)
  3. per condition cK <-> witness index K-1:    (status)  PROVED  <-> proved
  4. per condition: identical pair of row ids   (pair; unordered)
  5. per condition: identical interpretation    (masking / unique-cause)
Row ids are comparable because witness numbers rows in `--invoke` order and
the verdict crates number `run_row_N` like the table.  Row *vectors* (T/F
cells) are NOT compared: witness's cells use wasm early-exit polarity
(`interpretation_polarity`), which inverts source-level values per condition.

Condition mapping: table cK == witness condition index K-1.  Rust `&&`/`||`
chains evaluate in source order, so the k-th operand is the k-th br_if.

Other tiers (explicit, never silent):
  zero-decision  range_overlap, mixed_or_and: rustc lowers them to bitwise
                 arithmetic; the check asserts witness reports 0 decisions.
  intent         json_lite, nom_numbers, state_machine: the table only states
                 intent (row_count, expected outcomes), not per-condition pairs
                 tied to a witness decision.  Checked: row count == rows
                 invoked.  Reported as NOT-COMPARABLE for conditions.
  prose-only     multi_context: no machine-readable block; declared in
                 PROSE_ONLY and reported as NOT-COMPARABLE.
A table in none of these tiers (or with a malformed block) is an ERROR.
"""
import argparse
import json
import pathlib
import re
import sys

ZERO_DECISION = {
    "range_overlap": "rustc lowers `a && b` over comparisons to bitwise arithmetic (no br_if)",
    "mixed_or_and": "rustc lowers `(a || b) && (c || d)` to bitwise arithmetic (no br_if)",
}
PROSE_ONLY = {
    "multi_context": "table is prose (inline-chain demo); no per-condition truth table to compare",
}
# Rows invoked per intent-tier verdict come from run.json["invoked"].


class TableError(Exception):
    """A TRUTH-TABLE.md that cannot be parsed. Never skipped."""


def parse_table(name, text):
    """Return a dict: {tier, ...} parsed from table markdown, or raise TableError."""
    if name in PROSE_ONLY:
        if "```json" in text:
            raise TableError(f"{name}: listed PROSE_ONLY but now has a JSON block; update the checker")
        return {"tier": "prose", "why": PROSE_ONLY[name]}
    blocks = re.findall(r"```json\s*\n(.*?)\n```", text, flags=re.S)
    if len(blocks) != 1:
        raise TableError(f"{name}: expected exactly 1 ```json block, found {len(blocks)}")
    try:
        j = json.loads(blocks[0])
    except json.JSONDecodeError as e:
        raise TableError(f"{name}: JSON block does not parse: {e}")
    if j.get("verdict") != name:
        raise TableError(f"{name}: JSON verdict field is {j.get('verdict')!r}")
    if "conditions" in j:
        for key in ("passing_rows", "independent_effect_proofs"):
            if key not in j:
                raise TableError(f"{name}: decision table lacks {key!r}")
        conds = [c["id"] for c in j["conditions"]]
        if conds != [f"c{i}" for i in range(1, len(conds) + 1)]:
            raise TableError(f"{name}: condition ids not c1..cN in order: {conds}")
        proofs = j["independent_effect_proofs"]
        if sorted(proofs) != sorted(conds):
            raise TableError(f"{name}: proofs {sorted(proofs)} != conditions {sorted(conds)}")
        pairs = {}
        for cid, p in proofs.items():
            pr = p.get("pair")
            if not (isinstance(pr, list) and len(pr) == 2 and all(isinstance(x, int) for x in pr)):
                raise TableError(f"{name}: {cid} pair malformed: {pr!r}")
            if p.get("interpretation") not in ("masking", "unique-cause"):
                raise TableError(f"{name}: {cid} interpretation malformed: {p.get('interpretation')!r}")
            pairs[cid] = (frozenset(pr), p["interpretation"])
        _crosscheck_markdown(name, text, pairs)
        tier = "zero-decision" if name in ZERO_DECISION else "decision"
        return {"tier": tier, "conditions": conds, "pairs": pairs,
                "rows": len(j["passing_rows"])}
    if "expected_outcomes" in j and "row_count" in j:
        eo = j["expected_outcomes"]
        n = len(eo.get("true", [])) + len(eo.get("false", []))
        if n != j["row_count"]:
            raise TableError(f"{name}: expected_outcomes covers {n} rows, row_count says {j['row_count']}")
        return {"tier": "intent", "row_count": j["row_count"]}
    raise TableError(f"{name}: JSON block is neither a decision table nor an intent table")


def _crosscheck_markdown(name, text, pairs):
    """Prose `row X vs row Y` per condition must agree with the JSON pair."""
    sections = re.split(r"^### ", text, flags=re.M)[1:]
    seen = 0
    for sec in sections:
        m = re.match(r"(c\d+)\b", sec)
        if not m or m.group(1) not in pairs:
            continue
        pm = re.search(r"row (\d+) vs row (\d+)", sec)
        if not pm:
            continue
        seen += 1
        md = frozenset((int(pm.group(1)), int(pm.group(2))))
        if md != pairs[m.group(1)][0]:
            raise TableError(
                f"{name}: {m.group(1)} markdown pair {sorted(md)} != JSON pair {sorted(pairs[m.group(1)][0])}")


def compare(name, table, report, run):
    """Return list of mismatch strings (empty = match) and note lines."""
    diffs, notes = [], []
    tier = table["tier"]
    decisions = report.get("decisions", [])
    if tier == "prose":
        notes.append(f"NOT-COMPARABLE: {table['why']}")
        return diffs, notes
    if tier == "zero-decision":
        if decisions:
            diffs.append(f"decisions: expected 0 ({ZERO_DECISION[name]}), witness reports {len(decisions)}")
        else:
            notes.append(f"0 decisions as expected: {ZERO_DECISION[name]}")
        return diffs, notes
    if tier == "intent":
        rows = [i for i in run.get("invoked", []) if i.startswith("run_row_")]
        if len(rows) != table["row_count"]:
            diffs.append(f"rows: table row_count {table['row_count']}, witness invoked {len(rows)}")
        notes.append("NOT-COMPARABLE per condition: table states intent only (no per-condition pairs)")
        return diffs, notes
    # decision tier
    if len(decisions) != 1:
        diffs.append(f"decision count: table describes 1 decision, witness reports {len(decisions)}")
        return diffs, notes
    wconds = decisions[0]["conditions"]
    n = len(table["conditions"])
    if len(wconds) != n:
        diffs.append(f"condition count: table {n}, witness {len(wconds)}")
    for k, cid in enumerate(table["conditions"]):
        want_pair, want_interp = table["pairs"][cid]
        if k >= len(wconds):
            diffs.append(f"{cid} (witness index {k}): missing in witness; table says PROVED pair {sorted(want_pair)} {want_interp}")
            continue
        w = wconds[k]
        if w["status"] != "proved":
            diffs.append(f"{cid} (witness index {k}): status table PROVED, witness {w['status']}")
            continue
        got_pair = frozenset(w.get("pair", []))
        if got_pair != want_pair:
            diffs.append(f"{cid} (witness index {k}): pair table {sorted(want_pair)}, witness {sorted(got_pair)}")
        if w.get("interpretation") != want_interp:
            diffs.append(f"{cid} (witness index {k}): interpretation table {want_interp}, witness {w.get('interpretation')}")
    return diffs, notes


def check_all(verdicts_dir, evidence_dir):
    results, errors = {}, []
    tables = sorted(pathlib.Path(verdicts_dir).glob("*/TRUTH-TABLE.md"))
    if not tables:
        raise TableError(f"no TRUTH-TABLE.md under {verdicts_dir}")
    for t in tables:
        name = t.parent.name
        try:
            table = parse_table(name, t.read_text())
        except TableError as e:
            errors.append(str(e))
            continue
        ev = pathlib.Path(evidence_dir) / name
        try:
            report = json.loads((ev / "report.json").read_text())
            run = json.loads((ev / "run.json").read_text())
        except (OSError, json.JSONDecodeError) as e:
            errors.append(f"{name}: evidence unreadable in {ev}: {e}")
            continue
        results[name] = (table["tier"],) + compare(name, table, report, run)
    return results, errors


def render(results, errors):
    out = []
    for name, (tier, diffs, notes) in sorted(results.items()):
        out.append(f"{'FAIL' if diffs else 'ok  '} {name:<16} [{tier}]")
        for d in diffs:
            out.append(f"       MISMATCH {d}")
        for n in notes:
            out.append(f"       note: {n}")
    for e in errors:
        out.append(f"ERROR {e}")
    bad = sum(1 for v in results.values() if v[1])
    out.append(f"{len(results)} verdicts: {len(results) - bad} match, {bad} mismatch, {len(errors)} error")
    return "\n".join(out)


# ---------------------------------------------------------------- self-test
FIX_TABLE = """# fx

```json
{"verdict": "fx", "conditions": [{"id":"c1","source":"a"},{"id":"c2","source":"b"}],
 "passing_rows": [{"row":0},{"row":1},{"row":2}],
 "independent_effect_proofs": {
   "c1": {"pair": [1, 2], "interpretation": "masking"},
   "c2": {"pair": [0, 2], "interpretation": "unique-cause"}}}
```
"""
FIX_REPORT = {"decisions": [{"conditions": [
    {"index": 0, "status": "proved", "pair": [2, 1], "interpretation": "masking"},
    {"index": 1, "status": "proved", "pair": [0, 2], "interpretation": "unique-cause"}]}]}


def self_test():
    import copy
    failures = []

    def expect(label, cond):
        print(f"  {'ok  ' if cond else 'FAIL'} {label}")
        if not cond:
            failures.append(label)

    table = parse_table("fx", FIX_TABLE)
    d, _ = compare("fx", table, FIX_REPORT, {})
    expect("green: matching fixture has no mismatch", d == [])

    r = copy.deepcopy(FIX_REPORT)
    r["decisions"][0]["conditions"][1]["pair"] = [1, 2]
    d, _ = compare("fx", table, r, {})
    expect("NEG swapped pair row -> exactly one mismatch naming c2 and both pairs",
           len(d) == 1 and d[0].startswith("c2") and "[0, 2]" in d[0] and "[1, 2]" in d[0])
    print("       ->", d[0] if d else "(none)")

    r = copy.deepcopy(FIX_REPORT)
    r["decisions"][0]["conditions"][0]["status"] = "gap"
    d, _ = compare("fx", table, r, {})
    expect("NEG flipped verdict (proved->gap) -> mismatch on c1 status",
           len(d) == 1 and "c1" in d[0] and "status" in d[0])
    print("       ->", d[0] if d else "(none)")

    r = copy.deepcopy(FIX_REPORT)
    r["decisions"][0]["conditions"].pop()
    d, _ = compare("fx", table, r, {})
    expect("NEG dropped condition -> count + missing c2",
           any("condition count" in x for x in d) and any(x.startswith("c2") for x in d))

    r = copy.deepcopy(FIX_REPORT)
    r["decisions"][0]["conditions"][0]["interpretation"] = "unique-cause"
    d, _ = compare("fx", table, r, {})
    expect("NEG interpretation changed -> mismatch", len(d) == 1 and "interpretation" in d[0])

    d, _ = compare("fx", table, {"decisions": []}, {})
    expect("NEG zero decisions for a decision table -> mismatch", len(d) == 1 and "decision count" in d[0])

    # markdown/JSON disagreement must be an error
    bad_md = FIX_TABLE.replace("```json", "### c1\n\nPair: row 0 vs row 1.\n\n```json", 1)
    try:
        parse_table("fx", bad_md)
        expect("NEG markdown pair disagreeing with JSON -> TableError", False)
    except TableError as e:
        expect("NEG markdown pair disagreeing with JSON -> TableError", "markdown pair" in str(e))

    # parser fails loudly
    for label, text in (("no json block", "# fx\nprose only\n"),
                        ("broken json", "```json\n{nope\n```\n"),
                        ("wrong verdict name", FIX_TABLE.replace('"verdict": "fx"', '"verdict": "zz"')),
                        ("unknown shape", '```json\n{"verdict": "fx"}\n```\n'),
                        ("bad condition ids", FIX_TABLE.replace('"c2","source":"b"', '"c9","source":"b"'))):
        try:
            parse_table("fx", text)
            expect(f"unparseable table fails loudly: {label}", False)
        except TableError:
            expect(f"unparseable table fails loudly: {label}", True)

    # zero-decision tier
    zt = parse_table("range_overlap", FIX_TABLE.replace('"fx"', '"range_overlap"'))
    d, _ = compare("range_overlap", zt, {"decisions": []}, {})
    expect("zero-decision verdict with 0 decisions -> match", d == [])
    d, _ = compare("range_overlap", zt, FIX_REPORT, {})
    expect("NEG zero-decision verdict that gains a decision -> mismatch", len(d) == 1)

    if failures:
        print(f"self-test FAILED: {len(failures)} check(s)")
        return 1
    print("self-test passed")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--evidence", default="build/verdict-evidence")
    ap.add_argument("--verdicts", default="verdicts")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return self_test()
    try:
        results, errors = check_all(a.verdicts, a.evidence)
    except TableError as e:
        print(f"ERROR {e}")
        return 2
    print(render(results, errors))
    if errors:
        return 2
    return 1 if any(v[1] for v in results.values()) else 0


if __name__ == "__main__":
    sys.exit(main())
