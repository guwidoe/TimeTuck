#!/usr/bin/env python3
"""Matched sequence collector replacement/restore plus complete observations."""
import argparse
from datetime import datetime, timezone
import json
import platform
import random
import statistics
import subprocess
from differential_scores import ROOT, REF, WORK, source_digest, prepare_java
from benchmark_scores import version
from sequence_collector_checks import components

SEED = 20261221


def observation(points, slots):
    groups = components(points, {i for i in slots if i >= 0})
    out = [len(groups), max(0, len(groups)-1)] + [-1] * (3*len(points))
    for q, group in enumerate(groups):
        for i in group:
            out[2+3*i:5+3*i] = [1, points[i], q]
        out += [len(group), points[group[-1]]-points[group[0]]+1,
                points[group[0]], points[group[-1]], int(q == 0), int(q+1 == len(groups)),
                q-1 if q else -1, q if q+1 < len(groups) else -1]
    for q, (a, b) in enumerate(zip(groups, groups[1:])):
        out += [points[b[0]]-points[a[-1]], points[a[-1]], points[b[0]],
                int(q == 0), int(q+2 == len(groups))]
    return out


def expected(c):
    points, slots, slot, item = c['points'], c['slots'], c['slot'], c['replacement']
    if not c['id'] or any(x in c['id'] for x in '\t\r\n'):
        raise ValueError('Invalid identity')
    if not 1 <= len(points) <= 1024 or not slots or not 0 <= slot < len(slots):
        raise ValueError('Pool or selected slot outside benchmark scope')
    if any(type(v) is not int or not -10000 <= v <= 10000 for v in points):
        raise ValueError('Bounded integer indices required; overflow is separate evidence')
    if any(type(i) is not int or not -1 <= i < len(points) for i in [*slots, item]):
        raise ValueError('Invalid logical item')
    trial = slots.copy()
    trial[slot] = item
    return observation(points, slots), observation(points, trial)


def encode_state(s): return ','.join(map(str, s))
def checksum(s): return sum((i+1)*v for i, v in enumerate(s))
def encode(c):
    initial, trial = expected(c)
    return '\t'.join([c['id'], encode_state(c['points']), encode_state(c['slots']), str(c['slot']),
                      str(c['replacement']), encode_state(initial), encode_state(trial)])


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
        raise ValueError('Full sequence trial/restore/commit mismatch')
    if total != iterations*(31*checksum(trial)+checksum(initial)):
        raise ValueError('Checksum mismatch')
    return dict(elapsed_ns=elapsed, checksum=total, trial=observed[0], restored=observed[1], committed=observed[2])


def parse_batch(cases, output, iterations, samples):
    lines = output.splitlines()
    if not cases or samples < 1 or len({c['id'] for c in cases}) != len(cases) or len(lines) != len(cases)*samples:
        raise ValueError('Incomplete/ambiguous batch')
    return [[parse_result(c, line, iterations) for line in lines[i*samples:(i+1)*samples]]
            for i, c in enumerate(cases)]


