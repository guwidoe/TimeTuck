"""Original exact result checks for public collector composition and finish mapping."""
import random
import struct
import subprocess
from differential_scores import ROOT
SEED=20261226

def states(case):
    slots={}
    yield dict(slots)
    for kind,slot,item in case['operations']:
        old=slots.get(slot,-1)
        for new in ([item,old] if kind=='t' else [item]):
            if new>=0:slots[slot]=new
            else:slots.pop(slot,None)
            yield dict(slots)

def expected(case):
    out=[]
    for state,slots in enumerate(states(case)):
        values=[case['items'][i] for i in slots.values()];n=len(values)
        if case['mode']=='then':result=str(n*10)
        else:
            result=f'{min(values)},{max(values)}' if n else '-,-'
            if case['mode'] in ['three','four']:
                bits=struct.unpack('>Q',struct.pack('>d',float(sum(values))/float(n)))[0] if n else '-'
                result+=f',{bits}'
            if case['mode']=='four':result=f'{n},'+result
        out.append(f'{case["id"]}\t{state}\t{result}')
    return out

def generate():
    rng=random.Random(SEED);cases=[]
    for mode in ['two','three','four','then']:
        cases.append(dict(id=f'{mode}-empty',mode=mode,items=[0],operations=[]))
        for i in range(32):
            items=[rng.randrange(-1000000,1000001) for _ in range(18)]
            items[:4]=[0,-1,1,1]
            ops=[('c',0,1),('c',1,1),('c',2,2),('t',0,3)]
            ops += [('t' if rng.random()<.4 else 'c',rng.randrange(12),-1 if rng.random()<.25 else rng.randrange(len(items))) for _ in range(50)]
            ops += [('c',j,-1) for j in range(12)]
            cases.append(dict(id=f'{mode}-{i:02}',mode=mode,items=items,operations=ops))
    return cases

def validate(cases,lines):
    reference=[r for c in cases for r in expected(c)]
    if lines!=reference:raise ValueError('Composition response does not match all independent observations')
    return len(reference)

def run_checks(classpath):
    cases=generate();payload=''.join(c['id']+'\t'+c['mode']+'\t'+','.join(map(str,c['items']))+'\t'+';'.join(':'.join(map(str,o)) for o in c['operations'])+'\n' for c in cases);outputs={}
    for runtime,command in {'java':['java','-cp',classpath,'CompositionCollectorOracle'],'rust':[str(ROOT/'target/release/examples/composition_oracle')]}.items():
        run=subprocess.run(command,input=payload,text=True,capture_output=True,check=True,timeout=120);outputs[runtime]=run.stdout.splitlines();observations=validate(cases,outputs[runtime])
    return dict(seed=SEED,total=len(cases),passed=len(cases),observations=observations,cases=cases,outputs=outputs,scope='Public composition of two, three or four collectors and count finish mapping. Exact min/max/count and average f64 bits match independent reconstruction through duplicate values, replacements, removals and inverse-update trials. Bounded integer sums/differences are representable. Finisher callback counts/order, mutable aliases, Java ABI and timing are not compared; generated cases add no upstream-method credit.')
