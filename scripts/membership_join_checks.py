"""Original public membership-join comparisons; no upstream method credit."""
import random
import subprocess
from differential_scores import ROOT

SEED = 20261207


def generate(include_edge_keys=False):
    rng = random.Random(SEED)
    cases = []
    for mode in range(9):
        for sample in range(6):
            ops = []
            # Original boundaries: duplicated null and integer keys, all-empty
            # sets, identity-distinct equal rows and several shared keys.
            collections = [[], [None, None], [0, 0, 1], [None, 0, 1, 1], [2]]
            for side in ('L', 'R'):
                for i, keys in enumerate(collections):
                    ops.append(('c', side, i, [i % 2, i % 3, None if i == 1 else 0, i - 2, keys]))
            for _ in range(40):
                row = None if rng.random() < .25 else [rng.randrange(3), rng.randrange(3),
                    rng.choice([None, -1, 0, 1, 2]), rng.randrange(-9, 10),
                    [rng.choice([None, -1, 0, 1, 2]) for _ in range(rng.randrange(6))]]
                ops.append(('t' if rng.random() < .35 else 'c', rng.choice(['L', 'R']), rng.randrange(8), row))
            ops += [('c', side, i, None) for side in ('R', 'L') for i in range(8)]
            cases.append({'id': f'membership-{mode}-{sample}', 'mode': mode, 'operations': ops})
    if not include_edge_keys:
        # The agreement matrix has distinct non-null members. Preserve the raw
        # matrix separately for native checks; never call it Java agreement.
        for case in cases:
            for _, _, _, row in case['operations']:
                if row is not None:
                    row[2] = 3 if row[2] is None else row[2]
                    row[4] = list(dict.fromkeys(3 if k is None else k for k in row[4]))
    return cases


def boundary_probes():
    cases = []
    for mode in range(9):
        for name, left, right in [
            ('duplicate', [0, 0, 7, 2, [7, 7]], [0, 0, 7, 3, [7]]),
            ('null', [0, 0, None, 2, [None]], [0, 0, None, 3, [None]])]:
            # These constants characterize these exact pinned public-API inputs,
            # not a generalized substitute for Java's implementation.
            java_score = ([2, 12] if mode % 3 != 1 else [1, 6]) if name == 'duplicate' else ([0, 0] if mode in (0, 1) else [1, 6])
            cases.append({'id': f'{name}-{mode}', 'mode': mode,
                          'operations': [('c', 'L', 0, left), ('c', 'R', 0, right)],
                          'java_final_score': java_score})
    return cases


def validate_boundaries(cases, lines):
    if len(lines) != 3 * len(cases):
        raise ValueError('Boundary response count mismatch')
    for c, offset in zip(cases, range(0, len(lines), 3)):
        for i, score in enumerate([[0, 0], [0, 0], c['java_final_score']]):
            if lines[offset+i] != f'{c["id"]}\t{i}\t{score[0]}\t{score[1]}\t-':
                raise ValueError('Pinned membership boundary behavior changed')
    return len(lines)


def key(k):
    return 'n' if k is None else str(k)


def row_text(row):
    return ','.join([str(row[0]), str(row[1]), key(row[2]), str(row[3]), '/'.join(map(key, row[4]))])


def encode(case):
    ops = ';'.join(f'{kind}:{side}:{slot}:' + ('-' if row is None else row_text(row))
                   for kind, side, slot, row in case['operations'])
    return f'{case["id"]}\t{case["mode"]}\t{ops}'


def states(case):
    sides = {'L': {}, 'R': {}}
    yield {k: dict(v) for k, v in sides.items()}
    for kind, side, slot, row in case['operations']:
        if kind not in ('c', 't') or side not in sides or not 0 <= slot < 16:
            raise ValueError('Invalid operation')
        if row is not None:
            if len(row) != 5 or any(not -(1 << 31) <= x < 1 << 31 for x in [*row[:2], row[2], *row[4]] if x is not None) or not -1000 <= row[3] <= 1000:
                raise ValueError('Invalid row')
            row = (*row[:4], tuple(row[4]))
        old = sides[side].get(slot)
        if row is None:
            sides[side].pop(slot, None)
        else:
            sides[side][slot] = row
        yield {k: dict(v) for k, v in sides.items()}
        if kind == 't':
            if old is None:
                sides[side].pop(slot, None)
            else:
                sides[side][slot] = old
            yield {k: dict(v) for k, v in sides.items()}


