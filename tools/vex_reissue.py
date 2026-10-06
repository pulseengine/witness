#!/usr/bin/env python3
"""Re-issue VEX documents for already-shipped releases (#235, part 4).

Run weekly (vex-reissue.yml) and on demand. For each of the latest N
published releases, this audits the *tag's own committed lockfiles*
against today's advisory database and compares the resulting VEX with
the one published on that release:

  - unchanged answer  -> nothing happens;
  - changed or absent -> the release's witness-<ver>.vex.json asset is
    replaced (or first issued), re-signed per-asset via keyless cosign
    when available, and any match that has no human judgement yet gets
    a classification issue opened (one per advisory, deduped by title).

The VEX is deliberately OUTSIDE SHA256SUMS.txt (one lifecycle per
document — the sums file is immutable once cut, the VEX is not); its
integrity binding is the per-asset cosign .sig/.cert pair, which this
script refreshes together with the document.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = os.environ.get("GITHUB_REPOSITORY", "pulseengine/witness")
LOCKS = {
    "root": "Cargo.lock",
    "viz": "crates/witness-viz/Cargo.lock",
    "component": "crates/witness-component/Cargo.lock",
}


def run(cmd: list[str], *, check: bool = True, **kw) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, capture_output=True, text=True, check=check, **kw)


def latest_release_tags(n: int) -> list[str]:
    out = run(
        ["gh", "release", "list", "--repo", REPO, "--limit", "30",
         "--json", "tagName", "--jq", ".[].tagName"]
    ).stdout.split()
    import re

    return [t for t in out if re.fullmatch(r"v\d+\.\d+\.\d+", t)][:n]


def advisory_db_commit() -> str:
    base = Path.home() / ".cargo" / "advisory-db"
    for d in sorted(base.glob("*")) if base.is_dir() else []:
        if (d / ".git").exists():
            try:
                return run(["git", "-C", str(d), "rev-parse", "HEAD"]).stdout.strip()
            except subprocess.CalledProcessError:
                pass
    return "unknown"


def audit_tag_lock(tag: str, repo_path: str, tmp: Path, label: str) -> Path | None:
    """`git show tag:lock` -> cargo audit --json. None if the lock
    doesn't exist at that tag (older layouts)."""
    show = run(["git", "show", f"{tag}:{repo_path}"], check=False)
    if show.returncode != 0:
        return None
    lock = tmp / f"{tag}-{label}.lock"
    lock.write_text(show.stdout, encoding="utf-8")
    # cargo audit exits 1 when matches exist; the JSON is complete
    # either way. A missing/empty stdout is the real failure mode.
    res = run(["cargo", "audit", "--file", str(lock), "--json"], check=False)
    if not res.stdout.strip():
        print(f"::warning::cargo audit produced no JSON for {tag} {label}: "
              f"{res.stderr.strip()[:200]}")
        return None
    out = tmp / f"{tag}-{label}.audit.json"
    out.write_text(res.stdout, encoding="utf-8")
    return out


def normalized(doc: dict) -> str:
    """The comparison form: everything except the volatile assertion
    timestamp (and doc version counter)."""
    d = json.loads(json.dumps(doc))
    d.get("metadata", {}).pop("timestamp", None)
    d.pop("version", None)
    d["properties"] = [
        p for p in d.get("properties", []) if p.get("name") != "asserted-at"
    ]
    return json.dumps(d, sort_keys=True)


def existing_vex(tag: str, bare: str, tmp: Path) -> dict | None:
    dl = tmp / f"existing-{bare}"
    dl.mkdir(exist_ok=True)
    res = run(
        ["gh", "release", "download", tag, "--repo", REPO,
         "--pattern", f"witness-{bare}.vex.json", "--dir", str(dl)],
        check=False,
    )
    if res.returncode != 0:
        return None
    path = dl / f"witness-{bare}.vex.json"
    return json.loads(path.read_text(encoding="utf-8")) if path.exists() else None


