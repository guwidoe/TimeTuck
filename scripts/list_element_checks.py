#!/usr/bin/env python3
"""Original generated list-element comparisons. No upstream method credit."""
from copy import deepcopy
import random
import subprocess
from differential_scores import ROOT, WORK
from follower_checks import row, text
SEED = 20261005


def valid(case, state):
    durations, lists = state['durations'], state['lists']
    if case['model'] not in ('o', 'm') or len(durations) != len(case['durations']) or len(lists) != len(case['starts']):
        raise ValueError('List element shape/model')
    if not lists or any(type(x) is not int or x < 0 for x in case['starts']):
        raise ValueError('List element starts')
    if any(not (v is None and case['model'] == 'm') and (type(v) is not int or not 1 <= v <= 9) for v in durations):
        raise ValueError('List element duration')
    if case['model'] == 'o' and durations != case['durations']:
        raise ValueError('Fixed durations changed')
    values = [v for items in lists for v in items]
    if any(type(v) is not int or not 0 <= v < len(durations) for v in values) or len(values) != len(set(values)):
        raise ValueError('List value domain/uniqueness')


def apply(case, state, moves):
    result = deepcopy(state)
    valid(case, result)
    durations, lists = result['durations'], result['lists']
    def index(e, i, insertion=False):
        if type(e) is not int or not 0 <= e < len(lists) or type(i) is not int or not 0 <= i < len(lists[e]) + int(insertion):
            raise ValueError('List position')
    for op in moves:
        match op:
            case ['b', i, value] if case['model'] == 'm':
                if type(i) is not int or not 0 <= i < len(durations):
                    raise ValueError('Basic index')
                durations[i] = value
            case ['a', v, e, i]:
                index(e, i, True)
                lists[e].insert(i, v)
            case ['c', e, i, d, j]:
                index(e, i)
                v = lists[e].pop(i)
                index(d, j, True)
                lists[d].insert(j, v)
            case ['s', e, i, d, j]:
                index(e, i); index(d, j)
                lists[e][i], lists[d][j] = lists[d][j], lists[e][i]
            case ['u', e, i, j] if case['model'] == 'o':
                index(e, i)
                if type(j) is not int or not i < j <= len(lists[e]):
                    raise ValueError('Unassign span')
                del lists[e][i:j]
            case _:
                raise ValueError('List element operation')
        valid(case, result)
    return result


def states(case):
    current = {k: deepcopy(case[k]) for k in ('durations', 'lists')}
    valid(case, current)
    yield deepcopy(current)
    for batch in case['batches']:
        if batch['mode'] not in ('t', 'c'):
            raise ValueError('List element mode')
        candidate = apply(case, current, batch['moves'])
        yield deepcopy(candidate)
        if batch['mode'] == 't':
            yield deepcopy(current)
        else:
            current = candidate


def expected(case, index, state):
    valid(case, state)
    ds, ls = state['durations'], state['lists']
    output = [case['id'], str(index), ';'.join(row(items) for items in ls), row(ds)]
    if case['model'] == 'm':
        padded = [None if d is None else d + 1 for d in ds]
        totals = [None if any(padded[v] is None for v in items) else sum(padded[v] for v in items) for items in ls]
        return '\t'.join(output + ['_', '_', '_', row(padded), row(totals), '0'])
    owners, previous, starts, ends = ([None]*len(ds) for _ in range(4))
    totals = []
    for e, items in enumerate(ls):
        for i, v in enumerate(items):
            owners[v] = e
            previous[v] = items[i-1] if i else None
            starts[v] = case['starts'][e] + sum(ds[x] for x in items[:i])
            ends[v] = starts[v] + ds[v]
        totals.append(case['starts'][e] + sum(ds[x] for x in items))
    score = -100*owners.count(None) - sum(totals)
    return '\t'.join(output + [row(owners), row(previous), row(starts), row(ends), row(totals), str(score)])


def encode(case):
    batches = '|'.join(b['mode']+'@'+';'.join(','.join(map(text, op)) for op in b['moves']) for b in case['batches'])
    return '\t'.join([case['id'], case['model'], row(case['starts']), row(case['durations']),
                      ';'.join(row(items) for items in case['lists']), batches])


def validate_outputs(cases, outputs):
    observations = [(c,i,s) for c in cases for i,s in enumerate(states(c))]
    if set(outputs) != {'java','rust'} or any(len(rows) != len(observations) for rows in outputs.values()):
        raise ValueError('List element response count/runtime mismatch')
    results = []
    for j,(case,i,state) in enumerate(observations):
        want = expected(case,i,state)
        for runtime, rows in outputs.items():
            if rows[j] != want:
                raise ValueError(f'List element {runtime}: expected {want!r}, got {rows[j]!r}')
        results.append({'id':case['id'],'state':i,'assignment':state,**{r:rows[j] for r,rows in outputs.items()}})
    return results


