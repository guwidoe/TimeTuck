#!/usr/bin/env python3
"""Matched checked-sum, sorted-set and merged-map update/full-observation benchmarks."""
from verification_inputs import measurement_digest
from verification_cache import run_measurement
import argparse
from datetime import datetime, timezone
import json
import platform
import random
import statistics
import subprocess
from differential_scores import ROOT, REF, WORK, prepare_java
from benchmark_scores import version
SEED=20261228

def observation(mode,items,slots):
    rows=[items[i] for i in slots if i>=0]
    if mode=='sum':return [sum(v for _,v in rows)]
    if mode=='set':
        values=sorted({v for _,v in rows})
        return [len(values),*values]
    groups={}
    for k,v in rows:groups[k]=groups.get(k,0)+v
    return [len(groups),*[x for k,v in sorted(groups.items()) for x in [k,(v+2**31)%2**32-2**31]]]

def expected(c):
    if not c['id'] or any(x in c['id'] for x in '\t\r\n') or c['mode'] not in ['sum','set','map']:raise ValueError('Identity or mode')
    items,slots,slot,item=c['items'],c['slots'],c['slot'],c['replacement']
    if not 1<=len(items)<=1024 or not 1<=len(slots)<=1024 or type(slot) is not int or not 0<=slot<len(slots):raise ValueError('Pool/slots/selection')
    if any(len(r)!=2 or any(type(v) is not int or not -10000<=v<=10000 for v in r) for r in items):raise ValueError('Bounded integer pairs required')
    if any(type(i) is not int or not -1<=i<len(items) for i in [*slots,item]):raise ValueError('Invalid item reference')
    trial=slots.copy();trial[slot]=item
    return observation(c['mode'],items,slots),observation(c['mode'],items,trial)

def encode_state(s):return ','.join(map(str,s))
def checksum(s):return sum((i+1)*v for i,v in enumerate(s))
def encode(c):
    initial,trial=expected(c)
    return '\t'.join([c['id'],c['mode'],';'.join(encode_state(r) for r in c['items']),encode_state(c['slots']),str(c['slot']),str(c['replacement']),encode_state(initial),encode_state(trial)])
def parse_result(c, line, iterations):
    f = line.split('\t')
    if len(f) != 6 or f[0] != c['id'] or iterations < 1:
        raise ValueError('Response identity/shape/count')
    elapsed, total = map(int, f[1:3])
    if elapsed <= 0:
        raise ValueError('Nonpositive timing')
    observed = [list(map(int, s.split(','))) for s in f[3:]]
    initial, trial = expected(c)
    if observed != [trial, initial, trial]:
        raise ValueError('Full aggregate trial/restore/commit mismatch')
    if total != iterations*(31*checksum(trial)+checksum(initial)):
        raise ValueError('Checksum mismatch')
    return dict(elapsed_ns=elapsed, checksum=total, trial=observed[0], restored=observed[1], committed=observed[2])


def parse_batch(cases, output, iterations, samples):
    lines = output.splitlines()
    if not cases or samples < 1 or len({c['id'] for c in cases}) != len(cases) or len(lines) != len(cases)*samples:
        raise ValueError('Incomplete/ambiguous batch')
    return [[parse_result(c, line, iterations) for line in lines[i*samples:(i+1)*samples]]
            for i, c in enumerate(cases)]