def ensure_classification_issue(adv_id: str, tags: list[str]) -> None:
    title = f"VEX: classify {adv_id}"
    found = run(
        ["gh", "issue", "list", "--repo", REPO, "--state", "open",
         "--search", f'in:title "{title}"', "--json", "number",
         "--jq", "length"],
        check=False,
    ).stdout.strip()
    if found and found != "0":
        return
    body = (
        f"The weekly VEX re-issue matched **{adv_id}** against the shipped "
        f"lockfile(s) of: {', '.join(tags)}.\n\n"
        "Its VEX entries currently say `in_triage`. A human judgement is "
        "needed in `security/vex-judgements.json` (state + justification + "
        "detail); the next re-issue run then publishes it. See #235 — "
        "`not_affected` is a claim about this source tree and must never "
        "be auto-asserted."
    )
    run(["gh", "issue", "create", "--repo", REPO, "--title", title,
         "--body", body, "--label", "security"], check=False)
    print(f"::notice::opened classification issue for {adv_id}")


def sign_and_upload(tag: str, vex_path: Path, dry_run: bool) -> None:
    uploads = [vex_path]
    have_cosign = run(["which", "cosign"], check=False).returncode == 0
    if have_cosign:
        sig = vex_path.with_suffix(vex_path.suffix + ".sig")
        cert = vex_path.with_suffix(vex_path.suffix + ".cert")
        run(["cosign", "sign-blob", "--yes",
             "--output-signature", str(sig),
             "--output-certificate", str(cert), str(vex_path)])
        uploads += [sig, cert]
    else:
        print("::warning::cosign unavailable — uploading unsigned re-issue")
    if dry_run:
        print(f"[dry-run] would upload to {tag}: "
              f"{', '.join(p.name for p in uploads)}")
        return
    run(["gh", "release", "upload", tag, "--repo", REPO, "--clobber",
         *map(str, uploads)])
    print(f"::notice::re-issued {vex_path.name} on {tag}")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--releases", type=int, default=3)
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    tools = Path(__file__).resolve().parent
    judgements_path = tools.parent / "security" / "vex-judgements.json"
    db_commit = advisory_db_commit()
    needs_judgement: dict[str, list[str]] = {}
    changed = 0

    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        for tag in latest_release_tags(args.releases):
            bare = tag.lstrip("v")
            audit_args = []
            for label, repo_path in LOCKS.items():
                p = audit_tag_lock(tag, repo_path, tmp, label)
                if p is not None:
                    audit_args += ["--audit", f"{label}={p}"]
            if not audit_args:
                print(f"::warning::{tag}: no lockfiles found; skipping")
                continue
            candidate = tmp / f"witness-{bare}.vex.json"
            run([sys.executable, str(tools / "generate_vex.py"),
                 "--component", "witness", "--version", bare,
                 "--advisory-db-commit", db_commit,
                 "--judgements", str(judgements_path),
                 "--out", str(candidate), *audit_args])
            new_doc = json.loads(candidate.read_text(encoding="utf-8"))
            for v in new_doc.get("vulnerabilities", []):
                if v["analysis"]["state"] == "in_triage":
                    needs_judgement.setdefault(v["id"], []).append(tag)
            old_doc = existing_vex(tag, bare, tmp)
            if old_doc is not None and normalized(old_doc) == normalized(new_doc):
                print(f"{tag}: VEX unchanged")
                continue
            changed += 1
            sign_and_upload(tag, candidate, args.dry_run)

    for adv_id, tags in sorted(needs_judgement.items()):
        if args.dry_run:
            print(f"[dry-run] would ensure classification issue for "
                  f"{adv_id} ({', '.join(tags)})")
        else:
            ensure_classification_issue(adv_id, tags)

    print(f"done: {changed} release(s) re-issued, "
          f"{len(needs_judgement)} advisory(ies) awaiting judgement")


if __name__ == "__main__":
    main()
