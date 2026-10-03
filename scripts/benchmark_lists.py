#!/usr/bin/env python3
"""Validate and measure native/Java construction, relocation and two-opt search."""
import argparse
from copy import deepcopy
from datetime import datetime, timezone
from itertools import permutations, product
import json
import platform
import random
import statistics
import subprocess
from differential_scores import ROOT, REF, WORK, source_digest
from prepare_solver_oracle import prepare_solver_java
from benchmark_scores import version

SEED = 20261006


def encode(c):
    def points(items): return ','.join(f'{x}:{y}' for x,y in items) or '-'
    fields=[c['id'],points(c['points']),points(c['depots']),','.join(map(str,c['capacities'])),
                      ';'.join(','.join(map(str,r)) or '-' for r in c['initial']),
                      ''.join('1' if p else '0' for p in c['pins']),','.join(map(str,c['prefixes']))]
    if 'ranges' in c:
        fields.append(';'.join('*' if r is None else ','.join(map(str,r)) or '-' for r in c['ranges']))
    return '\t'.join(fields)


def score(c, lists):
    hard = -sum(max(0,len(row)-capacity) for row,capacity in zip(lists,c['capacities']))
    distance = 0
    for depot,row in zip(c['depots'],lists):
        tour = [depot]+[c['points'][v] for v in row]+[depot]
        distance += sum(abs(a[0]-b[0])+abs(a[1]-b[1]) for a,b in zip(tour,tour[1:]))
    return hard,-distance


def accepts(c, entity, value):
    ranges=c.get('ranges')
    return ranges is None or ranges[entity] is None or value in ranges[entity]


def optimum(c):
    """Independent exhaustive owner selection and route permutations, pins retained."""
    frozen = [row[:] if pin else row[:prefix] for row,pin,prefix in zip(c['initial'],c['pins'],c['prefixes'])]
    fixed = {v for row in frozen for v in row}
    movable = [v for v in range(len(c['points'])) if v not in fixed]
    destinations = [e for e,pin in enumerate(c['pins']) if not pin]
    best = None
    for owners in product(destinations, repeat=len(movable)):
        if any(not accepts(c,e,v) for v,e in zip(movable,owners)): continue
        groups = [[v for v,owner in zip(movable,owners) if owner==e] for e in range(len(frozen))]
        for ordered in product(*(permutations(g) for g in groups)):
            result=score(c,[prefix+list(tail) for prefix,tail in zip(frozen,ordered)])
            best=result if best is None else max(best,result)
    if best is None: raise ValueError('No complete state in validation case')
    return best


def parse_result(c, line, mode, optimal=None):
    if mode not in ('construct','hill','two-opt'): raise ValueError('Unknown mode')
    fields = line.split('\t')
    if len(fields)!=6 or fields[0]!=c['id']: raise ValueError('Response identity/shape mismatch')
    elapsed,hard,soft,calls=map(int,[fields[1],fields[2],fields[3],fields[5]])
    if elapsed<=0 or calls<1: raise ValueError('Invalid time or score-call count')
    lists=[[] if r=='-' else list(map(int,r.split(','))) for r in fields[4].split(';')]
    if len(lists)!=len(c['depots']): raise ValueError('Entity count mismatch')
    if sorted(v for row in lists for v in row)!=list(range(len(c['points']))): raise ValueError('Missing, duplicate or unknown value')
    for e,row in enumerate(lists):
        if any(not accepts(c,e,v) for v in row): raise ValueError('Value outside entity range')
    for before,after,pin,prefix in zip(c['initial'],lists,c['pins'],c['prefixes']):
        if (pin and before!=after) or before[:prefix]!=after[:prefix]: raise ValueError('Pin changed')
        if mode=='construct':
            retained=[v for v in after if v in before]
            if retained!=before: raise ValueError('Construction changed an initial value or its order')
    claimed=(hard,soft)
    if score(c,lists)!=claimed: raise ValueError('Score mismatch')
    if mode in ('hill','two-opt') and claimed<score(c,c['initial']): raise ValueError('Best regressed from initial')
    if optimal is not None and claimed>optimal: raise ValueError('Score exceeds independent optimum')
    return {'elapsed_ns':elapsed,'score':list(claimed),'lists':lists,'score_calls':calls}


