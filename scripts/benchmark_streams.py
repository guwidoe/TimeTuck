#!/usr/bin/env python3
"""Verify and measure incremental stream search using Community public APIs."""
import argparse
import json
import platform
import statistics
import subprocess
from datetime import datetime, timezone
from differential_scores import ROOT, REF, source_digest
from differential_solver import encode, generate
from differential_streams import SEED, optimum, score, prepare
from benchmark_scores import version


def parse_result(case, line):
    fields = line.split('\t')
    if len(fields) != 6 or fields[0] != case['id']:
        raise ValueError('Stream response identity/field mismatch')
    elapsed, hard, soft, calls = map(int, [fields[1], fields[2], fields[3], fields[5]])
    if elapsed <= 0 or calls < 0:
        raise ValueError('Invalid elapsed time or terminal-callback count')
    claimed = (hard, soft)
    values=list(map(int,fields[4].split(',')))
    if len(values)!=len(case['initial']) or any(v<0 or v>=case['value_count'] for v in values):
        raise ValueError('Invalid assignment')
    if any(pin and v!=old for v,old,pin in zip(values,case['initial'],case['pins'])):
        raise ValueError('Pin changed')
    if claimed!=score(case,values):
        raise ValueError('Incorrect stream score')
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
    classpath,artifacts=prepare()
    digest=source_digest()
    bases={'java':['java','-Xms256m','-Xmx256m','-cp',classpath,'StreamBenchmark'],
           'rust':[str(ROOT/'target/release/examples/stream_benchmark')]}
    def run(runtime,algorithm,cases,warmup,samples):
        command=bases[runtime]+[algorithm,str(args.steps),str(warmup),str(samples)]
        result=subprocess.run(command,input='\n'.join(map(encode,cases))+'\n',text=True,capture_output=True,timeout=180)
        if result.returncode:
            raise RuntimeError(f'{runtime} {algorithm}: {result.stderr}')
        rows=result.stdout.splitlines()
        if len(rows)!=len(cases)*samples:
            raise ValueError('Stream response count mismatch')
        return [[parse_result(case,line) for line in rows[i*samples:(i+1)*samples]] for i,case in enumerate(cases)]
    cases=generate()[:32]
    for case in cases:
        case['initial']=[0 if value is None else value for value in case['initial']]
    optima={case['id']:optimum(case) for case in cases}
    checks=[]
    for algorithm in ['hill','tabu']:
        runs={runtime:run(runtime,algorithm,cases,0,1) for runtime in bases}
        for i,case in enumerate(cases):
            entry={'case':case, 'algorithm':algorithm, 'optimal_score':list(optima[case['id']]),
                   **{runtime:runs[runtime][i][0] for runtime in bases}}
            entry['equal_scores']=entry['java']['score']==entry['rust']['score']
            checks.append(entry)
    results=[]
    for algorithm in ['hill','tabu']:
        for index,count in enumerate([32,96]):
            case=workload(count)
            measurements={'java':[],'rust':[]}
            for fork in range(args.forks):
                order=['java','rust'] if (fork+index)%2==0 else ['rust','java']
                for runtime in order:
                    measurements[runtime].extend(run(runtime,algorithm,[case],args.warmup,args.samples)[0])
            entry={'name':f'{"Hill climbing" if algorithm=="hill" else "Tabu"} · {count} variables',
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
        raise RuntimeError('Sources changed during stream comparison')
    cpu=next((s.split(':',1)[1].strip() for s in open('/proc/cpuinfo') if s.startswith('model name')),'unknown')
    report={'schema_version':1,'generated_at':datetime.now(timezone.utc).isoformat(),'source_digest':digest,
            'reference':REF,'artifacts':artifacts,'config':{**vars(args),'seed':SEED,'iterations':1},
            'validation':{'total':len(checks),'passed':len(checks),'equal_scores':sum(c['equal_scores'] for c in checks),
                          'optimal':{runtime:sum(c[runtime]['score']==c['optimal_score'] for c in checks) for runtime in bases},
                          'cases':checks},
            'workloads':results,
            'environment':{'cpu':cpu,'platform':platform.platform(),'rust':version(['rustc','--version']),'java':version(['java','-version'])},
            'methodology':'Original complete graph assignments with indexed edge/job joins and grouped soft score sum*abs(sum). Timefold Community ConstraintProvider and native persistent StreamGraph both score moves incrementally. Deterministic original-order change moves, best-accepted selection, first tie, best-rejected fallback, fixed completed-step limit; hill climbing accepts ties and tabu uses tenure 2 with best aspiration. Timed: per-solve setup, search, and independent score/assignment validation. Rust builds graph topology and loads fixed edge facts per solve; Java reuses SolverFactory and builds a solver per solve. Excluded: process/JVM startup, parsing, factory creation and output. Fresh-process warmups and alternating runtime order. Every sample is independently rescored in Python and cannot regress from its initial score. Terminal weighting callback counts are reported; propagation/coalescing and callback counts need not match. Small cases have independently enumerated optima; quality equality is reported separately. This is end-to-end search timing, not a raw tuple-update or general solver speed claim.'}
    (ROOT/'work/stream-performance.json').write_text(json.dumps(report,indent=2)+'\n')
    print(f'Stream search validation: {len(checks)} valid; {report["validation"]["equal_scores"]} equal-score cases',flush=True)

if __name__=='__main__': main()
