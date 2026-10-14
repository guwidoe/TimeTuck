#!/usr/bin/env python3
"""Compare required/optional list repair validity, quality and bounded timing."""
from verification_inputs import measurement_digest
from verification_cache import run_measurement
import argparse
from datetime import datetime, timezone
from itertools import permutations, product
import json
import platform
import random
import statistics
import subprocess
from differential_scores import ROOT, REF, WORK
from prepare_solver_oracle import prepare_solver_java
from benchmark_scores import version
from benchmark_heuristics import quality_summary

SEED = 20261225
MODES = ('half', 'all')


def encode(c):
    return '\t'.join([c['id'], str(int(c['optional'])), ','.join(map(str, c['caps'])),
                      ';'.join(','.join(map(str, row)) for row in c['costs']), ','.join(map(str, c['missing']))])


def initial(c): return [list(range(len(c['costs'])))]+[[] for _ in c['caps'][1:]]


def score(c, lists):
    assigned = {v for row in lists for v in row}
    cost = sum(max(0, len(row)-cap)**2 for row, cap in zip(lists, c['caps']))
    cost += sum(c['costs'][v][e] for e, row in enumerate(lists) for v in row)
    cost += sum(2 for row in lists for a, b in zip(row, row[1:]) if a < b)
    cost += sum(penalty for v, penalty in enumerate(c['missing']) if v not in assigned)
    return 0, -cost


def states(c):
    """Independent owner choices (including omission), then all list permutations."""
    destinations = list(range(len(c['caps'])))+([-1] if c['optional'] else [])
    for owners in product(destinations, repeat=len(c['costs'])):
        groups = [[v for v, owner in enumerate(owners) if owner == e] for e in range(len(c['caps']))]
        for rows in product(*(permutations(g) for g in groups)):
            yield [list(row) for row in rows]


def optimum(c): return max(score(c, rows) for rows in states(c))


def validate_scope(c):
    n, m = len(c['costs']), len(c['caps'])
    if (not c['id'] or not isinstance(c['optional'], bool) or not 2 <= n <= 96 or not 2 <= m <= 4
            or len(c['missing']) != n or c['caps'][0] != 0 or any(not 0 <= v <= 96 for v in c['caps'])):
        raise ValueError('Invalid list fixture dimensions')
    for row, missing in zip(c['costs'], c['missing']):
        if len(row) != m or not 0 <= missing <= row[0] or any(not 0 <= v <= min(10000, row[0]) for v in row):
            raise ValueError('Initial all-owner-zero cost must be worst')


def group_limit(c, mode):
    if mode not in MODES: raise ValueError('Unknown group mode')
    n = len(c['costs'])
    return n if mode == 'all' else (n+1)//2


def parse_result(c, line, steps, optimal, require_all=True):
    f = line.split('\t')
    if len(f) != 7 or f[0] != c['id']: raise ValueError('Response identity/shape mismatch')
    elapsed, hard, soft, calls, repairs = map(int, [f[1], f[2], f[3], f[5], f[6]])
    if elapsed <= 0 or calls < 1 or not 0 <= repairs <= steps or (require_all and repairs != steps):
        raise ValueError('Invalid time, score calls or completed repair count')
    lists = [[] if row == '-' else list(map(int, row.split(','))) for row in f[4].split(';')]
    values = [v for row in lists for v in row]
    if (len(lists) != len(c['caps']) or len(values) != len(set(values))
            or any(not 0 <= v < len(c['costs']) for v in values)
            or (not c['optional'] and len(values) != len(c['costs']))):
        raise ValueError('Invalid or incomplete list assignment')
    claimed = hard, soft
    if score(c, lists) != claimed: raise ValueError('Incorrect score')
    if claimed < score(c, initial(c)): raise ValueError('Regressed initial score')
    if claimed > optimal: raise ValueError('Score exceeds independent optimum')
    return dict(elapsed_ns=elapsed, score=list(claimed), lists=lists, score_calls=calls, repairs=repairs,
                unassigned=len(c['costs'])-len(values))


