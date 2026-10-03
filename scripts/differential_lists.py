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
SUBLIST_SEED = 20261124
SUBLIST_SWAP_SEED = 20261125


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
    elif kind == 'r':
        e, start, end, d, j, reverse = v
        if not 0 <= e < len(lists) or not 0 <= d < len(lists) or reverse not in (0,1):
            raise ValueError('Invalid span metadata')
        if not 0 <= start <= end <= len(lists[e]):
            raise ValueError('Invalid span bounds')
        if not 0 <= j <= len(lists[d])-(end-start if d==e else 0):
            raise ValueError('Invalid post-removal insertion')
        block = lists[e][start:end]
        lists[e] = lists[e][:start] + lists[e][end:]
        if reverse: block = block[::-1]
        lists[d] = lists[d][:j] + block + lists[d][j:]
    elif kind == 'x':
        e, start, end, d, other_start, other_end, reverse = v
        if not 0 <= e < len(lists) or not 0 <= d < len(lists) or reverse not in (0,1):
            raise ValueError('Invalid swap metadata')
        if not (0 <= start < end <= len(lists[e]) and 0 <= other_start < other_end <= len(lists[d])):
            raise ValueError('Invalid nonempty swap bounds')
        if e == d and start < other_end and other_start < end:
            raise ValueError('Overlapping swap spans')
        left, right = lists[e][start:end], lists[d][other_start:other_end]
        if reverse: left, right = left[::-1], right[::-1]
        # Read only pre-move rows; emit replacement blocks at their original starts.
        result = []
        for owner,row in enumerate(lists):
            output = []
            for i,value in enumerate(row):
                if owner == e and i == start: output.extend(right)
                if owner == d and i == other_start: output.extend(left)
                if not ((owner == e and start <= i < end) or (owner == d and other_start <= i < other_end)):
                    output.append(value)
            result.append(output)
        lists = result
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
    # Preserve the original 256 cases; append a separately seeded span corpus.
    add(5,[[0,1,2,3,4]],[['r',0,1,3,0,3,0]])
    add(5,[[0,1,2,3,4]],[['r',0,3,5,0,0,1]])
    add(5,[[0,1,2],[3,4]],[['r',0,0,3,1,1,1]])
    add(4,[[0,1],[2,3]],[['r',0,0,2,1,0,0],['r',1,0,3,0,0,1]])
    rng = random.Random(SUBLIST_SEED)
    for _ in range(124):
        count, entities = rng.randint(2,7), rng.randint(1,3)
        initial = [[] for _ in range(entities)]
        initial[rng.randrange(entities)] = rng.sample(range(count),count)
        current, moves = deepcopy(initial), []
        for _ in range(rng.randint(1,6)):
            options = [['r',e,start,end,d,j,reverse]
                       for e,row in enumerate(current) for start in range(len(row)) for end in range(start+1,len(row)+1)
                       for d,other in enumerate(current) for j in range(len(other)+1-(end-start if d==e else 0))
                       for reverse in (0,1) if not (e==d and j==start and (reverse==0 or end-start==1))]
            op = rng.choice(options)
            moves.append(op); current = apply(current,op)
        add(count,initial,moves)
    # Preserve all previous inputs; original swaps use an independent random seed.
    rng = random.Random(SUBLIST_SWAP_SEED)
    for _ in range(128):
        count, entities = rng.randint(4,8), rng.randint(1,3)
        initial = [[] for _ in range(entities)]
        for value in rng.sample(range(count),count): initial[rng.randrange(entities)].append(value)
        current, moves = deepcopy(initial), []
        for _ in range(rng.randint(1,6)):
            spans = [(e,start,end) for e,row in enumerate(current)
                     for start in range(len(row)) for end in range(start+1,len(row)+1)]
            options = [['x',e,start,end,d,other_start,other_end,reverse]
                       for e,start,end in spans for d,other_start,other_end in spans
                       if e != d or end <= other_start or other_end <= start
                       for reverse in (0,1)]
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
              'revision':REF['commit'],'seed':SEED,'sublist_seed':SUBLIST_SEED,'sublist_swap_seed':SUBLIST_SWAP_SEED,'total':len(cases),'passed':len(cases)-len(failed),'failures':failures,
              'moves':sum(len(c['moves']) for c in cases),'artifacts':artifacts,'environment':platform.platform(),
              'scope':'Original valid assign/change/swap/span-unassign composites, plus 128 span-relocation/reversal cases (seed 20261124). 128 original disjoint span-swap/reversal cases (seed 20261125), including unequal lengths and reversed argument order. Java public MoveTester candidate, undo and commit states, with owner/index/previous/next shadows, independently checked in Python. Rust now emits cached ListStreamScoreDirector value views and checks cached entity lists plus an independently recomputed native position score. No Java/Rust score comparison is claimed here. One optional list variable; no pinned Java cases, custom listeners, list solving, timing or upstream coverage credit.'}
    (ROOT/'work/list-differential.json').write_text(json.dumps(report,indent=2)+'\n')
    (WORK/'list-cases.json').write_text(json.dumps({'schema_version':1,'seed':SEED,'sublist_seed':SUBLIST_SEED,'sublist_swap_seed':SUBLIST_SWAP_SEED,'cases':cases,'outputs':outputs},indent=2)+'\n')
    print(f'Differential list cases: {report["passed"]}/{report["total"]} passed ({report["moves"]} moves)')
    for failure in failures[:8]: print(json.dumps(failure))
    return bool(failures)

if __name__ == '__main__': raise SystemExit(main())
