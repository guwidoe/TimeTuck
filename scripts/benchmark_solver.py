#!/usr/bin/env python3
"""Measure brute force and four branch-and-bound orders against pinned Timefold."""
from verification_inputs import measurement_digest
from verification_cache import run_measurement
import argparse
import json
import platform
import random
import statistics
import subprocess
from datetime import datetime, timezone
from differential_scores import ROOT, REF
from differential_solver import encode, optimum, validate
from prepare_solver_oracle import prepare_solver_java
from benchmark_scores import version


ALGORITHMS = {'brute':'Brute force', 'depth':'Branch and bound · depth first',
              'breadth':'Branch and bound · breadth first', 'score':'Branch and bound · score first',
              'bound':'Branch and bound · optimistic bound first'}


def workload(n, algorithm='brute'):
    rng = random.Random(20261001 + n)
    if algorithm not in ALGORITHMS: raise ValueError('Unknown exact-search algorithm')
    case = {'id': f'{ALGORITHMS[algorithm]} · {n} variables / 3 values', 'value_count':3, 'initial':[None]*n, 'pins':[False]*n,
            'edges':[(a,b) for a in range(n) for b in range(a+1,n) if rng.random() < 0.4],
            'weights':[[rng.randint(-9,9) for _ in range(3)] for _ in range(n)]}
    if algorithm != 'brute': case['weights'] = [[-abs(v) for v in row] for row in case['weights']]
    return case


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--iterations',type=int,default=5)
    parser.add_argument('--warmup',type=int,default=5)
    parser.add_argument('--samples',type=int,default=7)
    parser.add_argument('--forks',type=int,default=3)
    args = parser.parse_args()
    if min(args.iterations,args.samples,args.forks) < 1 or args.warmup < 0: parser.error('Positive counts required; warmup may be zero')
    classpath, artifacts = prepare_solver_java()
    subprocess.run(['cargo','build','--release','--example','solver_benchmark','--locked'],cwd=ROOT,check=True)
    digest = measurement_digest('solver-performance')
    results=[]
    for index, (algorithm, count) in enumerate((a,n) for a in ALGORITHMS for n in [6,8]):
        case = workload(count, algorithm)
        expected = optimum(case)
        common = [*map(str,[args.iterations,args.warmup,args.samples]), algorithm]
        commands = {'java':['java','-Xms256m','-Xmx256m','-cp',classpath,'SolverBenchmark',*common],
                    'rust':[str(ROOT / 'target/release/examples/solver_benchmark'),*common]}
        measurements = {'java':[],'rust':[]}
        for fork in range(args.forks):
            order = ['java','rust'] if (fork+index)%2==0 else ['rust','java']
            for name in order:
                run = subprocess.run(commands[name],input=encode(case)+'\n',text=True,capture_output=True,timeout=120)
                if run.returncode: raise RuntimeError(f'{name}: {run.stderr}')
                rows=run.stdout.splitlines()
                if len(rows)!=args.samples: raise RuntimeError('Unexpected sample count')
                for row in rows:
                    elapsed,hard,soft,values=row.split('\t')
                    if (int(hard),int(soft)) != tuple(v*args.iterations for v in expected): raise RuntimeError('Score checksum mismatch')
                    error=validate(case,f'{case["id"]}\t{expected[0]}\t{expected[1]}\t{values}',expected)
                    if error: raise RuntimeError(error)
                    measurements[name].append(int(elapsed)/args.iterations/1_000_000)
        entry={'name':case['id'],'algorithm':algorithm,'unit':'ms / solve','samples':measurements,'case':case,'complete_assignments':3**count,
               'optimal_score':list(expected)}
        for name, samples in measurements.items():
            entry[name]={'median':statistics.median(samples),'min':min(samples),'max':max(samples)}
        entry['java_over_rust']=entry['java']['median']/entry['rust']['median']
        results.append(entry)
        print(f'{case["id"]}: Rust {entry["rust"]["median"]:.3f}, Java {entry["java"]["median"]:.3f} ms/solve; {entry["java_over_rust"]:.2f}x',flush=True)
    if digest != measurement_digest('solver-performance'): raise RuntimeError('Sources changed during benchmark')
    cpu=next((s.split(':',1)[1].strip() for s in open('/proc/cpuinfo') if s.startswith('model name')),'unknown')
    report={'schema_version':1,'generated_at':datetime.now(timezone.utc).isoformat(),'source_digest':digest,
            'reference':REF,'artifacts':artifacts,'config':{**vars(args),'seed':20261001,'algorithms':list(ALGORITHMS)},'workloads':results,
            'environment':{'cpu':cpu,'platform':platform.platform(),'rust':version(['rustc','--version']),'java':version(['java','-version'])},
            'methodology':'Two problem sizes for brute force and each of four branch-and-bound orders (depth, breadth, score, optimistic bound). Brute force preserves the original mixed-sign preference workload; branch workloads use its nonpositive preferences so assigning another variable cannot improve either score level. Java uses BRANCH_AND_BOUND with ONLY_DOWN initializing-score trend; native bound is the partial full score. Both use original entity/domain order and full score recomputation. Independent Python enumeration verifies each optimum, sample checksum and final assignment; no identical traversal, node-count or bound-implementation claim. Models are parsed before timing; Java SolverFactory is also prepared once. Includes per-run solver setup, search and returned-assignment/score validation, excludes startup and parsing. Fresh processes alternate runtime order after warmups. Shared-host ranges include allocation/GC effects. These small exact-search workloads do not establish large-instance, heuristic, incremental-scoring or memory performance.'}
    (ROOT/'work/solver-performance.json').write_text(json.dumps(report,indent=2)+'\n')

if __name__ == '__main__':
    raise SystemExit(run_measurement('solver-performance', main))
