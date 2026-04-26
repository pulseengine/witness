#!/usr/bin/env python3
"""
Generate the v0.6 V-model traceability matrix from artifacts/*.yaml.

REQ-032 — every release ships a JSON + HTML matrix tracing every
requirement through its satisfying features, supporting design
decisions, verifying verdicts, test rows, and signed predicates.

The script is dependency-light by design — only PyYAML is needed,
which the action installs via apt before invoking. No rivet binary
required (the artefact graph in this repo is small enough to walk
directly), keeping the action portable.

Usage:
    trace-matrix.py <artifacts-dir> <verdict-evidence-dir> <output-dir>
"""

import json
import os
import sys
from pathlib import Path

try:
    import yaml
except ModuleNotFoundError:
    print("error: PyYAML is required. install: pip install pyyaml", file=sys.stderr)
    sys.exit(2)


def load_yaml(path: Path):
    with path.open(encoding="utf-8") as f:
        return yaml.safe_load(f) or {}


def main():
    if len(sys.argv) != 4:
        print(__doc__.strip(), file=sys.stderr)
        sys.exit(1)
    artifacts_dir, verdict_dir, out_dir = (Path(p) for p in sys.argv[1:])
    out_dir.mkdir(parents=True, exist_ok=True)

    requirements = load_yaml(artifacts_dir / "requirements.yaml").get("artifacts", [])
    features = load_yaml(artifacts_dir / "features.yaml").get("artifacts", [])
    decisions = load_yaml(artifacts_dir / "design-decisions.yaml").get("artifacts", [])

    by_id = {a["id"]: a for a in requirements + features + decisions}

    # Build inverted indices: id -> list of (kind, source_id) that point at it.
    incoming: dict[str, list[tuple[str, str]]] = {}
    for art in features + decisions:
        for link in art.get("links", []) or []:
            target = link.get("target")
            if target:
                incoming.setdefault(target, []).append((link["type"], art["id"]))

    # Collect verdict evidence directories that exist on disk.
    verdict_names: list[str] = []
    if verdict_dir.is_dir():
        for entry in sorted(verdict_dir.iterdir()):
            if entry.is_dir() and (entry / "report.json").is_file():
                verdict_names.append(entry.name)

    # ----------------- JSON matrix -----------------

    matrix: dict = {
        "schema": "https://pulseengine.eu/witness-trace-matrix/v1",
        "generated_at": os.environ.get("GITHUB_SHA", "local"),
        "release_label": os.environ.get("GITHUB_REF_NAME", "dev"),
        "totals": {
            "requirements": len(requirements),
            "features": len(features),
            "design_decisions": len(decisions),
            "verdicts": len(verdict_names),
        },
        "requirements": [],
        "verdicts": [],
    }

    def linked(req_id: str, want_kind: str) -> list[str]:
        return [src for (kind, src) in incoming.get(req_id, []) if kind == want_kind]

    for req in requirements:
        req_id = req["id"]
        matrix["requirements"].append(
            {
                "id": req_id,
                "title": req.get("title", ""),
                "status": req.get("status", "unknown"),
                "tags": req.get("tags", []),
                "satisfied_by_features": linked(req_id, "satisfies"),
                "supporting_decisions": [
                    src for (kind, src) in incoming.get(req_id, [])
                    if kind in ("satisfies", "refines") and src.startswith("DEC-")
                ],
            }
        )

    # Per-verdict block: pull the MC/DC report's overall section.
    for name in verdict_names:
        try:
            report = json.loads((verdict_dir / name / "report.json").read_text())
        except Exception as e:
            matrix["verdicts"].append({"name": name, "error": str(e)})
            continue
        overall = report.get("overall", {})
        matrix["verdicts"].append(
            {
                "name": name,
                "decisions_total": overall.get("decisions_total", 0),
                "decisions_full_mcdc": overall.get("decisions_full_mcdc", 0),
                "conditions_proved": overall.get("conditions_proved", 0),
                "conditions_gap": overall.get("conditions_gap", 0),
                "conditions_dead": overall.get("conditions_dead", 0),
                "trace_health": report.get("trace_health", {}),
                "signed_envelope": (verdict_dir / name / "signed.dsse.json").is_file(),
                "predicate": (verdict_dir / name / "predicate.json").is_file(),
            }
        )

    (out_dir / "traceability-matrix.json").write_text(json.dumps(matrix, indent=2) + "\n")

    # ----------------- HTML matrix -----------------

    html = ['<!doctype html>',
            '<html lang="en"><head><meta charset="utf-8">',
            f'<title>witness traceability matrix — {matrix["release_label"]}</title>',
            '<style>',
            'body{font-family:system-ui,sans-serif;margin:2rem;max-width:60rem}',
            'h1,h2,h3{font-weight:600}',
            'table{border-collapse:collapse;width:100%;margin:1rem 0}',
            'th,td{padding:.4rem .6rem;text-align:left;border-bottom:1px solid #ddd;vertical-align:top;font-size:.92rem}',
            'th{background:#f5f5f5;font-weight:600}',
            'code{background:#f5f5f5;padding:.05rem .25rem;border-radius:3px;font-size:.85rem}',
            '.tag{display:inline-block;padding:.05rem .35rem;background:#e7e7e7;border-radius:3px;margin-right:.2rem;font-size:.78rem}',
            '.status-approved{color:#0a7d32}.status-draft{color:#7d6c0a}.status-proposed{color:#5a5a5a}',
            '.full{color:#0a7d32}.partial{color:#7d6c0a}.none{color:#5a5a5a}',
            '</style></head><body>',
            f'<h1>witness traceability matrix</h1>',
            f'<p>Release <code>{matrix["release_label"]}</code> · commit <code>{matrix["generated_at"][:7]}</code></p>',
            f'<p>{matrix["totals"]["requirements"]} requirements · {matrix["totals"]["features"]} features · {matrix["totals"]["design_decisions"]} design-decisions · {matrix["totals"]["verdicts"]} verdicts shipped</p>']

    html.append('<h2>Verdict suite</h2>')
    html.append('<table><thead><tr><th>verdict</th><th>decisions</th><th>conditions</th><th>signed</th></tr></thead><tbody>')
    for v in matrix["verdicts"]:
        if "error" in v:
            html.append(f'<tr><td>{v["name"]}</td><td colspan=3 class=none>{v["error"]}</td></tr>')
            continue
        full = v["decisions_full_mcdc"]
        total = v["decisions_total"]
        cls = "full" if total > 0 and full == total else "partial" if total > 0 else "none"
        sigil = "yes" if v["signed_envelope"] else "—"
        cond_summary = f'{v["conditions_proved"]} proved / {v["conditions_gap"]} gap / {v["conditions_dead"]} dead'
        html.append(f'<tr><td><code>{v["name"]}</code></td><td class={cls}>{full}/{total} full MC/DC</td><td>{cond_summary}</td><td>{sigil}</td></tr>')
    html.append('</tbody></table>')

    html.append('<h2>Requirements</h2>')
    html.append('<table><thead><tr><th>id</th><th>title</th><th>status</th><th>satisfied by</th><th>supporting decisions</th></tr></thead><tbody>')
    for req in matrix["requirements"]:
        sat = " ".join(f'<code>{f}</code>' for f in req["satisfied_by_features"]) or "—"
        decs = " ".join(f'<code>{d}</code>' for d in req["supporting_decisions"]) or "—"
        tags = " ".join(f'<span class=tag>{t}</span>' for t in req["tags"])
        html.append(
            f'<tr><td><code>{req["id"]}</code></td>'
            f'<td>{req["title"]}<br>{tags}</td>'
            f'<td class=status-{req["status"]}>{req["status"]}</td>'
            f'<td>{sat}</td><td>{decs}</td></tr>'
        )
    html.append('</tbody></table>')

    html.append('</body></html>')
    (out_dir / "traceability-matrix.html").write_text("\n".join(html) + "\n")

    print(f"wrote {out_dir/'traceability-matrix.json'}")
    print(f"wrote {out_dir/'traceability-matrix.html'}")


if __name__ == "__main__":
    main()
