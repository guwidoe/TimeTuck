"""Original exact integer/decimal collector comparison through public APIs."""
import random
import subprocess
from differential_scores import ROOT

SEED = 20261103
KINDS = ['integer_sum', 'integer_average', 'decimal_sum', 'decimal_average']


def expected(case):
    slots = {}
    scale = 0
    out = []
    def emit(state):
        total = sum(coefficient * 10**(scale - exponent) for coefficient, exponent in slots.values())
        if case['kind'].endswith('average'):
            if not slots:
                value = '-'
            else:
                quotient, remainder = divmod(abs(total), len(slots))
                if remainder * 2 > len(slots) or (remainder * 2 == len(slots) and quotient % 2):
                    quotient += 1
                value = f'{-quotient if total < 0 else quotient}:{scale}'
        else:
            value = str(total) + (f':{scale}' if case['kind'].startswith('decimal') else '')
        out.append(f'{case["id"]}\t{state}\t{value}')
    emit(0)
    for state, (op, slot, *values) in enumerate(case['operations'], 1):
        if op == 'p':
            coefficient, exponent = values
            if not case['kind'].startswith('decimal') and exponent != 0:
                raise ValueError('Integer input must have zero scale')
            slots[slot] = (coefficient, exponent)
            scale = max(scale, exponent)
        elif op == 'r':
            slots.pop(slot, None)
        else:
            raise ValueError('Unknown operation')
        emit(state)
    return out


def generate():
    rng = random.Random(SEED)
    fixed = [[], [('p',0,1,0),('p',1,10,0),('r',0),('r',1)],
        [('p',0,10**1000+1,0),('p',1,-10**1000,0),('r',0),('r',1)]]
    fixed += [[('p',0,v,0),('p',1,0,0)] for v in [3,5,7,-3,-5,-7]]
    decimal = [
        [('p',0,35,1),('p',1,0,0)],
        [('p',0,1,0),('p',1,0,2),('r',1),('r',0),('p',2,7,0),('p',3,0,0)],
        [('p',0,1,-3),('r',0)],
        [('p',0,1,0),('p',1,0,2),('p',1,0,0)],
        [('p',0,35,1),('p',1,0,1),('p',0,350,2)],
        [('p',0,1,0),('p',1,10,1),('p',2,100,2),('r',1),('r',2)],
    ]
    cases = []
    for kind in KINDS:
        sequences = fixed + (decimal if kind.startswith('decimal') else [])
        for _ in range(64):
            ops = []
            for _ in range(50):
                slot = rng.randrange(12)
                if rng.random() < .25:
                    ops.append(('r',slot))
                else:
                    coefficient = rng.getrandbits(rng.choice([8,64,256,1024])) * rng.choice([-1,0,1])
                    scale = rng.randrange(-12,25) if kind.startswith('decimal') else 0
                    ops.append(('p',slot,coefficient,scale))
            ops += [('r',slot) for slot in range(12)]
            sequences.append(ops)
        cases += [{'id':f'big-{kind}-{i:03}','kind':kind,'operations':ops} for i,ops in enumerate(sequences)]
    return cases


def encode(case):
    decimal = case['kind'].startswith('decimal')
    operations = []
    for op,slot,*values in case['operations']:
        operations.append(f'{op}:{slot}' + (f':{values[0]}' + (f':{values[1]}' if decimal else '') if op=='p' else ''))
    return case['id']+'\t'+case['kind']+'\t'+';'.join(operations)


def validate(cases, lines):
    reference = [line for c in cases for line in expected(c)]
    if lines != reference:
        mismatch = next(((i,a,b) for i,(a,b) in enumerate(zip(lines,reference)) if a!=b),None)
        raise ValueError(f'Big-number results differ: actual/expected rows {len(lines)}/{len(reference)}; first mismatch {mismatch}')
    return len(reference)


def run_checks(classpath):
    cases = generate()
    payload = '\n'.join(map(encode,cases))+'\n'
    outputs = {}
    for runtime, command in {'java':['java','-cp',classpath,'BigNumberCollectorOracle'],
                             'rust':[str(ROOT/'target/release/examples/big_number_oracle')]}.items():
        result = subprocess.run(command,input=payload,text=True,capture_output=True,check=True,timeout=60)
        outputs[runtime] = result.stdout.splitlines()
        observations = validate(cases,outputs[runtime])
    return {'seed':SEED,'total':len(cases),'passed':len(cases),'observations':observations,
            'cases':cases,'outputs':outputs,
            'scope':'Original standalone arbitrary-precision integer/decimal sum and average sequences through Community public handles and native collectors. Python recomputes totals from captured values at the historical maximum scale, independently rounds half-even, and checks exact coefficients/scales. Includes 1001-digit inputs, negative values, ties, duplicates, removal/reinsertion, negative scales and historical precision. No extra upstream credit, stream lifecycle or timing claim.'}
