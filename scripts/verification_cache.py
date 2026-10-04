"""Reuse successful jobs only; retain original logs, dates, and output checksums."""
from datetime import datetime, timezone
from functools import lru_cache
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
from verification_inputs import ROOT, MEASUREMENTS, digest, measurement_digest


@lru_cache(maxsize=None)
def environment(kind):
    commands = {'python': [], 'rust': [['rustc', '-Vv'], ['cargo', '-V']],
                'measurement': [['rustc', '-Vv'], ['cargo', '-V'], ['java', '-version'], ['javac', '-version']]}
    versions = []
    for command in commands[kind]:
        try:
            result = subprocess.run(command, capture_output=True, text=True, check=True)
            versions.append(result.stdout + result.stderr)
        except (OSError, subprocess.CalledProcessError):
            versions.append('unavailable:' + repr(command))
    return {'platform': platform.platform(), 'python': sys.version, 'tools': versions,
            'env': {k: v for k, v in os.environ.items() if k.startswith(('CARGO_', 'RUST', 'JAVA_', 'JDK_'))
                    or k in ('PATH', 'PYTHONPATH', 'LANG', 'LC_ALL', 'TZ')}}


def checksum(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def cache_hit(record, key, outputs):
    try:
        data = json.loads(record.read_text())
        return (data['exit_code'] == 0 and data['key'] == key and
                data['outputs'] == {str(p): checksum(p) for p in outputs})
    except (OSError, ValueError, KeyError, TypeError):
        return False


def save_record(record, key, outputs):
    record.parent.mkdir(parents=True, exist_ok=True)
    data = {'key': key, 'exit_code': 0, 'finished_at': datetime.now(timezone.utc).isoformat(),
            'outputs': {str(p): checksum(p) for p in outputs}}
    temporary = record.with_suffix('.tmp')
    temporary.write_text(json.dumps(data, indent=2) + '\n')
    temporary.replace(record)


def run_job(name, command, inputs, *, kind='rust', force=False, root=ROOT, extra=None):
    """Run or reuse one command; callable inputs detect additions/removals too."""
    work = root / 'work/verification'
    work.mkdir(parents=True, exist_ok=True)
    log, record = work / f'{name}.log', work / f'{name}.json'
    def key():
        return digest(inputs(), root, extra={'command': command, 'environment': environment(kind), 'extra': extra})
    before = key()
    if not force and cache_hit(record, before, [log]):
        print(f'REUSE {name}', flush=True)
        return 0, log
    # A failed/interrupted forced rerun must not leave an older success reusable.
    record.unlink(missing_ok=True)
    print(f'RUN   {name}', flush=True)
    with log.open('w') as output:
        process = subprocess.Popen(command, cwd=root, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        for line in process.stdout:
            output.write(line)
            print(line, end='', flush=True)
        code = process.wait()
        process.stdout.close()
    if code == 0:
        if before != key():
            print(f'Inputs changed during {name}; result not cached.', file=sys.stderr)
            return 1, log
        save_record(record, before, [log])
    return code, log


def run_measurement(job, main):
    """Existing direct benchmark commands gain reuse and --force automatically."""
    force = '--force' in sys.argv
    if force: sys.argv.remove('--force')
    if any(a in sys.argv for a in ('-h', '--help')):
        return main()
    args = sys.argv[1:]
    report = ROOT / 'work' / f'{job}.json'
    record = ROOT / 'work/verification' / f'{job}.json'
    before = measurement_digest(job)
    key = digest([], extra={'source': before, 'args': args, 'environment': environment('measurement')})
    if not force and cache_hit(record, key, [report]):
        print(f'REUSE {job} ({report.relative_to(ROOT)}; original measurement date retained)')
        return 0
    record.unlink(missing_ok=True)
    result = main()
    if result not in (None, 0): return result
    if before != measurement_digest(job):
        raise RuntimeError('Sources changed during measurement; result not cached')
    data = json.loads(report.read_text())
    if data.get('source_digest') != before:
        raise RuntimeError('Measurement fingerprint does not match the job')
    save_record(record, key, [report])
    return result
