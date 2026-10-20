#!/usr/bin/env python3
"""Separate public application workload evidence; never upstream-method credit."""
import argparse
import math
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
import time

from real_world_reference import initial_schedule, score_schedule, validate_reported_score
from verification_inputs import ROOT, digest, rust_entry_inputs, selected_library_inputs

CASES = ('social-golfer-32x8x10-v1', 'sailing-trip-v1')
SEEDS = (7, 11, 23)
DEFAULT_SECONDS = 10.0
FIXTURES = ROOT / 'tests/application/real_world'
APPLICATION = ROOT / 'benchmarks/real_world'
DEPENDENCIES = ROOT / 'work/oracle/solver/dependencies'
DEFAULT_REPORT = ROOT / 'work/real_world-performance.json'


def real_world_inputs(root=ROOT):
    entries = rust_entry_inputs('crates/solver/examples/real_world_benchmark.rs', root)
    own = {str(p.relative_to(root)) for base in ('benchmarks/real_world', 'tests/application/real_world')
           for p in (root / base).rglob('*') if p.is_file() and '__pycache__' not in p.parts}
    own |= {'scripts/benchmark_real_world.py', 'scripts/real_world_reference.py',
            'scripts/verification_inputs.py', 'compat/upstream.json'}
    return own | entries | selected_library_inputs(entries, root)


def real_world_digest(root=ROOT):
    return digest(real_world_inputs(root), root)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def execute(command, *, timeout=180):
    started = time.monotonic_ns()
    try:
        result = subprocess.run(list(map(str, command)), cwd=ROOT, capture_output=True,
                                text=True, timeout=timeout)
        return {'command': list(map(str, command)), 'exit_code': result.returncode,
                'wall_elapsed_ns': time.monotonic_ns() - started,
                'stdout': result.stdout, 'stderr': result.stderr}
    except subprocess.TimeoutExpired as exc:
        def decoded(value):
            return value.decode(errors='replace') if isinstance(value, bytes) else value or ''
        return {'command': list(map(str, command)), 'exit_code': None, 'error': 'timeout',
                'wall_elapsed_ns': time.monotonic_ns() - started,
                'stdout': decoded(exc.stdout), 'stderr': decoded(exc.stderr)}
    except OSError as exc:
        return {'command': list(map(str, command)), 'exit_code': None, 'error': str(exc),
                'wall_elapsed_ns': time.monotonic_ns() - started, 'stdout': '', 'stderr': ''}


def version(command):
    result = execute(command, timeout=30)
    return {'command': result['command'], 'exit_code': result['exit_code'],
            'output': result['stdout'] + result['stderr']}


