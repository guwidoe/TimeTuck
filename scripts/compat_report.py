#!/usr/bin/env python3
"""Inventory annotated upstream test declarations and verify mapped Rust cases.

Counts source methods, not expanded JUnit invocations. Does not execute Java tests.
"""
import argparse
from collections import Counter, defaultdict
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

from verification_inputs import measurement_digest, report_digest, rust_digest

ROOT = Path(__file__).resolve().parents[1]
ANNOTATION = re.compile(r'@(?:org\.junit\.jupiter\.(?:api|params)\.)?(Test|ParameterizedTest|RepeatedTest|TestFactory|TestTemplate)\b')
# Erase strings/comments while preserving line numbers and offsets. Annotations
# in documentation and Java string literals must never inflate the denominator.
NOISE = re.compile(r'"""[\s\S]*?"""|"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'|//[^\n]*|/\*[\s\S]*?\*/')
METHOD = re.compile(r'\b(?:void|[A-Za-z_$][\w.$]*(?:\s*<[^;{}]+>)?(?:\[\])?)\s+([A-Za-z_$][\w$]*)\s*\(')


def verification_digest():
    return report_digest(ROOT)


def declarations(text):
    clean = NOISE.sub(lambda m: ''.join('\n' if c == '\n' else ' ' for c in m[0]), text)
    out = []
    for match in ANNOTATION.finditer(clean):
        cursor = match.end()
        # Skip the current annotation's arguments and all following annotations.
        while True:
            while cursor < len(clean) and clean[cursor].isspace(): cursor += 1
            if cursor < len(clean) and clean[cursor] == '(':
                depth = 1
                cursor += 1
                while cursor < len(clean) and depth:
                    depth += (clean[cursor] == '(') - (clean[cursor] == ')')
                    cursor += 1
                continue
            if cursor < len(clean) and clean[cursor] == '@':
                annotation = re.match(r'@[\w.]+', clean[cursor:])
                if annotation:
                    cursor += annotation.end()
                    continue
            break
        generic = re.match(r'<[^{};]+?>\s*', clean[cursor:])
        if generic: cursor += generic.end()
        method = METHOD.match(clean, cursor)
        if method is None:
            # Visibility/static modifiers may precede the return type.
            prefix = re.match(r'(?:(?:public|protected|private|final|static|abstract|synchronized)\s+)+', clean[cursor:])
            if prefix: method = METHOD.match(clean, cursor + prefix.end())
        if method is None:
            raise ValueError(f'Unclassified @{match[1]} at line {clean.count(chr(10), 0, match.start()) + 1}')
        out.append({'method': method[1], 'annotation': match[1], 'line': clean.count('\n', 0, method.start()) + 1})
    return out


def family(path):
    module = path.split('/src/test/java/')[0]
    if module != 'core':
        if module.startswith('service/'): return 'Service modules'
        if module.startswith(('quarkus', 'spring', 'persistence', 'migration')): return 'JVM integrations'
        if module.startswith('test'): return 'Test support'
        if module.startswith('tools'): return 'Tools'
        if module.startswith('benchmark'): return 'Benchmark tooling'
        return module.replace('/', ' / ').capitalize()
    package = path.split('/ai/timefold/solver/core/', 1)[-1]
    if package.startswith('api/score/') or package.startswith('impl/score/definition/'): return 'Score APIs & types'
    if '/score/stream/' in '/' + package or package.startswith('impl/bavet/'):
        return 'Constraint streams'
    if '/score/' in '/' + package: return 'Score calculation'
    if '/heuristic/' in '/' + package or '/move/' in '/' + package: return 'Moves & selectors'
    if any('/' + x + '/' in '/' + package for x in ['localsearch', 'constructionheuristic', 'exhaustivesearch', 'phase']): return 'Search algorithms'
    if '/domain/' in '/' + package: return 'Domain model'
    if '/solver/' in '/' + package: return 'Solver lifecycle'
    return 'Core utilities & configuration'


def inventory(ref):
    subprocess.run(['python3', str(ROOT / 'scripts/upstream_inventory.py')], check=True)
    files = json.loads((ROOT / 'work/upstream-tests.json').read_text())['files']
    methods = []
    for path in files:
        for item in declarations((ROOT / ref['path'] / path).read_text()):
            item.update(path=path, family=family(path), status='pending', reason='Not adapted to Rust yet')
            item['id'] = f'{path}#{item["method"]}:{item["line"]}'
            methods.append(item)
    return files, methods


def counts(items):
    c = Counter(x['status'] for x in items)
    total = len(items)
    return {'total': total, **{s: c[s] for s in ['passed', 'failed', 'pending', 'excluded', 'unverified']},
            'percent': round(c['passed'] * 100 / total, 3) if total else None}


def run_local(cases, execute):
    results, runs = {}, []
    for path in sorted({c['local_path'] for c in cases if c['status'] == 'implemented'}):
        if not execute: continue
        target = Path(path).stem
        selection = ['-p', 'timetuck', '--lib'] if path == 'crates/solver/src/lib.rs' else ['--test', target]
        command = ['cargo', 'test', '--locked', *selection, '--', '--format=pretty', '--color=never']
        run = subprocess.run(command, cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        runs.append({'command': command, 'exit_code': run.returncode, 'output': run.stdout})
        if run.returncode != 0:
            print(run.stdout, file=sys.stderr)
        for name, state in re.findall(r'^test ([\w:]+) \.\.\. (ok|FAILED|ignored)', run.stdout, re.M):
            results[(path, name)] = {'ok': 'passed', 'FAILED': 'failed', 'ignored': 'unverified'}[state]
    return results, runs


def load_measurement(name, digest=None):
    digest = measurement_digest(name, ROOT) if digest is None else digest
    path = ROOT / 'work' / f'{name}.json'
    if not path.exists(): return {'status': 'not_measured'}
    report = json.loads(path.read_text())
    report['status'] = 'current' if report.get('source_digest') == digest else 'stale'
    return report


def read_test_run(test_output, verified_source):
    log_path = Path(test_output)
    metadata = json.loads(log_path.with_suffix('.log.json').read_text())
    if metadata['source_before'] != verified_source or metadata['source_after'] != verified_source:
        raise ValueError('Test log is stale for these sources. Run bash scripts/check.sh.')
    if hashlib.sha256(log_path.read_bytes()).hexdigest() != metadata['log_sha256']:
        raise ValueError('Test log checksum mismatch. Run bash scripts/check.sh.')
    test_exit = metadata['exit_code']
    output = log_path.read_text()
    results = {}
    current = None
    for line in output.splitlines():
        if 'Running ' in line: current = None
        if re.search(r'Running unittests src/lib\.rs \(.*[/]timetuck-[0-9a-f]+\)', line):
            current = 'crates/solver/src/lib.rs'
        target = re.search(r'Running tests/(\w+)\.rs', line)
        if target: current = f'crates/solver/tests/{target[1]}.rs'
        state = re.match(r'test ([\w:]+) \.\.\. (ok|FAILED|ignored)', line)
        if state and current:
            results[(current, state[1])] = {'ok':'passed', 'FAILED':'failed', 'ignored':'unverified'}[state[2]]
    runs = [{'command': ['cargo', 'test', '--workspace', '--all-targets', '--locked'], 'exit_code': test_exit, 'output': output}]
    return results, runs


def resolve_provenance_target(lookup, case):
    """An optional pinned declaration line disambiguates nested same-name tests."""
    targets = lookup.get((case['upstream_path'], case['upstream_test']), [])
    if 'upstream_line' in case:
        line = case['upstream_line']
        if type(line) is not int or line < 1:
            raise ValueError(f'Invalid provenance source line: {case["id"]}')
        targets = [method for method in targets if method['line'] == line]
    if len(targets) != 1:
        raise ValueError(f'Case target missing or ambiguous: {case["id"]}')
    return targets[0]


def build_report(execute=True, test_output=None):
    verified_source = verification_digest()
    ref = json.loads((ROOT / 'compat/upstream.json').read_text())
    files, methods = inventory(ref)
    rules = json.loads((ROOT / 'compat/exclusions.json').read_text())['rules']
    for method in methods:
        for rule in rules:
            if method['path'].startswith(rule['path_prefix']):
                method.update(status='excluded', reason=rule['reason'])
                break
    ledger = json.loads((ROOT / 'compat/provenance.json').read_text())['cases']
    lookup = defaultdict(list)
    for method in methods: lookup[(method['path'], method['method'])].append(method)
    ids, mapped = set(), set()
    for case in ledger:
        if case['id'] in ids: raise ValueError(f'Duplicate provenance ID: {case["id"]}')
        ids.add(case['id'])
        if case['upstream_commit'] != ref['commit']: raise ValueError('Case revision differs from reference')
        resolve_provenance_target(lookup, case)
        key = (case['local_path'], case.get('local_test'))
        if key in mapped: raise ValueError(f'Multiple upstream methods mapped to one local test: {key}')
        mapped.add(key)
        for notice in case['notice_paths']:
            if not (ROOT / notice).is_file(): raise ValueError(f'Missing notice: {notice}')
    if test_output is None:
        results, runs = run_local(ledger, execute)
    else:
        results, runs = read_test_run(test_output, rust_digest(ROOT))

    for case in ledger:
        method = resolve_provenance_target(lookup, case)
        status = case['status']
        if status not in ['implemented', 'pending', 'excluded']: raise ValueError(f'Invalid status: {status}')
        method.update(local_path=case['local_path'], local_test=case.get('local_test'), provenance_id=case['id'])
        if status == 'implemented':
            method['status'] = results.get((case['local_path'], case.get('local_test')), 'unverified')
            method['reason'] = 'Adapted assertions executed in Rust' if method['status'] == 'passed' else 'Mapped test did not pass or was not executed'
        else:
            method['status'] = status
            method['reason'] = case.get('reason', case['modifications'])
    groups = []
    for name in sorted({m['family'] for m in methods}):
        subset = [m for m in methods if m['family'] == name]
        classes = []
        for path in sorted({m['path'] for m in subset}):
            classes.append({'name': Path(path).stem, 'path': path, **counts([m for m in subset if m['path'] == path])})
        groups.append({'name': name, **counts(subset), 'classes': classes})
    groups.sort(key=lambda g: (-g['passed'], -g['total'], g['name']))
    digest = verified_source
    if verification_digest() != verified_source:
        raise ValueError('Sources changed during verification; rerun to produce a consistent report.')
    return {'schema_version': 1, 'generated_at': datetime.now(timezone.utc).isoformat(),
            'reference': ref, 'source_digest': digest, 'verification_digest': verified_source,
            'inventory': {'source_files': len(files), 'annotations': dict(Counter(m['annotation'] for m in methods)),
                          'unit': 'statically discovered annotated test methods',
                          'exclusion_rules': rules,
                          'limitations': 'Not a JUnit runtime test count. Parameterized, repeated, and inherited template invocations are not expanded. Custom composed test annotations and non-Java tests are not included. The original Java suite is not run by this report.'},
            'verification': {'successful': bool(runs) and all(r['exit_code'] == 0 for r in runs)},
            'summary': counts(methods), 'families': groups, 'methods': methods, 'runs': runs,
            'differential': load_measurement('differential'),
            'performance': load_measurement('performance'),
            'solver_differential': load_measurement('solver-differential'),
            'list_differential': load_measurement('list-differential'),
            'solver_performance': load_measurement('solver-performance'),
            'mixed_performance': load_measurement('mixed-performance'),
            'heuristic_performance': load_measurement('heuristic-performance'),
            'repair_performance': load_measurement('repair-performance'),
            'list_repair_performance': load_measurement('list-repair-performance'),
            'construction_performance': load_measurement('construction-performance'),
            'list_performance': load_measurement('list-performance'),
            'kopt_performance': load_measurement('kopt-performance'),
            'stream_differential': load_measurement('stream-differential'),
            'stream_performance': load_measurement('stream-performance'),
            'concurrent_performance': load_measurement('concurrent-performance'),
            'sequence_performance': load_measurement('sequence-performance'),
            'aggregate_performance': load_measurement('aggregate-performance'),
            'pillar_performance': load_measurement('pillar-performance')}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inventory-only', action='store_true', help='Do not execute mapped tests; they remain unverified')
    parser.add_argument('--test-output', type=Path, help='Use captured all-target Cargo results instead of rerunning tests')
    args = parser.parse_args()
    report = build_report(not args.inventory_only, args.test_output)
    target = ROOT / 'dashboard/data/report.json'
    target.parent.mkdir(exist_ok=True, parents=True)
    temporary = target.with_suffix('.tmp')
    temporary.write_text(json.dumps(report, indent=2) + '\n')
    temporary.replace(target)
    print(json.dumps(report['summary']))
    print(f'Dashboard data: {target.relative_to(ROOT)}')
    return 1 if report['summary']['failed'] or any(r['exit_code'] for r in report['runs']) else 0

if __name__ == '__main__':
    raise SystemExit(main())
