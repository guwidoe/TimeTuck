#!/usr/bin/env python3
"""Original bounded warm/cold mixed depot/list workloads; not equal-work search."""
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

def partial_score(case, depots, routes):
    """Independent cold input score: missing depot/customer penalties precede distance."""
    missing_customers=len(case['points'])-sum(map(len,routes))
    hard=-missing_customers-sum(d is None for d in depots)
    hard-=sum(max(0,len(route)-cap) for route,cap in zip(routes,case['capacities']))
    distance=0
    for depot,route in zip(depots,routes):
        if depot is None:continue
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
    fields=[case['id'],xy(case['points']),xy(case['depots']),','.join(map(str,case['capacities'])),','.join(map(str,case['initial_depots'])),
        ';'.join(','.join(map(str,r)) or '-' for r in case['initial_routes']),''.join('1' if v else '0' for v in case['pins']),','.join(map(str,case['prefixes'])),str(case['steps']),str(case['evaluations'])]
    if case['start_mode']=='cold':fields.append(':'.join(map(str,case['quality_floor'])))
    return '\t'.join(fields)

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
        case['start_mode']='warm'
        case['quality_floor']=case['initial_score'][:]
    cold=[]
    for warm in [small,large]:
        case=deepcopy(warm);case['id']+='-cold';case['start_mode']='cold'
        case['warm_reference_score']=case['initial_score'][:]
        case['cold_initial_depots']=[d if pin else None for d,pin in zip(case['initial_depots'],case['pins'])]
        case['cold_initial_routes']=[route[:] if pin else route[:prefix] for route,pin,prefix in zip(case['initial_routes'],case['pins'],case['prefixes'])]
        case['initial_score']=list(partial_score(case,case['cold_initial_depots'],case['cold_initial_routes']))
        case['quality_floor']=case['reference_score'][:]
        case['strict_improvement']=False
        cold.append(case)
    return [small,large,*cold]

def cold_placements(case):
    return sum(d is None for d in case['cold_initial_depots'])+len(case['points'])-sum(map(len,case['cold_initial_routes']))

def parse_result(case, line, runtime, iterations, search_profile='joint'):
    if search_profile not in ['joint','component-refinement']:raise ValueError('Unknown search profile')
    row=json.loads(line)
    if row['id']!=case['id'] or type(row['elapsed_ns'])!=int or row['elapsed_ns']<=0: raise ValueError('Identity/time')
    actual=validate_assignment(case,row['depots'],row['routes'])
    if row['score']!=[v*iterations for v in actual]: raise ValueError('Score checksum')
    if actual[0]!=0 or actual<tuple(case['quality_floor']): raise ValueError('Quality floor')
    if case['strict_improvement'] and actual<=tuple(case['initial_score']): raise ValueError('Strict improvement')
    if case['reference_kind']=='independent_exhaustive_optimum' and actual>tuple(case['reference_score']): raise ValueError('Exceeds independent optimum')
    if type(row['score_calls'])!=int or row['score_calls']<iterations: raise ValueError('Actual callback count')
    cold=case['start_mode']=='cold'
    refinement=runtime!='java' and search_profile=='component-refinement'
    if refinement:
        if row.get('search_profile')!='component-refinement':raise ValueError('Native search profile')
        if row.get('complete') is not True:raise ValueError('Refinement incomplete result')
        phases=row.get('phase_evaluations');steps=row.get('phase_steps');terms=row.get('phase_terminations');placements=row.get('phase_construction_steps')
        maximum=3 if cold else 2
        if not isinstance(phases,list) or not 1<=len(phases)<=maximum or any(type(v)!=int or v<=0 for v in phases) or sum(phases)!=row['evaluations']:raise ValueError('Refinement phase evaluations')
        if not isinstance(steps,list) or len(steps)!=len(phases) or any(type(v)!=int or v<0 for v in steps) or sum(steps)!=row['steps']:raise ValueError('Refinement phase steps')
        if not isinstance(placements,list) or len(placements)!=len(phases) or any(type(v)!=int or v<0 for v in placements) or sum(placements)!=(row.get('construction_steps') if cold else 0):raise ValueError('Refinement phase construction steps')
        if not isinstance(terms,list) or len(terms)!=len(phases) or any(type(v)!=str for v in terms):raise ValueError('Refinement phase terminations')
        if not cold and 'construction_steps' in row:raise ValueError('Warm construction steps')
        if cold and (placements[0]!=row.get('construction_steps') or steps[0]!=0 or any(placements[1:]) or not terms[0].startswith('Construction(')):raise ValueError('Refinement construction phase')
        if any(not term.startswith('LocalSearch(') for term in terms[1 if cold else 0:]):raise ValueError('Refinement search phase')
        first=1 if cold else 0
        if len(phases)>first+1 and terms[first]!='LocalSearch(NoAcceptedMove)':raise ValueError('Joint refinement before component stall')
    elif any(k in row for k in ['search_profile','phase_steps','phase_construction_steps','phase_terminations']):raise ValueError('Unexpected strategy metadata')
    if not cold and not refinement and any(k in row for k in ['construction_steps','phase_evaluations','complete']):
        raise ValueError('Warm output shape changed')
    if runtime=='java':
        if cold and any(row.get(k) is not None for k in ['construction_steps','phase_evaluations']):
            raise ValueError('Java phase work is not observed')
        if row['evaluations'] is not None or row['steps'] is not None: raise ValueError('Java logical work is not observed')
    else:
        if type(row['evaluations'])!=int or not iterations<=row['evaluations']<=iterations*case['evaluations']: raise ValueError('Native evaluations')
        placement_count=cold_placements(case) if cold else 0
        if type(row['steps'])!=int or not 0<=row['steps']<=iterations*case['steps']: raise ValueError('Native steps')
        if cold:
            if row.get('complete') is not True:raise ValueError('Cold incomplete result')
            construction=row.get('construction_steps');phases=row.get('phase_evaluations')
            if type(construction)!=int or construction!=iterations*placement_count:raise ValueError('Cold construction steps')
            if not refinement and (not isinstance(phases,list) or len(phases)!=2 or any(type(v)!=int or v<=0 for v in phases) or sum(phases)!=row['evaluations']):
                raise ValueError('Cold phase evaluations')
    row['per_solve_score']=list(actual)
    return row

