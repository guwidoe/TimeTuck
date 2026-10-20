"""Original bounded public-metric comparison; no upstream method credit or timing."""
import json
import random
import subprocess
from differential_scores import ROOT, WORK

SEED = 20261219
MODES = ['highest', 'last', 'best']


def generate():
    rng = random.Random(SEED)
    matrices = []
    for i in range(400):
        scores = [tuple(rng.randrange(-8, 9) for _ in range(3)) for _ in range(rng.randrange(2, 12))]
        if i % 3 == 0:
            scores[-1] = scores[0]
        matrices.append((f'random-{i}', scores))
    # Both permutations preserve the exact-reference boundary rather than filtering it out.
    matrices += [('equal-first', [(6, 0, 0), (6, 0, 0), (2, 8, -2)]),
                 ('equal-last', [(6, 0, 0), (2, 8, -2), (6, 0, 0)])]
    return [dict(id=f'{name}-{mode}-{steps}', mode=mode, steps=steps, scores=scores)
            for name, scores in matrices for mode in MODES for steps in range(1, 5)]


def encode(c, encounter_order=False):
    mode = c['mode'] + ('-encounter' if encounter_order and c['mode'] != 'highest' else '')
    return '\t'.join([c['id'], mode, str(c['steps']),
                      ';'.join(','.join(map(str, s)) for s in c['scores'])])


def reference(c, encounter_order=False):
    scores = list(map(tuple, c['scores']))
    current = best = 0
    trace = []
    for _ in range(c['steps']):
        baseline = scores[best if c['mode'] == 'best' else current]
        eligible = [i for i in range(len(scores)) if i != current]
        if c['mode'] != 'highest':
            improving = [i for i in eligible if scores[i] > baseline]
            if improving:
                eligible = improving
            else:
                if encounter_order:
                    # Independent whole-segment reference: exact-reference rows
                    # and ordinary-level improvers alternate encounter segments.
                    # Only the last segment survives. Rows improving no level and
                    # unequal to the reference cannot start or end a segment.
                    segment, previous_kind = [], None
                    for i in eligible:
                        kind = (0 if scores[i] == baseline else
                                1 if any(a > b for a, b in zip(scores[i], baseline)) else None)
                        if kind is None:
                            continue
                        if kind != previous_kind:
                            segment = []
                        segment.append(i)
                        previous_kind = kind
                    if segment:
                        eligible = segment
                for level in range(3):
                    improving = [i for i in eligible if scores[i][level] > baseline[level]]
                    if improving:
                        eligible = improving
                        break
        current = max(eligible, key=scores.__getitem__)
        if scores[current] > scores[best]:
            best = current
        trace.append(current)
    return dict(step=list(scores[current]), best=list(scores[best]), best_id=best, trace=trace,
                selected=len(scores)-1, accepted=len(scores)-1)


def decode(c, line):
    f = line.split('\t')
    if len(f) != 10 or f[0] != c['id']:
        raise ValueError('response identity/shape')
    v = list(map(int, f[1:]))
    r = dict(step=v[:3], best=v[3:6], best_id=v[6], selected=v[7], accepted=v[8])
    if not 0 <= r['best_id'] < len(c['scores']) or r['best'] != list(c['scores'][r['best_id']]):
        raise ValueError('retained identity/score')
    if tuple(r['best']) < tuple(c['scores'][0]) or tuple(r['best']) < tuple(r['step']):
        raise ValueError('invalid retained best')
    if tuple(r['step']) not in list(map(tuple, c['scores'])):
        raise ValueError('step score outside domain')
    if r['selected'] != len(c['scores'])-1 or r['accepted'] != r['selected']:
        raise ValueError('incomplete or rejected candidate set')
    return r


def validate_native(c, native, encounter_order=False):
    expected = reference(c, encounter_order)
    actual = decode(c, native)
    if any(actual[k] != expected[k] for k in actual):
        raise ValueError('native reference mismatch')
    return actual


def compare(c, native, java):
    actual = validate_native(c, native)
    baseline = decode(c, java)
    return None if actual == baseline else dict(case=c['id'], native=actual, java=baseline)


