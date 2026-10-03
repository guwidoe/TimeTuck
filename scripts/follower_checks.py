#!/usr/bin/env python3
"""Generated follower comparisons; no additional upstream method credit.

The static testMove boundary is adapted from Timefold Community, Apache-2.0;
see compat/provenance.json and THIRD_PARTY_NOTICES.md. Other inputs are original.
"""
from collections import Counter
from copy import deepcopy
import random
import subprocess
from differential_scores import ROOT, WORK

SEED = 20261003


def valid(case, state):
    leaders, refs = state['leaders'], state['refs']
    if len(leaders) != len(case['leaders']) or len(refs) != len(case['refs']):
        raise ValueError('Follower state shape')
    for values, size in [(leaders, case['values']), (refs, len(leaders))]:
        if any(v is not None and (type(v) is not int or not 0 <= v < size) for v in values):
            raise ValueError('Follower value/reference domain')


def apply(case, state, moves):
    result = deepcopy(state)
    for kind, index, value in moves:
        if kind not in ('v', 'r') or (kind == 'r' and case['model'] == 'shared'):
            raise ValueError('Follower move kind')
        rows = result['leaders' if kind == 'v' else 'refs']
        if type(index) is not int or not 0 <= index < len(rows):
            raise ValueError('Follower move index')
        rows[index] = value
        valid(case, result)
    return result


def states(case):
    current = {'leaders': case['leaders'][:], 'refs': case['refs'][:]}
    valid(case, current)
    yield deepcopy(current)
    for batch in case['batches']:
        if batch['mode'] not in ('t', 'c'):
            raise ValueError('Follower batch mode')
        candidate = apply(case, current, batch['moves'])
        yield candidate
        if batch['mode'] == 't':
            yield deepcopy(current)
        else:
            current = candidate


def text(value):
    return '-' if value is None else str(value)


def row(values):
    return ','.join(map(text, values)) or '_'


def expected(case, index, state):
    valid(case, state)
    leaders, refs = state['leaders'], state['refs']
    shadows = [None if ref is None else leaders[ref] for ref in refs]
    # The pinned interface-based forEach includes all implementing entities.
    counts = Counter(leaders + shadows)
    score = -sum(n*n for n in counts.values())
    complete = all(v is not None for v in leaders + (refs if case['model'] == 'dynamic' else []))
    return '\t'.join([case['id'], str(index), str(int(complete)), str(score),
                      row(leaders), row(refs), row(shadows)])


def validate_outputs(cases, outputs):
    observations = [(case, i, state) for case in cases for i, state in enumerate(states(case))]
    if set(outputs) != {'java', 'rust'} or any(len(rows) != len(observations) for rows in outputs.values()):
        raise ValueError('Follower response count/runtime mismatch')
    results = []
    for j, (case, i, state) in enumerate(observations):
        want = expected(case, i, state)
        for runtime, rows in outputs.items():
            if rows[j] != want:
                raise ValueError(f'Follower {runtime} mismatch: expected {want!r}, got {rows[j]!r}')
        results.append({'id': case['id'], 'state': i, 'assignment': state,
                        **{runtime: rows[j] for runtime, rows in outputs.items()}})
    return results


def encode(case):
    batches = '|'.join(b['mode'] + '@' + ';'.join(','.join(map(text, op)) for op in b['moves']) for b in case['batches'])
    return '\t'.join([case['id'], str(case['values']), row(case['leaders']), row(case['refs']), batches, case['model']])


def generate():
    cases = []
    def add(values, leaders, refs, batches, model='dynamic'):
        cases.append({'id': f'follower-{len(cases):03}', 'values': values, 'leaders': leaders, 'refs': refs,
                      'model': model, 'batches': [{'mode': mode, 'moves': moves} for mode, moves in batches]})
    add(3, [None, 1], [0, 0, 1, None], [
        ('t', [['v', 0, 2], ['r', 0, 1], ['v', 1, None]]),
        ('c', [['v', 0, 0], ['r', 3, 0]]), ('t', [['r', 1, None]]),
        ('c', [['v', 0, 1], ['v', 0, 2]]), ('t', [])])
    add(1, [None], [], [('t', [['v', 0, 0]]), ('c', []), ('c', [['v', 0, 0]])])
    add(2, [0, 1, None], [None, None], [('t', [['r', 0, 2], ['r', 1, 2]]),
        ('c', [['r', 0, 0], ['r', 1, 0]]), ('t', [['v', 0, None]])])
    add(2, [None, None, None], [0,0,0,1,1], [('c', [['v',0,0]]),
        ('c', [['v',1,1]]), ('c', [['v',2,0]]), ('t', [['v',0,None]])], 'shared')
    add(2, [0], [None, None, 0], [('t', [['v',0,None]]), ('c', [])], 'shared')
    rng = random.Random(SEED)
    while len(cases) < 128:
        n, m, k = rng.randint(1, 6), rng.randint(0, 12), rng.randint(1, 4)
        add(k, [rng.choice([None, *range(k)]) for _ in range(n)],
            [rng.choice([None, *range(n)]) for _ in range(m)], [],
            'dynamic' if len(cases) % 2 else 'shared')
        for _ in range(12):
            moves = []
            for _ in range(rng.randint(1, 4)):
                kind = 'v' if cases[-1]['model'] == 'shared' or m == 0 or rng.random() < .5 else 'r'
                moves.append([kind, rng.randrange(n if kind == 'v' else m),
                              rng.choice([None, *range(k if kind == 'v' else n)])])
            cases[-1]['batches'].append({'mode': 't' if rng.random() < .5 else 'c', 'moves': moves})
    return cases


def run_checks(classpath):
    cases = generate()
    payload = '\n'.join(map(encode, cases)) + '\n'
    subprocess.run(['javac', '-cp', classpath, '-d', str(WORK/'solver'),
                    str(ROOT/'compat/oracle/FollowerOracle.java')], check=True)
    subprocess.run(['cargo', 'build', '--release', '--locked', '--example', 'follower_oracle'], cwd=ROOT, check=True)
    outputs = {}
    for runtime, cmd in {'java': ['java', '-Xms256m', '-Xmx256m', '-cp', classpath, 'FollowerOracle'],
                         'rust': [str(ROOT/'target/release/examples/follower_oracle')]}.items():
        result = subprocess.run(cmd, input=payload, text=True, capture_output=True, timeout=180)
        if result.returncode:
            raise RuntimeError(f'Follower {runtime}: {result.stderr}')
        outputs[runtime] = result.stdout.splitlines()
    results = validate_outputs(cases, outputs)
    return {'seed': SEED, 'total': len(cases), 'passed': len(cases), 'observations': len(results),
            'operations': sum(len(b['moves']) for case in cases for b in case['batches']),
            'cases': cases, 'results': results,
            'scope': 'Original generated leader-value variables with either fixed shared references or required dynamic follower-reference variables, including partial assignments, shared targets, unreferenced leaders, zero followers, null values/references, repeated writes, no-ops, composite commits and temporary moves with undo. Domain/scoring fixtures attributed separately. Java public MoveTester maintains annotated shadows; SolutionManager.UPDATE_SCORE_ONLY recomputes each observed score. Rust uses persistent reference/shared-supplier streams. Interface-based forEach includes null-valued rows in both models. Independent Python checks complete assignments, reference identities, every derived value and ordinary count-squared score. No Java supplier-call counts, structural score, incremental Java scoring, timing, search or additional upstream-method credit.'}
