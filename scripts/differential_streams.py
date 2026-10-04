#!/usr/bin/env python3
"""Compare Community full score updates with native transactional stream edits."""
import itertools
import json
import platform
import random
import subprocess
from collections import defaultdict
from datetime import datetime, timezone
from differential_scores import ROOT, REF, source_digest
from differential_solver import generate as basic_cases, encode as encode_basic
from prepare_solver_oracle import prepare_solver_java
from membership_join_checks import run_checks as check_memberships
from conditional_collector_checks import run_checks as check_conditionals
from average_collector_checks import run_checks as check_averages
from range_collector_checks import run_checks as check_ranges
from sequence_collector_checks import run_checks as check_sequences
from duration_collector_checks import run_checks as check_durations
from big_number_collector_checks import run_checks as check_big_numbers
from mixed_stream_checks import run_checks as check_mixed_streams
from follower_checks import run_checks as check_followers
from collection_follower_checks import run_checks as check_collection_followers
from list_element_checks import run_checks as check_list_elements
from concurrent_checks import run_checks as check_concurrent
from equality_existence_checks import run_checks as check_equality_existence
from ordered_join_checks import run_checks as check_ordered_joins

SEED = 20261017

def score(case, values):
    hard = -sum(values[a] is not None and values[a] == values[b] for a,b in case['edges'])
    groups = defaultdict(int)
    for i,v in enumerate(values):
        if v is not None: groups[v] += case['weights'][i][v]
    return hard, sum(s*abs(s) for s in groups.values())

def optimum(case):
    domains = [[v] if pin else range(case['value_count']) for v,pin in zip(case['initial'],case['pins'])]
    return max(score(case, values) for values in itertools.product(*domains))

def generate():
    rng = random.Random(SEED)
    cases = basic_cases()
    for i in range(64):
        n,k = rng.randint(1,7),rng.randint(1,4)
        cases.append({'id':f'stream-{i:02}', 'value_count':k,
                      'initial':[None if i%4==0 or rng.random()<.3 else rng.randrange(k) for _ in range(n)],
                      'pins':[False]*n,
                      'edges':[(rng.randrange(n),rng.randrange(n)) for _ in range(rng.randint(0,2*n))],
                      'weights':[[0 if i%8==0 else rng.randint(-9,9) for _ in range(k)] for _ in range(n)]})
    for case in cases:
        movable=[i for i,pin in enumerate(case['pins']) if not pin]
        case['operations']=[{'kind':'t' if rng.random()<.4 else 'c','variable':rng.choice(movable),
                             'value':None if rng.random()<.25 else rng.randrange(case['value_count'])}
                            for _ in range(16)] if movable else []
    return cases

def encode(case):
    return encode_basic(case)+'\t'+';'.join(f'{op["kind"]}:{op["variable"]}:{"-" if op["value"] is None else op["value"]}' for op in case['operations'])

def states(case):
    values=case['initial'][:]
    yield values[:]
    for op in case['operations']:
        i=op['variable']; old=values[i]; values[i]=op['value']
        yield values[:]
        if op['kind']=='t':
            values[i]=old
            yield values[:]

def expected_matches(case, values):
    edges=[i for i,(a,b) in enumerate(case['edges']) if values[a] is not None and values[a]==values[b]]
    groups=defaultdict(int)
    for i,v in enumerate(values):
        if v is not None: groups[v]+=case['weights'][i][v]
    return edges,sorted((k,v) for k,v in groups.items() if v!=0)

def validate(case, state, values, line, runtime):
    fields=line.split('\t')
    if len(fields)!=7 or fields[:2]!=[case['id'],str(state)]: raise ValueError('Stream response identity/field mismatch')
    if fields[2] not in ['0','1'] or (fields[2]=='1')!=all(v is not None for v in values): raise ValueError('Incorrect completeness')
    result=tuple(map(int,fields[3:5]))
    if result!=score(case,values): raise ValueError('Incorrect stream score')
    if runtime=='rust':
        edges=[] if not fields[5] else list(map(int,fields[5].split(',')))
        groups=[] if not fields[6] else [tuple(map(int,p.split(':'))) for p in fields[6].split(',')]
        if (edges,groups)!=expected_matches(case,values): raise ValueError('Incorrect native stream matches')
    elif runtime=='java':
        if fields[5:]!=['-','-']: raise ValueError('Unexpected Community match-analysis fields')
    else: raise ValueError('Unknown runtime')
    return {'score':list(result), 'complete':fields[2]=='1', 'line':line}

