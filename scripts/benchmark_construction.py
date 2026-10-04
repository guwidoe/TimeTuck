#!/usr/bin/env python3
"""Measure three construction strategies, validating assignments and quality separately."""
import argparse
from datetime import datetime, timezone
import json
import platform
import random
import statistics
import subprocess
from differential_scores import ROOT, REF, WORK, source_digest
from differential_solver import encode, score, optimum, validate
from value_queue_construction_checks import generate_cases, reference as queued_reference
from pooled_construction_checks import reference as pooled_reference
from prepare_solver_oracle import prepare_solver_java
from benchmark_scores import version

MODES = {'sequential': 'Sequential', 'pooled': 'Pooled cheapest insertion', 'value-queue': 'Cycling value queue'}
SEED = 20261221


def reference(case, mode):
    if mode == 'pooled': return pooled_reference(case)
    if mode == 'value-queue': return queued_reference(case)
    if mode != 'sequential': raise ValueError('Unknown construction mode')
    if 'domains' in case or 'optional' in case: raise ValueError('Requires homogeneous required domains')
    if case['value_count'] < 1: raise ValueError('Empty required domain')
    values = [v if pin else None for v, pin in zip(case['initial'], case['pins'])]
    for i, current in enumerate(values):
        if current is not None: continue
        if case['pins'][i]: raise ValueError('Missing required pinned value')
        trials = [values[:i]+[v]+values[i+1:] for v in range(case['value_count'])]
        values = max(trials, key=lambda v: score(case, v))
    return values, score(case, values)


def parse_result(case, line, optimal=None):
    fields = line.split('\t')
    if len(fields) != 6 or fields[0] != case['id']: raise ValueError('Response identity/shape mismatch')
    elapsed, hard, soft, calls = map(int, [fields[1], fields[2], fields[3], fields[5]])
    if elapsed <= 0 or calls < 1: raise ValueError('Invalid elapsed time or score-call count')
    error = validate(case, '\t'.join([fields[0], fields[2], fields[3], fields[4]]), (hard, soft))
    if error: raise ValueError(error)
    if optimal is not None and (hard, soft) > tuple(optimal): raise ValueError('Score exceeds independent optimum')
    return dict(elapsed_ns=elapsed, score=[hard, soft], values=list(map(int, fields[4].split(','))), score_calls=calls)


def quality_summary(rows):
    return dict(total=len(rows), passed=len(rows),
                equal_scores=sum(r['java']['score'] == r['rust']['score'] for r in rows),
                equal_assignments=sum(r['java']['values'] == r['rust']['values'] for r in rows),
                native_reference_passed=sum(r['rust']['values'] == r['reference_values'] for r in rows),
                optimal={name: sum(r[name]['score'] == r['optimal_score'] for r in rows) for name in ['java', 'rust']})


