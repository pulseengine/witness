#!/usr/bin/env python3
"""
Compare two verdict-suite output directories and emit a markdown table
suitable for a PR comment.

Used by .github/workflows/witness-delta.yml's verdict-suite-delta job.

Usage:
    verdict_delta.py <base-dir> <head-dir> <output-md>

Each directory is the output of `verdicts/run-suite.sh`. The script
walks the seven canonical verdict names; if either side is missing a
report (e.g. base predates v0.6.3 and lacks the suite), the entry is
flagged as "new" or "REGRESSION" accordingly.

Output is plain stdout-style markdown so the GitHub comment renders
the table inline. No PyYAML or other deps required.
"""

import json
import sys
from pathlib import Path

VERDICTS = [
    "leap_year",
    "range_overlap",
    "triangle",
    "state_guard",
    "mixed_or_and",
    "safety_envelope",
    "parser_dispatch",
]


def load_report(side_dir: Path, name: str):
    p = side_dir / name / "report.json"
    if not p.is_file():
        return None
    try:
        return json.loads(p.read_text())
    except Exception:
        return None


def overall(report):
    return (report or {}).get("overall", {})


def main():
    if len(sys.argv) != 4:
        print(__doc__.strip(), file=sys.stderr)
        sys.exit(1)
    base_dir = Path(sys.argv[1])
    head_dir = Path(sys.argv[2])
    out_path = Path(sys.argv[3])
    out_path.parent.mkdir(parents=True, exist_ok=True)

    rows = []
    for v in VERDICTS:
        h = load_report(head_dir, v)
        b = load_report(base_dir, v)
        if h is None and b is None:
            continue
        if b is None:
            ho = overall(h)
            rows.append(
                (
                    v,
                    "—",
                    f"{ho.get('decisions_full_mcdc', 0)}/{ho.get('decisions_total', 0)}",
                    f"{ho.get('conditions_proved', 0)}/{ho.get('conditions_gap', 0)}/{ho.get('conditions_dead', 0)}",
                    "new",
                )
            )
            continue
        if h is None:
            bo = overall(b)
            rows.append(
                (
                    v,
                    f"{bo.get('decisions_full_mcdc', 0)}/{bo.get('decisions_total', 0)}",
                    "—",
                    "?",
                    "**REGRESSION**",
                )
            )
            continue
        ho = overall(h)
        bo = overall(b)
        h_full = ho.get("decisions_full_mcdc", 0)
        h_total = ho.get("decisions_total", 0)
        b_full = bo.get("decisions_full_mcdc", 0)
        b_total = bo.get("decisions_total", 0)
        if h_full < b_full or h_total < b_total:
            flag = "**REGRESSION**"
        elif h_full > b_full or h_total > b_total:
            flag = "improvement"
        else:
            flag = "unchanged"
        rows.append(
            (
                v,
                f"{b_full}/{b_total}",
                f"{h_full}/{h_total}",
                f"{ho.get('conditions_proved', 0)}/{ho.get('conditions_gap', 0)}/{ho.get('conditions_dead', 0)}",
                flag,
            )
        )

    out: list[str] = ["## Verdict suite delta", ""]
    if not rows:
        out.append("_No verdict reports produced; suite may not be wired in this branch._")
    else:
        out.append(
            "| verdict | base full/total | head full/total | head conds (proved/gap/dead) | status |"
        )
        out.append("|---|---|---|---|---|")
        for r in rows:
            out.append("| " + " | ".join(str(x) for x in r) + " |")
        regressions = [r for r in rows if "REGRESSION" in r[-1]]
        if regressions:
            out.append("")
            out.append("**Regressions:** " + ", ".join(r[0] for r in regressions))
            out.append("")
            out.append("Review required.")
        else:
            out.append("")
            out.append("No regressions.")
    out.append("")
    out_path.write_text("\n".join(out))
    print(f"wrote {out_path}")


if __name__ == "__main__":
    main()
