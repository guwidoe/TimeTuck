#!/usr/bin/env python3
"""Run affected Rust targets and assemble actual, checksum-verified Cargo logs."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from verification_inputs import ROOT, rust_digest, rust_test_inputs
from verification_cache import run_job


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--force', action='store_true', help='Rerun even when successful evidence matches')
    args = parser.parse_args()
    before = rust_digest()
    logs, code = [], 0
    targets = [('lib', 'crates/solver/src/lib.rs', ['--lib'])]
    targets += [(f'test-{p.stem}', str(p.relative_to(ROOT)), ['--test', p.stem])
                for p in sorted((ROOT / 'crates/solver/tests').glob('*.rs'))]
    targets += [(f'example-{p.stem}', str(p.relative_to(ROOT)), ['--example', p.stem])
                for p in sorted((ROOT / 'crates/solver/examples').glob('*.rs'))]
    for name, entry, selection in targets:
        command = ['cargo', 'test', '-p', 'timetuck', '--locked', *selection, '--', '--color=never']
        status, log = run_job(name, command, lambda entry=entry: rust_test_inputs(entry), force=args.force)
        logs.append(log.read_text())
        code = code or status
    after = rust_digest()
    if after != before: code = code or 1
    path = ROOT / 'work/rust-tests.log'
    path.write_text('\n'.join(logs))
    metadata = {'command': 'per-target cargo test (matching successful targets reused)',
                'source_before': before, 'source_after': after, 'exit_code': code,
                'finished_at': datetime.now(timezone.utc).isoformat(),
                'log_sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
    path.with_suffix('.log.json').write_text(json.dumps(metadata, indent=2) + '\n')
    return code


if __name__ == '__main__':
    raise SystemExit(main())
