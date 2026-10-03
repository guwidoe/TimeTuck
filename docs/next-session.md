# TimeTuck implementation handoff

## Active user goal

Rewrite Timefold behavior in independent native Rust, using pinned Community tests
as specifications. Also maintain a visual dashboard of coverage by subsystem and
performance versus Java. The whole rewrite remains unfinished; do not mark the
active goal complete after one slice. No zero-mistake guarantee is possible.

## Current implementation

- Score levels support i64 and Decimal through ScoreLevel, including decimal
  progress and annealing estimates. SimpleDecimalScore, HardSoftDecimalScore and
  HardMediumSoftDecimalScore now implement fixed decimal families; Bendable remains pending.
- Rust library has SimpleScore, HardSoftScore, HardMediumSoftScore and
  BendableScore<H,S> with structural
  values, ordering/feasibility, parsing/formatting, arithmetic and checked arithmetic.
- 543 fully adapted source test methods (134 score + 10 basic move + 34 list move + 39 provider + 11 acceptor + 4 construction + 106 collector + 185 stream + 20 termination), recorded in
  `compat/provenance.json`.
- Finite domains, initial values, pinning, optional values and reversible
  change/swap/composite transactions. Read `docs/planning-contract.md`.
- Iterative exact search with evaluation budgets, cooperative elapsed-time limits and truthful
  exhaustion/optimality reporting. Original `tiny_schedule` example.
- Mixed basic/list models, atomic combined moves and exact joint search over the full
  product space. Read `docs/mixed-planning-contract.md`. `mixed_routes` proves
  `0hard/-4soft` over 96 states. Mixed local search now supports joint moves and all native acceptance policies;
  mixed greedy construction supports explicit component ordering. Scoped and transactional
  stream scoring share construction/local-search algorithms. Original Java mixed move/score
  comparisons now cover 128 cases / 2,378 states; original mixed solver-quality comparisons now exist as described below.
- Native ordered-list model with unique ownership, optional unassignment, prefix/full
  pins and derived owner/index/previous/next views. Atomic assign/change/swap/span
  unassign/composite transactions, plus iterative bounded exact list enumeration.
  Read `docs/list-planning-contract.md`. `tiny_routes` proves `0hard/-12soft`
  after 5,040 complete states.
- Atomic SubListChange relocates/reverses contiguous spans with post-removal
  indices. SubListSwap exchanges nonempty disjoint spans with pre-move indices,
  optional reversal of both blocks, and different lengths. An opt-in bounded span neighborhood feeds list and mixed search in
  both backends, including joint moves; defaults retain single-value neighborhoods.
- 512 Java/Rust/Python list-move cases (1,795 operations) check candidates,
  undo, commit and all four built-in shadow relationships; original seed 20261003,
  additional span relocation/reversal seed 20261124 and span-swap seed 20261125.
- Dedicated tail-swap and two-opt providers lazily enumerate nonempty cross-owner
  tails and within-list reversals. Neighborhood::TwoOpt selects this smaller list
  set; both backends and joint mixed search support it. Basic variables retain
  changes/swaps. Empty owners cannot receive values through this neighborhood.
- Scoped incremental basic/list/mixed score networks and atomic scored-move transactions.
  List scopes cover entity lists and value identities/shadows, with conservative
  neighbor invalidation and deferred cache updates.
- Lazy change/swap neighborhoods and bounded deterministic local search,
  strict-improvement, hill-climbing, late acceptance, variable tabu or seeded simulated annealing; explicit
  evaluation/step limits and optional stagnation, score-target and feasibility stops;
  first/best accepted candidate selection and original or
  seeded shuffled-variable or bounded candidate-window ordering. Optional
  max_move_evaluations independently caps candidates in both construction and
  local search, excluding initialization and diagnostics. CountLimit also exposes
  exact thresholds, remaining counts and capped progress; five upstream count-limit
  methods have full adaptations. See the latest increment below.
  Read `docs/incremental-search-contract.md` before modifying this layer.
- Optional max_time bounds each local-search phase with a monotonic clock,
  checking between candidates and retaining the best evaluated prefix. It is
  cooperative, includes initial scoring and can overshoot in active callbacks.
  Eight original controlled-clock regressions cover all six model/backend paths.
- Optional max_unimproved_time excludes initialization and resets only on strict
  retained-best improvements. Expiry latches before resets; all six search paths
  share this behavior, checked by six additional controlled-clock regressions.
  This adds no upstream method credit or Java timed-trace claim.
- Optional borrowed CancellationToken supports cooperative cross-thread cancellation
  in all six local-search paths, retaining the best scored prefix. Tokens are
  one-way and clone-shared; initialization, callbacks and diagnostics are not
  preempted. All six construction paths and three configurable exact-search entry
  points also accept the token, retaining partial progress and distinguishing
  cancellation from full exhaustion. Config types carry a token lifetime; existing
  exact budget-only APIs retain their signatures. No lifecycle method credit.
- ScoreTargetProgress and FeasibilityProgress offer validated weighted estimates
  and separate exact stop predicates. Score now requires HARD_LEVEL_COUNT metadata.
  Native full-width differences are widened before conversion to f64. Automatic
  step-based cooling and local-search stopping behavior remain unchanged.
- 426 original integration regression methods, forty-nine original unit tests, and forty public API documentation tests.
- Native solve/solve_list/solve_mixed and stream counterparts compose construction
  and ordered local-search phases under shared score/candidate budgets, cancellation
  and score/feasibility goals. Results retain complete best or partial initialization
  and per-phase work. Shared elapsed time now spans construction, improvement and
  stream graph preparation. Construction also has a phase-local max_time control.
  Read `docs/solver-contract.md`; stagnation remains phase-local, and stream graphs
  are rebuilt per phase.
- Greedy construction with four early-pick policies, explicit partial results and
  bounded scoring. `local_search_from` accepts a complete constructed assignment.
- `graph_coloring` constructs and improves 100 variables using 400 scoped constraints;
  702 combined score evaluations, 5,700 callbacks, verified `0hard/0soft`.
- Public-API Java oracle: 9,959 generated comparisons (30 expected malformed-input rejections) pass with seed 20261001.
- 128 original complete-planning cases agree across Java, Rust and independent
  Python enumeration: 64 homogeneous required and 64 per-variable range cases,
  including mixed optionality, empty optional ranges and pinned nulls. Movable
  initial values reset before both solves.
- 64 original mixed depot/route solver-quality cases validate all solutions against
  independent Python optima (32,116 joint assignments). Optima: Rust exact 64,
  Community BRUTE_FORCE 39, native scoped/stream local 63 each. Different
  strategies/starts; quality gaps are explicit, with no timing/trace/coverage claim.
- 219 original deterministic basic-search termination cases agree with Community
  and independent Python search; native scoped/stream counters and reasons also
  match 398 reference steps. Includes delayed improvements after fallback and
  recovery. Java internal counters/traces, list/mixed search, and timing are not
  claimed. Run differential_solver.py; see the incremental-search contract.
- Required-list comparisons cover 192 original cases: 64 construction, 64
  relocation hill climbing and 64 two-opt. The latter use first-accepted search,
  20 steps and a 10,000 score-evaluation budget; Java sampling differs from native seeded candidate-window
  ordering (seed 20261006, window 1024). All 64 are valid, 61 have equal scores,
  and optima counts are Rust 49 / Java 46. Six timing workloads show quality beside each time ratio.
- Separate score microbenchmarks and two small matched exact-search workloads.
  The latter compare BRUTE_FORCE/ORIGINAL_ORDER Java with Rust finite enumeration.
  Matched full-scoring heuristic comparisons now cover hill climbing and tabu,
  32/96 variables, 50-step budgets, scores and callback counts. 64 original small
  heuristic cases are independently checked. These older sections do not measure
  incremental scoring or establish large-instance performance.
- Persistent ordered joins support <, <=, > and >= through bounded AVL key
  traversal, with explicit left/right orientation and transactional rollback.
  Equality retains exact bucket lookup. Partitioned joins additionally index one
  ordered relation inside equality groups, including tuple equality keys.
  Interval overlap joins now index both strict endpoint comparisons, optionally
  within equality partitions, with persistent subtree maximum-end pruning.
  Read docs/constraint-stream-contract.md.
- Stream scoring has 128 original Java/Rust cases (2,899 states), checking
  partial assignments, signed/cancelling groups, duplicate/self edges, commits
  and undo. Python checks both ordinary scores and native exact match IDs/sums.
  Community `SolutionManager.update()` is used; `analyze()` requires Enterprise
  in this release and must not be used. No Java match-analysis claim.
- Direct ordered/partitioned/overlap Joiners comparisons now pass 153 original cases /
  10,155 states for five relations and zero/one/two equality keys, plus plain and
  equality-partitioned strict overlaps (including degenerate/reversed endpoints). Community score
  recalculation agrees with Rust incremental edits and independent Python pairs.
  Native exact match IDs/payloads are checked separately; no method-credit or
  Java incremental notification/timing claim.
- Equality-only existence/anti-existence now uses per-key right counts and linear
  retained state, including complement. Original Community checks pass 54 cases /
  4,834 states across zero/one/two equality keys, with native left IDs/payloads
  independently checked. Generic per-pair filters still use cached match links.
- Matched incremental-stream hill/tabu benchmarks use 32/96 variables and
  50-step budgets, with 64 small independently checked search cases. Scores,
  validity and terminal weighting calls accompany times. Rust graph construction
  is timed; Java reuses its factory outside timing. See `docs/dashboard.md`.
- Local visual dashboard: `python3 scripts/dashboard_server.py`, port 8765.
  Server was restarted after the reporting-code update and may still be running.
  Reuse it; restart after changing imported fingerprint/reporting Python code.
- Coverage denominator: 4,855 statically discovered annotated Java methods, not
  expanded runtime JUnit invocations. 481 passed; 282 JVM integration exclusions.
  Everything else is pending. `docs/dashboard.md` explains caveats and commands.
- Pinned upstream remains unmodified at 8b815bc6b5f3eb81feedc94dea0885a93b267762.

Read `docs/score-contract.md` before changing scores. The Java oracle showed that
score arithmetic CLEARS structural values, constructors retain positive values,
and full strings suppress positive structural prefixes. Do not reintroduce older
initialization-score assumptions. No production implementation has been ported.

## Next substantive work

0. Expand persistent operators and applicable upstream cases; mixed stream scoring,
   construction and local search now exist. Mixed solver-quality evidence now
   compares Java BRUTE_FORCE and native exact/local results; matched Java mixed
   local-search configurations and applicable upstream cases remain pending.
   Incremental distinct now retains one lowest-input-ID representative per ordered
   value class and refreshes changed representative payloads. Twenty mapping/
   distinct tests across uni/bi/tri/quad streams have full assertion adaptations.
   Indexed existence/anti-existence with pair filters is now implemented; thirteen
   more upstream methods preserve all score/match assertions. Equality-only
   existence now uses right-key counts and linear retained state; complement uses
   it too. Generic filtered existence still retains matching pairs. Four binary
   stream methods additionally preserve zero/one-equality existence assertions.
   Read the stream contract first.
   Public-API Community stream comparisons and matched timing now exist.
   Warmed stream samples show Rust slower than Java, with equal final scores.
   Use the latest work/stream-performance.json and profile these workloads before
   choosing optimizations; ratios vary on the shared machine.
   Checkpoints and saved candidates now share persistent AVL map roots, avoiding
   whole-node row cloning. Shared AVL update paths are copied while uniquely owned
   nodes are now reused after comparison preflight. Stream construction/local search already share existing
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
   Extend deterministic termination evidence beyond the 219 basic-search cases
   to list/mixed models, decimal gradients and upstream unimproved-time lifecycle adaptations.
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

Mixed Java move/score increment: MixedStreamOracle.java uses pinned public
MoveTester/Moves.compose for basic change and list assign/change/swap/span-unassign.
Initial built-in shadows are enriched once, and subsequent observations use the
moves' owner/index/previous/next updates. SolutionManager.UPDATE_SCORE_ONLY freshly
calculates each score. Native mixed_stream_oracle uses mixed graph transactions,
checks cached basic/entity rows and structural missing-basic counts, and emits
cached value views. Committed observations are read after commit.

scripts/mixed_stream_checks.py generates 128 original required-basic/optional-list
cases (seed 20261108), with 2,378 observed states and 3,611 primitive operations.
Independent Python derives ordinary scores, completeness, ordered assignments and
all four shadow views. Cases cover non-dense identities, empty registries/lists,
partial basics, pinned basics, signed coefficients, repeated writes and joint
composites. Four new Python regressions check a hand-calculated lifecycle,
corruption of every output field, reproducible generation/pins and invalid bounds/
domains/ownership. This is not mixed solver-search, Java match-analysis, custom
shadow, list pin/range, structural-score equivalence or mixed timing evidence.

The stream differential command now runs these checks and saves mixed_stream_checks
inside its report, with full cases, observed states and both runtime outputs.
Parent metadata records source digest, reference/artifact hashes and environment.
The existing dashboard comparison section shows the mixed supplement separately;
its counts do not change upstream-method coverage or existing timing workloads.
No upstream production implementation or tests/fixtures were copied in this slice.

Full debug/release all-target checks pass: 593 Rust tests (308 adapted + 276
original integration + nine unit), 21 doctests and 52 Python regressions. Coverage
remains 308/4,855 overall and 160/1,045 streams. Logs: work/mixed-java-{check,
release}.log. Public mixed solver-search comparisons and more upstream adaptations
are still needed; the full goal remains active.

Final mixed comparison verification also rejects stale index/previous/next shadows
on unassigned values in both runtime adapters. All 128 cases still pass. All nine
comparison/timing sections were refreshed after the final source edit; report,
source and live fingerprints match. The existing dashboard server was reused.
Browser checks pass at 1280px and 390px with no console errors, stale messages or
horizontal overflow. Mixed counts (128 cases, 2,378 states, 3,611 operations),
6.34% upstream coverage and the existing list32 quality difference are visible.
The verification browser was closed.

Current basic-stream medians in ms (Rust / Java): hill32 59.183 / 18.643,
hill96 264.987 / 34.088, tabu32 67.056 / 26.512, tabu96 249.434 / 37.706.
Scores agree; both 96-variable workloads remain infeasible at the 50-step cap.
List32 construction remains Rust 0hard/-1080soft versus Java 0hard/-1062soft.
These timings do not benchmark mixed models. Logs: work/mixed-java-stream-evidence.log,
mixed-java-stream-performance.log, mixed-java-list-refresh.log,
mixed-java-core-refresh.log and mixed-java-heuristic-performance.log.


