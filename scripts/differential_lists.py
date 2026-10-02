#!/usr/bin/env python3
"""Compare original list moves and four derived shadows with Java public APIs."""
from copy import deepcopy
from datetime import datetime, timezone
import json
import platform
import random
import subprocess
from differential_scores import ROOT, REF, WORK, source_digest
from prepare_solver_oracle import prepare_solver_java

SEED = 20261003


def apply(lists, op):
    """Independent reference based on slicing and concatenation."""
    lists = deepcopy(lists)
    kind, *v = op
    if kind == 'a':
        value, e, i = v
        if any(value in row for row in lists): raise ValueError('Already assigned')
        lists[e] = lists[e][:i] + [value] + lists[e][i:]
    elif kind == 'c':
        e, i, d, j = v
        value = lists[e][i]
        lists[e] = lists[e][:i] + lists[e][i+1:]
        lists[d] = lists[d][:j] + [value] + lists[d][j:]
    elif kind == 's':
        e, i, d, j = v
        a, b = lists[e][i], lists[d][j]
        lists[e][i], lists[d][j] = b, a
    elif kind == 'u':
        e, start, end = v
        lists[e] = lists[e][:start] + lists[e][end:]
    else:
        raise ValueError('Unknown move')
    return lists


def state(lists, count):
    flat = [v for row in lists for v in row]
    if len(flat) != len(set(flat)) or any(v not in range(count) for v in flat):
        raise ValueError('Invalid list assignment')
    shadows = []
    for v in range(count):
        found = [(e,i) for e,row in enumerate(lists) for i,x in enumerate(row) if x == v]
        if not found:
            shadows.append('-')
        else:
            e,i = found[0]
            prev = '-' if i == 0 else str(lists[e][i-1])
            nxt = '-' if i+1 == len(lists[e]) else str(lists[e][i+1])
            shadows.append(f'{e}:{i}:{prev}:{nxt}')
    return ';'.join(','.join(map(str,row)) or '-' for row in lists) + '/' + ','.join(shadows)


def encode(case):
    return '\t'.join([case['id'],str(case['count']),';'.join(','.join(map(str,row)) or '-' for row in case['initial']),
                      ';'.join(','.join(map(str,op)) for op in case['moves'])])


def expected(case):
    before = state(case['initial'],case['count'])
    after = deepcopy(case['initial'])
    for op in case['moves']: after = apply(after,op)
    after = state(after,case['count'])
    return '\t'.join([case['id'],after,before,after])


def validate(case, line):
    if line != expected(case): return 'Candidate, rollback, committed state or shadows differ from independent reference'
    return None


def generate():
    cases = []
    def add(count, initial, moves):
        cases.append({'id':f'list-{len(cases):03}', 'count':count, 'initial':initial, 'moves':moves})
    add(0,[[]],[])
    add(3,[[],[]],[['a',0,0,0],['a',1,0,1],['a',2,0,1]])
    for source,dest in [(0,2),(2,0),(1,0),(1,2),(0,1)]:
        add(3,[[0,1,2]],[['c',0,source,0,dest]])
    for dest in range(3): add(4,[[0,1],[2,3]],[['c',0,0,1,dest]])
    add(2,[[0,1],[]],[['c',0,0,1,0]])
    add(3,[[0,1,2]],[['s',0,0,0,2]])
    add(4,[[0,1],[2,3]],[['s',1,0,0,1]])
    add(3,[[0,1,2]],[['u',0,0,2]])
    add(4,[[0,1,2,3]],[['u',0,1,3]])
    add(1,[[0],[]],[['u',0,0,1],['a',0,1,0]])
    rng = random.Random(SEED)
    for _ in range(240):
        count, entities = rng.randint(0,7), rng.randint(1,3)
        initial = [[] for _ in range(entities)]
        values = list(range(count)); rng.shuffle(values)
        for value in values:
            if rng.random() < 0.8: initial[rng.randrange(entities)].append(value)
        current = deepcopy(initial)
        moves = []
        for _ in range(rng.randint(1,8)):
            options = []
            for e,row in enumerate(current):
                for value in range(count):
                    if not any(value in other for other in current):
                        options += [['a',value,e,i] for i in range(len(row)+1)]
                for i in range(len(row)):
                    for d,other in enumerate(current):
                        options += [['c',e,i,d,j] for j in range(len(other)+1-(d==e)) if (e,i)!=(d,j)]
                        options += [['s',e,i,d,j] for j in range(len(other)) if (e,i)!=(d,j)]
                options += [['u',e,start,end] for start in range(len(row)) for end in range(start+1,len(row)+1)]
            if not options: break
            op = rng.choice(options)
            moves.append(op); current = apply(current,op)
        add(count,initial,moves)
    return cases


def main():
    classpath, artifacts = prepare_solver_java()
    subprocess.run(['javac','-cp',classpath,'-d',str(WORK/'solver'),str(ROOT/'compat/oracle/ListMoveOracle.java')],check=True)
    subprocess.run(['cargo','build','--release','--example','list_move_oracle','--locked'],cwd=ROOT,check=True)
    digest = source_digest()
    cases = generate()
    payload = '\n'.join(map(encode,cases))+'\n'
    commands = {'java':['java','-cp',classpath,'ListMoveOracle'], 'rust':[str(ROOT/'target/release/examples/list_move_oracle')]}
    outputs = {}
    for name,command in commands.items():
        run = subprocess.run(command,input=payload,text=True,capture_output=True,timeout=120)
        (WORK/f'list-{name}.stderr.log').write_text(run.stderr)
        if run.returncode: raise RuntimeError(f'{name} failed; see work/oracle/list-{name}.stderr.log')
        outputs[name] = run.stdout.splitlines()
        if len(outputs[name]) != len(cases): raise RuntimeError(f'{name} response count mismatch')
    failures = []
    for i,case in enumerate(cases):
        for name in outputs:
            error = validate(case,outputs[name][i])
            if error: failures.append({'case':case['id'],'runtime':name,'error':error,'expected':expected(case),'actual':outputs[name][i]})
    if source_digest() != digest: raise RuntimeError('Sources changed during comparison')
    failed = {f['case'] for f in failures}
    report = {'schema_version':1,'generated_at':datetime.now(timezone.utc).isoformat(),'source_digest':digest,
              'revision':REF['commit'],'seed':SEED,'total':len(cases),'passed':len(cases)-len(failed),'failures':failures,
              'moves':sum(len(c['moves']) for c in cases),'artifacts':artifacts,'environment':platform.platform(),
              'scope':'Original valid assign/change/swap/span-unassign composites. Java public MoveTester candidate, undo and commit states, with owner/index/previous/next shadows, independently checked in Python. One optional list variable; no pinned Java cases, custom listeners, list solving, timing or upstream coverage credit.'}
    (ROOT/'work/list-differential.json').write_text(json.dumps(report,indent=2)+'\n')
    (WORK/'list-cases.json').write_text(json.dumps({'schema_version':1,'seed':SEED,'cases':cases,'outputs':outputs},indent=2)+'\n')
    print(f'Differential list cases: {report["passed"]}/{report["total"]} passed ({report["moves"]} moves)')
    for failure in failures[:8]: print(json.dumps(failure))
    return bool(failures)

if __name__ == '__main__': raise SystemExit(main())
