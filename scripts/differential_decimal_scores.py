#!/usr/bin/env python3
"""Original decimal score comparisons against Community APIs and Python fractions."""
import json
import random
import re
import subprocess
from decimal import Decimal
from fractions import Fraction
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LABELS = {'simple': [''], 'hard_soft': ['hard', 'soft'],
          'hard_medium_soft': ['hard', 'medium', 'soft']}
SEED = 20261212
BENDABLE_SHAPES = [(1, 2), (2, 3), (0, 0), (0, 2), (2, 0)]


def parts(text, labels, shape=None):
    structural = 0
    raw = 'structural/' in text
    if raw:
        prefix, text = text.split('structural/')
        structural = -2**63 if prefix == '*' else int(prefix)
        if not -2**63 <= structural < 2**63:
            raise ValueError('structural range')
    if shape is None:
        segments = text.split('/')
    else:
        groups = re.fullmatch(r'\[([^\[\]]*)\]hard/\[([^\[\]]*)\]soft', text)
        if groups is None:
            raise ValueError('groups')
        split = [group.split('/') if group else [] for group in groups.groups()]
        if tuple(map(len, split)) != shape:
            raise ValueError('dimensions')
        segments = split[0] + split[1]
    if len(segments) != len(labels):
        raise ValueError('arity')
    values = []
    for piece, label in zip(segments, labels):
        if not piece.endswith(label):
            raise ValueError('label')
        number = piece[:-len(label)] if label else piece
        if not re.fullmatch(r'[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?', number):
            raise ValueError('number')
        d = Decimal(number)
        if not -2**31 <= -d.as_tuple().exponent < 2**31:
            raise ValueError('scale')
        values.append(d.copy_abs() if d == 0 else d)
    if shape is None and not raw and all(v == 0 for v in values):
        values = [Decimal(0)] * len(labels)
    return structural, values


def quantized(value, scale):
    # Exact rational floor, independent of Rust's coefficient rescaling.
    integer = (value * Fraction(10)**scale).__floor__()
    return Decimal((int(integer < 0), tuple(map(int, str(abs(integer)))), -scale))


def expected(case):
    kind, op, left, right = (case[k] for k in ['type','operation','a','b'])
    shape = tuple(map(int, kind.split('_')[1:])) if kind.startswith('bendable_') else None
    labels = [''] * sum(shape) if shape is not None else LABELS[kind]
    try:
        structural, values = parts(left, labels, shape)
        if op in ['add','sub','cmp']:
            other_structural, other = parts(right, labels, shape)
        if op == 'cmp':
            a, b = (structural, values), (other_structural, other)
            return f'{(a > b)-(a < b)}|{str(a == b).lower()}'
        if op != 'parse' and not (shape is None and op == 'neg' and structural == 0 and all(v == 0 for v in values)):
            output = []
            for i, value in enumerate(values):
                scale = -value.as_tuple().exponent
                v = Fraction(value)
                if op in ['add','sub']:
                    scale = max(scale, -other[i].as_tuple().exponent)
                    v = v + Fraction(other[i]) if op == 'add' else v - Fraction(other[i])
                elif op == 'neg':
                    scale = max(scale, 0) if shape is None else scale
                    v = -v
                elif op == 'abs':
                    v = abs(v)
                elif op in ['mul','div']:
                    scale = max(scale,0)
                    factor = Fraction(Decimal(right))
                    v = v * factor if op == 'mul' else v / factor
                elif op == 'pow':
                    scale = max(scale,0)
                    v = v ** int(right)
                else:
                    raise ValueError('op')
                output.append(quantized(v,scale))
            structural, values = 0, output
            if shape is None and all(v == 0 for v in values):
                values = [Decimal(0)] * len(labels)
        if shape is None:
            full_body = '/'.join(str(v)+label for v,label in zip(values,labels))
            short_parts = [str(v)+label for v,label in zip(values,labels) if v]
            hard_count = 0 if kind == 'simple' else 1
        else:
            hard_count = shape[0]
            groups = [(values[:hard_count], 'hard'), (values[hard_count:], 'soft')]
            formatted = [('['+'/'.join(map(str, group))+']'+label, any(group)) for group,label in groups]
            full_body = '/'.join(text for text, nonzero in formatted)
            short_parts = [text for text, nonzero in formatted if nonzero]
        full = (f'{structural}structural/' if structural < 0 else '') + full_body
        short = '/'.join(([f'{structural}structural'] if structural else []) + short_parts) or '0'
        feasible = structural >= 0 and all(v >= 0 for v in values[:hard_count])
        coefficients = []
        for v in values:
            sign,digits,exponent = v.as_tuple()
            coefficient = int(''.join(map(str,digits))) * (-1 if sign else 1)
            coefficients.append(f'{coefficient}@{-exponent}')
        return f'{structural}|{full}|{short}|{str(feasible).lower()}|'+','.join(coefficients)
    except (ValueError, ArithmeticError):
        return 'ERROR'


