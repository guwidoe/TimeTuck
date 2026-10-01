#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
mkdir -p work
check_status=0
python3 scripts/run_rust_tests.py || check_status=$?
if [[ -e upstream/timefold-solver/.git ]]; then
  python3 scripts/compat_report.py --test-output work/rust-tests.log || check_status=$?
fi
if (( check_status != 0 )); then exit "$check_status"; fi
cargo test --workspace --doc --locked
python3 -m unittest discover -s scripts -p 'test_*.py'
