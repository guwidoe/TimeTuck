"""Job-specific inputs. No timestamps, Git revisions, or generated reports in keys."""
import ast
import hashlib
from functools import lru_cache
import json
from pathlib import Path
import re
from types import MappingProxyType

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
    'mixed-performance': ('benchmark_mixed', 'mixed_solver_benchmark', 'MixedSolverBenchmark'),
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


# This remains a development heuristic, not a Rust parser. Only exact cfg(test)
# attributes are removed; compound/unknown conditions remain conservative.
RUST_LITERALS = re.compile(
    r'//[^\n]*|/\*[\s\S]*?\*/|(?:br|r)(?P<hashes>\#*)"[\s\S]*?"(?P=hashes)'
    r'|(?:b)?"(?:\\.|[^"\\])*"|(?:b)?\'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^\'\\])\'')


@lru_cache(maxsize=2048)
def rust_dependencies_text(source):
    return RUST_LITERALS.sub(lambda m: ' ' * len(m[0]), source)


@lru_cache(maxsize=2048)
def production_rust(source):
    """Project library source onto items present without cfg(test)."""
    masked = rust_dependencies_text(source)
    spans = []
    attributes = r'(?:#\[[^\]]*\]\s*)+'
    for match in re.finditer(attributes, masked):
        if not re.search(r'#\[\s*cfg\s*\(\s*test\s*\)\s*\]', match[0]):
            continue
        if spans and match.start() < spans[-1][1]:
            continue
        # Skip a declaration/signature, then either its terminating semicolon or
        # balanced body. Strings/comments are masked, preserving source offsets.
        round_depth = square_depth = angle_depth = 0
        end = match.end()
        while end < len(masked):
            char = masked[end]
            if char == '(': round_depth += 1
            elif char == ')': round_depth -= 1
            elif char == '[': square_depth += 1
            elif char == ']': square_depth -= 1
            elif char == '<': angle_depth += 1
            elif char == '>' and masked[end - 1] not in '-=': angle_depth = max(0, angle_depth - 1)
            if not (round_depth or square_depth or angle_depth) and char in '{;':
                if char == '{':
                    depth = 1
                    end += 1
                    while end < len(masked) and depth:
                        depth += (masked[end] == '{') - (masked[end] == '}')
                        end += 1
                    if depth: break  # Unsupported/unbalanced syntax: retain it.
                else:
                    end += 1
                spans.append((match.start(), end))
                break
            end += 1
    for start, end in reversed(spans):
        source = source[:start] + source[end:]
    return source


@lru_cache(maxsize=2048)
def production_content(source):
    """Normalize trivia while retaining literal contents exactly."""
    source = production_rust(source)
    # Module contents are hashed through selected dependencies. A new unused
    # module declaration alone does not change an existing job's behavior.
    # Attributes stay hashed (including cfg/path), as does all other code.
    declarations = list(re.finditer(r'\b(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*;', rust_dependencies_text(source)))
    for declaration in reversed(declarations):
        source = source[:declaration.start()] + source[declaration.end():]
    chunks, cursor = [], 0
    for match in RUST_LITERALS.finditer(source):
        chunks.append(re.sub(r'\s+', ' ', source[cursor:match.start()]))
        chunks.append(' ' if match[0].startswith('/') else match[0])
        cursor = match.end()
    chunks.append(re.sub(r'\s+', ' ', source[cursor:]))
    return ''.join(chunks).strip()


def files(pattern, root=ROOT):
    return {p.relative_to(root).as_posix() for p in root.glob(pattern) if p.is_file()}


FACADE_INPUT = '@rust-facade:'
LIBRARY_INPUT = '@rust-library:'
DOCTEST_INPUT = '@rust-doctest:'