def prepare():
    cp,artifacts=prepare_solver_java()
    subprocess.run(['javac','-cp',cp,'-d',str(ROOT/'work/oracle/solver'),
                    str(ROOT/'compat/oracle/StreamOracle.java'),str(ROOT/'compat/oracle/StreamBenchmark.java'),
                    str(ROOT/'compat/oracle/AggregationBoundaryOracle.java'),str(ROOT/'compat/oracle/ConditionalCollectorOracle.java'),str(ROOT/'compat/oracle/MembershipJoinOracle.java'),str(ROOT/'compat/oracle/AverageCollectorOracle.java'),str(ROOT/'compat/oracle/DurationCollectorOracle.java'),str(ROOT/'compat/oracle/SequenceCollectorOracle.java'),str(ROOT/'compat/oracle/RangeCollectorOracle.java'),str(ROOT/'compat/oracle/BigNumberCollectorOracle.java'),str(ROOT/'compat/oracle/MixedStreamOracle.java'),str(ROOT/'compat/oracle/OrderedJoinOracle.java'),str(ROOT/'compat/oracle/ExistenceOracle.java')],check=True)
    subprocess.run(['cargo','build','--release','--example','membership_oracle','--example','conditional_oracle','--example','stream_oracle','--example','stream_benchmark','--example','average_oracle','--example','duration_oracle','--example','sequence_oracle','--example','range_oracle','--example','big_number_oracle','--example','mixed_stream_oracle','--example','ordered_join_oracle','--example','existence_oracle','--locked'],cwd=ROOT,check=True)
    return cp,artifacts

