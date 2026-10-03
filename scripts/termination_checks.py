#!/usr/bin/env python3
"""Original deterministic search termination comparisons; no upstream method credit."""
import copy
import json
import random
import subprocess
from differential_scores import ROOT, WORK
from differential_solver import encode as encode_problem, score, validate

SEED = 20261119


def reference(case):
    """Enumerate every one-variable neighbor; keep the earliest best solution."""
    values = case['initial'][:]
    best_values = values[:]
    best = score(case, values)
    current = best
    steps = best_step = accepted = 0
    trace = []
    while True:
        if steps >= case['steps']:
            reason = 'StepLimit'
        elif case['target'] is not None and best >= tuple(case['target']):
            reason = 'BestScoreLimit'
        elif case['feasible'] and best[0] >= 0:
            reason = 'FeasibleScore'
        elif case['unimproved'] is not None and steps - best_step >= case['unimproved']:
            reason = 'UnimprovedStepLimit'
        else:
            candidates = []
            for i, old in enumerate(values):
                if case['pins'][i]:
                    continue
                for new in range(case['value_count']):
                    if new == old:
                        continue
                    trial = values[:]
                    trial[i] = new
                    candidates.append((score(case, trial), trial))
            if not candidates:
                reason = 'NoAcceptedMove'
            else:
                # Hill climbing accepts >= current; if none qualifies, the
                # configured best-rejected fallback still selects a move.
                eligible = [v for v in candidates if v[0] >= current]
                selected_score, values = max(eligible or candidates, key=lambda v: v[0])
                accepted += bool(eligible)
                steps += 1
                if selected_score > best:
                    best, best_values, best_step = selected_score, values[:], steps
                current = selected_score
                trace.append({'values': values[:], 'score': list(current),
                              'best': list(best), 'accepted': bool(eligible)})
                continue
        return {'score': list(best), 'values': best_values, 'steps': steps,
                'unimproved': steps - best_step, 'accepted': accepted,
                'reason': reason, 'trace': trace}


def generate():
    def case(name, weights, edges=(), pins=None, initial=None, **limits):
        n = len(weights)
        return {'id':name, 'value_count':len(weights[0]), 'weights':weights,
                'edges':list(edges), 'initial': [0]*n if initial is None else initial,
                'pins':[False]*n if pins is None else pins,
                'steps':12, 'unimproved':None, 'target':None, 'feasible':False, **limits}
    cases = [
        case('plateau', [[0,0,0]], unimproved=3),
        case('improvement-reset-and-recovery', [[0,2,3],[0,2,3]], unimproved=2),
        case('target-after-two-improvements', [[0,2,3],[0,2,3]], target=[0,6]),
        case('initial-target', [[-10,-20]], target=[0,-10]),
        case('feasible-negative-soft', [[-5,-7],[-5,-7]], [(0,1)], feasible=True),
        case('unreachable-feasibility', [[0,1]], [(0,0)], feasible=True, steps=4),
        case('unreachable-target', [[0,1]], target=[1,0], steps=4),
        case('zero-steps', [[0,1]], steps=0),
        case('zero-stagnation', [[0,1]], unimproved=0),
        case('all-pinned', [[0,1],[0,1]], [(0,1)], pins=[True,True], unimproved=2),
        case('singleton-domain', [[0]], unimproved=2),
        case('all-criteria-initial', [[0,1]], steps=0, unimproved=0, target=[0,0], feasible=True),
    ]
    # Original fixture found with seed 20261120: a worse fallback and recovery
    # precede a new global best. Different limits produce different final scores,
    # so final-solution agreement can detect early/late Java termination here.
    delayed = case('delayed-improvement',
                   [[-3,-3,-5,-3],[-3,-6,-2,-5],[1,3,-1,4],[3,5,2,3]],
                   [(0,1),(0,2),(0,3),(1,2),(2,3)], initial=[1,3,1,3])
    for limit in [0,1,2,3,4]:
        cases.append({**delayed, 'id':f'delayed-stagnation-{limit}', 'unimproved':limit})
    for limit in [1,2,3,4,5]:
        cases.append({**delayed, 'id':f'delayed-step-{limit}', 'steps':limit})
    for target in [[0,-2],[0,0],[0,4],[1,0]]:
        cases.append({**delayed, 'id':f'delayed-target-{target[0]}-{target[1]}', 'target':target})
    cases.append({**delayed, 'id':'delayed-feasibility', 'feasible':True})
    rng = random.Random(SEED)
    for i in range(48):
        n, k = rng.randint(1,6), rng.randint(2,4)
        weights = [[rng.randint(-8,8) for _ in range(k)] for _ in range(n)]
        initial = [rng.randrange(k) for _ in range(n)]
        pins = [rng.random()<.25 for _ in range(n)]
        edges = [(a,b) for a in range(n) for b in range(a+1,n) if rng.random()<.45]
        if i % 8 == 0:
            edges += [(0,0),(0,n-1),(0,n-1)]
        base = case(f'random-{i}', weights, edges, pins, initial, steps=rng.choice([1,4,12]))
        initial_score = score(base, initial)
        for mode in ['stagnation','target','feasible','combined']:
            c = copy.deepcopy(base)
            c['id'] += '-' + mode
            if mode in ['stagnation','combined']:
                c['unimproved'] = rng.choice([0,1,2,4])
            if mode in ['target','combined']:
                c['target'] = list(rng.choice([initial_score,(0,0),(1,0),(-1,0)]))
            c['feasible'] = mode in ['feasible','combined']
            cases.append(c)
    return cases


