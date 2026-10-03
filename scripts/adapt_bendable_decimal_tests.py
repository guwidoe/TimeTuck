#!/usr/bin/env python3
"""Adapt the pinned BendableBigDecimalScoreTest, never implementation sources."""
import json
import re
import subprocess
from adapt_score_tests import ROOT, REF, args, group, snake
from adapt_decimal_score_tests import translate

SOURCE = 'core/src/test/java/ai/timefold/solver/core/api/score/BendableBigDecimalScoreTest.java'
LOCAL = 'crates/solver/tests/bendable_decimal_score.rs'
ALIASES = {(1, 2): 'Hss', (2, 3): 'Hhsss', (2, 1): 'Hhs', (0, 0): 'Empty', (0, 1): 'Soft'}


def constants(source):
    result = {}
    for name, value in re.findall(r'private static final BigDecimal (\w+) = (.*?);', source):
        if value in ['BigDecimal.ZERO', 'BigDecimal.ONE', 'BigDecimal.TEN']:
            literal = {'BigDecimal.ZERO': '0', 'BigDecimal.ONE': '1', 'BigDecimal.TEN': '10'}[value]
        elif value.startswith('BigDecimal.valueOf('):
            inner, end = group(value, value.index('('))
            if end != len(value): raise ValueError(value)
            literal = str(int(inner.replace('Integer.MIN_VALUE', '-2147483648')))
        elif value.startswith('new BigDecimal('):
            inner, end = group(value, value.index('('))
            if end != len(value): raise ValueError(value)
            literal = json.loads(inner)
        elif value.endswith('.negate()') and value[:-9] in {**result, 'BigDecimal.TEN':'10'}:
            literal = '-' + {**result, 'BigDecimal.TEN':'10'}[value[:-9]]
        else:
            raise ValueError(value)
        result[name] = literal
    return result


def constructor(h, s, values, structural=None):
    if len(values) != h+s: raise ValueError('Unexpected constructor dimensions')
    arrays = f'[{", ".join(values[:h])}], [{", ".join(values[h:])}]'
    return f'{ALIASES[h,s]}::new({arrays})' if structural is None else f'{ALIASES[h,s]}::from_parts({structural}, {arrays})'


def convert(body, named):
    for name, literal in named.items():
        body = re.sub(rf'(?<![.\w]){name}\b', lambda _: f'd({json.dumps(literal)})', body)
    body = re.sub(r'new BigDecimal\(("[^"]*")\)', r'd(\1)', body)
    body = re.sub(r'BigDecimal\.valueOf\((-?\d+)\)', r'd("\1")', body)
    body = body.replace('BigDecimal.ONE', 'd("1")').replace('BigDecimal.ZERO', 'd("0")')
    body = re.sub(r'new BendableBigDecimalScore\(([^,]+),\s*new BigDecimal\[\]\s*\{([^}]*)\},\s*new BigDecimal\[\]\s*\{([^}]*)\}\s*\)',
                  lambda m: constructor(len(args(m[2])), len(args(m[3])), args(m[2])+args(m[3]), m[1]), body)
    body = re.sub(r'new BendableBigDecimalScoreDefinition\((\d+),\s*(\d+)\)',
                  lambda m: 'definition'+ALIASES[int(m[1]), int(m[2])], body)
    definitions = [('scoreDefinitionHSS',1,2), ('scoreDefinitionHHSSS',2,3)]
    definitions += [('definition'+name,h,s) for (h,s),name in ALIASES.items()]
    for name,h,s in definitions:
        marker = name+'.createScore('
        while marker in body:
            start=body.index(marker)
            inner,end=group(body,start+len(marker)-1)
            body=body[:start]+constructor(h,s,args(inner))+body[end:]
        body=re.sub(rf'{name}\.parseScore\(("[^"]*")\)', lambda m: f'{m[1]}.parse::<{ALIASES[h,s]}>().unwrap()', body)
    for level in ['Hard','Soft']:
        marker='BendableBigDecimalScore.of'+level+'('
        while marker in body:
            start=body.index(marker); inner,end=group(body,start+len(marker)-1); values=args(inner)
            if len(values)!=4: raise ValueError(inner)
            h,s,index=map(int,values[:3])
            body=body[:start]+f'{ALIASES[h,s]}::of_{level.lower()}({index}, {values[3]}).unwrap()'+body[end:]
    if 'scoreDefinition' in body or 'new Bendable' in body or 'new BigDecimal' in body:
        raise ValueError('Unrecognized Java expression: '+body)
    return body


