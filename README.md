# Timetuck

An independent constraint optimization solver being written from scratch in Rust.
The goal is behavioral compatibility with selected **Timefold Solver Community**
features, using its open-source tests as a conformance reference.

**Status: scores, basic/list moves, exact basic/list search, incremental basic/list scoring, construction and local search.**
The solver is still under development. Scores, moves, acceptance, construction and constraint streams cover 252 adapted upstream
test methods. Three fixed integer score types and const-generic BendableScore also pass 9,959
generated Java/Rust checks (including 30 expected malformed-input rejections). Exact search solves small finite problems with explicit evaluation
budgets and domain/pinning validation; 128 original complete-planning cases agree
with Java and independent enumeration, including per-variable ranges and optional
values. Ordered lists support unique ownership, per-entity ranges, prefix/full pins and derived
owner/index/neighbor views. Scoped list/entity and element contributions support
incremental candidate scoring with transactional cache updates. Another 256 original list-move cases check candidate,
undo and commit states against Java and an independent reference. An additional
128 original list-solving cases validate construction/local search and compare
quality with independently enumerated optima, including entity-specific ranges. Separate full-scoring routing
benchmarks show Java/Rust timings together with final scores.
Native snapshot constraint streams now support typed filtering, joins, existence checks,
grouping and weighted scoring, with retractable integer collectors. Streams are
rebuilt inside affected score callbacks. Separate persistent operators now propagate
filter, equality/existence-join, distinct, keyed/global collection and score updates by stable tuple ID. A typed graph
automatically routes these changes and rolls back candidate transactions. Composable
collectors include sets, integer summaries, nested maps and lists ordered by stable
occurrence ID, with duplicate tracking
and immutable result snapshots. Stream
score directors bind it to basic-variable and ordered-list moves. List streams
expose entity lists and value owner/index/neighbor snapshots. Persistent ordered storage shares
unchanged tree branches across checkpoints and saved candidates. Stream construction
and local search share the existing search algorithms, including seeded acceptance
and exact work limits. Stream comparisons check 128 original cases across 2,899
states; separate incremental-search benchmarks report Java/Rust quality and timing.
See `docs/constraint-stream-contract.md`.
There is no full-suite or whole-solver compatibility claim yet. This project is not affiliated with or endorsed by Timefold.

## Development

Install Rust via rustup, Git, and Python 3. Run from the repository root:

```sh
bash scripts/check.sh
```

This checks formatting, Clippy, compilation, and Rust tests. It also records test outcomes and refreshes the local coverage report when the
reference submodule is initialized. See `docs/score-contract.md` and
`docs/planning-contract.md` for the verified scope.

Run the exact scheduling example with `cargo run --example tiny_schedule`, or
the exact route example with `cargo run --example tiny_routes`, or
the larger incremental examples with `cargo run --release --example graph_coloring`
and `cargo run --release --example list_routes`.
For graph-based scoring in both phases, run `cargo run --release --example stream_coloring`.
Run `cargo run --release --example stream_list_routes` for stream-scored ordered-list
construction and local search.
Local search supports strict improvement, hill climbing, late acceptance,
variable tabu and seeded simulated annealing, with original or seeded variable order and first/best accepted moves. See `docs/incremental-search-contract.md`
for limits and evidence. Greedy construction evaluates values before choosing each
unassigned required variable; `local_search_from` improves the resulting assignment.
For ordered lists, `greedy_list_construction` inserts missing required values and
`list_local_search_from` improves a complete assignment with lazy relocation,
assignment, unassignment, swap and replacement moves. See
`docs/list-planning-contract.md` for budgets, pinning and evidence.

## Upstream test reference

Timefold Community **2.7.0** is pinned as a separate Git submodule. It is not a
Rust dependency and is not linked into the library. Normal Rust checks need no JVM.

```sh
git submodule update --init upstream/timefold-solver
python3 scripts/upstream_inventory.py
```

The inventory verifies the pinned revision and writes a list of upstream Java
test-source files to `work/upstream-tests.json`. It does not execute tests or
measure coverage. Upstream JUnit tests cannot directly exercise a Rust library:
we need adapted behavioral cases and a Java/Rust comparison harness.

## Dashboard and comparisons

```sh
python3 scripts/differential_scores.py
python3 scripts/benchmark_scores.py
python3 scripts/differential_solver.py
python3 scripts/differential_lists.py
python3 scripts/benchmark_solver.py
python3 scripts/benchmark_heuristics.py
python3 scripts/benchmark_lists.py
python3 scripts/differential_streams.py
python3 scripts/benchmark_streams.py
python3 scripts/compat_report.py --test-output work/rust-tests.log
python3 scripts/dashboard_server.py
```

Open [the local dashboard](http://localhost:8765). It shows pass counts and
percentages by subsystem, individual test results, pending/excluded work, and
separate basic/list heuristic, incremental-stream, score-operation and small exact-search benchmarks versus Java.
Heuristic timings include final scores, feasibility and callback counts. Results reload every 15 seconds.
The oracle/benchmarks need JDK 21 and Maven Central access; ordinary Rust tests
need neither. See [dashboard measurement notes](docs/dashboard.md).

## Layout

- `crates/solver/`: original Rust library, adapted tests and development adapters.
- `upstream/timefold-solver/`: unmodified, pinned upstream reference.
- `compat/`: provenance ledger, exclusions, notices and public-API Java adapters.
- `dashboard/`: visual coverage and performance report.
- `docs/compatibility.md`: testing strategy and compatibility boundaries.
- `docs/legal-and-provenance.md`: naming, licensing, and test attribution.
- `docs/next-session.md`: starting point for the implementation session.

Original project files use Apache-2.0; see `LICENSE`. Upstream material retains
its own notices and license. See `THIRD_PARTY_NOTICES.md` before adapting tests.