Persistent flat-map increment: FlatMapNode and StreamGraphBuilder.flat_map expand
one input to a finite ordered sequence, preserving duplicate payloads under
(input_id, position) identities. Replacements update surviving positions and
retract the old tail. Cached persistent lengths make retraction callback-free;
prepared output/roots preserve standalone state after ordinary panics. Generic
closures retain tuple prefixes or append an expanded element. Distinct remains
explicit. There is no Java representative object-identity claim.

Fifteen adapted upstream methods in flat_map_stream_cases preserve every initial/
removal score and exact match multiset: uni flatten plus three flattenLast methods,
three bi methods, and four each for tri/quad. All provenance and fixture records
are added. Uni flattenLastWithDuplicates remains pending because it observes
which equal Pair instance a downstream identity filter receives. Bi
flattenLastWithDuplicates also remains pending: its asserted duplicate distribution
needs further examination; do not replace its assertions with the native result.

Six original regressions cover empty/nullable/duplicate outputs, reorder/shrink,
callback-free removal, mapping/iterator panics, graph errors/panics and failed
rebuild rollback, foreign ports, nested expansion/distinct, construction/search,
and 2,000 seeded graph edits (20261109) against independent full multisets.
Full debug/release all-target checks pass: 614 Rust tests (323 adapted + 282 original integration + nine
unit), 22 doctests, 52 Python regressions. The full goal remains unfinished.

Logs: work/flat-map-check.log and work/flat-map-release.log. Existing Java
comparisons and timing workloads have all been refreshed for this source; they do
not exercise the new flat-map operator and grant no further coverage credit.

All nine sections are current and report/source/live fingerprints match. The
existing dashboard server was reused. Browser checks pass at 1280px and 390px:
6.65% overall (323/4,855), streams 16.75% (175/1,045), no stale messages, console
errors or horizontal overflow. Search shows 14 flattenLast methods passed while
the uni/bi duplicate methods remain pending; uni flatten is the fifteenth new case.
Wait for loaded test items before checking filtered results. Browser session closed.

Current stream medians in ms (Rust / Java): hill32 71.260 / 22.441,
hill96 283.125 / 44.214, tabu32 62.536 / 24.318, tabu96 269.880 / 50.183.
Both 96-variable workloads remain infeasible under the 50-step cap. List32
construction remains Rust 0hard/-1080soft versus Java 0hard/-1062soft. Logs:
work/flat-map-stream-evidence.log, flat-map-stream-performance.log,
flat-map-list-refresh.log, flat-map-core-refresh.log and
flat-map-heuristic-performance.log. Upstream pin/checkout remain unmodified.


Persistent concat increment: StreamGraphBuilder.concat accepts different left/right
ID types with one common payload type and returns ConcatId::Left/Right identities.
The routing node is stateless; downstream caches participate in ordinary graph
transactions/checkpoints and saved candidate installation. Same-port concatenation
preserves two occurrences. Ordering is branch then ID, not event arrival. Explicit
map/Option fields pad tuple arities; no implicit Java null/padding API is claimed.

Forty-four adapted methods in concat_stream_cases preserve every score and exact
match assertion across all uni/bi/tri/quad operand combinations, duplicates,
distinct and group-by. Expected multisets are literal source assertions; source
fixture references and variable-update order are retained. Group cases preserve
counts, weighted contributions and exact concatenated value-code strings, including
group disappearance/reappearance. Native typed filters/joins, branch IDs and
Option padding replace descriptors/object references. Provenance records every
method and fixture. No upstream production implementation was used.

Six original regressions cover heterogeneous IDs, nullable values, nested/self
concatenation, foreign-port rejection, group summaries, graph errors/panics after
one branch propagates, rebuild recovery and construction/local search. 3,000
seeded shared-source updates (20261110) compare exact tagged multisets and group
results against independent full recomputation through commit/undo.
Full debug/release all-target checks pass: 664 Rust tests (367 adapted + 288 original integration + nine
unit), 23 doctests, 52 Python regressions. Full rewrite remains unfinished.

Logs: work/concat-check.log and work/concat-release.log. Forty-four source methods
now earn credit; original randomized cases remain separate. Stream complement,
more operators, custom shadows, arbitrary-precision score families and mixed Java
solver-search comparisons remain substantive follow-ups. Read the current stream
contract before composing further persistent operators.

All nine comparison/timing sections were refreshed after the final source edit.
Report/source/live fingerprints match. The existing dashboard server was reused.
Browser checks pass at 1280px and 390px: overall 7.56% (367/4,855), constraint
streams 20.96% (219/1,045), all 44 concat methods shown as passed after search,
result filtering and Show more. No stale messages, console errors or horizontal
overflow. The verification browser was closed. Upstream pin/checkout are unchanged.

Current stream medians in ms (Rust / Java): hill32 61.835 / 19.251,
hill96 276.557 / 47.431, tabu32 67.314 / 18.531, tabu96 267.876 / 41.927.
Scores agree; both 96-variable workloads remain infeasible under the 50-step cap.
List32 construction retains Rust 0hard/-1080soft versus Java 0hard/-1062soft.
Existing timings do not benchmark concat. Logs: work/concat-stream-evidence.log,
concat-stream-performance.log, concat-list-refresh.log, concat-core-refresh.log
and concat-heuristic-performance.log. Full goal remains active and unfinished.


Persistent complement increment: StreamGraphBuilder.complement retains all input
occurrences and pads each universe occurrence whose supplied key has no input
match. It composes indexed NotExists, map and concat; input IDs are ConcatId::Left,
padded universe IDs are Right. Both ports validate before topology is added.
Duplicate input keys hide padding until their last occurrence disappears;
duplicate universe keys remain independent missing occurrences. Input keys outside
the universe remain in output. Fact discovery and assigned filtering are explicit.
Shared-source delivery may transiently invoke padding even for a finally covered
fact; padding must be valid for every universe payload and deterministic.
Existence match-link caching can use quadratic memory in dense equal-key buckets;
no specialized linear-memory counts implementation or Java event-trace claim.

Four complement source methods preserve both score/match stages across four
arities, numeric index extraction, entity identity and exact MAX_VALUE/-1/0
padding. Six related expandTo methods preserve all initial/removal assertions,
retained entity prefixes, group/value identity, sums and exact code strings using
native map closures. Their random long properties are fixed to 17,311,90001 within
the source range, with literal expected sums. Every test/fixture is in provenance.
No upstream production implementation was used.

Six original regressions cover nullable keys, duplicate input/universe keys,
last-match removal, foreign ports, changing keys, independent/shared sources,
transient padding, key/padding/score panics, structural errors, rebuild recovery,
construction and local search. 3,000 seeded transactions (20261111), each with
one to four writes, compare exact tagged complement multisets to full independent
recomputation at every completed write, with both commit and undo.
Full debug/release all-target checks pass: 680 Rust tests (377 adapted + 294 original integration + nine
unit), 24 doctests, 52 Python regressions. Full rewrite remains unfinished.

The new 377/4,855 total exposed double rounding in the dashboard: the report's
three-decimal 7.765 passed through JavaScript toFixed(2) displayed 7.76, while the
actual count ratio rounds to 7.77. Coverage labels/ARIA now format directly from
passed/total, including zero totals, while preserving report schema and segments.

All nine comparison/timing sections are current after refresh. Report/source/live
fingerprints match. Existing dashboard server reused. Browser checks pass at
1280px and 390px: 7.77% overall (377/4,855), 21.91% constraint streams (229/1,045),
four complement and six expandTo methods found as passed. Every subsystem label
and ARIA percentage agrees with its count ratio; zero-total formatting is 0.00%.
No stale messages, console errors or horizontal overflow. Browser session closed.

Current stream medians in ms (Rust / Java): hill32 71.301 / 26.065,
hill96 281.240 / 52.472, tabu32 73.600 / 22.806, tabu96 256.684 / 38.504.
Scores agree; both 96-variable workloads remain infeasible under the 50-step cap.
List32 construction retains Rust 0hard/-1080soft versus Java 0hard/-1062soft.
Existing timings do not benchmark complement. Logs: work/complement-check.log,
complement-release.log, complement-stream-evidence.log,
complement-stream-performance.log, complement-list-refresh.log,
complement-core-refresh.log and complement-heuristic-performance.log.
Upstream pin/checkout remain unmodified. The full goal remains unfinished.


Ordered comparison join increment: JoinComparison adds Equal/LessThan/
LessThanOrEqual/GreaterThan/GreaterThanOrEqual, used by JoinNode::comparing and
StreamGraphBuilder::join_compare. Existing equality constructors retain exact
bucket lookup. Inequality traversal seeks bounds in the persistent AVL key index,
visiting matching buckets only; right-side updates reverse the relation. Pair
identity, cached keys, same-key replacement and graph checkpoint rollback are
preserved. SnapshotMap's borrowed bounded iterator is crate-private; included,
excluded, unbounded, empty and reversed bounds are checked on old/new snapshots.
Keys use native Ord; tuple order is lexicographic. No Java null ordering, compound
comparison index, interval indexing or event-order parity is claimed. Inspection
is left ID/right key/right ID; equality inspection retains its previous order.

27 adapted methods preserve every assertion in five JoinerTypeTest methods,
20 JoinersTest numeric projection methods across four arities, and two
ComparisonIndexerTest orientation methods with exact duplicate match membership
and sizes. BigInt sums and scale-zero Decimal projections preserve small source
values. Checked i64 conversion does not implement general Java longValue behavior.
Provenance includes source helpers. Internal routing/treeification/random-iterator
methods remain pending. No upstream production implementation was used.

Eight original regressions check extreme/duplicate keys, payload replacement,
cached retraction, 6,000 bidirectional edits (seed 20261112), 3,000 shared-source
transactions (seed 20261113) with one to three writes, commit/undo, full rebuild,
score/key panics and invalid structural contributions, foreign ports, grouping,
construction and local search. Sparse lookups in either orientation use fewer
than 512 key comparisons over 4,096 opposite keys. These checks earn no upstream
credit. Existing Java/Rust timing workloads exercise equality joins and do not
benchmark the new comparison operators.

Dashboard classification now places impl/bavet engine tests in Constraint streams,
where they belong, instead of Core utilities & configuration. This moves 256
source methods between subsystem denominators; the overall total stays 4,855.
Coverage is 404/4,855=8.32%, constraint streams 256/1,301=19.68%, with 4,169 pending,
282 excluded and zero failed/unverified methods. A Python regression protects
subsystem classification including unchanged score/JVM/util categories.
Full debug gate passes 716 Rust tests (404 adapted + 302 original integration + 10
unit), 25 doctests and 53 Python regressions. Full rewrite remains unfinished.

Release all-target tests also pass (716). All nine Java/Rust evidence sections are
current after refresh. Report/source/live fingerprints match. Dashboard server
restarted after the reporting-code update (exec session 17562); reuse it if alive.
Browser verified at 1280px and 390px, with 20 JoinersTest, five JoinerTypeTest and
two ComparisonIndexerTest methods listed as passed. Overall 8.32% and stream 19.68%
labels agree with exact count ratios. No visible stale/error messages, console
errors or horizontal overflow. Mobile screenshot: work/ordered-join-mobile.png.
Browser session closed.

Current stream medians in ms (Rust / Java): hill32 58.077 / 15.899,
hill96 233.408 / 26.370, tabu32 60.435 / 20.742, tabu96 223.253 / 31.676.
Scores agree; both 96-variable workloads remain infeasible under the 50-step cap.
List32 construction retains Rust 0hard/-1080soft versus Java 0hard/-1062soft.
Existing timings do not benchmark ordered comparisons. Logs: work/ordered-join-check.log,
ordered-join-release.log, ordered-join-stream-evidence.log,
ordered-join-stream-performance.log, ordered-join-list-refresh.log,
ordered-join-core-refresh.log and ordered-join-heuristic-performance.log.
Upstream pin/checkout remain unmodified. The full goal remains unfinished.


Partitioned join increment: PartitionedJoinNode and graph join_partitioned accept
projections returning (equality key, ordered key). Equality keys may be tuples;
the ordered key supports equality and all four ordered comparisons, with explicit
left/right orientation.
Each side holds a persistent equality map of existing JoinSide ordered indexes
and an ID-to-partition lookup. Updates visit only old/new partitions and matching
ranges. Empty side partitions are removed. Candidate roots and output events are
prepared before installation, so ordinary projection/clone/comparison failures
preserve this standalone node, even after a candidate's old partition was removed.
Graph checkpoints share roots. Existing equality-only/ordered-only indexes are
unchanged. Inspection orders equality key, left ID, right ordered key and right ID.
No multi-inequality index or Java event-order parity claim.

Ten EqualsAndComparisonIndexerTest methods preserve every empty, insertion,
membership, cardinality, ordinary iterator and boundary assertion. Stable literal
names and tuple payloads replace Java references/index handles; peekable/Option
preserves the tested iterator presence/exhaustion. Repeated-removal exception and
random neighborhood iterator methods remain pending. Provenance records cases and
helpers; upstream production code was not used.

Seven original regressions cover 6,000 bidirectional edits (seed 20261114), 3,000
shared-source transactions (seed 20261115) with commit/undo/full rebuild, sparse
queries over 4,096 ordered keys and 4,096 unrelated partitions in both orientations,
every key clone/ordering failure in a partition transfer, graph callbacks/errors,
foreign ports, grouping with tuple equality keys, construction and local search.
A private churn test checks partition/membership cleanup after 2,000 transfers.
Native regressions receive no upstream method credit.

Full debug gate passes: 734 Rust tests (414 adapted + 309 original integration +
11 unit), 26 doctests and 53 Python regressions. Coverage is 414/4,855 = 8.53%;
constraint streams 266/1,301 = 20.45%. There are 4,159 pending, 282 excluded and
zero failed/unverified methods. The full rewrite remains unfinished.

Release all-target checks also pass (734 tests). All nine comparison/timing
sections are current after refresh; report/source/live fingerprints agree.
The existing dashboard server was reused. Browser checks at 1280px and 390px
confirm 8.53% overall and 20.45% constraint streams, with ten passed and ten
pending EqualsAndComparisonIndexerTest methods; the passed filter shows exactly
ten. No visible stale messages, console errors or horizontal overflow. Browser
session closed.

