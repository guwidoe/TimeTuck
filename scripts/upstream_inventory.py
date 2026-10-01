#!/usr/bin/env python3
"""Verify the reference pin and inventory Java test sources; does not run tests."""
import json
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
reference = json.loads((root / "compat/upstream.json").read_text())
checkout = root / reference["path"]
if not (checkout / ".git").exists():
    sys.exit("Initialize the reference: git submodule update --init upstream/timefold-solver")


def git(*args):
    return subprocess.check_output(["git", "-C", str(checkout), *args], text=True).strip()


revision = git("rev-parse", "HEAD")
if revision != reference["commit"]:
    sys.exit(f"Reference revision mismatch: expected {reference['commit']}, got {revision}")
if git("status", "--porcelain"):
    sys.exit("Reference checkout is modified; restore or preserve changes before inventorying.")
paths = sorted(
    path for path in git("ls-files", "-z").split("\0")
    if "/src/test/java/" in path and path.endswith(".java")
)
if not paths:
    sys.exit("No Java test sources found at the pinned revision.")
output = root / "work/upstream-tests.json"
output.parent.mkdir(exist_ok=True)
output.write_text(json.dumps({
    "revision": revision,
    "description": "Java test-source files, including helpers; not a test or coverage count",
    "files": paths,
}, indent=2) + "\n")
print(f"Verified {reference['tag']} at {revision}")
print(f"Inventoried {len(paths)} Java test-source files: {output.relative_to(root)}")
