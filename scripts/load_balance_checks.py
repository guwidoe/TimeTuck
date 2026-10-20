"""Original exact controls and explicit wrapping-moment public comparisons."""
import random
import subprocess
from decimal import Decimal, localcontext, ROUND_HALF_EVEN
from differential_scores import ROOT
from sequence_collector_checks import states
from load_balance_binary64_reference import reference as binary64_reference
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


WRAPPING_POLICY = 'wrapping64-moments-exact-root'
BINARY64_POLICY = 'wrapping64-moments-binary64-root'
EXACT_POLICY = 'exact'


def wrapping_i64(value):
    return (value + 2**63) % 2**64 - 2**63


def wrapping_reference(case, slots):
    loads={};initials={}
    for i in slots.values():
        key,load,initial=case['items'][i]
        if key in initials and initials[key]!=initial:raise ValueError('Conflicting initial loads')
        initials[key]=initial
        loads[key]=wrapping_i64(loads.get(key,initial)+load)
    values=list(loads.values());n=len(values)
    total=wrapping_i64(sum(values))
    squares=wrapping_i64(sum(wrapping_i64(value*value) for value in values))
    squared_total=wrapping_i64(total*total)
    # Only moments wrap. Widen the numerator before multiplying/subtracting.
    numerator=n*squares-squared_total
    if numerator<0:return dict(error='NegativeRadicand')
    unfairness=[0,0]
    if numerator:
        with localcontext() as ctx:
            ctx.prec=120
            root=(Decimal(numerator)/n).sqrt()
            ctx.prec=6;ctx.rounding=ROUND_HALF_EVEN
            rounded=+root;t=rounded.as_tuple()
            unfairness=canonical(int(''.join(map(str,t.digits))),-t.exponent)
    return dict(loads=[[k,v] for k,v in sorted(loads.items())],unfairness=unfairness)


def selected_reference(arithmetic_policy):
    if arithmetic_policy==EXACT_POLICY:return reference
    if arithmetic_policy==WRAPPING_POLICY:return wrapping_reference
    if arithmetic_policy==BINARY64_POLICY:return binary64_reference
    raise ValueError('Unknown load balance arithmetic policy')


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


def validate(cases,lines,java=False,arithmetic_policy=EXACT_POLICY):
    reference_fn=selected_reference(arithmetic_policy)
    cursor=0;observations=0;passed=0;errors=[];differences=[];policy_errors=[]
    for case in cases:
        ok=True
        for state,slots in enumerate(states(case)):
            if cursor>=len(lines):raise ValueError('Missing load balance response')
            f=lines[cursor].split('\t')
            if len(f)==3 and f[:2]==[case['id'],'ERROR']:
                if not java or not f[2]:raise ValueError('Unexpected native/error response')
                errors.append(dict(case=case['id'],state=state,error=f[2]));cursor+=1;ok=False;break
            expected=reference_fn(case,slots)
            if len(f)==4 and f[:2]==[case['id'],str(state)] and f[2]=='ERROR':
                if java or arithmetic_policy not in (WRAPPING_POLICY,BINARY64_POLICY) or f[3]!='NegativeRadicand' or expected!=dict(error=f[3]):
                    raise ValueError('Unexpected load balance typed arithmetic error')
                actual=dict(error=f[3])
                policy_errors.append(dict(case=case['id'],state=state,error=f[3]))
            else:
                actual=parse(case,state,lines[cursor])
            cursor+=1;observations+=1
            if actual!=expected:
                if not java:raise ValueError(f'Native load balance differs: {case["id"]} state {state}: {actual} != {expected}')
                differences.append(dict(case=case['id'],state=state,actual=actual,expected=expected));ok=False
        passed+=ok
    if cursor!=len(lines):raise ValueError('Extra/reordered load balance responses')
    return dict(total=len(cases),passed=passed,observations=observations,errors=errors,differences=differences,
                divergent_cases=len({d['case'] for d in differences}),error_cases=len(errors),policy_errors=policy_errors)