Current stream medians in ms (Rust / Java): hill32 64.872 / 20.321,
hill96 241.347 / 34.591, tabu32 58.594 / 28.497, tabu96 259.714 / 45.167.
Scores agree; both 96-variable workloads remain infeasible under the 50-step cap.
List32 construction retains Rust 0hard/-1080soft versus Java 0hard/-1062soft.
Existing timings do not exercise partitioned joins. Logs: work/partitioned-check.log,
partitioned-release.log, partitioned-stream-evidence.log,
partitioned-stream-performance.log, partitioned-list-refresh.log,
partitioned-core-refresh.log and partitioned-heuristic-performance.log.
Upstream pin/checkout remain unmodified. The full goal remains unfinished.


Ordered-join direct evidence increment: OrderedJoinOracle.java uses only public
Community Joiners/ConstraintFactory/SolutionManager APIs. One assigned dummy
planning entity supports fact-only left/right relations. Fifteen modes cover five
ordered/equality relations with zero, one or two equality keys. Java rebuilds each
score observation; no Enterprise match analysis or incremental notification claim.
The Rust ordered_join_oracle example uses persistent graph transactions, exposes
exact pair IDs and both payloads, and checks full rebuild consistency after each
committed/undone operation. Python independently enumerates pairs and validates
counts, signed products and every native match field. No solver implementation or
upstream-method credit is added by this original evidence.

All 135 cases / 8,976 states pass, seed 20261116. Extreme/duplicate ordered keys,
signed/zero weights, payload-only replacements, equality/range key changes,
independent left/right insertion/removal, absent/last removals and trials/undo are
covered; every case finishes empty. Full inputs and Java/Rust outputs live under
ordered_join_checks in work/stream-differential.json and the downloadable report.
The dashboard describes the evidence separately from method coverage and timings.
Four Python regressions check literal truth tables and scores, snapshot undo,
corruption rejection (including stale zero-weight payloads), and generator coverage.
Full debug gate passes 734 Rust tests, 26 doctests and 57 Python regressions.
Coverage remains 414/4,855 (8.53%), streams 266/1,301 (20.45%); no new source methods.

Release all-target tests also pass (734). All nine evidence sections are current;
source/report/live fingerprints match. The existing dashboard server was reused.
Browser checks at 1280px and 390px confirm the 135/135, 8,976-state ordered-join
summary, unchanged 8.53% coverage, and no visible stale messages, console errors
or horizontal overflow. Browser session closed.

Current stream medians in ms (Rust / Java): hill32 62.108 / 29.937,
hill96 240.454 / 38.812, tabu32 64.125 / 20.833, tabu96 255.619 / 32.702.
Scores agree; both 96-variable workloads remain infeasible under the 50-step cap.
List32 construction retains Rust 0hard/-1080soft versus Java 0hard/-1062soft.
These existing timing workloads do not exercise ordered/partitioned comparisons.
Logs: work/ordered-evidence-check.log, ordered-evidence-release.log,
ordered-evidence-stream.log, ordered-evidence-stream-performance.log,
ordered-evidence-list-refresh.log, ordered-evidence-core-refresh.log and
ordered-evidence-heuristic-performance.log. Upstream pin/checkout remain unmodified.
The full goal remains unfinished.

## Interval-overlap index increment

Added OverlapJoinNode plus graph join_overlapping and
join_overlapping_partitioned. Projections capture equality/start/end keys. Each
partition has a persistent AVL interval index ordered by (start, ID), with subtree
maximum-end metadata for pruning. Candidate roots and output clones are prepared
before installation; graph checkpoints share roots. Same-key replacements update
pairs once; endpoint/partition changes retract old pairs before new puts.
Touching endpoints do not match. Endpoints are not normalized: zero-length and
reversed intervals follow the raw strict cross-endpoint predicate, confirmed
against Community public Joiners. No general multi-inequality index is claimed.

Five JoinersTest overlap methods preserve all 65 literal endpoint assertions,
BigInt/scale-zero Decimal projections, and ignored higher-arity strings. Fixture
and method provenance are recorded. Six original integration regressions cover
bidirectional randomized edits, shared-source transactions, exact replacement
events, foreign ports/downstream errors/projection panics, exhaustive key clone/
comparison fault injection, and counted sparse pruning. An original private unit
test checks AVL ordering, balance, size, maximum-end caches, snapshots and queries
against an independent map across 4,000 updates/removals.

The original Java/Rust/Python comparison matrix adds 18 overlap cases / 1,179
states, bringing ordered/partitioned/overlap checks to 153 cases / 10,155 states.
The new modes reuse the row protocol with first=start, key=end, second=equality;
other modes retain their original meaning and generated inputs. All comparisons
pass, including touching, zero-length, reversed and extreme boundaries. These
checks earn no additional method credit and compare Java score recalculation with
native incremental edits, not Java incremental notifications or overlap timing.

Development gate passed: 746 Rust tests (419 adapted, 315 original integration,
12 original unit), 27 doctests and 58 Python regressions. Release tests pass.
Coverage: 419 / 4,855 = 8.63%; Constraint streams 271 / 1,301 = 20.83%.
Pending 4,154; excluded 282; failed/unverified zero. Upstream remains unmodified.

Latest stream medians ms Rust/Java: hill32 78.306/22.688,
hill96 252.859/34.892, tabu32 61.493/16.906, tabu96 246.491/36.858.
The 96-variable scores remain [-44,-484], infeasible at the 50-step cap.
List32 construction remains Rust [0,-1080] versus Java [0,-1062]. Existing timing
workloads do not benchmark the new interval operator. Logs:
work/overlap-{check,release,stream-evidence,stream-performance,list-refresh,
core-refresh,heuristic-performance}.log.

All nine evidence sections were refreshed and verified current against source and
verification digests. The running dashboard's freshness digest matches. Browser
checks at 1280px and 390px confirmed coverage, the 153/153 and 10,155-state summary,
and all five overlap methods under the Passed filter, with no page errors,
staleness messages or horizontal overflow. Existing server was reused; browser
verification session closed. The full solver rewrite remains active and incomplete.

## Equality-only existence counts increment

Added ExistsEqualNode and graph existence_equal for Exists/NotExists with constant
or tuple equality keys. Persistent roots retain left rows and memberships, right
ID-to-key mappings and one count per right key. There are no pair links, and right
payloads are discarded after projection. Right changes visit left rows only when
a key crosses empty/nonempty; same-key replacements and duplicate insert/removal
emit no events. Left payload changes still replace visible rows. Retractions use
captured keys; two-key transfers emit deterministic left-ID order. All candidate
roots and output preparation precede installation. Graph checkpoints share roots.
Complement now composes this linear-state specialization with map and concat;
filtered ExistsNode remains unchanged and still retains matching pairs.

Four AbstractBiConstraintStreamTest methods fully preserve initial/removal scores
and match sets for no-joiner and one-equality Exists/NotExists. Native ID-ordered
self-joins generate the unique pairs; explicit facts retain source identities,
codes and assigned value references. Method/fixture provenance is recorded.
Nine original integration regressions cover 6,000 independent edits, 2,000 shared
source transactions, duplicate counts, key transfers, payload replacement,
callback/clone/ordering faults, terminal errors, optional/tuple keys and foreign
ports. A dense 16,384-by-16,384 case avoids materializing 268 million pairs and
bounds duplicate-update key comparisons. Non-Clone right payloads work standalone.
Existing complement tests run against the new composition.

New original Java/Rust/Python equality-existence checks pass 54 cases / 4,834
states, seed 20261118. Modes alternate Exists/NotExists with zero, one and two
equality keys. Each Rust transaction exposes exact retained left IDs/payloads;
independent Python nested enumeration verifies those and both runtime scores.
Java uses public ifExists/ifNotExists and SolutionManager.update recalculation;
no Java match-analysis, incremental notification, timing or method-credit claim.
Inputs and both outputs live under equality_existence_checks in the downloadable
stream report. Four Python regressions guard literal outcomes and response checks.

Development/release verification: 759 Rust tests (423 adapted, 324 original
integration, 12 original unit), 28 doctests and 62 Python checks pass. Coverage
423 / 4,855 = 8.71%; Constraint streams 275 / 1,301 = 21.14%. Pending 4,150,
excluded 282, failed/unverified zero. The Community pin remains unmodified.
Stream medians ms Rust/Java: hill32 53.977/16.167, hill96 260.152/44.017,
tabu32 63.002/24.613, tabu96 283.591/43.212. Both 96-variable scores remain
[-44,-484], infeasible at the 50-step cap. These benchmarks do not exercise pure
equality existence or complement, and do not measure this specialization's speed.
Logs: work/equality-existence-{check,release,stream-evidence,stream-performance,
list-refresh,core-refresh,heuristic-performance}.log. Full goal remains unfinished.

All nine evidence sections were refreshed and verified current against source and
verification fingerprints; the live dashboard freshness digest matches. Browser
checks at 1280px and 390px confirmed 8.71% overall, 21.14% streams, all four new
Passed methods and the 54/54, 4,834-state existence summary. No page errors,
staleness messages or horizontal overflow. Existing server reused; browser closed.
List32 construction remains Rust [0,-1080] versus Java [0,-1062].

## Snapshot copy-on-write profiling increment

Profiled baseline 6fd8b99 using locally extracted Valgrind 3.22.0. Callgrind showed
substantial allocation work. SnapshotMap previously rebuilt every AVL search path;
updates now reuse uniquely owned nodes and copy only shared paths with Arc::make_mut.
A bounded stack path resolves every key comparison before mutation, preserving
exact root/content state on comparison panics for both shared and unique trees.
Rotations and metadata refresh need no user comparisons or key/value clones.
Existing-key replacement retains the old key and avoids allocating a duplicate.
No unsafe code, dependency or public API change. Snapshot Send/Sync is retained.

One original ownership regression checks node reuse through inserts, replacements,
removals and single/double rotations. Existing comparison-panic checks now cover
unique trees and every retained entry. Existing 20,000-update reference/AVL,
shared payload, bounded-range, snapshot/rollback and saved-candidate tests pass.
Development gate: 760 Rust tests (423 adapted, 324 original integration, 13 original
unit), 28 doctests and 62 Python checks. Coverage stays 423 / 4,855 = 8.71%; streams
275 / 1,301 = 21.14%, pending 4,150, excluded 282, failed/unverified zero.

Final 32-variable hill/50-step/one-solve diagnostics: allocated blocks
1,027,292 -> 724,323 (-29.5%), total bytes 71,705,255 -> 55,254,648 (-22.9%);
peak live bytes 172,661 -> 172,446 (essentially unchanged). Callgrind instruction
references 370,789,666 -> 341,311,713. Diagnostic release builds include debug
information. Counts cover the whole process; they are not elapsed-time measures.

Separate uninstrumented A/B uses three fresh processes per binary/workload,
three warmups and five samples each, alternating order. All final assignments, scores
and terminal callback counts are identical before/after, with independent Python score
validation. Final before/after medians ms: hill32 71.201/63.927,
hill96 280.967/249.524, tabu32 67.638/68.745, tabu96 251.331/217.973.
Three medians improved; tabu32 was about 1.6% slower. Small wall-clock differences
are not conclusive on the shared machine. Do not claim a uniform/general speedup.
The contract includes the complete measurement scope and table.

Raw inputs, baseline binaries, compiler/environment and SHA256 manifest,
Callgrind/DHAT outputs, comparison runner and samples live in work/profiling/.
The first exploratory COW profiles are separately retained; final after.* files
and compare.json correspond to the final code. No system packages were changed.
The upstream checkout and pin remain unmodified. Full goal remains unfinished.

Release all-target tests also pass. Refreshed stream medians ms Rust/Java:
hill32 58.725/23.829, hill96 226.604/42.539, tabu32 58.735/26.824,
tabu96 236.649/42.363. Both 96-variable scores remain [-44,-484], infeasible at
50 steps. List32 construction remains Rust [0,-1080] versus Java [0,-1062].
Native A/B medians above are separate measurements; do not infer change magnitude
from successive Java/Rust refreshes on the shared machine. Verification logs:
work/cow-snapshots-{check,release,stream-evidence,stream-performance,list-refresh,
core-refresh,heuristic-performance}.log.

All nine comparison/performance sections are current and source/verification
fingerprints match the live server. Browser checks at 1280px and 390px verified
unchanged honest coverage, refreshed stream medians/quality/callback counts,
no stale warnings, no page errors and no horizontal overflow. Existing server
reused; browser session closed. This remains a performance increment, not new
upstream test-method coverage or completion of the full solver rewrite.

## Deterministic search termination increment

LocalSearchConfig now supports optional max_unimproved_steps, best_score_limit
and stop_when_feasible. All default to disabled. Basic, list and mixed search use
the same boundary criteria with both scoped and stream scoring. StepProgress
tracks completed steps and the last strict retained-best improvement; result
unimproved_steps includes ties, worsening/recovery and selected rejected fallback
steps. A neighborhood with no selection does not complete a step. StepCountLimit
provides exact integer threshold decisions and capped f64 progress, including
zero-budget behavior. The contract and public enum document precedence, including
evaluation exhaustion while selecting a move versus reaching a step boundary.

Two complete source methods (StepCountTerminationTest.phaseTermination and
UnimprovedStepCountTerminationTest.phaseTermination) preserve all twelve threshold
and twelve gradient assertions, with provenance and attribution. Mock Java step
indices are represented by native completed counts. Eleven original regressions
cover all six model/backend combinations, strict-improvement resets, ties, rejected
fallbacks, initial criteria, overlapping limits, unreachable targets, neighborhood
exhaustion and multi-hard feasibility. Score-target/feasibility gradient methods,
time-based termination and direct Java search comparisons of these optional
criteria remain pending. Existing benchmark configurations leave them disabled.

Development and release all-target suites: 773 Rust tests (425 adapted, 335
original integration, 13 original unit), 29 doctests and 62 Python checks pass.
Coverage is 425 / 4,855 = 8.75%; Solver lifecycle 2 / 379 = 0.53%; streams remain
275 / 1,301 = 21.14%. Pending 4,148, excluded 282, failed/unverified zero.
Logs use work/termination-{check,release,stream-evidence,stream-performance,
list-refresh,core-refresh,heuristic-performance}.log. The full goal is unfinished.

Refreshed stream medians ms Rust/Java: hill32 65.067/21.534,
hill96 264.286/58.380, tabu32 66.146/29.414, tabu96 295.032/44.287.
Both 96-variable scores remain [-44,-484], infeasible at the 50-step cap.
List32 construction remains Rust [0,-1080] versus Java [0,-1062]. These refreshed
wall times are not a controlled before/after comparison on the shared machine;
no performance improvement is claimed for this termination increment.

