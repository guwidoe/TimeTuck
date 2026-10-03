#!/usr/bin/env python3
"""Adapt the three fixed decimal score test classes; never production sources.

Decimal factors are exact native Decimal literals. Java's tested integral double
powers map to u32. Every translated method retains all of its source assertions.
"""
import json
import re
import subprocess
from adapt_score_tests import ROOT, REF, args, group, snake

CLASSES = [('SimpleBigDecimalScore', 'SimpleDecimalScore', 1),
           ('HardSoftBigDecimalScore', 'HardSoftDecimalScore', 2),
           ('HardMediumSoftBigDecimalScore', 'HardMediumSoftDecimalScore', 3)]


def expression(text, java, native):
    text = re.sub(r'(-?\d+)L\b', r'\1', text.strip())
    text = re.sub(rf'{java}\s*\.', f'{java}.', text)
    text = re.sub(r'new BigDecimal\(("[^"]*")\)', r'd(\1)', text)
    text = text.replace('BigDecimal.ZERO', 'd("0")').replace('BigDecimal.ONE', 'd("1")')
    marker = f'new {java}('
    while marker in text:
        start = text.index(marker)
        inner, end = group(text, start + len(marker) - 1)
        values = args(inner)
        text = text[:start] + f'{native}::from_parts({values[0]}, [{", ".join(values[1:])}])' + text[end:]
    text = text.replace(f'{java}.of(', f'{native}::new(')
    for old, new in [('ofHard', 'of_hard'), ('ofMedium', 'of_medium'), ('ofSoft', 'of_soft')]:
        text = text.replace(f'{java}.{old}(', f'{native}::{new}(')
    text = re.sub(rf'{java}\.parseScore\(("[^"]*")\)', rf'\1.parse::<{native}>().unwrap()', text)
    for method in ['multiply', 'divide']:
        text = re.sub(rf'\.{method}\((-?\d+(?:\.\d+)?)\)', rf'.{method}(&d("\1")).unwrap()', text)
    text = re.sub(r'\.power\((\d+)\.0\)', r'.power(\1).unwrap()', text)
    for old, new in [('hardScore', 'hard'), ('mediumScore', 'medium'), ('softScore', 'soft'),
                     ('subtract', 'sub'), ('negate', 'neg'), ('toShortString', 'to_short_string')]:
        text = text.replace(f'.{old}(', f'.{new}(')
    return text


def translate(body, method, java, native, n):
    if method == 'zero':
        zeros = ', '.join(['d("0")'] * n)
        one = ', '.join(['d("0")'] * (n - 1) + ['d("1")'])
        return [f'assert_eq!({native}::ZERO, {native}::new({zeros}));',
                f'assert!({native}::new({zeros}).is_zero());',
                f'assert!(!{native}::new({one}).is_zero());']
    body = re.sub(r'^\s*(?:SoftAssertions\.)?assertSoftly\(softly -> \{', '', body)
    body = re.sub(r'\}\);\s*$', '', body).replace('softly.assertThat(', 'assertThat(')
    out = []
    for statement in body.split(';'):
        statement = statement.strip()
        if not statement:
            continue
        if statement.startswith('assertThat('):
            left, end = group(statement, statement.index('('))
            suffix = statement[end:].strip()
            right, finish = group(suffix, suffix.index('('))
            if suffix[finish:].strip():
                raise ValueError(suffix)
            left, right = expression(left, java, native), expression(right, java, native)
            if suffix.startswith('.isEqualTo('):
                out.append(f'assert_eq!({left}, {right});')
            elif suffix.startswith('.hasToString('):
                out.append(f'assert_eq!(({left}).to_string(), {right});')
            else:
                raise ValueError(suffix)
        elif statement.startswith('assertThatIllegalArgumentException()'):
            literal = re.search(r'parseScore\(("[^"]*")\)', statement)
            if not literal:
                raise ValueError(statement)
            out.append(f'assert!({literal[1]}.parse::<{native}>().is_err());')
        else:
            call = statement[:statement.index('(')]
            inner, end = group(statement, statement.index('('))
            if statement[end:].strip():
                raise ValueError(statement)
            values = [expression(v, java, native) for v in args(inner)]
            if call in ['assertScoreFeasible', 'assertScoreNotFeasible']:
                negate = '!' if call == 'assertScoreNotFeasible' else ''
                out += [f'assert!({negate}({v}).is_feasible());' for v in values]
            else:
                helper = {'PlannerAssert.assertObjectsAreEqual': 'equal',
                          'PlannerAssert.assertObjectsAreNotEqual': 'distinct',
                          'PlannerAssert.assertCompareToOrder': 'ordered'}[call]
                out.append(helper + '(&[' + ',\n'.join(values) + ']);')
    return out


def main():
    subprocess.run(['python3', str(ROOT / 'scripts/upstream_inventory.py')], check=True)
    records = []
    for java, native, n in CLASSES:
        source = f'core/src/test/java/ai/timefold/solver/core/api/score/{java}Test.java'
        text = (ROOT / REF['path'] / source).read_text()
        methods = re.findall(r'@Test\s+void (\w+)\(\)\s*\{(.*?)\n    \}', text, re.S)
        assert len(methods) == text.count('@Test')
        local = f'crates/solver/tests/{snake(native)}.rs'
        header = f'''// Adapted and modified from Timefold Solver Community {REF['tag']}.
// Source: {source}
// Commit: {REF['commit']}
// License: Apache-2.0. See THIRD_PARTY_NOTICES.md and compat/licenses/.
// Changes: native Decimal literals/factors, u32 powers, constructors/operators,
// Rust equality/hash checks, ZERO, and Result handling. All assertions retained.
// Generated by scripts/adapt_decimal_score_tests.py from tests only.
#[allow(dead_code)]
mod common;
use common::{{distinct, equal, ordered}};
use std::ops::{{Add, Neg, Sub}};
use timetuck::{{Decimal, {native}}};
fn d(text: &str) -> Decimal {{ text.parse().unwrap() }}
'''
        for method, body in methods:
            lines = translate(body, method, java, native, n)
            header += f'\n#[test]\nfn {snake(method)}() {{\n' + '\n'.join(lines) + '\n}\n'
            records.append({'id': f'{java}Test.{method}', 'status': 'implemented',
                            'upstream_commit': REF['commit'], 'upstream_path': source,
                            'upstream_test': method, 'local_path': local, 'local_test': snake(method),
                            'license': 'Apache-2.0',
                            'notice_paths': ['THIRD_PARTY_NOTICES.md', 'compat/licenses/timefold-Apache-2.0.txt'],
                            'modifications': 'Preserve all assertions using native Decimal literals and exact Decimal multiply/divide factors; tested integral double powers become u32. Raw constructors use from_parts; factories normalize all-zero scores. Native operators, Result unwrap, ZERO, equality/hash consistency replace Java plumbing. No f64 input or JVM API parity claimed.'})
        (ROOT / local).write_text(header)
    path = ROOT / 'compat/provenance.json'
    ledger = json.loads(path.read_text())
    ids = {r['id'] for r in records}
    ledger['cases'] = [r for r in ledger['cases'] if r['id'] not in ids] + records
    path.write_text(json.dumps(ledger, indent=2) + '\n')
    subprocess.run(['cargo', 'fmt', '--all'], cwd=ROOT, check=True)
    print(f'Adapted {len(records)} decimal methods')


if __name__ == '__main__':
    main()
