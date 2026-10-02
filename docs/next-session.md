# TimeTuck implementation handoff

## Active user goal

Rewrite Timefold behavior in independent native Rust, using pinned Community tests
as specifications. Also maintain a visual dashboard of coverage by subsystem and
performance versus Java. The whole rewrite remains unfinished; do not mark the
active goal complete after one slice. No zero-mistake guarantee is possible.

## Current implementation

- Rust library has SimpleScore, HardSoftScore, HardMediumSoftScore and
  BendableScore<H,S> with structural
  values, ordering/feasibility, parsing/formatting, arithmetic and checked arithmetic.
- 308 fully adapted source test methods (74 score + 10 basic move + 22 list move + 27 provider + 11 acceptor + 4 construction + 98 collector + 62 stream), recorded in
  `compat/provenance.json`.
- Finite domains, initial values, pinning, optional values and reversible
  change/swap/composite transactions. Read `docs/planning-contract.md`.
- Iterative exact search with explicit evaluation budgets and truthful
  exhaustion/optimality reporting. Original `tiny_schedule` example.
- Mixed basic/list models, atomic combined moves and exact joint search over the full
  product space. Read `docs/mixed-planning-contract.md`. `mixed_routes` proves
  `0hard/-4soft` over 96 states. Mixed local search now supports joint moves and all native acceptance policies;
  mixed greedy construction supports explicit component ordering. Scoped and transactional
  stream scoring share construction/local-search algorithms. Java mixed comparisons remain pending.
- Native ordered-list model with unique ownership, optional unassignment, prefix/full
  pins and derived owner/index/previous/next views. Atomic assign/change/swap/span
  unassign/composite transactions, plus iterative bounded exact list enumeration.
  Read `docs/list-planning-contract.md`. `tiny_routes` proves `0hard/-12soft`
  after 5,040 complete states.
- 256 original Java/Rust/Python list-move cases (907 operations) check candidates,
  undo, commit and all four built-in shadow relationships; seed 20261003.
- Scoped incremental basic/list/mixed score networks and atomic scored-move transactions.
  List scopes cover entity lists and value identities/shadows, with conservative
  neighbor invalidation and deferred cache updates.
- Lazy change/swap neighborhoods and bounded deterministic local search,
  strict-improvement, hill-climbing, late acceptance, variable tabu or seeded simulated annealing; explicit
  evaluation/step limits, first/best accepted candidate selection and original or
  seeded shuffled-variable ordering.
  Read `docs/incremental-search-contract.md` before modifying this layer.
- 276 original integration regression methods, nine original unit tests, and twenty-one public API documentation tests.
- Greedy construction with four early-pick policies, explicit partial results and
  bounded scoring. `local_search_from` accepts a complete constructed assignment.
- `graph_coloring` constructs and improves 100 variables using 400 scoped constraints;
  702 combined score evaluations, 5,700 callbacks, verified `0hard/0soft`.
- Public-API Java oracle: 9,959 generated comparisons (30 expected malformed-input rejections) pass with seed 20261001.
- 128 original complete-planning cases agree across Java, Rust and independent
  Python enumeration: 64 homogeneous required and 64 per-variable range cases,
  including mixed optionality, empty optional ranges and pinned nulls. Movable
  initial values reset before both solves.
- Separate score microbenchmarks and two small matched exact-search workloads.
  The latter compare BRUTE_FORCE/ORIGINAL_ORDER Java with Rust finite enumeration.
  Matched full-scoring heuristic comparisons now cover hill climbing and tabu,
  32/96 variables, 50-step budgets, scores and callback counts. 64 original small
  heuristic cases are independently checked. These older sections do not measure
  incremental scoring or establish large-instance performance.
- Stream scoring has 128 original Java/Rust cases (2,899 states), checking
  partial assignments, signed/cancelling groups, duplicate/self edges, commits
  and undo. Python checks both ordinary scores and native exact match IDs/sums.
  Community `SolutionManager.update()` is used; `analyze()` requires Enterprise
  in this release and must not be used. No Java match-analysis claim.
- Matched incremental-stream hill/tabu benchmarks use 32/96 variables and
  50-step budgets, with 64 small independently checked search cases. Scores,
  validity and terminal weighting calls accompany times. Rust graph construction
  is timed; Java reuses its factory outside timing. See `docs/dashboard.md`.
- Local visual dashboard: `python3 scripts/dashboard_server.py`, port 8765.
  Server was restarted after the reporting-code update and may still be running.
  Reuse it; restart after changing imported fingerprint/reporting Python code.
- Coverage denominator: 4,855 statically discovered annotated Java methods, not
  expanded runtime JUnit invocations. 308 passed; 282 JVM integration exclusions.
  Everything else is pending. `docs/dashboard.md` explains caveats and commands.
- Pinned upstream remains unmodified at 8b815bc6b5f3eb81feedc94dea0885a93b267762.

Read `docs/score-contract.md` before changing scores. The Java oracle showed that
score arithmetic CLEARS structural values, constructors retain positive values,
and full strings suppress positive structural prefixes. Do not reintroduce older
initialization-score assumptions. No production implementation has been ported.

## Next substantive work

0. Expand persistent operators and applicable upstream cases; mixed stream scoring,
   construction and local search now exist. Add original Java mixed solver comparisons.
   Incremental distinct now retains one lowest-input-ID representative per ordered
   value class and refreshes changed representative payloads. Twenty mapping/
   distinct tests across uni/bi/tri/quad streams have full assertion adaptations.
   Indexed existence/anti-existence with pair filters is now implemented; thirteen
   more upstream methods preserve all score/match assertions. Cached match links
   require memory proportional to matching pairs; consider counts-only equality
   specialization when profiling warrants it. Read the stream contract first.
   Public-API Community stream comparisons and matched timing now exist.
   Warmed stream samples show Rust slower than Java, with equal final scores.
   Use the latest work/stream-performance.json and profile these workloads before
   choosing optimizations; ratios vary on the shared machine.
   Checkpoints and saved candidates now share persistent AVL map roots, avoiding
   whole-node row cloning. Stream construction/local search already share existing
   algorithms. Do not hide mutable nodes inside scoped callbacks: those callbacks
   cannot undo external state. Read `docs/constraint-stream-contract.md`.

1. Extend native exact-search/model coverage to more applicable upstream tests.
   Java trace/descriptor-specific assertions must not be credited by looser tests.
2. Extend list-solving evidence to optional lists and mixed models. Entity-specific
   ranges now apply across moves, construction, exact and local search. Required-list
   construction/hill-climbing now have 128 Java/Rust checks
   and four full-scoring routing benchmarks; exact optima are independently
   enumerated for small cases. Native greedy list construction, lazy neighborhoods
   and bounded heuristic search now exist.
   Native exact list enumeration and four derived shadows now exist; custom
   shadows and cascading/declarative listeners remain pending. Optional basic variables and per-variable ranges have 64 additional
   exact differential cases.
3. Extend construction, tabu (fading and entity grouping) and extend annealing with
   deterministic seeds, limits, quality checks and applicable
   upstream acceptance tests. Keep exact Java trace tests pending when not preserved.
4. Extend matched Java heuristic evidence to more workloads and randomized
   search. Separate stream benchmarks now use incremental scoring; older basic/list
   sections retain full scoring. Current measured searches are deterministic.
   Scoped graph-example callbacks are not runtime benchmarks.
5. Extend score coverage to exact decimal families, inspecting tests
   and public documentation only and preserving all test provenance.