All nine comparison/performance report sections are current. Live server
verification fingerprint matches. Desktop 1280px and mobile 390px browser checks
show 8.75% overall, Solver lifecycle 0.53%, both adapted termination methods under
the Passed filter, no stale alert, page errors or horizontal overflow. Existing
server reused; browser session closed. Upstream remains unmodified.

## Direct Community termination comparison increment

New original TerminationOracle.java configures only public Community APIs. The
native termination_oracle example runs both scoped and persistent stream scoring;
Python independently enumerates each complete change neighborhood. The regular
scripts/differential_solver.py command now additionally records termination_checks:
219 / 219 cases pass, seed 20261119, 398 reference completed steps. Java and Rust
return the same exact final scores and assignments; native counters and stop
reasons also match Python. Java internal counters, reasons and traces are neither
exposed nor inferred. No adapted test methods or library behavior were added.

Cases include zero limits, initial targets, signed soft preferences, hard conflicts,
duplicate/self edges, pins, no moves, unreachable criteria and OR composition.
An original four-variable fixture discovered with seed 20261120 improves twice,
selects a rejected worse fallback, recovers, then discovers another global best. Fifteen
limit variations give different returned scores for short and long budgets,
providing observable evidence of when Java stops. Five Python regressions check
literal milestones and ensure validators reject wrong final solutions, counters,
backend identities and reasons. Raw inputs, Python traces and both runtime outputs
are saved in work/oracle/termination-cases.json.

The dashboard displays this comparison as a separate original-case count in
Beyond the adapted tests, inheriting source freshness and showing mismatches.
Coverage remains 425 / 4,855 = 8.75%; pending 4,148, excluded 282, failed/unverified
zero. Development/release all-target suites pass 773 Rust tests, with 29 doctests
and 67 Python regressions passing the development gate. Java adapters remain
outside the Rust library dependency graph; no upstream files changed. List/mixed
termination comparisons, stochastic acceptance, time-based limits and source
score-target/feasibility gradient methods remain pending. Full goal unfinished.

All nine comparison/performance sections were refreshed and are current. Latest
stream medians ms Rust/Java: hill32 66.993/24.970, hill96 253.206/39.663,
tabu32 57.274/19.654, tabu96 238.183/40.099. Both 96-variable scores remain
[-44,-484], infeasible at 50 steps. List32 construction still returns Rust
[0,-1080] versus Java [0,-1062]. No performance-change claim: library code and
benchmark configurations were unchanged in this comparison increment.
Logs: work/termination-comparison-{check,release,stream-evidence,stream-performance,
list-refresh,core-refresh,heuristic-performance}.log.

The existing server's live fingerprint matches the final report. Browser checks
at 1280px and 390px verify 219 / 219 termination cases, the explicit limitations,
unchanged 8.75% upstream coverage, no stale alert, no page errors and no horizontal
overflow. No server restart was needed; browser session closed. Upstream clean.

## Weighted score and feasibility progress increment

New ScoreTargetProgress and FeasibilityProgress hold fixed starting scores and
validated split weights. Target progress uses all ordinary levels; feasibility
uses only the hard prefix. Score now requires HARD_LEVEL_COUNT (0 for Simple,
1 for HardSoft and HardMediumSoft, H for Bendable<H,S>). Custom native score types
must supply it consistently. Exact is_reached decisions remain separate from
floating estimates, which may saturate before a target under rounding/zero weights.
Negative structural current scores report zero and never meet the criterion;
flawed reference scores and wrong/nonfinite/out-of-range weights are rejected.

Per-level integer differences widen to i128 before f64 conversion, avoiding
wrapping and loss of adjacent large values. This native full-width policy is not
a Java overflow-parity claim. Positive hard scores do not compensate for another
negative hard level. No-hard-level scores are feasible when structurally sound.
Local-search stopping and automatic annealing cooling remain unchanged. Explicit
annealing probability_at/accepts_at callers can consume the new estimates.

Nine fully adapted integer methods from BestScoreTerminationTest and
BestScoreFeasibleTerminationTest preserve every threshold and gradient assertion,
including repeated observations, weights, multi-level scores and floating tolerances.
Native fixed references replace mocked initialized solver/phase scopes and score
definitions. Provenance/notices updated. The two decimal gradient methods, Java
lifecycle behavior, automatic gradient-driven cooling and direct Java gradient
oracle comparisons remain pending. Seven original regressions cover validation,
full-width boundaries, structural flaws, hard/soft separation, estimate saturation,
reversed intervals and explicit annealing integration.

Development gate and release all-target suites pass: 789 Rust tests (434 adapted,
342 original integration, 13 original unit), 30 doctests and 67 Python checks.
Coverage 434 / 4,855 = 8.94%; Solver lifecycle 11 / 379 = 2.90%; streams remain
275 / 1,301 = 21.14%. Pending 4,139, excluded 282, failed/unverified zero. The full
solver rewrite remains unfinished. Logs use work/score-progress-{check,release,
stream-evidence,stream-performance,list-refresh,core-refresh,heuristic-performance}.log.

All nine comparison/performance sections were refreshed and are current, including
219 termination and 128 exact-search cases. Latest stream medians ms Rust/Java:
hill32 45.048/16.932, hill96 171.007/23.375, tabu32 49.159/17.210,
tabu96 171.208/26.516. Both 96-variable scores remain [-44,-484], infeasible
at 50 steps. List32 construction remains Rust [0,-1080] versus Java [0,-1062].
Shared-machine timing shifts do not demonstrate an improvement from this feature.

The live server fingerprint matches the final report. Desktop 1280px and mobile
390px browser checks show 8.94% overall, Solver lifecycle 2.90%, all eleven passed
lifecycle cases, no stale alert, page errors or horizontal overflow. Existing
server reused; browser session closed; upstream remains clean.

## Cooperative elapsed-time local-search increment

LocalSearchConfig adds optional max_time: Duration, disabled by default, and
LocalSearchTermination adds TimeLimit. Basic, list and mixed local search share
the same integration for scoped and stream scoring. An Instant clock starts at
phase entry; first-fit initialization, initial scoring, validation inside the
phase and diagnostic recomputation count. Preparation before the search is not
covered. Clock reads are skipped when disabled. This is cooperative: an active
callback, validation, move enumeration or diagnostic cannot be interrupted.

Timing is polled after existing boundary criteria, before each emitted candidate,
and after selection/diagnostics. Expiry truncates lazy iteration and preserves
any selected winner from the evaluated prefix, then returns the retained global
best. A timed-out rejected prefix never reports neighborhood exhaustion. A detected
evaluation cap takes precedence over the final time poll; a time poll withholding
a candidate can prevent selection from observing that cap. Post-selection expiry
precedes newly reached boundary criteria. Zero time still scores and retains the
initial solution unless an earlier limit applies. Existing validation/zero-evaluation
and empty-space behavior remains in place.

ElapsedTimeLimit exposes exact Duration threshold and capped progress calculations.
Two fully adapted TimeMillisSpentTerminationTest methods preserve all twelve
threshold and twelve gradient assertions, recorded with notices/provenance.
Eight original unit regressions inject controlled clocks into the actual algorithms
across all six model/backend paths, check saved-prefix selection, rejected prefixes,
initial scoring, no clock use when disabled, zero time, overlapping limits,
regressing injected timestamps and latched expiry. A public Instant-backed zero
limit is verified without sleeps. No direct Java timed-search, whole-solve,
construction, unimproved-time, cancellation or preemption claim.

Development/release all-target suites pass: 799 Rust tests (436 adapted, 342
original integration, 21 original unit), 30 doctests and 67 Python checks.
Coverage 436 / 4,855 = 8.98%; Solver lifecycle 13 / 379 = 3.43%; streams remain
275 / 1,301 = 21.14%. Pending 4,137, excluded 282, failed/unverified zero. Full goal
unfinished. Logs use work/timed-search-{check,release,stream-evidence,
stream-performance,list-refresh,core-refresh,heuristic-performance}.log.

All nine comparison/performance report sections are refreshed and current,
including 219 termination and 128 exact-search cases. Stream medians ms Rust/Java:
hill32 46.686/16.833, hill96 183.258/27.170, tabu32 46.238/17.122,
tabu96 175.950/22.806. Both 96-variable scores remain [-44,-484], infeasible at
50 steps. List32 construction remains Rust [0,-1080] versus Java [0,-1062].
Benchmarks leave timing disabled and retain their existing deterministic budgets;
no elapsed-limit runtime comparison or performance improvement is claimed.

The live server fingerprint matches. Browser checks at 1280px and 390px verify
8.98% overall, Solver lifecycle 3.43%, all thirteen passed lifecycle cases including
the two elapsed-time methods, no stale alert, no page errors and no horizontal
overflow. Existing server reused; browser session closed; upstream clean.

## Cooperative unimproved-time increment

LocalSearchConfig now includes max_unimproved_time, disabled by default, and
LocalSearchTermination distinguishes UnimprovedTimeLimit. All six local-search
model/backend paths share the monotonic clock and behavior. Initialization and
initial diagnostics are excluded from stagnation timing. Only an installed strict
retained-global-best improvement resets it; ties, worsening fallback and recovery
to an old best do not. Polling before reset preserves expiry even when scoring
overshoots the deadline. An improving selected prefix remains in the result.
Total time wins simultaneously observed time expiry; already-observed evaluation
exhaustion retains its precedence. See the incremental-search contract.

Six additional original controlled-clock regressions exercise these semantics,
including long initialization, zero limits, backwards clock samples and maximum
Duration boundaries. No new upstream method credit: the six upstream
UnimprovedTimeMillisSpentTerminationTest methods remain pending, including their
mocked solver/phase lifecycle and construction suppression assertions. Source test
was inspected as specification only; no source production implementation read.
No direct Java timed-search or wall-clock trace parity claim.

Development and release checks pass: 805 Rust tests (436 adapted, 342 original
integration, 27 original unit), 30 doctests and 67 Python regressions. Coverage is
unchanged at 436 / 4,855 = 8.98%; lifecycle 13 / 379 = 3.43%. All nine evidence
sections are refreshed and current. Original deterministic termination comparisons
remain 219 / 219 with 398 independently checked native steps. Logs use
work/unimproved-time-{check,release,stream-evidence,stream-performance,list-refresh,
core-refresh,heuristic-performance}.log.

Stream medians ms Rust/Java: hill32 46.216/17.817, hill96 176.526/35.654,
tabu32 46.135/18.030, tabu96 170.932/31.718. Scores still agree, including
infeasible [-44,-484] at 96 variables after 50 steps. List32 construction remains
Rust [0,-1080] versus Java [0,-1062]. Both timers are disabled in deterministic
benchmarks; this increment makes no performance-improvement claim.

Live server fingerprint matches the report. Browser checks at 1280px and 390px
show 8.98% coverage, all 13 passed lifecycle cases, no stale alerts, page errors
or horizontal overflow. Existing server reused, browser closed. Upstream remains
unmodified; the full goal remains active and unfinished.


## Mixed solver-quality evidence increment

MixedSolverOracle.java uses pinned Community public BRUTE_FORCE configuration
with unsorted entities/values for coupled depot/ordered-route problems. The new
native mixed_solver_oracle runs joint exhaustive search plus scoped/stream local
search. mixed_solver_checks.py generates 64 original cases (seed 20261123), validates
all returned assignments/scores and enumerates 32,116 joint reference assignments.
The report is nested in solver_differential.mixed_solver_checks and shown separately
on the dashboard, with no upstream method credit or mixed timing claim.

Observed optima: Rust exact 64/64, Community BRUTE_FORCE 39/64, native scoped and
stream local 63/64 each. Different starts/strategies are explicit. Community starts
unassigned; native local starts at the first depot with all customers on owner
zero, using strict improvement, best accepted, joint moves and 20 steps. Native
backends also agree on exact assignments/counters/reasons. Exact native work must
match the complete independent product count. This is quality evidence, not a
claim of equivalent algorithms or Java phase/counter traces.

The literal coupled-depots case returns Java 0hard/-22soft versus the independently
proven and native exact 0hard/-4soft. Keep this discrepancy visible. The native
local trap is mixed-search-18: depots [-4,-7,9], customer points [5,-8,8,-4],
capacities [1,3]. Both native local backends return depots [9,-7], routes [[2],
[0,3,1]], 0hard/-28soft after 751 evaluations/3 steps (NoAcceptedMove). Exact joint
optimum is 0hard/-26soft over 1,080 assignments; Java returns 0hard/-32soft. The
raw case corpus is work/oracle/mixed-solver-cases.json. Future improvements can
use this case without pretending every local optimum is globally optimal.

Five original Python regression methods guard the validator and quality accounting.
No upstream fixture or production implementation was copied. Inspected upstream
mixed exhaustive test assertions are phase/trace-specific and remain pending.

Verification: bash scripts/check.sh passes (805 Rust tests, 30 doctests, 72 Python
regressions), as does the release all-target suite. All nine measurement/comparison
sections are regenerated and current; live server fingerprint matches. Overall
coverage stays 436/4,855 = 8.98%, pending 4,137, excluded 282, failed/unverified zero.
No extra upstream method credit is claimed for these original comparisons.
Logs use work/mixed-solver-{check,release,stream-evidence,stream-performance,
list-refresh,core-refresh,heuristic-performance}.log. Stream medians ms Rust/Java:
hill32 48.376/14.499, hill96 178.025/27.516, tabu32 48.157/13.076,
tabu96 180.425/27.466; scores still agree. These are existing stream workloads,
not mixed-search timings or a performance-improvement claim.

Dashboard checks at 1280px and 390px confirm the new 64 valid cases and distinct
64/39/63 optimum counts, no stale alert, no page errors or horizontal overflow;
manual reload works. Existing server reused; browser session closed. Upstream
unchanged and full goal remains active. Next useful work includes matched mixed
Java local-search evidence, broader neighborhoods/acceptance for the retained
local trap, and remaining upstream cases without relaxing their assertions.

## Sublist relocation/reversal and neighborhoods

ListMove::SubListChange now atomically removes a half-open span, optionally reverses
it and inserts at a post-removal destination. All bounds are checked before draining;
final pins/ranges and rollback follow existing list transactions. Empty spans and
singletons are supported. Touched owners feed existing scoped and stream cache
invalidation, including combined mixed moves.

