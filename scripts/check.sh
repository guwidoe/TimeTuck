#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
exec python3 - "$@" <<'PY'
import os
import sys

# Repeated launcher prefixes do not change command lookup, but would otherwise
# invalidate every job's environment fingerprint. Keep the first occurrence,
# including an empty entry (the current directory), in its original order.
if "PATH" in os.environ:
    os.environ["PATH"] = os.pathsep.join(dict.fromkeys(os.environ["PATH"].split(os.pathsep)))
os.execv(sys.executable, [sys.executable, "scripts/verify_checks.py", *sys.argv[1:]])
PY
