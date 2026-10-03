#!/usr/bin/env python3
"""Original concurrent schedule inputs and independent recursive reference."""
from copy import deepcopy
import base64
import random
import subprocess
from differential_scores import ROOT,WORK
from follower_checks import row,text
SEED=20261006

def valid(case,lists):
    n=case['count'];members=[v for g in case['groups'] for v in g];assigned=[v for l in lists for v in l]
    if type(n) is not int or not 0<=n<=64 or not lists or len(lists)!=len(case['lists']):raise ValueError('Concurrent shape')
    for values in [members,assigned]:
        if any(type(v) is not int or not 0<=v<n for v in values) or len(values)!=len(set(values)):raise ValueError('Concurrent identity')
    if any(not g for g in case['groups']):raise ValueError('Empty group')

def apply(case,lists,moves):
    lists=deepcopy(lists);valid(case,lists)
    def position(e,i,insert=False):
        if type(e) is not int or not 0<=e<len(lists) or type(i) is not int or not 0<=i<len(lists[e])+int(insert):raise ValueError('Concurrent position')
    for op in moves:
        match op:
            case ['a',v,e,i]:position(e,i,True);lists[e].insert(i,v)
            case ['c',e,i,d,j]:
                position(e,i);v=lists[e].pop(i);position(d,j,True);lists[d].insert(j,v)
            case ['s',e,i,d,j]:position(e,i);position(d,j);lists[e][i],lists[d][j]=lists[d][j],lists[e][i]
            case _:raise ValueError('Concurrent move')
        valid(case,lists)
    return lists

def states(case):
    current=deepcopy(case['lists']);valid(case,current);yield deepcopy(current)
    for batch in case['batches']:
        if batch['mode'] not in ('c','t'):raise ValueError('Concurrent mode')
        candidate=apply(case,current,batch['moves']);yield deepcopy(candidate)
        if batch['mode']=='t':yield deepcopy(current)
        else:current=candidate

def project(case,lists):
    valid(case,lists);n=case['count'];roots=list(range(n))
    for group in case['groups']:
        for v in group:roots[v]=group[0]
    owner,index,previous,next_value=([None]*n for _ in range(4))
    for e,items in enumerate(lists):
        for i,v in enumerate(items):
            owner[v],index[v]=e,i
            previous[v]=items[i-1] if i else None
            next_value[v]=items[i+1] if i+1<len(items) else None
    dependencies={r:set() for r in roots}
    for v,p in enumerate(previous):
        if p is not None:dependencies[roots[v]].add(roots[p])
    cache={};active=set()
    def start(r):
        if r in active:return True,None
        if r in cache:return cache[r]
        active.add(r);inputs=[start(p) for p in sorted(dependencies[r])];active.remove(r)
        if any(bad for bad,_ in inputs):out=True,None
        elif not any(owner[v] is not None and roots[v]==r for v in range(n)):out=False,None
        else:out=False,max([0,*[s+60 for _,s in inputs]])
        cache[r]=out;return out
    times=[start(r) for r in roots]
    ready=[None if owner[v] is None else 0 if previous[v] is None else None if times[previous[v]][0] else times[previous[v]][1]+60 for v in range(n)]
    begins=[t for _,t in times];finish=[None if t is None else t+30 for t in begins];bad=[int(b) for b,_ in times]
    return {'owner':owner,'index':index,'previous':previous,'next':next_value,'cascading':[None if i is None else i*1440 for i in index],
            'ready':ready,'start':begins,'finish':finish,'inconsistent':bad,
            'hard':-sum(bad),'soft':-sum(finish[v] for v in range(n) if owner[v] is not None and not bad[v])}

def expected(case,i,lists):
    p=project(case,lists)
    return '\t'.join([case['id'],str(i),';'.join(row(l) for l in lists),str(p['hard']),str(p['soft']),
                      *[row(p[k]) for k in ['owner','index','previous','next','cascading','ready','start','finish','inconsistent']]])

