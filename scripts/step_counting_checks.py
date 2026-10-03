#!/usr/bin/env python3
"""Original public-Java and independent-Python step-counting search comparisons."""
import copy
import json
import random
import subprocess
from differential_scores import ROOT, WORK
from differential_solver import encode as encode_problem, score
from termination_checks import check_java, check_rust

SEED = 20261218
MODES = ['STEP', 'EQUAL_OR_IMPROVING_STEP', 'IMPROVING_STEP', 'SELECTED_MOVE', 'ACCEPTED_MOVE']


def reference(case):
    """Batch scored neighbors, count qualifying events, replace a boundary threshold."""
    values = case['initial'][:]
    current = best = threshold = score(case, values)
    best_values = values[:]
    steps = best_step = accepted = count = 0
    trace = []
    while steps < case['steps']:
        trials = []
        for i, old in enumerate(values):
            if case['pins'][i]:
                continue
            for new in range(case['value_count']):
                if new == old:
                    continue
                trial = values[:]
                trial[i] = new
                candidate = score(case, trial)
                eligible = candidate >= current or candidate >= threshold
                trials.append((candidate, trial, eligible))
                if eligible and case['selection'] == 'first':
                    break
            if trials and trials[-1][2] and case['selection'] == 'first':
                break
        if not trials:
            break
        eligible = [v for v in trials if v[2]]
        chosen, values, is_accepted = max(eligible or trials, key=lambda v: v[0])
        increment = {
            'STEP': 1,
            'EQUAL_OR_IMPROVING_STEP': int(chosen >= current),
            'IMPROVING_STEP': int(chosen > current),
            'SELECTED_MOVE': len(trials),
            'ACCEPTED_MOVE': len(eligible),
        }[case['mode']]
        count += increment
        if count >= case['period']:
            threshold, count = chosen, 0
        steps += 1
        accepted += is_accepted
        if chosen > best:
            best, best_values, best_step = chosen, values[:], steps
        current = chosen
        trace.append({'values': values[:], 'score': list(current), 'threshold': list(threshold),
                      'count': count, 'trials': len(trials), 'eligible': len(eligible), 'accepted': is_accepted})
    return {'score': list(best), 'values': best_values, 'steps': steps,
            'unimproved': steps-best_step, 'accepted': accepted,
            'reason': 'StepLimit' if steps == case['steps'] else 'NoAcceptedMove', 'trace': trace}


def generate():
    rng = random.Random(SEED)
    bases = []
    for i in range(16):
        n, k = rng.randint(2,5), rng.randint(2,4)
        bases.append({'id': f'random-{i}', 'value_count': k,
                      'initial': [rng.randrange(k) for _ in range(n)],
                      'pins': [rng.random()<.2 for _ in range(n)],
                      'weights': [[rng.randint(-8,8) for _ in range(k)] for _ in range(n)],
                      'edges': [(a,b) for a in range(n) for b in range(a+1,n) if rng.random()<.5],
                      'steps': 12})
    bases += [
        {'id': 'plateau', 'value_count': 3, 'initial': [0], 'pins': [False], 'weights': [[0,0,0]], 'edges': [], 'steps': 7},
        {'id': 'fallback', 'value_count': 3, 'initial': [0], 'pins': [False], 'weights': [[0,-3,-8]], 'edges': [], 'steps': 7},
        {'id': 'pinned', 'value_count': 2, 'initial': [0], 'pins': [True], 'weights': [[0,1]], 'edges': [], 'steps': 3},
        {'id': 'zero-steps', 'value_count': 2, 'initial': [0], 'pins': [False], 'weights': [[0,1]], 'edges': [], 'steps': 0},
    ]
    cases = []
    for base in bases:
        for mode in MODES:
            for period in [1,2,7]:
                for selection in ['first','best']:
                    c = copy.deepcopy(base)
                    c.update(id=f'{base["id"]}-{mode}-{period}-{selection}', mode=mode, period=period, selection=selection)
                    cases.append(c)
    return cases


def encode(c):
    return encode_problem(c) + '\t' + '\t'.join([c['mode'],str(c['period']),str(c['steps']),c['selection']])


def run_checks(classpath):
    subprocess.run(['cargo','build','--release','--example','step_counting_oracle','--locked'],cwd=ROOT,check=True)
    cases = generate()
    payload = '\n'.join(map(encode,cases))+'\n'
    outputs = {}
    for name, command in [('java',['java','-Xms256m','-Xmx256m','-cp',classpath,'StepCountingOracle']),
                          ('rust',[str(ROOT/'target/release/examples/step_counting_oracle')])]:
        run = subprocess.run(command,input=payload,text=True,capture_output=True,timeout=180)
        (WORK/f'step-counting-{name}.stderr.log').write_text(run.stderr)
        if run.returncode:
            raise RuntimeError(f'{name} step-counting adapter failed; see work/oracle/step-counting-{name}.stderr.log')
        outputs[name] = run.stdout.splitlines()
        if len(outputs[name]) != len(cases)*(2 if name=='rust' else 1):
            raise ValueError(f'{name} step-counting response count')
    failures, references = [], []
    for i, c in enumerate(cases):
        expected = reference(c)
        references.append(expected)
        for runtime in ['java','scoped','stream']:
            try:
                if runtime == 'java':
                    check_java(c,outputs['java'][i],expected)
                else:
                    check_rust(c,outputs['rust'][2*i+(runtime=='stream')],expected,runtime)
            except (ValueError, IndexError) as error:
                failures.append({'case':c['id'],'runtime':runtime,'error':str(error)})
    (WORK/'step-counting-cases.json').write_text(json.dumps({'schema_version':1,'seed':SEED,'cases':cases,'references':references,'outputs':outputs},indent=2)+'\n')
    failed = {f['case'] for f in failures}
    report = {'seed':SEED,'total':len(cases),'passed':len(cases)-len(failed),'failures':failures,
              'modes': MODES, 'native_reference_steps':sum(r['steps'] for r in references),
              'scope':'Original required-variable graph cases; five step-counting modes, periods 1/2/7, original-order first/best accepted selection, first ties and best-rejected fallback. Java final assignments and scores are compared to independent Python neighbor enumeration. Rust scoped/stream counters and termination reasons also match Python. No Java internal counters/traces, randomized order, list/mixed differential comparison or new upstream method credit.'}
    print(f'Step counting: {report["passed"]}/{report["total"]} cases; {report["native_reference_steps"]} native reference steps',flush=True)
    for failure in failures[:8]: print(failure,flush=True)
    return report


if __name__ == '__main__':
    from prepare_solver_oracle import prepare_solver_java
    cp, _ = prepare_solver_java()
    raise SystemExit(bool(run_checks(cp)['failures']))
