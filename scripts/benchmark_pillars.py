#!/usr/bin/env python3
"""Original explicit-group pillar transactions checked against Python and Java."""
from verification_inputs import measurement_digest
from verification_cache import run_measurement
import argparse
from copy import deepcopy
from datetime import datetime, timezone
import json
import platform
import random
import statistics
import subprocess
from differential_scores import ROOT, REF, WORK
from prepare_solver_oracle import prepare_solver_java
from benchmark_scores import version

SEED = 20261017
VALUES = [None, -2, -1, 0, 1, 2, 3]


def reference(c):
    """Simultaneous column writes from the original matrix, preserving other cells."""
    rows, columns, left, right = (c[k] for k in ('rows', 'columns', 'left', 'right'))
    if (c['kind'] not in ('c', 's') or not 1 <= len(rows) <= 256
            or any(len(r) != 3 or any(v not in VALUES for v in r) for r in rows)
            or not columns or len(set(columns)) != len(columns)
            or any(col not in range(3) for col in columns)
            or not left or (c['kind'] == 's' and not right)
            or len(set(left + right)) != len(left + right)
            or any(i not in range(len(rows)) for i in left + right)
            or c['target'] not in VALUES
            or (c['kind'] == 'c' and (right or len(columns) != 1))):
        raise ValueError('Invalid case')
    for members in [left, right]:
        if members and any([rows[r][col] for col in columns] != [rows[members[0]][col] for col in columns] for r in members):
            raise ValueError('Nonuniform supplied pillar')
    result = deepcopy(rows)
    for row in range(len(rows)):
        for col in columns:
            if row in left:
                result[row][col] = c['target'] if c['kind'] == 'c' else rows[right[0]][col]
            elif row in right:
                result[row][col] = rows[left[0]][col]
    return result


def encode(c):
    reference(c)
    def scalar(v): return '-' if v is None else str(v)
    def ids(v): return ','.join(map(str, v)) or '-'
    return '\t'.join([c['id'], c['kind'], ids(c['columns']), ids(c['left']), ids(c['right']),
                      scalar(c['target']), ';'.join(','.join(map(scalar, r)) for r in c['rows'])])


def flatten(rows): return [v for r in rows for v in r]


def checksum(state): return sum((i + 1) * (7 if v is None else v) for i, v in enumerate(state))


def parse_result(c, line, iterations):
    fields = line.split('\t')
    if len(fields) != 6 or fields[0] != c['id']:
        raise ValueError('Response identity/shape mismatch')
    elapsed, actual_checksum = map(int, fields[1:3])
    if elapsed <= 0 or iterations < 1:
        raise ValueError('Invalid timing/count')
    states = [[None if s == '-' else int(s) for s in field.split(',')] for field in fields[3:]]
    initial, expected = flatten(c['rows']), flatten(reference(c))
    if states != [expected, initial, expected]:
        raise ValueError('Trial, undo or commit mismatch')
    if actual_checksum != iterations * (31 * checksum(expected) + checksum(initial)):
        raise ValueError('Checksum mismatch')
    return {'elapsed_ns': elapsed, 'checksum': actual_checksum,
            'trial': states[0], 'restored': states[1], 'committed': states[2]}


def parse_batch(cases, output, iterations, samples):
    if not cases or samples < 1 or len({c['id'] for c in cases}) != len(cases):
        raise ValueError('Empty or ambiguous request')
    lines = output.splitlines()
    if len(lines) != len(cases) * samples:
        raise ValueError('Response count mismatch')
    return [[parse_result(c, line, iterations) for line in lines[i*samples:(i+1)*samples]] for i, c in enumerate(cases)]


def generate_case(rng, index, count, kind, width):
    columns = sorted(rng.sample(range(3), width))
    ids = list(range(count)); rng.shuffle(ids)
    # Unequal and noncontiguous selected groups; remaining matching rows are untouched.
    split = rng.randrange(1, count) if kind == 's' else rng.randrange(1, count+1)
    left = ids[:split]
    right = ids[split:split+rng.randrange(1, count-split+1)] if kind == 's' else []
    rows = [[rng.choice(VALUES) for _ in range(3)] for _ in range(count)]
    for members in [left, right]:
        for col in columns:
            v = rng.choice(VALUES)
            for row in members: rows[row][col] = v
    return {'id': f'pillar-{index}', 'kind': kind, 'columns': columns,
            'left': left, 'right': right, 'target': rng.choice(VALUES), 'rows': rows}


def generate():
    rng = random.Random(SEED)
    return [generate_case(rng, i, rng.randrange(2, 13), 'c' if i % 4 == 0 else 's',
                          1 if i % 4 == 0 else i % 3 + 1) for i in range(128)]