def generate_cases():
    rng = random.Random(SEED)
    cases = []
    for optional in [False, True]:
        for i in range(16):
            n, m = 2+i % 4, 2+i % 2
            costs = [[30]+[rng.randint(0, 16) for _ in range(m-1)] for _ in range(n)]
            costs[0][1] = 0
            c = dict(id=f'list-repair-{int(optional)}-{i}', optional=optional, caps=[0]+[(n+m-2)//(m-1)]*(m-1),
                     costs=costs, missing=[5]+[rng.randint(1, 8) for _ in range(n-1)])
            validate_scope(c);cases.append(c)
    return cases


def workload(n, optional):
    costs = []; missing = []; planted = [[] for _ in range(4)]
    for v in range(n):
        omit = optional and v > 0 and v % 5 == 0
        row = [1000, 50, 50, 50]
        if not omit:
            e = 1+v % 3; row[e] = 0; planted[e].insert(0, v)
        costs.append(row);missing.append(0 if omit else 10)
    c = dict(id=f'list-repair-timing-{int(optional)}-{n}', optional=optional, caps=[0]+[(n+2)//3]*3,
             costs=costs, missing=missing)
    validate_scope(c)
    if score(c, planted) != (0, 0): raise ValueError('Invalid planted optimum')
    return c, planted


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--steps', type=int, default=8)
    parser.add_argument('--warmup', type=int, default=5)
    parser.add_argument('--samples', type=int, default=7)
    parser.add_argument('--forks', type=int, default=3)
    parser.add_argument('--ruin-sampling', choices=['independent', 'coverage-cycle'], default='independent',
                        help='Native repair sampling policy; Java retains its public random selector')
    args = parser.parse_args()
    if min(args.steps, args.samples, args.forks) < 1 or args.warmup < 0 or args.steps >= 2147483647 or args.warmup+args.samples > 2147483647:
        parser.error('Invalid run counts')
    cp, artifacts = prepare_solver_java()
    subprocess.run(['javac', '-cp', cp, '-d', str(WORK/'solver'), str(ROOT/'compat/oracle/ListRepairBenchmark.java')], check=True)
    subprocess.run(['cargo', 'build', '--release', '--locked', '--example', 'list_repair_benchmark'], cwd=ROOT, check=True)
    digest = measurement_digest('list-repair-performance')
    commands = dict(java=['java', '-Xms256m', '-Xmx256m', '-cp', cp, 'ListRepairBenchmark'],
                    rust=[str(ROOT/'target/release/examples/list_repair_benchmark')])
    def run(runtime, c, group, minimum, warmup, samples, optimal, require_all=True):
        validate_scope(c)
        command = [*commands[runtime], str(group), str(args.steps), str(warmup), str(samples), str(minimum)]
        if runtime == 'rust':
            command.append(args.ruin_sampling)
        r = subprocess.run(command,
                           input=encode(c)+'\n', capture_output=True, text=True, timeout=180)
        if r.returncode: raise RuntimeError(f'{runtime} {c["id"]}: {r.stderr}')
        lines = r.stdout.splitlines()
        if len(lines) != samples: raise ValueError('Response count mismatch')
        return [parse_result(c, line, args.steps, optimal, require_all) for line in lines]
    cases = generate_cases(); optima = {c['id']: optimum(c) for c in cases}
    checks = []
    for mode in MODES:
        for c in cases:
            optimal = optima[c['id']]
            checks.append(dict(case=c, algorithm=f'{"optional" if c["optional"] else "required"}-{mode}', optimal_score=list(optimal),
                               **{r: run(r, c, group_limit(c, mode), 1, 0, 1, optimal)[0] for r in commands}))
    summary = quality_summary(checks)
    print(f'List repair validation: {summary}', flush=True)
    probes = []
    for optional in [False, True]:
        c = dict(id=f'list-repair-fixed-boundary-{int(optional)}', optional=optional, caps=[0, 2], costs=[[20, 0], [20, 9], [20, 1]], missing=[5, 3, 5])
        optimal = optimum(c)
        for minimum in [1, 3]:
            probes.append(dict(case=c, minimum=minimum, maximum=3, requested_repairs=args.steps, optimal_score=list(optimal),
                               **{r: run(r, c, 3, minimum, 0, 1, optimal, False)[0] for r in commands}))
    workloads = []
    for mode_index, mode in enumerate(MODES):
        for optional in [False, True]:
            for size in [24, 48]:
                c, planted = workload(size, optional); measurements = {r: [] for r in commands}
                for fork in range(args.forks):
                    order = ['java', 'rust'] if (mode_index+int(optional)+fork) % 2 == 0 else ['rust', 'java']
                    for runtime in order:
                        measurements[runtime].extend(run(runtime, c, group_limit(c, mode), 1, args.warmup, args.samples, (0, 0)))
                policy_label = ' · Rust coverage cycle' if args.ruin_sampling == 'coverage-cycle' else ''
                entry = dict(name=f'{"Optional" if optional else "Required"} list · {mode} maximum · {size} values{policy_label}',
                             algorithm=f'{"optional" if optional else "required"}-{mode}', case=c, minimum_ruined=1,
                             maximum_ruined=group_limit(c, mode), unit='ms / solve', optimal_score=[0, 0], planted=planted, samples=measurements)
                for runtime, rows in measurements.items():
                    if len({(tuple(r['score']), tuple(map(tuple, r['lists']))) for r in rows}) != 1: raise ValueError('Seeded results vary')
                    times = [r['elapsed_ns']/1e6 for r in rows]
                    entry[runtime] = dict(median=statistics.median(times), min=min(times), max=max(times), score=rows[0]['score'],
                                         unassigned=rows[0]['unassigned'], **{field: dict(min=min(r[field] for r in rows), max=max(r[field] for r in rows)) for field in ['score_calls', 'repairs']})
                entry['equal_scores'] = entry['java']['score'] == entry['rust']['score']
                entry['java_over_rust'] = entry['java']['median']/entry['rust']['median']
                workloads.append(entry)
                print(f'{entry["name"]}: Rust {entry["rust"]["median"]:.3f}, Java {entry["java"]["median"]:.3f} ms; scores {entry["rust"]["score"]} / {entry["java"]["score"]}', flush=True)
    if measurement_digest('list-repair-performance') != digest: raise RuntimeError('Sources changed during measurement')
    report = dict(schema_version=1, generated_at=datetime.now(timezone.utc).isoformat(), source_digest=digest, reference=REF, artifacts=artifacts,
                  config={**vars(args), 'seed': SEED, 'iterations': 1, 'algorithms': sorted({r['algorithm'] for r in checks})},
                  validation={**summary, 'by_algorithm': {m: quality_summary([r for r in checks if r['algorithm'] == m]) for m in sorted({r['algorithm'] for r in checks})}, 'cases': checks},
                  boundary_probes=probes, workloads=workloads,
                  environment=dict(cpu=next((line.split(':',1)[1].strip() for line in open('/proc/cpuinfo') if line.startswith('model name')), 'unknown'), platform=platform.platform(), rust=version(['rustc', '--version']), java=version(['java', '-version'])),
                  methodology=f'Native repair sampling: {args.ruin_sampling}; Java retains its public independent random selector. Coverage-cycle is an explicit native policy, not a Java seeded-trace claim. '+ 'Original homogeneous required/optional lists without pins or restricted receivers. Hard score is zero; soft score penalizes owner costs, squared capacity excess, ascending neighboring value IDs and missing values. Initial ascending list at owner zero attains the worst possible score. Long step-counting threshold (steps+1) keeps every completed candidate acceptable. Both configurations request groups from one to half/all of the registered value count; actual sampled values, eligibility, PRNG and reconstruction orders differ. Each measured solve must finish exactly the requested repairs; public Java MOVE_EVALUATION_COUNT and native attempts/complete reconstructions are checked. Fixed-minimum optional behavior is retained separately in untimed boundary probes and does not qualify as equal-work timing. Both runtimes use full scoring. Timing includes per-solve solver/state setup, initialization, repair and final validity/score validation; excludes process startup, input parsing, model/network/factory creation, registry setup, final metric reads and output formatting. Java metrics remain active during timing. Alternating runtime order, fresh-process forks, warmups and raw samples retained. Every output independently checked for uniqueness, valid IDs, required completeness, optional omissions, recomputed score, initial non-regression and optimum bound. Small cases enumerate owners/omissions and every permutation; timing cases have a planted zero optimum with nonnegative costs. Equality and optimum attainment are separate from validity. No identical trace, equal scoring work, mixed-model, weighted-union or incremental-stream performance claim. Original fixtures earn no upstream-method credit; shared-host ranges include allocation and GC effects.')
    (ROOT/'work/list-repair-performance.json').write_text(json.dumps(report, indent=2)+'\n')


if __name__ == '__main__':
    raise SystemExit(run_measurement('list-repair-performance', main))
