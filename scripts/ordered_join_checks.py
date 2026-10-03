"""Original ordered/partitioned stream comparisons; no upstream method credit."""
import operator
import random
import subprocess
from differential_scores import ROOT

SEED = 20261116
MIN, MAX = -(1 << 31), (1 << 31) - 1
RELATIONS = [operator.eq, operator.lt, operator.le, operator.gt, operator.ge]


def generate():
    rng = random.Random(SEED)
    cases = []
    for mode in range(15):
        boundaries = [MIN, -1, 0, 0, 1, MAX]
        ops = [('c', side, i, [i % 2, i % 3, key, (-1 if i % 2 else 1) * (i + 1)])
               for side in ['L', 'R'] for i, key in enumerate(boundaries)]
        # Payload-only replacement, both equality keys moving, trials, absent
        # removals and removal of the last member of each equality partition.
        ops += [('t', 'L', 0, [1, 2, MAX, 0]), ('c', 'R', 2, [0, 2, 0, -11]),
                ('c', 'L', 2, [2, 2, 0, 7]), ('t', 'R', 3, None),
                ('c', 'L', 15, None)]
        ops += [('c', side, i, None) for side in ['R', 'L'] for i in range(6)]
        cases.append({'id': f'ordered-{mode:02}-boundary', 'mode': mode, 'operations': ops})
        for sample in range(8):
            ops = []
            for _ in range(40):
                row = None if rng.random() < .25 else [rng.randrange(-2, 3), rng.randrange(-2, 3),
                    rng.choice(boundaries) if sample % 2 else rng.randrange(-9, 10),
                    0 if sample == 0 else rng.randrange(-20, 21)]
                ops.append(('t' if rng.random() < .35 else 'c', rng.choice(['L', 'R']), rng.randrange(8), row))
            ops += [('c', side, i, None) for side in ['L', 'R'] for i in range(8)]
            cases.append({'id': f'ordered-{mode:02}-{sample:02}', 'mode': mode, 'operations': ops})
    return cases


def encode(case):
    ops = ';'.join(f'{kind}:{side}:{slot}:' + ('-' if row is None else ','.join(map(str, row)))
                   for kind, side, slot, row in case['operations'])
    return f'{case["id"]}\t{case["mode"]}\t{ops}'


def states(case):
    sides = {'L': {}, 'R': {}}
    yield {k: dict(v) for k, v in sides.items()}
    for kind, side, slot, row in case['operations']:
        if kind not in ('c', 't') or side not in sides or not 0 <= slot < 16:
            raise ValueError('Invalid operation')
        if row is not None and (len(row) != 4 or any(not MIN <= x <= MAX for x in row[:3]) or not -1000 <= row[3] <= 1000):
            raise ValueError('Invalid row')
        old = sides[side].get(slot)
        if row is None:
            sides[side].pop(slot, None)
        else:
            sides[side][slot] = tuple(row)
        yield {k: dict(v) for k, v in sides.items()}
        if kind == 't':
            if old is None:
                sides[side].pop(slot, None)
            else:
                sides[side][slot] = old
            yield {k: dict(v) for k, v in sides.items()}


def expected(mode, state):
    if not 0 <= mode < 15:
        raise ValueError('Invalid mode')
    equalities = mode // 5
    pairs = [(i, j, *a, *b) for i, a in sorted(state['L'].items()) for j, b in sorted(state['R'].items())
             if a[:equalities] == b[:equalities] and RELATIONS[mode % 5](a[2], b[2])]
    return (len(pairs), sum(row[5] * row[9] for row in pairs)), ';'.join(','.join(map(str, row)) for row in pairs)


def validate(cases, lines, runtime):
    if runtime not in ('java', 'rust'):
        raise ValueError('Unknown runtime')
    observations = [(case, i, state) for case in cases for i, state in enumerate(states(case))]
    if len(lines) != len(observations):
        raise ValueError('Ordered join response count mismatch')
    for line, (case, i, state) in zip(lines, observations):
        fields = line.split('\t')
        if len(fields) != 5 or fields[:2] != [case['id'], str(i)]:
            raise ValueError('Ordered join response identity/field mismatch')
        score, matches = expected(case['mode'], state)
        if tuple(map(int, fields[2:4])) != score:
            raise ValueError(f'Ordered join score mismatch: {case["id"]} state {i}')
        if fields[4] != (matches if runtime == 'rust' else '-'):
            raise ValueError(f'Ordered join match fields mismatch: {case["id"]} state {i}')
    return len(observations)


def run_checks(classpath):
    cases = generate()
    payload = '\n'.join(map(encode, cases)) + '\n'
    outputs = {}
    for runtime, command in {
        'java': ['java', '-Xms256m', '-Xmx256m', '-cp', classpath, 'OrderedJoinOracle'],
        'rust': [str(ROOT / 'target/release/examples/ordered_join_oracle')],
    }.items():
        run = subprocess.run(command, input=payload, text=True, capture_output=True, timeout=180, check=True)
        outputs[runtime] = run.stdout.splitlines()
        observations = validate(cases, outputs[runtime], runtime)
    return {'seed': SEED, 'total': len(cases), 'passed': len(cases), 'observations': observations,
            'cases': cases, 'outputs': outputs,
            'scope': 'Original five-relation checks for no equality partition, one equality key and two equality keys. Duplicate/extreme ordered keys, signed/zero weights, payload and key replacements, independent left/right insertion/removal, trials and undo. Community SolutionManager.update recalculates each score using public Joiners; Rust updates persistent ordered/partitioned graphs. Independent Python pairs validate counts, weighted scores and exact native match IDs/payloads. Java exposes scores only; no Java match-analysis or incremental-update timing claim. No additional upstream method credit.'}