list_sublist_change_moves lazily enumerates legal spans of length 2..=max_length,
respecting pinned prefixes, full pins and destination ranges. Forward same-position
no-ops are omitted; aliases are allowed. Neighborhood::ChangeSwapAndSubList adds
these after existing list moves, including inside mixed joint products. Basic-only
search still includes changes/swaps. Bounds below two add no spans and usize::MAX
is clipped safely. Default neighborhood order/count remains the same.

Seven fully adapted SubListChangeMoveTest methods preserve all ordered-state,
undo and touched-entity assertions. Provenance and notices are recorded; other
metadata/rebase methods remain pending. Five original regressions check errors,
pins/ranges, panic rollback, independent successor sets, 120 scoped/stream shadow-
sensitive trial/commit rounds, list and joint-mixed plateau escape in both backends,
and basic swap retention. This does not claim Java span-search trace equivalence.

The list move protocol adds r,entity,start,end,destination,index,reverse. The
original 256 cases are preserved; the 128-case supplement includes two attributed
source fixtures, two original boundaries and 124 generated cases (seed 20261124). All
384 cases / 1,348 operations compare Java public MoveTester and native cached
stream candidate/undo/commit states plus four derived shadows to independent
Python slicing. One added Python regression validates span indexing/reversal and
rejects malformed metadata. Generated cases add no extra upstream method credit.

Development and release all-target suites pass: 817 Rust tests (443 adapted,
347 original integration, 27 unit), 30 doctests and 73 Python checks. Overall
coverage is 443/4,855 = 9.12% on the dashboard; report.percent rounds separately
to 9.125, and the UI correctly derives its display from counts. Moves & selectors
is 66/874 = 7.55%. Pending 4,130; exclusions 282; failed/unverified zero. Logs use
work/sublist-change-{check,release,stream-evidence,stream-performance,list-refresh,
core-refresh,heuristic-performance}.log. Full goal remains active and unfinished.

All nine reports are refreshed/current and the live fingerprint matches. Existing
stream medians ms Rust/Java: hill32 46.951/15.840, hill96 181.182/29.181,
tabu32 47.153/12.410, tabu96 177.225/24.777; equal scores remain, including
infeasible [-44,-484] at 96 variables. These timings do not measure the opt-in
span neighborhood. Mixed quality checks retain 64/39/63 optimum counts under their
existing configurations; no claim that the new neighborhood fixes that local trap.

Browser checks at 1280px and 390px verify 9.12% coverage, moves/selectors 7.55%,
all seven passed SubListChangeMoveTest adaptations and 384/384 list cases, with
no stale alerts, errors or horizontal overflow. Existing server reused; browser
closed; upstream unmodified. Remaining work includes sublist swapping and provider
assertions, broader search quality/performance evidence and pending solver features.

## Disjoint sublist swaps — latest verified increment

ListRange and ListMove::SubListSwap exchange two nonempty disjoint pre-move spans,
including unequal lengths, adjacent spans, reversed argument order and reversal of
both exchanged blocks. Empty/overlapping/invalid ranges fail atomically. Pins,
per-owner ranges, composite rollback and panic rollback use the existing guard.
Touched owner metadata integrates scoped/stream caches and mixed scoring.

list_sublist_swap_moves lazily enumerates canonical owner/start pairs, bounded
lengths and both orientations. At least one span has length >=2; single/single
moves already belong to ordinary swaps. Prefix/full pins and receiving ranges are
respected. ChangeSwapAndSubList now adds these moves after sublist relocations;
default neighborhoods remain unchanged. No Java span-search trace claim is made.

Five complete SubListSwapMoveTest methods are adapted with provenance/notices:
non-overlap, out-of-order normalization, differing lengths, temporary undo and
cross-owner metadata. Java pre-move value metadata, equality/string/rebase and
selector mock listener assertions remain pending. Five original regressions cover
all disjoint spans across every two-owner split of seven values, both argument
orders and reversals, independent successor sets, invalid edits/rollback,
pins/ranges, 120 scored trial/commit rounds and list/joint-mixed plateau escape in
both backends where a single relocation cannot reach the target.

The list oracle retains its previous 384 inputs unchanged and appends 128 original
cases (seed 20261125). All 512 cases / 1,795 operations agree between Java public
MoveTester, native cached views and the Python pre-move reconstruction reference,
including candidate, undo, commit and owner/index/previous/next shadows. These
original checks receive no upstream-method credit.

Development and release all-target suites pass: 827 Rust tests (448 adapted,
352 original integration, 27 unit), 30 doctests and 74 Python checks. Coverage is
448/4,855 = 9.23% displayed; moves/selectors 71/874 = 8.12%. Pending 4,125,
excluded 282, failed/unverified zero. All nine reports were regenerated from the
same measured sources; live freshness matches. Logs are
work/sublist-swap-{check,release,stream-evidence,stream-performance,list-refresh,
core-refresh,heuristic-performance}.log.

Stream medians ms Rust/Java: hill32 48.761/20.076, hill96 170.120/25.951,
tabu32 47.850/14.646, tabu96 170.319/25.062. Both 96-variable stream runs retain
[-44,-484] (infeasible at 50 steps). List32 construction still has unequal quality:
Rust [0,-1080], Java [0,-1062]. Mixed quality retains 64/39/63 optimum counts for
native exact/Java/native local under documented starts and strategies. These
existing performance/quality workloads do not enable the new span neighborhood.

Browser checks at 1280x900 and 390x844 verified displayed coverage, all five new
passed methods and 512/512 list comparisons, without horizontal overflow, visible
alerts or page errors. Browser session closed; upstream unmodified. Remaining
work includes broader provider and listener semantics, Java comparisons using
larger neighborhoods, stream performance and pending solver infrastructure.
The overarching rewrite goal is still active.

## Dedicated two-opt/tail providers — latest verified increment

list_tail_swap_moves exchanges nonempty tails across owners, optionally reversing
both except when both tails are singletons. list_two_opt_moves reverses each
unpinned within-list span of length >=2, then optionally exchanges cross-owner
tails without reversal. Both generators are finite/lazy, use canonical owner
pairs, and preserve pins and both receiving value ranges. They reuse existing
span primitives; no new list mutation implementation or Java sampling-order claim.

Neighborhood::TwoOpt { cross_entity } selects only this list set, including in
both list/mixed scoring backends and mixed joint products. Basic variables retain
changes/swaps. Empty owners cannot receive values and optional assignment status
cannot change through these moves. Defaults remain unchanged. The selected list
iterator is boxed once per enumeration: the initial combined concrete iterator
type overflowed the debug stack in the existing mixed_local_search lazy-product
regression. That test and the full gate pass with bounded dispatcher stack usage.

Twelve complete ListTailSwapMoveProviderTest/TwoOptListMoveProviderTest methods
are adapted with provenance/notices, including supporting two-entity and ranged
fixtures. Four original methods check independent small successor sets with
pins/ranges/empty owners, 120 scored trial/commit rounds, list/joint-mixed plateau
escape in both backends, and basic-swap/optional-list behavior. An initial original
test fixture mistakenly assigned an out-of-range value and was corrected before
verification; no implementation relaxation was made.

Development and release all-target gates pass: 843 Rust tests (460 adapted,
356 original integration, 27 unit), 30 doctests and 74 Python checks. Overall
460/4,855 = 9.47% displayed; moves/selectors 83/874 = 9.50%. Pending 4,113,
excluded 282, failed/unverified zero. Existing 512 Java/Rust/Python list-move cases
(1,795 operations) still pass. They check the underlying primitives and do not
establish Java dedicated-provider enumeration or two-opt solve equivalence.

All nine reports were refreshed sequentially against the final measured sources;
source and live verification digests match. Logs:
work/two-opt-{check,release,stream-evidence,stream-performance,list-refresh,
core-refresh,heuristic-performance}.log. Stream medians ms Rust/Java:
hill32 44.526/14.349, hill96 168.554/28.212, tabu32 46.908/14.759,
tabu96 178.145/28.211. Both 96-variable stream runs remain [-44,-484] at 50 steps.
List32 construction remains Rust [0,-1080] vs Java [0,-1062]; mixed-search quality
remains exact64/Java39/native-local63 optima among 64 original cases. These
workloads retain their default neighborhoods; no two-opt performance result.

Browser QA at 1280x900 and 390x844 verified displayed counts and all twelve new
passed methods, with no horizontal overflow, nonempty alerts or page errors.
Browser session closed; upstream is unmodified. Remaining work includes broader
move/provider semantics, direct Java route-neighborhood solving comparisons,
stream performance and pending solver infrastructure. Goal remains active.

## Java/native two-opt solving comparison — latest verified increment

ListSolverBenchmark.java now selects the Community public preview
TwoOptListMoveProvider through NeighborhoodProvider/NeighborhoodBuilder and
NEIGHBORHOODS. Only public signatures and pinned tests were consulted, without
production implementation inspection. The native list_solver_benchmark selects
Neighborhood::TwoOpt { cross_entity:true }. Both use first-accepted hill climbing,
complete identical starts, the requested step limit (default 20) and a 10,000
score-calculation/evaluation budget. Java seeded sampling and native deterministic
finite enumeration differ, as does score-count accounting. No equal-work or
identical search-trace claim; final state/score/pins/shadows are validated.

benchmark_lists.py preserves the prior construction/relocation inputs and adds
64 two-opt cases copied from the complete original/ranged hill inputs with new
IDs. All 192 cases are valid; 183 have equal scores. Per-phase optima/equalities:
construction 64 equal, Rust 48 / Java 48 optimal; relocation 64 equal, Rust 61 / Java 61;
two-opt 55 equal, Rust 44 / Java 46. These are optima in the full pin/range-constrained
routing space, not necessarily reachable within the selected neighborhood.
No adapted-method credit. Two original Python tests ensure corpus/start identity,
workload completeness and rejection of regressing/impossible/unknown-mode results.

The list dashboard now has six warmed timing cards and per-phase quality counts.
Two-opt median ms Rust/Java: 16 customers 0.059/9.042 with scores
[0,-1484]/[0,-1254] and calls 39/155; 32 customers 0.190/10.004 with scores
[0,-2472]/[0,-2070] and calls 134/153. Rust is faster but finds worse solutions;
the UI marks both ratios as quality-different, retains raw score/callback/range
information, and states sampling differences. The existing list32 construction
quality gap remains [0,-1080]/[0,-1062]. Three forks, three warmups and five measured
solves per fork; smoke-run startup-heavy numbers were replaced by full measurements.

Full development/release gates pass: 843 Rust tests, 30 doctests, 76 Python checks.
Adapted source coverage stays 460/4855 = 9.47%, moves 83/874 = 9.50%, with 4113 pending,
282 excluded and zero failed/unverified. All nine reports were refreshed from the
final measured sources; live freshness matches. Logs:
work/two-opt-comparison-{check,release,stream-evidence,stream-performance,
list-refresh,core-refresh,heuristic-performance}.log.
Stream median ms Rust/Java: hill32 43.316/16.801, hill96 177.939/33.612,
tabu32 48.489/15.111, tabu96 176.643/27.151. Stream96 retains [-44,-484] at 50 steps.
Mixed quality counts remain exact 64 / Java 39 / native local 63 among 64 original cases.

Desktop 1280x900 / mobile 390x844 checks verified six list timing cards, three explicit
quality-difference labels, two-opt 55/64 equality and 44/46 optima, 192/192 valid cases,
correct budget caption and 9.47% coverage, with no overflow, stale alerts or page
errors. Browser closed; upstream unchanged. Next work should investigate the
native deterministic first-accepted route-search quality gap, along with remaining
solver behavior and stream performance. The overall rewrite goal remains active.

## Bounded seeded move ordering — latest verified increment

The original first-accepted two-opt 16-customer workload entered a two-state
neutral reversal cycle after five steps (independently replayed with Python).
MoveOrder::ShuffledMoves { seed, window: NonZeroUsize } now optionally shuffles
candidate order for all three model types and both scoring backends. A rolling
buffer fills from the lazy source, selects a random buffered move with SplitMix64,
and replenishes on the next request. Whole-neighborhood windows give uniform
permutations; smaller windows give local shuffling, not globally uniform sampling.
No Java PRNG/trace claim and no general quality guarantee.

Window size is capped per scan by remaining evaluation budget, minimum one to
preserve exhausted-budget detection. No eager pulls or requested-size reservation;
first output pulls at most the effective window, later outputs at most one more
candidate. Original and ShuffledVariables behavior/random sequences remain intact;
a one-slot window preserves original order without random draws. Generation is
unscored, and bounded prefetch may overshoot cooperative time limits. Acceptance
randomness stays independent. Early selection discards unused lookahead.

Three original unit tests cover conservation/repeatability across 64 seeds and
window sizes, bounded lazy pulls on infinite sources with huge requested windows
and tiny budgets, and empty/singleton/non-fused sources. Three original integration
methods check neutral-cycle escape across basic/list/joint-mixed models with both
backends, backend score/counter/termination agreement, repeatability, one-slot
identity and evaluation limits. This adds no upstream coverage credit.

The native two-opt benchmark explicitly opts into seed 20261006/window 1024;
library defaults remain Original and other benchmark configurations are unchanged.
All 192 routing cases remain valid. Two-opt equality rises from 55 to 61 of 64;
native optima rise from 44 to 49 while Java stays at 46. Total equal-score cases
are 189/192. This comparison changes native ordering configuration; Java uses its
unchanged public seeded provider. The same numeric seed does not match its PRNG.

Warmed two-opt medians ms Rust/Java: 16 customers 0.262/9.241, scores
[0,-1250]/[0,-1254], calls 132/155; 32 customers 0.868/13.152, scores
[0,-1964]/[0,-2070], calls 207/153. Native prior deterministic scores were
[0,-1484] and [0,-2472]. Different final quality remains labeled beside ratios;
these are selected bounded workloads, not general solver speed comparisons.

Development/release gates pass: 849 Rust tests (460 adapted, 359 original
integration, 30 unit), 30 doctests and 76 Python checks. Clippy caught split generic
bound declarations during development; corrected before the final full gate.
Coverage remains 460/4855 = 9.47%, moves 83/874 = 9.50%; pending 4113, excluded 282,
failed/unverified zero. All nine reports were refreshed sequentially from final
sources, and live freshness matches. Logs:
work/move-shuffle-{check,release,stream-evidence,stream-performance,list-refresh,
core-refresh,heuristic-performance}.log. Stream median ms Rust/Java:
hill32 47.657/17.186, hill96 173.467/28.744, tabu32 48.448/20.014,
tabu96 175.321/25.785. Stream96 remains [-44,-484] at 50 steps; list32 construction
and mixed-solver quality differences remain as previously documented.

