#!/usr/bin/env python3
"""Original bounded warm-start mixed depot/list workloads; not equal-work search."""
import argparse
from copy import deepcopy
from datetime import datetime, timezone
from itertools import permutations, product
import json
import platform
import statistics
import subprocess
from differential_scores import ROOT, REF, WORK
from prepare_solver_oracle import prepare_solver_java
from benchmark_scores import version
from verification_inputs import measurement_digest
from verification_cache import run_measurement

SEED=20261017

def score(case, depots, routes):
    hard=-sum(max(0,len(route)-cap) for route,cap in zip(routes,case['capacities']))
    distance=0
    for depot,route in zip(depots,routes):
        home=case['depots'][depot]
        tour=[home]+[case['points'][i] for i in route]+[home]
        distance+=sum(abs(a[0]-b[0])+abs(a[1]-b[1]) for a,b in zip(tour,tour[1:]))
    return hard,-distance

def validate_assignment(case, depots, routes):
    owners=len(case['capacities'])
    if len(depots)!=owners or len(routes)!=owners: raise ValueError('Owner shape')
    if any(type(d)!=int or not 0<=d<len(case['depots']) for d in depots): raise ValueError('Depot range')
    values=[v for route in routes for v in route]
    if any(type(v)!=int for v in values) or sorted(values)!=list(range(len(case['points']))): raise ValueError('Ownership')
    for owner,(before,after) in enumerate(zip(case['initial_routes'],routes)):
        prefix=case['prefixes'][owner]
        if before[:prefix]!=after[:prefix]: raise ValueError('Prefix pin')
        if case['pins'][owner] and (before!=after or depots[owner]!=case['initial_depots'][owner]): raise ValueError('Whole pin')
    return score(case,depots,routes)

def encode(case):
    xy=lambda rows: ','.join(f'{x}:{y}' for x,y in rows)
    return '\t'.join([case['id'],xy(case['points']),xy(case['depots']),','.join(map(str,case['capacities'])),','.join(map(str,case['initial_depots'])),
        ';'.join(','.join(map(str,r)) or '-' for r in case['initial_routes']),''.join('1' if v else '0' for v in case['pins']),','.join(map(str,case['prefixes'])),str(case['steps']),str(case['evaluations'])])

def optimum(case):
    """Independent exhaustive partition/permutation/depot product, small input only."""
    if len(case['points'])>6: raise ValueError('Independent exact bound')
    fixed=[route[:] if pin else route[:prefix] for route,pin,prefix in zip(case['initial_routes'],case['pins'],case['prefixes'])]
    assigned={v for route in fixed for v in route}
    movable=[v for v in range(len(case['points'])) if v not in assigned]
    owners=[i for i,pin in enumerate(case['pins']) if not pin]
    domains=[[initial] if pin else range(len(case['depots'])) for initial,pin in zip(case['initial_depots'],case['pins'])]
    best=None
    for placement in product(owners,repeat=len(movable)):
        tails=[[v for v,owner in zip(movable,placement) if owner==i] for i in range(len(fixed))]
        for orders in product(*(permutations(tail) for tail in tails)):
            routes=[head+list(tail) for head,tail in zip(fixed,orders)]
            for depots in product(*domains):
                candidate=score(case,depots,routes)
                best=candidate if best is None else max(best,candidate)
    if best is None: raise ValueError('No independent complete assignment')
    return best

