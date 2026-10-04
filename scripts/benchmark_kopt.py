#!/usr/bin/env python3
"""Compare bounded cyclic k-opt quality/time using public Community configuration."""
import argparse
from datetime import datetime, timezone
from itertools import permutations
import json
import platform
import random
import statistics
import subprocess
from benchmark_lists import encode, accepts
from benchmark_scores import version
from differential_scores import ROOT, WORK, REF, source_digest
from prepare_solver_oracle import prepare_solver_java

SEED = 20261008
WINDOW = 4096


def score(c, row):
    points = c['points']
    return (0, -sum(abs(points[a][0]-points[b][0])+abs(points[a][1]-points[b][1])
                    for a,b in zip(row, row[1:]+row[:1])))


def optimum(c):
    """Independent exhaustive cycles anchored at identity zero; reversal retained."""
    return max(score(c, [0,*tail]) for tail in permutations(range(1,len(c['points']))))


def parse_result(c, line, optimal=None):
    f=line.split('\t')
    if len(f)!=6 or f[0]!=c['id']: raise ValueError('Response identity/shape mismatch')
    elapsed,hard,soft,calls=map(int,[f[1],f[2],f[3],f[5]])
    if elapsed<=0 or calls<1: raise ValueError('Invalid time or score-call count')
    row=list(map(int,f[4].split(',')))
    if sorted(row)!=list(range(len(c['points']))): raise ValueError('Missing, duplicate or unknown identity')
    if any(not accepts(c,0,v) for v in row): raise ValueError('Value outside entity range')
    claimed=(hard,soft)
    if score(c,row)!=claimed: raise ValueError('Score mismatch')
    if claimed<score(c,c['initial'][0]): raise ValueError('Best regressed from initial')
    if optimal is not None and claimed>optimal: raise ValueError('Score exceeds independent optimum')
    return dict(elapsed_ns=elapsed,score=list(claimed),lists=[row],score_calls=calls)


def case(n,index):
    rng=random.Random(SEED+n*100+index)
    points=[(rng.randrange(-20,21),rng.randrange(-20,21)) for _ in range(n)]
    if index==0: points=[(0,0)]*n
    if index==1: points=[(i,0) for i in range(n)]
    order=list(range(n));rng.shuffle(order)
    return dict(id=f'cycle-{n}-{index}',points=points,depots=[(0,0)],capacities=[n],
                initial=[order],pins=[False],prefixes=[0])


def generate(): return [case(8,i) for i in range(32)]


def summarize(rows):
    scores={tuple(r['score']) for r in rows}
    if len(scores)!=1: raise ValueError('Deterministic scores vary across runs')
    times=[r['elapsed_ns']/1e6 for r in rows]
    return dict(median=statistics.median(times),min=min(times),max=max(times),score=list(next(iter(scores))),
                score_calls=dict(min=min(r['score_calls'] for r in rows),max=max(r['score_calls'] for r in rows)))



def check_search_work(rows, budget):
    if not rows or any(not 1 < row['score_calls'] <= budget for row in rows):
        raise ValueError('Search comparison must score candidates within its budget')


