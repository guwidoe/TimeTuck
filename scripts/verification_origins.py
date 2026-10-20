"""Classify recorded native test identities without changing Java method credit.

The caller supplies only checksum/source-validated Cargo observations. This module
does not infer execution from Rust source, fixture presence or benchmark results.
"""
from collections import Counter
import hashlib
import json
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parents[1]
STATES = ('passed', 'failed', 'unverified')
CATEGORIES = (
    ('upstream_adapted', 'Upstream adapted tests',
     'Exact native identities mapped by implemented upstream provenance cases.'),
    ('external_application', 'External application regressions',
     'Recorded tests in explicitly declared external application test files; zero upstream-method credit.'),
    ('native_regression', 'Additional native regressions',
     'Other recorded Rust checks. They may reuse attributed fixtures; this does not claim independently authored source.'),
)


def _counts(states):
    counts = Counter(states)
    return {**{state: counts[state] for state in STATES}, 'total': sum(counts.values())}


def _local_path(value):
    path = PurePosixPath(value)
    if path.is_absolute() or '..' in path.parts or str(path) != value or value in ('', '.'):
        raise ValueError(f'Invalid additional-test repository path: {value!r}')
    return value


def _test(path, name, status, **extra):
    return {'id': f'{path}#{name}', 'local_path': path, 'local_test': name,
            'status': status, **extra}


def _fixture_evidence(fixture, root):
    files = []
    for snapshot in fixture['snapshots']:
        path = root / _local_path(snapshot['local_path'])
        expected = snapshot['sha256']
        if len(expected) != 64 or any(c not in '0123456789abcdef' for c in expected):
            raise ValueError(f'Invalid fixture SHA-256: {snapshot["local_path"]}')
        actual = hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None
        files.append({'role': snapshot['role'], 'local_path': snapshot['local_path'],
                      'source_path': snapshot['source_path'], 'expected_sha256': expected,
                      'actual_sha256': actual,
                      'status': 'missing' if actual is None else 'valid' if actual == expected else 'mismatch'})
    status = ('mismatch' if any(f['status'] == 'mismatch' for f in files)
              else 'missing' if not files or any(f['status'] == 'missing' for f in files) else 'valid')
    return {'id': fixture['id'], 'status': status, 'files': files}


def build_test_origins(results, ledger, root=ROOT):
    """Partition each observed (local_path, local_test) exactly once.

    ``counts`` always counts recorded native identities, not Java methods or all
    runtime invocations. Missing declared application tests appear in
    ``expected_tests`` as unverified, outside the recorded-identity denominator.
    Fixtures are validated against their exact copied bytes independently of
    test execution. No benchmark result is loaded or credited here.
    """
    root = Path(root)
    manifest_path = root / 'compat/additional-tests.json'
    manifest = json.loads(manifest_path.read_text()) if manifest_path.is_file() else {'suites': []}
    suites = manifest['suites']
    implemented = {(case['local_path'], case['local_test']) for case in ledger
                   if case.get('status') == 'implemented'}
    owners, expected_by_suite, suite_ids = {}, {}, set()
    for suite in suites:
        if suite['id'] in suite_ids:
            raise ValueError(f'Duplicate additional-test suite: {suite["id"]}')
        suite_ids.add(suite['id'])
        if suite['origin'] != 'external_application' or suite['upstream_method_credit'] != 0:
            raise ValueError(f'Invalid external application attribution: {suite["id"]}')
        for path in suite['native_test_paths']:
            _local_path(path)
            if path in owners:
                raise ValueError(f'Additional-test file belongs to multiple suites: {path}')
            owners[path] = suite['id']
        expected = set()
        for test in suite['expected_tests']:
            identity = test['local_path'], test['local_test']
            if owners.get(identity[0]) != suite['id'] or not identity[1] or identity in expected:
                raise ValueError(f'Invalid or duplicate expected test: {identity}')
            expected.add(identity)
        expected_by_suite[suite['id']] = expected

    classified = {category: [] for category, _, _ in CATEGORIES}
    external = {suite['id']: [] for suite in suites}
    for (path, name), status in sorted(results.items()):
        if status not in STATES:
            raise ValueError(f'Unknown native test status: {status!r}')
        category = ('upstream_adapted' if (path, name) in implemented else
                    'external_application' if path in owners else 'native_regression')
        classified[category].append(status)
        if category == 'external_application':
            external[owners[path]].append(_test(path, name, status))

    suite_rows = []
    for suite in suites:
        tests = external[suite['id']]
        expected = [_test(path, name, results.get((path, name), 'unverified'),
                          recorded=(path, name) in results)
                    for path, name in sorted(expected_by_suite[suite['id']])]
        missing = sum(not test['recorded'] for test in expected)
        states = [test['status'] for test in tests]
        execution = ('failed' if 'failed' in states else
                     'unverified' if not tests or missing or 'unverified' in states else 'passed')
        fixtures = [_fixture_evidence(fixture, root) for fixture in suite.get('fixtures', [])]
        fixture_status = ('mismatch' if any(f['status'] == 'mismatch' for f in fixtures)
                          else 'missing' if not fixtures or any(f['status'] == 'missing' for f in fixtures)
                          else 'valid')
        suite_rows.append({key: suite[key] for key in
                           ('id', 'label', 'origin', 'upstream_method_credit', 'benchmark_job', 'native_test_paths')}
                          | {'source_repository': suite.get('source_repository'),
                             'application_model_provenance': suite.get('application_model_provenance'),
                             'counts': _counts(states), 'tests': tests, 'expected_tests': expected,
                             'missing_test_count': missing, 'execution_status': execution,
                             'fixture_status': fixture_status, 'fixtures': fixtures})
    return {
        'schema_version': 1,
        'unit': 'recorded_native_test_identities',
        'scope_note': ('Counts checksum/source-validated recorded native test identities, not all runtime '
                       'invocations or upstream Java source methods. Missing declared tests are unverified '
                       'and excluded from recorded counts. Additional tests earn zero upstream-method credit.'),
        'counts': _counts(results.values()),
        'categories': [{'id': category, 'label': label, 'description': description,
                        'counts': _counts(classified[category])}
                       for category, label, description in CATEGORIES],
        'external_suites': suite_rows,
    }