def main():
    subprocess.run(['python3',str(ROOT/'scripts/upstream_inventory.py')],check=True)
    text=(ROOT/REF['path']/SOURCE).read_text()
    methods=re.findall(r'@Test\s+void (\w+)\(\)\s*\{(.*?)\n    \}',text,re.S)
    if len(methods)!=26 or text.count('@Test')!=26: raise ValueError('Review changed source tests')
    named=constants(text)
    output=f'''// Adapted from Timefold Community {REF['tag']}, {SOURCE}.
// Commit: {REF['commit']}; Apache-2.0. See THIRD_PARTY_NOTICES.md.
// Generated from tests only by scripts/adapt_bendable_decimal_tests.py.
// Native const dimensions replace definitions; all assertions/values preserved.
// Decimal factors, u32 powers, Result/Option, ZERO and Rust Eq/Ord/Hash helpers.
mod common;
use common::{{distinct, equal, ordered}};
use std::ops::{{Add, Neg, Sub}};
use timetuck::{{BendableDecimalScore, Decimal}};
fn d(s: &str) -> Decimal {{ s.parse().unwrap() }}
'''
    for (h,s),alias in ALIASES.items(): output+=f'type {alias}=BendableDecimalScore<{h},{s}>;\n'
    records=[]
    for method,body in methods:
        if method=='zero':
            if body.count('softly.assertThat')!=3: raise ValueError(body)
            lines=['assert_eq!(Soft::ZERO,Soft::new([],[d("0")]));','assert!(Soft::ZERO.is_zero());','assert!(!Soft::of_soft(0,d("1")).unwrap().is_zero());']
        elif method=='getHardOrSoftScore':
            if body.count('assertThat(')!=3: raise ValueError(body)
            lines=['let score=Hss::new([d("-5")],[d("-10"),d("-200")]);',
                   'assert_eq!(score.level(0),Some(&d("-5")));','assert_eq!(score.level(1),Some(&d("-10")));','assert_eq!(score.level(2),Some(&d("-200")));']
        elif method=='parseScoreIllegalArgument':
            literals=re.findall(r'parseScore\(("[^"]*")\)',body)
            if len(literals)!=2: raise ValueError(body)
            lines=[f'assert!({s}.parse::<Hss>().is_err());' for s in literals]
        else:
            lines=translate(convert(body,named),method,'BendableBigDecimalScore','BendableDecimalScore',0)
        output+=f'\n#[test]\nfn {snake(method)}() {{\n'+ '\n'.join(lines)+'\n}\n'
        records.append({'id':'BendableBigDecimalScoreTest.'+method,'status':'implemented',
                        'upstream_commit':REF['commit'],'upstream_path':SOURCE,'upstream_test':method,
                        'local_path':LOCAL,'local_test':snake(method),'license':'Apache-2.0',
                        'notice_paths':['THIRD_PARTY_NOTICES.md','compat/licenses/timefold-Apache-2.0.txt'],
                        'modifications':'Preserve every assertion, decimal literal/constant and shape. Const-generic arrays replace Java score definitions; dimension-checked FromStr replaces definition parsing. Exact Decimal factors and u32 powers replace tested double literals. Native Result/Option/borrowed lookup, ZERO and Rust Eq/Ord/Hash helpers replace Java plumbing. No JVM ABI or mutable-array parity claimed.'})
    (ROOT/LOCAL).write_text(output)
    path=ROOT/'compat/provenance.json'; ledger=json.loads(path.read_text()); ids={r['id'] for r in records}
    ledger['cases']=[r for r in ledger['cases'] if r['id'] not in ids]+records
    path.write_text(json.dumps(ledger,indent=2)+'\n')
    subprocess.run(['cargo','fmt','--all'],cwd=ROOT,check=True)
    print(f'Adapted {len(records)} Bendable decimal methods')


if __name__=='__main__': main()
