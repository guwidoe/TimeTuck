"""Original component-wise calendar period checks through public collectors."""
import random
import subprocess
from differential_scores import ROOT
SEED=20261224
LOW,HIGH=-2**31,2**31-1


def arithmetic(a,b,subtract=False):
    out=[x-y if subtract else x+y for x,y in zip(a,b)]
    if any(not LOW<=x<=HIGH for x in out):raise OverflowError
    return out


def expected(case):
    total=[0,0,0];slots={};state=0;out=[f'{case["id"]}\t0\t0,0,0']
    for kind,slot,item in case['operations']:
        old=slots.get(slot,-1)
        for new in ([item,old] if kind=='t' else [item]):
            state+=1
            try:
                prior=slots.get(slot,-1)
                if prior>=0:total=arithmetic(total,case['items'][prior],True)
                if new>=0:total=arithmetic(total,case['items'][new])
            except OverflowError:
                out.append(f'{case["id"]}\t{state}\tOVERFLOW')
                return out
            if new>=0:slots[slot]=new
            else:slots.pop(slot,None)
            # Reconstruct every successful result with independent unbounded sums.
            rebuilt=[sum(case['items'][i][j] for i in slots.values()) for j in range(3)]
            if total!=rebuilt:raise ValueError('Period reference disagrees with full rebuild')
            out.append(f'{case["id"]}\t{state}\t'+','.join(map(str,total)))
    return out


def generate():
    cases=[dict(id='period-empty',items=[[0,0,0]],operations=[]),
           dict(id='period-mixed',items=[[1,12,-3],[-2,-1,4],[0,0,0]],operations=[('c',0,0),('c',1,1),('c',2,1),('t',0,2),('c',1,-1),('c',2,-1),('c',0,-1)])]
    for axis in range(3):
        def row(v):
            x=[0,0,0];x[axis]=v;return x
        items=[row(v) for v in [-1,HIGH,1,-1,-2,LOW,2]]
        prefix=[('c',0,0),('c',1,1),('c',2,2)]
        ops=[[('c',0,1),('c',1,2)], [('c',0,5),('c',1,0)],
             prefix+[('c',0,-1)],prefix+[('c',0,0)],prefix+[('c',0,3)],prefix+[('c',0,4)],
             [('c',0,5),('c',0,-1)], [('c',0,5),('c',0,1),('c',0,-1)],
             prefix+[('t',1,5)],prefix+[('t',0,4)],prefix+[('c',2,6)]]
        for i,operations in enumerate(ops):cases.append(dict(id=f'period-boundary-{axis}-{i}',items=items,operations=operations))
    rng=random.Random(SEED)
    for boundary in [False,True]:
        for i in range(96):
            pool=[LOW,LOW+1,-13,-1,0,1,13,HIGH-1,HIGH]
            items=[[rng.choice(pool) if boundary else rng.randrange(-30,31) for _ in range(3)] for _ in range(18)]
            ops=[('c',0,0),('c',1,0),('c',2,1)]
            ops += [('t' if rng.random()<.4 else 'c',rng.randrange(12),-1 if rng.random()<.25 else rng.randrange(len(items))) for _ in range(50)]
            ops += [('c',j,-1) for j in range(12)]
            cases.append(dict(id=f'period-{int(boundary)}-{i:02}',items=items,operations=ops))
    return cases


def validate(cases,lines):
    reference=[row for c in cases for row in expected(c)]
    if lines!=reference:
        mismatch=next(((i,a,b) for i,(a,b) in enumerate(zip(lines,reference)) if a!=b),None)
        raise ValueError(f'Period response mismatch; rows {len(lines)}/{len(reference)}; first {mismatch}')
    return dict(observations=len(reference),expected_overflow_rejections=sum(row.endswith('\tOVERFLOW') for row in reference))


def run_checks(classpath):
    cases=generate();payload=''.join(c['id']+'\t'+';'.join(','.join(map(str,r)) for r in c['items'])+'\t'+';'.join(':'.join(map(str,o)) for o in c['operations'])+'\n' for c in cases)
    outputs={};stats=None
    for runtime,command in {'java':['java','-cp',classpath,'PeriodCollectorOracle'],
                            'rust':[str(ROOT/'target/release/examples/period_oracle')]}.items():
        run=subprocess.run(command,input=payload,text=True,capture_output=True,check=True,timeout=120)
        outputs[runtime]=run.stdout.splitlines();stats=validate(cases,outputs[runtime])
    return dict(seed=SEED,total=len(cases),passed=len(cases),**stats,cases=cases,outputs=outputs,
                scope='Standalone public period collector add/replace/remove plus inverse-update trials. All signed year/month/day fields and checked i32 intermediate overflows match independent Python, with successful states also reconstructed from captured inputs. '
                'No normalization or fixed-duration conversion. Replacement subtracts then adds even for identical/equal periods. Expected overflow is agreement on rejection, not a successful update; both runtimes stop at the first error and no Java post-error state is claimed. '
                'Native atomic failure and real graph rollback are separately regression-tested. No date/time-zone application, Java ABI, timing or generated-case upstream-method credit.')
