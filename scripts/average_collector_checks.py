"""Original bit-exact average collector comparison, called by differential_streams."""
import random
import struct
import subprocess
from differential_scores import ROOT

SEED = 20261029
MIN, MAX = -(1 << 63), (1 << 63) - 1


def checked(value):
    if not MIN <= value <= MAX:
        raise OverflowError
    return value


def expected(case):
    slots = {}
    total = 0
    out = [f'{case["id"]}\t0\t-']
    for state, op in enumerate(case['operations'], 1):
        kind, slot, *values = op
        try:
            if kind == 'p':
                value = values[0]
                if slot in slots:
                    delta = checked(value - slots[slot])
                    total = (total + delta - MIN) % (1 << 64) + MIN
                else:
                    total = checked(total + value)
                slots[slot] = value
            elif kind == 'r':
                if slot in slots:
                    total = checked(total - slots.pop(slot))
            else:
                raise ValueError('Unknown operation')
        except OverflowError:
            out.append(f'{case["id"]}\t{state}\tOVERFLOW')
            break
        value = '-' if not slots else struct.pack('>d', float(total) / len(slots)).hex()
        out.append(f'{case["id"]}\t{state}\t{value}')
    return out


def generate():
    operations = [
        [], [('p', 0, MAX), ('p', 1, MAX)],
        [('p', 0, MIN), ('p', 0, MAX)],
        [('p', 0, MAX), ('p', 0, MIN)],
        [('p', 0, MAX), ('p', 1, -1), ('p', 1, 1)],
        [('p', 0, 1), ('p', 1, MIN), ('p', 2, -1), ('p', 0, 2)],
        [('p', 0, 1), ('p', 1, MIN), ('p', 2, -1), ('r', 0)],
        [('p', 0, 1), ('p', 1, MIN), ('p', 2, -1), ('p', 0, 1)],
        [('p', 0, 0), ('p', 1, 0), ('r', 0), ('r', 1), ('r', 1)],
    ]
    rng = random.Random(SEED)
    boundaries = [MIN, MIN+1, MAX, MAX-1, -(1 << 53)-1, (1 << 53)+1, -1, 0, 1]
    for i in range(128):
        ops = []
        for _ in range(40):
            slot = rng.randrange(12)
            if rng.random() < .25:
                ops.append(('r', slot))
            else:
                value = rng.choice(boundaries) if i % 2 else rng.randrange(-10000, 10001)
                ops.append(('p', slot, value))
        ops.extend(('r', slot) for slot in range(12))
        operations.append(ops)
    return [{'id': f'average-{i:03}', 'operations': ops} for i, ops in enumerate(operations)]


def validate(cases, lines):
    reference = [line for case in cases for line in expected(case)]
    if lines != reference:
        mismatch = next(((i, a, b) for i, (a, b) in enumerate(zip(lines, reference)) if a != b), None)
        raise ValueError(f'Average result mismatch: actual/expected rows {len(lines)}/{len(reference)}; first mismatch {mismatch}')
    return len(reference), sum(line.endswith('\tOVERFLOW') for line in reference)


def run_checks(classpath):
    cases = generate()
    payload = '\n'.join(case['id'] + '\t' + ';'.join(':'.join(map(str, op)) for op in case['operations']) for case in cases) + '\n'
    outputs = {}
    for runtime, command in {
        'java': ['java', '-cp', classpath, 'AverageCollectorOracle'],
        'rust': [str(ROOT / 'target/release/examples/average_oracle')],
    }.items():
        result = subprocess.run(command, input=payload, text=True, capture_output=True, timeout=60, check=True)
        outputs[runtime] = result.stdout.splitlines()
        observations, rejected = validate(cases, outputs[runtime])
    return {'seed': SEED, 'total': len(cases), 'passed': len(cases),
            'observations': observations, 'expected_overflow_rejections': rejected,
            'cases': cases, 'outputs': outputs,
            'scope': 'Original standalone integer-average collector sequences through public Community handles and native try methods. Python independently checks empty results, exact IEEE-754 bits, checked insert/remove sums and checked replacement differences with wrapping replacement totals. Stop each case on its first expected overflow; no Java post-error state or score-stream lifecycle claim. No added upstream-method credit or timing measurement.'}