def generate():
    rng=random.Random(SEED)
    bases=[{'points':[],'depots':[(0,0)],'capacities':[0],'initial':[[]],'pins':[False],'prefixes':[0]},
           {'points':[(2,3)],'depots':[(0,0)],'capacities':[1],'initial':[[0]],'pins':[True],'prefixes':[0]},
           {'points':[(2,3),(4,5)],'depots':[(0,0)],'capacities':[1],'initial':[[0,1]],'pins':[False],'prefixes':[1]},
           {'points':[(1,0),(2,0),(8,0),(9,0)],'depots':[(0,0),(10,0)],'capacities':[2,2],'initial':[[2,0],[1,3]],'pins':[False,False],'prefixes':[0,0]}]
    for _ in range(28):
        n,m=rng.randint(3,5),rng.randint(1,3)
        rows=[[] for _ in range(m)];values=list(range(n));rng.shuffle(values)
        for v in values: rows[rng.randrange(m)].append(v)
        pins=[rng.random()<0.2 for _ in rows];pins[-1]=False
        prefixes=[0 if pin else rng.randrange(len(row)+1) for pin,row in zip(pins,rows)]
        bases.append({'points':[(rng.randint(-5,10),rng.randint(-5,10)) for _ in range(n)],
                      'depots':[(rng.randint(-5,10),rng.randint(-5,10)) for _ in rows],
                      'capacities':[rng.randint(0,n) for _ in rows], 'initial':rows,'pins':pins,'prefixes':prefixes})
    cases={'construct':[],'hill':[]}
    for i,base in enumerate(bases):
        for mode in cases:
            c=deepcopy(base);c['id']=f'list-{mode}-{i:02}'
            if mode=='construct':
                for e,row in enumerate(c['initial']):
                    if not c['pins'][e]: c['initial'][e]=row[:c['prefixes'][e]]+[v for v in row[c['prefixes'][e]:] if rng.random()<0.25]
            cases[mode].append(c)
    return cases


def generate_extended():
    """32 inputs per phase with disjoint/overlapping ranges, empty ranges and pins."""
    rng=random.Random(SEED+1)
    cases=generate()
    for mode,items in cases.items():
        for c in items:
            c['id']=c['id'].replace('list-', 'list-range-',1)
            ranges=[set(row) for row in c['initial']]
            for v in range(len(c['points'])):
                for e in range(len(ranges)):
                    if rng.random()<0.35:ranges[e].add(v)
                if not any(v in row for row in c['initial']):
                    candidates=[e for e,pin in enumerate(c['pins']) if not pin]
                    ranges[rng.choice(candidates)].add(v)
            c['ranges']=[sorted(r) for r in ranges]
    return cases


def generate_two_opt():
    """Reuse complete original/ranged inputs; preserve earlier phase corpora."""
    cases = deepcopy(generate()['hill'] + generate_extended()['hill'])
    for c in cases: c['id'] = c['id'].replace('hill','two-opt')
    return cases


