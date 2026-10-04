"""Original public Community cheapest-insertion and native pooled checks."""
import json
import subprocess
from differential_scores import ROOT, WORK
from differential_solver import generate, encode, score, optimum, validate


def generate_cases():
    return generate() + [
        dict(id='pool-global-order', value_count=2, initial=[None, None], pins=[False, False], edges=[(0, 1)], weights=[[5, 0], [10, 1]]),
        dict(id='pool-ties', value_count=2, initial=[None, None, None], pins=[False]*3, edges=[], weights=[[0, 0]]*3),
    ]


def reference(case):
    """Rebuild every candidate assignment; no native selection or scoring code."""
    if 'domains' in case or 'optional' in case:
        raise ValueError('comparison requires homogeneous required domains')
    values = [value if pin else None for value, pin in zip(case['initial'], case['pins'])]
    while any(v is None for v in values):
        candidates = []
        for i, current in enumerate(values):
            if current is not None:
                continue
            if case['pins'][i]:
                raise ValueError('required pinned value is missing')
            for v in range(case['value_count']):
                candidate = values.copy()
                candidate[i] = v
                candidates.append(candidate)
        if not candidates:
            raise ValueError('empty required domain')
        # max retains the first encountered tie in Python.
        values = max(candidates, key=lambda v: score(case, v))
    return values, score(case, values)


def observed(case, line):
    fields = line.split('\t')
    if len(fields) != 4 or fields[0] != case['id']:
        raise ValueError('response identity/field mismatch')
    try:
        values = [None if v == '-' else int(v) for v in fields[3].split(',')]
        claimed = tuple(map(int, fields[1:3]))
        # Validate shape/ranges/pins before calling the objective ourselves.
        error = validate(case, line, claimed)
        if error:
            raise ValueError(error)
        return values, claimed
    except (ValueError, IndexError) as e:
        raise ValueError(f'invalid construction response: {e}') from e


def run_cases(classpath, mode, cases, reference, scope):
    subprocess.run(['cargo', 'build', '--release', '--example', 'solver_oracle', '--locked'], cwd=ROOT, check=True)
    payload = '\n'.join(map(encode, cases))+'\n'
    outputs = {}
    for runtime, command in [('java', ['java', '-Xms256m', '-Xmx256m', '-cp', classpath, 'SolverOracle']),
                             ('rust', [str(ROOT/'target/release/examples/solver_oracle')])]:
        result = subprocess.run([*command, mode], input=payload, text=True, capture_output=True, timeout=120)
        (WORK/f'{mode}-{runtime}.stderr.log').write_text(result.stderr)
        if result.returncode:
            raise RuntimeError(f'{runtime} {mode} adapter failed; see work/oracle/{mode}-{runtime}.stderr.log')
        outputs[runtime] = result.stdout.splitlines()
        if len(outputs[runtime]) != len(cases):
            raise ValueError(f'{runtime} {mode} response count mismatch')
    failures, differences = [], []
    native_passed = equal_scores = equal_assignments = 0
    optimal = dict(java=0, rust=0)
    for i, case in enumerate(cases):
        expected = reference(case)
        best = optimum(case)
        results = {}
        for runtime, lines in outputs.items():
            try:
                results[runtime] = observed(case, lines[i])
                optimal[runtime] += results[runtime][1] == best
                if runtime == 'rust':
                    if results[runtime] != expected:
                        raise ValueError(f'native differs from greedy reference: {results[runtime]} != {expected}')
                    native_passed += 1
            except ValueError as e:
                failures.append(dict(case=case['id'], runtime=runtime, error=str(e)))
        if len(results) == 2 and not any(f['case'] == case['id'] for f in failures):
            equal_scores += results['rust'][1] == results['java'][1]
            equal_assignments += results['rust'][0] == results['java'][0]
            if results['rust'] != results['java']:
                differences.append(dict(case=case['id'], rust=results['rust'], java=results['java']))
    report = dict(seed=20261001, total=len(cases), passed=equal_scores,
                  native_reference_passed=native_passed, equal_assignments=equal_assignments,
                  optimal=optimal, differences=differences, failures=failures,
                  scope=scope)
    (WORK/f'{mode}-construction-cases.json').write_text(json.dumps(dict(cases=cases, outputs=outputs, report=report), indent=2)+'\n')
    print(f'{mode} construction: {equal_scores}/{len(cases)} equal scores; {native_passed} native references; {len(failures)} failures', flush=True)
    return report


def run_checks(classpath):
    return run_cases(classpath, 'pooled', generate_cases(), reference, '66 original homogeneous required graph cases with pins and signed weights. Public Java CHEAPEST_INSERTION, original entity/value order and NEVER early pick; native pooled construction. Every returned score/assignment independently validated, native assignment also checked against full candidate reconstruction. Final equal scores and exact optima are separate counts. No Java trace/list/mixed/timing or upstream-method credit.')


if __name__ == '__main__':
    from prepare_solver_oracle import prepare_solver_java
    cp, _ = prepare_solver_java()
    raise SystemExit(bool(run_checks(cp)['failures']))
