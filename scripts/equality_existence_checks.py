"""Original public-API equality existence comparisons; no method coverage credit."""
import random
import subprocess
from differential_scores import ROOT
from ordered_join_checks import states, encode, MIN, MAX

SEED = 20261118

def generate():
    rng = random.Random(SEED)
    cases = []
    for mode in range(6):
        # Duplicate key presence changes only on first insertion/last removal;
        # changed non-key payloads must not multiply or suppress a left row.
        ops = [('c', side, i, [first, second, i, weight]) for side in ('L', 'R')
               for i, (first, second, weight) in enumerate([(MIN, 0, 0), (MIN, 0, -7), (MAX, 0, 7), (MAX, 1, 3)])]
        ops += [('c', 'R', 0, [MAX, 1, 9, -99]), ('t', 'R', 1, None),
                ('c', 'L', 1, [MIN, 0, 9, 11]), ('c', 'L', 2, [0, 0, 9, -11]), ('c', 'R', 15, None)]
        ops += [('c', side, i, None) for side in ('R', 'L') for i in range(4)]
        cases.append({'id': f'existence-{mode}-boundary', 'mode': mode, 'operations': ops})
        for sample in range(8):
            ops = []
            for _ in range(60):
                row = None if rng.random() < .25 else [rng.randrange(-2, 3), rng.randrange(-2, 3), rng.randrange(-5, 6), rng.randrange(-20, 21)]
                ops.append(('t' if rng.random() < .35 else 'c', rng.choice(['L', 'R']), rng.randrange(8), row))
            ops += [('c', side, i, None) for side in ('L', 'R') for i in range(8)]
            cases.append({'id': f'existence-{mode}-{sample}', 'mode': mode, 'operations': ops})
    return cases

def expected(mode, state):
    if not 0 <= mode < 6:
        raise ValueError('Invalid existence mode')
    arity = mode // 2
    retained = [(i, *a) for i, a in sorted(state['L'].items())
                if any(a[:arity] == b[:arity] for b in state['R'].values()) == (mode % 2 == 0)]
    return (len(retained), sum(row[4] for row in retained)), ';'.join(','.join(map(str, row)) for row in retained)

def validate(cases, lines, runtime):
    if runtime not in ('java', 'rust'):
        raise ValueError('Unknown runtime')
    observations = [(c, i, state) for c in cases for i, state in enumerate(states(c))]
    if len(lines) != len(observations):
        raise ValueError('Existence response count mismatch')
    for line, (case, i, state) in zip(lines, observations):
        fields = line.split('\t')
        if len(fields) != 5 or fields[:2] != [case['id'], str(i)]:
            raise ValueError('Existence response identity/field mismatch')
        score, matches = expected(case['mode'], state)
        if tuple(map(int, fields[2:4])) != score:
            raise ValueError(f'Existence score mismatch: {case["id"]} state {i}')
        if fields[4] != (matches if runtime == 'rust' else '-'):
            raise ValueError(f'Existence match mismatch: {case["id"]} state {i}')
    return len(observations)

def run_checks(classpath):
    cases = generate()
    payload = '\n'.join(map(encode, cases)) + '\n'
    outputs = {}
    for runtime, command in {
        'java': ['java', '-Xms256m', '-Xmx256m', '-cp', classpath, 'ExistenceOracle'],
        'rust': [str(ROOT / 'target/release/examples/existence_oracle')],
    }.items():
        run = subprocess.run(command, input=payload, text=True, capture_output=True, timeout=180, check=True)
        outputs[runtime] = run.stdout.splitlines()
        observations = validate(cases, outputs[runtime], runtime)
    return {'seed': SEED, 'total': len(cases), 'passed': len(cases), 'observations': observations,
            'cases': cases, 'outputs': outputs,
            'scope': 'Original Exists/NotExists checks with zero, one and two equality keys. Duplicate right matches, first/last presence transitions, key/payload changes, signed/zero left weights, independent edits, commit and undo. Community public ifExists/ifNotExists score recalculation and Rust count-based incremental graphs agree with Python nested enumeration. Exact native left IDs/payloads checked separately. No Java match-analysis, incremental notification, timing or upstream method-credit claim.'}
