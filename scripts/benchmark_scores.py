#!/usr/bin/env python3
"""Run matched warm score microbenchmarks; never claim whole-solver performance."""
import argparse
import json
import os
import platform
import statistics
import subprocess
from datetime import datetime, timezone
from pathlib import Path
from differential_scores import ROOT, WORK, JAR, REF, prepare_java, source_digest


def version(command):
    return subprocess.run(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=True).stdout.strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--iterations', type=int, default=5_000_000)
    parser.add_argument('--samples', type=int, default=7)
    parser.add_argument('--warmup', type=int, default=5)
    parser.add_argument('--forks', type=int, default=3)
    args = parser.parse_args()
    if min(args.iterations, args.samples, args.forks) < 1 or args.warmup < 0: parser.error('Counts must be positive (warmup may be zero)')
    artifact = prepare_java()
    subprocess.run(['javac', '-cp', str(JAR), '-d', str(WORK), str(ROOT / 'compat/oracle/ScoreBenchmark.java')], check=True)
    subprocess.run(['cargo', 'build', '--release', '--example', 'score_benchmark', '--locked'], cwd=ROOT, check=True)
    results = []
    for index, kind in enumerate(['simple', 'hard_soft', 'hard_medium_soft']):
        measurements = {'rust': [], 'java': []}
        checksums = set()
        common = [kind, str(args.iterations), '20261001', str(args.warmup), str(args.samples)]
        commands = {'java': ['java', '-Xms256m', '-Xmx256m', '-cp', f'{WORK}:{JAR}', 'ScoreBenchmark', *common],
                    'rust': [str(ROOT / 'target/release/examples/score_benchmark'), *common]}
        # Alternate process order across forks; each fresh process warms itself.
        for fork in range(args.forks):
            order = ['java', 'rust'] if (fork + index) % 2 == 0 else ['rust', 'java']
            for runtime in order:
                output = subprocess.run(commands[runtime], text=True, capture_output=True, check=True).stdout
                rows = [row.split('\t') for row in output.splitlines()]
                if len(rows) != args.samples: raise RuntimeError('Missing timing samples')
                for elapsed, checksum in rows:
                    measurements[runtime].append(int(elapsed) / args.iterations)
                    checksums.add(int(checksum))
        if len(checksums) != 1: raise RuntimeError(f'Java/Rust checksum mismatch for {kind}')
        entry = {'name': kind, 'unit': 'ns / iteration', 'checksum': checksums.pop(), 'samples': measurements}
        for runtime, samples in measurements.items():
            entry[runtime] = {'median': statistics.median(samples), 'min': min(samples), 'max': max(samples)}
        entry['java_over_rust'] = entry['java']['median'] / entry['rust']['median']
        results.append(entry)
        print(f'{kind}: Rust {entry["rust"]["median"]:.2f}, Java {entry["java"]["median"]:.2f} ns/iteration; ratio {entry["java_over_rust"]:.2f}x', flush=True)
    cpu = next((s.split(':', 1)[1].strip() for s in Path('/proc/cpuinfo').read_text().splitlines() if s.startswith('model name')), 'unknown')
    report = {'schema_version': 1, 'generated_at': datetime.now(timezone.utc).isoformat(), 'source_digest': source_digest(),
              'reference': REF, 'artifact': artifact, 'config': {**vars(args), 'seed': 20261001}, 'workloads': results,
              'environment': {'platform': platform.platform(), 'cpu': cpu, 'logical_cpus': os.cpu_count(),
                              'rust': version(['rustc', '-Vv']), 'java': version(['java', '-version'])},
              'scope': 'Score arithmetic and lexicographic comparison only. Not solver throughput, solution quality, memory, or startup time.',
              'methodology': 'Identical seeded LCG inputs, score addition and conditional subtraction per iteration. Rust release; warmed JVM. Alternating fresh process forks; medians of all samples. Checksums must match. Startup and compilation excluded. Shared machine; no CPU isolation. Not JMH or a statistical significance claim.'}
    (ROOT / 'work/performance.json').write_text(json.dumps(report, indent=2) + '\n')

if __name__ == '__main__': main()
