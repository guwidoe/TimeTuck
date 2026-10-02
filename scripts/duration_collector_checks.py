"""Original signed-duration arithmetic and public collector conformance checks."""
import random
import subprocess
from differential_scores import ROOT

SEED = 20261031
BILLION = 10**9
LOW, HIGH = -(1 << 63), (1 << 63) - 1
MIN, MAX = LOW * BILLION, HIGH * BILLION + BILLION - 1


def checked(value, low=MIN, high=MAX):
    if not low <= value <= high:
        raise OverflowError
    return value


def add_parts(total, seconds, nanos):
    s, n = divmod(total, BILLION)
    return checked(checked(s + seconds, LOW, HIGH) * BILLION + n + nanos)


def add(total, value):
    return add_parts(total, *divmod(value, BILLION))


def subtract(total, value):
    s, n = divmod(value, BILLION)
    if s == LOW:
        return add_parts(add_parts(total, HIGH, -n), 1, 0)
    return add_parts(total, -s, -n)


def expected(case):
    slots, total = {}, 0
    out = [f'{case["id"]}\t0\t' + ('0' if case['kind'] == 'sum' else '-')]
    for state, (op, slot, *values) in enumerate(case['operations'], 1):
        try:
            if slot in slots:
                total = subtract(total, slots.pop(slot))
            if op == 'p':
                total = add(total, values[0])
                slots[slot] = values[0]
            elif op != 'r':
                raise ValueError('Unknown operation')
        except OverflowError:
            out.append(f'{case["id"]}\t{state}\tOVERFLOW:update')
            break
        try:
            if case['kind'] == 'sum':
                result = str(total)
            elif not slots:
                result = '-'
            else:
                checked(total, LOW, HIGH)
                result = str((1 if total >= 0 else -1) * (abs(total) // len(slots)))
        except OverflowError:
            out.append(f'{case["id"]}\t{state}\tOVERFLOW:finish')
            break
        out.append(f'{case["id"]}\t{state}\t{result}')
    return out


def generate():
    fixed = [[], [('p',0,-1),('p',1,0)], [('p',0,-2),('p',1,0),('p',2,0)],
        [('p',0,MAX),('p',1,1)], [('p',0,MIN),('p',0,MAX)],
        [('p',0,MAX),('p',0,MAX)], [('p',0,1),('p',1,MIN),('p',2,-1)],
        [('p',0,1),('p',1,MIN),('r',0)], [('p',0,MIN),('p',0,MIN)],
        [('p',0,HIGH),('p',1,1)], [('p',0,LOW),('p',1,-1)],
        [('p',0,HIGH),('p',0,-HIGH)], [('p',0,HIGH),('p',1,0),('p',0,HIGH+1)],
        [('p',0,0),('p',1,0),('r',0),('r',1),('r',1)],
        [('p',0,-1),('p',1,MAX),('p',2,1),('p',0,0)],
        [('p',0,MIN),('p',1,1),('p',1,2)]]
    rng = random.Random(SEED)
    cases = []
    boundaries = [MIN, MIN+1, MIN+BILLION, MAX, MAX-1, LOW, LOW+1, HIGH, HIGH-1, -BILLION-1, -1, 0, 1, BILLION-1]
    for kind in ['sum', 'average']:
        sequences = list(fixed)
        for i in range(128):
            ops = []
            for _ in range(40):
                slot = rng.randrange(12)
                if rng.random() < .25:
                    ops.append(('r', slot))
                else:
                    value = rng.choice(boundaries) if i % 2 else rng.randrange(-10**15,10**15)
                    ops.append(('p',slot,value))
            ops.extend(('r',slot) for slot in range(12))
            sequences.append(ops)
        cases.extend({'id':f'duration-{kind}-{i:03}', 'kind':kind, 'operations':ops} for i,ops in enumerate(sequences))
    return cases


def validate(cases, lines):
    reference = [line for case in cases for line in expected(case)]
    if lines != reference:
        mismatch = next(((i,a,b) for i,(a,b) in enumerate(zip(lines,reference)) if a != b), None)
        raise ValueError(f'Duration results differ: actual/expected rows {len(lines)}/{len(reference)}; first mismatch {mismatch}')
    return len(reference), sum('OVERFLOW:' in line for line in reference)


def run_checks(classpath):
    cases = generate()
    payload = ''.join(c['id']+'\t'+c['kind']+'\t'+';'.join(':'.join(map(str,o)) for o in c['operations'])+'\n' for c in cases)
    outputs = {}
    for runtime, command in {
        'java':['java','-cp',classpath,'DurationCollectorOracle'],
        'rust':[str(ROOT/'target/release/examples/duration_oracle')],
    }.items():
        result = subprocess.run(command,input=payload,text=True,capture_output=True,check=True,timeout=60)
        outputs[runtime] = result.stdout.splitlines()
        observations, rejected = validate(cases,outputs[runtime])
    return {'seed':SEED,'total':len(cases),'passed':len(cases),'observations':observations,
        'expected_overflow_rejections':rejected,'cases':cases,'outputs':outputs,
        'scope':'Original standalone signed duration sum/average sequences. Python checks normalized seconds intermediates, subtraction then insertion on replacement, signed-64-bit total nanoseconds before average division, truncation toward zero, and update versus finish overflow. Stop at first error; no Java post-error state or stream lifecycle/timing claim. No added upstream-method credit.'}