Run `bash scripts/check.sh` after Rust changes. It records all-target results and
regenerates coverage if the submodule is present. Java comparisons and benchmarks
are separate explicit commands; refresh them when measured sources change, then
regenerate the report using the captured log. Never treat a stale or skipped case
as passing. `scripts/adapt_score_tests.py` regenerates only the first three test
families and preserves other provenance entries; its deliberately limited parser
fails on unfamiliar assertions.

All crates remain unpublished. The user requested occasional commits; checkpoint
verified milestones with Conventional Commits after inspecting staged changes.
Never amend or rewrite commits. No remote publication has been requested.

Earlier verification: `scripts/check.sh` passed (549 Rust integration methods,
seven unit tests, seventeen doctests, 48 Python regressions). Incremental-scoring, neighborhood, acceptance and local-search regressions
also passed in release mode; the complete all-target and documentation suites passed
after the persistent-storage increment. Earlier dashboard layout QA
at desktop 1440px and mobile 390px passed, including move filtering and focus
preservation on refresh. Current saved Java
evidence includes 9,959 score cases, 128 complete-planning cases and 256 list-move
cases. The benchmark
ratios are workload-specific and noisy; retain raw sample ranges and scope labels.

Late-acceptance scope: history uses completed-step scores; new hard-score bests
reset every slot. Public acceptance configuration now creates a session with
`start(initial_best)`; the session owns `accepts` and `step_ended`. Seeded variable
ordering keeps value order unchanged. The negative Java history-size case remains pending. The Bendable
late-acceptance case is now adapted and preserves nine history assertions.

Construction scope: required unassigned basic variables only; existing values,
pins and optional None stay unchanged. Network callbacks must handle partial
assignments. Candidate selection saves assignment + sparse contribution updates
inside the director; it does not rescore the selected winner. Local phase handoff
creates a new director and counts that initial rescore. See the incremental-search
contract for exact budget/termination semantics. Upstream structural-flaw rejection
remains pending because it is distinct from native unassigned-count structure.

Tabu scope: stable keys, tenure in completed steps, strict global-best aspiration.
The integrated policy keys basic variables; it has no multi-variable entity model.
`Acceptor::accepts` and `step_ended` now take a touched-variable slice (score-only
policies ignore it). `MoveSelection::BestAccepted` selects the best accepted
candidate from the scanned prefix; it may commit a candidate at EvaluationLimit.
Selected swap keys include both variables. Fading and move/value tabu remain pending.
Current latest logs: work/tabu-check.log, work/tabu-release.log, work/tabu-evidence.log.

Heuristic comparisons: scripts/benchmark_heuristics.py builds Java/Rust adapters,
checks 64 original cases, and measures four workloads with independent per-sample
validation. work/heuristic-performance.json feeds a separate dashboard section
showing time, quality, feasibility and score calls. Existing exact adapters still
reset movable initial values; the heuristic adapter retains every initial value.
Case::parse now takes retain_initial explicitly. Java config uses only public
APIs, NO_ASSERT, original-order change moves, first tie and best rejected fallback.

Fallback is explicit via RejectedMovePolicy::BestScore (default Stop). max_steps
and completed_steps include rejected fallback steps; max_accepted_moves and
accepted_moves remain separate. Benchmarks use max_steps=50 and effectively no
accepted-move cap. The public API oracle exposed the need for this distinction.
Source logs: work/heuristic-check.log, heuristic-release.log, heuristic-evidence.log.
Dashboard server restarted after the report import change; reuse the live server.


Annealing scope: `Acceptance<S>` and `LocalSearchConfig<S>` now carry the score
(type parameter defaults to SimpleScore; inference works at search call sites).
`Score::ordinary_levels()` exposes an ordered i64 iterator without structural state.
`Acceptor::accepts` now takes `&mut self` because annealing trials consume PRNG
state. Existing deterministic policies retain their behavior. The selection
predicate is FnMut; winning candidates are still installed without replay.

`SimulatedAnnealing` linearly cools on committed steps (including fallback steps),
using its own SplitMix64 stream. A separate positive cooling_steps schedule can
finish before the search step cap. Native multi-level energy sums only losses;
lexicographic improvements/ties always pass. Upstream coverage covers scalar
probability decisions plus invalid multi-level temperature only. No Java trace or
multi-level annealing compatibility/performance claim. Ten original regressions
include 32 fixed-seed valley escapes to an independently enumerated optimum,
integer precision/overflow boundaries, seeded frequency, budgets and fallback.
See the annealing section of docs/incremental-search-contract.md. Logs:
work/annealing-check.log, work/annealing-release.log, work/annealing-evidence.log.


Extended range oracle: `RangeSolverOracle.java` uses only public annotations/config
and two planning fields per Java entity, with the inactive field constrained to
one complete value. This handles mixed required/optional inputs without changing
scores or widening the native assignment space. The optional field supports empty
ranges and pinned nulls. See docs/planning-contract.md. Nine-field protocol lines
extend the legacy six fields with domains/optional bits/unassigned weights; the
legacy exact and heuristic benchmarks keep their six-field inputs. Python
`generate()` retains the original 64 cases (also used by heuristic validation);
`generate_extended()` adds 64, with a separate seed 20261002. Both engines' outputs
are independently checked for every range, pin, optional flag, score and optimum.
No new upstream method credit. Four original Rust protocol tests and four Python
validation tests added. Current logs: work/ranges-check.log,
work/ranges-release.log and work/ranges-evidence.log. Timing comparisons are
refreshed for the source revision but still cover the original workloads only.


BendableScore: native const-generic hard/soft arrays, structural-first order,
all-hard-level feasibility, checked/wrapping arithmetic, typed parsing and group
formatting. Zero-sized groups are valid. All 27 BendableScoreTest methods are
adapted by scripts/adapt_bendable_tests.py; the script reads test sources only
and preserves unrelated provenance. One late-acceptance method adds nine history
assertions. Seven original regressions cover boundaries and solver integration.
The score oracle has five Bendable shapes; unexpected ERROR agreement is now a
failure, with two Python guards. Original fixed-family adapter remains separate.
Score::ordinary_levels now returns an iterator instead of a slice; annealing was
updated accordingly. Read docs/score-contract.md. Timing benchmarks remain the
existing fixed-score, exact and hill/tabu workloads; no Bendable performance claim.
Current logs: work/bendable-check.log, work/bendable-release.log,
work/bendable-evidence.log. Dashboard source freshness and measurement digests
must be current before committing; all upstream source files remain unmodified.


Ordered-list milestone: 22 methods from ListAssignMoveTest (5), ListSwapMoveTest
(6), ListChangeMoveTest (9 state methods) and SubListUnassignMoveTest (2 state
methods), with TestdataListSolution fixture attribution. Twelve original Rust
regressions independently enumerate owner/permutation spaces and check optima,
budgets, pins, all small valid moves, atomic errors, ordinary and clone panics,
shadow projection and deep iterative traversal. Four Python guards validate
reference outputs and cannot accept corrupt undo/shadows. New ListMoveOracle.java
uses only public metamodel/Moves/MoveTester/shadow APIs. Empty move sequences
compare shadow enrichment only; Java rejects empty composition and same-index
relocation. Native no-ops are documented and tested separately.

Run python3 scripts/differential_lists.py to save work/list-differential.json and
work/oracle/list-cases.json. compat_report includes list_differential and the
dashboard renders a separate evidence row. Sources must be stable throughout
measurement, and all earlier benchmarks must be refreshed after Rust/script edits.
No list solver Java compatibility or performance claim; exact list optima have
independent native reference tests. Current logs: work/list-check.log,
work/list-release.log and work/list-evidence.log.


