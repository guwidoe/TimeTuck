# Timetuck

An independent constraint optimization solver being written from scratch in Rust.
The target is **Timefold Solver Community** behavior, using its open-source tests
as a conformance reference. Current compatibility is limited to verified cases.

**Status: scores, basic/list moves, exact basic/list/mixed search, incremental basic/list/mixed scoring, construction, local search and multi-phase solving.**
The solver is still under development. Scores, moves, acceptance, construction, termination, lifecycle, derived values and constraint streams have adapted upstream
tests; the [dashboard](docs/dashboard.md) reports current verified method coverage. Three fixed integer score types and const-generic BendableScore also pass 9,959
generated Java/Rust checks (including 30 expected malformed-input rejections). Corresponding
fixed and bendable decimal scores pass 8,170 generated comparisons, with 225 expected
errors; decimal timing and general floating-exponent equivalence remain outside this evidence. Exact search solves small finite problems with explicit evaluation
budgets and domain/pinning validation; 128 original complete-planning cases agree
with Java and independent enumeration, including per-variable ranges and optional
values. Ordered lists support unique ownership, per-entity ranges, prefix/full pins and derived
owner/index/neighbor views. Mixed models combine both variable kinds with atomic edits
and exact joint optimization. Mixed scoped constraints cache contributions across
both components and restore them atomically. Mixed local search supports joint
moves under shared budgets and seeded acceptance. Atomic pillar moves change or
swap whole groups across selected basic-variable columns; opt-in whole-group
neighborhoods serve basic and mixed local search in both scoring backends. Bounded
subpillar selection also supports all member combinations or consecutive windows,
with independent size bounds for each swap side. An explicit `PillarSchema` on the
model groups aligned entity rows by selected column values; both scoring backends
use it for basic and mixed search. Scoped pillar providers can instead own separate
schemas, built from typed field bindings, and combine neighborhoods for different
entity types in the same search.
Mixed greedy construction fills
missing required values in an explicit basic/list order before local search; see `docs/mixed-planning-contract.md`. Scoped list/entity and element contributions support
incremental candidate scoring with transactional cache updates. Another 512 original list-move cases check candidate,
undo and commit states against Java and an independent reference. An additional
192 original list-solving cases validate construction/local search, including two-opt, and compare
quality with independently enumerated optima, including entity-specific ranges. Separate full-scoring routing
benchmarks show Java/Rust timings together with final scores.
Native snapshot constraint streams now support typed filtering, joins, existence checks,
grouping and weighted scoring, with retractable integer collectors. Streams are
rebuilt inside affected score callbacks. Separate persistent operators now propagate
filter, one-to-many expansion, concatenation, complement, equality/ordered/partitioned/existence-join, distinct, keyed/global collection and score updates by stable tuple ID. A typed graph
automatically routes these changes and rolls back candidate transactions. Composable
collectors include sets, integer summaries/averages, checked signed integer sums, arbitrary-precision integer/decimal sums and averages, signed duration sums/averages, calendar-period sums, key-projected minima/maxima for non-ordered payloads, consecutive sequences and connected ranges with custom distance metrics, exact load balancing, nested maps and lists ordered by stable
occurrence ID, with duplicate tracking
and immutable result snapshots. Pure finishing adapters transform or flatten nested
collector results while preserving each inner collector's update semantics. Stream
score directors bind it to basic, ordered-list and combined mixed moves. Indexed
`follow_reference` streams derive values through changing entity references, retain
missing targets explicitly, and restore their inverse dependencies on rollback.
`follow_shared_reference` computes one supplier result per referenced group and
shares it across followers, with a deterministic representative.
`DependencyGraph` evaluates explicit fixed/dynamic derived-variable dependencies,
rejects fixed cycles and marks temporary cycles and downstream values inconsistent.
`DependencyCache` propagates changed derived values through persistent snapshots;
`derive_dependencies` integrates its state with stream transactions, undo and saved
search candidates. `map_cached` owns per-input projection caches; concurrent schedule
scoring uses it with independent full-score checks. Equal values stop propagation.
Dynamic edge changes still rebuild topology, and no whole-solver speed improvement is claimed. List streams
expose entity lists and value owner/index/neighbor snapshots. Persistent ordered storage shares
unchanged tree branches across checkpoints and saved candidates. Stream construction
and local search share the existing search algorithms, including seeded acceptance
and exact work limits. Stream comparisons check 128 original cases across 2,899
states; a separate 128-case mixed-model comparison checks 2,378 states and 3,611
primitive basic/list operations against Java public moves and score updates.
Another 128 generated follower cases compare 2,373 states and 3,714 operations,
including dynamic references and fixed shared suppliers, against Java public moves
and independent Python values/scores. Another 128 collection-follower cases
compare 2,368 states and 3,724 operations, preserving UTF-16 code order and
first-reference ties. Another 128 list-element cases compare 2,365 states and
3,675 operations for ordered time chains and nullable owner aggregates. Shared
cyclic scheduling adds 128 generated cases: all native states match Python, while
107 cases agree with Java and 21 terminate with recorded Java errors. A minimal
partial-group error is reproduced with unchanged pinned fixtures. Separate incremental-search benchmarks
report Java/Rust quality and timing.
Consecutive-sequence comparisons additionally check 64 original cases and 4,689 states,
with a separate explicit Java integer-overflow discrepancy and stable native tie ordering.
Four dedicated sequence benchmarks compare replacement/inverse updates plus complete
result observations on dense/fragmented 64/512-item workloads; all generated and timed
observations must match independent Python before a timing is reported.
Connected-range checks retain 197 cases: all native results match independent Python;
65 cases agree with Java and 132 differ, including shared-object removal, empty-range
overlap flags and integer overflow. All 64 distinct-identity positive-length cases agree.
See `docs/constraint-stream-contract.md`.
There is no full-suite or whole-solver compatibility claim yet. This project is not affiliated with or endorsed by Timefold.

