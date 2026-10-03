#!/usr/bin/env python3
"""Compare fixed concurrent shadow-update transactions; no scoring/search timing."""
import argparse
from datetime import datetime, timezone
import json
import platform
import random
import statistics
import subprocess
from concurrent_checks import apply, project, valid
from differential_scores import ROOT, REF, WORK, source_digest
from prepare_solver_oracle import prepare_solver_java
from benchmark_scores import version

SEED = 20261004
FIELDS = ['owner', 'index', 'previous', 'next', 'cascading', 'ready', 'start', 'finish', 'inconsistent']

def expected(c):
    valid(c,c['lists'])
    if not 2 <= c['count'] <= 64 or sorted(v for ls in c['lists'] for v in ls)!=list(range(c['count'])):
        raise ValueError('Fully assigned 2–64 visits required')
    op=c['move']
    if len(op)!=5 or op[0] not in ('c','s') or op[1:3]==op[3:5]:raise ValueError('Single change/swap between distinct positions required')
    trial=apply(c,c['lists'],[op])
    def flatten(lists):
        p=project(c,lists)
        return [p[k][i] for i in range(c['count']) for k in FIELDS]
    return flatten(c['lists']),flatten(trial)

def encode_state(s):return ','.join('-' if v is None else str(v) for v in s)
def checksum(s):return sum((i+1)*(-1 if v is None else v) for i,v in enumerate(s))
def encode(c):
    initial,trial=expected(c)
    def row(s):return ','.join(map(str,s)) or '_'
    return '\t'.join([c['id'],str(c['count']),';'.join(map(row,c['groups'])) or '_',';'.join(map(row,c['lists'])),row(c['move']),encode_state(initial),encode_state(trial)])
def parse_result(c,line,iterations):
    f=line.split('\t')
    if len(f)!=6 or f[0]!=c['id'] or iterations<1:raise ValueError('Response identity/shape/count')
    elapsed,total=map(int,f[1:3])
    if elapsed<=0:raise ValueError('Nonpositive timing')
    observed=[[None if v=='-' else int(v) for v in field.split(',')] for field in f[3:]]
    initial,trial=expected(c)
    if observed!=[trial,initial,trial]:raise ValueError('Shadow trial/undo/commit mismatch')
    if total!=iterations*(31*checksum(trial)+checksum(initial)):raise ValueError('Checksum mismatch')
    return dict(elapsed_ns=elapsed,checksum=total,trial=observed[0],restored=observed[1],committed=observed[2])
def parse_batch(cases,output,iterations,samples):
    lines=output.splitlines()
    if not cases or samples<1 or len({c['id'] for c in cases})!=len(cases) or len(lines)!=len(cases)*samples:raise ValueError('Incomplete/ambiguous batch')
    return [[parse_result(c,line,iterations) for line in lines[i*samples:(i+1)*samples]] for i,c in enumerate(cases)]
def workload(count,mode):
    if count not in (16,64) or mode not in ('swap','relocate'):raise ValueError('Workload')
    lists=[list(range(0,count,2)),list(range(1,count,2))]
    op=['s',0,0,0,len(lists[0])-1] if mode=='swap' else ['c',0,len(lists[0])-1,1,len(lists[1])]
    return dict(id=f'{mode}-{count}',count=count,groups=[[i,i+1] for i in range(0,count,2)],lists=lists,move=op)