Incremental-list-scoring milestone: ListConstraintNetwork, ListScope, ListFact,
ListScoreDirector and ScoredListMove. Entity-list and value/shadow scopes, mixed
and repeated arguments, immutable shape validation, contribution-only structural
zero and native missing-required-value structure. Before/after touched-list
membership identifies dirty element constraints, including neighbors. Full
list snapshots and linear location lookups remain; only callbacks are incremental.
Nine original integration tests include 1,200 deterministic composites (seed
20261004), independent formula and full-cache checks, callback failures/unwinding,
constant/unrelated callback counts, pins, structure and modular Bendable boundaries.
One API doctest added. No upstream case credit; 129/4,855 stays unchanged.
Java evidence and existing benchmarks are refreshed for sources, but do not measure
list incremental scoring or list search. Next: construction and lazy neighborhoods
plus bounded list local search with winner installation without callback replay.
Current logs: work/list-scoring-check.log, work/list-scoring-release.log,
work/list-scoring-evidence.log. Reuse the live dashboard server unless its imported
Python reporting code changes.


List heuristic milestone: `greedy_list_construction` inserts missing required
values; `list_local_search_from` improves complete assignments with cached
contributions and explicit evaluation/step/accepted-move limits. Shared policies
include all five acceptors, first/best selection and rejected fallback. List tabu
keys touched entity-list indices; shuffled-variable order means shuffled entities.
Read docs/list-planning-contract.md for exact semantics. Thirteen provider methods
are fully adapted; range/constructor/metadata methods remain pending. Fourteen
original regressions plus one unit regression check neighborhood result sets,
budgets, pins, optional replacement, swaps, seeds, small exact-optimum quality,
callback counts and atomic selection failures. `list_routes` independently checks
a bounded 40-customer/four-vehicle solve: `0hard/-80soft`, 941 construction
evaluations/944 callbacks and 3,241 local evaluations/5,764 callbacks. Greedy
construction already meets the quality threshold, so local search accepts no
steps. It is not a Java runtime benchmark.
Logs: work/list-heuristic-check.log, work/list-heuristic-release.log and
work/list-heuristic-evidence.log. Required-list Java solving is now compared below.


List comparison milestone: `python3 scripts/benchmark_lists.py` verifies 64
original required-list construction/hill-climbing cases with exact Python optima,
and measures four 16/32-customer routing workloads with full scoring. Native/Java
original orders and construction ties may differ; do not force trace equality or
claim identical larger-instance quality. The dashboard shows a separate list
section with final scores/callbacks and unknown larger-workload optima. Small
validation: all 64 valid and equal-score, 51 optimum scores in each runtime. Four
original Rust protocol regressions and six Python evidence regressions added;
upstream method coverage remains 142/4855. Logs: work/list-comparison-check.log,
work/list-comparison-release.log, work/list-comparison-evidence.log. Refresh all
seven evidence sections after measured-source changes. The server must be
restarted after reporting-code updates; verify its live freshness digest.

Latest list-comparison dashboard QA: desktop and 390px mobile render all four
list workload cards, quality differences and unknown optima; no console errors
or horizontal overflow. Mobile timing labels and headings stack to avoid
collisions. HTML asset URLs include content-version queries after a cached CSS
reload was observed. Keep these queries current when editing dashboard assets.
All seven saved evidence sections were refreshed against the measured sources.


Entity-range milestone supersedes earlier pending-range notes: native
`ListEntity::with_value_range` accepts immutable subsets of the global registry;
omission means all values, an empty subset accepts none. All list operations,
neighborhoods and solvers enforce membership. Final composite range validation
matches pin semantics. Required orphan values or only-pinned destinations return
no-completion before callbacks; optional orphans can remain unassigned. Read
docs/list-planning-contract.md. Six provider methods and four fixture records
added with provenance; upstream coverage is 148/4855. Eight native regressions,
two protocol regressions and three Python evidence regressions added.

The list-solving adapter now accepts an optional eighth field for entity ranges.
Java uses public entity-level ValueRangeProvider annotations, with the full global
registry supplied per entity for legacy inputs. Small evidence expands to 128
valid/equal-score cases, with 109 optima in each engine. Larger timing workloads
still use unrestricted ranges. All seven measurements refreshed. Logs:
work/list-ranges-check.log, work/list-ranges-release.log, work/list-ranges-evidence.log.


Constraint-stream foundation milestone: eager typed multiset streams, deterministic
encounter order, predicate/hash-equality joins, exists/not-exists, position-unique
pairs, map/flat-map/concat/distinct, groups and integer summaries. IntCollector
supports duplicate retractions, wrapping i64 sums and count/distinct/min/max.
Score terminals reject structural contributions before addition. Five upstream
collector methods preserve all seven state assertions; graph methods remain
pending. Twelve original regressions cover 4,096 seeded collector edits, 256
join cases with forced hash collisions, all 81 partial scheduling assignments,
transaction rollback/commit, exact solving, construction and local improvement.
No stream-specific Java timing claim yet. Logs: work/stream-check.log,
work/stream-release.log, work/stream-evidence.log. All seven existing comparison
sections must be refreshed after measured source changes.


Incremental stream-operator milestone: TupleChange with stable generic IDs;
FilterNode caches predicate decisions; JoinNode caches both sides and ordered key
indexes; GroupNode retracts captured integers and removes empty groups; ScoreNode
caches individual ordinary contributions and totals. Connect typed output events
explicitly and drain in order. Self-joins accept either side order. Automatic DAG
scheduling, graph-wide rollback, exists/distinct propagation and solver move
binding remain pending. Standalone callback failures before mutation are atomic;
trait panics during mutation poison the node, so access fails closed. Rebuild a
pipeline after any partially delivered failure.

Seven upstream methods preserve all initial/incremental score and match states,
with two fixture/helper provenance records. Nine original regressions include
1,500 alternating-order self-join edits, 2,000 composed pipeline edits checked
against independent recomputation, affected-key callback counts, boundaries and
poison handling. One new API doctest. Logs: work/incremental-stream-check.log,
work/incremental-stream-release.log, work/incremental-stream-evidence.log.

All seven existing Java comparison/benchmark sections were refreshed. Live
dashboard verification matched the source fingerprint and displayed all seven
new methods under the passed filter, with no browser errors or horizontal
overflow. The existing dashboard server was reused.


Transactional stream graph milestone: StreamGraphBuilder creates a typed fixed
DAG of sources/filter/map/join/group/score nodes. Reused ports share nodes, FIFO
propagation drains before each apply returns, and typed constraint handles expose
individual match tuples/scores. Handles carry graph ownership; foreign handles
and duplicate names reject before registration. GraphTransaction checkpoints each
visited node once and restores source facts, indexes, summaries, contributions
and matches on drop/error/ordinary panic. Reads/updates/commit after a failed
transaction reject. Undo/commit do not replay callbacks. Checkpoints clone full
node state (not sparse undo); graph and callbacks are single-threaded, owned rows
are 'static and callbacks may borrow immutable data.

StreamScoreDirector binds one empty `(index, Option<V>)` source to FiniteProblem,
loads all variables and applies final composite values once per touched index.
Model validation happens before graph callbacks. Scored guard undo/commit covers
both model and graph, including failures after earlier variable updates. Other
sources are immutable during the session. Required nulls contribute structural
penalties; optional nulls do not. Existing construction and local_search_from
remain scoped-network APIs; generic stream integration is the next step.