def encode(c):
    batches='|'.join(b['mode']+'@'+';'.join(','.join(map(text,op)) for op in b['moves']) for b in c['batches'])
    return '\t'.join([c['id'],str(c['count']),';'.join(row(g) for g in c['groups']) or '_',';'.join(row(l) for l in c['lists']),batches])

def validate_outputs(cases,outputs):
    observations=[(c,i,ls) for c in cases for i,ls in enumerate(states(c))]
    if set(outputs)!={'java','rust'} or len(outputs['rust'])!=len(observations):raise ValueError('Concurrent response count/runtime')
    results=[];failures=[];failed=set();errors=set();divergent=set()
    java_index=0;java_observations=0;native_index=0
    for c in cases:
        errored=False
        for i,ls in enumerate(states(c)):
            want=expected(c,i,ls);native=outputs['rust'][native_index];native_index+=1
            if native!=want:raise ValueError(f'Concurrent rust: expected {want!r}, got {native!r}')
            java=None
            if not errored:
                if java_index>=len(outputs['java']):raise ValueError('Missing Java observation/error')
                java=outputs['java'][java_index];java_index+=1
                fields=java.split('\t')
                if fields[0]=='ERROR':
                    if len(fields)!=3 or fields[1]!=c['id']:raise ValueError('Malformed Java error identity')
                    message=base64.b64decode(fields[2],validate=True).decode('utf-8')
                    failures.append({'case':c['id'],'state':i,'kind':'java_runtime_error','message':message})
                    failed.add(c['id']);errors.add(c['id']);errored=True;java=None
                else:
                    if len(fields)!=14 or fields[:2]!=[c['id'],str(i)]:raise ValueError('Java observation order/shape')
                    java_observations+=1
                    if java!=want:
                        failures.append({'case':c['id'],'state':i,'kind':'java_divergence','expected':want,'actual':java})
                        failed.add(c['id']);divergent.add(c['id'])
            results.append({'id':c['id'],'state':i,'lists':ls,'rust':native,'java':java})
    if java_index!=len(outputs['java']):raise ValueError('Extra Java observations')
    return {'passed':len(cases)-len(failed),'total':len(cases),'native_reference_passed':len(cases),
            'observations':len(results),'java_observations':java_observations,'java_error_cases':len(errors),
            'divergent_cases':len(divergent),'failures':failures,'results':results}

def generate():
    cases=[]
    def add(n,groups,lists,batches):
        cases.append({'id':f'concurrent-{len(cases):03}','count':n,'groups':groups,'lists':lists,'batches':[{'mode':m,'moves':ops} for m,ops in batches]})
    # Original boundaries, distinct from the pinned method fixtures.
    add(4,[[0,2]],[[0,1],[2,3]],[('t',[['s',0,0,0,1]]),('c',[['c',1,0,0,2]]),('t',[['c',0,2,1,0]])])
    add(3,[[0,1]],[[2,1],[]],[('t',[['a',0,1,0]]),('c',[['c',0,1,1,0]])])
    add(0,[],[[],[]],[('t',[])])
    rng=random.Random(SEED)
    while len(cases)<128:
        n=rng.randint(1,8);m=rng.randint(1,4);order=list(range(n));rng.shuffle(order)
        groups=[]
        while len(order)>1:
            k=rng.randint(1,min(3,len(order)))
            members=order[:k];del order[:k]
            if k>1:groups.append(sorted(members))
        order=list(range(n));rng.shuffle(order);lists=[[] for _ in range(m)]
        for v in order:
            if rng.random()<.8:lists[rng.randrange(m)].append(v)
        add(n,groups,lists,[]);case=cases[-1];current=deepcopy(lists)
        for _ in range(12):
            candidate=deepcopy(current);moves=[]
            for _ in range(rng.randint(1,4)):
                positions=[(e,i) for e,ls in enumerate(candidate) for i in range(len(ls))]
                free=sorted(set(range(n))-{v for ls in candidate for v in ls})
                changes=[(e,i,d,j) for e,i in positions for d in range(m) for j in range(len(candidate[d])+1-int(e==d)) if (e,i)!=(d,j)]
                swaps=[(e,i,d,j) for e,i in positions for d,j in positions if (e,i)!=(d,j)]
                kinds=(['a'] if free else [])+(['c'] if changes else [])+(['s'] if swaps else [])
                if not kinds:break
                k=rng.choice(kinds)
                if k=='a':e=rng.randrange(m);op=['a',rng.choice(free),e,rng.randrange(len(candidate[e])+1)]
                else:op=[k,*rng.choice(changes if k=='c' else swaps)]
                candidate=apply(case,candidate,[op]);moves.append(op)
            mode=rng.choice(['c','t']);case['batches'].append({'mode':mode,'moves':moves})
            if mode=='c':current=candidate
    return cases

