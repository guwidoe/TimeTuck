#!/usr/bin/env python3
"""Original public-Java and independent-Python Great Deluge search comparisons."""
import copy
import json
import math
import random
import subprocess
from differential_scores import ROOT, WORK
from differential_solver import encode as encode_problem, score
from termination_checks import check_java, check_rust

SEED = 20261220
RAINS = [('fixed',0,1),('fixed',1,0),('fixed',1,3),('ratio',1,10),('ratio',1,2),('ratio',1,100)]


def reference(case, *, floating_ratio=False):
    """Independently enumerate neighbors and form exact thresholds from elapsed steps."""
    values = case['initial'][:]
    current = best = initial = score(case, values)
    threshold = initial
    accumulated_ratio = 0.0
    best_values = values[:]
    steps = best_step = accepted = 0
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
                eligible = candidate > current or candidate >= threshold
                trials.append((candidate, trial, eligible))
                if eligible and case['selection'] == 'first':
                    break
            if trials and trials[-1][2] and case['selection'] == 'first':
                break
        if not trials:
            break
        eligible = [v for v in trials if v[2]]
        chosen, values, is_accepted = max(eligible or trials, key=lambda v: v[0])
        steps += 1
        if case['rain'] == 'fixed':
            threshold = tuple(v + steps * inc for v, inc in zip(initial, case['parameters']))
        else:
            num, den = case['parameters']
            if floating_ratio:
                # A hypothesis for public-output boundary probes, not a claim
                # about Java internals. The native reference always uses integers.
                accumulated_ratio += num / den
                threshold = tuple(math.floor(v + abs(v) * accumulated_ratio) for v in initial)
            else:
                threshold = tuple(v + abs(v) * num * steps // den for v in initial)
        accepted += is_accepted
        if chosen > best:
            best, best_values, best_step = chosen, values[:], steps
        current = chosen
        trace.append({'values': values[:], 'score': list(current), 'threshold': list(threshold),
                      'trials': len(trials), 'eligible': len(eligible), 'accepted': is_accepted})
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
    bases += [
        {'id': 'strict-aspiration', 'value_count': 3, 'initial': [0], 'pins': [False], 'weights': [[0,0,1]], 'edges': [], 'steps': 2},
        {'id': 'fractional-rain', 'value_count': 4, 'initial': [0], 'pins': [False], 'weights': [[-8,-9,-7,-6]], 'edges': [], 'steps': 12},
        {'id': 'positive-water', 'value_count': 4, 'initial': [0], 'pins': [False], 'weights': [[8,8,9,7]], 'edges': [], 'steps': 12},
    ]
    cases = []
    for base in bases:
        for rain, a, b in RAINS:
            for selection in ['first','best']:
                c = copy.deepcopy(base)
                c.update(id=f'{base["id"]}-{rain}-{a}-{b}-{selection}', rain=rain, parameters=[a,b], selection=selection)
                cases.append(c)
    return cases


def encode(c):
    return encode_problem(c) + '\t' + '\t'.join([c['rain'],*map(str,c['parameters']),str(c['steps']),c['selection']])


def known_java_boundary(case):
    # Frozen observed public-API output, kept outside agreement passes. Any
    # change fails verification rather than silently accepting arbitrary drift.
    if case['id'] == 'random-9-ratio-1-10-first':
        return {'score':[0,1], 'values':[2,1,0]}
    return None


def run_checks(classpath):
    subprocess.run(['cargo','build','--release','--example','great_deluge_oracle','--locked'],cwd=ROOT,check=True)
    matrix = generate()
    boundary = next(c for c in matrix if known_java_boundary(c))
    probes = [{**copy.deepcopy(boundary), 'id':f'rounding-prefix-{n}', 'steps':n} for n in range(1,26)]
    cases = matrix + probes
    payload = '\n'.join(map(encode,cases))+'\n'
    outputs = {}
    for name, command in [('java',['java','-Xms256m','-Xmx256m','-cp',classpath,'GreatDelugeOracle']),
                          ('rust',[str(ROOT/'target/release/examples/great_deluge_oracle')])]:
        run = subprocess.run(command,input=payload,text=True,capture_output=True,timeout=180)
        (WORK/f'great-deluge-{name}.stderr.log').write_text(run.stderr)
        if run.returncode:
            raise RuntimeError(f'{name} Great Deluge adapter failed; see work/oracle/great-deluge-{name}.stderr.log')
        outputs[name] = run.stdout.splitlines()
        if len(outputs[name]) != len(cases)*(2 if name=='rust' else 1):
            raise ValueError(f'{name} Great Deluge response count')
    failures, references, divergences, probe_results = [], [], [], []
    for i, c in enumerate(cases):
        expected = reference(c)
        references.append(expected)
        for runtime in ['java','scoped','stream']:
            try:
                if runtime == 'java':
                    baseline = known_java_boundary(c)
                    if i >= len(matrix):
                        baseline = ({'score':[0,1], 'values':[2,1,0]} if c['steps'] in [11,12] else expected)
                    check_java(c,outputs['java'][i],baseline or expected)
                    if baseline and any(baseline[k] != expected[k] for k in ['score','values']):
                        divergences.append({'case':c['id'], 'java':baseline, 'native':{k:expected[k] for k in ['score','values']}})
                    if i >= len(matrix):
                        check_java(c,outputs['java'][i],reference(c, floating_ratio=True))
                        probe_results.append({'case':c['id'],'steps':c['steps'],'exact':{k:expected[k] for k in ['score','values']},'java':baseline})
                else:
                    check_rust(c,outputs['rust'][2*i+(runtime=='stream')],expected,runtime)
            except (ValueError, IndexError) as error:
                failures.append({'case':c['id'],'runtime':runtime,'error':str(error)})
    (WORK/'great-deluge-cases.json').write_text(json.dumps({'schema_version':1,'seed':SEED,'cases':cases,'references':references,'outputs':outputs},indent=2)+'\n')
    failed = {f['case'] for f in failures}
    divergent = {d['case'] for d in divergences}
    report = {'seed':SEED,'total':len(matrix),'passed':sum(c['id'] not in failed|divergent for c in matrix),'failures':failures,
              'native_reference_passed':sum(not any(f['case']==c['id'] and f['runtime']!='java' for f in failures) for c in matrix),
              'diverged':sum(c['id'] in divergent for c in matrix), 'divergences':divergences,
              'boundary_probes':{'total':len(probes),'agreed':sum(c['id'] not in failed|divergent for c in probes),
                  'diverged':sum(c['id'] in divergent for c in probes),'results':probe_results,
                  'scope':'Frozen step-prefix outputs check the observed ratio boundary: exact Rust reaches score (0,9) at step 11; Java reaches it at step 13. A separate Python binary-floating cumulative-ratio hypothesis reproduces these final outputs; no Java internal threshold/trace claim.'},
              'rains': RAINS, 'native_reference_steps':sum(r['steps'] for r in references),
               'scope':'Original required-variable graph cases; fixed rain (0,1)/(1,0)/(1,3) and ratios 1/10, 1/2, 1/100, original-order first/best accepted selection, first ties and best-rejected fallback. Java final assignments and scores are compared to independent Python enumeration; one observed ratio boundary differs and is recorded separately, never counted as an agreement pass. Rust scoped/stream counters and reasons also match Python. Native arithmetic uses exact rational accumulation; no claim for general Java floating drift/overflow, Java internal traces, decimal/list/mixed differential behavior or new upstream method credit.' }
    print(f'Great Deluge: {report["passed"]}/{report["total"]} cases; {report["native_reference_steps"]} native reference steps',flush=True)
    for failure in failures[:8]: print(failure,flush=True)
    return report


if __name__ == '__main__':
    from prepare_solver_oracle import prepare_solver_java
    cp, _ = prepare_solver_java()
    raise SystemExit(bool(run_checks(cp)['failures']))
