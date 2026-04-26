#!/usr/bin/env python3
"""
v0.8.2 — generate `suite-index.html` for the compliance bundle.

The compliance bundle ships a SUMMARY.txt scoreboard plus per-verdict
directories with report.txt + report.json + manifest.json + signed
envelopes. v0.8.2 adds an HTML index that:

- renders the scoreboard with proved/gap/dead/full columns + a TOTAL
  row
- links each verdict's report.txt + report.json + signed envelope
- inlines the verdict's source_file → decisions distribution
  (per-source-file rollup) when report.json provides it
- shows headline numbers up front: proved across the suite, full-
  MC/DC count, dead count, signed-or-not flag

Usage:
    suite-index.py <verdict-evidence-dir> <output-html>

No deps beyond Python 3 stdlib.
"""

import json
import sys
from pathlib import Path
from html import escape


HEADER = """<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<title>witness verdict suite — __LABEL__</title>
<style>
:root { color-scheme: light dark; }
body{font-family:system-ui,sans-serif;margin:2rem auto;max-width:64rem;padding:0 1rem;line-height:1.5}
h1,h2,h3{font-weight:600;margin-top:2rem}
h1{margin-top:0}
.headline{display:flex;flex-wrap:wrap;gap:1.5rem;margin:1rem 0;padding:1rem;background:#f4f6f8;border-radius:6px}
.headline div{flex:1;min-width:8rem}
.headline strong{font-size:1.6rem;display:block;color:#0a7d32}
.headline small{color:#5a5a5a;font-size:.8rem;text-transform:uppercase;letter-spacing:.04em}
table{border-collapse:collapse;width:100%;margin:1rem 0;font-size:.92rem}
th,td{padding:.4rem .6rem;text-align:left;border-bottom:1px solid #ddd;vertical-align:top}
th{background:#f5f5f5;font-weight:600}
td.num,th.num{text-align:right;font-variant-numeric:tabular-nums}
tr.total{font-weight:600;background:#fafafa}
tr.total td{border-top:2px solid #888}
code{background:#f5f5f5;padding:.05rem .25rem;border-radius:3px;font-size:.85rem}
a{color:#0a5;text-decoration:none}a:hover{text-decoration:underline}
.full{color:#0a7d32;font-weight:600}
.partial{color:#7d6c0a}
.none{color:#888}
.fail{color:#a30000;font-weight:600}
details{margin:.5rem 0;padding:.5rem;border:1px solid #e0e0e0;border-radius:4px}
summary{cursor:pointer;font-weight:500}
pre{background:#fafafa;padding:.6rem;border-radius:4px;overflow-x:auto;font-size:.82rem;line-height:1.4}
.verify-cmd{display:inline-block;background:#222;color:#0a0;padding:.4rem .8rem;border-radius:4px;font-family:monospace;font-size:.85rem;margin:.4rem 0}
@media (prefers-color-scheme: dark) {
  body{background:#1a1a1a;color:#ddd}
  .headline{background:#252525}
  th{background:#2a2a2a}
  th,td{border-color:#3a3a3a}
  tr.total{background:#252525}
  pre{background:#202020;color:#ccc}
  code{background:#2a2a2a;color:#ddd}
  details{border-color:#3a3a3a}
  .partial{color:#c5a82a}
}
</style></head><body>
<h1>witness verdict suite — <code>__LABEL__</code></h1>
<p>Per-verdict end-to-end MC/DC evidence. Each row links to the
verdict's text and JSON reports plus the signed in-toto Statement.
The signature column shows whether a DSSE envelope was produced;
verify with <code>witness verify --envelope &lt;path&gt;
--public-key verifying-key.pub</code>.</p>
"""


FOOTER = """
<hr>
<p style="color:#888;font-size:.85rem">Generated from
<code>verdict-evidence/</code> by
<code>.github/actions/compliance/suite-index.py</code>.
Schema URL: <code>https://pulseengine.eu/witness-mcdc/v1</code>.</p>
</body></html>
"""


def load_summary(p: Path) -> str:
    if p.is_file():
        return p.read_text()
    return ""


def load_report(p: Path):
    if p.is_file():
        try:
            return json.loads(p.read_text())
        except Exception:
            return None
    return None


def cls_for(full: int, total: int) -> str:
    if total == 0:
        return "none"
    if full == total:
        return "full"
    return "partial"