Browser checks at 1280x900 / 390x844 verified six list cards, both updated two-opt
scores/times, 61/64 equality, 49/46 optima and the documented 1,024 window, with no
horizontal overflow, stale alerts or page errors. Browser closed; upstream
unmodified. Remaining work includes broader move/search infrastructure, additional
source semantics, and stream performance. The overarching goal remains active.


## Cooperative cancellation — latest verified increment

CancellationToken provides clone-shared, one-way atomic cancellation with an
idempotent cancel() result. LocalSearchConfig<'a, S> optionally borrows a token;
MixedLocalSearchConfig carries the same lifetime. Both remain Copy. Helpers or
aliases with an explicit return/type signature may need a lifetime; configurations
without a borrowed token can use 'static. Default configuration has no token.

All six local-search model/backend paths poll before scoring, at step boundaries,
between candidates and after selection. A pre-requested valid nonzero-budget run
returns Cancelled with no best and zero work. A request during scoring finishes
that callback and retains the selected scored prefix and the best seen solution.
Rejected fallback can update the working state without losing that retained best.
Validation still precedes cancellation; zero evaluation budget and empty basic
search space retain their previous reasons. Already-observed evaluation exhaustion
wins after selection, otherwise cancellation precedes time/no-accepted reasons.
See docs/incremental-search-contract.md for detailed precedence and API behavior.

Cancellation is cooperative: no preemption of callbacks, validation, diagnostics
or bounded prefetch, and no hard latency bound. Construction, exact search,
whole-solve orchestration, daemon operation and problem-change queues remain
outside this new API. The inspected upstream BasicPlumbingTerminationTest and
DefaultSolver lifecycle cases are not fully adapted, and receive no method credit.

Six original integration methods cover all six paths with first/best selection,
initial/candidate cancellation, repeated runs, zero budgets, invalid inputs,
rejected fallback and a separate controller thread cancelling a blocked callback.
Eight simultaneous requests also verify exactly one first cancel result. One
public API doctest exercises token sharing. The upstream coverage ledger is
unchanged: 460/4855 = 9.47%, pending 4113, excluded 282, failed/unverified zero.

Development and release gates pass: 855 Rust tests (460 adapted, 365 original
integration, 30 unit), 31 doctests and 76 Python checks. All nine comparison and
performance reports were refreshed sequentially from final measured sources;
verification succeeds and the live freshness digest matches. Logs:
work/cancellation-{check,release,stream-evidence,stream-performance,list-refresh,
core-refresh,heuristic-performance}.log.

Refreshed stream median ms Rust/Java: hill32 45.457/16.404, hill96 181.838/29.041,
tabu32 45.809/16.556, tabu96 165.014/23.577. The existing stream performance gap
remains; both 96-variable workloads retain [-44,-484] at 50 steps. List validation
remains 192 valid / 189 equal scores; two-opt 61/64 equal and Rust49/Java46 optima.
Two-opt medians: 16 customers 0.226/10.339 ms, 32 customers 0.852/9.866 ms; scores
and callback counts are unchanged. Mixed quality remains native exact64 / Java39 /
native scoped and stream local63 optima among64. Cancellation is disabled in these
benchmarks; no cancellation-latency comparison is claimed.

Browser checks at 1280x900 and 390x844 verified coverage, passed-method filtering,
six list timing cards with refreshed figures and explicit quality differences,
no horizontal overflow, no stale alerts and no page errors. Browser closed;
upstream unmodified. Remaining work includes broader solver lifecycle semantics,
additional source-test adaptations and stream performance. The goal stays active.

## Construction and exact-search cancellation — latest verified increment

The shared CancellationToken now controls all six construction paths and all three
exact-search models. ConstructionConfig<'a> borrows an optional token and remains
Copy; MixedConstructionConfig carries the lifetime. Full struct literals now need
cancellation: None or a default update, and named token-free helper return types
may use 'static. Defaults remain unchanged apart from the disabled option.

Construction validates first, retains impossibility/zero-budget precedence, then
checks the token before initialization, before each required item, between emitted
candidates and at completion. Interrupted scans install the first best evaluated
candidate using its cached score and retain partial or complete assignments and
counters. Callbacks and diagnostics finish; diagnostics are excluded from counters.
An observed evaluation cap wins, otherwise the final cancellation poll wins even
if the last candidate completed the model. A complete assignment and a score are
still both required for solution(). Neither completion nor cancellation proves
hard feasibility or optimality.

ExactSearchConfig<'a> defaults to max_evaluations 10,000 and no token. The new
exhaustive_search_with_config, exhaustive_list_search_with_config and
exhaustive_mixed_search_with_config entry points accept it. Existing budget-only
signatures delegate with no token. Budget exhaustion is checked before cancellation
at each complete candidate. Active callbacks finish and eligible best scores are
retained. Enumeration is not preempted. The final candidate can request cancellation
and still prove exhaustion; empty spaces are also exhausted, without an optimal
solution. Zero-variable models have one empty assignment that still needs scoring.
There is no bounded cancellation latency. Whole-solve orchestration, daemon
operation, problem changes and construction elapsed-time limits remain pending.

Five construction regressions cover all six paths, both mixed orders, early-pick
and full-scan policies, every small budget/request prefix, independent assignment
and score validation, controller-thread requests, diagnostic cancellation, token
reuse, impossible models, invalid shape/source and already-complete initialization.
Three exact-search regressions cover all basic/list/mixed request/budget prefixes,
pins and optionality, eligible-best retention, empty versus singleton-empty spaces,
final-callback exhaustion and controller-thread requests. One public API doctest
shows the new exact entry point. These are original tests, not adapted Java
lifecycle methods; no provenance or adapted-method count changes.

Development gate passes: 863 Rust tests (460 adapted, 373 original integration,
30 unit), 32 doctests and 76 Python checks. Test development caught missing explicit
None fields in older struct literals, incorrect helper API names and score-type
inference in panic-only callbacks; corrected before the full gate. Coverage stays
460/4855 = 9.47%, pending 4113, excluded 282, failed/unverified zero.

Release also passes all 863 Rust tests. All nine evidence/performance sections
were refreshed sequentially from final measured sources and report current;
verification succeeds and the live freshness digest matches. Logs:
work/all-search-cancellation-{check,release,stream-evidence,stream-performance,
list-refresh,core-refresh,heuristic-performance}.log.

Stream median ms Rust/Java: hill32 44.456/14.573, hill96 171.946/23.270,
tabu32 48.006/16.069, tabu96 168.323/23.237. The existing stream gap and
96-variable infeasible scores [-44,-484] remain visible. List cases remain
192 valid / 189 equal scores; two-opt remains 61/64 equality and Rust49/Java46
optima. Two-opt medians: 16 customers 0.239/9.138 ms, 32 customers 0.980/13.316 ms.
Scores, call counts and mixed-quality counts are unchanged. All these benchmarks
use disabled cancellation; no cross-runtime cancellation latency is claimed.

Desktop 1280x900 and mobile 390x844 browser checks confirm 9.47% coverage,
passed-method filtering, six list and four stream timing cards, refreshed timing
figures, no overflow, no stale alerts and no page errors. Browser closed; upstream
unmodified. The full rewrite goal remains active, with solver orchestration,
additional upstream semantics and stream performance still unfinished.

## Independent candidate budgets — latest verified increment

ConstructionConfig and LocalSearchConfig now have max_move_evaluations: Option<u64>,
disabled by default. It caps every evaluated candidate, including rejected trials,
while excluding initialization and diagnostic recomputations. The six result types
expose move_evaluations() alongside total score evaluations, accepted moves/steps
and constraint-callback counts. Mixed configurations inherit the control. Existing
full construction config literals were updated with explicit None fields.

A shared internal CandidateBudget determines the tighter remaining score/move cap
before selection, and retains which reason applies afterward. Strictly tighter
candidate caps report MoveEvaluationLimit; score caps and ties report the existing
EvaluationLimit. Both construction and local search install the best evaluated
prefix before reporting an observed limit. Local shuffled lookahead also uses the
tighter budget. A zero candidate cap permits initial scoring, whereas a zero score
cap does not. Caps are detected by a further candidate attempt, so already-complete
construction/empty neighborhoods retain completion/no-move reasons. Step/score
boundary criteria, cancellation and time can stop before a cap is observed;
already-observed caps retain precedence over later polls or diagnostic requests.
These are phase limits; global multi-phase accounting remains unfinished.

CountLimit provides exact u64 threshold checks, saturating remaining work, capped
f64 progress, and checked signed input through TryFrom<i64>. Near u64::MAX the
fraction may round to 1 before the limit is reached; stopping uses exact integers.
All five methods in MoveCountTerminationTest and ScoreCalculationCountTerminationTest
are fully adapted (48 threshold/gradient assertions plus negative-limit rejection).
Provenance and notices record native explicit counters replacing Java mocked
scopes/directors; no Java lifecycle or counter-accounting equivalence is claimed.

Five original integration methods test small score/move budget pairings across all
six local-search and six construction paths, strict/hill acceptance, first/best
selection, fallback, bounded shuffling, mixed construction order, early picking,
diagnostic exclusion and cancellation precedence. One original controlled-clock
unit method distinguishes time-first, move-first and tied score/move caps across
all local-search paths. Existing timed and cancellation regressions still pass.

Development gate passes: 874 Rust tests (465 adapted, 378 original integration,
31 unit), 32 doctests and 76 Python checks. Coverage is 465/4855 = 9.58%, pending
4108, excluded 282, failed/unverified zero. Solver lifecycle rises to 18/379;
other family adapted counts are unchanged. Final measured sources include the
clarified precedence documentation and all tests; upstream remains unmodified.

Release also passes 874 Rust tests. All nine evidence and performance reports were
refreshed sequentially from final sources; verification and live freshness match.
Logs: work/move-budget-{check,release,stream-evidence,stream-performance,list-refresh,
core-refresh,heuristic-performance}.log. Stream medians ms Rust/Java: hill32
45.184/13.161, hill96 166.899/25.542, tabu32 44.990/14.455, tabu96 176.120/25.963.
Existing stream96 infeasible scores remain [-44,-484]. List validation remains
192 valid/189 equal scores, two-opt61/64 equality and Rust49/Java46 optima. Two-opt
medians16 0.307/9.182 ms,32 0.849/10.949 ms. Mixed quality and other solution scores
are unchanged. Timed workloads leave the new move cap disabled; these timings
refresh the baseline and do not claim a new Java move-limit comparison.

Browser checks at 1280x900 and 390x844 verified 9.58% coverage, lifecycle18/379,
the new source methods and passed-only lifecycle filtering, refreshed benchmark
figures, six list cards, no horizontal overflow, no stale alerts and no page errors.
Selecting a family resets filters as before; reapply Passed after selecting it.
Browser closed; upstream unmodified. Overall rewrite remains active. Whole-solve
orchestration/accounting, broader source semantics and stream performance remain
substantive unfinished work.

## Multi-phase solver orchestration — latest verified increment

Public solve/solve_list/solve_mixed entry points now compose construction and an
ordered vector of local-search phases using scoped networks. solve_stream,
solve_list_stream and solve_mixed_stream use fallible fresh-graph factories. All six
share one engine in crates/solver/src/solver.rs. Read docs/solver-contract.md before
changing ownership, limits or lifecycle behavior.

SolverConfig defaults to construction plus one local phase; MixedSolverConfig
preserves construction order and each phase's joint-move flag. SolverLimits defaults
to 100,000 total score evaluations with optional global candidate cap, cancellation,
score target and feasibility stop. Phases receive the tighter own/remaining caps.
Each phase's initial score counts separately; candidate counts sum per phase instead
of subtracting one from the total. Diagnostics are excluded. No time is hidden in a
claimed whole-solve deadline: elapsed/stagnation timing remains phase-local.

Every local phase starts from the retained best complete assignment with fresh
acceptor/seed state. Incomplete construction returns partial progress without
entering local search. Complete construction truncated by its own cap can continue.
Whole-solve goals stop later phases; phase-local goals/limits normally allow them.
An observed matching shared limit stops the solve, with equal caps treated as
shared. Before later phases, zero remaining score/move budgets stop without another
initial score (score wins simultaneous exhaustion). PlanCompleted means the plan
ended, not that every phase exhausted its neighborhood or that optimality is proven.

A single global cancellation token is forwarded to all phases. A phase token can
be absent or share the global token's identity; incompatible simultaneous tokens
fail preflight instead of silently losing one. Without a global token, distinct
phase tokens are allowed; a cancelled phase ends the solve. All acceptance/token
settings validate before callbacks, including unused future phases. Graph factories
are lazy per started phase; model/network/source validation remains at entry points.
Errors identify their phase and do not return prior partial results. Graph factories
must preserve the same objective/facts. Whole-solve timing, daemon/problem changes,
solver-manager services and cross-phase score-cache reuse remain pending.

SolverResult returns assignment, optional score, completeness, aggregate SolverWork,
ordered PhaseReports and overall SolverTermination. solution() only exposes complete
scored results, without a hard-feasibility or optimality implication. Phase reports
retain original reasons and logical score/candidate/step/acceptance/callback counts.

Eleven original integration methods cover all six paths, exact small shared-budget
boundaries and competing own/global budgets, two-phase handoffs, target scoping,
zero-work phases, partial initialization, preflight, graph errors, impossible models
and cross-thread cancellation in the second improvement phase. A mixed four-state
coupled trap verifies that a later phase's joint flag reaches the independent exact
optimum. Initial test development incorrectly expected equal callbacks for one joint
move; the stream join legitimately evaluates after both basic and list row delivery.
The corrected test explicitly verifies eight scoped versus nine stream callbacks,
with identical score/move/step counts, assignments and phase reasons.

The mixed_routes example now runs automatic scoped/stream solves for both orders,
independently recomputes scores and checks the known optimum 0hard/-4soft. It matches
manual phase composition: 141 evaluations basic-first, 173 list-first; callbacks
249/353 and313/469 scoped/stream. One API doctest shows basic usage. README stale
coverage and case totals were corrected and the new contract linked.

Development gate passes: 885 Rust tests (465 adapted,389 original integration,31 unit),
33 doctests and76 Python checks. No new upstream method credit: coverage465/4855
=9.58%, pending4108, excluded282, failed/unverified zero. No production upstream
code inspected or copied, and the pinned checkout is unmodified.

