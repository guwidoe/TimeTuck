"""Additional application evidence. It never changes upstream compatibility credit."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def application_input_digest(root=ROOT):
    """Presentation/application inputs are separate from the original 18 jobs."""
    from benchmark_real_world import real_world_digest
    h = hashlib.sha256(real_world_digest(root=root).encode())
    for name in ('compat/additional-tests.json', 'scripts/verification_origins.py',
                 'scripts/application_report.py'):
        path = root / name
        h.update(name.encode() + b'\0')
        h.update(path.read_bytes() if path.is_file() else b'missing')
    return h.hexdigest()


def load_application_benchmark(root=ROOT):
    from benchmark_real_world import real_world_digest
    expected = real_world_digest(root=root)
    path = root / 'work/real_world-performance.json'
    base = {'id': 'real_world', 'origin': 'external_application',
            'upstream_method_credit': 0, 'expected_source_digest': expected}
    if not path.is_file():
        return {**base, 'status': 'not_measured'}
    try:
        result = json.loads(path.read_text())
        if not isinstance(result, dict) or (result.get('schema_version') != 1 or result.get('kind') != 'application_benchmark'
                or result.get('upstream_method_credit') != 0 or not isinstance(result.get('runs'), list)
                or not isinstance(result.get('summaries'), list)):
            raise ValueError('Unsupported application benchmark report.')
    except (OSError, ValueError, TypeError) as error:
        return {**base, 'status': 'invalid', 'error': str(error)}
    return {**result, **base,
            'status': 'current' if result.get('input_sha256') == expected else 'stale'}