def workload(n,mode):
    rng=random.Random(SEED+n)
    c={'id':f'{mode}-{n}', 'points':[(rng.randrange(101),rng.randrange(101)) for _ in range(n)],
       'depots':[(0,0),(100,0),(100,100),(0,100)],'capacities':[n//4]*4,
       'initial':[list(range(e,n,4)) if mode!='construct' else [] for e in range(4)],
       'pins':[False]*4,'prefixes':[0]*4}
    return c


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--steps',type=int,default=20);parser.add_argument('--warmup',type=int,default=3)
    parser.add_argument('--samples',type=int,default=5);parser.add_argument('--forks',type=int,default=3)
    args=parser.parse_args()
    if min(args.steps,args.samples,args.forks)<1 or args.warmup<0 or args.steps>2_147_483_647: parser.error('Invalid counts')
    classpath,artifacts=prepare_solver_java()
    subprocess.run(['javac','-cp',classpath,'-d',str(WORK/'solver'),str(ROOT/'compat/oracle/ListSolverBenchmark.java')],check=True)
    subprocess.run(['cargo','build','--release','--example','list_solver_benchmark','--locked'],cwd=ROOT,check=True)
    digest=source_digest()
    bases={'java':['java','-Xms256m','-Xmx256m','-cp',classpath,'ListSolverBenchmark'],
           'rust':[str(ROOT/'target/release/examples/list_solver_benchmark')]}
    def run(runtime,mode,cases,warmup,samples,optima=None):
        result=subprocess.run(bases[runtime]+[mode,str(args.steps),str(warmup),str(samples)],
            input='\n'.join(map(encode,cases))+'\n',text=True,capture_output=True,timeout=180)
        if result.returncode: raise RuntimeError(f'{runtime} {mode}: {result.stderr}')
        rows=result.stdout.splitlines()
        if len(rows)!=len(cases)*samples: raise ValueError('Response count mismatch')
        return [[parse_result(c,line,mode,None if optima is None else optima[c['id']])
                 for line in rows[i*samples:(i+1)*samples]] for i,c in enumerate(cases)]
    checks=[]
    cases_by_mode=generate()
    for mode,items in generate_extended().items():cases_by_mode[mode].extend(items)
    cases_by_mode['two-opt']=generate_two_opt()
    for mode,cases in cases_by_mode.items():
        optima={c['id']:optimum(c) for c in cases}
        runs={runtime:run(runtime,mode,cases,0,1,optima) for runtime in bases}
        for i,c in enumerate(cases):
            entry={'case':c,'mode':mode,'optimal_score':list(optima[c['id']]),**{r:runs[r][i][0] for r in bases}}
            entry['equal_scores']=entry['rust']['score']==entry['java']['score'];checks.append(entry)
    print(f'List solving validation: {len(checks)} valid; {sum(c["equal_scores"] for c in checks)} equal-score cases',flush=True)
    results=[]
    for mode in ['construct','hill','two-opt']:
        for index,n in enumerate([16,32]):
            c=workload(n,mode);measurements={r:[] for r in bases}
            for fork in range(args.forks):
                order=['java','rust'] if (fork+index)%2==0 else ['rust','java']
                for runtime in order: measurements[runtime].extend(run(runtime,mode,[c],args.warmup,args.samples)[0])
            entry={'name':f'{ {"construct":"Construction","hill":"Hill climbing","two-opt":"Two-opt"}[mode]} · {n} customers',
                   'mode':mode,'case':c,'unit':'ms / solve','optimal_score':None,'samples':measurements}
            for runtime,rows in measurements.items():
                scores={tuple(r['score']) for r in rows}
                if len(scores)!=1: raise ValueError('Deterministic scores vary across runs')
                times=[r['elapsed_ns']/1e6 for r in rows]
                entry[runtime]={'median':statistics.median(times),'min':min(times),'max':max(times),'score':list(next(iter(scores))),
                                'score_calls':{'min':min(r['score_calls'] for r in rows),'max':max(r['score_calls'] for r in rows)}}
            entry['equal_scores']=entry['rust']['score']==entry['java']['score'];entry['java_over_rust']=entry['java']['median']/entry['rust']['median'];results.append(entry)
            print(f'{entry["name"]}: Rust {entry["rust"]["median"]:.3f}, Java {entry["java"]["median"]:.3f} ms; scores {entry["rust"]["score"]} / {entry["java"]["score"]}',flush=True)
    if digest!=source_digest(): raise RuntimeError('Sources changed during measurement')
    report={'schema_version':1,'generated_at':datetime.now(timezone.utc).isoformat(),'source_digest':digest,'reference':REF,'artifacts':artifacts,
            'config':{**vars(args),'seed':SEED,'range_seed':SEED+1,'two_opt_score_limit':10_000,'iterations':1},'workloads':results,
            'validation':{'total':len(checks),'passed':len(checks),'equal_scores':sum(c['equal_scores'] for c in checks),
                          'optimal':{r:sum(c[r]['score']==c['optimal_score'] for c in checks) for r in bases},
                          'by_mode':{mode:{'total':sum(c['mode']==mode for c in checks),
                                           'equal_scores':sum(c['mode']==mode and c['equal_scores'] for c in checks),
                                           'optimal':{r:sum(c['mode']==mode and c[r]['score']==c['optimal_score'] for c in checks) for r in bases}}
                                     for mode in cases_by_mode},'cases':checks},
            'environment':{'cpu':next((s.split(':',1)[1].strip() for s in open('/proc/cpuinfo') if s.startswith('model name')),'unknown'),
                           'platform':platform.platform(),'rust':version(['rustc','--version']),'java':version(['java','-version'])},
            'methodology':'Required ordered lists with Manhattan round-trip distance and per-route capacity penalties. Construction inserts missing values using best insertion with no early pick; engine-specific tie handling is retained. Relocation hill climbing uses complete identical inputs, best accepted/first tie, best-rejected fallback, and the same step cap. Two-opt uses identical complete starts, within-route reversals and non-reversing cross-owner nonempty tail exchanges, first accepted hill climbing and the same step cap, with an additional 10,000 score-calculation/evaluation budget. Java uses its seeded public preview TwoOptListMoveProvider; native enumeration is deterministic. Candidate sampling/order and score-count accounting differ, so these are bounded quality/time comparisons, not equal-work or equal-trace claims. Deterministic native original orders may differ (Rust entity/position order; Java configured ORIGINAL); scores, callback work and optima agreement are reported, not identical traces. Full scoring in both engines: Java EasyScoreCalculator, one global Rust constraint. Timed: per-solve setup, solving and assignment/score/shadow validation. Excluded: input parsing, model/network/factory creation, process startup and output encoding. Warmups and alternating runtime order across fresh processes. Every measured output is independently checked for ownership, completeness, pins, score and local-search non-regression; construction retains existing route values in relative order. Small original cases, including per-entity ranges, have independently enumerated optima. Legacy seven-field inputs use the full global registry as every entity range; eight-field inputs add explicit subsets. Larger timing workloads retain unrestricted ranges. Larger workload optima are unknown. Rust list transactions clone state and derive shadows by linear lookup; Java uses managed shadows. No general solver, incremental-scoring, memory, optional-list or mixed-model performance claim.'}
    (ROOT/'work/list-performance.json').write_text(json.dumps(report,indent=2)+'\n')

if __name__=='__main__':main()