Release also passes all885 Rust tests and the mixed_routes example. All nine evidence
and performance reports are refreshed/current with matching live freshness. Logs:
work/solver-plan-{check,focused,example,release,release-example,stream-evidence,
stream-performance,list-refresh,core-refresh,heuristic-performance}.log.
Stream medians ms Rust/Java: hill32 45.103/12.995,hill96 165.897/23.362,
tabu32 44.349/12.609,tabu96 168.140/24.675. Existing stream96 infeasibility remains
[-44,-484]. List validation stays192 valid/189 equal scores; two-opt61/64 equality
and Rust49/Java46 optima. Two-opt medians16 0.241/7.694ms,32 0.838/11.837ms.
Mixed quality counts and other final scores are unchanged. These benchmarks retain
their existing direct-phase entry points; they do not claim orchestration timing.

Browser1280x900/390x844 checks confirm9.58% coverage, passed-method filtering,
six list cards, refreshed list/stream figures, no overflow, stale alerts or errors.
Browser closed; upstream unchanged. The overall rewrite remains active. Shared
elapsed/stagnation controls, problem changes/daemon/manager services, further
upstream adaptations and stream performance remain substantive unfinished work.


## Construction and shared elapsed time — latest increment

ConstructionConfig::max_time now cooperatively bounds all six basic/list/mixed
construction paths. Initial scoring and diagnostics count as elapsed time; zero
permits the initial score only. Polls before items, between candidates and at the
end retain the best evaluated prefix and distinguish TimeLimit from completion,
including partial, already-complete, empty and optional-only models. Observed
count caps and cancellation preserve their documented precedence. Disabled timing
never starts or samples a clock. The shared algorithms accept private injected
clocks for deterministic regressions; no sleeps were introduced.

SolverLimits::max_time now spans construction and all configured improvement
phases. The monotonic clock starts on entry to the shared engine, before plan
validation. Each wrapper clips its phase limit immediately before entry, after
stream graph preparation. Factories, diagnostics and phase handoffs consume the
same total budget. An equal same-kind cap belongs to the shared limit; shorter
phase caps can end just that phase. The engine polls on return and before starting
another improvement phase, keeping each original PhaseReport reason. Validation,
initial scoring and active callbacks remain cooperative; passing a remaining
relative duration leaves a handoff/outer-validation interval before the phase
clock starts, so no hard wall-clock bound is claimed. See docs/solver-contract.md.

Six original construction unit tests exercise all model/backend paths, both mixed
orders, early-pick policies, independent score/assignment validation, scored
prefixes, diagnostics, competing limits, backwards clocks and no-required-work
models. Five engine unit tests use actual stream construction/local search and a
shared simulated clock to verify multiple phases and factory cost. Three new public
integration methods verify all six orchestration entry points at zero time,
phase-local continuation and validation/cancellation/count precedence. These
fourteen methods add no adapted upstream credit. No upstream production code was
inspected or copied.

Development gate and release each pass 899 Rust tests: 465 adapted, 392 original
integration and 42 unit methods. Development additionally passes 33 doctests and
76 Python tests. Coverage remains 465/4,855 (9.58%), 4,108 pending and 282 excluded,
with no failed or unverified methods. Logs use work/solve-time-*.log.

Global stagnation, exact-search timing, live changes/daemon/manager services,
additional upstream method adaptations and remaining solver/stream features are
still pending. Keep the overall goal active. Do not infer whole-solver compatibility
or Java timed lifecycle equivalence from native timing regressions.

All nine Java/Rust evidence sections were refreshed successfully against the final
source digest, and the report matches the live server fingerprint. Latest warmed
stream medians (Rust/Java ms): hill32 47.158/14.077, hill96 162.038/23.961,
tabu32 45.585/12.027, tabu96 160.467/23.000. Scores are equal; both 96-variable
runs remain infeasible [-44,-484] at 50 steps. List comparisons remain 192 valid,
189 equal; two-opt remains 61/64 equal with 49 Rust / 46 Java optima. Two-opt16
medians are 0.283/8.884 ms and two-opt32 0.853/10.659 ms, retaining the existing
score gaps. List32 construction remains [0,-1080] Rust / [0,-1062] Java. These are
existing direct-phase workloads with timing limits disabled; no deadline overhead
or orchestrated-solve performance claim is made.

The existing dashboard server on port 8765 was reused. Browser checks at 1280x900
and 390x844 verified 9.58% overall coverage, all 18 passed lifecycle methods
(including the two elapsed-time source methods), six list/four stream cards,
refreshed timing text, no visible stale/error messages, no page errors and no
horizontal overflow. Browser session closed; upstream pin and submodule unchanged.


## Conditional aggregation — latest increment

ConditionalCollector<C> captures selected Some values and rejected None inputs.
The owning CollectNode retains every input in group membership; rejected-only
groups emit C's empty result. Graph helpers group_collect_conditionally and
collect_conditionally evaluate the key/predicate once per put and map values only
when selected. Removals use captured values without callbacks. Selected nullable
payloads are represented by Some(None), distinct from rejection. Active-to-active
replacement delegates C::replace; other transitions insert/remove. Delegate state
is isolated before installing changes, including panic/error paths. This uses the
existing persistent collector/transaction engine, not a new rollback mechanism.

Eight conditionally/conditionallyUpdate methods across uni/bi/tri/quad upstream
collector tests now have complete adaptations. The native tests preserve all
empty, insertion, duplicate, retraction and replacement assertions, source-specific
min/max choices and zero-valued tuple suffixes. They use captured Options and stable
IDs instead of Java retractors/slots. Provenance and notices are recorded; no
Timefold production implementation was inspected or copied.

Six original integration methods additionally check optional payloads, missing
occurrences, delegated average replacement/overflow behavior, callback laziness,
group presence, foreign ports, failed transactions, composition, 6,000 seeded graph
edits (20261203) against independent multisets, and basic construction/local search
with diagnostic rebuilds and saved candidates. Two public API doctests show the
new collector and global graph helper.

ConditionalCollectorOracle.java and examples/conditional_oracle.rs compare
34 original cases / 2,139 states with independent Python scores (seed 20261204).
They cover global selected counts, keyed sums, rejected-only group/global presence,
replacements, key transfers, deletion and trial undo. Java uses Community
SolutionManager.update for each observation; Rust updates persistent graph state.
This establishes scores, not Java match IDs, notification traces or performance.
The new checks run inside differential_streams.py, are saved under
conditional_collector_checks, and appear separately in the dashboard's comparison
details. Two Python regressions validate reference boundary scores and rejection of
missing/extra/corrupt output. Original cases earn no upstream method credit.

Development checks pass 913 Rust methods (473 adapted,398 original integration,
42 unit), 35 doctests and 78 Python tests. Coverage is 473/4,855 (9.74%), with
4,100 pending, 282 excluded, no failed or unverified methods. Constraint streams
are 283/1,301 (21.75%). Logs use work/conditional-*.log. The full solver rewrite
remains active and incomplete; see the earlier remaining-work list.

Release also passes all 913 Rust tests. All nine evidence sections are current;
the existing successful stream differential was retained after verifying its exact
source digest, while the other comparisons/benchmarks were refreshed. Latest warmed
stream Rust/Java medians in ms: Hill climbing · 32 variables: 43.780/12.525; Hill climbing · 96 variables: 163.241/24.892; Tabu · 32 variables: 43.692/16.804; Tabu · 96 variables: 162.558/29.080.
Scores remain equal; 96-variable stream runs are still infeasible [-44,-484] at
50 steps. List comparisons retain 192 valid / 189 equal scores. Two-opt Rust/Java
medians in ms: Two-opt · 16 customers: 0.308/9.569; Two-opt · 32 customers: 0.900/11.348. Existing list construction/two-opt quality
gaps remain visible; these workloads do not measure the new conditional collector.

The existing server on port 8765 was reused. Browser QA at 1280x900 and 390x844
confirmed overall 9.74%, streams 21.75%, all eight passed conditional methods in the
search/filter view, the distinct 34-case/2,139-state comparison detail, six list and
four stream timing cards, no visible stale/error message, no page errors and no
horizontal overflow. The live report verification digest matches, the browser is
closed, and the upstream checkout remains unchanged at the pinned commit.


## Collection membership joins — latest increment

MembershipJoinNode and four graph helpers implement containing, contained-in and
any-common-element joins with optional tuple equality partitions. Independent
persistent inverted indexes retain captured distinct per-row keys and deduplicate
pair identities. Replacements retract lost pairs and put new/surviving pairs;
projections run once per put and never on retraction. Empty-key rows are retained.
Option::None is an explicit key. Persistent roots isolate clone/comparison/payload
preparation before installation, and existing graph transactions restore downstream
failures. See the membership section of docs/constraint-stream-contract.md.

Eight complete size/forEach/remove-last-reuse source methods from the three
membership indexer test classes are adapted, with provenance and notices. Java
entry handles, random iterators and combined-indexer methods remain pending.
Eight original integration methods cover seeded standalone/graph updates, exact
pairs/payloads, commit/undo, failure injection, rebuild, sparse queries, foreign
ports and basic construction/local search with saved prefixes. One unit churn test
and one doctest cover cleanup and public usage. No production upstream code was
inspected or copied.

MembershipJoinOracle.java, examples/membership_oracle.rs and
scripts/membership_join_checks.py compare 54 original cases / 4,420 states for
three relations and zero/one/two equality partitions (seed 20261207). This agreement
matrix deliberately uses distinct non-null collection members. A separate native
54-case / 4,420-state matrix checks duplicates/nulls against independent Python.
Eighteen fixed Java public-API boundary probes retain eight known differences:
containing and any-of can multiply duplicate-member matches, and unpartitioned
containing/contained-in skip the singleton-null match. Rust consistently retains
one pair. Exact probe inputs and observed outputs are documented and asserted;
these differences are not counted as passing compatibility evidence. Five Python
regressions validate the reference logic, payloads, identities, response counts,
fixed divergent results and the distinction between the matrices. The dashboard
comparison detail explicitly shows eight of eighteen probe differences. No
membership timing or Java incremental-notification/match-ID parity is claimed.

Development gate passes 930 Rust methods (481 adapted, 406 original integration,
43 unit), 36 doctests and 83 Python tests. Overall coverage: 481/4,855 (9.91%),
4,092 pending, 282 excluded, no failed/unverified methods. Constraint streams:
291/1,301 (22.37%). Logs use work/membership-*.log. The full solver rewrite remains
active and incomplete; broader orchestration/lifecycle/stream gaps remain as listed
above. Do not infer full membership compatibility from the adapted indexer tests.

Release also passes all 930 tests. All nine evidence sections were refreshed and
are current against the final source digest; the live verification digest matches.
Warmed stream Rust/Java medians in ms: hill32 43.844/12.979, hill96
170.473/22.532, tabu32 42.826/10.641, tabu96 172.390/26.769. Scores remain
equal, with both 96-variable runs infeasible [-44,-484] at 50 steps. List solving
retains 192 valid / 189 equal-score cases. Two-opt medians Rust/Java: 16 customers
0.336/9.109 ms, 32 customers 0.858/10.498 ms; the existing scores remain
[0,-1250]/[0,-1254] and [0,-1964]/[0,-2070]. List32 construction remains
[0,-1080]/[0,-1062]. These existing workloads do not use the new membership node;
no membership performance claim is made.

Reused the existing dashboard server. Browser QA at 1280x900 and 390x844 verified
9.91% overall, 22.37% streams, all eight new adapted methods under the passed
Contain filter, the distinct/non-null agreement and explicit 8/18 boundary
differences, six list/four stream timing cards, no visible alerts or page errors,
and no horizontal overflow. Browser closed. Upstream pin and checkout unchanged.


## Exact-search elapsed time — latest increment

ExactSearchConfig now has optional max_time (default None), and Termination includes
TimeLimit. All three configurable public exact entry points start a monotonic clock
before setup. Controlled-clock internal counterparts reuse the existing TimeBudget.
One timer spans mixed product enumeration and owned joint candidate preparation.
Each complete candidate checks evaluation count, then cancellation, then time before
scoring; mixed time is checked after its joint assignment copy. Active callbacks
finish and eligible best results are retained even if the callback overruns. Real
exhaustion wins over limits, including after the last callback; impossible spaces
remain exhausted with no candidate, while singleton-empty assignments need scoring.
No preemption occurs inside enumeration/value operations/callbacks, and no hard
latency bound is claimed. Disabled controls never sample a clock. Budget-only
entry points preserve their signatures and results; full config literals/exhaustive
Termination matches need to account for the added field/variant.

Six original controlled-clock methods cover all count/cancel/deadline prefix
combinations across pinned/optional basic/list/mixed models, late callbacks,
structural rejection, first-tie retention, disabled/large/backwards clocks, empty
spaces, model setup costs and the final mixed candidate copy. Two original public
regressions check zero-duration precedence and compare disabled/large limits with
budget-only results. One public doctest demonstrates zero time. Existing cancellation
regressions use Default for the added field. No upstream source-method credit,
Java timed-trace equivalence or timing-enabled benchmark claim is added.

Development gate passes 938 Rust tests (481 adapted, 408 original integration,
49 unit), 37 doctests and 83 Python tests. Adapted coverage remains 481/4,855
(9.91%), 291/1,301 streams (22.37%), 4,092 pending and 282 excluded, with no
failed/unverified methods. Logs use work/exact-time-*.log. The full rewrite remains
active: shared stagnation, exact orchestration phases, live changes/daemon/manager,
additional source-method adaptations and other solver/stream gaps remain unfinished.

Release also passes all 938 tests. All nine Java/Rust evidence sections are current
and the live verification fingerprint matches. Existing deadline-disabled exact
medians Rust/Java: 6-variable 0.011/3.881 ms, 8-variable 0.140/5.008 ms. Stream
hill32 45.638/12.854 ms, hill96 168.006/23.188 ms, tabu32 46.797/10.917 ms,
tabu96 164.366/25.495 ms. Both 96-variable stream scores remain infeasible
[-44,-484] at 50 steps. List solving remains 192 valid / 189 equal-score cases;
two-opt16 0.251/7.860 ms and two-opt32 0.892/10.220 ms retain their existing
quality differences. These runs do not measure deadline-enabled overhead or timed
Java parity. The 128 complete-planning comparison cases still agree.

Reused the existing server on 8765. Browser verification at 1280x900 and 390x844
confirmed unchanged 9.91% overall / 22.37% streams, updated exact and stream timing
text, six list/four stream cards and the explicit 8/18 membership boundary
differences. No visible alerts, page errors or horizontal overflow. Browser closed;
upstream pin and checkout remain unchanged.