def generate():
    cases=[]
    def add(model, starts, durations, lists, batches):
        cases.append({'id':f'list-element-{len(cases):03}', 'model':model,'starts':starts,'durations':durations,'lists':lists,
                      'batches':[{'mode':m,'moves':ops} for m,ops in batches]})
    add('o',[4,11],[3,7,2],[[2,0,1],[]],[('t',[['u',0,0,3],['a',0,1,0]]),('c',[['c',0,2,1,0],['s',0,0,0,1]]),('t',[])])
    add('m',[0,0],[None,3,2],[[0,1],[2]],[('t',[['b',0,5],['c',0,1,1,0]]),('c',[['c',0,0,1,0]]),('t',[['b',0,2],['b',0,None]])])
    add('o',[3],[],[[]],[('t',[])])
    add('m',[0],[],[[]],[('c',[])])
    rng=random.Random(SEED)
    while len(cases)<128:
        model='o' if len(cases)%2==0 else 'm'
        n=rng.randint(1,8); m=rng.randint(1,4)
        durations=[rng.choice([None,*range(1,10)]) if model=='m' else rng.randint(1,9) for _ in range(n)]
        items=list(range(n));rng.shuffle(items); lists=[[] for _ in range(m)]
        for v in items:
            if rng.random()<.8: lists[rng.randrange(m)].append(v)
        add(model,[rng.randint(0,12) if model=='o' else 0 for _ in range(m)],durations,lists,[])
        case=cases[-1]; current={k:deepcopy(case[k]) for k in ('durations','lists')}
        for _ in range(12):
            candidate=deepcopy(current); moves=[]
            for _ in range(rng.randint(1,4)):
                ls=candidate['lists'];positions=[(e,i) for e,vs in enumerate(ls) for i in range(len(vs))]
                unassigned=sorted(set(range(n))-{v for vs in ls for v in vs})
                changes=[(e,i,d,j) for e,i in positions for d in range(m) for j in range(len(ls[d])+1-int(e==d)) if (e,i)!=(d,j)]
                swaps=[(e,i,d,j) for e,i in positions for d,j in positions if (e,i)!=(d,j)]
                kinds=(['b'] if model=='m' else [])+(['a'] if unassigned else [])+(['c'] if changes else [])+(['s'] if swaps else [])+(['u'] if model=='o' and positions else [])
                kind=rng.choice(kinds)
                if kind=='b': op=['b',rng.randrange(n),rng.choice([None,*range(1,10)])]
                elif kind=='a':
                    e=rng.randrange(m);op=['a',rng.choice(unassigned),e,rng.randrange(len(ls[e])+1)]
                else:
                    e,i=rng.choice(positions)
                    if kind=='c':
                        op=['c',*rng.choice(changes)]
                    elif kind=='s': op=['s',*rng.choice(swaps)]
                    else: op=['u',e,i,rng.randint(i+1,len(ls[e]))]
                candidate=apply(case,candidate,[op]);moves.append(op)
            mode=rng.choice(['c','t']);case['batches'].append({'mode':mode,'moves':moves})
            if mode=='c':current=candidate
    return cases


def run_checks(classpath):
    cases=generate();payload='\n'.join(map(encode,cases))+'\n'
    subprocess.run(['javac','-cp',classpath,'-d',str(WORK/'solver'),str(ROOT/'compat/oracle/ListElementOracle.java')],check=True)
    subprocess.run(['cargo','build','--release','--locked','--example','list_element_oracle'],cwd=ROOT,check=True)
    outputs={}
    for runtime,cmd in {'java':['java','-Xms256m','-Xmx256m','-cp',classpath,'ListElementOracle'],
                        'rust':[str(ROOT/'target/release/examples/list_element_oracle')]}.items():
        result=subprocess.run(cmd,input=payload,text=True,capture_output=True,timeout=180)
        if result.returncode:raise RuntimeError(f'List element {runtime}: {result.stderr}')
        outputs[runtime]=result.stdout.splitlines()
    results=validate_outputs(cases,outputs)
    return {'seed':SEED,'passed':len(cases),'total':len(cases),'observations':len(results),
            'operations':sum(len(b['moves']) for c in cases for b in c['batches']),'cases':cases,'results':results,
            'scope':'Original fixed-duration ordered chains and nullable genuine-duration list aggregates. Public Java MoveTester maintains annotated shadows; ordered scores are rebuilt with UPDATE_ALL on a detached solution reconstructed from genuine/fixed inputs; original MoveTester shadows are observed unchanged. Native streams rescan touched owner chains or update membership aggregates. Independent Python validates lists, durations, ordered owners/previous/start/end, padded values and owner aggregates through sequential composite moves, commit and undo. Mixed fixtures have no scoring provider; their zero score field is a protocol placeholder, not scoring evidence. Includes empty lists, zero values, initial unassigned values, relocation, swap, ordered assign/unassign and mixed basic-only updates. No supplier counts, Java structural fields, mutable fixed starts/durations, search quality, timing or additional upstream-method credit.'}
