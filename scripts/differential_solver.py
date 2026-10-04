#!/usr/bin/env python3
"""Compare original finite planning cases against Java and an independent oracle."""
import itertools
import json
import platform
import random
import subprocess
from datetime import datetime, timezone
from differential_scores import ROOT, WORK, REF, source_digest
from prepare_solver_oracle import prepare_solver_java


def generate():
    rng = random.Random(20261001)
    cases = []
    for i in range(64):
        n, k = rng.randint(1, 7), rng.randint(2, 4)
        initial = [rng.randrange(k) if rng.random() < 0.6 else None for _ in range(n)]
        pins = [v is not None and (i % 8 == 0 or rng.random() < 0.2) for v in initial]
        cases.append({'id': f'graph-{i:02}', 'value_count': k, 'initial': initial, 'pins': pins,
                      'edges': [(a,b) for a in range(n) for b in range(a+1,n) if rng.random() < 0.45],
                      'weights': [[rng.randint(-9,9) for _ in range(k)] for _ in range(n)]})
    return cases


def generate_extended():
    """Original cases with per-variable ranges, optionality and null preferences."""
    def case(name, domains, optional, weights, unassigned, edges=(), initial=None, pins=None):
        n = len(domains)
        return {'id':name, 'value_count':len(weights[0]), 'domains':domains,
                'optional':optional, 'weights':weights, 'unassigned_weights':unassigned,
                'edges':list(edges), 'initial':[None]*n if initial is None else initial,
                'pins':[False]*n if pins is None else pins}
    cases = [
        case('optional-prefers-value', [[0,1]], [True], [[5,9]], [0]),
        case('optional-prefers-null', [[0,1]], [True], [[-5,-9]], [0]),
        case('required-distinct-ranges', [[1],[0,2]], [False,False], [[0,4,0],[1,0,7]], [0,0], [(0,1)]),
        case('mixed-required-optional', [[0,1],[1]], [False,True], [[5,9],[4,8]], [0,-1], [(0,1)]),
        case('empty-optional-range', [[]], [True], [[5,9]], [-2]),
        case('pinned-null', [[0,1],[1]], [True,False], [[5,9],[4,8]], [-5,0], [(0,1)], pins=[True,False]),
        case('all-pinned-mixed', [[0],[1],[]], [False,True,True], [[7,0],[0,8],[0,0]], [0,-5,-2], [(0,1),(1,2)], [0,1,None], [True]*3),
        case('range-order-ties', [[2,0],[1,0]], [False,True], [[0]*3,[0]*3], [0,0], [(0,1)], [2,1]),
    ]
    rng = random.Random(20261002)
    for i in range(56):
        n, k = rng.randint(1,7), rng.randint(2,4)
        optional = [rng.random() < 0.5 for _ in range(n)]
        domains = [rng.sample(range(k), rng.randint(0 if allow else 1,k)) for allow in optional]
        initial = [rng.choice(domain) if domain and rng.random() < 0.6 else None for domain in domains]
        pins = [(allow or value is not None) and rng.random() < 0.25 for allow,value in zip(optional,initial)]
        cases.append(case(f'ranges-{i:02}', domains, optional,
                          [[rng.randint(-9,9) for _ in range(k)] for _ in range(n)],
                          [rng.randint(-9,9) for _ in range(n)],
                          [(a,b) for a in range(n) for b in range(a+1,n) if rng.random() < 0.45], initial,pins))
    return cases


def encode(case):
    fields = [case['id'], str(case['value_count']), ','.join('-' if v is None else str(v) for v in case['initial']),
              ''.join('1' if p else '0' for p in case['pins']), ','.join(f'{a}:{b}' for a,b in case['edges']),
              ','.join(str(v) for row in case['weights'] for v in row)]
    if 'domains' in case:
        fields += [';'.join(':'.join(map(str,domain)) for domain in case['domains']),
                   ''.join('1' if allow else '0' for allow in case['optional']),
                   ','.join(map(str,case['unassigned_weights']))]
    return '\t'.join(fields)


def score(case, values):
    null_weights = case.get('unassigned_weights', [0]*len(values))
    return (-sum(values[a] is not None and values[a] == values[b] for a,b in case['edges']),
            sum(null_weights[i] if v is None else case['weights'][i][v] for i,v in enumerate(values)))


def optimum(case):
    n = len(case['initial'])
    ranges = case.get('domains', [list(range(case['value_count'])) for _ in range(n)])
    optional = case.get('optional', [False]*n)
    domains = [[value] if pin else list(domain) + ([None] if allow else [])
               for value,pin,domain,allow in zip(case['initial'],case['pins'],ranges,optional)]
    return max(score(case, values) for values in itertools.product(*domains))