def prep_module():
    spec = importlib.util.spec_from_file_location('real_world_prep', APPLICATION / 'prep.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def normalized_schedule(schedule):
    return {session: {group: sorted(people) for group, people in groups.items()}
            for session, groups in schedule.items()}


def java_schedule(problem, path):
    people = [p['id'] for p in problem['scenario']['people']]
    groups = [g['id'] for g in problem['scenario']['groups']]
    count = problem['scenario']['session_count']
    result = {f'session_{s}': {g: [] for g in groups} for s in range(count)}
    for row in path.read_text().splitlines():
        p, s, g = map(int, row.split())
        if not (0 <= p < len(people) and 0 <= g < len(groups) and 0 <= s < count):
            raise ValueError('Java assignment index outside scenario')
        result[f'session_{s}'][groups[g]].append(people[p])
    return result


def summarize(runs, cases):
    summaries = []
    for case in cases:
        runtimes = {}
        for runtime in sorted({r['runtime'] for r in runs if r['case'] == case}):
            rows = [r for r in runs if r['case'] == case and r['runtime'] == runtime]
            valid = [r for r in rows if r.get('valid')]
            runtimes[runtime] = {
                'runs': len(rows), 'valid_runs': len(valid),
                'median_evaluations_per_second': statistics.median(r['evaluations_per_second'] for r in valid) if valid else None,
                'scores': [r.get('validation', {}).get('score') for r in rows],
                'feasible_runs': sum(r.get('validation', {}).get('feasible', False) for r in rows),
                'median_elapsed_seconds': statistics.median(r['reported']['elapsed_ns'] / 1e9 for r in valid) if valid else None}
        rust = runtimes.get('rust-streams', {}).get('median_evaluations_per_second')
        java = runtimes.get('java-streams', {}).get('median_evaluations_per_second')
        complete = all(r.get('valid') for r in runs if r['case'] == case)
        ratio = rust / java if rust is not None and java else None
        summaries.append({'case': case, 'runtimes': runtimes,
                          'rust_over_java_streams_evaluations_per_second': ratio,
                          'throughput_target_met': complete and ratio is not None and ratio >= 1,
                          'quality_equal_gated': False,
                          'quality_note': 'Raw hard/soft scores are separate observations. Sailing lacks whole keep-together moves; hard annealing and cooling semantics differ.'})
    return summaries


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--seconds', type=float, default=DEFAULT_SECONDS)
    parser.add_argument('--seeds', type=int, nargs='+', default=list(SEEDS))
    parser.add_argument('--cases', choices=CASES, nargs='+', default=list(CASES))
    parser.add_argument('--java-modes', choices=('streams', 'incremental'), nargs='+', default=['streams'])
    parser.add_argument('--cpus', help='Comma-separated CPUs; default 2,3 where allowed, else first two allowed CPUs')
    parser.add_argument('--no-build', action='store_true', help='Use already-built artifacts, recording their hashes')
    parser.add_argument('--rust-binary', type=Path, default=ROOT / 'target/release/examples/real_world_benchmark')
    parser.add_argument('--report', type=Path, default=DEFAULT_REPORT)
    args = parser.parse_args(argv)
    if not math.isfinite(args.seconds) or args.seconds <= 0 or len(set(args.seeds)) != len(args.seeds) or len(set(args.cases)) != len(args.cases):
        parser.error('seconds must be positive, seeds and cases must be unique')
    if len(set(args.java_modes)) != len(args.java_modes):
        parser.error('Java modes must be unique')
    if 'streams' not in args.java_modes:
        parser.error('actual Timefold streams is required; incremental is an optional separate comparator')
    available = sorted(os.sched_getaffinity(0)) if hasattr(os, 'sched_getaffinity') else []
    cpus = [int(n) for n in args.cpus.split(',')] if args.cpus else ([2, 3] if {2, 3} <= set(available) else available[:2])
    if any(cpu not in available for cpu in cpus):
        parser.error('requested CPUs are outside process affinity')
    affinity = ['taskset', '-c', ','.join(map(str, cpus))] if cpus and shutil.which('taskset') else []
    if cpus and not affinity:
        parser.error('taskset required for selected CPU affinity')
    before = real_world_digest()
    stamp = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
    run_dir = ROOT / 'work/real_world-runs' / stamp
    run_dir.mkdir(parents=True)
    classes = ROOT / 'work/real_world-java/classes'
    classes.mkdir(parents=True, exist_ok=True)
    jars = sorted(DEPENDENCIES.glob('*.jar'))
    classpath = os.pathsep.join(map(str, [classes, *jars]))
    report = {'schema_version': 1, 'kind': 'application_benchmark', 'name': 'Public RealWorld workloads',
              'upstream_method_credit': 0, 'input_sha256': before,
              'generated_at': datetime.now(timezone.utc).isoformat(), 'run_directory': str(run_dir),
              'configuration': {'cases': args.cases, 'seeds': args.seeds, 'seconds': args.seconds,
                                'java_modes': args.java_modes, 'cpu_affinity': cpus,
                                'scale': 100, 'rust': {'acceptor': 'simulated_annealing', 'soft_temperature': 500, 'cooling_steps': 40000},
                                'java': {'acceptor': 'simulated_annealing', 'temperature': '0hard/500soft',
                                         'accepted_count_limit': 1, 'environment_mode': 'NO_ASSERT',
                                         'termination': 'milliseconds spent', 'heap': '-Xmx1g'},
                                'move_families': ['capacity-filtered change', 'same-session swap'],
                                'initialization': 'fixed first, max spare capacity, last group on ties',
                                'attribute_balance': 'occupied groups only, retained application spike policy',
                                'pairing': 'same-machine serial pairs, rotating runtime order by seed index'},
              'machine': {'platform': platform.platform(), 'architecture': platform.machine(),
                          'hostname': platform.node(), 'python': sys.version, 'allowed_cpus': available,
                          'cpuinfo': Path('/proc/cpuinfo').read_text() if Path('/proc/cpuinfo').exists() else None,
                          'tools': [version(c) for c in (['rustc', '-Vv'], ['cargo', '-V'], ['java', '-version'], ['javac', '-version'])]},
              'inputs': {}, 'artifacts': {}, 'builds': [], 'runs': [], 'errors': []}
    if not jars or not (DEPENDENCIES / 'timefold-solver-core-2.7.0.jar').exists():
        report['errors'].append('Pinned Community 2.7.0 jars unavailable; prepare_solver_oracle must provision them')
    if not args.no_build and not report['errors']:
        for command in (['cargo', 'build', '--release', '--locked', '--example', 'real_world_benchmark'],
                        ['javac', '-cp', classpath, '-d', str(classes), str(APPLICATION / 'ApplicationBenchmark.java')]):
            build = execute(command, timeout=1800)
            report['builds'].append(build)
            if build['exit_code'] != 0:
                report['errors'].append('artifact build failed')
                break
    if args.rust_binary.is_file():
        report['artifacts']['rust'] = {'path': str(args.rust_binary), 'sha256': sha(args.rust_binary)}
    else:
        report['errors'].append('Rust executable unavailable')
    classfiles = sorted(classes.glob('ApplicationBenchmark*.class'))
    if not classfiles:
        report['errors'].append('Java classes unavailable')
    report['artifacts']['java_classes'] = {str(p): sha(p) for p in classfiles}
    report['artifacts']['java_dependencies'] = {str(p): sha(p) for p in jars}
    artifact_binding = ROOT / 'work/real_world-java/build-receipt.json'
    if not args.no_build and not report['errors'] and real_world_digest() == before:
        artifact_binding.write_text(json.dumps({'input_sha256': before, 'artifacts': report['artifacts']}, indent=2) + '\n')
    if args.no_build:
        try:
            binding = json.loads(artifact_binding.read_text())
            if binding != {'input_sha256': before, 'artifacts': report['artifacts']}:
                report['errors'].append('Prebuilt artifacts do not match the current source build receipt')
        except (OSError, ValueError):
            report['errors'].append('Prebuilt artifacts lack a current source build receipt; run without --no-build')
    report['artifacts_source_bound'] = not report['errors']
    prep = prep_module()
    for case in args.cases:
        problem_path = FIXTURES / case / 'problem.json'
        problem = json.loads(problem_path.read_text())
        start = initial_schedule(problem)
        initial = score_schedule(problem, start)
        start_path = run_dir / f'{case}-initial.json'
        start_path.write_text(json.dumps({'incumbent_schedule': start}, indent=2) + '\n')
        model_path = run_dir / f'{case}-model.txt'
        prep.model(str(problem_path), str(model_path))
        # Verify the flattened actual Java initial assignments, independently.
        flattened = model_path.read_text().splitlines()
        offset = 2 + problem['scenario']['session_count']
        nv = int(flattened[offset])
        flat_start = run_dir / f'{case}-initial-java.txt'
        flat_start.write_text('\n'.join(' '.join(row.split()[:3]) for row in flattened[offset+1:offset+1+nv]) + '\n')
        if normalized_schedule(java_schedule(problem, flat_start)) != normalized_schedule(start):
            report['errors'].append(f'{case}: Java model initial schedule differs')
        report['inputs'][case] = {'problem_sha256': sha(problem_path), 'model_sha256': sha(model_path),
                                  'manifest_sha256': sha(FIXTURES / case / 'manifest.json'),
                                  'initial_schedule_sha256': sha(start_path), 'initial_validation': initial}
        if initial['structural_errors'] or initial['pin_violations']:
            report['errors'].append(f'{case}: invalid initial schedule')
        if report['errors']:
            continue
        for seed_index, seed in enumerate(args.seeds):
            runtimes = ['rust-streams', *('java-' + mode for mode in args.java_modes)]
            runtimes = runtimes[seed_index % len(runtimes):] + runtimes[:seed_index % len(runtimes)]
            for runtime in runtimes:
                suffix = 'json' if runtime == 'rust-streams' else 'txt'
                output = run_dir / f'{case}-{seed}-{runtime}.{suffix}'
                if runtime == 'rust-streams':
                    command = [*affinity, str(args.rust_binary), str(problem_path), str(args.seconds), str(seed), str(output)]
                else:
                    command = [*affinity, 'java', '-Xmx1g', '-cp', classpath, 'ApplicationBenchmark', str(model_path),
                               runtime.removeprefix('java-'), 'sa', str(args.seconds), str(seed), str(output), '500']
                run = execute(command, timeout=max(120, args.seconds * 5))
                run.update(case=case, seed=seed, runtime=runtime, assignment_path=str(output), valid=False)
                try:
                    if run['exit_code'] != 0:
                        raise ValueError('solver process failed')
                    reported = json.loads(run['stdout'].strip())
                    run['reported'] = reported
                    if type(reported['evaluations']) is not int or reported['evaluations'] < 0 or reported['elapsed_ns'] <= 0:
                        raise ValueError('Invalid solver work/time diagnostics')
                    initial_matches = True
                    if runtime == 'rust-streams':
                        exported = json.loads(output.read_text())
                        schedule = exported['incumbent_schedule']
                        initial_matches = (normalized_schedule(exported['benchmark_initial_schedule'])
                                           == normalized_schedule(start))
                    else:
                        schedule = java_schedule(problem, output)
                        converted = run_dir / f'{case}-{seed}-{runtime}.json'
                        converted.write_text(json.dumps({**problem, 'incumbent_schedule': schedule,
                                                       'solver_settings': json.loads((FIXTURES / 'solver-settings.json').read_text())}, indent=2) + '\n')
                        run['converted_assignment_path'] = str(converted)
                    validation = validate_reported_score(problem, schedule, reported['best_score'])
                    validation['initial_score_matches'] = reported['initial_score'] == initial['score']
                    validation['initial_schedule_matches'] = initial_matches
                    validation['valid'] &= validation['initial_score_matches'] and initial_matches
                    run['validation'] = validation
                    run['valid'] = validation['valid']
                    run['evaluations_per_second'] = reported['evaluations'] / (reported['elapsed_ns'] / 1e9)
                    run['assignment_sha256'] = sha(output)
                    if not run['valid']:
                        run['error'] = 'independent score, structure, pin or initial-score validation failed'
                except (ValueError, KeyError, TypeError, OSError, ZeroDivisionError) as exc:
                    run['error'] = str(exc)
                report['runs'].append(run)
                (run_dir / f'{case}-{seed}-{runtime}-receipt.json').write_text(json.dumps(run, indent=2) + '\n')
                print(f"{case} seed {seed} {runtime}: {'valid' if run['valid'] else run.get('error')}", flush=True)
    report['summaries'] = summarize(report['runs'], args.cases)
    report['source_unchanged'] = real_world_digest() == before
    if not report['source_unchanged']:
        report['errors'].append('Relevant source changed during measurement')
    report['valid'] = (not report['errors'] and bool(report['runs']) and all(r['valid'] for r in report['runs'])
                       and len(report['runs']) == len(args.cases) * len(args.seeds) * (1 + len(args.java_modes)))
    report['qualified'] = (report['valid'] and set(args.cases) == set(CASES) and set(args.seeds) == set(SEEDS)
                           and args.seconds == DEFAULT_SECONDS)
    report['qualification_note'] = 'Qualification means complete default application measurements with independent scoring, never native/Java parity or upstream coverage.'
    report['limits'] = ['Move evaluation counts describe each runtime work, not identical work.',
                        'Search traces and random streams differ despite shared numeric seeds.',
                        'Native step cooling differs from Timefold wall-time cooling and hard-score annealing.',
                        'Whole keep-together moves are absent; sailing quality is not equal-gated.',
                        'Optional Java incremental is a separate hand-written domain-cache comparator.',
                        'RealWorld solver_settings are retained export metadata; they do not configure these solvers.']
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(str(args.report))
    return 0 if report['valid'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