Two adapted upstream join methods preserve full tuples, per-match scores and
named constraint totals before/after edits, with fixture/helper provenance. Twelve
original regressions include 1,000 multi-source transactions, 600 reconverging-path
updates, clone-failure injection, 2,000-node routing, all 81 partial assignments,
and exact/first-improvement solving. Changes-only search encounters a genuine
local optimum; adding swaps reaches the independently verified zero-score optimum.
Two API doctests added. Coverage is 162/4855 (3.337%), with 14/1045 constraint-stream
methods; no additional upstream credit for original regressions.
Logs: work/stream-graph-check.log, work/stream-graph-release.log,
work/stream-graph-evidence.log. Existing benchmark workloads do not measure streams.

All seven saved Java comparison/timing sections are current for this source
fingerprint. Live dashboard/API verification matched the verification digest,
showed both new graph methods under the passed filter and reported no browser
errors. The existing dashboard server was reused.


Stream search integration milestone: a private SearchDirector interface shares
construction/local-search algorithms across scoped and stream backends. New public
entry points consume a graph and empty planning source: greedy_stream_construction,
stream_local_search and stream_local_search_from. Validation and all existing
termination/acceptance/ordering behavior are preserved. Saved candidates contain
assignment plus full affected-node state; installation uses owned state and never
replays callbacks. Sparse undo remains pending. Terminal scoring invocations are
counted separately from score evaluations; graph preparation, predicates/key
callbacks and diagnostic work are not included in the terminal count.

StreamGraph/StreamScoreDirector assert_consistent resets all caches, rebuilds from
source facts and compares terminal match IDs/scores, individual cached contributions,
constraint totals and overall score, then restores the original state even on
failure. Graph transactions fail closed after a diagnostic error/panic. Diagnostic
rebuild work does not affect search metrics. List stream binding remains pending.

Six original integration regressions cover 24 construction and 200 local-search
configurations against separate full scoring, phase handoff, accounting without
replay, diagnostics, validation and zero limits, immutable preloaded scores and
empty/impossible models. One unit regression covers scorer errors/panics and
acceptor panics after saving an earlier winner. No new adapted-method credit;
coverage remains 162/4855. The original stream_coloring example verifies both
phases on 24 variables/4 colors against independent loops and the zero upper bound.
Logs: work/stream-search-check.log, work/stream-search-release.log,
work/stream-search-evidence.log, work/stream-coloring.log.

The stream_coloring example reaches verified 0hard/0soft in 602 combined score
evaluations and 3,623 terminal callbacks, including both phase initializations.
These callback counts are not a runtime comparison with Java.

All seven comparison/timing sections were refreshed for stream-search integration.
The live dashboard verification digest matches the current source and test log;
coverage and benchmark sections render without browser errors or horizontal
overflow. Existing Java benchmark workloads still do not measure stream scoring.


Persistent storage milestone: independently implemented SnapshotMap uses immutable
AVL nodes and shared key/value allocations with atomic reference counts. Internal
updates copy and rebalance only a search path; cloning a map needs no key/value
Clone bounds. Source/filter rows, join side/index/buckets, group input/collectors,
integer multiplicities and terminal contribution/match maps now use this storage.
Node checkpoint and saved-candidate clones share roots; graph restoration moves
roots back without user callbacks. Old whole-node cloning descriptions above are
historical. Assignment copies/model validation/structural scans remain linear, and
join updates still materialize every affected match. No constant-time move claim.

Public inspection source_rows/constraint_matches now returns &SnapshotMap rather
than &BTreeMap. It supports ordered iteration, keys/values, lookup/indexing,
length/empty and first/last entries, and stable inexpensive clones across updates.
Map writes are crate-private. SnapshotMap and IntCollector preserve Send/Sync for
appropriate key/value types; graph execution remains single-threaded.

Three storage unit tests verify 20,000 operations vs BTreeMap with balance/height/
size/order invariants, retained snapshots, non-Clone keys/values, bounded new paths
on 8,192 keys and comparison-panic atomicity. Two original graph/collector tests
verify equal changed-row clone counts at 16 and 4,096 source rows, snapshot
isolation, rollback/commit, multiplicities/extrema and Send/Sync. One API doctest.
No upstream credit added: 162/4855 (3.337%). Logs: work/persistent-storage-check.log,
work/persistent-storage-release.log, work/persistent-storage-evidence.log,
work/persistent-storage-example.log. Existing benchmarks do not measure streams.

Persistent-storage verification passed all 345 Rust tests in debug and release,
eight doctests and 31 Python regressions. stream_coloring retains 0hard/0soft and
602 evaluations / 3,623 terminal calls. All seven Java comparison/timing sections
are current; live dashboard/source digests agree and browser checks found no
errors or horizontal overflow. The existing server was reused.

Current check totals: 556 Rust tests (549 integration + seven unit), seventeen doctests,
and 48 Python reporting/validator tests. Refresh all nine evidence artifacts after
measured source changes, including `differential_streams.py` and
`benchmark_streams.py`, then regenerate the report with the captured test log.

Stream evidence milestone: all nine saved comparison/timing sections are current.
128 stream cases / 2,899 states pass; all 64 stream search cases have equal scores,
and both engines reach 52 independently proven optima. Final timing medians in ms
(Rust / Java): hill32 180.560 / 31.951; hill96 595.365 / 85.196;
tabu32 191.274 / 45.901; tabu96 633.120 / 57.869. These are workload-specific,
shared-machine end-to-end results including documented setup differences.
Debug/release all-target checks pass. Dashboard server restarted as PID 13462;
live/report fingerprints agree. Desktop and 390px browser checks show the four
new cards, current evidence, correct callback labels and no errors or overflow.

Existence milestone: indexed Exists/NotExists nodes now integrate with graph
transactions, construction and local search. Thirteen adapted cases plus seven
original regressions are added. Verified totals: 365 Rust tests, nine doctests,
39 Python tests, and 175/4,855 upstream passes. No Java existence timing claim.

Existence verification is complete: debug/release gates pass and all nine saved
Java comparison/timing artifacts were refreshed for the new sources. Upstream
is unmodified. Live dashboard/source fingerprints match; the browser shows
27/1,045 stream passes (2.58%) and the passed existence methods in search.
The existing dashboard server (PID 13462) was reused without restart because
reporting/fingerprint Python code did not change. Logs: work/existence-check.log,
work/existence-release.log and work/existence-evidence-{a,b}.log.

Distinct milestone: persistent duplicate elimination now integrates with graph
transactions, construction and local search, with twenty adapted upstream methods
and seven original regressions. Verified totals: 392 Rust tests, ten doctests,
39 Python tests; 195/4,855 upstream passes and 47/1,045 stream passes.

Distinct verification: debug/release gates pass. All nine comparison/timing
artifacts were refreshed; live dashboard/report hashes agree. Browser verification
shows 47/1,045 stream passes (4.50%) and all twelve newly adapted distinct-named methods passing
(the other eight adapted methods check mapping multiplicity). No browser errors
or horizontal overflow. Existing server PID 13462 was reused; upstream stays
unmodified. Logs: work/distinct-check.log, work/distinct-release.log and
work/distinct-evidence-{a,b}.log. The first batch ended with signal 15 after
saving its timing artifacts; the uncompleted list oracle ran in the second batch
and passed. No successful measurements were inferred from an interrupted command.


Composable collector increment: ReversibleCollector, CountCollector, SetCollector,
PairCollector and CollectNode now support keyed/global aggregation through
StreamGraphBuilder::group_collect_with / collect. Empty global groups emit no row;
standalone empty finish remains zero/empty. Public Community boundary probing
corrected the initially assumed empty-row behavior. The reproducible original
AggregationBoundaryOracle runs three full-update states from differential_streams.
Sixteen adapted groupBy_0Mapping{1..4}Collector methods across all four arities and
seven original regressions are added. Custom collectors must have isolated Clone,
empty Default and immutable finished outputs. See the stream contract.