def workloads():
    small={'id':'mixed-small','points':[(1,1),(2,0),(18,0),(19,1)],'depots':[(0,0),(20,0)],'capacities':[2,2],
        'initial_depots':[0,0],'initial_routes':[[2,0],[3,1]],'pins':[False,False],'prefixes':[0,0],'steps':16,'evaluations':10000,
        'certificate_depots':[0,1],'certificate_routes':[[0,1],[2,3]],'reference_kind':'independent_exhaustive_optimum','strict_improvement':True}
    homes=[(0,0),(30,0),(0,30),(30,30)];offsets=[(1,0),(2,1),(3,0),(1,2),(2,3),(3,2)]
    points=[(x+dx,y+dy) for x,y in homes for dx,dy in offsets]
    remaining=[v for v in range(6,24) if v not in [6,12,18]]
    large={'id':'mixed-clustered-pins','points':points,'depots':homes,'capacities':[6]*4,'initial_depots':[0,3,1,2],
        'initial_routes':[list(range(6))]+[[6*owner]+remaining[owner-1::3] for owner in range(1,4)],'pins':[True,False,False,False],
        'prefixes':[0,1,1,1],'steps':24,'evaluations':40000,'certificate_depots':[0,1,2,3],
        'certificate_routes':[list(range(6*i,6*i+6)) for i in range(4)],'reference_kind':'independently_scored_feasible_certificate','strict_improvement':False}
    for case in [small,large]:
        case['initial_score']=list(validate_assignment(case,case['initial_depots'],case['initial_routes']))
        case['reference_score']=list(validate_assignment(case,case['certificate_depots'],case['certificate_routes']))
        if case['initial_score'][0]!=0 or case['reference_score'][0]!=0: raise ValueError('Feasible input/certificate required')
        if case['reference_kind']=='independent_exhaustive_optimum' and tuple(case['reference_score'])!=optimum(case): raise ValueError('Certificate not exact optimum')
    return [small,large]

def parse_result(case, line, runtime, iterations):
    row=json.loads(line)
    if row['id']!=case['id'] or type(row['elapsed_ns'])!=int or row['elapsed_ns']<=0: raise ValueError('Identity/time')
    actual=validate_assignment(case,row['depots'],row['routes'])
    if row['score']!=[v*iterations for v in actual]: raise ValueError('Score checksum')
    if actual[0]!=0 or actual<tuple(case['initial_score']): raise ValueError('Quality floor')
    if case['strict_improvement'] and actual<=tuple(case['initial_score']): raise ValueError('Strict improvement')
    if case['reference_kind']=='independent_exhaustive_optimum' and actual>tuple(case['reference_score']): raise ValueError('Exceeds independent optimum')
    if type(row['score_calls'])!=int or row['score_calls']<iterations: raise ValueError('Actual callback count')
    if runtime=='java':
        if row['evaluations'] is not None or row['steps'] is not None: raise ValueError('Java logical work is not observed')
    else:
        if type(row['evaluations'])!=int or not iterations<=row['evaluations']<=iterations*case['evaluations']: raise ValueError('Native evaluations')
        if type(row['steps'])!=int or not 0<=row['steps']<=iterations*case['steps']: raise ValueError('Native steps')
    row['per_solve_score']=list(actual)
    return row