def generate():
    rng = random.Random(SEED)
    values = ['0','0.00','-0.000','1.00','-1.23','123456789012345678901234567890.12345',
              '-99999999999999999999999999999.99','1E+20','1E-20','0.000001','0.0000001',
              '-2.5000','1.999','-1.999','1E+2']
    cases = []
    for kind, labels in LABELS.items():
        def score(levels, prefix=None):
            return (f'{prefix}structural/' if prefix is not None else '') + '/'.join(v+l for v,l in zip(levels,labels))
        for i in range(120):
            levels = [rng.choice(values) if i<50 else str(Decimal((rng.randrange(2),tuple(map(int,str(rng.randrange(1,10**18)))),rng.randint(-12,6)))) for _ in labels]
            a = score(levels, rng.choice([None,-2,0,2]))
            b = score([rng.choice(values) for _ in labels], rng.choice([None,-1,0]))
            for op in ['parse','add','sub','neg','abs','mul','div','pow','cmp']:
                rhs = b if op in ['add','sub','cmp'] else str(rng.randrange(5)) if op == 'pow' else rng.choice(['-2','0','0.5','1.2','2','5','1.25'])
                cases.append({'type':kind,'operation':op,'a':a,'b':rhs,'expect_error':op == 'div' and Decimal(rhs) == 0})
        for literal in ['0.00','1E-2147483647','1E+2147483648','1.0E+2147483649','+1.2','.5','5.','0001.20']:
            for prefix in [None,-1,0,2,'*']:
                cases.append({'type':kind,'operation':'parse','a':score([literal]*len(labels),prefix),'b':''})
        for literal in ['', 'NaN','Infinity',' 1','1_0','1e-2147483648','1e2147483649','.', '+', '*']:
            cases.append({'type':kind,'operation':'parse','a':score([literal]*len(labels)),'b':'','expect_error':True})
    # Independent stream preserves the established fixed-family corpus.
    rng = random.Random(SEED + 2)
    for hard, soft in BENDABLE_SHAPES:
        kind = f'bendable_{hard}_{soft}'
        def score(levels, prefix=None):
            return (f'{prefix}structural/' if prefix is not None else '') + '['+'/'.join(levels[:hard])+']hard/['+'/'.join(levels[hard:])+']soft'
        for i in range(100):
            levels = [rng.choice(values) if i < 50 else str(Decimal((rng.randrange(2), tuple(map(int,str(rng.randrange(1,10**18)))), rng.randint(-12,6)))) for _ in range(hard+soft)]
            a = score(levels, rng.choice([None,-2,0,2]))
            b = score([rng.choice(values) for _ in levels], rng.choice([None,-1,0]))
            for op in ['parse','add','sub','neg','abs','mul','div','pow','cmp']:
                rhs = b if op in ['add','sub','cmp'] else str(rng.randrange(5)) if op == 'pow' else rng.choice(['-2','0','0.5','1.2','2','5','1.25'])
                cases.append({'type':kind,'operation':op,'a':a,'b':rhs,'expect_error':op == 'div' and hard+soft > 0 and Decimal(rhs) == 0})
        for literal in ['0.00','1E-2147483647','1E+2147483648','1.0E+2147483649','+1.2','.5','5.','0001.20']:
            for prefix in [None,-1,0,2,'*']:
                cases.append({'type':kind,'operation':'parse','a':score([literal]*(hard+soft),prefix),'b':''})
        if hard+soft:
            for literal in ['', 'NaN','Infinity',' 1','1_0','1e-2147483648','1e2147483649','.', '+', '*']:
                cases.append({'type':kind,'operation':'parse','a':score([literal]*(hard+soft)),'b':'','expect_error':True})
        valid = score(['1']*(hard+soft))
        malformed = ['', '-147', '[1]hard', valid+'junk', valid.replace('hard/','soft/'), 'badstructural/'+valid,
                     '['+'/'.join(['1']*(hard+1))+']hard/['+'/'.join(['1']*soft)+']soft',
                     '['+'/'.join(['1']*hard)+']hard/['+'/'.join(['1']*(soft+1))+']soft']
        for text in malformed:
            cases.append({'type':kind,'operation':'parse','a':text,'b':'','expect_error':True})
    return cases


