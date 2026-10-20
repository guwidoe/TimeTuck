"""Original full-field connected-range checks with explicit reference divergences."""
import random
import subprocess
from collections import Counter
from differential_scores import ROOT
from sequence_collector_checks import states
SEED = 20261222


def generate():
    rng = random.Random(SEED)
    cases = []
    for empty in [False, True]:
        for i in range(64):
            intervals = []
            for _ in range(12):
                a = rng.randrange(-8, 9)
                intervals.append([a, a+rng.randrange(0 if empty else 1, 6)])
            ops = [('c',0,0), ('c',1,0), ('c',2,1)]
            ops += [('t' if rng.random() < .4 else 'c', rng.randrange(10),
                     -1 if rng.random() < .25 else rng.randrange(12)) for _ in range(30)]
            ops += [('c',j,-1) for j in range(10)]
            cases.append(dict(id=f'range-{int(empty)}-{i:02}', boundary=True, intervals=intervals, operations=ops))
    # Keep all original shared-identity cases above, including their failures.
    # Add a separate cohort with at most one active occurrence per logical ID.
    for i in range(64):
        intervals=[]
        for _ in range(12):
            a=rng.randrange(-8,9);intervals.append([a,a+rng.randrange(1,6)])
        slots={};ops=[]
        for _ in range(40):
            slot=rng.randrange(10);kind='t' if rng.random()<.4 else 'c'
            choices=[j for j in range(12) if j not in {v for k,v in slots.items() if k!=slot}]
            item=-1 if rng.random()<.25 else rng.choice(choices)
            ops.append((kind,slot,item))
            if kind=='c':
                if item<0:slots.pop(slot,None)
                else:slots[slot]=item
        ops += [('c',j,-1) for j in range(10)]
        cases.append(dict(id=f'range-distinct-{i:02}',boundary=False,intervals=intervals,operations=ops))
    fixed = [([[1,1],[1,1]],[('c',0,0),('c',1,1)]),
             ([[0,2],[1,1]],[('c',0,0),('c',1,1),('c',1,-1)]),
             ([[0,2],[2,3],[2,2]],[('c',0,0),('c',1,1),('c',2,2)]),
             ([[-2**31,2**31-1]],[('c',0,0)]),
             ([[0,2]],[('c',0,0),('c',1,0),('c',1,-1),('c',0,-1)])]
    for i,(intervals,ops) in enumerate(fixed):
        cases.append(dict(id=f'range-boundary-{i}',boundary=True,intervals=intervals,operations=ops))
    return cases


def reference(case, slots, *, wrapping=False):
    def distance(a, b):
        value = b - a
        return (value + 2**31) % 2**32 - 2**31 if wrapping else value
    intervals = case['intervals']
    counts = Counter(slots.values())
    remaining = set(counts)
    groups = []
    while remaining:
        todo = [min(remaining, key=lambda i: (*intervals[i], i))]
        found = set()
        while todo:
            i = todo.pop()
            if i in found:
                continue
            found.add(i)
            a,b = intervals[i]
            todo += [j for j in remaining-found if a <= intervals[j][1] and intervals[j][0] <= b]
        remaining -= found
        ids = sorted((i for i in found for _ in range(counts[i])), key=lambda i: (*intervals[i], i))
        points = sorted({p for i in found for p in intervals[i]})
        active = [sum(counts[i] for i in found if 2*intervals[i][0] <= a+b < 2*intervals[i][1])
                  for a,b in zip(points, points[1:])]
        low, high = (min(active),max(active)) if active else (None,None)
        groups.append(dict(start=points[0],end=points[-1],length=distance(points[0],points[-1]),count=len(ids),
                           minimum=low,maximum=high,overlap=high is not None and high > 1,items=sorted(ids)))
    gaps = [[a['end'],b['start'],distance(a['end'],b['start'])] for a,b in zip(groups,groups[1:])]
    return dict(ranges=groups,gaps=gaps)


def parse(case, state, line, java):
    f = line.split('\t')
    if len(f)!=4 or f[:2]!=[case['id'],str(state)]:
        raise ValueError('Range response identity/shape')
    ranges = []
    for raw in ([] if f[2]=='-' else f[2].split(';')):
        parts = raw.split(':')
        if len(parts)!=2:
            raise ValueError('Range item fields')
        meta = parts[0].split(',')
        if len(meta)!=7 or meta[6] not in ('0','1'):
            raise ValueError('Range metadata fields')
        start,end,length,count = map(int,meta[:4])
        low,high = [None if v=='-' else int(v) for v in meta[4:6]]
        if java and start==end and (low,high)==(2**31-1,-2**31):
            low=high=None  # No positive-length segment: explicit native Option mapping.
        ids = list(map(int,parts[1].split(',')))
        if any(not 0<=i<len(case['intervals']) for i in ids):
            raise ValueError('Unknown range identity')
        if not java and ids != sorted(ids,key=lambda i:(*case['intervals'][i],i)):
            raise ValueError('Native range iteration order')
        ranges.append(dict(start=start,end=end,length=length,count=count,minimum=low,maximum=high,
                           overlap=meta[6]=='1',items=sorted(ids)))
    gaps = []
    for raw in ([] if f[3]=='-' else f[3].split(';')):
        v=list(map(int,raw.split(',')))
        if len(v)!=3:
            raise ValueError('Gap fields')
        gaps.append(v)
    return dict(ranges=ranges,gaps=gaps)