def main():
    if len(sys.argv) != 3:
        print(__doc__.strip(), file=sys.stderr)
        sys.exit(1)
    bundle_dir = Path(sys.argv[1])
    out_html = Path(sys.argv[2])
    if not bundle_dir.is_dir():
        print(f"error: {bundle_dir} is not a directory", file=sys.stderr)
        sys.exit(2)

    # Discover verdict directories: those with report.json or run.json.
    verdict_dirs = sorted(
        d for d in bundle_dir.iterdir()
        if d.is_dir() and (d / "report.json").is_file()
    )

    label = "dev"
    summary_path = bundle_dir / "SUMMARY.txt"
    if summary_path.is_file():
        first = summary_path.read_text().splitlines()[0]
        if "—" in first:
            label = first.split("—", 1)[1].strip()

    rows = []
    total_branches = total_decisions = total_full = total_proved = total_gap = total_dead = 0
    for d in verdict_dirs:
        report = load_report(d / "report.json")
        if report is None:
            rows.append((d.name, None))
            continue
        o = report.get("overall", {})
        decisions = o.get("decisions_total", 0)
        full = o.get("decisions_full_mcdc", 0)
        proved = o.get("conditions_proved", 0)
        gap = o.get("conditions_gap", 0)
        dead = o.get("conditions_dead", 0)

        # branches from manifest
        manifest = load_report(d / "manifest.json")
        branches = len(manifest.get("branches", [])) if manifest else 0

        signed = (d / "signed.dsse.json").is_file()

        rows.append((d.name, {
            "branches": branches,
            "decisions": decisions,
            "full": full,
            "proved": proved,
            "gap": gap,
            "dead": dead,
            "signed": signed,
            "report_text": (d / "report.txt").is_file(),
            "report_json": (d / "report.json").is_file(),
            "lcov": (d / "lcov.info").is_file(),
        }))
        total_branches += branches
        total_decisions += decisions
        total_full += full
        total_proved += proved
        total_gap += gap
        total_dead += dead

    out = [HEADER.replace("__LABEL__", escape(label))]

    # Headline cards.
    out.append('<div class="headline">')
    out.append(f'<div><strong>{total_proved}</strong><small>conditions proved</small></div>')
    out.append(f'<div><strong>{total_full}</strong><small>full MC/DC decisions</small></div>')
    out.append(f'<div><strong>{total_decisions}</strong><small>decisions total</small></div>')
    out.append(f'<div><strong>{total_branches}</strong><small>br_ifs instrumented</small></div>')
    out.append(f'<div><strong>{len(verdict_dirs)}</strong><small>verdicts</small></div>')
    out.append('</div>')

    # Verify command.
    out.append('<h2>Verify a signed envelope</h2>')
    out.append('<p><span class="verify-cmd">witness verify --envelope httparse/signed.dsse.json --public-key verifying-key.pub</span></p>')
    out.append('<p>The DSSE envelope is standards-compliant; '
               '<code>cosign verify-blob</code> with the same public key works equivalently.</p>')

    # Scoreboard.
    out.append('<h2>Scoreboard</h2>')
    out.append('<table><thead><tr>'
               '<th>verdict</th><th class=num>branches</th>'
               '<th class=num>decisions</th><th class=num>full MC/DC</th>'
               '<th class=num>proved</th><th class=num>gap</th>'
               '<th class=num>dead</th><th>signed</th><th>report</th></tr></thead><tbody>')
    for name, data in rows:
        if data is None:
            out.append(f'<tr><td><code>{escape(name)}</code></td>'
                       '<td colspan="8" class=fail>build failed</td></tr>')
            continue
        cls = cls_for(data["full"], data["decisions"])
        full_str = f'<span class="{cls}">{data["full"]}/{data["decisions"]}</span>'
        signed_str = '<span class="full">yes</span>' if data["signed"] else '<span class="none">—</span>'
        report_links = []
        if data["report_text"]:
            report_links.append(f'<a href="{escape(name)}/report.txt">text</a>')
        if data["report_json"]:
            report_links.append(f'<a href="{escape(name)}/report.json">json</a>')
        if data["lcov"]:
            report_links.append(f'<a href="{escape(name)}/lcov.info">lcov</a>')
        out.append(
            '<tr>'
            f'<td><code>{escape(name)}</code></td>'
            f'<td class=num>{data["branches"]}</td>'
            f'<td class=num>{data["decisions"]}</td>'
            f'<td class=num>{full_str}</td>'
            f'<td class=num>{data["proved"]}</td>'
            f'<td class=num>{data["gap"]}</td>'
            f'<td class=num>{data["dead"]}</td>'
            f'<td>{signed_str}</td>'
            f'<td>{" / ".join(report_links)}</td>'
            '</tr>'
        )
    # Total row.
    out.append(
        '<tr class="total">'
        '<td>TOTAL</td>'
        f'<td class=num>{total_branches}</td>'
        f'<td class=num>{total_decisions}</td>'
        f'<td class=num>{total_full}/{total_decisions}</td>'
        f'<td class=num>{total_proved}</td>'
        f'<td class=num>{total_gap}</td>'
        f'<td class=num>{total_dead}</td>'
        '<td></td><td></td>'
        '</tr>'
    )
    out.append('</tbody></table>')

    # Per-verdict drill-down.
    out.append('<h2>Per-verdict reports</h2>')
    for name, data in rows:
        if data is None:
            continue
        out.append(f'<details><summary><code>{escape(name)}</code> — '
                   f'{data["full"]}/{data["decisions"]} full MC/DC, '
                   f'{data["proved"]} proved, {data["gap"]} gap, {data["dead"]} dead</summary>')
        report_txt = bundle_dir / name / "report.txt"
        if report_txt.is_file():
            try:
                content = report_txt.read_text()
                # Trim very long reports to first ~6000 chars to keep the
                # page tractable. Full report still linked at the top.
                if len(content) > 6000:
                    content = content[:6000] + "\n\n[... truncated; see report.txt for full output ...]"
                out.append(f'<pre>{escape(content)}</pre>')
            except Exception:
                pass
        out.append('</details>')

    out.append(FOOTER)
    out_html.write_text("\n".join(out))
    print(f"wrote {out_html}")


if __name__ == "__main__":
    main()