Use `solve`, `solve_list` or `solve_mixed` to construct and improve a model with
shared evaluation budgets and cancellation. `ruin_recreate_search_from` and its stream
counterpart explore seeded groups of basic variables through bounded greedy
reconstruction, retaining the best complete solution. `ruin_recreate_list_search_from`
and its stream counterpart provide list-value repair with full/prefix pins and
entity-specific ranges. Local search accepts nested `StepTermination::All`
and `Any` conditions over step counts, stagnation, accepted moves, score targets and
feasibility; see the [search contract](docs/incremental-search-contract.md). Local search also supports all five
step-counting hill-climbing modes, with 600 original Java/Rust/Python comparisons. SolverPlan and MixedSolverPlan can mix
local search with exact phases; only fully exhausted enumeration proves optimality.
Stream counterparts use a fresh graph
factory for each phase. Results include partial progress, retained solutions and
per-phase work. See [the solver contract](docs/solver-contract.md) for configuration,
error behavior and limits; `mixed_routes` exercises both scoring backends.

Basic, list and mixed models also support branch-and-bound, with four exploration
orders, optional optimistic bounds, best-solution observations and bounded search
controls. Exhaustion proves optimality when supplied bounds are admissible; unknown
bounds keep branches open. `branch_and_bound`, `branch_and_bound_lists` and
`branch_and_bound_mixed` share pruning and controls; mixed search evaluates the
coupled objective. Ordered Exact phases accept the same exploration strategies
with unknown bounds, an explicit non-increasing-score assumption, or declared
per-level initialization trends with symbolic infinity for unknown levels. The
`mixed_routes` example demonstrates a metric-distance bound for standalone search.
See [the planning contract](docs/planning-contract.md)
for callback semantics, validation and limits. The dashboard compares all four
orders with Community on independently verified small exact-search workloads.

