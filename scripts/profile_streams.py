#!/usr/bin/env python3
"""Instrument an isolated native source copy; never use these times as benchmarks."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
from datetime import datetime, timezone
from benchmark_streams import workload, parse_result
from differential_solver import encode
from differential_scores import ROOT, source_digest

INSTRUMENTATION = r'''
use std::{cell::RefCell,collections::BTreeMap,time::Instant};
thread_local! {static STATS:RefCell<BTreeMap<&'static str,(u64,u128)>>=RefCell::new(BTreeMap::new());}
pub struct Span(&'static str,Instant);
impl Span {pub fn new(name:&'static str)->Self {Self(name,Instant::now())}}
impl Drop for Span {fn drop(&mut self){let elapsed=self.1.elapsed().as_nanos();STATS.with(|s|{let mut s=s.borrow_mut();let v=s.entry(self.0).or_default();v.0+=1;v.1+=elapsed;});}}
pub fn count(name:&'static str){STATS.with(|s|s.borrow_mut().entry(name).or_default().0+=1);}
pub fn dump(){STATS.with(|s|{for(k,(count,ns))in s.borrow().iter(){eprintln!("{k}\t{count}\t{ns}");}});}
'''


def replace_once(path, old, new):
    source = path.read_text()
    if source.count(old) != 1:
        raise ValueError(f'Instrumentation point changed in {path.name}; review the profiler')
    path.write_text(source.replace(old, new))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--variables', type=int, choices=[32, 96], default=96)
    parser.add_argument('--algorithm', choices=['hill', 'tabu'], default='hill')
    parser.add_argument('--steps', type=int, default=50)
    args = parser.parse_args()
    if args.steps < 1:
        parser.error('steps must be positive')
    work = ROOT / 'work'
    work.mkdir(exist_ok=True)
    digest = source_digest()
    with tempfile.TemporaryDirectory(prefix='native-profile-', dir=work) as directory:
        copy = Path(directory)
        for name in ['Cargo.toml', 'Cargo.lock']:
            shutil.copy2(ROOT / name, copy / name)
        crate = copy / 'crates/solver'
        shutil.copytree(ROOT / 'crates/solver', crate)
        (crate / 'src/profiling.rs').write_text(INSTRUMENTATION)
        with (crate / 'src/lib.rs').open('a') as file:
            file.write('\npub mod profiling;\n')
        graph = crate / 'src/stream_graph.rs'
        replace_once(graph, 'trait Node<S: AdditiveScore> {',
                     "trait Node<S: AdditiveScore> {\n    fn profile_name(&self)->&'static str {std::any::type_name::<Self>()}")
        replace_once(graph, 'let outputs = self.graph.nodes[id].apply(edge.right, &event)?;',
                     'let outputs = { let _span=crate::profiling::Span::new(self.graph.nodes[id].profile_name()); self.graph.nodes[id].apply(edge.right, &event)? };')
        for marker, label in [
            ('fn checkpoint(&mut self, id: usize) {', 'graph checkpoint'),
            ('pub(crate) fn capture(&self) -> Result<GraphState<S>, GraphError> {', 'graph capture'),
            ('fn drop(&mut self) {', 'graph undo/drop'),
        ]:
            replace_once(graph, marker, marker + f'\n        let _span=crate::profiling::Span::new("{label}");')
        replace_once(crate / 'src/snapshot_map.rs', '    Arc::new(Node {',
                     '    crate::profiling::count("AVL node allocations");\n    Arc::new(Node {')
        replace_once(crate / 'examples/stream_benchmark.rs', '    Ok(())\n}',
                     '    timetuck::profiling::dump();\n    Ok(())\n}')
        target = work / 'native-profile-target'
        build = subprocess.run(['cargo', 'build', '--release', '--example', 'stream_benchmark', '--locked'],
                               cwd=copy, env={**os.environ, 'CARGO_TARGET_DIR': str(target)},
                               text=True, capture_output=True, timeout=180)
        (work / 'native-profile-build.log').write_text(build.stdout + build.stderr)
        build.check_returncode()
        case = workload(args.variables)
        run = subprocess.run([str(target / 'release/examples/stream_benchmark'), args.algorithm,
                              str(args.steps), '0', '1'], input=encode(case) + '\n',
                             text=True, capture_output=True, timeout=180, check=True)
        lines = run.stdout.splitlines()
        if len(lines) != 1:
            raise ValueError('Expected one instrumented solve')
        result = parse_result(case, lines[0])
        entries = []
        for line in run.stderr.splitlines():
            label, calls, elapsed = line.split('\t')
            entries.append({'name': label, 'calls': int(calls), 'elapsed_ns': int(elapsed)})
        if not entries:
            raise ValueError('Missing profiler output')
    if source_digest() != digest:
        raise RuntimeError('Source changed during profiling')
    entries.sort(key=lambda entry: entry['elapsed_ns'], reverse=True)
    report = {'schema_version': 1, 'generated_at': datetime.now(timezone.utc).isoformat(),
              'source_digest': digest, 'config': vars(args), 'case': case, 'result': result,
              'entries': entries,
              'scope': 'Original native benchmark with timers around node apply, checkpoint, saved-candidate capture and undo; count every AVL node allocation. One instrumented solve including graph setup. Timer/counter overhead distorts runtime. This is coarse hotspot evidence, not a before/after or Java timing benchmark, and gives no upstream-method credit. Main Rust sources remain unmodified; instrumentation compiles only an isolated copy.'}
    (work / 'native-stream-profile.json').write_text(json.dumps(report, indent=2) + '\n')
    for entry in entries:
        print(f'{entry["elapsed_ns"]/1_000_000:9.3f} ms {entry["calls"]:9} {entry["name"]}')


if __name__ == '__main__':
    main()
