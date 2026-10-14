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


def normalized(doc: dict) -> list[tuple]:
    """The comparison form: only the answer — (advisory, affected ref,
    analysis). Provenance (db commit, advisory count, timestamps) must
    not trigger a re-issue, or every RustSec update re-signs every
    release."""
    return sorted(
        (
            v.get("id", ""),
            a.get("ref", ""),
            v.get("analysis", {}).get("state", ""),
            v.get("analysis", {}).get("justification", ""),
            v.get("analysis", {}).get("detail", ""),
        )
        for v in doc.get("vulnerabilities", [])
        for a in v.get("affects", [])
    )


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
    cmd = ["gh", "issue", "create", "--repo", REPO, "--title", title,
           "--body", body]
    res = run([*cmd, "--label", "security"], check=False)
    if res.returncode != 0:
        print(f"::warning::gh issue create with label 'security' failed for "
              f"{adv_id}: {res.stderr.strip()[:300]}; retrying without label")
        res = run(cmd, check=False)
    if res.returncode != 0:
        print(f"::warning::could not open classification issue for {adv_id}: "
              f"{res.stderr.strip()[:300]}")
        return
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


def self_test() -> None:
    def doc(commit: str, count: int, ts: str, state: str = "in_triage") -> dict:
        return {
            "version": 1,
            "metadata": {"timestamp": ts},
            "vulnerabilities": [{
                "id": "RUSTSEC-2026-9901",
                "affects": [{"ref": "root:fixture-a@1.0.0"}],
                "analysis": {"state": state, "detail": "d"},
            }],
            "properties": [
                {"name": "advisory-db.commit", "value": commit},
                {"name": "asserted-at", "value": ts},
                {"name": "advisory-db.advisory-count",
                 "value": json.dumps({"root": count})},
            ],
        }

    base = doc("aaa", 1290, "2026-01-01T00:00:00Z")
    assert normalized(base) == normalized(doc("bbb", 1311, "2026-02-01T00:00:00Z")), \
        "provenance-only change must compare equal"
    assert normalized(base) != normalized(
        doc("aaa", 1290, "2026-01-01T00:00:00Z", "not_affected")), \
        "changed analysis must compare unequal"
    extra = doc("aaa", 1290, "2026-01-01T00:00:00Z")
    extra["vulnerabilities"].append(
        {"id": "RUSTSEC-2026-9902", "affects": [{"ref": "root:b@2.0.0"}],
         "analysis": {"state": "in_triage"}})
    assert normalized(base) != normalized(extra), "new match must compare unequal"
    print("vex_reissue self-test: OK")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--releases", type=int, default=3)
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        self_test()
        return

    tools = Path(__file__).resolve().parent
    judgements_path = tools.parent / "security" / "vex-judgements.json"
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
