"""Job-specific inputs. No timestamps, Git revisions, or generated reports in keys."""
import ast
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
# name: (runner, native entry points, Java entry points). Differential runners
# also execute their imported check modules; discover those adapter roots below.
MEASUREMENTS = {
    'differential': ('differential_scores', 'score_oracle decimal_score_oracle', 'ScoreOracle DecimalScoreOracle'),
    'performance': ('benchmark_scores', 'score_benchmark', 'ScoreBenchmark'),
    'solver-differential': ('differential_solver', '', ''),
    'list-differential': ('differential_lists', 'list_move_oracle', 'ListMoveOracle'),
    'stream-differential': ('differential_streams', '', ''),
    'solver-performance': ('benchmark_solver', 'solver_benchmark', 'SolverBenchmark'),
    'heuristic-performance': ('benchmark_heuristics', 'heuristic_benchmark', 'HeuristicBenchmark'),
    'construction-performance': ('benchmark_construction', 'construction_benchmark', 'ConstructionBenchmark'),
    'repair-performance': ('benchmark_repair', 'repair_benchmark', 'RepairBenchmark'),
    'list-repair-performance': ('benchmark_list_repair', 'list_repair_benchmark', 'ListRepairBenchmark'),
    'list-performance': ('benchmark_lists', 'list_solver_benchmark', 'ListSolverBenchmark'),
    'kopt-performance': ('benchmark_kopt', 'k_opt_benchmark', 'KOptBenchmark'),
    'stream-performance': ('benchmark_streams', 'stream_benchmark', 'StreamBenchmark'),
    'concurrent-performance': ('benchmark_concurrent', 'concurrent_benchmark', 'ConcurrentBenchmark'),
    'sequence-performance': ('benchmark_sequences', 'sequence_benchmark', 'SequenceBenchmark'),
    'aggregate-performance': ('benchmark_aggregates', 'aggregate_benchmark', 'AggregateBenchmark'),
    'pillar-performance': ('benchmark_pillars', 'pillar_move_benchmark', 'PillarMoveBenchmark'),
}


def rust_dependencies_text(source):
    source = re.sub(r'"(?:\\.|[^"\\])*"|/\*[\s\S]*?\*/|//[^\n]*', ' ', source)
    pattern = r'#\[cfg\(test\)\]\s*(?:pub\s+)?mod\s+\w+\s*\{'
    while match := re.search(pattern, source):
        depth, end = 1, match.end()
        while end < len(source) and depth:
            depth += (source[end] == '{') - (source[end] == '}')
            end += 1
        source = source[:match.start()] + source[end:]
    return source


def files(pattern, root=ROOT):
    return {p.relative_to(root).as_posix() for p in root.glob(pattern) if p.is_file()}


def digest(paths, root=ROOT, extra=None):
    h = hashlib.sha256(json.dumps(extra, sort_keys=True).encode())
    for name in sorted(set(paths)):
        path = root / name
        h.update(name.encode() + b'\0')
        h.update(hashlib.sha256(path.read_bytes()).digest() if path.is_file() else b'missing')
    return h.hexdigest()


def python_inputs(entry, root=ROOT):
    """Follow local imports, including imports inside executed check functions."""
    pending, found = [entry], set()
    while pending:
        name = pending.pop()
        if name in found or not (root / name).is_file():
            continue
        found.add(name)
        for node in ast.walk(ast.parse((root / name).read_text())):
            modules = ([node.module] if isinstance(node, ast.ImportFrom) else
                       [a.name for a in node.names] if isinstance(node, ast.Import) else [])
            pending.extend(f'scripts/{m.replace(".", "/")}.py' for m in modules if m)
    return found


def cargo_inputs(root=ROOT):
    return ({'Cargo.toml', 'Cargo.lock', 'rust-toolchain', 'rust-toolchain.toml'} |
            files('crates/**/Cargo.toml', root) | files('crates/**/build.rs', root) |
            files('.cargo/*', root))


def library_inputs(root=ROOT):
    return cargo_inputs(root) | files('crates/*/src/**/*', root)


