#!/usr/bin/env python3
"""Selective local verification; --force bypasses all successful-result caches."""
import argparse
import os
import subprocess
from verification_cache import run_job
from verification_inputs import ROOT, cargo_inputs, files, doctest_inputs, python_inputs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--force', action='store_true')
    args = parser.parse_args()
    def all_rust():
        return cargo_inputs() | files('crates/**/*.rs') | {'scripts/verification_cache.py', 'scripts/verification_inputs.py', 'scripts/verify_checks.py'}
    jobs = [
        ('fmt', ['cargo', 'fmt', '--all', '--', '--check'], all_rust),
        ('clippy', ['cargo', 'clippy', '--workspace', '--all-targets', '--locked',
                    '-j', os.environ.get('CARGO_BUILD_JOBS', '4'), '--', '-D', 'warnings'],
         lambda: all_rust() | files('crates/*/tests/fixtures/**/*')),
    ]
    for name, command, inputs in jobs:
        code, _ = run_job(name, command, inputs, force=args.force)
        if code: return code
    code = subprocess.call(['python3', 'scripts/run_rust_tests.py', *(['--force'] if args.force else [])], cwd=ROOT)
    if (ROOT / 'upstream/timefold-solver/.git').exists():
        report = subprocess.call(['python3', 'scripts/compat_report.py', '--test-output', 'work/rust-tests.log'], cwd=ROOT)
        code = code or report
    if code: return code
    code, _ = run_job('doctests', ['cargo', 'test', '--workspace', '--doc', '--locked'],
                      lambda: doctest_inputs() | {'scripts/verification_cache.py', 'scripts/verification_inputs.py'}, force=args.force)
    if code: return code
    for path in sorted((ROOT / 'scripts').glob('test_*.py')):
        code, _ = run_job(path.stem, ['python3', '-m', 'unittest', 'discover', '-s', 'scripts', '-p', path.name],
                         lambda path=path: python_inputs(str(path.relative_to(ROOT))) | {'compat/upstream.json'} |
                         ({f for f in files('tests/application/real_world/**/*') | files('benchmarks/real_world/**/*')
                           if '__pycache__' not in f}  # importing prep.py writes bytecode here
                          if path.name == 'test_real_world_reference.py' else set()),
                         kind='python', force=args.force)
        if code: return code
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
