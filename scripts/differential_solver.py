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


def encode(case):
    return '\t'.join([case['id'], str(case['value_count']), ','.join('-' if v is None else str(v) for v in case['initial']),
                      ''.join('1' if p else '0' for p in case['pins']), ','.join(f'{a}:{b}' for a,b in case['edges']),
                      ','.join(str(v) for row in case['weights'] for v in row)])


def score(case, values):
    return (-sum(values[a] == values[b] for a,b in case['edges']), sum(row[v] for row,v in zip(case['weights'], values)))


def optimum(case):
    domains = [[value] if pin else range(case['value_count']) for value,pin in zip(case['initial'],case['pins'])]
    return max(score(case, values) for values in itertools.product(*domains))


def validate(case, line, expected):
    fields = line.split('\t')
    if len(fields) != 4 or fields[0] != case['id']: return 'Response identity/field mismatch'
    try:
        claimed = tuple(map(int, fields[1:3]))
        values = list(map(int, fields[3].split(',')))
    except ValueError:
        return 'Noninteger or unassigned result'
    if len(values) != len(case['initial']) or any(v < 0 or v >= case['value_count'] for v in values): return 'Invalid assignment'
    if any(pin and v != initial for pin,v,initial in zip(case['pins'], values, case['initial'])): return 'Pin changed'
    recomputed = score(case, values)
    if claimed != recomputed: return f'Incorrect score: claimed {claimed}, recomputed {recomputed}'
    if claimed != expected: return f'Suboptimal: {claimed}, expected {expected}'
    return None


def main():
    classpath, artifacts = prepare_solver_java()
    subprocess.run(['cargo','build','--release','--example','solver_oracle','--locked'], cwd=ROOT, check=True)
    digest = source_digest()
    cases = generate()
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
    if source_digest() != digest: raise RuntimeError('Sources changed during comparison')
    failed_ids = {failure['case'] for failure in failures}
    report = {'schema_version':1, 'generated_at':datetime.now(timezone.utc).isoformat(), 'source_digest':digest,
              'revision':REF['commit'], 'seed':20261001, 'total':len(cases), 'passed':len(cases)-len(failed_ids),
              'failures':failures, 'artifacts':artifacts, 'environment':platform.platform(),
              'scope':'Original complete graph-assignment cases; both runtimes checked against independent Python enumeration. Brute force, homogeneous finite ranges, pins, HardSoft scores; movable initial values reset before both solves. No upstream coverage credit.'}
    (ROOT / 'work/solver-differential.json').write_text(json.dumps(report,indent=2)+'\n')
    (WORK / 'solver-cases.json').write_text(json.dumps({'schema_version':1,'cases':cases,'outputs':outputs},indent=2)+'\n')
    print(f'Differential solver cases: {report["passed"]}/{report["total"]} passed')
    for failure in failures[:8]: print(json.dumps(failure))
    return bool(failures)

if __name__ == '__main__':
    raise SystemExit(main())