def workload(count, mode):
    c = generate_case(random.Random(SEED + count), f'{mode}-{count}', count, 'c' if mode == 'change' else 's', 3 if mode == 'tuple-swap' else 1)
    c['left'] = list(range(0, count//2))
    c['right'] = [] if mode == 'change' else list(range(count//2, count-count//4))
    for col in c['columns']:
        for row in range(count): c['rows'][row][col] = 0 if row < count//2 else 1
    c['target'] = 2
    reference(c)
    return c


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--iterations', type=int, default=1000)
    parser.add_argument('--warmup', type=int, default=5)
    parser.add_argument('--samples', type=int, default=7)
    parser.add_argument('--forks', type=int, default=3)
    args = parser.parse_args()
    if not 1 <= args.iterations <= 1000000 or min(args.samples, args.forks) < 1 or args.warmup < 0:
        parser.error('Use positive iterations (at most 1000000), samples/forks and nonnegative warmups')
    digest = measurement_digest('pillar-performance')
    classpath, artifacts = prepare_solver_java()
    subprocess.run(['javac', '-cp', classpath, '-d', str(WORK/'solver'), str(ROOT/'compat/oracle/PillarMoveBenchmark.java')], check=True)
    subprocess.run(['cargo', 'build', '--release', '--locked', '--example', 'pillar_move_benchmark'], cwd=ROOT, check=True)
    bases = {'java': ['java', '-Xms256m', '-Xmx256m', '-cp', classpath, 'PillarMoveBenchmark'],
             'rust': [str(ROOT/'target/release/examples/pillar_move_benchmark')]}
    def run(runtime, cases, iterations, warmup, samples):
        p = subprocess.run(bases[runtime]+list(map(str, [iterations, warmup, samples])),
                           input='\n'.join(map(encode, cases))+'\n', text=True, capture_output=True, timeout=180)
        if p.returncode: raise RuntimeError(f'{runtime}: {p.stderr}')
        return parse_batch(cases, p.stdout, iterations, samples)
    cases = generate()
    runs = {r: run(r, cases, 2, 0, 1) for r in bases}
    checks = [{'case': c, **{r: runs[r][i][0] for r in bases}} for i, c in enumerate(cases)]
    print(f'Pillar transactions: {len(checks)}/{len(cases)} match independent trial, undo and commit states', flush=True)
    results = []
    for mode in ['change', 'swap', 'tuple-swap']:
        for count in [16, 64]:
            c = workload(count, mode); measurements = {r: [] for r in bases}
            for fork in range(args.forks):
                for runtime in (['java', 'rust'] if (fork+len(results)) % 2 == 0 else ['rust', 'java']):
                    measurements[runtime].extend(run(runtime, [c], args.iterations, args.warmup, args.samples)[0])
            entry = {'name': f'{mode.replace("-", " ").title()} · {count} entities', 'case': c, 'unit': 'ms / transaction', 'samples': measurements}
            for runtime, rows in measurements.items():
                times = [r['elapsed_ns']/1e6/args.iterations for r in rows]
                entry[runtime] = {'median': statistics.median(times), 'min': min(times), 'max': max(times)}
            entry['java_over_rust'] = entry['java']['median']/entry['rust']['median']
            results.append(entry)
            print(f'{entry["name"]}: Rust {entry["rust"]["median"]:.6f}, Java {entry["java"]["median"]:.6f} ms / transaction', flush=True)
    if digest != measurement_digest('pillar-performance'): raise RuntimeError('Sources changed during measurement')
    report = {'schema_version': 1, 'generated_at': datetime.now(timezone.utc).isoformat(),
              'source_digest': digest, 'reference': REF, 'artifacts': artifacts,
              'config': {**vars(args), 'seed': SEED}, 'workloads': results,
              'validation': {'total': len(checks), 'passed': len(checks), 'cases': checks},
              'environment': {'cpu': next((s.split(':', 1)[1].strip() for s in open('/proc/cpuinfo') if s.startswith('model name')), 'unknown'),
                              'platform': platform.platform(), 'rust': version(['rustc', '--version']), 'java': version(['java', '-version'])},
              'methodology': 'Original explicit-group basic-variable transactions, with three optional integer columns per entity. Supplied members and columns are identical; no automatic grouping, schema-provider enumeration, random sampling, scoring or search is timed. Change writes one column; swap exchanges one or three columns between unequal groups, leaving some matching members untouched (subpillars). Every iteration constructs native Pillars/moves or Java Samples/public Moves, temporarily applies the move, copies and checks the full trial state, undoes it, copies and checks the restored state and consumes both checksums. Rust includes Pillar structural/uniformity checks and FiniteProblem domain/pin validation; Java includes public MoveTester infrastructure. These validation/framework costs differ. Parsing, model/test-context setup, expected states, startup, output encoding and a separate committed-state check are outside timing. Each sample repeats the same transaction; medians and ranges are per-transaction batch averages, not latency percentiles. Warmup batches, alternating runtime order and fresh processes are recorded; JVM compilation/GC may still affect samples. Python independently verifies trial, restored and committed states and every batch checksum, including 128 seeded small cases with optional values, no-ops, noncontiguous members and 1–3 columns. All cases are original and earn no upstream-method credit. No general solver speedup, schema-provider or memory claim.'}
    (ROOT/'work/pillar-performance.json').write_text(json.dumps(report, indent=2)+'\n')

if __name__ == '__main__':
    raise SystemExit(run_measurement('pillar-performance', main))