def comparison_report(cases,outputs):
    """Derive all three policy counts from actual full observations, never constants."""
    exact_java=validate(cases,outputs['java'],java=True)
    exact_native=validate(cases,outputs['rust_exact'])
    exact_root_java=validate(cases,outputs['java'],java=True,arithmetic_policy=WRAPPING_POLICY)
    exact_root_native=validate(cases,outputs['rust_exact_root'],arithmetic_policy=WRAPPING_POLICY)
    java=validate(cases,outputs['java'],java=True,arithmetic_policy=BINARY64_POLICY)
    native=validate(cases,outputs['rust'],arithmetic_policy=BINARY64_POLICY)
    ordinary={c['id'] for c in cases if not c['boundary']}
    for result in [java,exact_java,exact_root_java]:
        if any(d['case'] in ordinary for d in result['errors']+result['differences']):
            raise ValueError('Bounded load balance comparison failed')
    def summary(java,native,policy):
        return dict(arithmetic_policy=policy,total=len(cases),passed=java['passed'],
                    observations=native['observations'],native_reference_passed=native['passed'],
                    java_observations=java['observations'],ordinary_total=len(ordinary),
                    ordinary_passed=len(ordinary),boundary_total=len(cases)-len(ordinary),
                    divergent_cases=java['divergent_cases'],java_error_cases=java['error_cases'],
                    differences=java['differences'],errors=java['errors'],native_policy_errors=native['policy_errors'])
    exact_controls=summary(exact_java,exact_native,EXACT_POLICY)
    exact_controls.update(results=dict(java=exact_java,rust=exact_native),
                          outputs=dict(java=outputs['java'],rust_exact=outputs['rust_exact']))
    exact_root_controls=summary(exact_root_java,exact_root_native,WRAPPING_POLICY)
    exact_root_controls.update(results=dict(java=exact_root_java,rust=exact_root_native),
                               outputs=dict(java=outputs['java'],rust_exact_root=outputs['rust_exact_root']))
    report=summary(java,native,BINARY64_POLICY)
    report.update(seed=SEED,cases=cases,outputs=outputs,results=dict(java=java,rust=native),
                  exact_controls=exact_controls,exact_root_controls=exact_root_controls,
                  scope='Public add/replace/remove and inverse-update trial/undo on all original full inputs. '
                  'Selected arithmetic_policy=wrapping64-moments-binary64-root uses wrapped signed i64 loads and moments; '
                  'it separately converts Q, wrapped S*S and unsigned key count to binary64, divides, subtracts and takes a binary64 root, '
                  'then rounds that exact binary root value to six significant decimal digits with half-even ties. '
                  'An independent Fraction/integer midpoint model checks each binary64 operation without float/libm sqrt. '
                  'Exact-default and wrapping-exact-root controls retain complete actual results, outputs and differences separately; defaults remain unchanged. '
                  'Java runs once on the identical unchanged payload; the same actual Java output is compared independently under all three policies. '
                  'Native typed NegativeRadicand errors are accepted only at independently predicted states, including all later recovery states. '
                  'Java errors and differences receive no agreement credit. Native error behavior, cancellation behavior and root-conversion order '
                  'define an explicit policy, not general Java formula, nonfinite or decimal-scale equivalence. '
                  'Trailing-zero representations normalize numerically. Initial loads must be consistent per active key; zero-load keys participate. '
                  'Full original inputs, states, outputs, settings and observed Java error prefixes remain retained. '
                  'Owned snapshots replace Java live maps. No timing or extra upstream-method credit.')
    return report


def run_checks(classpath):
    cases=generate();payload=''.join(c['id']+'\t'+';'.join(','.join(map(str,r)) for r in c['items'])+'\t'+
                                   ';'.join(':'.join(map(str,o)) for o in c['operations'])+'\n' for c in cases)
    executable=str(ROOT/'target/release/examples/load_balance_oracle')
    commands={'java':['java','-cp',classpath,'LoadBalanceOracle'],
              'rust_exact':[executable], 'rust_exact_root':[executable,'--wrapping64-moments'],
              'rust':[executable,'--wrapping64-moments-binary64-root']}
    outputs={}
    for runtime,command in commands.items():
        result=subprocess.run(command,input=payload,text=True,capture_output=True,check=True,timeout=120)
        outputs[runtime]=result.stdout.splitlines()
    return comparison_report(cases,outputs)