def original_fixture_boundary(classpath):
    fixture=ROOT/'upstream/timefold-solver/core/src/test/java/ai/timefold/solver/core/testdomain/shadow/concurrent'
    subprocess.run(['javac','-cp',classpath,'-d',str(WORK/'solver'),*[str(p) for p in sorted(fixture.glob('*.java'))],str(ROOT/'compat/oracle/ConcurrentOriginalBoundary.java')],check=True)
    result=subprocess.run(['java','-cp',classpath,'ConcurrentOriginalBoundary'],text=True,capture_output=True,timeout=60)
    if result.returncode:raise RuntimeError('Original fixture diagnostic: '+result.stderr)
    lines=result.stdout.splitlines()
    if len(lines)!=2 or lines[0]!='assigned_member_finish=null' or not lines[1].startswith('error=java.lang.NullPointerException:'):
        raise ValueError('Original fixture boundary changed: '+result.stdout)
    return {'outcome':'java_runtime_error','output':lines,'scope':'Original minimal input: 3 values, group [0,1], lists [2,1]/empty, first group member unassigned. Compile unchanged pinned concurrent test fixtures; invoke public MoveTester and detached SolutionManager.UPDATE_ALL. Assigned member finish remains null and original scoring throws NullPointerException. Diagnostic only, no pass or method credit.'}

def run_checks(classpath):
    cases=generate();payload='\n'.join(map(encode,cases))+'\n'
    subprocess.run(['javac','-cp',classpath,'-d',str(WORK/'solver'),str(ROOT/'compat/oracle/ConcurrentOracle.java')],check=True)
    subprocess.run(['cargo','build','--release','--locked','--example','concurrent_oracle'],cwd=ROOT,check=True)
    outputs={}
    for runtime,cmd in {'java':['java','-Xms256m','-Xmx256m','-cp',classpath,'ConcurrentOracle'],'rust':[str(ROOT/'target/release/examples/concurrent_oracle')]}.items():
        r=subprocess.run(cmd,input=payload,text=True,capture_output=True,timeout=180)
        if r.returncode:raise RuntimeError(f'Concurrent {runtime}: {r.stderr}')
        outputs[runtime]=r.stdout.splitlines()
    validated=validate_outputs(cases,outputs)
    boundary=original_fixture_boundary(classpath)
    return {'seed':SEED,**validated,'original_fixture_boundary':boundary,'operations':sum(len(b['moves']) for c in cases for b in c['batches']),
            'cases':cases,'scope':'Original fixed disjoint alignment groups over required planning lists, including unassigned group members, dynamic cycles and downstream invalidation/recovery, zero values, shared owners, assign/relocate/swap composites and trial undo. Public Java MoveTester shadows are observed unchanged; detached UPDATE_ALL solutions provide ordinary scores. Native shared dependency evaluation is validated against independent Python recursion for every state. Java runtime errors and divergent states are retained and excluded from agreement credit. Successful Java observations compare every owner/index/neighbor/cascading/ready/start/finish field, inconsistency flag and ordinary score. Times are minute offsets from a fixed base; no date/time API parity. Supplier-call counts, Java structural fields, mutable groups, search quality and timing are not compared; generated inputs earn no upstream method credit.'}