def workload(count,mode):
    if count not in (64,512) or mode not in ('sum','set','map'):raise ValueError('Workload')
    items=[[i//8,(i//2-count//4) if mode=='set' else (37*i)%1001-500] for i in range(count)]
    items.append([count+3,999])
    return dict(id=f'{mode}-{count}',mode=mode,items=items,slots=list(range(count)),slot=count//2,replacement=count)

def generate():
    rng=random.Random(SEED);cases=[]
    for mode in ['sum','set','map']:
        for i in range(32):
            items=[[rng.randrange(-4,5),rng.randrange(-30,31)] for _ in range(rng.randint(1,20))]
            slots=[rng.randrange(-1,len(items)) for _ in range(rng.randint(1,16))]
            if i%8==0:slots=[-1]*len(slots)
            slot=rng.randrange(len(slots));item=-1 if i%4==0 else slots[slot] if i%4==1 else rng.randrange(len(items))
            cases.append(dict(id=f'{mode}-bench-{i}',mode=mode,items=items,slots=slots,slot=slot,replacement=item))
    return cases

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--iterations', type=int, default=200)
    parser.add_argument('--warmup', type=int, default=5)
    parser.add_argument('--samples', type=int, default=7)
    parser.add_argument('--forks', type=int, default=3)
    args = parser.parse_args()
    if not 1 <= args.iterations <= 10000 or min(args.samples, args.forks) < 1 or args.warmup < 0:
        parser.error('Positive iterations (at most 10000), samples/forks; nonnegative warmups')
    digest = measurement_digest('aggregate-performance')
    artifacts = prepare_java()
    classpath = str(WORK / f'timefold-solver-core-{REF["tag"][1:]}.jar')+':'+str(WORK)
    subprocess.run(['javac', '-cp', classpath, '-d', str(WORK), str(ROOT/'compat/oracle/AggregateBenchmark.java')], check=True)
    subprocess.run(['cargo', 'build', '--release', '--locked', '--example', 'aggregate_benchmark'], cwd=ROOT, check=True)
    bases = {'java': ['java', '-Xms256m', '-Xmx256m', '-cp', classpath, 'AggregateBenchmark'],
             'rust': [str(ROOT/'target/release/examples/aggregate_benchmark')]}
    def run(runtime, cases, iterations, warmup, samples):
        p = subprocess.run(bases[runtime]+list(map(str, [iterations, warmup, samples])),
                           input='\n'.join(map(encode, cases))+'\n', text=True, capture_output=True, timeout=180)
        if p.returncode:
            raise RuntimeError(f'{runtime}: {p.stderr}')
        return parse_batch(cases, p.stdout, iterations, samples)
    cases = generate()
    runs = {r: run(r, cases, 3, 0, 1) for r in bases}
    checks = [{'case': c, **{r: runs[r][i][0] for r in bases}} for i, c in enumerate(cases)]
    print(f'Aggregate update/restore observations: {len(checks)}/{len(cases)} validated cases', flush=True)
    results = []
    for mode in ['sum', 'set', 'map']:
        for count in [64, 512]:
            c = workload(count, mode)
            measurements = {r: [] for r in bases}
            for fork in range(args.forks):
                for runtime in (['java', 'rust'] if (fork+len(results)) % 2 == 0 else ['rust', 'java']):
                    measurements[runtime].extend(run(runtime, [c], args.iterations, args.warmup, args.samples)[0])
            entry = dict(name=f'{dict(sum="Checked sum",set="Sorted set",map="Merged map")[mode]} · {count} captures', case=c, unit='ms / update-restore pair', samples=measurements)
            for runtime, rows in measurements.items():
                times = [r['elapsed_ns']/1e6/args.iterations for r in rows]
                entry[runtime] = dict(median=statistics.median(times), min=min(times), max=max(times))
            entry['java_over_rust'] = entry['java']['median']/entry['rust']['median']
            results.append(entry)
            print(f'{entry["name"]}: Rust {entry["rust"]["median"]:.6f}, Java {entry["java"]["median"]:.6f} ms / pair', flush=True)
    if measurement_digest('aggregate-performance') != digest:
        raise RuntimeError('Sources changed during measurement')
    report = dict(schema_version=1, generated_at=datetime.now(timezone.utc).isoformat(), source_digest=digest,
                  reference=REF, artifacts=artifacts, config={**vars(args), 'seed': SEED}, workloads=results,
                  validation=dict(total=len(checks), passed=len(checks), cases=checks),
                  environment=dict(cpu=next((s.split(':',1)[1].strip() for s in open('/proc/cpuinfo') if s.startswith('model name')), 'unknown'),
                                   platform=platform.platform(), rust=version(['rustc','--version']), java=version(['java','-version'])),
                  methodology='Standalone checked i64 sums, naturally sorted sets and sorted maps merged with wrapping i32 addition, using 64 or 512 captured occurrences. '
                  'Each pair replaces one selected capture and restores it by an inverse update. Both results are fully copied into integer arrays, '
                  'checked against independent Python reconstruction after every operation, and consumed in batch checksums. A separate committed update '
                  'is checked outside timing. Sorted sets contain duplicate pairs (32 or 256 distinct values); maps start with 8 or 64 keys and eight '
                  'captures per key, and the replacement introduces a new key. Sum values and all arithmetic intermediates are bounded and representable. '
                  'Includes update/inverse update, finish, full observation allocation/copying, assertions and checksums. Native captured-value APIs and '
                  'Java value handles retain their own update bookkeeping. Native sets explicitly select production SortedSetCollector with an immutable SnapshotSet; existing SetCollector and its SnapshotMap API remain unchanged. Immutable Rust snapshots differ from Java live result views. '
                  'Initialization, parsing, process startup and output encoding are excluded. This does not isolate collector mutation or measure '
                  'stream graph rollback, scoring, solver search, memory, generic merge functions or overflow performance. '
                  'Repeated pairs use the same initial state; warmup batches and raw samples across alternating runtime order in fresh processes are retained. '
                  'Observed ranges are per-pair batch averages, not latency percentiles. Shared machine with no CPU isolation, JMH or statistical-significance claim. '
                  'All 96 original validation cases and every timing sample must pass, otherwise the run aborts. No new upstream-method credit.')
    (ROOT/'work/aggregate-performance.json').write_text(json.dumps(report, indent=2)+'\n')

if __name__ == '__main__':
    raise SystemExit(run_measurement('aggregate-performance', main))