def facade_content(source, symbols):
    """Project a crate facade onto used names; retain other root code/attributes.

    This is the same deliberately selective development heuristic as the module
    scan, not Rust name resolution. Unknown export syntax remains conservative.
    """
    source = production_rust(source)
    source = re.sub(r'"(?:\\.|[^"\\])*"|/\*[\s\S]*?\*/|//[^\n]*',
                    lambda m: ' ' if m[0].startswith('/') else m[0], source)
    def declaration(match):
        attributes, module, exports = match.group(1, 2, 3)
        if module:
            return match[0] if module in symbols else ''
        if '*' in exports or exports.count('{') > 1:
            return match[0]
        prefix, _, members = exports.partition('{')
        if members:
            items = [v.strip() for v in members.rstrip().removesuffix('}').split(',') if v.strip()]
        else:
            prefix, _, member = exports.rpartition('::')
            prefix += '::'
            items = [member]
        kept = []
        for item in items:
            name = re.split(r'\s+as\s+', item)[-1].strip()
            if name == 'self': name = prefix.rstrip(':').split('::')[-1]
            if name in symbols: kept.append(item)
        return f'{attributes}pub use {prefix}{{{",".join(sorted(kept))}}};' if kept else ''
    # Attributes belong to the declaration; crate-level #![...] is retained.
    source = re.sub(r'((?:#\[[^\]]*\]\s*)*)(?:(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;|pub\s+use\s+([^;]+);)',
                    declaration, source)
    # Preserve whitespace inside ordinary strings when normalizing formatting.
    return re.sub(r'"(?:\\.|[^"\\])*"|\s+', lambda m: m[0] if m[0].startswith('"') else ' ', source).strip()


def digest(paths, root=ROOT, extra=None):
    h = hashlib.sha256(json.dumps(extra, sort_keys=True).encode())
    for name in sorted(set(paths)):
        h.update(name.encode() + b'\0')
        if name.startswith(FACADE_INPUT):
            path = root / 'crates/solver/src/lib.rs'
            data = facade_content(path.read_text(), set(json.loads(name[len(FACADE_INPUT):]))).encode() if path.is_file() else b'missing'
            h.update(hashlib.sha256(data).digest())
        elif name.startswith(DOCTEST_INPUT):
            path = root / name[len(DOCTEST_INPUT):]
            data = production_rust(path.read_text()).encode() if path.is_file() else b'missing'
            h.update(hashlib.sha256(data).digest())
        elif name.startswith(LIBRARY_INPUT):
            path = root / name[len(LIBRARY_INPUT):]
            data = production_content(path.read_text()).encode() if path.is_file() else b'missing'
            h.update(hashlib.sha256(data).digest())
        else:
            path = root / name
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


def local_trait_members(source):
    """Names usable through a local trait, including default/associated items.

    Source has literals/comments masked. Balanced bodies keep one trait's methods
    from leaking into the next item; this remains a development dependency scan.
    """
    traits = {}
    for match in re.finditer(r'\bpub\s+trait\s+(\w+)\b[^{};]*\{', source):
        depth, end = 1, match.end()
        while end < len(source) and depth:
            depth += (source[end] == '{') - (source[end] == '}')
            end += 1
        if depth:
            continue
        body = source[match.end():end-1]
        traits[match[1]] = set(re.findall(r'\b(fn|type|const)\s+(\w+)', body))
    return traits