## Owned score storage — latest increment

Score now requires Clone instead of Copy. Its structural/is_feasible/hard_cmp
methods borrow self; hard_cmp borrows the other score as well. Existing integer
families remain Copy, and ordinary_levels still returns i64. Owned arbitrary-
precision decimal levels are not implemented yet. This removes the storage
restriction needed for them without narrowing the ultimate score-family goal.

Score caches, scoped/stream candidate results, acceptor history, construction
results, phase plans and orchestration now retain explicit cloned values as needed.
Read-only comparisons borrow values, acceptance validate/start borrow config, and
result accessors accept Clone scores. Graph rollback moves the captured original
total back using mem::replace and does not clone in Drop. Scoped moves still
prepare candidate contributions before publishing any cache update. Custom score
clones/comparisons/arithmetic must preserve independent immutable logical values;
shared interior mutation and panicking destructors remain outside guarantees.

Six original tests in owned_scores.rs use a heap-owned two-level integer score
in support/owned_score.rs. The 720-configuration solver matrix compares all six
model/backend entry points against HardSoftScore, including initialized/uninitialized
starts, both mixed construction orders, five acceptance policies, score budgets,
goals, two improvement phases, exact phase reports/work and retained assignments.
All three exact model types are compared over budgets 0..130. Separate tests cover
hard-reset history, annealing, progress, clone fault sweeps for graph/scoped move
preparation and rollback with the next clone armed to fail. A doctest demonstrates
custom owned scores in exact search. No upstream source credit or decimal behavior
is claimed, and no upstream production implementation was inspected.

Development gate passes 944 Rust tests (481 adapted, 414 original integration,
49 unit), 38 doctests and 83 Python tests. Coverage remains 481/4,855 (9.91%),
291/1,301 streams (22.37%), 4,092 pending, 282 excluded, with no failed/unverified
methods. Logs use work/owned-score-*.log. The overall rewrite remains active.

Next decimal work must generalize the i64 ordinary-level contract and the
integer-specific widened fractions/annealing losses before adding actual decimal
score families. Read the pinned *BigDecimalScoreTest methods and probe Community
public APIs for ordering/equality, scale, parsing and rounding; do not infer those
contracts from the existing Decimal collector. Preserve existing integer boundary
behavior and add real solver integration plus independent decimal comparisons.

Specification inspected while verification ran: the complete pinned
core/src/test/java/ai/timefold/solver/core/api/score/SimpleBigDecimalScoreTest.java
contains 19 methods. It explicitly preserves scale for nonzero of(1.0), normalizes
zero text to "0", and requires numeric equality/hash equivalence between -10.0
and -10.000. Its arithmetic examples include multiply/divide/power clamping
negative scale. Do not directly reuse Decimal's scale-sensitive Eq/Hash/Ord for
score equality. The higher-level decimal families and public boundary behavior
still need inspection; no adaptations or method credit were added by reading this.

Final release verification also passes all 944 Rust tests. Regenerated all nine
comparison/benchmark sections after the final source edit; each is current and
the live /api/freshness digest matches the report. Desktop 1280x900 and mobile
390x844 browser checks show all 19 timing cards, 9.91% coverage, no visible alerts,
no page errors and no horizontal overflow. Membership's 8/18 boundary differences
remain visible. The existing dashboard server is still at 127.0.0.1:8765.

Latest warmed Rust/Java median ms: exact 6-variable 0.010/3.612 and 8-variable
0.181/5.551; incremental hill 32-variable 47.604/16.126 and 96-variable
161.285/27.082; incremental tabu 32-variable 45.134/12.023 and 96-variable
165.940/21.205. Both 96-variable stream results remain infeasible at [-44,-484]
after 50 steps. List two-opt 16-customer 0.258/8.043 and 32-customer 0.886/9.422;
respective scores remain Rust [0,-1250]/Java [0,-1254] and Rust [0,-1964]/Java
[0,-2070]. Construction at 32 customers remains Rust [0,-1080]/Java [0,-1062].
These are refreshed shared-machine observations, not a significance, general
speedup, decimal-score performance, or equal-trace claim.


## Numeric score levels — latest increment

Score now declares type Level: ScoreLevel; ordinary_levels returns those values.
Existing fixed/Bendable/owned integer scores use i64. ScoreLevel exposes zero,
numeric comparison, capped progress_fraction and cooling_loss; i64 and Decimal
implement it. Annealing/progress now delegate numeric calculations to the level.
Integer widening/operation order is preserved. Decimal::ZERO is available as an
owned const for additive custom scores. Decimal's representation-sensitive Eq/Ord
are unchanged; scores must use numeric_cmp independently.

Decimal estimates keep an eighteen-digit significand and separate exponent. They
subtract exactly if the scale gap is at most the larger coefficient's decimal
digit count plus 32 (cancellation can matter); otherwise the smaller operand is
negligible below 10^-32 and is omitted. This bounds extra allocation by existing
coefficient sizes rather than an arbitrary scale gap. Zero operands avoid padding.
Split powers preserve representable finite/subnormal ratios near f64 limits.
These are approximate gradients/energies, not correctly-rounded decimal arithmetic
or Java decimal-gradient/annealing parity. Exact score predicates remain exact.

Five new original numeric_score_levels.rs methods compare 42,875 fixed-point
combinations with independent i128 rational arithmetic, plus cooling, thousand-digit
cancellation, alternate scales, i32-extreme scales, subnormals, validation and
exact predicates. A sixth new method in owned_scores.rs runs 2,160 decimal solver
configurations at scales -1000, 3 and 1000 across every model/backend and acceptance
policy, initialized/uninitialized starts, both mixed orders and six budgets. The
fixture is tests/support/decimal_score.rs; its score Eq/Ord use numeric comparison.
Returned scores are independently rescaled using BigInt arithmetic. A new doctest
uses tolerance for decimal progress (0.6 need not be bit-exact).

Development gate passes 950 Rust tests (481 adapted, 420 original integration,
49 unit), 39 doctests and 83 Python tests. No provenance changes or upstream method
credit: coverage remains 481/4,855 (9.91%), streams 291/1,301 (22.37%). Logs use
work/numeric-level-*.log. Overall rewrite is still active.

Next: implement the actual decimal score families with documented native APIs and
pinned source-test adapters, then original Java/Rust/Python decimal comparisons.
SimpleBigDecimalScoreTest (19 methods) was read in the previous increment; this
increment also read all HardSoftBigDecimalScoreTest (20) and
HardMediumSoftBigDecimalScoreTest (21). No production source was inspected.
Both multi-level test classes preserve a zero level's scale in full formatting
when another level is nonzero (e.g. 0.0hard/...); short form omits numeric zero.
Negative and positive nonzero levels preserve scale. Equality/hash are numeric
across trailing zeroes. Feasibility uses structural + hard only (medium can be
negative); arithmetic examples and negative-scale clamping parallel Simple.
The tests do not settle every rounding, parsing, zero-construction or floating
boundary: probe pinned public APIs before choosing behavior. The complete
BendableBigDecimalScoreTest (26 methods) was also read while release verification
ran. It checks HSS and HHSSS shapes, all hard levels independently for feasibility,
empty shape formatting, group-level omission in short form, shape-aware parsing,
numeric equality/hash across scales, division flooring (21/5 -> 4, -21/5 -> -5
at scale zero), and the same negative-scale clamping cases. Adapt shape definitions
using native const dimensions as with integer BendableScore; do not inspect the
production definition implementation. This increment claims no BigDecimal method pass.

Final release gate passes all 950 tests. All nine evidence sections were regenerated
from the final sources and are current; /api/freshness matches the report digest.
Desktop 1280x900/mobile 390x844 checks confirm 19 benchmark cards, current timings,
9.91% coverage, explicit infeasible stream scores and 8/18 membership differences.
No visible alerts, page errors or horizontal overflow. Browser closed; existing
127.0.0.1:8765 dashboard server reused. Upstream pin and checkout unchanged.

Latest warmed Rust/Java median ms: exact 6-variable 0.009/3.948 and 8-variable
0.131/5.955; stream hill 32-variable 45.427/15.772, 96-variable 168.711/27.327;
stream tabu 32-variable 45.968/14.205, 96-variable 164.334/25.886. Both 96-variable
stream scores remain infeasible [-44,-484] after 50 steps. List two-opt 16-customer
0.276/7.578, 32-customer 0.951/9.465; scores still Rust [0,-1250]/Java [0,-1254]
and Rust [0,-1964]/Java [0,-2070]. List construction at 32 customers still differs:
Rust [0,-1080], Java [0,-1062]. These timings use integer score workloads and do not
measure decimal performance or establish a significant regression/improvement.


## Fixed decimal score families — latest increment

Added SimpleDecimalScore, HardSoftDecimalScore and HardMediumSoftDecimalScore with
arbitrary-precision Decimal levels, Score/AdditiveScore, numeric equality/order/hash,
factory helpers, raw from_parts, structural state, ZERO/default, parsing and full/
short formatting. Decimal gains FromStr and to_standard_string while its existing
coefficient-e Display and representation-sensitive equality/hash remain unchanged.
Native multiply/divide take exact &Decimal factors and power takes u32; these are
explicit native APIs, not Java double-input parity. They floor at each original
scale clamped to zero and return Result. Ordinary arithmetic clears structural
state; all-zero factory/arithmetic output normalizes to ZERO. Raw construction or
an explicit structural parse prefix preserves every scale. Negation keeps a sound
numeric zero's representation, otherwise subtracts from ZERO (negative scales rise
to zero); abs retains scales before zero normalization. Scale shifts beyond u32
report ScaleOverflow; arbitrary coefficient/padding growth otherwise has no fixed
memory/time bound. See docs/score-contract.md.

Public API probes inspected only javap -public signatures and invoked the pinned
Community artifact. Ignored work/DecimalScoreProbe.java, DecimalPartsProbe.java and
work/decimal-score-probes.txt retain original observations; no implementation source
was inspected. Important behavior includes explicit 0structural/0.00 preserving
scale, factory 0.00 becoming 0, numeric equality/hash across scale, negative-scale
negation expanding to scale zero and structurally sound raw zero negation retaining
its representation. Generated comparisons caught and corrected the negation rule;
Python's signed decimal zero needed normalization to match both native/Java values.

scripts/adapt_decimal_score_tests.py reads only the three fixed pinned test classes
and regenerates 19+20+21 methods, retaining all assertions and provenance/notices.
Exact native Decimal literals replace tested Java double factors, tested integral
double powers become u32, and from_parts preserves raw constructor arguments.
Two further BestScoreTerminationTest decimal gradient methods preserve all six
start/target/current observations per method, original split weights and offsets.
The initial original numeric estimate failed the source's exact 0.6 assertion;
folding a negative exponent into a finite denominator removes the avoidable extra
rounding, with the previous exponent-separated fallback at extremes. Their tests
first failed (work/decimal-gradient-before.log), then passed unchanged. No Java
lifecycle or universal correctly-rounded gradient claim.

Six original decimal_score_regression methods cover parsing/formatting at full i32
scale bounds, factory/raw zeros, numeric hash normalization beyond i32, exact huge
coefficients, floors/errors, negation scales and fractional exact-search ordering.
The 2,160-configuration solver matrix in owned_scores.rs now uses the actual public
HardSoftDecimalScore. The custom DecimalScore fixture remains in numeric level
regressions. One new API doctest demonstrates floor rounding and numeric equality.

Original three-way comparisons (seed 20261212) cover 3,390 decimal cases, including
86 expected errors, and compare exact coefficients/scales, structural/full/short
text, feasibility, equality, ordering and arithmetic. Java uses public score APIs;
Python independently uses Fraction arithmetic and tuple-based decimal formatting.
Finite factors are -2,0,0.5,1.2,2,5,1.25; powers 0..4. DecimalScoreOracle.java and the
native decimal_score_oracle example implement the protocol. The new script's
result_matches rejects unexpected all-ERROR agreement; three Python unit methods
check the independent reference/error validation. differential_scores.py includes
its record as decimal_checks in the existing score comparison section; dashboard
adds a separate decimal evidence row and states factor/power/timing limitations.
No decimal benchmark claim or additional method credit from generated cases.

Final development check passes 1,018 Rust tests (543 adapted, 426 original integration,
49 unit), 40 doctests and 86 Python tests. Coverage is 543/4,855 (11.18%), with 4,030
pending, 282 excluded and no failed/unverified methods; score family coverage is
134/230, solver lifecycle 20/379, streams remain 291/1,301. Logs use
work/decimal-score-*.log and work/decimal-gradient-*.log. Overall rewrite remains active.

Next: implement BendableDecimalScore with native const dimensions and adapt all 26
BendableBigDecimalScoreTest methods (complete test file was read in the previous
increment), then extend the original three-way decimal comparison matrix across
HSS/HHSSS and empty shapes. Preserve public dimensional errors and per-level floor/
formatting/hash behavior. Remaining broader work still includes shared stagnation,
exact orchestration phases, live problem changes/manager/daemon, remaining streams
and source method adaptations. Do not mistake fixed-score completion for the whole goal.

Final release verification also passes all 1,018 Rust tests. All nine report
sections were regenerated from the final sources; all are current and the live
/api/freshness digest matches. The decimal detail is 3,390/3,390 with 86 expected
rejections. Desktop 1280x900 and mobile 390x844 show 11.18% overall, score APIs
58.26% (134/230), lifecycle 5.28% (20/379), all 19 timing cards and the separate
decimal evidence/scope text. Selecting score APIs and filtering
SimpleBigDecimalScoreTest shows exactly its 19 passed methods. No visible alerts,
page errors or horizontal overflow. Browser closed; existing server at
127.0.0.1:8765 reused. Upstream pin and checkout remain unchanged.

Latest warmed Rust/Java median ms: exact 6-variable 0.012/3.588, 8-variable
0.158/4.846; stream hill 32-variable 45.011/12.897, 96-variable 169.380/25.985;
stream tabu 32-variable 47.311/15.292, 96-variable 161.899/27.509. Both 96-variable
stream results remain infeasible [-44,-484] after 50 steps. List two-opt 16-customer
0.271/8.130, 32-customer 0.900/11.132, with unchanged differing scores
Rust [0,-1250]/Java [0,-1254] and Rust [0,-1964]/Java [0,-2070]. List construction
32-customer scores remain Rust [0,-1080]/Java [0,-1062]. Timings still use integer
scores and do not establish decimal performance or statistically significant change.