def measure_case(case,args,classpath,index):
    common=list(map(str,[args.iterations,args.warmup,args.samples]))
    commands={'java':['java','-Xms256m','-Xmx256m','-cp',classpath,'MixedSolverBenchmark',*common],
        **{backend:[str(ROOT/'target/release/examples/mixed_solver_benchmark'),*common,backend] for backend in ['dense','stream']}}
    if case['start_mode']=='cold':
        for command in commands.values():command.append('cold')
    profile=getattr(args,'native_search_profile','joint')
    if profile=='component-refinement':
        for backend in ['dense','stream']:commands[backend].append('component-refinement')
    measured={runtime:[] for runtime in commands}
    for fork in range(args.forks):
        order=['java','dense','stream'];offset=(index+fork)%3;order=order[offset:]+order[:offset]
        for runtime in order:
            run=subprocess.run(commands[runtime],input=encode(case)+'\n',text=True,capture_output=True,timeout=180)
            if run.returncode: raise RuntimeError(f'{runtime}: {run.stderr}')
            lines=run.stdout.splitlines()
            if len(lines)!=args.samples: raise RuntimeError('Unexpected sample count')
            measured[runtime].extend(parse_result(case,line,runtime,args.iterations,profile) for line in lines)
    entries=[]
    for backend in ['dense','stream']:
        observations={'java':measured['java'],'rust':measured[backend]}
        samples={runtime:[row['elapsed_ns']/args.iterations/1e6 for row in rows] for runtime,rows in observations.items()}
        entry={'name':case['id']+' · '+backend,'algorithm':('mixed cold construction + hill climbing' if case['start_mode']=='cold' else 'mixed warm-start hill climbing'),'start_mode':case['start_mode'],'native_backend':backend,'native_search_profile':profile,'unit':'ms / solve','samples':samples,'case':deepcopy(case),
            'java_sample_group':case['id'],'java_samples_shared_between_native_rows':True,
            'equal_scores':{tuple(row['per_solve_score']) for row in observations['java']}=={tuple(row['per_solve_score']) for row in observations['rust']},
            'quality_floor':case['quality_floor'],'reference_kind':case['reference_kind'],'reference_score':case['reference_score'],
            'observations':observations}
        for runtime,values in samples.items():
            rows=observations[runtime]
            entry[runtime]={'median':statistics.median(values),'min':min(values),'max':max(values),'score':rows[0]['per_solve_score'],
                'score_calls':{'min':min(r['score_calls']/args.iterations for r in rows),'max':max(r['score_calls']/args.iterations for r in rows)}}
            if any(r['per_solve_score']!=rows[0]['per_solve_score'] for r in rows):raise RuntimeError('Fixed-seed score varied between repetitions')
            if runtime=='rust':
                entry[runtime]['constraint_calls']=entry[runtime]['score_calls'].copy()
                for field in ['evaluations','steps']+(['construction_steps'] if case['start_mode']=='cold' else []):entry[runtime][field]={'min':min(r[field]/args.iterations for r in rows),'max':max(r[field]/args.iterations for r in rows)}
        if case['reference_kind']=='independent_exhaustive_optimum':entry['optimal_score']=case['reference_score'][:]

        entry['java_over_rust']=entry['java']['median']/entry['rust']['median'];entries.append(entry)
    return entries

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name,default in [('iterations',3),('warmup',3),('samples',5),('forks',3)]:parser.add_argument('--'+name,type=int,default=default)
    parser.add_argument('--native-search-profile',choices=['joint','component-refinement'],default='component-refinement',help='Native component search followed by joint refinement, or original joint search at every step')
    args=parser.parse_args()
    if min(args.iterations,args.samples,args.forks)<1 or args.warmup<0:parser.error('Positive counts; warmup may be zero')
    classpath,artifacts=prepare_solver_java()
    subprocess.run(['javac','-cp',classpath,'-d',str(WORK/'solver'),str(ROOT/'compat/oracle/MixedSolverBenchmark.java')],cwd=ROOT,check=True)
    subprocess.run(['cargo','build','--release','--locked','--example','mixed_solver_benchmark'],cwd=ROOT,check=True)
    digest=measurement_digest('mixed-performance');results=[]
    for index,case in enumerate(workloads()):results.extend(measure_case(case,args,classpath,index))
    if measurement_digest('mixed-performance')!=digest:raise RuntimeError('Benchmark inputs changed')
    cpu=next((line.split(':',1)[1].strip() for line in open('/proc/cpuinfo') if line.startswith('model name')),'unknown')
    report={'schema_version':1,'generated_at':datetime.now(timezone.utc).isoformat(),'source_digest':digest,'reference':REF,'artifacts':artifacts,'config':{**vars(args),'seed':SEED,'seed_scope':'Java seed; native Original order is deterministic without random draws','max_steps':24,'max_evaluations':40000,'start_modes':['warm','cold'],'native_joint_moves':True,'java_joint_moves':False,'native_cold_stream_unassigned_penalty':('route-cardinality' if args.native_search_profile=='component-refinement' else 'all-values-zero-for-assigned'),'native_cold_dense_unassigned_penalty':('route-cardinality' if args.native_search_profile=='component-refinement' else 'per-value-constraints'),'native_phase_policy':('component search until stalled, then joint refinement; shared per-case step/evaluation bounds' if args.native_search_profile=='component-refinement' else 'joint candidates at every local-search step')},
        'workloads':results,'environment':{'cpu':cpu,'platform':platform.platform(),'rust':version(['rustc','--version']),'java':version(['java','-version'])},
        'methodology':'Original coupled depot/basic and customer/list Manhattan tours with capacity penalties, complete feasible warm starts and cold starts derived by clearing unpinned depots/routes while preserving prefixes and whole-owner pins. Encoded complete inputs specify pins and warm references; cold initial_score independently includes missing-depot/customer penalties and retained-route distances. Small optimum independently exhaustively certified; larger reference is an independently scored feasible certificate, NOT an optimum. Warm quality floors retain the original feasible input; cold small must reach its independent optimum and cold larger must reach its feasible certificate or better. Every repeated solve validates its output after timing; Python checks every observed checksum/assignment/pin/score and quality floor. Warm small unpinned case must strictly improve. Cold construction plus local search share the native evaluation budget; evaluations include both phases; steps count completed local-search steps only, with construction_steps reported separately and the configured local-search step limit. Native Dense/Stream use the explicitly recorded native_search_profile: component-refinement runs component-only Original-order BestAccepted until stall, then joint refinement; joint retains the original all-joint-at-every-step control. Both search phases share the original per-case step/evaluation caps; cold construction shares the evaluation cap and its placements remain separate. Additional phase initialization consumes budget. Warm graph/network shape stays unchanged. In component-refinement cold scoring, both native backends incorporate missing customers as registered count minus the sum of route lengths into the existing per-owner route constraint, charging the constant once to owner zero even when its depot is unassigned. The original joint control retains its per-value penalties. Complete planning source rows and shadow metadata remain retained. The partial/final objective is identical, but cold constraint grouping, graph/network shape, explanations and physical callback counts differ. Warm declarations are unchanged. The strategy changes candidate work, tie paths and potentially quality; it is not an implementation-only or equal-work speedup. All configured warm/cold/backend workloads, including slower cold/small cases, remain in the report. Native profiles use finite score/candidate/step ceilings; Java uses deterministic ORIGINAL union of basic changes, list relocation and swaps with public step/score-call termination, no joint-move or trace equivalence. Native actual evaluations/steps/callbacks and Java measured physical EasyScoreCalculator calls are different work metrics; Java logical steps/evaluations are unknown, never inferred. Timings include per-solve scoring/solver setup and search, exclude process startup, parsing, Java factory preparation and postsolve diagnostics. Native cold-model creation, graph/network setup, construction and local search are timed together; warm timing is unchanged. Java cold input reset/copy, construction and local search are included in timing; Java warm input object copy remains outside timing. Cold protocol carries the required quality floor so both binaries validate every repeated solve, including earlier iterations, before accepting a sample. Runtime order rotates Java/Dense/Stream by fork; a single measured Java sample group per case is reused in both native rows, not counted as independent extra runs. Quality and work must accompany speed ratios; these bounded warm/cold workloads establish no general mixed-solver scalability, memory or equal-work claim.'}
    (ROOT/'work/mixed-performance.json').write_text(json.dumps(report,indent=2)+'\n')

if __name__=='__main__':raise SystemExit(run_measurement('mixed-performance',main))