def workload(n):
    rng = random.Random(SEED+n)
    return dict(id=f'construction-{n}', value_count=4,
                initial=[i % 4 if i % 17 == 0 else None for i in range(n)], pins=[i % 17 == 0 for i in range(n)],
                edges=[(i, j) for i in range(n) for j in range(i+1, n) if i % 4 != j % 4 and rng.random() < 0.3],
                weights=[[0 if v == i % 4 else -rng.randint(1, 12) for v in range(4)] for i in range(n)])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--warmup', type=int, default=5)
    parser.add_argument('--samples', type=int, default=7)
    parser.add_argument('--forks', type=int, default=3)
    args = parser.parse_args()
    if min(args.samples, args.forks) < 1 or args.warmup < 0 or args.warmup+args.samples > 2_147_483_647:
        parser.error('Invalid sample counts')
    classpath, artifacts = prepare_solver_java()
    subprocess.run(['javac', '-cp', classpath, '-d', str(WORK/'solver'), str(ROOT/'compat/oracle/ConstructionBenchmark.java')], check=True)
    subprocess.run(['cargo', 'build', '--release', '--locked', '--example', 'construction_benchmark'], cwd=ROOT, check=True)
    digest = source_digest()
    commands = {'java': ['java', '-Xms256m', '-Xmx256m', '-cp', classpath, 'ConstructionBenchmark'],
                'rust': [str(ROOT/'target/release/examples/construction_benchmark')]}
    def run(runtime, mode, cases, warmup, samples, optima):
        result = subprocess.run([*commands[runtime], mode, str(warmup), str(samples)],
                                input='\n'.join(map(encode, cases))+'\n', capture_output=True, text=True, timeout=180)
        if result.returncode: raise RuntimeError(f'{runtime} {mode}: {result.stderr}')
        lines = result.stdout.splitlines()
        if len(lines) != len(cases)*samples: raise ValueError('Response count mismatch')
        output = []
        for i, c in enumerate(cases):
            rows = [parse_result(c, line, optima[c['id']]) for line in lines[i*samples:(i+1)*samples]]
            expected_values, expected_score = reference(c, mode)
            if runtime == 'rust' and any(r['values'] != expected_values or r['score'] != list(expected_score) for r in rows):
                raise ValueError('Native construction disagrees with independent reference')
            output.append(rows)
        return output
    cases = generate_cases()
    optima = {c['id']: optimum(c) for c in cases}
    checks = []
    for mode in MODES:
        runs = {r: run(r, mode, cases, 0, 1, optima) for r in commands}
        for i, c in enumerate(cases):
            values, _ = reference(c, mode)
            checks.append(dict(case=c, mode=mode, reference_values=values, optimal_score=list(optima[c['id']]),
                               **{r: runs[r][i][0] for r in commands}))
    summary = quality_summary(checks)
    print(f'Construction validation: {summary}', flush=True)
    workloads = []
    for mode_index, mode in enumerate(MODES):
        for index, size in enumerate([32, 96]):
            c = workload(size)
            # Each contribution is nonpositive; the planted coloring attains zero.
            if score(c, [i % 4 for i in range(size)]) != (0, 0): raise ValueError('Invalid planted optimum')
            measurements = {r: [] for r in commands}
            for fork in range(args.forks):
                order = ['java', 'rust'] if (mode_index+index+fork) % 2 == 0 else ['rust', 'java']
                for runtime in order:
                    measurements[runtime].extend(run(runtime, mode, [c], args.warmup, args.samples, {c['id']: (0, 0)})[0])
            entry = dict(name=f'{MODES[mode]} · {size} variables', algorithm=mode, case=c, unit='ms / solve',
                         optimal_score=[0, 0], samples=measurements)
            for runtime, rows in measurements.items():
                if len({(tuple(r['score']), tuple(r['values'])) for r in rows}) != 1: raise ValueError('Construction varies across runs')
                times = [r['elapsed_ns']/1e6 for r in rows]
                entry[runtime] = dict(median=statistics.median(times), min=min(times), max=max(times), score=rows[0]['score'],
                                      score_calls=dict(min=min(r['score_calls'] for r in rows), max=max(r['score_calls'] for r in rows)))
            entry['equal_scores'] = entry['java']['score'] == entry['rust']['score']
            entry['equal_assignments'] = measurements['java'][0]['values'] == measurements['rust'][0]['values']
            entry['java_over_rust'] = entry['java']['median']/entry['rust']['median']
            workloads.append(entry)
            print(f'{entry["name"]}: Rust {entry["rust"]["median"]:.3f}, Java {entry["java"]["median"]:.3f} ms; scores {entry["rust"]["score"]} / {entry["java"]["score"]}', flush=True)
    if source_digest() != digest: raise RuntimeError('Sources changed during measurement')
    report = dict(schema_version=1, generated_at=datetime.now(timezone.utc).isoformat(), source_digest=digest,
                  reference=REF, artifacts=artifacts, config={**vars(args), 'seed': SEED, 'validation_seed': 20261001, 'iterations': 1, 'algorithms': list(MODES)},
                  workloads=workloads, validation={**summary, 'by_algorithm': {m: quality_summary([r for r in checks if r['mode'] == m]) for m in MODES}, 'cases': checks},
                  environment=dict(cpu=next((s.split(':', 1)[1].strip() for s in open('/proc/cpuinfo') if s.startswith('model name')), 'unknown'),
                                   platform=platform.platform(), rust=version(['rustc', '--version']), java=version(['java', '-version'])),
                  methodology='Three basic-variable construction strategies with original entity/value order and NEVER early picks: sequential best insertion (Java ALLOCATE_ENTITY_FROM_QUEUE), pooled cheapest insertion (CHEAPEST_INSERTION), and cycling value queue (ALLOCATE_TO_VALUE_FROM_QUEUE). Homogeneous required graph-coloring variables, pins and signed preferences; movable initial values reset in both adapters. Full scoring in both engines: Java EasyScoreCalculator and one global Rust constraint. Timed per-solve setup, initialization, construction, and returned assignment/score validation; excludes process startup, parsing, model/network/factory creation and output encoding. Warmups and alternating runtime order across fresh processes. Score-call counts include solver scoring only, not the final validation recomputation; counts need not match across engines. Every output is independently checked for domains, pins, completeness and score. Every native output also matches Python greedy reconstruction. 67 small cases per strategy have enumerated optima; the 32/96-variable timing cases have planted optima of zero because all score contributions are nonpositive. Equal final scores/assignments and optimum attainment are reported separately. No identical trace, incremental/stream/list/mixed/optional-variable or general solver performance claim. Shared-host ranges include allocation and GC effects.')
    (ROOT/'work/construction-performance.json').write_text(json.dumps(report, indent=2)+'\n')


if __name__ == '__main__': main()