def main():
    cp,artifacts=prepare();digest=source_digest();cases=generate();payload='\n'.join(map(encode,cases))+'\n'
    subprocess.run(['java','-Xms256m','-Xmx256m','-cp',cp,'AggregationBoundaryOracle'],check=True,timeout=60)
    membership_checks=check_memberships(cp)
    print(f'Membership joins: {membership_checks["passed"]}/{membership_checks["total"]} cases; {membership_checks["observations"]} observations',flush=True)
    conditional_checks=check_conditionals(cp)
    print(f'Conditional collectors: {conditional_checks["passed"]}/{conditional_checks["total"]} cases; {conditional_checks["observations"]} observations',flush=True)
    average_checks=check_averages(cp)
    print(f'Average collectors: {average_checks["passed"]}/{average_checks["total"]} cases; {average_checks["observations"]} observations',flush=True)
    range_checks=check_ranges(cp)
    print(f'Connected ranges: {range_checks["passed"]}/{range_checks["total"]} Java agreement; {range_checks["divergent_cases"]} differing cases; {range_checks["native_reference_passed"]} native passes',flush=True)
    sequence_checks=check_sequences(cp)
    print(f'Sequence collectors: {sequence_checks["passed"]}/{sequence_checks["total"]} cases; {sequence_checks["observations"]} observations; {sequence_checks["boundary_divergences"]} explicit overflow divergence',flush=True)
    duration_checks=check_durations(cp)
    print(f'Duration collectors: {duration_checks["passed"]}/{duration_checks["total"]} cases; {duration_checks["observations"]} observations',flush=True)
    big_number_checks=check_big_numbers(cp)
    print(f'Big-number collectors: {big_number_checks["passed"]}/{big_number_checks["total"]} cases; {big_number_checks["observations"]} observations',flush=True)
    ordered_checks=check_ordered_joins(cp)
    print(f'Ordered joins: {ordered_checks["passed"]}/{ordered_checks["total"]} cases; {ordered_checks["observations"]} observations',flush=True)
    existence_checks=check_equality_existence(cp)
    print(f'Equality existence: {existence_checks["passed"]}/{existence_checks["total"]} cases; {existence_checks["observations"]} observations',flush=True)
    mixed_checks=check_mixed_streams(cp)
    print(f'Mixed streams: {mixed_checks["passed"]}/{mixed_checks["total"]} cases; {mixed_checks["observations"]} observations',flush=True)
    follower_checks=check_followers(cp)
    print(f'Follower shadows: {follower_checks["passed"]}/{follower_checks["total"]} cases; {follower_checks["observations"]} observations',flush=True)
    collection_follower_checks=check_collection_followers(cp)
    print(f'Collection follower shadows: {collection_follower_checks["passed"]}/{collection_follower_checks["total"]} cases; {collection_follower_checks["observations"]} observations',flush=True)
    list_element_checks=check_list_elements(cp)
    print(f'List element shadows: {list_element_checks["passed"]}/{list_element_checks["total"]} cases; {list_element_checks["observations"]} observations',flush=True)
    concurrent_checks=check_concurrent(cp)
    print(f'Concurrent shadows: {concurrent_checks["passed"]}/{concurrent_checks["total"]} agreement; {concurrent_checks["java_error_cases"]} Java error cases; {concurrent_checks["observations"]} native observations',flush=True)
    outputs={}
    for runtime,cmd in {'java':['java','-Xms256m','-Xmx256m','-cp',cp,'StreamOracle'],
                        'rust':[str(ROOT/'target/release/examples/stream_oracle')]}.items():
        run=subprocess.run(cmd,input=payload,text=True,capture_output=True,timeout=180)
        if run.returncode: raise RuntimeError(f'{runtime}: {run.stderr}')
        outputs[runtime]=run.stdout.splitlines()
    observations=[(case,i,values) for case in cases for i,values in enumerate(states(case))]
    if any(len(lines)!=len(observations) for lines in outputs.values()): raise ValueError('Stream response count mismatch')
    results=[]
    for index,(case,state,values) in enumerate(observations):
        results.append({'id':case['id'],'state':state,'values':values,
                        **{runtime:validate(case,state,values,lines[index],runtime) for runtime,lines in outputs.items()}})
    if digest!=source_digest(): raise RuntimeError('Sources changed during stream comparisons')
    report={'schema_version':1,'generated_at':datetime.now(timezone.utc).isoformat(),'source_digest':digest,
            'reference':REF,'artifacts':artifacts,'seed':SEED,'total':len(cases),'passed':len(cases),
            'observations':len(observations),'failures':[],'cases':cases,'results':results,'platform':platform.platform(),
            'membership_join_checks':membership_checks,
            'conditional_collector_checks':conditional_checks,
            'average_collector_checks':average_checks,
            'duration_collector_checks':duration_checks,
            'sequence_collector_checks':sequence_checks,
            'range_collector_checks':range_checks,
            'big_number_collector_checks':big_number_checks,
            'mixed_stream_checks':mixed_checks,
            'follower_checks':follower_checks,
            'collection_follower_checks':collection_follower_checks,
            'list_element_checks':list_element_checks,
            'concurrent_checks':concurrent_checks,
            'ordered_join_checks':ordered_checks,'equality_existence_checks':existence_checks,
            'aggregation_boundary_checks':{'passed':3,'total':3,
                'scope':'Separate Community full score updates for empty, one zero-valued fact, and removed-last-fact global count/sum groups. Native boundaries checked by collecting_stream_regression; no additional differential-case or upstream-method credit.'},
            'methodology':'Original required-variable cases, including partial assignments, duplicate/self edges, signed weights, zero/cancelling group sums, pins, committed and trial/undo edits. Constraints: indexed edge/job joins penalize equal endpoint values; nonzero group sums contribute sum*abs(sum) to soft score. Java Community SolutionManager.update rebuilds a score session for each observation; Rust uses persistent incremental transactions. Both ordinary scores checked against independent Python loops. Native exact conflict IDs and group sums also checked. Community update exposes no match-analysis API; no Java match-set, structural-validation or incremental-update timing claim. No additional upstream method coverage.'}
    (ROOT/'work/stream-differential.json').write_text(json.dumps(report,indent=2)+'\n')
    print(f'Stream differential: {len(cases)}/{len(cases)} cases; {len(observations)} observations',flush=True)

if __name__=='__main__':main()