def encode(case):
    target = case['target']
    return encode_problem(case) + '\t' + '\t'.join([
        str(case['steps']), '-' if case['unimproved'] is None else str(case['unimproved']),
        '-' if target is None else f'{target[0]}hard/{target[1]}soft', str(int(case['feasible']))])


def check_java(case, line, expected):
    error = validate(case, line, tuple(expected['score']))
    if error:
        raise ValueError(error)
    values = list(map(int, line.split('\t')[3].split(',')))
    if values != expected['values']:
        raise ValueError('Java final assignment differs from deterministic reference')


def check_rust(case, line, expected, backend):
    f = line.split('\t')
    if len(f) != 9 or f[1] != backend:
        raise ValueError('Rust backend/field mismatch')
    error = validate(case, '\t'.join([f[0],*f[2:5]]), tuple(expected['score']))
    if error:
        raise ValueError(error)
    values = list(map(int, f[4].split(',')))
    actual = {'values':values, 'steps':int(f[5]), 'unimproved':int(f[6]),
              'reason':f[7], 'accepted':int(f[8])}
    if any(actual[k] != expected[k] for k in actual):
        raise ValueError(f'Rust {backend} differs from reference: {actual}')


def run_checks(classpath):
    subprocess.run(['cargo','build','--release','--example','termination_oracle','--locked'],cwd=ROOT,check=True)
    cases = generate()
    payload = '\n'.join(map(encode,cases))+'\n'
    outputs = {}
    for name, command in [('java',['java','-Xms256m','-Xmx256m','-cp',classpath,'TerminationOracle']),
                          ('rust',[str(ROOT/'target/release/examples/termination_oracle')])]:
        run = subprocess.run(command,input=payload,text=True,capture_output=True,timeout=180)
        (WORK/f'termination-{name}.stderr.log').write_text(run.stderr)
        if run.returncode:
            raise RuntimeError(f'{name} termination adapter failed; see work/oracle/termination-{name}.stderr.log')
        outputs[name] = run.stdout.splitlines()
        if len(outputs[name]) != len(cases)*(2 if name=='rust' else 1):
            raise ValueError(f'{name} termination response count')
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
    (WORK/'termination-cases.json').write_text(json.dumps({'schema_version':1,'seed':SEED,'cases':cases,'references':references,'outputs':outputs},indent=2)+'\n')
    failed = {f['case'] for f in failures}
    report = {'seed':SEED,'total':len(cases),'passed':len(cases)-len(failed),'failures':failures,
              'native_reference_steps':sum(r['steps'] for r in references),
              'scope':'Original complete required-variable graph cases. Public Community OR-composed step, unimproved-step, best-score and feasibility phase limits; deterministic original-order best-accepted hill climbing with first tie and best-rejected fallback. Final scores and assignments agree with independent Python neighbor enumeration; Rust scoped and stream step counts, stagnation counts, acceptance counts and reasons are checked separately. Java internal step counts/reasons/traces, randomized acceptance, list/mixed search, timing and upstream method coverage are not claimed.'}
    print(f'Termination: {report["passed"]}/{report["total"]} cases; {report["native_reference_steps"]} native reference steps',flush=True)
    for failure in failures[:8]: print(failure,flush=True)
    return report


if __name__ == '__main__':
    from prepare_solver_oracle import prepare_solver_java
    cp, _ = prepare_solver_java()
    raise SystemExit(bool(run_checks(cp)['failures']))
