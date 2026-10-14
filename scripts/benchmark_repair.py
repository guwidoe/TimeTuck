#!/usr/bin/env python3
"""Compare bounded basic-variable repair quality and time with explicit work counts."""
from verification_inputs import measurement_digest
from verification_cache import run_measurement
import argparse
from datetime import datetime, timezone
import json
import platform
import random
import statistics
import subprocess
from differential_scores import ROOT, REF, WORK
from differential_solver import encode, score, optimum
from benchmark_construction import parse_result as parse_solution
from benchmark_heuristics import quality_summary
from prepare_solver_oracle import prepare_solver_java
from benchmark_scores import version

SEED = 20261222
MODES = {'half': 'Half-group repair', 'all': 'Whole-group repair'}


def workload(n, index=0, k=4):
    rng = random.Random(SEED+n*101+index)
    planted = [1+i % (k-1) for i in range(n)]
    return dict(id=f'repair-{n}-{index}', value_count=k, initial=[0]*n, pins=[False]*n,
                edges=[(i, j) for i in range(n) for j in range(i+1, n)
                       if planted[i] != planted[j] and rng.random() < 0.4],
                weights=[[-12]+[0 if v == planted[i] else -rng.randint(1, 11) for v in range(1, k)] for i in range(n)])


def generate_cases():
    rng = random.Random(SEED)
    cases = []
    for i in range(32):
        n, k = 2+i % 5, 3
        cases.append(dict(id=f'repair-quality-{i}', value_count=k, initial=[0]*n, pins=[False]*n,
                          edges=[(a, b) for a in range(n) for b in range(a+1, n) if rng.random() < 0.6],
                          weights=[[-12]+[rng.randint(-11, 0) for _ in range(k-1)] for _ in range(n)]))
    return cases


def group_size(case, mode):
    if mode not in MODES: raise ValueError('Unknown repair mode')
    return len(case['initial']) if mode == 'all' else (len(case['initial'])+1)//2


def validate_scope(case):
    if ('domains' in case or 'optional' in case or case['value_count'] < 2 or not case['initial']
            or any(v != 0 for v in case['initial']) or any(case['pins'])
            or any(w[0] != min(w) for w in case['weights'])):
        raise ValueError('Requires homogeneous required unpinned variables with worst all-zero initial score')


def parse_result(case, line, steps, optimal):
    fields = line.split('\t')
    if len(fields) != 7: raise ValueError('Response shape mismatch')
    result = parse_solution(case, '\t'.join(fields[:6]), optimal)
    repairs = int(fields[6])
    if steps < 1 or repairs != steps: raise ValueError('Incomplete or excess repair work')
    if tuple(result['score']) < score(case, case['initial']): raise ValueError('Regressed retained score')
    return dict(result, repairs=repairs)