Collector verification: debug/release all-target checks pass (415 Rust tests),
with eleven doctests and 39 Python regressions. Coverage is 211/4,855 (4.346%)
and streams 63/1,045 (6.029%). All nine Java comparison/timing artifacts are
current. The three-state Community aggregation probe and 128/2,899 stream
comparisons pass; all 64 stream search cases remain valid with equal scores.
Current stream timing medians in ms (Rust / Java): hill32 139.279 / 20.251,
hill96 466.240 / 29.956, tabu32 128.804 / 16.034, tabu96 510.881 / 28.929.
These timings exercise the specialized join/group workload, not generic collectors.

The old dashboard server was no longer listening. Restarted it as PID 21564,
exec session 31475. Browser shows 211 passes and all sixteen new global-aggregation
methods under the passed search filter. Live/report hashes agree; no browser errors
or horizontal overflow. Logs: work/collecting-check.log, collecting-release.log,
collecting-{stream,list,score,solver}-evidence.log and corresponding *-performance.log,
plus collecting-heuristic-performance.log. Upstream remains unmodified.


List-stream increment: ListStreamSources binds entity lists and all value identities
with owned position/neighbor snapshots. ListStreamScoreDirector validates before
callbacks, conservatively updates touched-list membership and rolls back graph
and model together. Required structure follows the native list model. New public
greedy_list_stream_construction and list_stream_local_search_from share the old
list algorithms through private ListSearchDirector. Saved winners install roots
without callback replay. Read the stream/list contracts before modifying this.

Four adapted assigned/including-unassigned list stream methods, nine original
integration regressions, one unit regression and one doctest are added. Native
list_move_oracle now emits cached stream shadows to the existing 256-case Java/
Python comparison and checks entity rows plus its native position score.
No Java/Rust list-stream score or solver timing claim; existing timings retain
their workloads. stream_list_routes is an independently verified native example.

List-stream verification: debug/release all-target gates pass (429 Rust tests:
215 adapted + 207 original integration + seven unit), twelve doctests and 39 Python
regressions. stream_list_routes independently verifies 0hard/-80soft for 40 values:
941 construction evaluations / 7,824 terminal calls, then 3,241 local evaluations /
63,404 calls and zero improving steps. This is a native quality check, not timing.

All nine saved Java comparison/timing artifacts are current for this source.
The updated list oracle passes 256 cases / 907 operations with cached stream
views; other correctness counts remain unchanged. Logs: work/list-stream-check.log,
list-stream-release.log, list-stream-example.log, list-stream-{list,stream,score,
solver}-evidence.log and corresponding *-performance.log, plus
list-stream-heuristic-performance.log. Current basic-stream medians in ms
(Rust / Java): hill32 117.144 / 18.974, hill96 499.187 / 32.700,
tabu32 124.010 / 15.436, tabu96 448.805 / 41.126. Keep these workload-specific.

Existing server PID 21564 was reused. Live/report verification hashes agree;
browser shows 215/4,855 (4.43%) overall and 67/1,045 (6.41%) streams, with all four
new listVar methods under the passed filter. No browser errors or horizontal
overflow. Upstream remains unmodified; no Enterprise source/artifact used.


Stream optimization increment: same-key JoinNode updates retain index membership
and emit one put per surviving pair. Changed keys still retract old pairs before
inserting new pairs. Graph terminals now store payloads/contributions together in
one persistent map plus the cached total. Public standalone ScoreNode is unchanged.
Four original regressions add same-key/changed-key, self-join, downstream group,
error/panic/undo, snapshot and zero/wrapping-score coverage. Totals: 433 Rust tests
(215 adapted + 211 original integration + seven unit), twelve doctests, 39 Python.
Coverage remains 215/4,855; streams remain 67/1,045.

Coarse native profiling is reproducible with scripts/profile_streams.py; it builds
only an isolated instrumented copy. The 96-variable, 50-step hill workload drops
from 5,128,231 AVL allocations at a898c2c to 2,103,775, with consistent scores.
Instrumented time includes counter/timer overhead and is not a benchmark result.
An uninstrumented before/after study used three fresh processes, two warmups and
five measured solves, alternating binary order per fork. All samples independently
rescore and retain identical full assignments. Median ms before / after:
hill32 154.434 / 71.435; hill96 622.622 / 298.640;
tabu32 155.230 / 70.118; tabu96 683.095 / 293.319.
These native workloads show 2.08–2.33x improvement on this shared machine; no general
solver or Java-relative claim follows. Raw samples, binary hashes, baseline commit
and source fingerprint are in work/stream-optimization-comparison.json; profile
output is work/native-stream-profile.json. Build/check/release logs use the
work/join-replacement- prefix. Neither study grants upstream test credit.

Refreshed Java stream medians in ms (Rust / Java): hill32 79.718 / 31.043,
hill96 278.786 / 47.126, tabu32 74.576 / 28.358, tabu96 281.708 / 58.530.
Rust remains slower on these end-to-end incremental workloads despite the native
improvement. The 32-variable timing cases finish at [0,0]; 96-variable cases finish
at [-44,-484] in both engines within the fixed 50 steps. All 64 small search
checks have valid equal scores. Keep quality and setup differences visible.

Verification is complete: debug/release all-target gates pass, and all nine saved
comparison/timing sections have current source fingerprints. Live server PID
21564 was reused; its verification fingerprint matches the report and source.
Browser shows 215/4,855 and all four refreshed stream timing cards with explicit
quality/callback counts. No browser errors or horizontal overflow. Both native
stream examples retain their previous scores/evaluation/callback counts.
Logs: work/join-replacement-{check,release,examples,profile,comparison}.log,
join-replacement-stream-{evidence,performance}.log, join-replacement-list-refresh.log,
join-replacement-core-refresh.log and join-replacement-heuristic-performance.log.
The upstream submodule remains unchanged; no new adapted methods or Enterprise
artifacts were used.


Map collector increment: MapCollector<K,V,C> independently accumulates an inner
collector per ordered key and caches finished outputs in a persistent map. Explicit
occurrence counts keep zero/empty results present until their last input disappears.
Clones/finished snapshots share roots; only changed inner collectors are cloned.
Candidate groups and finished output roots install together after successful work.
Maps compose with sets, integer summaries, pairs, counts and nested maps. Existing
generic graph aggregation integrates them with transactional scoring and search.
Read docs/constraint-stream-contract.md before changing this layer.

Twenty-five upstream methods cover map/sorted-map/merged-sum insertions, duplicates,
updates and removals across four arities. Five original regressions cover 4,500
graph edits, nested snapshots, failures/rollback and search quality. New totals:
240 adapted methods, 216 original integration tests, seven unit tests (463 Rust
all-target tests), thirteen doctests and 39 Python regressions. Verified upstream
coverage is 240/4,855 (4.943%) and stream coverage 92/1,045 (8.804%). Existing Java timing workloads
retain their scope and do not benchmark the new map collector.

Map collector verification completed: debug/release all-target checks pass,
all nine saved Java comparison/timing sections are current, and the live server's
verification hash matches source/report. Browser checks show 4.94% overall,
8.80% streams, all seventeen toMap and eight toSortedMap methods passing under
search. No browser errors or horizontal overflow. Existing server PID 21564 was
reused. The pinned upstream submodule remains unmodified.

