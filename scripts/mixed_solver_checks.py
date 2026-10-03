#!/usr/bin/env python3
"""Original coupled mixed-search quality checks; no trace or method-credit claim."""
import itertools
import json
import math
import random
import subprocess
from differential_scores import ROOT, WORK

SEED = 20261123


def generate():
    cases = [
        {'id': 'coupled-depots', 'depots': [0, 10], 'points': [1, 9, 10], 'capacities': [2, 2]},
        {'id': 'duplicate-points', 'depots': [0, 10], 'points': [5, 5, 5], 'capacities': [1, 1]},
        {'id': 'one-depot', 'depots': [-2], 'points': [-3, 5, 9, -8], 'capacities': [0, 0]},
        {'id': 'one-customer', 'depots': [-9, 0, 9], 'points': [9], 'capacities': [0, 1, 0]},
    ]
    rng = random.Random(SEED)
    for i in range(60):
        cases.append({'id': f'mixed-search-{i:02}',
                      'depots': rng.sample(range(-12, 13), rng.randint(1, 3)),
                      'points': [rng.randint(-12, 12) for _ in range(rng.randint(1, 4))],
                      'capacities': [rng.randint(0, 3) for _ in range(rng.randint(1, 3))]})
    return cases


def encode(case):
    return '\t'.join([case['id'], *(','.join(map(str, case[k])) for k in ['depots', 'points', 'capacities'])])


def score(case, depots, routes):
    hard = -sum(max(0, len(route)-cap) for route, cap in zip(routes, case['capacities']))
    distance = 0
    for depot, route in zip(depots, routes):
        tour = [depot, *(case['points'][i] for i in route), depot]
        distance += sum(abs(b-a) for a,b in zip(tour,tour[1:]))
    return hard, -distance


def reference(case):
    """Independent permutations and weak ordered splits, then depot products."""
    n, owners = len(case['points']), len(case['capacities'])
    best, count = None, 0
    for order in itertools.permutations(range(n)):
        for cuts in itertools.combinations_with_replacement(range(n+1), owners-1):
            bounds = (0, *cuts, n)
            routes = [order[a:b] for a,b in zip(bounds,bounds[1:])]
            for depots in itertools.product(case['depots'], repeat=owners):
                candidate = score(case, depots, routes)
                best = candidate if best is None else max(best,candidate)
                count += 1
    expected_count = math.factorial(n)*math.comb(n+owners-1, owners-1)*len(case['depots'])**owners
    if count != expected_count:
        raise AssertionError('independent enumeration count')
    return {'score': list(best), 'assignments': count}


def validate(case, line, optimum, mode):
    f = line.split('\t')
    native = mode != 'java'
    if len(f) != (9 if native else 5) or f[0] != case['id']:
        raise ValueError('Mixed search identity/fields')
    if native:
        if f[1] != mode or mode not in ('exact', 'scoped', 'stream'):
            raise ValueError('Mixed search backend')
        f = [f[0], *f[2:]]
    claimed = tuple(map(int, f[1:3]))
    depots = list(map(int, f[3].split(',')))
    routes = [[] if r == '-' else list(map(int,r.split(','))) for r in f[4].split(';')]
    owners = len(case['capacities'])
    if len(depots) != owners or len(routes) != owners or any(d not in case['depots'] for d in depots):
        raise ValueError('Mixed search depot/owner shape')
    if sorted(v for row in routes for v in row) != list(range(len(case['points']))):
        raise ValueError('Mixed search missing/duplicate/out-of-range customer')
    if claimed != score(case, depots, routes):
        raise ValueError('Mixed search false score')
    optimum_score = tuple(optimum['score'])
    if claimed > optimum_score:
        raise ValueError('Mixed search score exceeds independent optimum')
    result = {'score': list(claimed), 'depots': depots, 'routes': routes,
              'optimal': claimed == optimum_score}
    if native:
        evaluations, steps = map(int,f[5:7])
        reason = f[7]
        if mode == 'exact':
            if claimed != optimum_score or evaluations != optimum['assignments'] or steps != 0 or reason != 'Exhausted':
                raise ValueError('Native mixed exact search did not prove the complete optimum')
        else:
            initial = score(case, [case['depots'][0]]*owners, [list(range(len(case['points']))), *([[]]*(owners-1))])
            if claimed < initial or not 1 <= evaluations <= 1_000_000 or not 0 <= steps <= 20:
                raise ValueError('Native mixed local quality/work limits')
            if reason not in ('NoAcceptedMove','StepLimit') or (reason == 'StepLimit' and steps != 20):
                raise ValueError('Unexpected mixed local termination')
        result.update(evaluations=evaluations, steps=steps, termination=reason)
    return result


def run_checks(classpath):
    subprocess.run(['javac','-cp',classpath,'-d',str(WORK/'solver'),str(ROOT/'compat/oracle/MixedSolverOracle.java')],check=True)
    subprocess.run(['cargo','build','--release','--example','mixed_solver_oracle','--locked'],cwd=ROOT,check=True)
    cases = generate()
    payload = '\n'.join(map(encode,cases))+'\n'
    commands = {'java':['java','-Xms256m','-Xmx256m','-cp',classpath,'MixedSolverOracle'],
                'rust':[str(ROOT/'target/release/examples/mixed_solver_oracle')]}
    outputs = {}
    for runtime, command in commands.items():
        run = subprocess.run(command,input=payload,text=True,capture_output=True,timeout=120)
        (WORK/f'mixed-solver-{runtime}.stderr.log').write_text(run.stderr)
        if run.returncode:
            raise RuntimeError(f'Mixed search {runtime} failed; inspect work/oracle/mixed-solver-{runtime}.stderr.log')
        outputs[runtime] = run.stdout.splitlines()
        if len(outputs[runtime]) != len(cases)*(3 if runtime == 'rust' else 1):
            raise ValueError('Mixed search response count')
    results = []
    modes = ('exact','scoped','stream')
    for i, case in enumerate(cases):
        optimum = reference(case)
        result = {'id':case['id'], 'reference':optimum,
                  'java':validate(case,outputs['java'][i],optimum,'java')}
        for offset, mode in enumerate(modes):
            result[mode] = validate(case,outputs['rust'][3*i+offset],optimum,mode)
        if result['scoped'] != result['stream']:
            raise ValueError('Mixed scoped/stream search divergence')
        results.append(result)
    report = {'seed':SEED,'total':len(cases),'passed':len(cases),'failures':[],
              'reference_assignments':sum(r['reference']['assignments'] for r in results),
              'optimal':{mode:sum(r[mode]['optimal'] for r in results) for mode in ('java',*modes)},
              'equal_exact_scores':sum(r['java']['score']==r['exact']['score'] for r in results),
              'scope':'Original required mixed depot/ordered-route cases: 1-3 owners, 1-4 customers, 1-3 depots, duplicate customer coordinates, capacity deficits and ties. Independent permutations/splits/depot products check joint optima. Java uses Community BRUTE_FORCE with unsorted entities/values, starting unassigned. Rust exhausts the joint product. Native scoped/stream strict-improvement search starts complete at first depot/all customers on first owner; best accepted, joint moves, 20-step cap. All returned assignments and scores are validated; quality differences are reported, not silently treated as equal scores. No Java local search, trace/counter parity, pins/ranges, optional lists, timing or upstream method-credit claim.'}
    (WORK/'mixed-solver-cases.json').write_text(json.dumps({'schema_version':1,**report,'cases':cases,'outputs':outputs,'results':results},indent=2)+'\n')
    print(f'Mixed search: {len(cases)}/{len(cases)} valid; optimal counts {report["optimal"]}',flush=True)
    return report