def validate(cases, lines, java=False, *, wrapping=False):
    cursor=0; observations=0; passed=0; errors=[]; differences=[]
    for case in cases:
        case_passed=True
        for state,slots in enumerate(states(case)):
            if cursor>=len(lines):raise ValueError('Missing range response')
            f=lines[cursor].split('\t')
            if len(f)==3 and f[:2]==[case['id'],'ERROR']:
                if not java or not f[2]:raise ValueError('Unexpected native/error response')
                errors.append(dict(case=case['id'],state=state,error=f[2]));cursor+=1;case_passed=False;break
            actual=parse(case,state,lines[cursor],java);expected=reference(case,slots,wrapping=wrapping)
            cursor+=1;observations+=1
            if actual!=expected:
                if not java:raise ValueError(f'Native range differs: {case["id"]} state {state}')
                differences.append(dict(case=case['id'],state=state,expected=expected,actual=actual));case_passed=False
        passed+=case_passed
    if cursor!=len(lines):raise ValueError('Extra/reordered range responses')
    return dict(total=len(cases),passed=passed,observations=observations,errors=errors,differences=differences,
                divergent_cases=len({d['case'] for d in differences}),error_cases=len(errors))


def run_checks(classpath):
    cases=generate()
    payload=''.join(c['id']+'\t'+';'.join(','.join(map(str,p)) for p in c['intervals'])+'\t'+
                    ';'.join(':'.join(map(str,o)) for o in c['operations'])+'\n' for c in cases)
    outputs={};results={}
    for runtime,command in {'java':['java','-cp',classpath,'RangeCollectorOracle'],
                            'rust_exact':[str(ROOT/'target/release/examples/range_oracle')],
                            'rust':[str(ROOT/'target/release/examples/range_oracle'),'--wrapping32']}.items():
        r=subprocess.run(command,input=payload,text=True,capture_output=True,check=True,timeout=120)
        outputs[runtime]=r.stdout.splitlines()
        results[runtime]=validate(cases,outputs[runtime],java=runtime=='java',wrapping=runtime=='rust')
    ordinary={c['id'] for c in cases if not c['boundary']}
    exact=results['java'];java=validate(cases,outputs['java'],java=True,wrapping=True)
    for result in [exact,java]:
        if any(d['case'] in ordinary for d in result['errors']+result['differences']):
            raise ValueError('Distinct positive-length range comparison failed')
    previous={d['case'] for d in exact['differences']};selected={d['case'] for d in java['differences']}
    if previous-selected!={'range-boundary-3'} or selected-previous:
        raise ValueError('Explicit wrapping policy must close only the original length-overflow difference')
    return dict(seed=SEED,total=len(cases),passed=java['passed'],observations=results['rust']['observations'],
                native_reference_passed=results['rust']['passed'],java_observations=java['observations'],
                ordinary_total=len(ordinary),ordinary_passed=len(ordinary),boundary_total=len(cases)-len(ordinary),
                divergent_cases=java['divergent_cases'],java_error_cases=java['error_cases'],
                differences=java['differences'],errors=java['errors'],cases=cases,outputs=outputs,
                arithmetic_policy='wrapping32',
                exact_controls=dict(total=len(cases),passed=exact['passed'],
                    native_reference_passed=results['rust_exact']['passed'],
                    observations=results['rust_exact']['observations'],
                    divergent_cases=exact['divergent_cases'],java_error_cases=exact['error_cases'],
                    differences=exact['differences'],errors=exact['errors'],
                    output_keys=dict(java='java',rust='rust_exact')),
                scope='Explicit WrappingIntegerRangeMetric with signed i32 endpoints is selected for the Community comparison; '
                'default exact i64 results and their complete original difference cohort are retained under exact_controls and outputs.rust_exact. '
                'Public add/replace/remove and inverse-update trial/undo check every membership occurrence, '
                'component endpoint/length/count/overlap bound/flag and gap endpoint/length against independent '
                'Python pairwise connectivity and midpoint counts, with modular 32-bit lengths for the selected policy. '
                'Native stable start/end/ID order is checked separately; Java item order is not compared. '
                'Java zero-length MAX/MIN overlap sentinels normalize to native None. '
                'Shared-identity removal failures and other zero-length overlap flags/count differences remain explicit divergences, '
                'never agreement credit; runtime errors retain case identity and the observed prefix. Positive-length bounded '
                'cases with distinct active identities must all agree. Native snapshots are owned; Java live object/hash mutation is not claimed. '
                'No timing or extra upstream-method credit.')