def measure_case(case,args,classpath,index):
    common=list(map(str,[args.iterations,args.warmup,args.samples]))
    commands={'java':['java','-Xms256m','-Xmx256m','-cp',classpath,'MixedSolverBenchmark',*common],
        **{backend:[str(ROOT/'target/release/examples/mixed_solver_benchmark'),*common,backend] for backend in ['dense','stream']}}
    measured={runtime:[] for runtime in commands}
    for fork in range(args.forks):
        order=['java','dense','stream'];offset=(index+fork)%3;order=order[offset:]+order[:offset]
        for runtime in order:
            run=subprocess.run(commands[runtime],input=encode(case)+'\n',text=True,capture_output=True,timeout=180)
            if run.returncode: raise RuntimeError(f'{runtime}: {run.stderr}')
            lines=run.stdout.splitlines()
            if len(lines)!=args.samples: raise RuntimeError('Unexpected sample count')
            measured[runtime].extend(parse_result(case,line,runtime,args.iterations) for line in lines)
    entries=[]
    for backend in ['dense','stream']:
        observations={'java':measured['java'],'rust':measured[backend]}
        samples={runtime:[row['elapsed_ns']/args.iterations/1e6 for row in rows] for runtime,rows in observations.items()}
        entry={'name':case['id']+' · '+backend,'algorithm':'mixed warm-start hill climbing','native_backend':backend,'unit':'ms / solve','samples':samples,'case':deepcopy(case),
            'java_sample_group':case['id'],'java_samples_shared_between_native_rows':True,
            'equal_scores':{tuple(row['per_solve_score']) for row in observations['java']}=={tuple(row['per_solve_score']) for row in observations['rust']},
            'quality_floor':case['initial_score'],'reference_kind':case['reference_kind'],'reference_score':case['reference_score'],
            'observations':observations}
        for runtime,values in samples.items():
            rows=observations[runtime]
            entry[runtime]={'median':statistics.median(values),'min':min(values),'max':max(values),'score':rows[0]['per_solve_score'],
                'score_calls':{'min':min(r['score_calls']/args.iterations for r in rows),'max':max(r['score_calls']/args.iterations for r in rows)}}
            if any(r['per_solve_score']!=rows[0]['per_solve_score'] for r in rows):raise RuntimeError('Fixed-seed score varied between repetitions')
            if runtime=='rust':
                entry[runtime]['constraint_calls']=entry[runtime]['score_calls'].copy()
                for field in ['evaluations','steps']:entry[runtime][field]={'min':min(r[field]/args.iterations for r in rows),'max':max(r[field]/args.iterations for r in rows)}
        if case['reference_kind']=='independent_exhaustive_optimum':entry['optimal_score']=case['reference_score'][:]

        entry['java_over_rust']=entry['java']['median']/entry['rust']['median'];entries.append(entry)
    return entries

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name,default in [('iterations',3),('warmup',3),('samples',5),('forks',3)]:parser.add_argument('--'+name,type=int,default=default)
    args=parser.parse_args()
    if min(args.iterations,args.samples,args.forks)<1 or args.warmup<0:parser.error('Positive counts; warmup may be zero')
    classpath,artifacts=prepare_solver_java()
    subprocess.run(['javac','-cp',classpath,'-d',str(WORK/'solver'),str(ROOT/'compat/oracle/MixedSolverBenchmark.java')],cwd=ROOT,check=True)
    subprocess.run(['cargo','build','--release','--locked','--example','mixed_solver_benchmark'],cwd=ROOT,check=True)
    digest=measurement_digest('mixed-performance');results=[]
    for index,case in enumerate(workloads()):results.extend(measure_case(case,args,classpath,index))
    if measurement_digest('mixed-performance')!=digest:raise RuntimeError('Benchmark inputs changed')
    cpu=next((line.split(':',1)[1].strip() for line in open('/proc/cpuinfo') if line.startswith('model name')),'unknown')
    report={'schema_version':1,'generated_at':datetime.now(timezone.utc).isoformat(),'source_digest':digest,'reference':REF,'artifacts':artifacts,'config':{**vars(args),'seed':SEED,'seed_scope':'Java seed; native Original order is deterministic without random draws','max_steps':24,'max_evaluations':40000,'warm_start':True,'native_joint_moves':True,'java_joint_moves':False},
        'workloads':results,'environment':{'cpu':cpu,'platform':platform.platform(),'rust':version(['rustc','--version']),'java':version(['java','-version'])},
        'methodology':'Original coupled depot/basic and customer/list Manhattan tours with capacity penalties, complete feasible warm starts, whole-owner and prefix pins. Small optimum independently exhaustively certified; larger reference is an independently scored feasible certificate, NOT an optimum or mandatory finite-budget target. Every repeated solve validates its output after timing; Python checks every observed checksum/assignment/pin/score and quality floor. Small unpinned case must strictly improve. Native Dense/Stream use Original-order BestAccepted basic/list plus joint moves with finite score/candidate/step ceilings; Java uses deterministic ORIGINAL union of basic changes, list relocation and swaps with public step/score-call termination, no joint-move or trace equivalence. Native actual evaluations/steps/callbacks and Java measured physical EasyScoreCalculator calls are different work metrics; Java logical steps/evaluations are unknown, never inferred. Timings include per-solve scoring/solver setup and search, exclude process startup, parsing, Java factory preparation and postsolve diagnostics. Native graph setup is included; Java input object copy is outside timing. Runtime order rotates Java/Dense/Stream by fork; a single measured Java sample group per case is reused in both native rows, not counted as independent extra runs. Quality and work must accompany speed ratios; these bounded warm-start workloads establish no cold construction, general mixed-solver scalability, memory or equal-work claim.'}
    (ROOT/'work/mixed-performance.json').write_text(json.dumps(report,indent=2)+'\n')

if __name__=='__main__':raise SystemExit(run_measurement('mixed-performance',main))
