#!/usr/bin/env python3
"""Original generated collection-follower inputs; no upstream method credit.

Domain/supplier fixtures in the runtime adapters are attributed separately.
"""
from collections import Counter
import random
import subprocess
from differential_scores import ROOT, WORK
from follower_checks import row, text

SEED = 20261004


def valid(case, values):
    if len(values) != len(case['initial']):
        raise ValueError('Collection follower state shape')
    if any(v is not None and (type(v) is not int or not 0 <= v < len(case['codes'])) for v in values):
        raise ValueError('Collection follower value domain')
    if any(not refs or any(type(r) is not int or not 0 <= r < len(values) for r in refs)
           for refs in case['dependencies']):
        raise ValueError('Dependency collection')


def states(case):
    current = case['initial'][:]
    valid(case, current)
    yield current[:]
    for batch in case['batches']:
        if batch['mode'] not in ('t', 'c'):
            raise ValueError('Collection follower batch mode')
        candidate = current[:]
        for index, value in batch['moves']:
            if type(index) is not int or not 0 <= index < len(candidate):
                raise ValueError('Collection follower move index')
            candidate[index] = value
            valid(case, candidate)
        yield candidate[:]
        if batch['mode'] == 't':
            yield current[:]
        else:
            current = candidate


def expected(case, index, values):
    valid(case, values)
    shadows = []
    for refs in case['dependencies']:
        candidates = [values[r] for r in refs if values[r] is not None]
        # Big-endian UTF-16 bytes preserve Java code-unit order. Python min
        # retains the first item for equal keys, as required by the supplier.
        shadows.append(min(candidates, key=lambda v: case['codes'][v].encode('utf-16-be')) if candidates else None)
    counts = Counter(values + shadows)
    score = -sum(n*n for n in counts.values())
    return '\t'.join([case['id'], str(index), str(int(all(v is not None for v in values))),
                      str(score), row(values), row(shadows)])


def encode(case):
    batches = '|'.join(b['mode'] + '@' + ';'.join(','.join(map(text, op)) for op in b['moves']) for b in case['batches'])
    return '\t'.join([case['id'], ','.join(case['codes']), row(case['initial']),
                      ';'.join(row(refs) for refs in case['dependencies']) or '_', batches])


def validate_outputs(cases, outputs):
    observations = [(case, i, values) for case in cases for i, values in enumerate(states(case))]
    if set(outputs) != {'java', 'rust'} or any(len(rows) != len(observations) for rows in outputs.values()):
        raise ValueError('Collection follower response count/runtime mismatch')
    results = []
    for j, (case, i, values) in enumerate(observations):
        want = expected(case, i, values)
        for runtime, rows in outputs.items():
            if rows[j] != want:
                raise ValueError(f'Collection follower {runtime}: expected {want!r}, got {rows[j]!r}')
        results.append({'id': case['id'], 'state': i, 'values': values,
                        **{runtime: rows[j] for runtime, rows in outputs.items()}})
    return results


def generate():
    cases = []
    def add(codes, initial, dependencies, batches):
        cases.append({'id': f'collection-follower-{len(cases):03}', 'codes': codes, 'initial': initial,
                      'dependencies': dependencies, 'batches': [{'mode': m, 'moves': ops} for m, ops in batches]})
    add(['\ue000', '\U00010000', 'same', 'same'], [0, 1, 2, 3],
        [[0,1], [1,0], [2,3], [3,2], [0,0,1]],
        [('t', [[0,None], [1,None], [2,None]]), ('c', [[0,2], [1,3]]),
         ('t', [[0,3], [0,None], [1,None]]), ('c', [])])
    add(['10', '2', ''], [None, None], [[1,0,1], [0]],
        [('c', [[0,1]]), ('t', [[1,0]]), ('c', [[0,2]]), ('t', [[0,None]])])
    add(['x'], [None], [], [('t', [[0,0]]), ('c', []), ('c', [[0,0]])])
    rng = random.Random(SEED)
    pool = ['', 'a', 'aa', '2', '10', '\ue000', '\U00010000', 'same']
    while len(cases) < 128:
        n, k, m = rng.randint(1, 6), rng.randint(1, 8), rng.randint(0, 12)
        add([rng.choice(pool) for _ in range(k)], [rng.choice([None, *range(k)]) for _ in range(n)],
            [[rng.randrange(n) for _ in range(rng.randint(1, 6))] for _ in range(m)], [])
        for _ in range(12):
            moves = [[rng.randrange(n), rng.choice([None, *range(k)])] for _ in range(rng.randint(1, 4))]
            cases[-1]['batches'].append({'mode': 't' if rng.random() < .5 else 'c', 'moves': moves})
    return cases


def run_checks(classpath):
    cases = generate()
    payload = '\n'.join(map(encode, cases)) + '\n'
    subprocess.run(['javac', '-cp', classpath, '-d', str(WORK/'solver'),
                    str(ROOT/'compat/oracle/FollowerOracle.java'), str(ROOT/'compat/oracle/CollectionFollowerOracle.java')], check=True)
    subprocess.run(['cargo', 'build', '--release', '--locked', '--example', 'collection_follower_oracle'], cwd=ROOT, check=True)
    outputs = {}
    for runtime, cmd in {'java': ['java', '-Xms256m', '-Xmx256m', '-cp', classpath, 'CollectionFollowerOracle'],
                         'rust': [str(ROOT/'target/release/examples/collection_follower_oracle')]}.items():
        result = subprocess.run(cmd, input=payload, text=True, capture_output=True, timeout=180)
        if result.returncode:
            raise RuntimeError(f'Collection follower {runtime}: {result.stderr}')
        outputs[runtime] = result.stdout.splitlines()
    results = validate_outputs(cases, outputs)
    return {'seed': SEED, 'total': len(cases), 'passed': len(cases), 'observations': len(results),
            'operations': sum(len(b['moves']) for c in cases for b in c['batches']), 'cases': cases, 'results': results,
            'scope': 'Original generated inputs with fixed nonempty dependency collections, duplicate references, null leader values, zero followers, empty/duplicate/Unicode codes, stable distinct value identities, first-reference ties, sequential repeated writes, composite commits and temporary moves with undo. Domain/supplier fixtures attributed separately. Java public MoveTester updates annotated shadows; SolutionManager.UPDATE_SCORE_ONLY recomputes scores. Rust uses membership joins, conditional ordered collection and reference projections. Independent Python validates every derived identity, leader value, ordinary interface-based count-squared score and completeness flag. Codes are fixed; Java UTF-16 code-unit ordering is preserved. No dependency-list mutation, empty/null dependency collections, PlanningId validation, supplier-call counts, Java structural-score fields, search, timing or additional upstream-method credit.'}
