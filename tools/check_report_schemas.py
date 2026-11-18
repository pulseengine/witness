#!/usr/bin/env python3
"""Validate witness report.json and predicate.json files against the schema
each one declares.

    python3 tools/check_report_schemas.py <evidence-dir> [--schemas docs/schemas]

For every <evidence-dir>/*/report.json the schema is the one whose `$id`
equals the report's `schema` field; for every */predicate.json it is the
one whose `$id` equals `predicateType`. Exits 1 when a file is invalid,
names a schema that docs/schemas does not ship, or when no file was
checked at all (a validator that checked nothing has verified nothing).
Needs the `jsonschema` package.
"""
import argparse
import json
import pathlib
import sys

try:
    import jsonschema
except ImportError:
    sys.exit("check_report_schemas: needs jsonschema  (pip install jsonschema)")


def load_schemas(schema_dir):
    by_id = {}
    for p in sorted(pathlib.Path(schema_dir).glob("*.json")):
        s = json.loads(p.read_text())
        if "$id" in s:
            by_id[s["$id"]] = (p, s)
    return by_id


def check(evidence_dir, schema_dir):
    by_id = load_schemas(schema_dir)
    checked = failed = 0
    for name, key in (("report.json", "schema"), ("predicate.json", "predicateType")):
        for f in sorted(pathlib.Path(evidence_dir).glob(f"*/{name}")):
            doc = json.loads(f.read_text())
            url = doc.get(key)
            checked += 1
            if url not in by_id:
                print(f"FAIL {f}: {key} {url!r} names no schema in {schema_dir}")
                failed += 1
                continue
            path, schema = by_id[url]
            cls = jsonschema.validators.validator_for(schema)
            errors = sorted(cls(schema).iter_errors(doc), key=lambda e: list(e.absolute_path))
            if errors:
                failed += 1
                print(f"FAIL {f} against {path.name}: {len(errors)} error(s)")
                for e in errors[:5]:
                    loc = "/".join(str(x) for x in e.absolute_path) or "(root)"
                    print(f"     at {loc}: {e.message[:200]}")
            else:
                print(f"ok   {f} against {path.name}")
    print(f"{checked} file(s) checked, {failed} failed")
    if checked == 0:
        print("FAIL: no report.json or predicate.json found; nothing was validated")
        return 1
    return 1 if failed else 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("evidence_dir")
    ap.add_argument("--schemas", default="docs/schemas")
    a = ap.parse_args()
    return check(a.evidence_dir, a.schemas)


if __name__ == "__main__":
    sys.exit(main())
