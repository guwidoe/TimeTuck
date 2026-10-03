"""Original Community/Rust conditional group presence, updates and undo checks."""
import random
import subprocess
from differential_scores import ROOT

SEED = 20261204


def generate():
    sequences = [[], [
        ('c', 0, (1, -1)), ('c', 1, (1, 0)), ('c', 2, (2, -2)),
        ('c', 0, (1, 7)), ('t', 0, (3, -1)), ('c', 0, (2, 5)),
        ('c', 0, None), ('c', 2, None), ('c', 1, None), ('c', 1, None),
    ]]
    rng = random.Random(SEED)
    for _ in range(32):
        ops = [(rng.choice(('c', 'c', 't')), rng.randrange(12),
                None if rng.random() < .25 else (rng.randrange(4), rng.randrange(-10, 11)))
               for _ in range(40)]
        ops.extend(('c', i, None) for i in range(12))
        sequences.append(ops)
    return [{'id': f'conditional-{i:02}', 'operations': ops} for i, ops in enumerate(sequences)]


def expected(case):
    rows = {}
    out = []

    def emit():
        # Every input establishes its group, including rejected/zero values.
        count = sum(v > 0 for _, v in rows.values())
        soft = int(bool(rows)) + sum(k * 100 for k in {k for k, _ in rows.values()})
        soft += sum(v for _, v in rows.values() if v > 0)
        out.append(f'{case["id"]}\t{len(out)}\t{count}\t{soft}')

    emit()
    for kind, slot, value in case['operations']:
        old = rows.get(slot)
        if value is None:
            rows.pop(slot, None)
        else:
            rows[slot] = value
        emit()
        if kind == 't':
            if old is None:
                rows.pop(slot, None)
            else:
                rows[slot] = old
            emit()
    return out


def validate(cases, lines):
    reference = [line for case in cases for line in expected(case)]
    if lines != reference:
        mismatch = next(((a,b) for a,b in zip(lines,reference) if a != b), None)
        raise ValueError(f'Conditional mismatch: rows {len(lines)}/{len(reference)}; {mismatch}')
    return len(reference)


def run_checks(classpath):
    cases = generate()
    payload = '\n'.join(case['id']+'\t'+';'.join(
        f'{kind}:{slot}:'+('-' if v is None else f'{v[0]},{v[1]}')
        for kind, slot, v in case['operations']) for case in cases)+'\n'
    outputs = {}
    for runtime, command in {
        'java': ['java', '-cp', classpath, 'ConditionalCollectorOracle'],
        'rust': [str(ROOT / 'target/release/examples/conditional_oracle')],
    }.items():
        result = subprocess.run(command, input=payload, text=True, capture_output=True, timeout=60, check=True)
        outputs[runtime] = result.stdout.splitlines()
        observations = validate(cases, outputs[runtime])
    return {'seed': SEED, 'total': len(cases), 'passed': len(cases),
            'observations': observations, 'cases': cases, 'outputs': outputs,
            'scope': 'Original conditional global counts, keyed sums and group-presence scores, with rejected inputs, duplicates, key transfers, replacements, removals and trial undo. Community SolutionManager.update recalculates each state; native streams update transactionally; independent Python multisets check every ordinary score. No Java match IDs, incremental notifications, timing, or additional upstream-method credit.'}