@lru_cache(maxsize=8)
def library_symbol_owners(texts, facade_source):
    """Reuse parsing across targets, keyed by source contents and module names.

    Callers still read the files and rediscover modules on every scan; this cache
    cannot hide an edit, added/removed module, or changed export in this process.
    Cached results are immutable so one target cannot affect another's inputs.
    """
    owners, traits = {}, {}
    for name, source in texts:
        for trait, members in local_trait_members(source).items():
            traits.setdefault(trait, set()).update(members)
            owners.setdefault(trait, set()).add(name)
    sources = {name for name, _ in texts}
    for name, source in texts:
        symbols = {Path(name).stem}
        symbols.update(re.findall(r'^pub(?:\([^)]*\))?\s+(?:struct|enum|trait|type)\s+(\w+)', source, re.M))
        symbols.update(re.findall(r'^pub(?:\([^)]*\))?\s+(?:async\s+)?fn\s+(\w+)', source, re.M))
        for symbol in symbols:
            owners.setdefault(symbol, set()).add(name)
    for module, exports in re.findall(r'pub use (\w+)::([^;]+);', facade_source):
        for symbol in re.findall(r'\b\w+\b', exports):
            owner = f'crates/solver/src/{module}.rs'
            if owner in sources: owners.setdefault(symbol, set()).add(owner)
    # Foreign-trait impls follow the target type (operators, formatting and
    # conversions can be implicit). Inherent extensions follow named members.
    # Local extension traits follow
    # trait references or member access, not every use of the underlying type.
    # Public trait impls require a referenced receiver and trait/member. Blanket
    # impls match any receiver. Internal traits retain the broad target rule.
    for name, source in texts:
        for block in re.finditer(r'^impl\b([^{}]+)\{', source, re.M):
            header = block[1].strip()
            parameters = set()
            generic_bounds = ''
            if header.startswith('<'):
                depth = 0
                for i, char in enumerate(header):
                    depth += (char == '<') - (char == '>')
                    if depth == 0:
                        generic_bounds = header[1:i]
                        parameters = set(re.findall(r'(?:^|,)\s*(\w+)\s*(?=[:=,]|$)', header[1:i]))
                        header = header[i+1:].strip()
                        break
            header = re.split(r'\bwhere\b', header)[0]
            parts = re.split(r'\bfor\b', header)
            trait = re.match(r'(?:\w+::)*(\w+)', parts[0].strip()) if len(parts) > 1 else None
            target = parts[-1].strip()
            symbol = re.match(r'(?:&\s*)?(?:\w+::)*(\w+)', target)
            if trait and trait[1] in traits and symbol:
                receiver = '*' if symbol[1] in parameters else symbol[1]
                wrapper = re.fullmatch(r'(?:\w+::)*(?:Box|Rc|Arc)\s*<\s*(\w+)\s*>', target)
                bound = (re.search(r'(?:^|,)\s*' + re.escape(wrapper[1]) +
                                   r'\s*:\s*(?:\w+::)*(\w+)', generic_bounds)
                         if wrapper is not None else None)
                forwarding_wrapper = bound is not None and bound[1] == trait[1]
                owners.setdefault('@trait-impl:' + receiver + ':' + trait[1], set()).add(name)
                for kind, member in traits[trait[1]]:
                    if kind == 'fn':
                        # Box<T>/Rc<T>/Arc<T> forwarding impls require a named
                        # trait dependency. An unrelated boxed payload and a
                        # common method such as size must not select them.
                        # Factory return types and concrete implementations reveal
                        # the trait; explicit trait-based lookup remains above.
                        if forwarding_wrapper:
                            continue
                        key = '@trait-method:' + receiver + ':' + member
                    else:
                        # Qualify common associated names such as Error to avoid
                        # selecting adapters for unrelated std::error::Error imports.
                        key = '@trait-associated:' + symbol[1] + ':' + member
                    owners.setdefault(key, set()).add(name)
                continue
            if symbol and not trait:
                if symbol[1] not in owners:
                    continue  # Private names such as Node are local to their module.
                depth, end = 1, block.end()
                while end < len(source) and depth:
                    depth += (source[end] == '{') - (source[end] == '}')
                    end += 1
                if depth:  # Unsupported body: retain the old broad rule.
                    owners.setdefault(symbol[1], set()).add(name)
                    continue
                body = source[block.end():end-1]
                depth = 0
                for item in re.finditer(r'[{}]|\b(?:fn|const|type)\s+(\w+)|\b(\w+)!\s*(?=[({\[])', body):
                    if item[0] == '{': depth += 1
                    elif item[0] == '}': depth -= 1
                    elif depth == 0 and item[1]:
                        owners.setdefault('@inherent-member:' + symbol[1] + ':' + item[1], set()).add(name)
                    elif depth == 0 and item[2]:
                        # Macro-generated members cannot be resolved here.
                        owners.setdefault(symbol[1], set()).add(name)
                continue
            if symbol and symbol[1] in owners:
                owners[symbol[1]].add(name)
    return MappingProxyType({symbol: frozenset(paths) for symbol, paths in owners.items()})