def result_matches(case, results):
    return len(set(results)) == 1 and (results[0] == 'ERROR') == case.get('expect_error', False)


def run_checks(jar):
    subprocess.run(['javac','-cp',str(jar),'-d',str(ROOT/'work/oracle'),str(ROOT/'compat/oracle/DecimalScoreOracle.java')],check=True)
    subprocess.run(['cargo','build','--release','--example','decimal_score_oracle','--locked'],cwd=ROOT,check=True)
    cases = generate()
    payload = ''.join('\t'.join(c[k] for k in ['type','operation','a','b'])+'\n' for c in cases)
    commands = {'java':['java','-cp',f'{ROOT / "work/oracle"}:{jar}','DecimalScoreOracle'],
                'rust':[str(ROOT/'target/release/examples/decimal_score_oracle')]}
    outputs = {k:subprocess.run(cmd,input=payload,text=True,capture_output=True,check=True).stdout.splitlines() for k,cmd in commands.items()}
    outputs['python'] = [expected(c) for c in cases]
    if any(len(v)!=len(cases) for v in outputs.values()):
        raise RuntimeError('Decimal response count mismatch')
    failures = [{'case':c, **{k:values[i] for k,values in outputs.items()}} for i,c in enumerate(cases)
                if not result_matches(c, [values[i] for values in outputs.values()])]
    report = {'seed':SEED,'total':len(cases),'passed':len(cases)-len(failures),
              'bendable_seed':SEED+2,'bendable_shapes':BENDABLE_SHAPES,
              'fixed_total':sum(not c['type'].startswith('bendable_') for c in cases),
              'bendable_total':sum(c['type'].startswith('bendable_') for c in cases),
              'expected_errors':outputs['python'].count('ERROR'),'failures':failures,
              'scope':'Original three fixed decimal families and five Bendable shapes (including empty groups), dimension-checked parsing, exact coefficients/scales, full/short text, structural state, feasibility, numeric comparison/equality, arithmetic. Native exact decimal factors correspond to the selected finite Java double literals; powers are nonnegative integers. Independent Python Fraction arithmetic. No general f64-input, fractional-power, decimal timing or new source-method coverage claim.'}
    (ROOT/'work/decimal-score-differential.json').write_text(json.dumps({'cases':cases,'outputs':outputs,**report},indent=2)+'\n')
    print(f'Decimal score comparisons: {report["passed"]}/{report["total"]}; expected errors {report["expected_errors"]}')
    for failure in failures[:5]: print(failure)
    if failures: raise RuntimeError('Decimal score comparison failed')
    return report


if __name__ == '__main__':
    from differential_scores import JAR, prepare_java
    prepare_java()
    run_checks(JAR)