def validate(case, line, expected):
    fields = line.split('\t')
    if len(fields) != 4 or fields[0] != case['id']: return 'Response identity/field mismatch'
    try:
        claimed = tuple(map(int, fields[1:3]))
        values = [None if v == '-' else int(v) for v in fields[3].split(',')]
    except ValueError:
        return 'Noninteger result'
    n = len(case['initial'])
    ranges = case.get('domains', [range(case['value_count']) for _ in range(n)])
    optional = case.get('optional', [False]*n)
    if len(values) != n or any((not allow if v is None else v not in domain)
                              for v,domain,allow in zip(values,ranges,optional)): return 'Invalid assignment'
    if any(pin and v != initial for pin,v,initial in zip(case['pins'], values, case['initial'])): return 'Pin changed'
    recomputed = score(case, values)
    if claimed != recomputed: return f'Incorrect score: claimed {claimed}, recomputed {recomputed}'
    if claimed != expected: return f'Suboptimal: {claimed}, expected {expected}'
    return None


def main():
    classpath, artifacts = prepare_solver_java()
    subprocess.run(['cargo','build','--release','--example','solver_oracle','--locked'], cwd=ROOT, check=True)
    digest = source_digest()
    cases = generate() + generate_extended()
    payload = '\n'.join(map(encode, cases)) + '\n'
    commands = {'java': ['java','-Xms256m','-Xmx256m','-cp',classpath,'SolverOracle'],
                'rust': [str(ROOT / 'target/release/examples/solver_oracle')]}
    outputs = {}
    for name, command in commands.items():
        run = subprocess.run(command, input=payload, text=True, capture_output=True, timeout=120)
        (WORK / f'solver-{name}.stderr.log').write_text(run.stderr)
        if run.returncode: raise RuntimeError(f'{name} failed; see work/oracle/solver-{name}.stderr.log')
        outputs[name] = run.stdout.splitlines()
        if len(outputs[name]) != len(cases): raise RuntimeError(f'{name} response count mismatch')
    failures = []
    for i, case in enumerate(cases):
        expected = optimum(case)
        for name in outputs:
            error = validate(case, outputs[name][i], expected)
            if error: failures.append({'case':case['id'],'runtime':name,'error':error})
    from termination_checks import run_checks
    termination = run_checks(classpath)
    from mixed_solver_checks import run_checks as check_mixed
    mixed = check_mixed(classpath)
    from step_counting_checks import run_checks as check_step_counting
    step_counting = check_step_counting(classpath)
    from great_deluge_checks import run_checks as check_great_deluge
    great_deluge = check_great_deluge(classpath)
    from oscillation_checks import run_checks as check_oscillation
    oscillation = check_oscillation(classpath)
    from score_trend_checks import run_checks as check_trends
    trends = check_trends(classpath)
    if source_digest() != digest: raise RuntimeError('Sources changed during comparison')
    failed_ids = {failure['case'] for failure in failures}
    report = {'schema_version':1, 'generated_at':datetime.now(timezone.utc).isoformat(), 'source_digest':digest,
              'revision':REF['commit'], 'seed':20261001,
              'seeds':{'homogeneous_required':20261001, 'per_variable_ranges':20261002}, 'total':len(cases), 'passed':len(cases)-len(failed_ids),
              'failures':failures, 'score_trend_checks':trends, 'oscillation_checks':oscillation, 'termination_checks':termination, 'step_counting_checks':step_counting, 'great_deluge_checks':great_deluge, 'mixed_solver_checks':mixed, 'artifacts':artifacts, 'environment':platform.platform(),
              'groups': {label:{'total':len(group), 'passed':sum(c['id'] not in failed_ids for c in group)}
                         for label,group in [('homogeneous_required',cases[:64]),('per_variable_ranges',cases[64:])]},
              'scope':'Original graph-assignment cases checked against independent Python enumeration: 64 homogeneous required and 64 per-variable range cases, including mixed required/optional values, empty optional ranges, pinned nulls and signed null preferences. Movable initial values reset. The extended Java adapter uses two fields per entity; its inactive field has one fixed choice. HardSoft scores, complete optima, no trace equivalence or upstream coverage credit.'}
    (ROOT / 'work/solver-differential.json').write_text(json.dumps(report,indent=2)+'\n')
    (WORK / 'solver-cases.json').write_text(json.dumps({'schema_version':1,'seeds':report['seeds'],'cases':cases,'outputs':outputs},indent=2)+'\n')
    print(f'Differential solver cases: {report["passed"]}/{report["total"]} passed')
    for failure in failures[:8]: print(json.dumps(failure))
    return bool(failures or termination['failures'] or mixed['failures'] or step_counting['failures'] or great_deluge['failures'] or oscillation['failures'] or trends['failures'])

if __name__ == '__main__':
    raise SystemExit(main())