def quality_summary(checks):
    if not checks: raise ValueError('Empty comparison suite')
    identities=[(c['case']['id'],c['minimum'],c['maximum'],c['native_strategy']) for c in checks]
    if len(set(identities))!=len(identities): raise ValueError('Duplicate paired comparison')
    def counts(rows):
        return dict(total=len(rows),equal_scores=sum(c['rust']['score']==c['java']['score'] for c in rows),
                    optimal={r:sum(c[r]['score']==c['optimal_score'] for c in rows) for r in ('java','rust')})
    return dict(**counts(checks),passed=len(checks),
                by_maximum={str(k):counts([c for c in checks if c['maximum']==k]) for k in sorted({c['maximum'] for c in checks})},
                by_strategy={strategy:counts([c for c in checks if c['native_strategy']==strategy]) for strategy in sorted({c['native_strategy'] for c in checks})},cases=checks)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--steps',type=int,default=30)
    parser.add_argument('--evaluations',type=int,default=10_000)
    parser.add_argument('--warmup',type=int,default=3)
    parser.add_argument('--samples',type=int,default=5)
    parser.add_argument('--forks',type=int,default=3)
    args=parser.parse_args()
    if not 1<=args.steps<=2_147_483_647 or not WINDOW<=args.evaluations<=2**63-1 or min(args.samples,args.forks)<1 or args.warmup<0: parser.error('Invalid counts; evaluation budget must cover the shuffle window')
    classpath,artifacts=prepare_solver_java()
    subprocess.run(['javac','-cp',classpath,'-d',str(WORK/'solver'),str(ROOT/'compat/oracle/ListSolverBenchmark.java'),str(ROOT/'compat/oracle/KOptBenchmark.java')],check=True)
    subprocess.run(['cargo','build','--release','--example','k_opt_benchmark','--locked'],cwd=ROOT,check=True)
    digest=source_digest()
    bases={'java':['java','-Xms256m','-Xmx256m','-cp',classpath,'KOptBenchmark'],
           'rust':[str(ROOT/'target/release/examples/k_opt_benchmark')]}
    def run(runtime,minimum,maximum,cases,warmup,samples,optima=None,strategy="window"):
        cmd=bases[runtime]+list(map(str,[minimum,maximum,args.steps,args.evaluations,SEED,warmup,samples]))
        if runtime=="rust": cmd.append(strategy)
        result=subprocess.run(cmd,input='\n'.join(map(encode,cases))+'\n',text=True,capture_output=True,timeout=180)
        if result.returncode: raise RuntimeError(f'{runtime} {minimum}-{maximum}: {result.stderr}')
        lines=result.stdout.splitlines()
        if len(lines)!=len(cases)*samples: raise ValueError('Response count mismatch')
        return [[parse_result(c,line,None if optima is None else optima[c['id']])
                 for line in lines[i*samples:(i+1)*samples]] for i,c in enumerate(cases)]
    inputs=generate();optima={c['id']:optimum(c) for c in inputs}
    checks=[]
    strategies=['window','sampled']
    for maximum in [3,4]:
        java=run('java',2,maximum,inputs,0,1,optima)
        for strategy in strategies:
            runs={'java':java,'rust':run('rust',2,maximum,inputs,0,1,optima,strategy)}
            for i,c in enumerate(inputs):
                for runtime in bases: check_search_work(runs[runtime][i],args.evaluations)
                entry=dict(case=c,minimum=2,maximum=maximum,native_strategy=strategy,optimal_score=list(optima[c['id']]),**{r:runs[r][i][0] for r in bases})
                entry['equal_scores']=entry['rust']['score']==entry['java']['score'];checks.append(entry)
    print(f'K-opt validation: {len(checks)} paired comparisons valid; {sum(c["equal_scores"] for c in checks)} equal scores',flush=True)
    # Retain exact-bound behavior separately, without search speed ratios.
    probes=[]
    for k in [3,4]:
        for n in [8,16]:
            c=case(n,9)
            optimal=optimum(c) if n==8 else None
            runs={r:run(r,k,k,[c],0,1,None if optimal is None else {c['id']:optimal})[0][0] for r in bases}
            probes.append(dict(case=c,minimum=k,maximum=k,optimal_score=optimal,**runs))
            print(f'Exact k={k}, {n} values: score calls Java {runs["java"]["score_calls"]}, Rust {runs["rust"]["score_calls"]}',flush=True)
    workloads=[]
    for strategy in strategies:
        for maximum in [3,4]:
            for index,n in enumerate([12,16]):
                c=case(n,99);measurements={r:[] for r in bases}
                for fork in range(args.forks):
                    order=['java','rust'] if (fork+index)%2==0 else ['rust','java']
                    for runtime in order: measurements[runtime].extend(run(runtime,2,maximum,[c],args.warmup,args.samples,strategy=strategy)[0])
                for rows in measurements.values(): check_search_work(rows,args.evaluations)
                entry=dict(name=f'K-opt 2–{maximum} · {n} values · {strategy}',minimum=2,maximum=maximum,native_strategy=strategy,case=c,unit='ms / solve',optimal_score=None,samples=measurements,
                           **{r:summarize(rows) for r,rows in measurements.items()})
                entry['equal_scores']=entry['rust']['score']==entry['java']['score']
                entry['java_over_rust']=entry['java']['median']/entry['rust']['median'];workloads.append(entry)
                print(f'{entry["name"]}: Rust {entry["rust"]["median"]:.3f}, Java {entry["java"]["median"]:.3f} ms; scores {entry["rust"]["score"]}/{entry["java"]["score"]}',flush=True)
    if digest!=source_digest(): raise RuntimeError('Sources changed during measurement')
    report=dict(schema_version=1,generated_at=datetime.now(timezone.utc).isoformat(),source_digest=digest,reference=REF,artifacts=artifacts,
                config={**vars(args),'seed':SEED,'native_window':WINDOW,'native_attempts_per_step':args.evaluations,'native_strategies':strategies,'iterations':1},workloads=workloads,boundary_probes=probes,
                validation=quality_summary(checks),
                environment=dict(cpu=next((s.split(':',1)[1].strip() for s in open('/proc/cpuinfo') if s.startswith('model name')),'unknown'),
                                 platform=platform.platform(),rust=version(['rustc','--version']),java=version(['java','-version'])),
                methodology='Original single-entity, complete unpinned cyclic Manhattan routes. Capacity is nonbinding. Same initial orders, configured k ranges 2–3/2–4, seed, step and score-evaluation limits. First-accepted hill climbing; best result retained. Public Java KOptListMoveSelectorConfig uses random selection. Rust window strategy enumerates anchored tours by exact changed-edge count then cut/path order and uses a seeded rolling shuffle window of 4096; its first window reaches each configured k at measured sizes. Rust sampled strategy instead uses the configured score-evaluation budget as its maximum number of proposals per step, selecting entity/k/cut edges/path order/directions with seeded randomness and rejecting retained cut edges. The seed varies deterministically per completed step. Both native strategies include both cycle directions; final-tour sampling is not uniform. Java may retain removed edges or select equivalent cycles; native exact-edge enumeration excludes these. Sampling, PRNGs, anchor conventions, score-count accounting and work differ. Counts and quality accompany time; no identical-trace, equal-work or general speed claim. Full scoring in both engines. Timed: per-solve setup, solving, full score/non-regression/ownership/shadow validation. Excluded: parsing, model/network/factory construction, process startup and output encoding. Three warmups per process by default; alternating runtime order across fresh process forks. All outputs independently validated in Python; 128 paired comparisons (64 per native strategy over the same 32 routes and two k ranges) have exhaustive cyclic optima. Large workload optima unknown. Four exact-3/exact-4 boundary probes are excluded from timing comparisons because pinned Java returned without candidate scoring in the initial observations; actual counts remain recorded. No extra upstream method coverage.')
    (ROOT/'work/kopt-performance.json').write_text(json.dumps(report,indent=2)+'\n')

if __name__=='__main__':main()
