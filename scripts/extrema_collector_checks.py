"""Original public key-projected extrema contract checks; ties allow any active item."""
import random
import subprocess
from differential_scores import ROOT
SEED=20261225


def states(case):
    slots={}
    yield dict(slots)
    for kind,slot,item in case['operations']:
        old=slots.get(slot,-1)
        for new in ([item,old] if kind=='t' else [item]):
            if new>=0:slots[slot]=new
            else:slots.pop(slot,None)
            yield dict(slots)


def allowed(case,slots):
    ids=set(slots.values())
    if not ids:return []
    key=(min if case['mode']=='min' else max)(case['items'][i][0] for i in ids)
    return sorted(i for i in ids if case['items'][i][0]==key)


def expected(case):
    out=[]
    for state,slots in enumerate(states(case)):
        ids=allowed(case,slots)
        value='-' if not ids else ','.join(map(str,[ids[0],*case['items'][ids[0]]]))
        out.append(f'{case["id"]}\t{state}\t{value}')
    return out


def validate(cases,lines,native=False):
    rows=[(c,s,slots) for c in cases for s,slots in enumerate(states(c))]
    if len(rows)!=len(lines):raise ValueError('Extrema observation count mismatch')
    for (c,s,slots),line in zip(rows,lines):
        fields=line.split('\t')
        if len(fields)!=3 or fields[:2]!=[c['id'],str(s)]:raise ValueError('Extrema identity mismatch')
        ids=allowed(c,slots)
        if not ids:
            if fields[2]!='-':raise ValueError('Nonempty extremum for empty state')
            continue
        try:result=list(map(int,fields[2].split(',')))
        except ValueError as e:raise ValueError('Invalid extremum fields') from e
        if len(result)!=3 or result[0] not in ids:raise ValueError('Inactive or non-extreme selected ID')
        if result[1:]!=c['items'][result[0]]:raise ValueError('Incorrect captured key/payload')
        if native and result[0]!=ids[0]:raise ValueError('Native tie rule mismatch')
    return len(rows)


def generate():
    cases=[];rng=random.Random(SEED)
    for mode in ['min','max']:
        cases.append(dict(id=f'{mode}-empty',mode=mode,items=[[0,0]],operations=[]))
        cases.append(dict(id=f'{mode}-ties',mode=mode,items=[[1,10],[1,20],[0,30]],operations=[('c',0,0),('c',1,1),('t',0,2),('c',0,-1),('c',1,-1)]))
        for i in range(96):
            keys=[-2**63,-2**63+1,-1,0,1,2**63-2,2**63-1] if i%2 else list(range(-3,4))
            items=[[rng.choice(keys),rng.randrange(-100,101)] for _ in range(18)]
            ops=[('c',0,0),('c',1,0),('c',2,1)]
            ops += [('t' if rng.random()<.4 else 'c',rng.randrange(12),-1 if rng.random()<.25 else rng.randrange(len(items))) for _ in range(50)]
            ops += [('c',j,-1) for j in range(12)]
            cases.append(dict(id=f'{mode}-{i:02}',mode=mode,items=items,operations=ops))
    return cases


def run_checks(classpath):
    cases=generate();payload=''.join(c['id']+'\t'+c['mode']+'\t'+';'.join(','.join(map(str,r)) for r in c['items'])+'\t'+';'.join(':'.join(map(str,o)) for o in c['operations'])+'\n' for c in cases)
    outputs={}
    for runtime,command in {'java':['java','-cp',classpath,'ExtremaCollectorOracle'],'rust':[str(ROOT/'target/release/examples/extrema_oracle')]}.items():
        run=subprocess.run(command,input=payload,text=True,capture_output=True,check=True,timeout=120)
        outputs[runtime]=run.stdout.splitlines();observations=validate(cases,outputs[runtime],runtime=='rust')
    differences=[dict(java=a,rust=b) for a,b in zip(outputs['java'],outputs['rust']) if a!=b]
    return dict(seed=SEED,total=len(cases),passed=len(cases),observations=observations,tie_choice_differences=len(differences),differences=differences,cases=cases,outputs=outputs,
                scope='Public key-projected min/max selection: each selected ID is active, carries its captured payload and has the independent extreme key. Java permits any tied value; native ties select lowest stable ID. Valid tie identity differences are retained, not claimed as exact identity agreement. Includes duplicate identities, replacements, removals and inverse-update trials. No Java ABI, callback ordering, timing or generated-case upstream-method credit.')