Current basic-stream medians in ms (Rust / Java): hill32 92.621 / 24.442,
hill96 279.846 / 54.327, tabu32 69.748 / 27.442, tabu96 341.567 / 40.774.
Scores agree; both 96-variable cases remain infeasible within the fixed step
limit. These workloads do not exercise maps, so no map-specific performance
claim follows. Logs: work/map-collector-{check,release,focused,regression}.log,
map-collector-stream-{evidence,performance}.log, map-collector-list-refresh.log,
map-collector-core-refresh.log and map-collector-heuristic-performance.log.


Collected-list increment: ListCollector<I,V> and immutable ListSnapshot<I,V>
retain distinct occurrences in ascending stable-ID order. Graph collect_list /
group_list supply tuple IDs automatically and reuse generic transactional
aggregation. Private collector mapping now receives the ID as well as the payload;
existing public generic mappings retain their prior signatures. GroupedListStreamPort
names the keyed-list output type. Native order is independent of delivery history,
so full rebuilds reproduce even order-sensitive scores. It does not claim general
Java iteration order, which the public collection API leaves unspecified.

Snapshot len/index/get/iteration are read-only; rank lookup uses the persistent
AVL subtree sizes. Clone/finish share roots. Equality, order and hash compare
ordered values, ignoring IDs. Duplicate IDs reject before mutation; removal uses
ID alone. Twelve adapted list/ArrayList methods preserve all ordered assertions.
Six original regressions cover 8,000 occurrence edits, 3,000 graph writes,
rollback/failures, composition, distinct keys and construction/search quality.
Verified totals: 481 Rust tests (252 adapted + 222 original integration + seven unit),
fourteen doctests and 39 Python regressions. Coverage: 252/4,855 overall and
104/1,045 streams. Existing timing workloads do not exercise this new feature.

Collected-list verification is complete: debug/release gates pass, all nine saved
Java comparison/timing sections are current, and live/report/source fingerprints
agree. Browser shows 252/4,855 (5.19%) overall, 104/1,045 (9.95%) streams, eight
toList and four toCollectionUpdate methods passing under search. No browser errors
or horizontal overflow. Existing server PID 21564 was reused. The upstream pin
and checkout remain unmodified.

Current basic-stream medians in ms (Rust / Java): hill32 57.930 / 18.796,
hill96 294.888 / 34.742, tabu32 68.278 / 22.478, tabu96 285.035 / 37.897.
Scores agree; both 96-variable cases remain infeasible at the fixed step cap.
These timings do not measure collected lists. Logs:
work/list-collector-{check,release,focused,regression}.log,
list-collector-stream-{evidence,performance}.log, list-collector-list-refresh.log,
list-collector-core-refresh.log and list-collector-heuristic-performance.log.


Integer-average increment: AverageCollector wraps captured integer multiplicities
and returns Option<f64>. Public Community probing established checked insertion
and removal sums, checked replacement differences, and wrapping addition of a
valid replacement difference. Native try methods preserve state on errors;
trait methods panic and participate in graph rollback. Rebuild diagnostics may
reject a prefix after a wrapping replacement or a different replay order. Read
the average arithmetic section of docs/constraint-stream-contract.md.

ReversibleCollector now has a default isolated replace(old,new) method. CollectNode
uses it for same-key replacements; PairCollector and MapCollector delegate it
through composition. Key changes retain removal/insertion behavior. This avoids
incorrectly rejecting replacement when an intermediate removal would overflow.
Eight adapted methods and five original regressions are added. New totals:
494 Rust tests (260 adapted + 227 original integration + seven unit), fifteen
doctests and 42 Python regressions. Overall coverage: 260/4,855; streams: 112/1,045.

AverageCollectorOracle.java and examples/average_oracle.rs are original adapters,
not copied upstream fixtures. scripts/average_collector_checks.py independently
validates 137 cases / 3,936 observations, exact double bits and expected overflow
rejections. differential_streams.py runs them, saves full inputs/outputs under
average_collector_checks, and the dashboard adds their separate evidence counts.
No extra upstream credit, Java post-error transactional behavior or timing claim.
Average verification is complete: debug and release all-target gates pass with
494 tests; fifteen doctests and 42 Python regressions pass. All nine comparison
and timing sections are current, with source/report/live verification fingerprints
matching. Average checks include 68 expected overflow rejections. Browser checks
show 260/4,855 (5.36%) overall, 112/1,045 (10.72%) streams, all eight adapted
average methods passing, and the separate 137-case evidence with its own seed.
Desktop 1280px and mobile 390px show no horizontal overflow or browser errors.
The script asset version was updated after observing a cached prior copy.
The prior dashboard process had stopped; current server PID 27800 listens on
127.0.0.1:8765. Browser verification sessions are closed. Upstream is unmodified.

Current basic-stream medians in ms (Rust / Java): hill32 55.034 / 19.477,
hill96 214.127 / 29.560, tabu32 51.670 / 12.598, tabu96 203.631 / 25.765.
Scores agree; both 96-variable cases remain infeasible at the fixed step cap.
These workloads do not time average collectors; no average performance claim.
Logs: work/average-{check,release,focused}.log,
average-stream-{evidence,performance}.log, average-list-refresh.log,
average-core-refresh.log and average-heuristic-performance.log.


Signed-duration increment: SignedDuration, DurationSumCollector and
DurationAverageCollector implement checked normalized arithmetic, persistent
multiplicities and composed graph collection. Average finishing narrows the total
to i64 nanoseconds before division toward zero; it may fail after a successful
update. Graph transactions restore both update and finish failures. Read the
signed-duration section of the stream contract for intermediate overflow rules.
Sixteen fully adapted methods and six original regressions are added. Verified
new totals: 516 Rust tests (276 adapted + 233 original integration + seven unit),
sixteen doctests and 45 Python regressions; streams 128/1,045.
Original duration adapters check 288 cases / 7,650 observations, including 142
expected update/finish errors. Integrated into differential_streams and dashboard
as separate evidence with seed 20261031. Full debug/release gates pass and all
nine comparison/timing sections are current. Source/report/live fingerprints
agree. Browser checks at desktop 1280px and mobile 390px show 276/4,855 (5.68%)
overall, 128/1,045 (12.25%) streams, sixteen duration methods passing under search,
and correct supplemental counts/seed. No browser errors or horizontal overflow.
Server PID 27800 was reused; browser sessions are closed. Upstream is unmodified.

Current basic-stream medians in ms (Rust / Java): hill32 53.812 / 15.565,
hill96 208.301 / 30.494, tabu32 54.741 / 14.864, tabu96 199.678 / 28.108.
Scores agree; both 96-variable cases remain infeasible at the fixed step cap.
These timing workloads do not exercise durations. Logs:
work/duration-{check,release,regression,build}.log,
duration-stream-{evidence,performance}.log, duration-list-refresh.log,
duration-core-refresh.log and duration-heuristic-performance.log.


Arbitrary-precision increment: new Decimal value and BigInt/Decimal sum/average
collectors. num-bigint 0.5.1 supplies arithmetic storage; no Timefold production
code was read or ported. Native collector/decimal logic is independent. Decimal
Eq includes scale and Ord uses it to break numeric ties; numeric_cmp compares value
alone. Averages round half-even at the cached sum scale. Decimal scale persists
after removals, including empty standalone state. Normalize inputs to fixed scale
for history-independent solver objectives/rebuilds. Read the stream contract.
Thirty-two adapted methods and eight original regressions added; verified totals:
556 Rust tests (308 adapted + 241 original integration + seven unit), seventeen
doctests and 48 Python regressions. Constraint streams: 160/1,045. Original
public-API adapters agree on 304 cases / 16,298 observations with Python exact
recomputation (seed 20261103), including 1,001-digit inputs and decimal scale
history. Full debug/release gates passed; all nine comparison/timing sections are
current and source/report/live fingerprints match. Browser checks show 308/4,855
(6.34%) overall, 160/1,045 (15.31%) streams, all thirty-two new methods passing
under search, and correct supplemental counts/seed. Desktop 1280px and mobile
390px have no browser errors or horizontal overflow. Server PID 27800 was reused;
browser sessions are closed. Upstream remains unmodified.

