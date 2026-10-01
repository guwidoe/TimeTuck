#!/usr/bin/env python3
"""Adapt three pinned Community score test classes, never implementation sources.

This intentionally restricted translator fails on unfamiliar assertions. The output
is reviewable Rust, and every source test method gets its own provenance record.
"""
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REF = json.loads((ROOT / 'compat/upstream.json').read_text())
CLASSES = ['SimpleScore', 'HardSoftScore', 'HardMediumSoftScore']


def snake(name):
    return re.sub(r'(?<!^)(?=[A-Z])', '_', name).lower()


def args(text):
    # Delimiters inside quoted strings must not affect depth or split points.
    result, start, depth, quoted = [], 0, 0, False
    for i, char in enumerate(text):
        if char == '"' and (i == 0 or text[i - 1] != '\\'):
            quoted = not quoted
        if quoted:
            continue
        if char == '(':
            depth += 1
        elif char == ')':
            depth -= 1
        elif char == ',' and depth == 0:
            result.append(text[start:i].strip())
            start = i + 1
    if text[start:].strip():
        result.append(text[start:].strip())
    return result


def group(text, start):
    depth, quoted = 0, False
    for i in range(start, len(text)):
        char = text[i]
        if char == '"' and (i == 0 or text[i - 1] != '\\'):
            quoted = not quoted
        if quoted:
            continue
        if char == '(':
            depth += 1
        elif char == ')':
            depth -= 1
            if depth == 0:
                return text[start + 1:i], i + 1
    raise ValueError(text)


def expression(text, cls):
    text = text.strip()
    if text.startswith('"'):
        return text
    text = re.sub(r'(-?\d+)L\b', r'\1', text)
    text = text.replace('Long.MIN_VALUE', 'i64::MIN').replace('Long.MAX_VALUE', 'i64::MAX')
    text = text.replace('Integer.MIN_VALUE', '(i32::MIN as i64)').replace('Integer.MAX_VALUE', '(i32::MAX as i64)')
    if text.startswith(f'new {cls}('):
        inner, end = group(text, text.index('('))
        values = args(inner)
        text = f'{cls}::new({", ".join(values[1:])}).with_structural({values[0]})' + text[end:]
    # Convert nested constructors and factory calls, then Rust method spelling.
    text = text.replace(f'{cls}.of(', f'{cls}::new(')
    for old, new in [('ofHard', 'of_hard'), ('ofSoft', 'of_soft'), ('ofMedium', 'of_medium')]:
        text = text.replace(f'{cls}.{old}(', f'{cls}::{new}(')
    for old, new in [('subtract', 'sub'), ('negate', 'neg'), ('toShortString', 'to_short_string')]:
        text = text.replace(f'.{old}(', f'.{new}(')
    return text


def translate(body, method, cls):
    if method == 'zero':
        values = ['0'] * (CLASSES.index(cls) + 1)
        one = values[:-1] + ['1']
        return f'    assert_eq!({cls}::ZERO, {cls}::new({", ".join(values)}));\n    assert!({cls}::ZERO.is_zero());\n    assert!(!{cls}::new({", ".join(one)}).is_zero());'
    lines = []
    for statement in body.split(';'):
        statement = statement.strip()
        if not statement:
            continue
        if statement.startswith('assertThat('):
            left, end = group(statement, statement.index('('))
            suffix = statement[end:].strip()
            right, _ = group(suffix, suffix.index('('))
            left, right = expression(left, cls), expression(right, cls)
            if suffix.startswith('.isEqualTo('):
                lines.append(f'assert_eq!({left}, {right});')
            elif suffix.startswith('.hasToString('):
                lines.append(f'assert_eq!(({left}).to_string(), {right});')
            else:
                raise ValueError(suffix)
        elif statement.startswith('assertThatIllegalArgumentException()'):
            match = re.search(r'parseScore\(("[^"]*")\)', statement)
            if not match:
                raise ValueError(statement)
            lines.append(f'assert!({match[1]}.parse::<{cls}>().is_err());')
        else:
            call = statement[:statement.index('(')]
            inner, _ = group(statement, statement.index('('))
            values = ',\n'.join(expression(x, cls) for x in args(inner))
            if call in ['assertScoreFeasible', 'assertScoreNotFeasible']:
                negate = '!' if call == 'assertScoreNotFeasible' else ''
                lines.extend(f'assert!({negate}({expression(value, cls)}).is_feasible());' for value in args(inner))
            else:
                helper = {'PlannerAssert.assertObjectsAreEqual': 'equal',
                          'PlannerAssert.assertObjectsAreNotEqual': 'distinct',
                          'PlannerAssert.assertCompareToOrder': 'ordered'}[call]
                lines.append(f'{helper}(&[{values}]);')
    output = '\n'.join('    ' + line for line in lines)
    output = re.sub(rf'{cls}\.parseScore\(("[^"]*")\)', rf'\1.parse::<{cls}>().unwrap()', output)
    return output


def main():
    import subprocess
    subprocess.run(['python3', str(ROOT / 'scripts/upstream_inventory.py')], check=True)
    cases = []
    for cls in CLASSES:
        source = f'core/src/test/java/ai/timefold/solver/core/api/score/{cls}Test.java'
        local = f'crates/solver/tests/{snake(cls)}.rs'
        text = (ROOT / REF['path'] / source).read_text()
        methods = re.findall(r'@Test\s+void (\w+)\(\)\s*\{(.*?)\n    \}', text, re.S)
        output = f'''// Adapted and modified from Timefold Solver Community {REF['tag']}.
// Source: {source}
// Commit: {REF['commit']}
// License: Apache-2.0. See THIRD_PARTY_NOTICES.md and compat/licenses/.
// Changes: native Rust constructors, operators, assertions, equality/hash checks.
// Generated by scripts/adapt_score_tests.py from test sources only.

#[allow(dead_code)]
mod common;
#[allow(unused_imports)]
use common::{{distinct, equal, ordered}};
use std::ops::{{Add, Neg, Sub}};
use timetuck::{cls};

'''
        for method, body in methods:
            test = snake(method)
            output += f'#[test]\nfn {test}() {{\n{translate(body, method, cls)}\n}}\n\n'
            cases.append({'id': f'{cls}Test.{method}', 'status': 'implemented',
                          'upstream_commit': REF['commit'], 'upstream_path': source,
                          'upstream_test': method, 'local_path': local,
                          'local_test': test, 'license': 'Apache-2.0',
                          'notice_paths': ['THIRD_PARTY_NOTICES.md', 'compat/licenses/timefold-Apache-2.0.txt'],
                          'modifications': 'Translated all assertions to native Rust score APIs. Java object equality/hash checks use Rust equality and Hash consistency; zero uses ZERO. No JVM API parity claimed.'})
        (ROOT / local).write_text(output)
    existing_ledger = json.loads((ROOT / 'compat/provenance.json').read_text())
    existing = existing_ledger['cases']
    generated_ids = {case['id'] for case in cases}
    cases += [case for case in existing if case['id'] not in generated_ids]
    (ROOT / 'compat/provenance.json').write_text(json.dumps({**existing_ledger, 'schema_version': 1, 'cases': cases}, indent=2) + '\n')
    subprocess.run(['cargo', 'fmt', '--all'], cwd=ROOT, check=True)
    print(f'Adapted {len(cases)} upstream test methods.')

if __name__ == '__main__':
    main()
