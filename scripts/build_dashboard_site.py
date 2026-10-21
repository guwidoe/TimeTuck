#!/usr/bin/env python3
"""Build the Pages site from an explicitly sanitized, source-bound report snapshot."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
ASSETS = {'index.html': 'dashboard/index.html', 'app.js': 'dashboard/app.js',
          'styles.css': 'dashboard/styles.css',
          'assets/Barlow-SemiBold.ttf': 'dashboard/assets/Barlow-SemiBold.ttf',
          'assets/Barlow-OFL.txt': 'dashboard/assets/Barlow-OFL.txt', 'LICENSE': 'LICENSE'}
OMIT = frozenset(('stdout', 'stderr', 'output', 'command', 'commands', 'run_directory',
                  'source_repository', 'application_model_provenance', 'native_test_paths',
                  'local_path', 'local_test', 'local_provenance', 'source_session',
                  'source_path', 'hostname', 'machine', 'builds', 'inputs', 'fixtures'))
RAW_ARRAYS = frozenset(('cases', 'results', 'outputs', 'runs', 'observations',
                        'differences', 'errors', 'failures'))
BRAND = re.compile(r'private[_ -]?application', re.I)
LOCAL_PATH = re.compile(r'(?:/home/|/mnt/[a-z]/|/tmp/|[A-Z]:[\\/]|\\\\wsl\.)', re.I)
CASE_NAMES = {'social-golfer-32x8x10-v1': 'use-case-a', 'sailing-trip-v1': 'use-case-b'}


def public_text(value):
    if LOCAL_PATH.search(value):
        return 'Local execution detail omitted from public snapshot.'
    for original, replacement in CASE_NAMES.items():
        value = value.replace(original, replacement)
    value = BRAND.sub('Real-world use cases', value)
    value = re.sub(r'social[- _]golfer|golfer', 'Use case A', value, flags=re.I)
    return re.sub(r'sailing(?:[- ]trip)?', 'Use case B', value, flags=re.I)


def project(value):
    """Keep numeric observations, statuses and hashes; remove private execution data."""
    if isinstance(value, dict):
        result = {}
        for key, item in value.items():
            if key in OMIT:
                continue
            if key in RAW_ARRAYS and isinstance(item, (dict, list)):
                result[f'{key}_count'] = len(item)
                continue
            if key == 'samples' and isinstance(item, dict) and any(
                    isinstance(rows, list) and rows and isinstance(rows[0], dict)
                    for rows in item.values()):
                result['sample_counts'] = {runtime: len(rows) for runtime, rows in item.items()}
                continue
            public_key = 'real_world' if BRAND.fullmatch(key) else public_text(key)
            result[public_key] = project(item)
        return result
    if isinstance(value, list):
        return [project(item) for item in value]
    return public_text(value) if isinstance(value, str) else value


def validate_public(report):
    if (report.get('schema_version') != 1 or not isinstance(report.get('families'), list)
            or not report['families'] or not isinstance(report.get('methods'), list)
            or not isinstance(report.get('summary'), dict)
            or report.get('publication', {}).get('kind') != 'public-snapshot'):
        raise ValueError('A nonempty, versioned public dashboard snapshot is required.')
    encoded = json.dumps(report, allow_nan=False)
    if BRAND.search(encoded) or re.search(r'social[- ]golfer|sailing', encoded, re.I):
        raise ValueError('Application identity remains in the public snapshot.')

    def inspect(value):
        if isinstance(value, dict):
            for key, item in value.items():
                if key in OMIT or (key in RAW_ARRAYS and isinstance(item, (dict, list))):
                    raise ValueError(f'Private execution field in public snapshot: {key}')
                inspect(item)
        elif isinstance(value, list):
            for item in value:
                inspect(item)
        elif isinstance(value, str) and LOCAL_PATH.search(value):
            raise ValueError('Local path remains in the public snapshot.')
    inspect(report)


def public_report(source, *, source_sha256, source_commit, exported_at, freshness):
    report = project(source)
    # An allowlist prevents a future application-runner field from becoming a
    # publication leak. The original private report is never rewritten.
    apps = [app for app in source.get('application_benchmarks', {}).values()
            if app.get('origin') == 'external_application']
    if len(apps) > 1:
        raise ValueError('Review public labels before adding another application suite.')
    report['application_benchmarks'] = {}
    for app in apps:
        public_app = project({key: app[key] for key in (
            'schema_version', 'origin', 'status', 'generated_at', 'source_digest',
            'expected_source_digest', 'input_sha256', 'valid', 'qualified',
            'qualification_note', 'limits', 'error') if key in app})
        public_app['configuration'] = project({key: app.get('configuration', {})[key]
            for key in ('seeds', 'seconds', 'scale', 'cpu_affinity', 'java_modes')
            if key in app.get('configuration', {})})
        public_app['summaries'] = []
        for index, item in enumerate(app.get('summaries', [])):
            summary = project({key: item[key] for key in ('case',
                'rust_over_java_streams_evaluations_per_second', 'throughput_target_met',
                'quality_equal_gated', 'quality_note') if key in item})
            summary['label'] = f'Use case {chr(65 + index)}'
            summary['runtimes'] = {runtime: {key: values[key] for key in (
                'runs', 'valid_runs', 'median_evaluations_per_second', 'scores',
                'feasible_runs', 'median_elapsed_seconds') if key in values}
                for runtime, values in item.get('runtimes', {}).items()}
            public_app['summaries'].append(summary)
        report['application_benchmarks']['real_world'] = public_app
    # Preserve the source report's evidence dates/digests/statuses. Publication
    # metadata is separate and can never turn stale or focused evidence into a pass.
    report['publication'] = {
        'kind': 'public-snapshot', 'exported_at': exported_at,
        'source_report_sha256': source_sha256, 'source_commit': source_commit,
        'verification_fresh': freshness['verification'],
        'completion_fresh': freshness['completion'], 'native_fresh': freshness['native'],
        'scope': 'Sanitized summary snapshot; raw logs, commands, source paths and application provenance are omitted. Original evidence dates, numeric results and statuses are retained.',
    }
    suites = report.get('test_origins', {}).get('external_suites', [])
    for suite in suites:
        suite['id'] = 'real_world'
        suite['label'] = 'Real-world use cases'
        suite['expected_tests'] = [{'label': f'Application check {index + 1}',
                                    'status': test['status']}
            for index, test in enumerate(suite.get('expected_tests', []))]
    validate_public(report)
    return report


def build_site(report, output, *, root=ROOT):
    validate_public(report)
    output = output.resolve()
    if output == root.resolve() or root.resolve().is_relative_to(output):
        raise ValueError('Site output must not contain the repository.')
    allowed = set(ASSETS) | {'data/report.json', '.nojekyll'}
    if output.exists():
        for item in output.rglob('*'):
            if item.is_symlink() or (item.is_file() and item.relative_to(output).as_posix() not in allowed):
                raise ValueError('Site output contains an unexpected file; use a dedicated directory.')
    for name, source_name in ASSETS.items():
        source = root / source_name
        if source.is_symlink() or not source.is_file():
            raise ValueError(f'Missing regular dashboard asset: {name}')
    for name, source_name in ASSETS.items():
        target = output / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(root / source_name, target)
    (output / 'data').mkdir(exist_ok=True)
    (output / 'data/report.json').write_text(json.dumps(report, separators=(',', ':'), allow_nan=False) + '\n')
    (output / '.nojekyll').write_text('')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', type=Path, default=ROOT / 'dashboard/public/report.json',
                        help='Existing public snapshot, or a private report for explicit export')
    parser.add_argument('--output', type=Path, default=ROOT / 'work/pages', help='Dedicated static-site directory')
    parser.add_argument('--snapshot-output', type=Path, help='Save an explicitly exported public snapshot for review/commit')
    args = parser.parse_args()
    raw = args.report.read_bytes()
    source = json.loads(raw)
    if source.get('publication', {}).get('kind') == 'public-snapshot':
        report = source
        validate_public(report)
    else:
        if args.snapshot_output is None:
            parser.error('Private reports require --snapshot-output so the public projection can be reviewed.')
        from compat_report import verification_digest, completion_digest, native_completion_digest
        report = public_report(source, source_sha256=hashlib.sha256(raw).hexdigest(),
            source_commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
            exported_at=datetime.now(timezone.utc).isoformat(), freshness={
                'verification': source.get('verification_digest') == verification_digest(),
                'completion': source.get('completion', {}).get('model_digest') == completion_digest(),
                'native': source.get('native_readiness', {}).get('model_digest') == native_completion_digest(),
            })
    build_site(report, args.output)
    if args.snapshot_output is not None:
        if args.snapshot_output.resolve() == args.report.resolve():
            parser.error('Snapshot output must differ from the evidence input.')
        args.snapshot_output.parent.mkdir(parents=True, exist_ok=True)
        args.snapshot_output.write_text(json.dumps(report, separators=(',', ':'), allow_nan=False) + '\n')
    print(f'Built public dashboard: {args.output} ({len(json.dumps(report)):,} JSON characters)')


if __name__ == '__main__':
    main()