def expected(mode, state):
    if not 0 <= mode < 9:
        raise ValueError('Invalid mode')
    def matches(a, b):
        arity = mode // 3
        if a[:arity] != b[:arity]:
            return False
        if mode % 3 == 0:
            return b[2] in a[4]
        if mode % 3 == 1:
            return a[2] in b[4]
        return any(k in b[4] for k in a[4])
    pairs = [(i, j, a, b) for i, a in sorted(state['L'].items()) for j, b in sorted(state['R'].items()) if matches(a, b)]
    return (len(pairs), sum(a[3] * b[3] for _, _, a, b in pairs)), ';'.join(
        f'{i},{j},{row_text(a)}|{row_text(b)}' for i, j, a, b in pairs)


def validate(cases, lines, runtime):
    if runtime not in ('java', 'rust'):
        raise ValueError('Unknown runtime')
    observations = [(c, i, state) for c in cases for i, state in enumerate(states(c))]
    if len(lines) != len(observations):
        raise ValueError('Membership response count mismatch')
    for line, (case, i, state) in zip(lines, observations):
        fields = line.split('\t')
        if len(fields) != 5 or fields[:2] != [case['id'], str(i)]:
            raise ValueError('Membership response identity/field mismatch')
        score, matches = expected(case['mode'], state)
        if tuple(map(int, fields[2:4])) != score or fields[4] != (matches if runtime == 'rust' else '-'):
            raise ValueError(f'Membership result mismatch: {case["id"]} state {i}')
    return len(observations)


def run_checks(classpath):
    commands = {
        'java': ['java', '-Xms256m', '-Xmx256m', '-cp', classpath, 'MembershipJoinOracle'],
        'rust': [str(ROOT / 'target/release/examples/membership_oracle')],
    }
    def execute(cases, runtime):
        payload = '\n'.join(map(encode, cases)) + '\n'
        run = subprocess.run(commands[runtime], input=payload, text=True, capture_output=True, timeout=180)
        if run.returncode:
            raise RuntimeError(f'{runtime}: {run.stderr}')
        return run.stdout.splitlines()
    cases = generate()
    outputs = {runtime: execute(cases, runtime) for runtime in commands}
    for runtime, lines in outputs.items():
        observations = validate(cases, lines, runtime)
    native_cases = generate(include_edge_keys=True)
    native_lines = execute(native_cases, 'rust')
    native_observations = validate(native_cases, native_lines, 'rust')
    probes = boundary_probes()
    probe_outputs = {runtime: execute(probes, runtime) for runtime in commands}
    validate(probes, probe_outputs['rust'], 'rust')
    probe_observations = validate_boundaries(probes, probe_outputs['java'])
    diverged = sum(c['java_final_score'] != [1, 6] for c in probes)
    return {'seed': SEED, 'total': len(cases), 'passed': len(cases), 'observations': observations,
            'cases': cases, 'outputs': outputs,
            'native_edge_checks': {'total': len(native_cases), 'passed': len(native_cases),
                'observations': native_observations, 'cases': native_cases, 'outputs': native_lines},
            'boundary_probes': {'total': len(probes), 'checked': len(probes), 'observations': probe_observations,
                'agreed': len(probes)-diverged, 'diverged': diverged, 'cases': probes, 'outputs': probe_outputs},
            'scope': 'Agreement matrix: contains, contained-in and any-common-element joins with distinct non-null members and zero, one or two equality keys. Signed/zero weights, independent left/right replacements, trials and undo. Separate native duplicate/null matrix is validated only against Python. Eighteen fixed public-API boundary probes record eight divergences: duplicated left members produce two Java matches for containing/any-of, and unpartitioned containing/contained-in skip the null match. Rust emits one pair for these inputs. Java recalculates scores with Community SolutionManager.update; Rust uses persistent membership indexes. Python validates counts, weighted scores and exact native IDs/payloads. Java exposes scores only; no Java match-analysis, incremental timing or additional upstream method credit.'}