Current basic-stream medians in ms (Rust / Java): hill32 54.245 / 16.493,
hill96 204.738 / 29.140, tabu32 54.184 / 19.425, tabu96 204.543 / 34.712.
Scores agree; both 96-variable cases remain infeasible at the fixed step cap.
These workloads do not exercise arbitrary-precision collectors. Logs:
work/big-number-{check,release,adapted,regression,build}.log,
big-number-stream-{evidence,performance}.log, big-number-list-refresh.log,
big-number-core-refresh.log and big-number-heuristic-performance.log.


Mixed-model increment: MixedProblem<B,L> composes validated basic/list domains;
MixedAssignment exposes both read-only. MixedMove contains one (possibly composite)
move per component. Candidate preparation is isolated; AppliedMixedMove commits or
rolls back both components. Domain/pin validation occurs at component boundaries;
list operations retain their per-operation uniqueness/index rules. Snapshotting
is a baseline, not incremental mixed scoring. Read docs/mixed-planning-contract.md.

exhaustive_mixed_search enumerates the complete product space with one objective
and one evaluation budget. Private visitors extracted from both existing exact
engines preserve their original order and budget behavior. Lists vary outside,
basic assignments inside; ties retain the first candidate. Impossible components
short-circuit without enumerating the other space. Required/optional domains,
pins, empty spaces, exact final budget exhaustion and structural score rejection
retain their component semantics. Mixed heuristics and incremental/stream scoring
remain pending, as do public-API Java mixed solver comparisons and mixed timings.

Eight original integration regressions independently verify 128 complete product
spaces through subset/permutation/split enumeration, atomic errors/undo/commit/panic,
composite pin/range restoration, coupled objectives and all small budget boundaries.
The original mixed_routes example jointly chooses two depots and ordered routes,
proving 0hard/-4soft over 96 states with a separate distance lower bound. No copied
fixtures or adapted methods were added. Inspected upstream mixed exhaustive tests
also require Java phase/step/trace assertions, so they remain pending. Coverage
stays 308/4,855 overall and 160/1,045 streams.

Debug/release all-target gates pass with 564 Rust tests (308 adapted + 249 original
integration + seven unit), eighteen doctests and 48 Python regressions. The release
mixed_routes example passes. Logs: work/mixed-{check,release,example}.log.

All nine existing Java comparison/timing sections are refreshed and current for
this source. Live server PID 27800 was reused; live/report/source verification
fingerprints agree. Browser checks at desktop 1280px and mobile 390px show
308/4,855 (6.34%) overall and 160/1,045 (15.31%) streams, refreshed timings/quality,
no stale-result messages, no browser errors and no horizontal overflow. Browser
session closed; upstream submodule remains unmodified.

Current basic-stream medians in ms (Rust / Java): hill32 51.578 / 13.802,
hill96 214.335 / 29.353, tabu32 53.294 / 14.096, tabu96 208.172 / 32.162.
Scores agree; both 96-variable cases remain infeasible at the 50-step cap. Existing
list construction at 32 customers returns different scores (Rust 0hard/-1080soft,
Java 0hard/-1062soft); the dashboard retains that quality difference beside timing.
None of these timings measures mixed models. Evidence logs:
work/mixed-stream-{evidence,performance}.log, mixed-list-refresh.log,
mixed-core-refresh.log and mixed-heuristic-performance.log.


Mixed scoped-scoring increment: MixedConstraintNetwork<B,L,S> registers Basic,
ListEntity and ListValue scopes with ordered/repeated callback arguments. Value
indices refer to registry positions. MixedScoreDirector caches named contributions;
combined moves invalidate the union of basic dependencies, touched lists and values
in those lists before/after the composite. Neighbor/index changes are covered;
each affected constraint runs once and constants stay cached. ScoredMixedMove
commits state and deferred cache updates without callback replay. Errors and
ordinary unwinding restore both components. Read docs/mixed-planning-contract.md.

Structural state combines missing required basic variables and list values;
optional unassignment adds no penalty. Ordinary levels retain AdditiveScore
arithmetic. Invalid scopes/registrations/shapes/models reject before callbacks;
assert_consistent checks individual contributions, including cancelling errors.
Only callbacks are incremental: snapshots, validation, missing-value counting and
dependency discovery still copy/scan model state. Mixed heuristic/stream search,
Java mixed scoring comparisons and mixed timing measurements remain pending.

Six original regressions cover 1,200 seeded compound moves, including 400 list-only
rounds that cannot mask value-dependency bugs through basic invalidation. Every
candidate is compared to a separate direct formula and full calculation; commits
and rejection also check the base cache. Other cases cover partial required and
optional states, full/prefix/basic pins, non-dense IDs, repeated scopes, no-op
writes, unchanged callback reuse, invalid metadata/moves, later callback errors
and panics, caller unwinding, cancelling corruption and BendableScore wrapping.
One public API doctest exercises a coupled constraint and commit. No upstream
method credit is added: coverage remains 308/4,855 overall and 160/1,045 streams.

Full debug/release gates pass: 570 Rust tests (308 adapted + 255 original integration
and seven unit), nineteen doctests and 48 Python regressions. Logs:
work/mixed-scoring-{check,release}.log. The suite was recaptured after strengthening
the seeded rounds to include list moves that leave basic variables untouched.

All nine saved comparison/timing sections are current for this source; live/report/
source verification fingerprints agree. Existing server on 127.0.0.1:8765 was
reused. Browser checks at 1280px and 390px show 308/4,855 (6.34%) overall,
160/1,045 (15.31%) streams, refreshed timings, no stale-result messages, no browser
errors and no horizontal overflow. Browser session closed; upstream unmodified.

Current basic-stream medians in ms (Rust / Java): hill32 56.525 / 21.243,
hill96 198.941 / 25.197, tabu32 54.201 / 19.116, tabu96 199.989 / 27.755.
Scores agree; both 96-variable cases remain infeasible at the 50-step cap. The
32-customer list construction scores still differ: Rust 0hard/-1080soft, Java
0hard/-1062soft. No mixed scoring timing or speedup claim follows. Logs:
work/mixed-scoring-stream-{evidence,performance}.log,
mixed-scoring-list-refresh.log, mixed-scoring-core-refresh.log and
mixed-scoring-heuristic-performance.log.


Mixed local-search increment: mixed_moves lazily composes native basic and list
neighborhoods. It emits basic changes/swaps first, then list moves, then optional
pairs of one basic and one list move. The product is lazy and preserves complete
states, pins and ranges. MixedLocalSearchConfig wraps LocalSearchConfig and the
joint_moves switch (false by default). mixed_local_search_from requires a complete
initial state; mixed score-aware construction remains pending.

The shared native acceptance policies, budget semantics, selection and fallback
rules now operate under one mixed objective. Basic/list order shuffles use one
seeded stream; family/value/index order stays fixed. Tabu keys are basic indices
and basic_count+list_entity_index, preventing collisions between the components.
MixedScoreDirector::select_best retains assignments and sparse updates inside an
exclusive borrow, installing winners without callback replay. Errors/panics after
an earlier saved candidate leave the base and caches intact. Global best survives
worsening/fallback steps; no local-search termination proves global optimality.
Read docs/mixed-planning-contract.md before changing these semantics.