@lru_cache(maxsize=2048)
def dependency_words(source, external=False):
    imports = re.findall(r'\buse\s+([^;]+);', source)
    # Tests/examples have their own crate namespace. Their common:: helpers and
    # std/dependency imports are not library exports, even when names collide.
    aliases = {'timetuck'}
    if external:
        for text in imports:
            alias = re.fullmatch(r'\s*timetuck\s+as\s+(\w+)\s*', text)
            if alias: aliases.add(alias[1])
    def library_import(text):
        first = re.match(r'\s*(?:::)?\s*(\w+)', text)
        return first is not None and (first[1] in aliases if external else
                                     first[1] not in ('std', 'core', 'alloc'))
    native_imports = [text for text in imports if library_import(text)]
    shadowed = set()
    if external:
        # Remove local/imported names from glob-call and type discovery. Explicit
        # timetuck paths are added separately and are not hidden by local names.
        shadowed.update(re.findall(r'\b(?:struct|enum|trait|type|fn|mod)\s+(\w+)', source))
        for text in imports:
            if not library_import(text):
                # Only imported bindings shadow a glob, not every path segment.
                shadowed.update(alias or name for name, alias in re.findall(
                    r'(?:^|::|[{,])\s*(\w+)(?:\s+as\s+(\w+))?\s*(?=[,}]|$)', text))
    words = set(re.findall(r'\b\w+\b', ' '.join(native_imports)))
    if not external:
        words.update(re.findall(r'\b(?:crate|super|timetuck)::(\w+)', source))
    qualified = re.sub(r'\buse\s+[^;]+;', '', source)
    explicit = set()
    # These standard wrappers can carry library trait implementations. Preserve
    # their canonical receivers even when qualified or imported with an alias.
    for text in imports:
        if re.match(r'\s*(?:std|alloc)\s*::', text):
            explicit.update(re.findall(r'\b(?:Box|Rc|Arc)\b', text))
    def path_words(match):
        parts = re.findall(r'\w+', match[0])
        if parts[0] in aliases:
            explicit.update(parts[1:])
            explicit.update('@trait-associated:' + target + ':' + member
                            for target, member in zip(parts[1:], parts[2:]))
        elif not external and parts[0] not in ('std', 'core', 'alloc'):
            explicit.update(parts[:-1])
        elif parts[0] in ('std', 'alloc') and parts[-1] in ('Box', 'Rc', 'Arc'):
            explicit.add(parts[-1])
        # Qualified local/external paths cannot become bare library identifiers.
        # Bare imported types (Score::new) still identify their library receiver.
        if external and (parts[0] in shadowed or parts[0][0].islower()):
            return ' '
        return match[0]
    qualified = re.sub(r'\b\w+(?:\s*::\s*\w+)+', path_words, qualified)
    words.update(set(re.findall(r'\b[A-Z][A-Za-z0-9_]+\b', qualified)) - shadowed)
    # Dot calls and associated items also work through glob-imported traits.
    words.update('@trait-member:' + name for name in
                 re.findall(r'(?:\.|::)\s*([A-Za-z_]\w*)', source))
    words.update('@trait-associated:' + target + ':' + member for target, member in
                 re.findall(r'(?=\b(\w+)\s*::\s*(\w+))', qualified))
    if any('*' in text for text in native_imports):
        calls = set(re.findall(r'(?<![.:\w])([a-z_]\w*)\s*(?:::<[^;]+?>)?\s*\(', qualified))
        local = set(re.findall(r'\bfn\s+(\w+)', source))
        words.update(calls - local - shadowed)
    words.update(explicit)
    return frozenset(words)


@lru_cache(maxsize=8)
def conditional_dependency_rules(owner_items):
    """Wake an implementation rule only when one of its prerequisites appears."""
    by_word = {}
    for key, paths in owner_items:
        if key.startswith(('@trait-impl:', '@trait-method:', '@inherent-member:')):
            kind, receiver, member = key.split(':', 2)
            trigger = member if kind == '@trait-impl' else '@trait-member:' + member
            rule = (receiver, trigger, paths)
            by_word.setdefault(trigger, []).append(rule)
            if receiver != '*' and receiver != trigger:
                by_word.setdefault(receiver, []).append(rule)
    return MappingProxyType({word: tuple(rules) for word, rules in by_word.items()})


@lru_cache(maxsize=2048)
def attributed_rust_modules(source):
    return tuple(re.findall(r'(?:#\[[^\]]*\]\s*)+(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;', source))


