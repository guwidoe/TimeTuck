#!/usr/bin/env python3
"""Verify and measure full-scoring hill-climbing, entity/move/value tabu, combined late acceptance/tabu, step-counting and Great Deluge workloads in both engines."""
import argparse
import json
import platform
import statistics
import subprocess
from datetime import datetime, timezone
from differential_scores import ROOT, REF, source_digest
from differential_solver import encode, generate, optimum, score, validate
from prepare_solver_oracle import prepare_solver_java
from benchmark_scores import version


def parse_result(case, line):
    fields = line.split('\t')
    if len(fields) != 6 or fields[0] != case['id']:
        raise ValueError('Heuristic response identity/field mismatch')
    elapsed, hard, soft, calls = map(int, [fields[1], fields[2], fields[3], fields[5]])
    if elapsed <= 0 or calls < 1:
        raise ValueError('Invalid elapsed time or score-call count')
    claimed = (hard, soft)
    error = validate(case, '\t'.join([fields[0],fields[2],fields[3],fields[4]]), claimed)
    if error:
        raise ValueError(error)
    if claimed < score(case, case['initial']):
        raise ValueError('Returned best is worse than initial assignment')
    return {'elapsed_ns':elapsed, 'score':list(claimed),
            'values':list(map(int,fields[4].split(','))), 'score_calls':calls}


