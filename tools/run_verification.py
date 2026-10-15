#!/usr/bin/env python3
"""Execute every rivet `type: test-case` artifact's `fields.steps[].run` commands.

Reads artifacts via `rivet list --filter <sexp>` + `rivet get <id> --format json`,
runs each step under a configurable shell, collects per-artifact pass/fail, and
writes a structured JSON summary alongside human-readable stdout progress.

The `test-case` type is defined in schemas/witness-verification.yaml — see the
schema for the recognised fields (`method`, `steps`).

Usage:
    tools/run_verification.py [--filter '<sexp>'] [--results-json PATH] [--shell SHELL]
                              [--junit PATH --suite-name NAME --timestamp ISO8601]
    tools/run_verification.py --junit-self-test

Defaults:
    --filter '(= type "test-case")'
    --results-json verification-results.json
    --shell        bash

Exit code:
    0  if every matched artifact's steps passed (or were skipped)
    1  if any artifact failed
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
import time
import xml.etree.ElementTree as ET
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Optional


@dataclass
class Result:
    filter: str
    total: int = 0
    passed_count: int = 0
    failed_count: int = 0
    skipped_count: int = 0
    passed: list[str] = field(default_factory=list)
    failed: list[str] = field(default_factory=list)
    skipped: list[str] = field(default_factory=list)


def rivet_list_ids(filter_sexp: str) -> list[str]:
    proc = subprocess.run(
        ["rivet", "list", "--filter", filter_sexp, "--format", "json"],
        capture_output=True,
        text=True,
        check=True,
    )
    data = json.loads(proc.stdout)
    return [a["id"] for a in data.get("artifacts", [])]


def rivet_get_steps(artifact_id: str) -> list[str]:
    proc = subprocess.run(
        ["rivet", "get", artifact_id, "--format", "json"],
        capture_output=True,
        text=True,
        check=True,
    )
    data = json.loads(proc.stdout)
    return [s["run"] for s in data.get("fields", {}).get("steps", []) if "run" in s]


@dataclass
class Case:
    id: str
    status: str  # "pass" | "fail" | "skip"
    seconds: float = 0.0
    message: str = ""


def build_junit(cases: list[Case], suite_name: str, timestamp: str) -> ET.ElementTree:
    """One <testcase> per artifact, name = the rivet ID and NO classname:
    `rivet import-results` joins on `<classname>.<name>`, which only equals
    a rivet ID when classname is absent."""
    suite = ET.Element(
        "testsuite",
        name=suite_name,
        timestamp=timestamp,
        tests=str(len(cases)),
        failures=str(sum(c.status == "fail" for c in cases)),
        skipped=str(sum(c.status == "skip" for c in cases)),
        time=f"{sum(c.seconds for c in cases):.3f}",
    )
    for c in cases:
        tc = ET.SubElement(suite, "testcase", name=c.id, time=f"{c.seconds:.3f}")
        if c.status == "fail":
            ET.SubElement(tc, "failure", message=c.message)
        elif c.status == "skip":
            ET.SubElement(tc, "skipped")
    ET.indent(suite)
    return ET.ElementTree(suite)


def write_junit(path: Path, cases: list[Case], suite_name: str, timestamp: str) -> None:
    build_junit(cases, suite_name, timestamp).write(
        path, encoding="utf-8", xml_declaration=True
    )


def junit_self_test() -> int:
    fixture = [
        Case("TEST-FIX-PASS", "pass", 1.5),
        Case("TEST-FIX-FAIL", "fail", 0.25, "step failed: grep -q \"a<b\" f && echo 'x&y'"),
        Case("TEST-FIX-SKIP", "skip"),
    ]
    ids = [c.id for c in fixture]
    with tempfile.TemporaryDirectory() as tmp:
        xml_path = Path(tmp) / "fixture.junit.xml"
        write_junit(xml_path, fixture, "witness-vTEST", "2026-01-02T03:04:05Z")
        root = ET.parse(xml_path).getroot()
        assert root.tag == "testsuite"
        assert root.get("name") == "witness-vTEST"
        assert root.get("timestamp") == "2026-01-02T03:04:05Z"
        assert (root.get("tests"), root.get("failures"), root.get("skipped")) == ("3", "1", "1")
        cases = root.findall("testcase")
        assert [t.get("name") for t in cases] == ids, "testcase names must be exact IDs"
        assert all("classname" not in t.attrib for t in cases), "classname must be absent"
        assert cases[0].get("time") == "1.500"
        assert cases[1].find("failure").get("message") == fixture[1].message
        assert cases[2].find("skipped") is not None
        print("PASS  junit XML half: parses, names == IDs, no classname, message round-trips")

        try:
            ok = subprocess.run(
                ["rivet", "import-results", "--help"], capture_output=True
            ).returncode == 0
        except FileNotFoundError:
            ok = False
        if not ok:
            print("SKIP  rivet import-results half: rivet absent or lacks the subcommand")
            return 0
        out = Path(tmp) / "out"
        subprocess.run(
            ["rivet", "import-results", "--format", "junit", str(xml_path), "--output", str(out)],
            check=True, capture_output=True,
        )
        values = []
        for f in sorted(out.glob("*.yaml")):
            for line in f.read_text().splitlines():
                key, _, val = line.strip().partition(":")
                if key.lstrip("- ").strip() == "artifact":
                    values.append(val.strip().strip("\"'"))
        assert sorted(values) == sorted(ids), f"imported artifacts {values} != {ids}"
        print(f"PASS  rivet import half: artifacts {values} are exactly the fixture IDs")
    return 0


def run_one_step(cmd: str, shell: str) -> bool:
    """Return True iff exit code is 0. On failure, captures last 2 KB of
    combined stdout/stderr and echoes it so the CI log surfaces what
    actually broke instead of just `failed: <cmd>`."""
    proc = subprocess.run(
        [shell, "-c", cmd],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    if proc.returncode != 0:
        tail = proc.stdout[-2048:].decode("utf-8", errors="replace") if proc.stdout else ""
        if tail:
            for line in tail.splitlines():
                print(f"         > {line}")
    return proc.returncode == 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--filter",
        default='(= type "test-case")',
        help='rivet s-expression filter (default: %(default)r)',
    )
    parser.add_argument(
        "--results-json",
        default="verification-results.json",
        type=Path,
        help="path for the JSON summary (default: %(default)s)",
    )
    parser.add_argument(
        "--shell",
        default=os.environ.get("VERIFY_SHELL", "bash"),
        help="shell used to execute each step (default: %(default)s)",
    )
    parser.add_argument("--junit", type=Path, help="also write a JUnit XML file here")
    parser.add_argument("--suite-name", default="witness-verification")
    parser.add_argument(
        "--timestamp",
        help="testsuite timestamp (ISO-8601 UTC); pass a fixed value for reproducible output",
    )
    parser.add_argument("--junit-self-test", action="store_true")
    args = parser.parse_args()

    if args.junit_self_test:
        return junit_self_test()
    if args.junit and not args.timestamp:
        parser.error("--junit requires --timestamp (rivet derives the run id from it)")
    cases: list[Case] = []

    result = Result(filter=args.filter)

    print("== rivet verification gate ==")
    print(f"filter: {args.filter}")
    print(f"shell:  {args.shell}")
    print()

    try:
        ids = rivet_list_ids(args.filter)
    except subprocess.CalledProcessError as e:
        print(f"rivet list failed: {e.stderr}", file=sys.stderr)
        return 2

    if not ids:
        print("No artifacts matched filter.", file=sys.stderr)
        args.results_json.write_text(json.dumps(asdict(result), indent=2))
        if args.junit:
            write_junit(args.junit, cases, args.suite_name, args.timestamp)
        return 1

    print(f"matched {len(ids)} artifacts")
    print()
    result.total = len(ids)

    for artifact_id in ids:
        try:
            steps = rivet_get_steps(artifact_id)
        except subprocess.CalledProcessError as e:
            print(f"[FAIL] {artifact_id}: rivet get failed: {e.stderr}")
            result.failed.append(artifact_id)
            cases.append(Case(artifact_id, "fail", 0.0, f"rivet get failed: {e.stderr}"))
            continue

        if not steps:
            print(f"[SKIP] {artifact_id} (no fields.steps[].run)")
            result.skipped.append(artifact_id)
            cases.append(Case(artifact_id, "skip"))
            continue

        started = time.monotonic()
        failed_cmd = ""

        print(f"[RUN ] {artifact_id}")
        ok = True
        for cmd in steps:
            print(f"       + {cmd}")
            if not run_one_step(cmd, args.shell):
                ok = False
                failed_cmd = cmd
                print(f"       ✗ failed: {cmd}")
                break

        elapsed = time.monotonic() - started
        if ok:
            print(f"[ OK ] {artifact_id}")
            result.passed.append(artifact_id)
            cases.append(Case(artifact_id, "pass", elapsed))
        else:
            print(f"[FAIL] {artifact_id}")
            result.failed.append(artifact_id)
            cases.append(Case(artifact_id, "fail", elapsed, f"step failed: {failed_cmd}"))

    result.passed_count = len(result.passed)
    result.failed_count = len(result.failed)
    result.skipped_count = len(result.skipped)

    args.results_json.write_text(json.dumps(asdict(result), indent=2))
    if args.junit:
        write_junit(args.junit, cases, args.suite_name, args.timestamp)

    print()
    print("== summary ==")
    print(f"passed:  {result.passed_count}")
    print(f"failed:  {result.failed_count}")
    print(f"skipped: {result.skipped_count}")
    if result.failed:
        print("failed IDs:")
        for fid in result.failed:
            print(f"  - {fid}")

    return 0 if result.failed_count == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
