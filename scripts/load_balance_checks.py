"""Original exact load-map and unfairness comparisons through public collectors."""
import random
import subprocess
from decimal import Decimal, localcontext, ROUND_HALF_EVEN
from differential_scores import ROOT
from sequence_collector_checks import states
SEED = 20261223


def generate():
    rng=random.Random(SEED); cases=[]
    for i in range(96):
        initial=[rng.randrange(-20,21) for _ in range(6)]
        items=[[k,rng.randrange(-30,31),initial[k]] for k in range(6) for _ in range(3)]
        ops=[('c',0,0),('c',1,0),('c',2,3)]
        ops += [('t' if rng.random()<.4 else 'c',rng.randrange(12),-1 if rng.random()<.25 else rng.randrange(len(items))) for _ in range(50)]
        ops += [('c',j,-1) for j in range(12)]
        cases.append(dict(id=f'balance-{i:02}',boundary=False,items=items,operations=ops))
    fixed=[[[0,10**9,0],[1,10**9+1,0]],[[0,2**63-1,0],[1,0,0]],
           [[0,2**63-1,0],[1,2**63-2,0]],[[0,2**63-1,2**63-1],[1,-2**63,-2**63]],
           [[0,0,0],[1,10**6,0],[2,10**6,0],[3,0,0]]]
    for i,items in enumerate(fixed):
        ops=[('c',j,j) for j in range(len(items))]+[('t',0,1),('c',8,0),('c',8,-1)]+[('c',j,-1) for j in range(len(items))]
        cases.append(dict(id=f'balance-boundary-{i}',boundary=True,items=items,operations=ops))
    return cases


def canonical(coefficient,scale):
    if coefficient==0:return [0,0]
    while coefficient%10==0:coefficient//=10;scale-=1
    return [coefficient,scale]


def reference(case,slots):
    loads={};initials={}
    for i in slots.values():
        k,load,initial=case['items'][i]
        if k in initials and initials[k]!=initial:raise ValueError('Conflicting initial loads')
        initials[k]=initial;loads[k]=loads.get(k,initial)+load
    values=list(loads.values())
    # Independent pairwise distances: sum_{i<j}(load_i-load_j)^2 / item count.
    numerator=sum((a-b)**2 for i,a in enumerate(values) for b in values[:i])
    if not numerator:u=[0,0]
    else:
        # 120-digit decimal evaluation followed by six-digit rounding, separate
        # from the native integer-root and squared-midpoint algorithm.
        with localcontext() as ctx:
            ctx.prec=120
            root=(Decimal(numerator)/len(values)).sqrt()
            ctx.prec=6;ctx.rounding=ROUND_HALF_EVEN
            rounded=+root
            t=rounded.as_tuple()
            u=canonical(int(''.join(map(str,t.digits))),-t.exponent)
    return dict(loads=[[k,v] for k,v in sorted(loads.items())],unfairness=u)


def parse(case,state,line):
    f=line.split('\t')
    if len(f)!=5 or f[:2]!=[case['id'],str(state)]:raise ValueError('Load balance response identity/shape')
    loads=[]
    for raw in ([] if f[2]=='-' else f[2].split(';')):
        v=list(map(int,raw.split(':')))
        if len(v)!=2:raise ValueError('Load balance map entry')
        loads.append(v)
    if len({k for k,v in loads})!=len(loads) or loads!=sorted(loads):raise ValueError('Load balance duplicate/unordered keys')
    if any(k not in {r[0] for r in case['items']} for k,v in loads):raise ValueError('Load balance unknown key')
    coefficient,scale=map(int,f[3:])
    if coefficient<0 or not -100<=scale<=100:raise ValueError('Load balance unfairness representation')
    return dict(loads=loads,unfairness=canonical(coefficient,scale))


def validate(cases,lines,java=False):
    cursor=0;observations=0;passed=0;errors=[];differences=[]
    for case in cases:
        ok=True
        for state,slots in enumerate(states(case)):
            if cursor>=len(lines):raise ValueError('Missing load balance response')
            f=lines[cursor].split('\t')
            if len(f)==3 and f[:2]==[case['id'],'ERROR']:
                if not java or not f[2]:raise ValueError('Unexpected native/error response')
                errors.append(dict(case=case['id'],state=state,error=f[2]));cursor+=1;ok=False;break
            actual=parse(case,state,lines[cursor]);expected=reference(case,slots);cursor+=1;observations+=1
            if actual!=expected:
                if not java:raise ValueError(f'Native load balance differs: {case["id"]} state {state}: {actual} != {expected}')
                differences.append(dict(case=case['id'],state=state,actual=actual,expected=expected));ok=False
        passed+=ok
    if cursor!=len(lines):raise ValueError('Extra/reordered load balance responses')
    return dict(total=len(cases),passed=passed,observations=observations,errors=errors,differences=differences,
                divergent_cases=len({d['case'] for d in differences}),error_cases=len(errors))


def run_checks(classpath):
    cases=generate();payload=''.join(c['id']+'\t'+';'.join(','.join(map(str,r)) for r in c['items'])+'\t'+
                                   ';'.join(':'.join(map(str,o)) for o in c['operations'])+'\n' for c in cases)
    outputs={};results={}
    for runtime,command in {'java':['java','-cp',classpath,'LoadBalanceOracle'],
                            'rust':[str(ROOT/'target/release/examples/load_balance_oracle')]}.items():
        r=subprocess.run(command,input=payload,text=True,capture_output=True,check=True,timeout=120)
        outputs[runtime]=r.stdout.splitlines();results[runtime]=validate(cases,outputs[runtime],java=runtime=='java')
    java=results['java'];ordinary={c['id'] for c in cases if not c['boundary']}
    if any(d['case'] in ordinary for d in java['errors']+java['differences']):raise ValueError('Bounded load balance comparison failed')
    return dict(seed=SEED,total=len(cases),passed=java['passed'],observations=results['rust']['observations'],
                native_reference_passed=results['rust']['passed'],java_observations=java['observations'],
                ordinary_total=len(ordinary),ordinary_passed=len(ordinary),boundary_total=len(cases)-len(ordinary),
                divergent_cases=java['divergent_cases'],java_error_cases=java['error_cases'],differences=java['differences'],errors=java['errors'],cases=cases,outputs=outputs,
                scope='Public add/replace/remove and inverse-update trial/undo; every active key, exact total load and six-significant-digit unfairness checked against independent Python pairwise distances and high-precision Decimal square root. '
                'Trailing-zero decimal representations normalize numerically; Java numerical scale is not claimed. Native totals/moments are exact, avoiding Java overflow and floating cancellation. '
                'All differing/error cases and observed prefixes are retained. Initial load is constant per active key; inconsistent initial loads reject atomically in Rust and are outside paired inputs. '
                'Only present keys participate, including zero loads. Owned snapshots replace Java live maps. No timing or extra upstream-method credit.')
