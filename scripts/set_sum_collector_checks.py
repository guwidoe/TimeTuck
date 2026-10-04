"""Original public set, sorted set, wrapping map merge and checked sum checks."""
import random
import subprocess
from differential_scores import ROOT
SEED=20261227
LOW,HIGH=-2**63,2**63-1

def checked(v):
    if not LOW<=v<=HIGH:raise OverflowError
    return v

def expected(c):
    slots={};total=0;state=0;out=[]
    def emit():
        rows=[c['items'][i] for i in slots.values()]
        if c['mode']=='sum':result=str(total)
        elif c['mode']=='map':
            groups={}
            for key,value in rows:groups[key]=groups.get(key,0)+value
            result=','.join(f'{k}:{(v+2**31)%2**32-2**31}' for k,v in sorted(groups.items())) or '-'
        else:result=','.join(map(str,sorted({v for _,v in rows}))) or '-'
        out.append(f'{c["id"]}\t{state}\t{result}')
    emit()
    for kind,slot,item in c['operations']:
        old=slots.get(slot,-1)
        for new in ([item,old] if kind=='t' else [item]):
            state+=1;prior=slots.get(slot,-1)
            if c['mode']=='sum':
                try:
                    a=c['items'][prior][1] if prior>=0 else 0;b=c['items'][new][1] if new>=0 else 0
                    if prior>=0 and new>=0:total=checked(total+checked(b-a))
                    elif prior>=0:total=checked(total-a)
                    elif new>=0:total=checked(total+b)
                except OverflowError:
                    out.append(f'{c["id"]}\t{state}\tOVERFLOW');return out
            if new>=0:slots[slot]=new
            else:slots.pop(slot,None)
            if c['mode']=='sum' and total!=sum(c['items'][i][1] for i in slots.values()):raise ValueError('Sum disagrees with full rebuild')
            emit()
    return out

def generate():
    rng=random.Random(SEED);cases=[]
    for mode in ['set','sorted','map','sum']:
        cases.append(dict(id=f'{mode}-empty',mode=mode,items=[[0,0]],operations=[]))
        for i in range(64):
            edge=[-2**31,-1,0,1,2**31-1] if mode=='map' else [LOW,LOW+1,-1,0,1,HIGH-1,HIGH]
            items=[[rng.randrange(-3,4),rng.choice(edge) if i%2 else rng.randrange(-30,31)] for _ in range(18)]
            ops=[('c',0,0),('c',1,0),('c',2,1)]+[('t' if rng.random()<.4 else 'c',rng.randrange(12),-1 if rng.random()<.25 else rng.randrange(len(items))) for _ in range(50)]+[('c',j,-1) for j in range(12)]
            cases.append(dict(id=f'{mode}-{i:02}',mode=mode,items=items,operations=ops))
    items=[[0,v] for v in [-1,HIGH,1,LOW,-2,0,2]];prefix=[('c',0,0),('c',1,1),('c',2,2)]
    boundaries=[[('c',0,1),('c',1,2)],[('c',0,3),('c',1,0)],[('c',0,3),('c',0,1)],prefix+[('c',0,-1)],prefix+[('c',0,0)],prefix+[('c',0,4)],prefix+[('c',2,6)],[('c',0,3),('c',0,-1)],prefix+[('t',1,3)],[('c',0,1),('c',0,3)]]
    for i,ops in enumerate(boundaries):cases.append(dict(id=f'sum-boundary-{i}',mode='sum',items=items,operations=ops))
    return cases

def validate(cases,lines):
    ref=[r for c in cases for r in expected(c)]
    if lines!=ref:
        mismatch=next(((i,a,b) for i,(a,b) in enumerate(zip(lines,ref)) if a!=b),None)
        raise ValueError(f'Set/sum response mismatch; rows {len(lines)}/{len(ref)}; first {mismatch}')
    return dict(observations=len(ref),expected_overflow_rejections=sum(r.endswith('\tOVERFLOW') for r in ref))

def run_checks(classpath):
    cases=generate();payload=''.join(c['id']+'\t'+c['mode']+'\t'+';'.join(','.join(map(str,r)) for r in c['items'])+'\t'+';'.join(':'.join(map(str,o)) for o in c['operations'])+'\n' for c in cases);outputs={}
    for runtime,command in {'java':['java','-cp',classpath,'SetSumCollectorOracle'],'rust':[str(ROOT/'target/release/examples/set_sum_oracle')]}.items():
        run=subprocess.run(command,input=payload,text=True,capture_output=True,check=True,timeout=120);outputs[runtime]=run.stdout.splitlines();stats=validate(cases,outputs[runtime])
    return dict(seed=SEED,total=len(cases),passed=len(cases),**stats,cases=cases,outputs=outputs,scope='Standalone sets, naturally sorted sets, sorted maps merged with wrapping i32 addition and checked i64 sums. Independent full multisets verify every successful state. Sum replacement checks the difference then total; expected overflow terminates each case and is not a successful update or Java post-error recovery claim. Set outputs normalize order; sorted map key order is compared. No Java ABI, null-key/value, general merger-function, timing or generated-case upstream-method credit.')
