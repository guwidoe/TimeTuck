#!/usr/bin/env python3
"""Capture real Cargo results with a source fingerprint and log checksum."""
from datetime import datetime, timezone
import hashlib
import json
import subprocess
from compat_report import ROOT, verification_digest

if __name__ == '__main__':
    path = ROOT / 'work/rust-tests.log'
    path.parent.mkdir(exist_ok=True)
    before = verification_digest()
    command = ['cargo', 'test', '--workspace', '--all-targets', '--locked', '--', '--color=never']
    with path.open('w') as log:
        process = subprocess.Popen(command, cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        for line in process.stdout:
            log.write(line)
            print(line, end='', flush=True)
        code = process.wait()
    metadata = {'command': command, 'source_before': before, 'source_after': verification_digest(),
                'exit_code': code, 'finished_at': datetime.now(timezone.utc).isoformat(),
                'log_sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
    path.with_suffix('.log.json').write_text(json.dumps(metadata, indent=2) + '\n')
    raise SystemExit(code)
