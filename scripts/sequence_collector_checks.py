"""Original full-field public sequence comparisons, with explicit native differences."""
import random
import subprocess
from differential_scores import ROOT

SEED = 20261220


def generate():
    rng = random.Random(SEED)
    cases = []
    for i in range(64):
        points = [rng.randrange(-8, 9) for _ in range(16)]
        operations = [('c', 0, 0), ('c', 1, 0), ('c', 2, 1), ('c', 0, -1)]
        operations += [('t' if rng.random() < .4 else 'c', rng.randrange(12),
                        -1 if rng.random() < .25 else rng.randrange(16)) for _ in range(40)]
        operations += [('c', j, -1) for j in range(12)]
        cases.append({'id': f'sequence-{i:02}', 'points': points, 'operations': operations})
    return cases


def states(case):
    slots = {}
    yield slots.copy()
    for kind, slot, item in case['operations']:
        old = slots.copy()
        if item < 0:
            slots.pop(slot, None)
        else:
            slots[slot] = item
        yield slots.copy()
        if kind == 't':
            slots = old
            yield slots.copy()


def components(points, ids):
    # Independent graph reachability over all pairs, not adjacent-index scanning.
    remaining = set(ids)
    groups = []
    while remaining:
        pending = [min(remaining, key=lambda i: (points[i], i))]
        reached = set()
        while pending:
            i = pending.pop()
            if i in reached:
                continue
            reached.add(i)
            pending.extend(j for j in remaining - reached if abs(points[j] - points[i]) <= 1)
        remaining -= reached
        groups.append(sorted(reached, key=lambda i: (points[i], i)))
    return groups


def validate(cases, lines, java=False, *, wrapping=False):
    rows = [(c, i, slots) for c in cases for i, slots in enumerate(states(c))]
    if len(rows) != len(lines):
        raise ValueError('Sequence observation count differs')
    for (case, state, slots), line in zip(rows, lines):
        f = line.split('\t')
        if len(f) != 4 or f[:2] != [case['id'], str(state)]:
            raise ValueError('Sequence observation identity differs')
        points = case['points']
        groups = components(points, set(slots.values()))
        ss = [] if f[2] == '-' else f[2].split(';')
        bs = [] if f[3] == '-' else f[3].split(';')
        if len(ss) != len(groups) or len(bs) != max(0, len(groups)-1):
            raise ValueError('Sequence or break count differs')
        def distance(a, b):
            v = points[b] - points[a]
            return (v + 2**31) % 2**32 - 2**31 if java or wrapping else v
        gaps = [distance(a[-1], b[0]) for a, b in zip(groups, groups[1:])]
        ends = []
        for i, (raw, group) in enumerate(zip(ss, groups)):
            parts = raw.split(':')
            if len(parts) != 2:
                raise ValueError('Malformed sequence fields')
            meta = parts[0].split(',')
            ids = list(map(int, parts[1].split(',')))
            length = distance(group[0], group[-1]) + 1
            if java or wrapping:
                length = (length + 2**31) % 2**32 - 2**31
            expected = [str(len(group)), str(length),
                        str(int(i == 0)), str(int(i+1 == len(groups))),
                        '-' if i == 0 else str(gaps[i-1]),
                        '-' if i+1 == len(groups) else str(gaps[i])]
            if len(meta) != 8 or meta[:6] != expected:
                raise ValueError('Sequence metadata differs')
            if sorted(ids) != sorted(group) or any(points[a] > points[b] for a, b in zip(ids, ids[1:])):
                raise ValueError('Sequence membership or index order differs')
            if not java and ids != group:
                raise ValueError('Native stable tie order differs')
            if list(map(int, meta[6:])) != [ids[0], ids[-1]]:
                raise ValueError('Sequence endpoint identities differ')
            ends.append((ids[0], ids[-1]))
        for i, raw in enumerate(bs):
            expected = [gaps[i], int(i == 0), int(i+1 == len(bs)), ends[i][1], ends[i+1][0]]
            if list(map(int, raw.split(','))) != expected:
                raise ValueError('Break metadata differs')
    return len(rows)


def run_checks(classpath):
    cases = generate()
    boundary = {'id': 'sequence-int-overflow', 'points': [-2**31, 2**31-1],
                'operations': [('c', 0, 0), ('c', 1, 1)]}
    inputs = cases + [boundary]
    payload = ''.join(c['id']+'\t'+','.join(map(str, c['points']))+'\t'+
                      ';'.join(':'.join(map(str, o)) for o in c['operations'])+'\n' for c in inputs)
    outputs = {}
    ordinary_observations = None
    for runtime, command in {
        'java': ['java', '-cp', classpath, 'SequenceCollectorOracle'],
        'rust_exact': [str(ROOT/'target/release/examples/sequence_oracle')],
        'rust': [str(ROOT/'target/release/examples/sequence_oracle'),'--wrapping32'],
    }.items():
        r = subprocess.run(command, input=payload, text=True, capture_output=True, check=True, timeout=60)
        lines = r.stdout.splitlines()
        observations = validate(cases, lines[:-3], java=runtime == 'java',wrapping=runtime == 'rust')
        validate([boundary], lines[-3:], java=runtime == 'java',wrapping=runtime == 'rust')
        if ordinary_observations is None:
            ordinary_observations = observations
        else:
            assert observations == ordinary_observations
        outputs[runtime] = lines
    return {'seed': SEED, 'total': len(inputs), 'passed': len(inputs),
            'native_reference_passed':len(inputs),'observations':ordinary_observations+3,
            'ordinary_total':len(cases),'ordinary_passed':len(cases),
            'boundary_total':1,'boundary_passed':1,'boundary_divergences':0,
            'arithmetic_policy':'wrapping32','boundary': boundary, 'cases': cases, 'outputs': outputs,
            'exact_controls':{'total':len(cases),'passed':len(cases),'observations':ordinary_observations,
                'boundary_divergences':1,'output_keys':{'java':'java','rust':'rust_exact'}},
            'scope': 'Explicit WrappingIntegerSequenceMetric with signed i32 indices is selected for the Community comparison. '
            'All original cases and the original MIN/MAX boundary pass with modular 32-bit distances; '
            'the default exact i64 controls and their separate overflow difference remain under exact_controls and outputs.rust_exact. '
            'Public standalone collector add, replace, remove and trial/undo. '
            'Python checks membership, count, length, first/last flags, neighboring breaks and endpoint identities. '
            'Java ties may use different identity ordering; each observed endpoint must match its own ordered items. '
            'Rust additionally requires stable logical-ID ties. Snapshots use owned values; '
            'Java live views and mutable object aliasing are not claimed. No timing or upstream-method credit.'}