def generate():
    rng=random.Random(SEED);cases=[]
    for i in range(64):
        n=rng.randint(2,12);order=list(range(n));rng.shuffle(order)
        groups=[]
        while len(order)>1:
            width=rng.randint(1,min(3,len(order)))
            if width>1:groups.append(order[:width])
            del order[:width]
        lists=[[] for _ in range(rng.randint(1,4))];order=list(range(n));rng.shuffle(order)
        for v in order:lists[rng.randrange(len(lists))].append(v)
        positions=[(e,j) for e,ls in enumerate(lists) for j in range(len(ls))]
        e,j=rng.choice(positions)
        if i%2:
            d,k=rng.choice([p for p in positions if p!=(e,j)]);op=['s',e,j,d,k]
        else:
            d,k=rng.choice([(d,k) for d in range(len(lists)) for k in range(len(lists[d])+1-int(e==d)) if (e,j)!=(d,k)]);op=['c',e,j,d,k]
        cases.append(dict(id=f'concurrent-bench-{i}',count=n,groups=groups,lists=lists,move=op))
    return cases

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--iterations',type=int,default=200)
    parser.add_argument('--warmup',type=int,default=5)
    parser.add_argument('--samples',type=int,default=7)
    parser.add_argument('--forks',type=int,default=3)
    args=parser.parse_args()
    if not 1<=args.iterations<=10000 or min(args.samples,args.forks)<1 or args.warmup<0:parser.error('Positive iterations (at most 10000), samples/forks; nonnegative warmups')
    digest=source_digest();classpath,artifacts=prepare_solver_java()
    subprocess.run(['javac','-cp',classpath,'-d',str(WORK/'solver'),str(ROOT/'compat/oracle/ConcurrentOracle.java'),str(ROOT/'compat/oracle/ConcurrentBenchmark.java')],check=True)
    subprocess.run(['cargo','build','--release','--locked','--example','concurrent_benchmark'],cwd=ROOT,check=True)
    bases={'java':['java','-Xms256m','-Xmx256m','-cp',classpath,'ConcurrentBenchmark'],'rust':[str(ROOT/'target/release/examples/concurrent_benchmark')]}
    def run(runtime,cases,iterations,warmup,samples):
        p=subprocess.run(bases[runtime]+list(map(str,[iterations,warmup,samples])),input='\n'.join(map(encode,cases))+'\n',text=True,capture_output=True,timeout=180)
        if p.returncode:raise RuntimeError(f'{runtime}: {p.stderr}')
        return parse_batch(cases,p.stdout,iterations,samples)
    cases=generate();runs={r:run(r,cases,3,0,1) for r in bases}
    checks=[{'case':c,**{r:runs[r][i][0] for r in bases}} for i,c in enumerate(cases)]
    print(f'Concurrent shadow transactions: {len(checks)}/{len(cases)} complete trial/undo/commit cases',flush=True)
    results=[]
    for mode in ['swap','relocate']:
        for count in [16,64]:
            c=workload(count,mode);measurements={r:[] for r in bases}
            for fork in range(args.forks):
                for runtime in (['java','rust'] if (fork+len(results))%2==0 else ['rust','java']):
                    measurements[runtime].extend(run(runtime,[c],args.iterations,args.warmup,args.samples)[0])
            entry=dict(name=f'{mode.title()} · {count} visits',case=c,unit='ms / transaction',samples=measurements)
            for runtime,rows in measurements.items():
                times=[r['elapsed_ns']/1e6/args.iterations for r in rows]
                entry[runtime]=dict(median=statistics.median(times),min=min(times),max=max(times))
            entry['java_over_rust']=entry['java']['median']/entry['rust']['median'];results.append(entry)
            print(f'{entry["name"]}: Rust {entry["rust"]["median"]:.6f}, Java {entry["java"]["median"]:.6f} ms / transaction',flush=True)
    if source_digest()!=digest:raise RuntimeError('Sources changed during measurement')
    report=dict(schema_version=1,generated_at=datetime.now(timezone.utc).isoformat(),source_digest=digest,reference=REF,artifacts=artifacts,config={**vars(args),'seed':SEED},workloads=results,validation=dict(total=len(checks),passed=len(checks),cases=checks),environment=dict(cpu=next((s.split(':',1)[1].strip() for s in open('/proc/cpuinfo') if s.startswith('model name')),'unknown'),platform=platform.platform(),rust=version(['rustc','--version']),java=version(['java','-version'])),methodology='Fixed fully assigned concurrent schedules, with disjoint shared-start groups and distinct source/destination positions (required by the Java move API). Each transaction constructs a public change/swap move, applies it temporarily, copies and validates all nine shadow fields for every visit, undoes it, checks restored fields and ownership, and consumes checksums. Rust uses ListProblem moves and CachedSchedule (the evaluator used by cached stream scoring), explicitly snapshotting/restoring cache and output rows; Java uses public MoveTester with the attributed annotated domain. Rust rebuilds topology on changed edges; Java maintains its shadow listeners. Runtime framework and validation costs differ. No score engine, constraint stream graph, search, full-reference rescoring or model setup is timed. Expected complete field arrays come from independent Python recursion outside timing. A separate committed move is checked outside each timed batch. Parsing, initialization, process startup and output encoding are excluded; move-string decoding, cloning, output copying, checks and checksum work are included equally in scope. Samples repeat the same trial from the same initial state; numbers are medians/ranges of per-transaction batch averages, not latency percentiles. Warmup batches and alternating runtime order across fresh processes are recorded. The machine is shared, without CPU isolation; no JMH, memory, statistical significance or whole-solver speed claim. All 64 generated validation cases and every timing sample must match; errors abort measurement and are never dropped. Fully assigned inputs are an explicit benchmark scope, not a correction of the 21 known Java partial-group errors retained in the separate 128-case correctness comparison. Original cases earn no upstream method credit.')
    (ROOT/'work/concurrent-performance.json').write_text(json.dumps(report,indent=2)+'\n')

if __name__=='__main__':main()
