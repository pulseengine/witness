#!/usr/bin/env python3
"""Create a GitHub-signed commit on a branch through the GraphQL API.

`main` requires signed commits, and a bot's plain `git commit` is
unsigned, so a CI-made PR can never merge. Commits created by the
`createCommitOnBranch` mutation are signed by GitHub and show as
verified. This script:

  1. points `--branch` at `--base-sha` (creating the branch, or
     force-resetting it so a re-run overwrites rather than stacks);
  2. creates one commit on it that adds/replaces the given files.

It shells out to `gh api` (the token comes from GH_TOKEN, e.g. an
org-app installation token). Stdlib only.

Usage:
  python3 tools/gh_signed_commit.py --repo owner/name \
      --branch rivet-results/v1.2.3 --base-sha <sha> \
      --message-file msg.txt results/a.yaml results/b.yaml

Prints the new commit's oid.
"""

from __future__ import annotations

import argparse
import base64
import json
import subprocess
import sys
from pathlib import Path


def gh_api(args: list[str], payload: dict | None = None) -> dict:
    cmd = ["gh", "api", *args]
    res = subprocess.run(
        cmd,
        input=json.dumps(payload) if payload is not None else None,
        capture_output=True,
        text=True,
    )
    if res.returncode != 0:
        raise SystemExit(f"gh api {' '.join(args)} failed: {res.stderr.strip()}")
    return json.loads(res.stdout) if res.stdout.strip() else {}


def point_branch(repo: str, branch: str, sha: str) -> None:
    exists = subprocess.run(
        ["gh", "api", f"repos/{repo}/git/ref/heads/{branch}"],
        capture_output=True,
        text=True,
    ).returncode == 0
    if exists:
        gh_api(
            ["-X", "PATCH", f"repos/{repo}/git/refs/heads/{branch}", "--input", "-"],
            {"sha": sha, "force": True},
        )
    else:
        gh_api(
            ["-X", "POST", f"repos/{repo}/git/refs", "--input", "-"],
            {"ref": f"refs/heads/{branch}", "sha": sha},
        )


def build_mutation(repo: str, branch: str, base_sha: str, headline: str,
                   body: str, files: dict[str, bytes]) -> dict:
    return {
        "query": (
            "mutation($input: CreateCommitOnBranchInput!) {"
            " createCommitOnBranch(input: $input) { commit { oid url } } }"
        ),
        "variables": {
            "input": {
                "branch": {"repositoryNameWithOwner": repo, "branchName": branch},
                "message": {"headline": headline, "body": body},
                "expectedHeadOid": base_sha,
                "fileChanges": {
                    "additions": [
                        {"path": p, "contents": base64.b64encode(c).decode()}
                        for p, c in sorted(files.items())
                    ]
                },
            }
        },
    }


def self_test() -> None:
    m = build_mutation(
        "o/r", "b", "abc", "head", "body\nline",
        {"results/z.yaml": b"z: 1\n", "results/a.yaml": b"a: <&\"> \n"},
    )
    inp = m["variables"]["input"]
    assert inp["expectedHeadOid"] == "abc"
    assert inp["branch"] == {"repositoryNameWithOwner": "o/r", "branchName": "b"}
    adds = inp["fileChanges"]["additions"]
    assert [a["path"] for a in adds] == ["results/a.yaml", "results/z.yaml"], "sorted"
    assert base64.b64decode(adds[0]["contents"]) == b"a: <&\"> \n", "lossless"
    assert "createCommitOnBranch" in m["query"]
    print("gh_signed_commit self-test: OK")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--repo")
    ap.add_argument("--branch")
    ap.add_argument("--base-sha")
    ap.add_argument("--message-file", type=Path)
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("paths", nargs="*")
    args = ap.parse_args()
    if args.self_test:
        self_test()
        return
    if not (args.repo and args.branch and args.base_sha and args.message_file and args.paths):
        ap.error("--repo, --branch, --base-sha, --message-file and at least one path are required")

    lines = args.message_file.read_text(encoding="utf-8").splitlines()
    headline, body = lines[0], "\n".join(lines[1:]).strip()
    files = {p: Path(p).read_bytes() for p in args.paths}

    point_branch(args.repo, args.branch, args.base_sha)
    out = gh_api(["graphql", "--input", "-"],
                 build_mutation(args.repo, args.branch, args.base_sha, headline, body, files))
    if out.get("errors"):
        raise SystemExit(f"createCommitOnBranch failed: {out['errors']}")
    commit = out["data"]["createCommitOnBranch"]["commit"]
    print(commit["oid"])
    print(f"signed commit: {commit['url']}", file=sys.stderr)


if __name__ == "__main__":
    main()