def selected_library_inputs(entries, root=ROOT):
    """Development dependency scan, at module granularity (not a Rust compiler).

    Follow named types/functions/modules and out-of-line implementations. Glob
    imports resolve against identifiers used in the target. The facade is hashed but its
    reexports do not drag every unrelated module into every target.
    """
    sources = {p for p in files('crates/solver/src/**/*.rs', root) if not p.endswith('_tests.rs')}
    facade = 'crates/solver/src/lib.rs'
    if facade in entries:
        return library_inputs(root)
    owners, texts = {}, {}
    for name in sources - {facade}:
        source = rust_dependencies_text((root / name).read_text())
        texts[name] = source
        symbols = {Path(name).stem}
        symbols.update(re.findall(r'^pub(?:\([^)]*\))?\s+(?:struct|enum|trait|type)\s+(\w+)', source, re.M))
        symbols.update(re.findall(r'^pub(?:\([^)]*\))?\s+(?:async\s+)?fn\s+(\w+)', source, re.M))
        for symbol in symbols:
            owners.setdefault(symbol, set()).add(name)
    if (root / facade).is_file():
        for module, exports in re.findall(r'pub use (\w+)::([^;]+);', (root / facade).read_text()):
            for symbol in re.findall(r'\b\w+\b', exports):
                owner = f'crates/solver/src/{module}.rs'
                if owner in sources: owners.setdefault(symbol, set()).add(owner)
    # Inherent/trait implementations may live outside their type's module.
    for name, source in texts.items():
        for header in re.findall(r'^impl\b([^{}]+)\{', source, re.M):
            header = header.strip()
            if header.startswith('<'):
                depth = 0
                for i, char in enumerate(header):
                    depth += (char == '<') - (char == '>')
                    if depth == 0:
                        header = header[i+1:].strip()
                        break
            header = re.split(r'\bwhere\b', header)[0]
            target = re.split(r'\bfor\b', header)[-1].strip()
            symbol = re.match(r'(?:&\s*)?(\w+)', target)
            if symbol and symbol[1] in owners:
                owners[symbol[1]].add(name)
    selected, pending = set(), list(entries)
    while pending:
        name = pending.pop()
        if name in selected or not (root / name).is_file(): continue
        selected.add(name)
        pending.extend(rust_entry_inputs(name, root) - selected)
        source = texts[name] if name in texts else rust_dependencies_text((root / name).read_text())
        imports = ' '.join(re.findall(r'\buse\s+(?:crate|super|timetuck)::([^;]+);', source))
        words = set(re.findall(r'\b\w+\b', imports))
        words.update(re.findall(r'\b(?:crate|super|timetuck)::(\w+)', source))
        words.update(re.findall(r'\b[A-Z][A-Za-z0-9_]+\b', source))
        if re.search(r'\buse\s+(?:crate|timetuck|super)\b[^;]*\*', source):
            calls = set(re.findall(r'(?<![.\w])([a-z_]\w*)\s*(?:::<[^;]+?>)?\s*\(', source))
            local = set(re.findall(r'\bfn\s+(\w+)', source))
            words.update(calls - local)
        for word in words:
            pending.extend(owners.get(word, set()) - selected)
    return cargo_inputs(root) | {facade} | {p for p in selected if p.startswith('crates/solver/src/')}


def rust_entry_inputs(entry, root=ROOT):
    """One test/example, its local modules and literal include files."""
    pending, found = [entry], set()
    while pending:
        name = pending.pop()
        if name in found:
            continue
        found.add(name)
        path = root / name
        if not path.is_file() or path.suffix != '.rs':
            continue
        source = path.read_text()
        base = path.parent if path.name in ('mod.rs', 'lib.rs') or path.parent.name in ('tests', 'examples') else path.with_suffix('')
        for module in re.findall(r'\bmod\s+(\w+)\s*;', source):
            for child in [base / f'{module}.rs', base / module / 'mod.rs']:
                if child.is_file(): pending.append(child.relative_to(root).as_posix())
        for literal in re.findall(r'(?:include(?:_str|_bytes)?!\s*\(\s*|#\[path\s*=\s*)"([^"]+)"', source):
            pending.append((path.parent / literal).resolve().relative_to(root.resolve()).as_posix())
    return found


def rust_test_inputs(entry=None, root=ROOT):
    entries = rust_entry_inputs(entry, root) if entry else files('crates/*/tests/**/*', root) | files('crates/*/examples/**/*', root)
    return ((selected_library_inputs(entries, root) if entry else library_inputs(root)) |
            {'scripts/run_rust_tests.py', 'scripts/verification_cache.py',
                                   'scripts/verification_inputs.py'} |
            entries)


def measurement_inputs(job, root=ROOT):
    script, examples, java = MEASUREMENTS[job]
    selected = python_inputs(f'scripts/{script}.py', root)
    # These modules implement key/cache plumbing, not the measured behavior.
    # Hash them explicitly without following their imports into report/UI tools.
    selected |= {'scripts/verification_inputs.py', 'scripts/verification_cache.py'}
    if not examples:
        source = '\n'.join((root / p).read_text() for p in selected if (root / p).is_file())
        examples = ' '.join(re.findall(r'target/release/examples/(\w+)', source))
        java = ' '.join(re.findall(r'compat/oracle/(\w+)\.java', source))
    entries = set()
    for example in examples.split():
        entries |= rust_entry_inputs(f'crates/solver/examples/{example}.rs', root)
    selected |= entries
    pending, found = java.split(), set()
    adapters = {p.stem: p for p in (root / 'compat/oracle').glob('*.java')}
    while pending:
        name = pending.pop()
        if name in found: continue
        found.add(name)
        selected.add(f'compat/oracle/{name}.java')
        if name in adapters:
            words = set(re.findall(r'\b\w+\b', adapters[name].read_text()))
            pending.extend(words & adapters.keys() - found)
    return (selected | selected_library_inputs(entries, root) |
            {'compat/upstream.json', 'compat/oracle/pom.xml'})


def measurement_digest(job, root=ROOT):
    return digest(measurement_inputs(job, root), root, extra={'job': job, 'schema': 1})


def rust_digest(root=ROOT):
    return digest(rust_test_inputs(root=root), root, extra='rust-tests-v2')


def report_digest(root=ROOT):
    # Report freshness includes the ledger and all job inputs; it does NOT serve
    # as the Rust execution key or as an individual measurement key.
    paths = rust_test_inputs(root=root) | {'compat/provenance.json', 'compat/exclusions.json',
             'compat/upstream.json', 'scripts/compat_report.py', 'scripts/upstream_inventory.py'}
    for job in MEASUREMENTS:
        paths |= measurement_inputs(job, root)
    return digest(paths, root, extra='report-v2')