def run_checks(classpath):
    subprocess.run(['cargo', 'build', '--release', '--example', 'oscillation_oracle', '--locked'], cwd=ROOT, check=True)
    cases = generate()
    payload = '\n'.join(map(encode, cases))+'\n'
    outputs = {}
    for name, command in [('java', ['java', '-Xms256m', '-Xmx256m', '-cp', classpath, 'OscillationOracle']),
                          ('rust', [str(ROOT/'target/release/examples/oscillation_oracle')]),
                          ('rust-encounter', [str(ROOT/'target/release/examples/oscillation_oracle')])]:
        run_payload = ('\n'.join(encode(c, True) for c in cases)+'\n'
                       if name == 'rust-encounter' else payload)
        run = subprocess.run(command, input=run_payload, text=True, capture_output=True, timeout=180)
        (WORK/f'oscillation-{name}.stderr.log').write_text(run.stderr)
        if run.returncode:
            raise RuntimeError(f'{name} oscillation adapter failed; see work/oracle/oscillation-{name}.stderr.log')
        outputs[name] = run.stdout.splitlines()
        if len(outputs[name]) != len(cases):
            raise ValueError(f'{name} oscillation response count')
    # A completed solver must stay strongly reachable through its weak metric
    # observations. Force collection between solve and observation on 24 cases.
    gc_cases = cases[:24]
    gc_run = subprocess.run(['java', '-Xms64m', '-Xmx64m', '-cp', classpath,
                             'OscillationOracle', '--gc-check'],
                            input='\n'.join(map(encode,gc_cases))+'\n', text=True,
                            capture_output=True, timeout=60)
    (WORK/'oscillation-java-gc.stderr.log').write_text(gc_run.stderr)
    if gc_run.returncode:
        raise RuntimeError('Java metric GC regression failed; see work/oracle/oscillation-java-gc.stderr.log')
    gc_outputs = gc_run.stdout.splitlines()
    if len(gc_outputs) != len(gc_cases) or gc_outputs != outputs['java'][:len(gc_cases)]:
        raise ValueError('Java metric observations changed or disappeared under GC')
    gc_checks = dict(total=len(gc_cases), passed=len(gc_outputs))
    failures, differences = [], []
    native_passed = 0
    for i, c in enumerate(cases):
        try:
            validate_native(c, outputs['rust'][i])
            native_passed += 1
            difference = compare(c, outputs['rust'][i], outputs['java'][i])
            if difference:
                differences.append(difference)
        except ValueError as error:
            failures.append(dict(case=c['id'], error=str(error)))
    failed = {r['case'] for r in failures}
    divergent = {r['case'] for r in differences}
    report = dict(seed=SEED, metric_gc_checks=gc_checks, total=len(cases), passed=len(cases)-len(failed | divergent),
                  native_reference_passed=native_passed, diverged=len(divergent),
                  failures=failures, divergences=differences,
                  scope='Original bounded three-level score tables, 402 matrices x3 ranking modes x4 step prefixes. Public Java STEP_SCORE and move-count gauges observe the last step and complete accepted candidate set; returned best identity/score is checked separately. Native forager matches independent Python. Exact-reference encounter-order differences remain outside agreement counts. Large annealing temperatures make all bounded Java candidates accepted, verified at every prefix; no general annealing, PRNG, Java internal trace, timing or upstream-method credit.')
    policy_failures, policy_differences = [], []
    for i, c in enumerate(cases):
        try:
            native = validate_native(c, outputs['rust-encounter'][i], True)
            java = decode(c, outputs['java'][i])
            if native != java:
                policy_differences.append(dict(case=c['id'], native=native, java=java))
        except ValueError as error:
            policy_failures.append(dict(case=c['id'], error=str(error)))
    total_order_report = report
    report = dict(
        seed=SEED, metric_gc_checks=gc_checks,
        total=len(cases), passed=len(cases)-len(policy_failures)-len(policy_differences),
        native_reference_passed=len(cases)-len(policy_failures),
        diverged=len(policy_differences),
        failures=policy_failures + [dict(case=d['case'], error='encounter policy differs from Java')
                                    for d in policy_differences],
        divergences=policy_differences,
        total_order_controls=total_order_report,
        selected_native_modes=['HighestScore', 'StrategicOscillationLastStepEncounterOrder',
                               'StrategicOscillationBestEncounterOrder'],
        scope='Explicit encounter-order variants are selected for the strategic comparisons; '
              'the native defaults and existing strategic modes are unchanged. Identical original '
              '4824 fixtures, Java public API configuration, four prefixes, candidate counts '
              'and exact output checks. Independent Python whole-segment ranking validates '
              'native outputs. Complete original total-order comparison and its independent '
              'reference validation remain separately under total_order_controls. No Java '
              'internal trace, PRNG, timing, general podium parity or source-method credit.')
    (WORK/'oscillation-cases.json').write_text(json.dumps(dict(schema_version=1, seed=SEED, cases=cases, outputs=outputs, metric_gc_outputs=gc_run.stdout.splitlines()), indent=2)+'\n')
    if failures or policy_failures:
        raise RuntimeError('native reference or protocol validation failed; see oscillation-cases.json')
    print(f'Oscillation encounter policy: {report["passed"]}/{report["total"]} '
          f'agree, {report["diverged"]} differ, {len(report["failures"])} failures', flush=True)
    print(f'Oscillation original total order: {total_order_report["passed"]}/{len(cases)} '
          f'agree, {total_order_report["diverged"]} differ', flush=True)
    return report


if __name__ == '__main__':
    from prepare_solver_oracle import prepare_solver_java
    cp, _ = prepare_solver_java()
    raise SystemExit(bool(run_checks(cp)['failures']))