def workload(count, mode):
    if count not in (64, 512) or mode not in ('dense', 'fragmented'):
        raise ValueError('Workload')
    points = list(range(count)) if mode == 'dense' else [3*i for i in range(count)]
    points.append(3*count if mode == 'dense' else 1)
    return dict(id=f'{mode}-{count}', points=points, slots=list(range(count)),
                slot=count//2, replacement=count)


def generate():
    rng = random.Random(SEED)
    cases = []
    for i in range(64):
        points = [rng.randrange(-12, 13) for _ in range(rng.randint(1, 20))]
        slots = [rng.randrange(-1, len(points)) for _ in range(rng.randint(1, 16))]
        if i % 8 == 0:
            slots = [-1]*len(slots)
        slot = rng.randrange(len(slots))
        item = -1 if i % 4 == 0 else slots[slot] if i % 4 == 1 else rng.randrange(len(points))
        cases.append(dict(id=f'sequence-bench-{i}', points=points, slots=slots, slot=slot, replacement=item))
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
    digest = source_digest()
    artifacts = prepare_java()
    classpath = str(WORK / f'timefold-solver-core-{REF["tag"][1:]}.jar')+':'+str(WORK)
    subprocess.run(['javac', '-cp', classpath, '-d', str(WORK), str(ROOT/'compat/oracle/SequenceBenchmark.java')], check=True)
    subprocess.run(['cargo', 'build', '--release', '--locked', '--example', 'sequence_benchmark'], cwd=ROOT, check=True)
    bases = {'java': ['java', '-Xms256m', '-Xmx256m', '-cp', classpath, 'SequenceBenchmark'],
             'rust': [str(ROOT/'target/release/examples/sequence_benchmark')]}
    def run(runtime, cases, iterations, warmup, samples):
        p = subprocess.run(bases[runtime]+list(map(str, [iterations, warmup, samples])),
                           input='\n'.join(map(encode, cases))+'\n', text=True, capture_output=True, timeout=180)
        if p.returncode:
            raise RuntimeError(f'{runtime}: {p.stderr}')
        return parse_batch(cases, p.stdout, iterations, samples)
    cases = generate()
    runs = {r: run(r, cases, 3, 0, 1) for r in bases}
    checks = [{'case': c, **{r: runs[r][i][0] for r in bases}} for i, c in enumerate(cases)]
    print(f'Sequence update/restore observations: {len(checks)}/{len(cases)} validated cases', flush=True)
    results = []
    for mode in ['dense', 'fragmented']:
        for count in [64, 512]:
            c = workload(count, mode)
            measurements = {r: [] for r in bases}
            for fork in range(args.forks):
                for runtime in (['java', 'rust'] if (fork+len(results)) % 2 == 0 else ['rust', 'java']):
                    measurements[runtime].extend(run(runtime, [c], args.iterations, args.warmup, args.samples)[0])
            entry = dict(name=f'{mode.title()} · {count} items', case=c, unit='ms / update-restore pair', samples=measurements)
            for runtime, rows in measurements.items():
                times = [r['elapsed_ns']/1e6/args.iterations for r in rows]
                entry[runtime] = dict(median=statistics.median(times), min=min(times), max=max(times))
            entry['java_over_rust'] = entry['java']['median']/entry['rust']['median']
            results.append(entry)
            print(f'{entry["name"]}: Rust {entry["rust"]["median"]:.6f}, Java {entry["java"]["median"]:.6f} ms / pair', flush=True)
    if source_digest() != digest:
        raise RuntimeError('Sources changed during measurement')
    report = dict(schema_version=1, generated_at=datetime.now(timezone.utc).isoformat(), source_digest=digest,
                  reference=REF, artifacts=artifacts, config={**vars(args), 'seed': SEED}, workloads=results,
                  validation=dict(total=len(checks), passed=len(checks), cases=checks),
                  environment=dict(cpu=next((s.split(':',1)[1].strip() for s in open('/proc/cpuinfo') if s.startswith('model name')), 'unknown'),
                                   platform=platform.platform(), rust=version(['rustc','--version']), java=version(['java','-version'])),
                  methodology='Standalone collector replacement followed by inverse update, with full result observation after both operations. '
                  'Dense consecutive and fragmented spaced indices use 64 or 512 distinct items. Native SequenceCollector owns immutable '
                  'snapshots and currently rebuilds sequence boundaries at finish; Java public collector handles expose maintained live chains. '
                  'Both drivers copy observations into arrays indexed by logical ID, validate membership and index/rank plus every sequence '
                  'count, length, endpoint index, first/last flag and neighboring-break ordinal, and every break distance/endpoint/flag. '
                  'Equal-index identity tie order is intentionally canonicalized away; the separate sequence differential checks exact endpoint '
                  'identities against each runtime order. Every timed update and restoration must match arrays computed by independent Python '
                  'pairwise connectivity outside timing. A separately committed update is also checked outside each batch. '
                  'Includes replacement, inverse update, finish/materialization, array allocation/copying, assertions and consumed checksums. '
                  'Initialization, parsing, process startup and output encoding are excluded. Runtime validation and allocation costs differ. '
                  'This is an update-plus-full-observation benchmark, not isolated insertion/removal speed, stream graph rollback, scoring or search. '
                  'Repeated trials use the same starting state. Warmup batches, raw samples and alternating order across fresh processes are '
                  'recorded; ranges are per-pair batch averages, not latency percentiles. Shared machine, no CPU isolation, JMH, memory or '
                  'statistical-significance claim. All 64 generated validation cases and every timing sample must pass; errors abort. '
                  'Bounded integer indices exclude the known Java overflow divergence, which remains explicit in separate correctness evidence. '
                  'Original cases add no upstream-method credit.')
    (ROOT/'work/sequence-performance.json').write_text(json.dumps(report, indent=2)+'\n')

if __name__ == '__main__': main()