def selected_library_inputs(entries, root=ROOT):
    """Development dependency scan, at module granularity (not a Rust compiler).

    Follow named types/functions/modules and out-of-line implementations.
    Inherent extension methods need a referenced receiver and member; merely
    declaring an unused child module does not select its contents. Glob
    imports resolve against identifiers used in the target. Facade inputs include
    referenced declarations/reexports and other root code, excluding unrelated exports.
    Public extension traits follow trait/member references plus concrete receiver
    names. Blanket impls match any receiver. Internal/foreign traits and inherent
    implementations remain dependencies of their target types.
    """
    facade = 'crates/solver/src/lib.rs'
    if facade in entries:
        return library_inputs(root)
    # Discover each library file once per selection. Reuse its direct edges in
    # the fixed-point walk instead of recursively reopening it for every symbol.
    # This is a per-call snapshot: the next selection still reads fresh contents.
    library_edges, library_texts = rust_entry_graph(facade, root, include_test_items=False)
    texts = {name: rust_dependencies_text(source)
             for name, source in sorted(library_texts.items()) if name != facade}
    facade_source = (root / facade).read_text() if (root / facade).is_file() else ''
    owners = library_symbol_owners(tuple(texts.items()), facade_source)
    rules_by_word = conditional_dependency_rules(tuple(owners.items()))
    selected, pending, referenced = set(), list(entries), set()
    while pending:
        name = pending.pop()
        if name in selected: continue
        if name not in library_texts and not (root / name).is_file(): continue
        selected.add(name)
        if name in library_edges and name not in entries:
            # Plain child modules are discovered by referenced symbols/members.
            # Literal includes and attributed modules keep their explicit edges.
            _, literals = rust_file_references(library_texts.get(name, ''))
            pending.extend((root / Path(name).parent / literal).resolve().relative_to(root.resolve()).as_posix()
                           for literal in literals)
            attributed = attributed_rust_modules(texts.get(name, ''))
            path = root / name
            base = path.parent if path.name in ('mod.rs', 'lib.rs') else path.with_suffix('')
            for module in attributed:
                for child in (base / f'{module}.rs', base / module / 'mod.rs'):
                    if child.is_file(): pending.append(child.relative_to(root).as_posix())
        else:
            pending.extend(rust_entry_inputs(name, root, include_test_items=name in entries or not name.startswith('crates/solver/src/')) - selected)
        source = texts[name] if name in texts and name not in entries else rust_dependencies_text((root / name).read_text())
        new_words = dependency_words(source, external=not name.startswith('crates/solver/src/')) - referenced
        referenced.update(new_words)
        for word in new_words:
            pending.extend(owners.get(word, frozenset()) - selected)
            # Either prerequisite can arrive first, including via a factory in a
            # later module. Reconsider only rules affected by newly revealed names.
            for receiver, trigger, paths in rules_by_word.get(word, ()):
                if trigger in referenced and (receiver == '*' or receiver in referenced):
                    pending.extend(paths - selected)
    modules = {p for p in selected if p.startswith('crates/solver/src/')}
    referenced.update(Path(p).stem for p in modules)
    projected = FACADE_INPUT + json.dumps(sorted(referenced), separators=(',', ':'))
    return cargo_inputs(root) | {projected} | {LIBRARY_INPUT + p if p.endswith('.rs') else p for p in modules}


@lru_cache(maxsize=2048)
def rust_file_references(source):
    return (tuple(re.findall(r'\bmod\s+(\w+)\s*;', source)),
            tuple(re.findall(r'(?:include(?:_str|_bytes)?!\s*\(\s*|#\[path\s*=\s*)"([^"]+)"', source)))


def rust_entry_graph(entry, root=ROOT, *, include_test_items=True):
    """Discover direct file references and source text in one fresh traversal."""
    pending, edges, texts = [entry], {}, {}
    while pending:
        name = pending.pop()
        if name in edges:
            continue
        edges[name] = set()
        path = root / name
        if not path.is_file() or path.suffix != '.rs':
            continue
        source = path.read_text()
        if not include_test_items:
            source = production_rust(source)
        texts[name] = source
        relative = Path(name)
        base = relative.parent if relative.name in ('mod.rs', 'lib.rs') or relative.parent.name in ('tests', 'examples') else relative.with_suffix('')
        modules, literals = rust_file_references(source)
        for module in modules:
            for child in [base / f'{module}.rs', base / module / 'mod.rs']:
                if (root / child).is_file(): edges[name].add(child.as_posix())
        for literal in literals:
            edges[name].add((path.parent / literal).resolve().relative_to(root.resolve()).as_posix())
        pending.extend(edges[name])
    return edges, texts


def rust_entry_inputs(entry, root=ROOT, *, include_test_items=True):
    """One test/example, its local modules and literal include files."""
    edges, _ = rust_entry_graph(entry, root, include_test_items=include_test_items)
    return set(edges)


def doctest_inputs(root=ROOT):
    # Rustdoc does not enable cfg(test), but documentation itself must stay hashed.
    paths = rust_entry_inputs('crates/solver/src/lib.rs', root, include_test_items=False)
    return cargo_inputs(root) | {DOCTEST_INPUT + p if p.endswith('.rs') else p for p in paths}


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
