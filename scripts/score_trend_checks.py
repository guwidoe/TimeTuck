"""Original mixed-sign exact-search comparisons using public Java trend config."""
import json
import subprocess
from differential_scores import ROOT, WORK
from differential_solver import generate, encode, optimum, validate

ORDERS = ('depth', 'breadth', 'score', 'bound')
TRENDS = ('ANY', 'ONLY_DOWN/ANY')


def run_checks(classpath):
    subprocess.run(['cargo', 'build', '--release', '--example', 'solver_oracle', '--locked'], cwd=ROOT, check=True)
    cases = generate()
    expected = [optimum(c) for c in cases]
    payload = '\n'.join(map(encode, cases))+'\n'
    outputs, failures, groups = {}, [], []
    for order in ORDERS:
        for trend in TRENDS:
            key = order + ':' + trend
            outputs[key] = {}
            for runtime, base in [('java', ['java', '-Xms256m', '-Xmx256m', '-cp', classpath, 'SolverOracle']),
                                  ('rust', [str(ROOT/'target/release/examples/solver_oracle')])]:
                result = subprocess.run([*base, order, trend], input=payload, text=True, capture_output=True, timeout=120)
                (WORK/f'trend-{order}-{trend.replace("/", "-")}-{runtime}.stderr.log').write_text(result.stderr)
                if result.returncode:
                    raise RuntimeError(f'{runtime} {key} trend adapter failed; see work/oracle/trend-*.stderr.log')
                lines = result.stdout.splitlines()
                if len(lines) != len(cases):
                    raise ValueError(f'{runtime} {key} response count mismatch')
                outputs[key][runtime] = lines
            bad = set()
            for i, case in enumerate(cases):
                for runtime, lines in outputs[key].items():
                    error = validate(case, lines[i], expected[i])
                    if error:
                        bad.add(case['id'])
                        failures.append(dict(order=order, trend=trend, case=case['id'], runtime=runtime, error=error))
            groups.append(dict(order=order, trend=trend, total=len(cases), passed=len(cases)-len(bad)))
    report = dict(seed=20261001, total=sum(g['total'] for g in groups), passed=sum(g['passed'] for g in groups),
                  groups=groups, failures=failures,
                  scope='64 original homogeneous required graph cases with pins and signed soft preferences, four branch orders, ANY and ONLY_DOWN/ANY trends. Public Community configuration and native ordered bounded phases both checked against independent Python complete optima, assignment validity and score recomputation. No trace, bound-representation, timing or upstream-method credit.')
    (WORK/'score-trend-cases.json').write_text(json.dumps(dict(cases=cases, outputs=outputs, report=report), indent=2)+'\n')
    print(f'Score trends: {report["passed"]}/{report["total"]} exact optima agree', flush=True)
    return report


if __name__ == '__main__':
    from prepare_solver_oracle import prepare_solver_java
    cp, _ = prepare_solver_java()
    raise SystemExit(bool(run_checks(cp)['failures']))
