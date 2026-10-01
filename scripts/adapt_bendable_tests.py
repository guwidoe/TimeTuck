#!/usr/bin/env python3
"""Adapt the pinned BendableScore tests only; never read production sources."""
import json
import re
import subprocess
from adapt_score_tests import ROOT, REF, args, snake, translate

SOURCE = 'core/src/test/java/ai/timefold/solver/core/api/score/BendableScoreTest.java'
LOCAL = 'crates/solver/tests/bendable_score.rs'
ALIASES = {(1,2):'Hss', (2,3):'Hhsss', (2,1):'Hhs', (0,0):'Empty', (0,1):'Soft'}


def constructor(h, s, values):
    values = args(values)
    if len(values) != h+s: raise ValueError('Unexpected constructor shape')
    return f'{ALIASES[h,s]}::new([{", ".join(values[:h])}], [{", ".join(values[h:])}])'


def convert(body):
    body = re.sub(r'new BendableScore\(([^,]+),\s*new long\[\]\s*\{([^}]*)\},\s*new long\[\]\s*\{([^}]*)\}\s*\)',
                  lambda m: constructor(len(args(m[2])), len(args(m[3])), m[2]+','+m[3])+f'.with_structural({m[1]})', body)
    for name,h,s in [('scoreDefinitionHSS',1,2),('scoreDefinitionHHSSS',2,3)]:
        body = re.sub(rf'{name}\.createScore\(([^)]*)\)', lambda m: constructor(h,s,m[1]), body)
        body = re.sub(rf'{name}\.parseScore\(("[^"]*")\)', lambda m: f'{m[1]}.parse::<{ALIASES[h,s]}>().unwrap()', body)
    body = re.sub(r'new BendableScoreDefinition\((\d+),\s*(\d+)\)\.createScore\(([^)]*)\)',
                  lambda m: constructor(int(m[1]),int(m[2]),m[3]), body)
    body = re.sub(r'BendableScore\.of(Hard|Soft)\((\d+),\s*(\d+),\s*(\d+),\s*([^)]*)\)',
                  lambda m: f'{ALIASES[int(m[2]),int(m[3])]}::of_{m[1].lower()}({m[4]}, {m[5]}).unwrap()', body)
    if 'scoreDefinition' in body or 'new Bendable' in body: raise ValueError('Unrecognized Java expression')
    return body


def main():
    subprocess.run(['python3',str(ROOT/'scripts/upstream_inventory.py')],check=True)
    source = (ROOT/REF['path']/SOURCE).read_text()
    methods = re.findall(r'@Test\s+void (\w+)\(\)\s*\{(.*?)\n    \}',source,re.S)
    if len(methods) != 27: raise ValueError('Review changed upstream test methods')
    output = f'''// Adapted from Timefold Community {REF['tag']}, {SOURCE}.
// Commit: {REF['commit']}; Apache-2.0. See THIRD_PARTY_NOTICES.md.
// Generated from tests only by scripts/adapt_bendable_tests.py.
// Compile-time shapes replace score definitions. Assertions and values preserved.
mod common;
use common::{{equal, distinct, ordered}};
use std::ops::{{Add, Neg, Sub}};
use timetuck::BendableScore;
'''
    for (h,s),alias in ALIASES.items(): output += f'type {alias} = BendableScore<{h}, {s}>;\n'
    cases=[]
    for method,body in methods:
        if method == 'zero':
            if body.count('softly.assertThat') != 3: raise ValueError(body)
            code='    assert_eq!(Soft::ZERO, Soft::new([], [0]));\n    assert!(Soft::ZERO.is_zero());\n    assert!(!Soft::of_soft(0, 1).unwrap().is_zero());'
        elif method == 'getHardOrSoftScore':
            if body.count('assertThat(') != 3: raise ValueError(body)
            code='    let score = Hss::new([-5], [-10, -200]);\n    assert_eq!(score.level(0), Some(-5));\n    assert_eq!(score.level(1), Some(-10));\n    assert_eq!(score.level(2), Some(-200));'
        elif method == 'parseScoreIllegalArgument':
            strings=re.findall(r'parseScore\(("[^"]*")\)',body)
            if len(strings) != 2: raise ValueError(body)
            code='\n'.join(f'    assert!({s}.parse::<Hss>().is_err());' for s in strings)
        else:
            code=translate(convert(body),method,'BendableScore')
        output+=f'\n#[test]\nfn {snake(method)}() {{\n{code}\n}}\n'
        cases.append({'id':'BendableScoreTest.'+method,'status':'implemented','upstream_commit':REF['commit'],
                      'upstream_path':SOURCE,'upstream_test':method,'local_path':LOCAL,'local_test':snake(method),
                      'license':'Apache-2.0','notice_paths':['THIRD_PARTY_NOTICES.md','compat/licenses/timefold-Apache-2.0.txt'],
                      'modifications':'Preserve every assertion and score value. Native const-generic hard/soft arrays replace Java score definitions; dimension-checked FromStr replaces definition parsing. Rust Eq/Ord/Hash helpers preserve equality and ordering assertions. Single-level constructors and lookup return Option; valid results are unwrapped or compared to Some. ZERO replaces same-shape zero(). No Java ABI or mutable-array parity claimed.'})
    (ROOT/LOCAL).write_text(output)
    p=ROOT/'compat/provenance.json'; ledger=json.loads(p.read_text()); ids={c['id'] for c in cases}
    ledger['cases']=[c for c in ledger['cases'] if c['id'] not in ids]+cases
    p.write_text(json.dumps(ledger,indent=2)+'\n')
    subprocess.run(['cargo','fmt','--all'],cwd=ROOT,check=True)
    print(f'Adapted {len(cases)} complete BendableScore methods')

if __name__=='__main__': main()