Load balancing retains exact per-key loads and rounded decimal unfairness through
updates and undo. All 101 generated cases / 8,290 states match independent Python;
97 agree with Java, including all 96 bounded cases. Four large-number cases expose
Java overflow or cancellation and remain explicit discrepancies. Initial loads must
be constant per active key. No load-balance timing claim is made.

Calendar-period aggregation preserves signed year/month/day fields and checks
32-bit overflow independently. All 227 generated cases / 8,771 observations agree
with Java and Python, including 120 expected arithmetic rejections. Failed native
updates preserve captured state; no date application or period timing is claimed.

Aggregate collector benchmarks compare checked sums, sorted sets and additive
merged maps against Java at 64 and 512 captures. Run
`python3 scripts/benchmark_aggregates.py` to measure replacement and restoration
with complete result validation. The dashboard retains timing ranges, raw samples
and methodology; these workloads do not establish overall solver speed.

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
the joint depot/route example with `cargo run --example mixed_routes`, or
the larger incremental examples with `cargo run --release --example graph_coloring`
and `cargo run --release --example list_routes`.
For graph-based scoring in both phases, run `cargo run --release --example stream_coloring`.
Run `cargo run --release --example stream_list_routes` for stream-scored ordered-list
construction and local search.
Mixed stream construction and local search are also verified by `mixed_routes`,
using joined depot/route facts and independent final-score checks.
Run `cargo run --example configured_routes` to resolve owned text properties,
apply shared time limits and diminished-return settings, and solve a coupled
depot/route model through both scoring backends. Its namespace is an explicit
native configuration profile; it does not load a Java framework.
Construction and multi-phase solving support cooperative elapsed-time limits; a
shared solve timer includes stream graph preparation. A shared unimproved-time
limit excludes construction and the first search initialization, then spans later
phases. A shared unimproved-step limit similarly counts completed search steps
across phases, resetting only for strict retained-best improvements. See `docs/solver-contract.md`.
Basic/list/mixed exact search also supports a cooperative `ExactSearchConfig::max_time`
limit, with explicit TimeLimit results; see `docs/planning-contract.md`.
Local search supports strict improvement, hill climbing, late acceptance,
variable tabu, seeded fading variable tabu, and seeded simulated annealing, with original or seeded variable order. Candidate selection supports first/best accepted moves, early improvement over the current or best score, per-step accepted-candidate limits, and first or seeded random ties. See `docs/incremental-search-contract.md`
for limits and evidence. Optional diminished-returns termination compares exact
sliding-window improvement rates after initialization and stops individual local
phases. A separate significant-improvement limit requires a configured score gain
within each window, with exact arithmetic and optional ignored lower score levels. Greedy construction can visit one item at a time or use pooled cheapest
insertion across all eligible missing items. Value-queue construction cycles through
basic values and chooses their best unassigned receivers; `local_search_from` improves
the resulting assignment. The dashboard compares all three basic construction
strategies with score quality and known optima beside the timings.
For ordered lists, `greedy_list_construction` inserts missing required values and
`list_local_search_from` improves a complete assignment with lazy relocation,
assignment, unassignment, swap and replacement moves. See
`docs/list-planning-contract.md` for budgets, pinning and evidence.

The common score interface accepts owned `Clone` values as well as the built-in
`Copy` integer scores. Numeric levels support `i64` and exact `Decimal` values for
custom scores. `SimpleDecimalScore`, `HardSoftDecimalScore`,
`HardMediumSoftDecimalScore` and `BendableDecimalScore<H,S>` provide numeric
equality/hash, scale-preserving text and exact decimal arithmetic. See
`docs/score-contract.md`.

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
python3 scripts/benchmark_construction.py
python3 scripts/benchmark_lists.py
python3 scripts/differential_streams.py
python3 scripts/benchmark_streams.py
python3 scripts/benchmark_pillars.py
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