def workload(n):
    # Planted optimum: color i%4 yields no conflicts and no preference penalties.
    return {'id':f'{n} variables / 4 values', 'value_count':4,
            'initial':[0]*n, 'pins':[False]*n,
            'edges':[(i,(i+d)%n) for i in range(n) for d in range(1,4)],
            'weights':[[0 if v==i%4 else -1 for v in range(4)] for i in range(n)]}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--steps',type=int,default=50)
    parser.add_argument('--warmup',type=int,default=5)
    parser.add_argument('--samples',type=int,default=7)
    parser.add_argument('--forks',type=int,default=3)
    args=parser.parse_args()
    if min(args.steps,args.samples,args.forks)<1 or args.warmup<0 or args.steps>2_147_483_647:
        parser.error('Positive counts required; steps must fit Java int; warmup may be zero')
    classpath,artifacts=prepare_solver_java()
    subprocess.run(['cargo','build','--release','--example','heuristic_benchmark','--locked'],cwd=ROOT,check=True)
    digest=source_digest()
    bases={'java':['java','-Xms256m','-Xmx256m','-cp',classpath,'HeuristicBenchmark'],
           'rust':[str(ROOT/'target/release/examples/heuristic_benchmark')]}
    def run(runtime,algorithm,cases,warmup,samples):
        command=bases[runtime]+[algorithm,str(args.steps),str(warmup),str(samples)]
        result=subprocess.run(command,input='\n'.join(map(encode,cases))+'\n',text=True,capture_output=True,timeout=180)
        if result.returncode:
            raise RuntimeError(f'{runtime} {algorithm}: {result.stderr}')
        rows=result.stdout.splitlines()
        if len(rows)!=len(cases)*samples:
            raise ValueError('Heuristic response count mismatch')
        return [[parse_result(case,line) for line in rows[i*samples:(i+1)*samples]] for i,case in enumerate(cases)]
    cases=generate()[:32]
    for case in cases:
        case['initial']=[0 if value is None else value for value in case['initial']]
    optima={case['id']:optimum(case) for case in cases}
    checks=[]
    for algorithm in ['hill','tabu','move_tabu','value_tabu','late_tabu','step','deluge']:
        runs={runtime:run(runtime,algorithm,cases,0,1) for runtime in bases}
        for i,case in enumerate(cases):
            entry={'case':case, 'algorithm':algorithm, 'optimal_score':list(optima[case['id']]),
                   **{runtime:runs[runtime][i][0] for runtime in bases}}
            entry['equal_scores']=entry['java']['score']==entry['rust']['score']
            checks.append(entry)
    results=[]
    for algorithm in ['hill','tabu','move_tabu','value_tabu','late_tabu','step','deluge']:
        for index,count in enumerate([32,96]):
            case=workload(count)
            measurements={'java':[],'rust':[]}
            for fork in range(args.forks):
                order=['java','rust'] if (fork+index)%2==0 else ['rust','java']
                for runtime in order:
                    measurements[runtime].extend(run(runtime,algorithm,[case],args.warmup,args.samples)[0])
            entry={'name':f'{dict(hill="Hill climbing",tabu="Tabu",move_tabu="Move tabu",value_tabu="Value tabu",late_tabu="Late acceptance + tabu",step="Step counting",deluge="Great Deluge")[algorithm]} · {count} variables',
                   'algorithm':algorithm,'case':case,'unit':'ms / solve','optimal_score':[0,0],
                   'samples':measurements}
            for runtime,rows in measurements.items():
                scores={tuple(row['score']) for row in rows}
                if len(scores)!=1:
                    raise ValueError(f'{runtime} deterministic benchmark returned varying scores')
                times=[row['elapsed_ns']/1_000_000 for row in rows]
                entry[runtime]={'median':statistics.median(times),'min':min(times),'max':max(times),
                                'score':list(next(iter(scores))),
                                'score_calls':{'min':min(r['score_calls'] for r in rows),'max':max(r['score_calls'] for r in rows)}}
            entry['equal_scores']=entry['java']['score']==entry['rust']['score']
            entry['java_over_rust']=entry['java']['median']/entry['rust']['median']
            results.append(entry)
            print(f'{entry["name"]}: Rust {entry["rust"]["median"]:.3f}, Java {entry["java"]["median"]:.3f} ms; scores {entry["rust"]["score"]} / {entry["java"]["score"]}',flush=True)
    if digest!=source_digest():
        raise RuntimeError('Sources changed during heuristic comparison')
    cpu=next((s.split(':',1)[1].strip() for s in open('/proc/cpuinfo') if s.startswith('model name')),'unknown')
    report={'schema_version':1,'generated_at':datetime.now(timezone.utc).isoformat(),'source_digest':digest,
            'reference':REF,'artifacts':artifacts,'config':{**vars(args),'seed':20261001,'iterations':1, 'value_tabu_tenure':2, 'move_tabu_tenure':2, 'combined_late_history':3, 'combined_tabu_tenure':2, 'step_counting_period':7, 'great_deluge_rise':[0,1]},
            'validation':{'total':len(checks),'passed':len(checks),'equal_scores':sum(c['equal_scores'] for c in checks),
                          'optimal':{runtime:sum(c[runtime]['score']==c['optimal_score'] for c in checks) for runtime in bases},
                          'cases':checks},
            'workloads':results,
            'environment':{'cpu':cpu,'platform':platform.platform(),'rust':version(['rustc','--version']),'java':version(['java','-version'])},
            'methodology':'Original complete graph-assignment inputs. Deterministic original-order change moves, best-accepted selection, first tie, best-rejected fallback, fixed completed-step limit. Hill climbing accepts ties; entity, move and value tabu use tenure 2 and global-best aspiration; move tabu retains each selected variable/target-value change descriptor; value tabu retains target values across variables (native extraction includes snapshot replay overhead); combined late acceptance/tabu requires both history size 3 (native hill-climbing shortcut enabled) and tenure 2 with global-best aspiration; step counting uses STEP mode with period 7 and replaces the threshold at completed-step boundaries; Great Deluge uses fixed rain (0hard/1soft) from the initial score with strict current-score improvement aspiration. Full score recalculation in both engines (Java EasyScoreCalculator; one global Rust constraint); no incremental-scoring speed claim. All input assignments retained, including pins. Timed: per-solve setup, search, and complete assignment/score validation. Excluded: process/JVM startup, input parsing, one-time model/network/factory setup, output encoding. Warmups precede samples in each fresh process; runtime order alternates. Every sample is independently rescored in Python and must retain at least the initial quality. Scores and score-call counts accompany timings; quality agreement is reported separately from validity. Small validation cases also have independently enumerated optima. These workloads do not establish general solver, random-search, memory or production-scale performance.'}
    (ROOT/'work/heuristic-performance.json').write_text(json.dumps(report,indent=2)+'\n')
    print(f'Heuristic validation: {len(checks)} valid; {report["validation"]["equal_scores"]} equal-score cases',flush=True)

if __name__=='__main__': main()
