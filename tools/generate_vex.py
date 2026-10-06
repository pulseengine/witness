#!/usr/bin/env python3
"""Generate a CycloneDX VEX document from cargo-audit results (#235).

A VEX answers the question an SBOM cannot: *is advisory RUSTSEC-X
present in — and does it affect — what this release actually ships?*
The generator is deliberately split along the mechanical/judgement
line the issue draws:

  - the MATCH LIST is mechanical: `cargo audit --json` against the
    shipped lockfile(s) says which advisories match which crate
    versions. This script consumes that output verbatim.
  - the JUDGEMENTS are human: an entry in security/vex-judgements.json
    may classify a match (`not_affected`, `false_positive`, ...) with
    a justification. A match with NO judgement is emitted as
    `in_triage` — never auto-asserted `not_affected`, because a
    generated justification would be a false statement signed into a
    release.

The document records what made it true — the advisory-db commit and
the assertion timestamp — as top-level properties, so a stale VEX is
detectable instead of silently authoritative.

Usage (release workflow):
  cargo audit --file Cargo.lock --json > audit-root.json || true
  python3 tools/generate_vex.py \
      --component witness --version 0.45.0 \
      --audit root=audit-root.json --audit viz=audit-viz.json \
      --advisory-db-commit "$DB_COMMIT" \
      --judgements security/vex-judgements.json \
      --out release-assets/witness-0.45.0.vex.json

Self-test (no network, used by the rivet verification gate):
  python3 tools/generate_vex.py --self-test
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import sys
from pathlib import Path

SPEC_VERSION = "1.5"  # matches the release SBOM (cargo-cyclonedx)

# CycloneDX impact-analysis states a judgement may assert. `in_triage`
# is the only state this script assigns on its own.
VALID_STATES = {
    "resolved",
    "resolved_with_pedigree",
    "exploitable",
    "in_triage",
    "false_positive",
    "not_affected",
}


def _load_audit(path: Path) -> dict:
    """Parse one cargo-audit --json output. Tolerates the non-zero-exit
    case (cargo audit exits 1 when vulnerabilities are found; the JSON
    on stdout is complete either way)."""
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def _matches_from_audit(audit: dict, lock_label: str) -> list[dict]:
    """Flatten cargo-audit's vulnerabilities into (advisory, package)
    match records tagged with which lockfile they came from."""
    out = []
    for entry in audit.get("vulnerabilities", {}).get("list", []):
        adv = entry.get("advisory", {})
        pkg = entry.get("package", {})
        patched = entry.get("versions", {}).get("patched", [])
        out.append(
            {
                "id": adv.get("id", "UNKNOWN"),
                "title": adv.get("title", ""),
                "url": adv.get("url")
                or f"https://rustsec.org/advisories/{adv.get('id', '')}",
                "cvss": adv.get("cvss"),
                "package": pkg.get("name", "unknown"),
                "version": pkg.get("version", "unknown"),
                "patched": patched,
                "lock": lock_label,
            }
        )
    return out


def _analysis_for(match_id: str, judgements: dict) -> dict:
    """The human judgement for a match, or honest `in_triage`."""
    j = judgements.get(match_id)
    if j is None:
        return {
            "state": "in_triage",
            "detail": (
                "Mechanical match from cargo-audit; no human judgement "
                "recorded in security/vex-judgements.json yet."
            ),
        }
    state = j.get("state", "in_triage")
    if state not in VALID_STATES:
        raise SystemExit(
            f"vex-judgements: {match_id} has invalid state {state!r}; "
            f"allowed: {sorted(VALID_STATES)}"
        )
    analysis = {"state": state}
    if "justification" in j:
        analysis["justification"] = j["justification"]
    if "detail" in j:
        analysis["detail"] = j["detail"]
    else:
        raise SystemExit(
            f"vex-judgements: {match_id} needs a `detail` explaining the "
            "judgement — a bare state is not reviewable."
        )
    return analysis


def build_vex(
    component: str,
    version: str,
    audits: dict[str, dict],
    judgements: dict,
    advisory_db_commit: str,
    asserted_at: str,
) -> dict:
    vulnerabilities = []
    for lock_label, audit in sorted(audits.items()):
        for m in _matches_from_audit(audit, lock_label):
            vuln = {
                "id": m["id"],
                "source": {"name": "RustSec", "url": m["url"]},
                "description": m["title"],
                "affects": [
                    {"ref": f"{lock_label}:{m['package']}@{m['version']}"}
                ],
                "analysis": _analysis_for(m["id"], judgements),
                "properties": [
                    {"name": "witness:lockfile", "value": m["lock"]},
                    {
                        "name": "witness:patched-versions",
                        "value": ", ".join(m["patched"]) or "none listed",
                    },
                ],
            }
            vulnerabilities.append(vuln)

    db_counts = {
        label: audit.get("database", {}).get("advisory-count")
        for label, audit in audits.items()
    }
    return {
        "bomFormat": "CycloneDX",
        "specVersion": SPEC_VERSION,
        "version": 1,
        "metadata": {
            "timestamp": asserted_at,
            "component": {
                "type": "application",
                "name": component,
                "version": version,
            },
        },
        "vulnerabilities": vulnerabilities,
        "properties": [
            {"name": "advisory-db.commit", "value": advisory_db_commit},
            {"name": "asserted-at", "value": asserted_at},
            {
                "name": "advisory-db.advisory-count",
                "value": json.dumps(db_counts, sort_keys=True),
            },
            {
                "name": "witness:lockfiles-audited",
                "value": ", ".join(sorted(audits.keys())),
            },
        ],
    }


# --- self-test fixtures (no network, no cargo) -----------------------

_FIXTURE_AUDIT = {
    "database": {"advisory-count": 1290},
    "vulnerabilities": {
        "count": 2,
        "list": [
            {
                "advisory": {
                    "id": "RUSTSEC-2026-9901",
                    "title": "Fixture: judged advisory",
                    "url": "https://rustsec.org/advisories/RUSTSEC-2026-9901",
                },
                "package": {"name": "fixture-a", "version": "1.0.0"},
                "versions": {"patched": [">=1.0.1"]},
            },
            {
                "advisory": {
                    "id": "RUSTSEC-2026-9902",
                    "title": "Fixture: unjudged advisory",
                    "url": "https://rustsec.org/advisories/RUSTSEC-2026-9902",
                },
                "package": {"name": "fixture-b", "version": "2.0.0"},
                "versions": {"patched": []},
            },
        ],
    },
}

_FIXTURE_JUDGEMENTS = {
    "RUSTSEC-2026-9901": {
        "state": "not_affected",
        "justification": "vulnerable_code_not_in_execute_path",
        "detail": "Fixture judgement: the vulnerable API is never called.",
    }
}


def self_test() -> None:
    doc = build_vex(
        component="witness",
        version="0.0.0-selftest",
        audits={"root": _FIXTURE_AUDIT},
        judgements=_FIXTURE_JUDGEMENTS,
        advisory_db_commit="fixture-commit",
        asserted_at="2026-01-01T00:00:00Z",
    )
    assert doc["bomFormat"] == "CycloneDX", "bomFormat"
    assert doc["specVersion"] == SPEC_VERSION, "specVersion"
    props = {p["name"]: p["value"] for p in doc["properties"]}
    assert props["advisory-db.commit"] == "fixture-commit", "db commit prop"
    assert props["asserted-at"] == "2026-01-01T00:00:00Z", "asserted-at prop"
    vulns = {v["id"]: v for v in doc["vulnerabilities"]}
    assert len(vulns) == 2, "both matches present"
    judged = vulns["RUSTSEC-2026-9901"]["analysis"]
    assert judged["state"] == "not_affected", "judged state honoured"
    assert judged["justification"] == "vulnerable_code_not_in_execute_path"
    unjudged = vulns["RUSTSEC-2026-9902"]["analysis"]
    assert unjudged["state"] == "in_triage", "unjudged defaults to in_triage"
    assert "justification" not in unjudged, "no fabricated justification"
    # A judgement without `detail` must be rejected, not silently kept.
    try:
        build_vex(
            "witness",
            "0",
            {"root": _FIXTURE_AUDIT},
            {"RUSTSEC-2026-9901": {"state": "not_affected"}},
            "c",
            "t",
        )
    except SystemExit:
        pass
    else:
        raise AssertionError("bare-state judgement was not rejected")
    # The clean case still records what made it true.
    clean = build_vex(
        "witness", "0", {"root": {"database": {"advisory-count": 5}}}, {}, "c", "t"
    )
    assert clean["vulnerabilities"] == [], "clean doc has empty vulns"
    assert {p["name"] for p in clean["properties"]} >= {
        "advisory-db.commit",
        "asserted-at",
    }
    print("generate_vex self-test: OK")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--component", default="witness")
    ap.add_argument("--version", help="release version, e.g. 0.45.0")
    ap.add_argument(
        "--audit",
        action="append",
        default=[],
        metavar="LABEL=PATH",
        help="cargo-audit --json output for one lockfile; repeatable",
    )
    ap.add_argument("--judgements", type=Path, default=None)
    ap.add_argument("--advisory-db-commit", default="unknown")
    ap.add_argument("--out", type=Path, default=None)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        self_test()
        return

    if not args.version or not args.audit:
        ap.error("--version and at least one --audit are required")

    audits = {}
    for spec in args.audit:
        label, _, path = spec.partition("=")
        if not path:
            ap.error(f"--audit needs LABEL=PATH, got {spec!r}")
        audits[label] = _load_audit(Path(path))

    judgements = {}
    if args.judgements and args.judgements.exists():
        judgements = json.loads(args.judgements.read_text(encoding="utf-8"))
        judgements.pop("_comment", None)

    asserted_at = (
        _dt.datetime.now(_dt.timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z")
    )
    doc = build_vex(
        component=args.component,
        version=args.version,
        audits=audits,
        judgements=judgements,
        advisory_db_commit=args.advisory_db_commit,
        asserted_at=asserted_at,
    )
    text = json.dumps(doc, indent=2, sort_keys=False) + "\n"
    if args.out:
        args.out.write_text(text, encoding="utf-8")
        in_triage = sum(
            1
            for v in doc["vulnerabilities"]
            if v["analysis"]["state"] == "in_triage"
        )
        print(
            f"wrote {args.out}: {len(doc['vulnerabilities'])} match(es), "
            f"{in_triage} in_triage"
        )
    else:
        sys.stdout.write(text)


if __name__ == "__main__":
    main()