Seven original integration regressions and one internal selection regression are
added. Checks include independent complete neighbor sets, a joint move escaping
a coupled local trap, exact-optimum comparison, pins/ranges/lazy cloning, budgeted
prefix winner reuse, zero-budget validation, separate tabu namespaces, fallback
accounting and earliest-best retention. Fifteen seeded policy/order combinations
repeat and reach the small instance's exact optimum. mixed_routes now verifies
both exact search and local search against a separate direct score and the known
0hard/-4soft optimum. Local search uses 127 evaluations / 234 callbacks / one step.
These are native quality checks, not mixed Java performance or trace evidence.

Full debug/release gates pass with 578 Rust tests (308 adapted, 262 original
integration, eight unit), nineteen doctests and 48 Python regressions. Release
mixed_routes passes. No upstream cases/fixtures were adapted: coverage remains
308/4,855 overall and 160/1,045 streams. Java mixed comparisons, mixed construction
and mixed stream scoring remain pending. Logs: work/mixed-search-{check,release,
example}.log.

All nine comparison/timing sections are refreshed for the current source. Live,
report and source fingerprints match. Browser verification at 1280px and 390px
shows 308/4,855 (6.34%) overall, refreshed timing cards, no stale messages, browser
errors or horizontal overflow. Existing local server was reused; browser session
closed. Upstream checkout/pin remain unmodified.

Current basic-stream medians in ms (Rust / Java): hill32 49.435 / 18.009,
hill96 205.529 / 29.910, tabu32 51.257 / 16.382, tabu96 207.254 / 33.301.
Scores agree; both 96-variable workloads remain infeasible at the 50-step cap.
The 32-customer construction score difference remains visible (Rust -1080 soft,
Java -1062 soft, both hard zero). These workloads do not measure mixed search.
Logs: work/mixed-search-stream-{evidence,performance}.log,
mixed-search-list-refresh.log, mixed-search-core-refresh.log and
mixed-search-heuristic-performance.log.


Mixed construction increment: greedy_mixed_construction fills missing required
basic/list values through one scoped contribution director. Configuration wraps
ConstructionConfig plus BasicThenLists (default) or ListsThenBasic. All initial
basic values and list relative order survive; optional unassignment is retained.
One budget counts the initial score and every candidate across both phases;
prefix winners reuse saved contributions without callback replay. Full diagnostics
are outside reported work. Empty required domains/unreachable required list values
short-circuit before callbacks, including at zero budget. solution() requires both
completeness and a calculated score. See docs/mixed-planning-contract.md.

Seven original integration methods check both orders, shared budgets and ties,
early-pick semantics, pins/ranges/initial values, optionality, shape/impossibility,
empty/complete models, failures after partial progress and a coupled quality trap.
The mixed_routes example now starts empty with both orders and improves each to
the independently proven 0hard/-4soft optimum. Basic-first construction starts at
0hard/-38soft, using 141 combined evaluations / 249 callbacks; list-first starts
at 0hard/-18soft, using 173 / 313. These counts include the local-phase initial
rescore and do not measure runtime or Java mixed equivalence.

Full debug/release all-target checks pass with 585 Rust tests (308 adapted,
269 original integration, eight unit), twenty doctests and 48 Python regressions.
Release mixed_routes passes. No upstream tests/fixtures were adapted; coverage
remains 308/4,855 overall and 160/1,045 streams. Mixed stream scoring and Java mixed
comparisons/timings remain pending. Logs: work/mixed-construction-{check,release,
example}.log. The overall solver rewrite remains unfinished.

All nine comparison/timing sections are refreshed for this source. Source/report/
live fingerprints agree. Browser checks at 1280px and 390px confirm 6.34% overall,
current timing cards, no stale messages, application errors or horizontal overflow.
The existing server was reused; the verification browser was closed. Upstream
checkout and pin remain unmodified.

Current basic-stream medians in ms (Rust / Java): hill32 55.929 / 17.037,
hill96 208.148 / 29.890, tabu32 51.625 / 13.896, tabu96 204.558 / 31.872.
Final scores agree; both 96-variable workloads remain infeasible at the 50-step cap.
The 32-customer construction score difference remains (Rust -1080 soft, Java
-1062 soft, both hard zero). None of these timings measures mixed models. Logs:
work/mixed-construction-stream-{evidence,performance}.log,
mixed-construction-list-refresh.log, mixed-construction-core-refresh.log and
mixed-construction-heuristic-performance.log.

Mixed stream increment: MixedStreamSources groups the basic source with the two
ListStreamSources. MixedStreamScoreDirector validates both components before
callbacks, then delivers final touched basics, entity lists and affected value
views, in that order. Joins may observe intermediate source updates. Drop/error/
ordinary unwind restores the combined assignment and graph roots. Initial and
candidate structural counts use the existing checked combined missing-count rule.
No custom shadows or Java mixed equivalence are implied.

MixedSearchDirector is now the internal boundary shared by scoped and stream
construction/local search. Both reuse exactly the same algorithm, configuration,
ordering, budgets, global-best retention and acceptance rules. Stream candidates
save persistent graph roots and install without callback replay. Phase counters
count terminal weights including intermediate joins; preprocessing and rebuild
checks remain excluded. Read docs/mixed-planning-contract.md before editing.

Seven original integration regressions cover 800 seeded moves (20261106), direct
scores, all bound source rows, pins/ranges, required/optional values, partial
composites, exact retained matches, empty moves, validation at zero budget,
impossible models, graph errors/panics, caller unwind and rebuild recovery.
Construction agrees with scoped scoring for both orders, four early-pick policies
and budgets 0..7. Local search agrees across five acceptors, two selections,
joint moves on/off and eight budgets (seed 20261107). One new internal regression
checks failure after saving an earlier joint candidate.

Full debug/release all-target gates pass: 593 Rust tests (308 adapted + 276
original integration + nine unit), twenty-one doctests and 48 Python regressions.
The release mixed_routes example checks both stream orders against independent
full scores and the proven 0hard/-4soft optimum. Basic-first uses 141 evaluations /
353 terminal calls; list-first uses 173 / 469, including phase initialization.
These are native correctness/work counts, not mixed Java or runtime comparisons.
No upstream method credit was added: coverage stays 308/4,855 overall and
160/1,045 streams. Java mixed comparisons/timings, custom shadows and further
constraint operators remain pending. Logs: work/mixed-stream-{focused,check,
release,example,doctest}.log. The goal remains active and unfinished.

All nine evidence/timing sections have been refreshed for this source. Source,
report and live fingerprints match. The prior dashboard server process had exited
and port 8765 refused connections; after confirming its PID and listener were gone,
it was restarted (PID 28636, exec session 85723, work/dashboard-server.log).
Browser/API/data checks now pass at 1280px and 390px: HTTP 200 responses, 6.34%
overall, current timings, no stale messages, errors or horizontal overflow. Desktop
screenshot: work/mixed-stream-dashboard.png. Verification browser was closed.

Current basic-stream medians in ms (Rust / Java): hill32 67.139 / 23.190,
hill96 296.657 / 39.627, tabu32 61.229 / 26.001, tabu96 271.186 / 50.292.
Scores agree; both 96-variable workloads remain infeasible at the 50-step cap.
List32 construction retains the quality difference (Rust -1080 soft, Java -1062
soft, both hard zero). These existing workloads do not benchmark mixed models.
Logs: work/mixed-stream-stream-{evidence,performance}.log,
mixed-stream-list-refresh.log, mixed-stream-core-refresh.log and
mixed-stream-heuristic-performance.log. Upstream checkout/pin remain unmodified.
