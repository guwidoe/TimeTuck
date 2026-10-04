"""Original list planning-value checks; no upstream method coverage credit."""
import base64
from copy import deepcopy
import json
import subprocess


def expected_values(case):
    # Read each child's input before applying it; later children see prior edits.
    from differential_lists import apply
    lists, touched = deepcopy(case['initial']), []
    for kind, *v in case['moves']:
        if kind == 'a':
            values = [v[0]]
        elif kind == 'c':
            values = [lists[v[0]][v[1]]]
        elif kind in ('r', 'u'):
            values = lists[v[0]][v[1]:v[2]]
        elif kind == 's':
            values = [lists[v[0]][v[1]], lists[v[2]][v[3]]]
        elif kind == 'x':
            values = lists[v[0]][v[1]:v[2]] + lists[v[3]][v[4]:v[5]]
        else:
            raise ValueError('Unknown move')
        for value in values:
            if value not in touched:
                touched.append(value)
        lists = apply(lists, [kind, *v])
    return touched


def parse(case, line, runtime):
    from differential_lists import expected
    fields = line.split('\t')
    if len(fields) < 2 or fields[0] != case['id']:
        raise ValueError('Missing or mismatched case identity')
    if fields[1] == 'unsupported_empty':
        if runtime != 'java' or case['moves'] or len(fields) != 2:
            raise ValueError('Invalid unsupported-empty response')
        return {'status': 'unsupported_empty'}
    if fields[1] == 'error':
        if runtime != 'java' or len(fields) != 3:
            raise ValueError('Invalid error response')
        detail = base64.b64decode(fields[2], validate=True).decode('utf-8')
        if not detail:
            raise ValueError('Empty error detail')
        return {'status': 'error', 'error': detail}
    if len(fields) != 7 or fields[1] != 'ok':
        raise ValueError('Expected seven success fields')
    if fields[2:5] != expected(case).split('\t')[1:]:
        raise ValueError('Candidate, undo or committed state differs')
    observations = []
    for field in fields[5:]:
        tokens = field.split(',') if field else []
        if any(not v.isascii() or not v.isdecimal() for v in tokens):
            raise ValueError('Invalid value identity')
        values = list(map(int, tokens))
        if any(v >= case['count'] for v in values):
            raise ValueError('Out-of-range value identity')
        observations.append(values)
    return {'status': 'ok', 'values': observations}


def assess(cases, outputs):
    summary = dict(total=len(cases), passed=0, native_reference_passed=0,
                   java_error_cases=0, unsupported_cases=0, divergent_cases=0,
                   duplicate_observations=0, order_differences=0, failures=[], differences=[])
    for index, case in enumerate(cases):
        wanted = expected_values(case)
        observed = {}
        for runtime in ('rust', 'java'):
            try:
                observed[runtime] = parse(case, outputs[runtime][index], runtime)
            except (ValueError, UnicodeError) as error:
                summary['failures'].append(dict(case=case['id'], runtime=runtime, error=str(error)))
        native = observed.get('rust', {})
        if native.get('status') == 'ok' and all(v == wanted for v in native['values']):
            summary['native_reference_passed'] += 1
        else:
            summary['failures'].append(dict(case=case['id'], runtime='rust',
                                            expected=wanted, actual=native))
        java = observed.get('java', {})
        status = java.get('status')
        if status == 'error':
            summary['java_error_cases'] += 1
            summary['differences'].append(dict(case=case['id'], **java))
        elif status == 'unsupported_empty':
            summary['unsupported_cases'] += 1
        elif status == 'ok':
            summary['duplicate_observations'] += sum(len(v) != len(set(v)) for v in java['values'])
            agrees = all(set(v) == set(wanted) for v in java['values'])
            if agrees:
                summary['order_differences'] += sum(v != wanted for v in java['values'])
            if agrees and native.get('values') == [wanted, wanted]:
                summary['passed'] += 1
            else:
                summary['divergent_cases'] += 1
                summary['differences'].append(dict(case=case['id'], expected=wanted, actual=java))
    return summary


def compare(cases, commands, work):
    from differential_lists import encode
    cases = [*cases,
             dict(id='values-swap-change', count=3, initial=[[0, 1], [2]],
                  moves=[['s', 0, 0, 1, 0], ['c', 0, 0, 1, 1]]),
             dict(id='values-swap-unassign', count=2, initial=[[0, 1]],
                  moves=[['s', 0, 0, 0, 1], ['u', 0, 0, 1]])]
    payload = '\n'.join(map(encode, cases)) + '\n'
    outputs = {}
    for runtime, command in commands.items():
        run = subprocess.run([*command, '--planning-values'], input=payload, text=True,
                             capture_output=True, timeout=120)
        (work / f'list-values-{runtime}.stderr.log').write_text(run.stderr)
        if run.returncode:
            raise RuntimeError(f'{runtime} value adapter failed; inspect stderr log')
        outputs[runtime] = run.stdout.splitlines()
        if len(outputs[runtime]) != len(cases):
            raise RuntimeError(f'{runtime} value response count mismatch')
    summary = assess(cases, outputs)
    summary['scope'] = (
        'Original corpus, observed twice per case: temporary execution and execution after undo. '
        'Java public getPlanningValues is called after execution; native snapshot capture before '
        'each execution matches independently replayed first-touch identities. Java membership is '
        'compared as a set; raw order and multiplicity remain recorded. Empty Java compositions '
        'are unsupported, not passing. No upstream method, search-trace or timing credit.')
    (work / 'list-values-cases.json').write_text(json.dumps(
        dict(schema_version=1, cases=cases, outputs=outputs, summary=summary), indent=2) + '\n')
    return summary