def validate_scoring_equivalence(incremental, full):
    for key in ('score', 'values', 'score_calls', 'repairs'):
        if incremental[key] != full[key]:
            raise ValueError(f'Native scoring models disagree on {key}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--steps', type=int, default=8)
    parser.add_argument('--warmup', type=int, default=5)
    parser.add_argument('--samples', type=int, default=7)
    parser.add_argument('--forks', type=int, default=3)
    parser.add_argument('--ruin-sampling', choices=['independent', 'coverage-cycle'], default='independent',
                        help='Native repair sampling; Java retains its public random selector')
    parser.add_argument('--construction-placement', choices=['sequential', 'cheapest'], default='sequential',
                        help='Native reconstruction placement; Java retains its public default')
    parser.add_argument('--native-scoring', choices=['full', 'incremental'], default='full',
                        help='Native scoring model; incremental also measures a fresh full-scoring baseline')
    args = parser.parse_args()
    if (min(args.steps, args.samples, args.forks) < 1 or args.warmup < 0
            or args.steps >= 2_147_483_647 or args.warmup+args.samples > 2_147_483_647):
        parser.error('Invalid run counts')
    classpath, artifacts = prepare_solver_java()
    subprocess.run(['javac', '-cp', classpath, '-d', str(WORK/'solver'), str(ROOT/'compat/oracle/RepairBenchmark.java')], check=True)
    subprocess.run(['cargo', 'build', '--release', '--locked', '--example', 'repair_benchmark'], cwd=ROOT, check=True)
    digest = measurement_digest('repair-performance')
    commands = {'java': ['java', '-Xms256m', '-Xmx256m', '-cp', classpath, 'RepairBenchmark'],
                'rust': [str(ROOT/'target/release/examples/repair_benchmark')]}
    if args.native_scoring == 'incremental':
        commands['rust_full'] = commands['rust']

    def run(runtime, mode, case, warmup, samples, optimal):
        validate_scope(case)
        command = [*commands[runtime], str(group_size(case, mode)), str(args.steps), str(warmup), str(samples)]
        if runtime != 'java':
            command.extend([args.ruin_sampling, args.construction_placement,
                            'full' if runtime == 'rust_full' else args.native_scoring])
        result = subprocess.run(command,
                                input=encode(case)+'\n', capture_output=True, text=True, timeout=180)
        if result.returncode: raise RuntimeError(f'{runtime} {mode} {case["id"]}: {result.stderr}')
        lines = result.stdout.splitlines()
        if len(lines) != samples: raise ValueError('Response count mismatch')
        return [parse_result(case, line, args.steps, optimal) for line in lines]

    cases = generate_cases()
    checks = []
    for mode in MODES:
        for case in cases:
            optimal = optimum(case)
            row = dict(case=case, algorithm=mode, optimal_score=list(optimal),
                       **{r: run(r, mode, case, 0, 1, optimal)[0] for r in commands})
            if 'rust_full' in row:
                validate_scoring_equivalence(row['rust'], row['rust_full'])
            checks.append(row)
    summary = quality_summary(checks)
    print(f'Repair validation: {summary}', flush=True)
    workloads = []
    for mode_index, mode in enumerate(MODES):
        for index, size in enumerate([32, 96]):
            case = workload(size)
            if score(case, [1+i % 3 for i in range(size)]) != (0, 0): raise ValueError('Invalid planted optimum')
            measurements = {r: [] for r in commands}
            for fork in range(args.forks):
                if 'rust_full' in commands:
                    runtimes = list(commands)
                    offset = (mode_index+index+fork) % len(runtimes)
                    order = runtimes[offset:]+runtimes[:offset]
                else:
                    order = ['java', 'rust'] if (mode_index+index+fork) % 2 == 0 else ['rust', 'java']
                for runtime in order:
                    measurements[runtime].extend(run(runtime, mode, case, args.warmup, args.samples, (0, 0)))
            policy_label = f' · Rust {args.ruin_sampling} / {args.construction_placement}' if (args.ruin_sampling, args.construction_placement) != ('independent', 'sequential') else ''
            if args.native_scoring == 'incremental':
                policy_label += ' · incremental scoring'
            entry = dict(name=f'{MODES[mode]} · {size} variables{policy_label}', algorithm=mode, case=case,
                         ruined_count=group_size(case, mode), unit='ms / solve', optimal_score=[0, 0], samples=measurements)
            for runtime, rows in measurements.items():
                if len({(tuple(r['score']), tuple(r['values'])) for r in rows}) != 1:
                    raise ValueError('Seeded repair results vary across runs')
                times = [r['elapsed_ns']/1e6 for r in rows]
                entry[runtime] = dict(median=statistics.median(times), min=min(times), max=max(times), score=rows[0]['score'],
                                     score_calls=dict(min=min(r['score_calls'] for r in rows), max=max(r['score_calls'] for r in rows)),
                                     repairs=dict(min=min(r['repairs'] for r in rows), max=max(r['repairs'] for r in rows)))
            entry['equal_scores'] = entry['java']['score'] == entry['rust']['score']
            entry['java_over_rust'] = entry['java']['median']/entry['rust']['median']
            if 'rust_full' in measurements:
                for incremental, full in zip(measurements['rust'], measurements['rust_full']):
                    validate_scoring_equivalence(incremental, full)
                entry['full_over_incremental'] = entry['rust_full']['median']/entry['rust']['median']
            workloads.append(entry)
            print(f'{entry["name"]}: Rust {entry["rust"]["median"]:.3f}, Java {entry["java"]["median"]:.3f} ms; scores {entry["rust"]["score"]} / {entry["java"]["score"]}', flush=True)
    if measurement_digest('repair-performance') != digest: raise RuntimeError('Sources changed during measurement')
    report = dict(schema_version=1, generated_at=datetime.now(timezone.utc).isoformat(), source_digest=digest,
                  reference=REF, artifacts=artifacts, config={**vars(args), 'seed': SEED, 'iterations': 1, 'algorithms': list(MODES)},
                  workloads=workloads, validation={**summary, 'by_algorithm': {m: quality_summary([r for r in checks if r['algorithm'] == m]) for m in MODES}, 'cases': checks},
                  environment=dict(cpu=next((s.split(':', 1)[1].strip() for s in open('/proc/cpuinfo') if s.startswith('model name')), 'unknown'),
                                   platform=platform.platform(), rust=version(['rustc', '--version']), java=version(['java', '-version'])),
                  methodology=f'Native sampling: {args.ruin_sampling}; native reconstruction: {args.construction_placement}; native scoring: {args.native_scoring}. Java retains its public independent random selector, reconstruction defaults and EasyScoreCalculator full scoring. '+
                  ('Native incremental scoring uses scoped edge/weight callbacks. A separate fresh native full-scoring baseline retains every assignment, score, logical evaluation count and repair count; all baseline samples are retained. This is a scoring-model optimization, not a change to pooled candidate selection or fewer logical evaluations. Native score_calls counts candidate score evaluations, not individual scoped constraint callbacks. ' if args.native_scoring == 'incremental' else 'Both engines use full scoring. ')+
                  'Original homogeneous required basic graph-coloring cases, no pins, all-zero initial assignment attaining the worst possible score. Fixed half or whole groups; seed 20261222 in each runtime, with different PRNGs and reconstruction orders. Java public RuinRecreateMoveSelector with acceptedCountLimit=1 and completed-step cap; native standalone repair with attempt cap and the explicitly reported reconstruction placement. Cheapest insertion rescans remaining unassigned variables and has a larger finite score budget; both configurations report actual scoring work. Step-counting hill climbing uses STEP mode with period steps+1, so every complete candidate is at least the unchanged initial threshold during this bounded run. Each solve must report exactly the requested repairs: Java public MOVE_EVALUATION_COUNT gauge, native attempts and complete_reconstructions, including no-ops. Work per repair differs; actual score calls are shown, not asserted equal. Timing includes per-solve setup, initial state copying, initialization, search and returned solution validation; excludes process startup, parsing, model/network/factory creation, registry setup, final metric reads and output encoding. Java default metric collection remains enabled during timing. Warmups and balanced runtime order across fresh processes; all raw samples retained. Every output independently checked for complete domain-valid assignments, recomputed score, initial non-regression and optimum upper bound. 32 small cases per group mode have exhaustively enumerated optima; timing cases have proven planted zero optima and nonpositive contributions. Different final quality is retained, not discarded. Ratios are observed runtime comparisons of these bounded configurations, not equal scoring work or scoring architecture, identical traces, general solver speed or list/mixed/optional/stream performance. Original cases earn no upstream-method credit; shared-host ranges include allocation and GC effects.')
    (ROOT/'work/repair-performance.json').write_text(json.dumps(report, indent=2)+'\n')


if __name__ == '__main__':
    raise SystemExit(run_measurement('repair-performance', main))
