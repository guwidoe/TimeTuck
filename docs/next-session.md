# TimeTuck implementation handoff

## Active user goal

Rewrite Timefold behavior in independent native Rust, using pinned Community tests
as specifications. Also maintain a visual dashboard of coverage by subsystem and
performance versus Java. The whole rewrite remains unfinished; do not mark the
active goal complete after one slice. No zero-mistake guarantee is possible.

## Current implementation

- Score levels support i64 and Decimal through ScoreLevel, including decimal
  progress and annealing estimates. SimpleDecimalScore, HardSoftDecimalScore and
  HardMediumSoftDecimalScore implement fixed decimal families; BendableDecimalScore<H,S>
  adds multiple hard/soft levels and empty groups.
- Rust library has SimpleScore, HardSoftScore, HardMediumSoftScore and
  BendableScore<H,S> with structural
  values, ordering/feasibility, parsing/formatting, arithmetic and checked arithmetic.
- 723 fully adapted source test methods, recorded individually in
  `compat/provenance.json`. See the latest verified increment below for counts.
- Finite domains, initial values, pinning, optional values and reversible
  change/swap/composite transactions. Read `docs/planning-contract.md`.
- Ordered SolverPlan/MixedSolverPlan mix repeated construction, custom, exact and local phases in all six solver entry points,
  sharing budgets, cancellation, time/stagnation and goals. Exact phases can finish
  partial construction; Optimal requires full enumeration and a complete scored result.
  See docs/solver-contract.md and the latest increment below.
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
- GreatDeluge supports exact fixed/rational rain with strict current-score aspiration.
  Three pinned methods are adapted; 275/276 original search cases agree with Java
  and all 276 with exact Python. One ratio boundary divergence remains explicit.
- StepCountingHillClimbing supports all five counting modes with a positive u64 period,
  fixed per-scan thresholds and phase-local state. Four pinned methods are adapted;
  600 original public-Java/native/Python cases check final assignments and scores.
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
- 577 original integration regression methods, 75 original unit tests, and 57 public API documentation tests.
- Native solve/solve_list/solve_mixed and stream counterparts compose construction
  and ordered local-search phases under shared score/candidate budgets, cancellation
  and score/feasibility goals. Results retain complete best or partial initialization
  and per-phase work. Shared elapsed time now spans construction, improvement and
  stream graph preparation. Construction also has a phase-local max_time control.
  Read `docs/solver-contract.md`; optional shared unimproved time and step limits
  span local-search phases, while stream graphs are rebuilt per phase.
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
  expanded runtime JUnit invocations. 594 passed; 282 JVM integration exclusions.
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
5. Extend remaining score/API and lifecycle coverage from tests and public
   documentation only; fixed and Bendable decimal families now have adaptations.
   Keep generated arithmetic cases separate from upstream method coverage.

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


## Bendable decimal score families — latest increment

Added BendableDecimalScore<H,S> with owned Decimal arrays, numeric Eq/Ord/Hash,
Score/AdditiveScore, const dimensions, all-hard-level feasibility, checked indexes,
full/short formatting and exact arithmetic. Unlike fixed decimal families, zero
scales survive construction and arithmetic; negation preserves negative scales.
All arithmetic clears structural state. Empty dimensions are supported, including
successful zero division for (0,0), which performs no level operations. Read
score-contract.md for scale, factor, structural and resource-limit details.

Only pinned Community tests and public API signatures/execution were used. Original
ignored work/BendableDecimalProbe.java and work/bendable-decimal-probes.txt retain
observations. No production implementation source was read or ported.

scripts/adapt_bendable_decimal_tests.py adapts all 26 BendableBigDecimalScoreTest
methods with provenance/notices. Native dimensions replace definitions; exact
Decimal factors and u32 powers preserve tested literals. Source helpers for
feasibility/equality/hash/order remain represented in native assertions.
Five original bendable_decimal_regression methods cover scale/empty/error/hash
boundaries, multi-hard-level feasibility/history/annealing and exact fractional
ordering. One additional owned_scores test compares 2,160 configurations with
integer Bendable scores across all six solver backends, five acceptance policies,
budgets, initialization states, mixed orders and scales -1000,3,1000.

Extended the existing Java/Rust/Python decimal comparison (nested in differential,
not a tenth report section). The fixed-family corpus remains 3,390 cases at seed
20261212; 4,780 new cases at seed 20261214 cover shapes (1,2),(2,3),(0,0),(0,2),(2,0).
The total is 8,170 cases with 225 explicit expected errors. Java checks dimensions
through public accessors; Python independently uses Fraction arithmetic and
scale-aware formatting. The dashboard evidence row now includes Bendable scope.
No decimal timing/general-f64-input claim; generated cases do not add method credit.

Next work: remaining solver orchestration/lifecycle (shared stagnation and exact
phases, live problem changes/manager semantics), remaining stream operators and
upstream behavioral adaptations. Do not mistake complete decimal score test
classes for complete score subsystem or whole-solver parity. Native API/ABI and
floating factor differences remain documented.

Verification for this increment: bash scripts/check.sh passes 1,050 Rust tests
(569 adapted, 432 original integration, 49 unit), 42 doctests and 87 Python tests.
All 1,050 release tests pass. Coverage is 569/4,855 (11.72%), pending 4,004,
excluded 282, failed/unverified zero. Score APIs: 160/230 (69.57%). All nine
correctness/performance sections were regenerated and are current; the live
/api/freshness verification_digest matches the report. Logs use
work/bendable-decimal-*.log. The rewrite remains active and incomplete.

Browser verification at 1280x900 and 390x844 confirms overall coverage, all 19
benchmark cards, the 8,170-case decimal evidence and Bendable scope. Selecting
score APIs and filtering BendableBigDecimalScoreTest shows exactly 26 passed
methods. No page errors, visible alerts or horizontal overflow; browser closed.

Latest warmed Rust/Java median ms: exact6 0.013/5.049, exact8 0.161/6.271;
stream hill32 59.143/18.126, hill96 232.292/37.930, tabu32 67.845/14.233,
tabu96 192.042/45.318. Both 96-variable stream scores remain [-44,-484],
infeasible under the 50-step cap. List two-opt16 0.402/11.358, two-opt32
1.138/14.382, with Rust/Java scores [0,-1250]/[0,-1254] and
[0,-1964]/[0,-2070]. Construction32 retains [0,-1080]/[0,-1062]. Quality gaps
and observed timing ranges remain explicit; these workloads use integer scores
and do not establish decimal performance or statistically significant change.


## Solve-wide unimproved time — latest increment

SolverLimits::max_unimproved_time and SolverTermination::UnimprovedTimeLimit add a
shared native stagnation timer across all six solver entry points. A private
SearchStagnation observer connects the existing three search engines' TimeBudget
start/improvement/poll events to SolveTime. Public standalone search signatures
remain unchanged; private monitored wrappers preserve their validation paths.
No thread-local state or unsafe code. Disabled observers do not sample a shared clock.

Construction and first search preparation/initialization are excluded. Start after
first initial scoring/diagnostics; later phase factories, handoffs, initialization
and diagnostics count. Strict retained-best improvement resets before expiry;
accepted ties/worsening/recovery, new phases and late improvements cannot reset it.
Expiry latches and backwards samples clamp. Phase-local timers remain independent.
Existing score/candidate/cancel precedence is preserved, followed by shared elapsed
time, shared stagnation, then goals. Original phase reports retain their own reason.
See solver-contract.md for cooperative limits and edge cases.

Three original integration methods in tests/solver.rs cover all six public entry
points, both initial states/orders, zero stagnation, construction-only/incomplete
plans, pre-search goals, invalid future phases and zero evaluation budgets. Six
new controlled-clock unit methods in solver_time_tests.rs run the actual stream
construction/search via run_with_clock and test shared clock invariants. They
cover cross-phase time consumption, reset/late improvement, first/later preparation,
equal-score steps, competing local/global/time/goal controls and preserved reports.
The previous phase-time tests now also use the observer-enabled internal path.

Read the complete pinned UnimprovedTimeMillisSpentTerminationTest.java as behavioral
context only. No adapted/copied tests or fixtures in this increment; provenance is
unchanged and its six methods remain pending. This native feature adds no Java
timed-trace or whole lifecycle equivalence claim. Next: shared unimproved-step
limits, orchestrated exact phases and remaining lifecycle/problem-change support.

Development gate and release verification pass 1,059 Rust tests: 569 adapted,
435 original integration and 55 unit. All 42 doctests and 87 Python tests pass.
Coverage stays 569/4,855 (11.72%), pending 4,004, excluded 282, failed/unverified
zero; native regression count is deliberately not upstream method coverage.

All nine correctness/performance sections were refreshed; all are current and the
live /api/freshness verification_digest matches the report. The old dashboard
process had stopped (no listener on 8765); restarted scripts/dashboard_server.py,
logging to work/dashboard-server.log. Browser verification then confirmed 11.72%,
569/4,855, all 19 benchmark cards, the 8,170-case decimal evidence, no stale-state
warnings, alerts, page errors or horizontal overflow. Browser closed; server left
running. Logs for this increment use work/shared-stagnation-*.log.

Latest warmed Rust/Java median ms: exact6 0.015/6.829, exact8 0.156/9.742;
stream hill32 64.415/30.009, hill96 232.784/38.956, tabu32 65.971/32.898,
tabu96 310.637/47.510. Both 96-variable stream scores remain [-44,-484],
infeasible under the cap. List two-opt16 0.403/19.237 and two-opt32 1.590/22.719;
quality differences remain Rust/Java [0,-1250]/[0,-1254] and
[0,-1964]/[0,-2070]. Construction32 remains [0,-1080]/[0,-1062]. These warmed
workloads retain fixed step/evaluation limits and do not benchmark the new timed
termination or establish statistically significant performance changes.


## Solve-wide unimproved steps — latest increment

Added SolverLimits::max_unimproved_steps and SolverTermination::UnimprovedStepLimit.
Private SearchStagnation becomes SearchControl and SolveTime becomes SolveControl,
with a shared StepProgress, started flag and latched step expiry. All three search
engines report complete_step(improved) at the same point as their existing local
StepProgress updates. Shared control spans all six solve entry points; public
standalone-search signatures remain unchanged. Step-only controls never read clocks.

Construction and initialization do not count. Every selected move counts, including
rejected fallback. Strict retained-best improvement resets; ties/worsening/recovery
and later phases do not. No-selected-move phases consume no steps. Per-phase step
limits remain independent. A zero shared cap allows construction and first search
initialization. Existing reported score/candidate/cancel precedence is followed by
shared elapsed time, shared unimproved time, shared unimproved steps, then goals.
Phase reports preserve their own reason; phase-local timers precede shared observer
reasons during search. See solver-contract.md for exact scope.

Six new original integration methods in tests/solver.rs use a generalized score
fixture to test plateaus, fallback/recovery, no-move phases, local/global caps,
construction/early goals and precedence across all six backends. The plateau matrix
covers caps 0..5 and phase sizes 1,2,5 and independently checks completed/accepted
steps, scores, completeness and aggregate phase accounting. Two new internal tests
cover no-clock operation, cross-phase reset/latching and time/step reason scope.
Existing tests remain intact through the fixture's original identity-score wrapper.

Read pinned UnimprovedStepCountTerminationTest.java; its existing adaptation remains
unchanged. No new copied/adapted tests, fixtures or provenance entries. Native
regressions add no source-method credit or Java whole-solve trace equivalence.
Next: orchestrated exact phases and remaining problem-change/lifecycle support.

Development gate and release tests pass 1,067 Rust tests: 569 adapted, 441 original
integration and 57 unit. All 42 doctests and 87 Python tests pass. Coverage remains
569/4,855 (11.72%), pending 4,004, excluded 282, failed/unverified zero. Logs for
this increment use work/shared-steps-*.log. The full solver rewrite remains active.

All nine correctness/performance sections were refreshed and are current. The live
/api/freshness verification_digest matches the generated report. Browser check
confirms 11.72%, 569/4,855, all 19 benchmark cards, decimal 8,170/8,170, and no
stale-state warnings, visible alerts, page errors or horizontal overflow. Browser
closed; the existing dashboard server remains running.

Latest warmed Rust/Java median ms: exact6 0.010/3.910, exact8 0.157/5.048;
stream hill32 49.125/17.423, hill96 177.256/30.316, tabu32 47.049/16.319,
tabu96 171.354/25.181. Both 96-variable stream results remain [-44,-484],
infeasible under the fixed cap. List two-opt16 0.258/8.921, two-opt32 0.964/11.380;
Rust/Java quality differences remain [0,-1250]/[0,-1254] and [0,-1964]/[0,-2070].
Construction32 remains [0,-1080]/[0,-1062]. These fixed-workload timings are not
benchmarks of shared stagnation termination or evidence of statistically significant
speed changes. The full rewrite remains unfinished; keep the goal active.


## Ordered exact solver phases — latest increment

Added SolverPlan, MixedSolverPlan, SolverPhase, MixedSolverPhase, ExactPhaseConfig
and ExactPhaseTermination in solver_plan.rs. All six solve entry points accept
impl Into<...Plan>; old SolverConfig/MixedSolverConfig convert without behavior
changes. Construction is still phase zero. Arbitrary exact/local improvement orders
retain the earlier complete best on ties and preserve each mixed local joint flag.
SolverResult::is_optimal and SolverTermination::Optimal require exact exhaustion
plus a complete scored result. Exhaustion stops future phases/factories immediately.

solver_exact.rs has a shared complete-candidate runner over the existing native
basic/list enumerators and their mixed product. Scoped candidates calculate the full
network score. Stream candidates fill empty planning sources in a transaction and
drop it after scoring, preserving immutable facts and restoring all planning/shadow
rows. Graph validation helpers are now pub(crate). Candidate errors identify the
phase. There is one graph per started phase, not per candidate.

Each exact assignment consumes one evaluation and one candidate, no initialization
rescore and no completed local steps. Shared and local caps/tokens/goals/timing are
combined. Budget, candidate, cancellation, timer, score-goal and feasibility polls
precede each complete candidate; final exhaustion wins over controls reached in the
final callback. Enumeration and callbacks remain cooperative, not preemptive.
Exact starts shared stagnation if no earlier improvement phase did. Strict exact
improvements reset shared time and the private StepProgress baseline without adding
steps; expired controls cannot revive. Construction remains outside stagnation.

An incomplete construction may proceed to an immediately following exact phase if
shared resources remain. Zero-work exact phases can hand partial progress to another
exact phase. Local search still requires complete input. A first complete exact
candidate supersedes partial progress. No bounded prefix claims optimality, including
prefixes whose retained incumbent happens to be optimal.

Fifteen original integration methods in tests/support/solver_exact_cases.rs reuse
the existing six-backend fixture (now accepting Into<SolverPlan>). They cover exact/
local ordering, shared/local cap matrices, partial construction, retained incumbents,
ties, cancellation including the final callback, shared/phase goals, zero timing/
stagnation, skipped-plan validation/factory failures, empty/impossible models,
source validation, owned decimal scores/pins/optional values, and escape from a local
optimum checked against independent enumeration. Five new unit methods cover simulated
deadlines/factories, stagnation/handoffs/final exhaustion, step resets without added
steps, list/mixed shadow/fact rollback and basic graph errors. Existing local-only
time tests now use the same ordered engine; no sleeps or upstream production source.

The development gate passes 1,087 Rust tests: 569 adapted, 456 original integration
and 62 unit. All 43 doctests and 87 Python tests pass. Coverage remains 569/4,855
(11.72%), pending 4,004, excluded 282, failed/unverified zero. No new adapted/copied
cases or provenance changes; native orchestration tests add no source-method credit.
Logs use work/exact-phase-*.log. The full rewrite remains active and unfinished.
Next work: remaining lifecycle/problem-change support and pending source semantics;
existing benchmarks do not measure the new orchestrated exact phases.

Release verification also passes all 1,087 Rust tests. All nine correctness and
performance sections were regenerated and report current; the live /api/freshness
verification_digest matches the report. Browser verification confirms 11.72%,
569/4,855, all 19 benchmark cards, 8,170 decimal comparisons, no stale-result messages,
page errors or horizontal overflow. Browser closed; dashboard server remains running.

Latest warmed Rust/Java median ms: exact6 0.011/4.679, exact8 0.171/5.794;
stream hill32 48.532/15.755, hill96 174.222/27.893, tabu32 45.217/14.578,
tabu96 177.437/23.971. Both 96-variable stream results remain [-44,-484],
infeasible under their fixed cap. List two-opt16 0.388/15.487, two-opt32 1.426/22.917;
Rust/Java scores remain [0,-1250]/[0,-1254] and [0,-1964]/[0,-2070].
Construction32 remains [0,-1080]/[0,-1062]. These fixed-workload observations do
not measure the new exact-phase orchestration or establish significant speed changes.


## Step-counting hill climbing — latest increment

Added step_counting.rs with StepCountingMode (Step, EqualOrImprovingStep,
ImprovingStep, SelectedMove, AcceptedMove) and StepCountingHillClimbing<S>.
Acceptance::StepCountingHillClimbing { period, mode } integrates through the shared
Acceptor, so all basic/list/mixed scoped/stream searches and solver plans support it.
Candidate acceptance is candidate >= current OR candidate >= threshold. Thresholds
start at the phase initial score. At a completed step reaching the period, install
that step's score and reset the count; excess trials never carry into the next
interval. No threshold changes mid-scan. Qualifying steps compare with the previous
step rather than global best; trial modes count every check or every accepted check,
including unselected/repeated trials. count() saturates at the period, so u64::MAX
needs no special allocation or arithmetic. Zero period validates before any work.
Owned decimal and multi-level scores retain native ordering and cloned state.

Pinned StepCountingHillClimbingAcceptorTest was read in full; four full methods are
adapted in step_counting_hill_climbing.rs, preserving all 109 acceptance assertions
and three six-step sequences, plus zero-size rejection. Provenance and notices include
AbstractAcceptorTest's initialized score fixture. negativeStepCountingHillClimbingSize
remains pending because native periods use u64. No production implementation source
was inspected/copied. Public 2.7.0 javap signatures verified all five config modes.
Timefold's public local-search documentation supplies the non-worsening acceptance
path and threshold description; see incremental-search-contract.md.

Six original standalone regressions cover trial counters/deferred threshold changes,
no excess carry, rejected fallback recovery, previous-step counting, maximum periods,
owned decimal ordering, independent cloned/fresh sessions and invalid configuration.
Two original solver regressions exercise all six model/backend paths, both selection
policies, periods 1/2/7 and two-phase handoffs, including validation of a phase that an
exact proof would otherwise skip. Existing search engines need no new per-model logic.

Original StepCountingOracle.java uses only public Community solver configuration
APIs. step_counting_oracle.rs runs scoped and stream backends. step_counting_checks.py
uses seed 20261218 and independent Python neighbor enumeration for 600 cases covering
five modes, periods 1/2/7, first/best accepted selection, fallback, plateaus, pins and
zero steps. All final Java/Rust assignments and scores match; native step/acceptance/
stagnation counters and reasons match Python across 6,180 reference steps. Java
internal counters/traces and list/mixed differential equivalence are not claimed.
Four Python tests check the reference, counter boundaries, reproducibility and
corrupted response rejection. New checks are nested under solver_differential as
step_counting_checks and surfaced in the dashboard. Failures fail the parent command.

HeuristicBenchmark and heuristic_benchmark now accept the internal `step` algorithm,
using STEP mode and period 7. benchmark_heuristics.py adds 32/96-variable full-scoring
workloads with the existing 50-step cap and sample protocol, expanding its small
validation to 96 cases and the dashboard from 19 to 21 timing cards. Only this counting
mode is timed; no incremental step-counting or general scalability claim.

Development gate passes 1,099 Rust tests: 573 adapted, 464 original integration and
62 unit. All 44 doctests and 91 Python tests pass. Coverage is 573/4,855 (11.80%),
pending 4,000, excluded 282, failed/unverified zero. Search algorithms reach 19/158
(12.03%). Java/Rust/Python comparisons add no additional source-method credit.
Logs use work/step-counting-*.log. The full rewrite remains active and incomplete.
Next gaps include lifecycle/problem-change support and the remaining search/stream
semantics; do not treat this acceptance increment as completion of the rewrite.

Release tests also pass all 1,099 cases. All nine correctness/performance sections
were refreshed and are current; /api/freshness matches the report verification_digest.
The 600-case step-counting check passes again within the parent solver comparison.
Heuristic validation reports 96/96 valid and equal-score cases. Browser confirms
11.80% overall, search 12.03%, all 21 timing cards, the 600/600 evidence row, no stale
messages or page errors, and no horizontal overflow at desktop or 390px width.
Browser closed; existing dashboard server remains running.

New STEP/period-7 warmed full-scoring median Rust/Java ms: 32 variables 2.100/8.246,
96 variables 13.807/15.312. Both return [0,0] at 32 and [-44,-22] at 96; the latter
is infeasible under the fixed 50-step cap. Existing warmed medians: exact6
0.011/4.329, exact8 0.137/5.665; incremental stream hill32 46.566/12.815,
hill96 176.633/26.188, tabu32 48.724/15.190, tabu96 179.627/25.831. Both 96-variable
stream outcomes remain [-44,-484]. List two-opt16 0.333/9.382 and two-opt32
0.906/12.497 retain Rust/Java quality differences [0,-1250]/[0,-1254] and
[0,-1964]/[0,-2070]; construction32 remains [0,-1080]/[0,-1062]. These warmed
fixed-workload observations do not prove general speedups or significant changes.


## Great Deluge — latest increment

Added GreatDeluge<S>, GreatDelugeRise<S> and Acceptance::GreatDeluge. Fixed rain
adds a nonnegative score; ratio rain uses positive u64 numerator/denominator and
exact accumulated fractions of the absolute initial water components. Thresholds
are unbounded Decimal values (integer scale zero, decimal initial scale clamped
to at least zero), preventing integer overflow and per-drop rounding loss. Water
starts at the phase initial score or explicit initial_water_level, rises after
every completed step including fallback, and stays fixed during trials. Acceptance
is candidate >= water OR candidate > current; ties below water are rejected.
Sessions bind S and clone/reset independently. All six model/backend paths and
ordered phase plans use the existing shared Acceptor integration.

ScoreLevel::exact_decimal is an optional capability, implemented by i64/Decimal.
Custom types default to None and retain other search policies; Great Deluge rejects
them before work. Existing constant score dimensions and precision contracts apply.
Native exact arithmetic is deliberate; no general Java floating/overflow parity.
Public 2.7.0 config signatures were checked with javap -public; no production source
was inspected. The pinned GreatDelugeAcceptorTest was read in full and all three
methods are adapted with 36 acceptance assertions and full provenance/notices.
Source final-step scope typos are documented; threshold advances do not depend on
the selected score. Eight original boundary tests and two six-backend integration
tests cover exact arithmetic, precision, aspiration, resets, invalid parameters,
custom unsupported levels, fallback and skipped-phase validation.

GreatDelugeOracle.java uses only public Community configuration and solve APIs.
great_deluge_oracle.rs covers scoped/stream. great_deluge_checks.py uses seed
20261220 and independent Python batch-neighbor enumeration: 276 cases with three
fixed increments, three rational increments, both selections, pins, zero steps,
plateaus, fallback, negative/positive water. All 276 native results/counters/reasons
match exact Python. Java assignments/scores agree in 275. The explicit remaining
case random-9-ratio-1-10-first returns Java score [0,1], values [2,1,0], versus
native [0,9], [2,2,0] at 12 steps. Frozen 1..25-step prefix outputs confirm that
Rust reaches [0,9] at step 11 and Java at 13 (23 agreements, two divergences).
A separate cumulative binary-floating ratio hypothesis reproduces those final
outputs; Java internal threshold/trace behavior was not inspected or claimed.
Both native backends run every prefix too; combined reference steps are 3,109.
Known differences are verified against frozen observations and never included in
agreement passes. Any other/changed result fails the parent differential command.
Four Python tests protect reference semantics, deterministic generation, response
validation and the arithmetic boundary. Dashboard exposes all counts and limits.

Full-scoring benchmark adds internal algorithm deluge with fixed (0hard/1soft)
rain, original ordering, best-accepted selection, first ties, best-rejected fallback
and the existing 50-step budget. Both Java and Rust share this configuration.
Adds 32/96-variable workloads and expands small independent validation to 128 cases;
ratio rain and incremental/decimal Great Deluge are not timed.

Development gate passes 1,112 Rust tests: 576 adapted, 474 original integration,
62 unit. All 45 doctests and 95 Python tests pass. Coverage is 576/4,855 (11.86%),
pending 3,997, excluded 282, failed/unverified zero. Search is 22/158 (13.92%).
Logs use work/great-deluge-*.log. The full rewrite remains active and incomplete;
remaining lifecycle/problem-change support and search/stream semantics are next gaps.

Release also passes all 1,112 Rust tests. All nine comparison/performance sections
were refreshed and are current; live /api/freshness matches verification_digest.
Great Deluge again reports 275/276 Java agreements, 276 native reference passes,
one explicit divergence, and the 25-prefix probe's 23 agreements/two divergences.
Heuristic small-case validation is 128/128 valid and equal-score cases; both
engines reach 106 independently enumerated optima. Browser verification confirms
11.86% overall, 13.92% search, all three new method rows, 23 timing cards and the
275/276 evidence with divergence explanation. No stale messages, browser errors
or horizontal overflow at 1280px and 390px. Browser closed; server remains running.

New fixed-rain Great Deluge warmed Rust/Java median ms: 32 variables 2.573/9.995,
96 variables 16.098/18.226. Both return [0,0] for 32 and [-44,-22] for 96; the latter
remains infeasible under the fixed 50-step cap. Small validation uses the same
fixed-rain configuration. Existing exact6/8 medians are 0.015/5.319 and 0.356/6.319;
stream tabu32/96 are 80.471/32.443 and 277.811/46.439, with scores [0,0] and
[-44,-484] respectively. List two-opt16/32 are 0.417/16.462 and 1.160/18.457;
quality differences remain [0,-1250]/[0,-1254] and [0,-1964]/[0,-2070]. Another
project's CPU-intensive build was observed during this refresh. These shared-machine
measurements and broad ranges do not establish significant speed changes or general
solver superiority. No unrelated processes were interrupted.


## Asynchronous solver manager — latest increment

Added SolverManager<R,E> with an explicitly sized native worker pool, FIFO queue,
u64 problem IDs, duplicate live-ID validation, Scheduled/Active/NotSolving states,
SolverJob handles and shared final results without Clone bounds on R/E. submit
accepts an owned Send closure with an independent CancellationToken. Build the
actual solver/network inside the worker and pass that token into native configs;
worker-local scoring closures need not be Send. R/E must be Send + Sync. Completion
publishes status/result and removes the ID atomically under queue -> job lock order.
Completed IDs can be reused; old handles cannot cancel replacement runs. Finished
results are retained only by their handles, not a growing manager registry.

Queued cancellation removes work and returns CancelledBeforeStart without executing
its closure. Active cancellation is nonblocking/cooperative; status stays Active
until the function returns, preserving existing solver best-prefix semantics.
request_shutdown closes submissions, cancels the queue and requests active stops.
shutdown additionally joins workers; own-worker shutdown returns WouldBlock rather
than self-joining. Drop requests shutdown and detaches workers. A function ignoring
its token can delay explicit shutdown indefinitely. Unbounded FIFO capacity and
explicit worker count are documented; no AUTO count or queue backpressure yet.

Solver errors and unwinding panics become job results and leave the worker usable.
Captured queued inputs are destroyed outside locks, permitting reentrancy. Worker
cleanup catches panics from abandoned result destructors. Destroying a caught panic
payload is guarded; a secondary destructor-panic payload is deliberately forgotten
to avoid recursive panic destruction. Abort/OOM and double-panic unwinding are not
recoverable. Standard panic hooks are retained. No unsafe code or new dependencies.

Pinned SolverManagerTest was inspected for parallel solve/status/duplicate-ID and
cancellation behavior, with public Community library-integration documentation.
Three complete methods are adapted in solver_manager_cases.rs: solveBatch_2InParallel,
getSolverStatus, runSameIdProcesses_throwsIllegalStateException. Provenance/notices
record its barrier helper and PlannerTestUtils/PlannerAssert fixtures (two required,
unassigned variables with values 1/2; initialized result assertions). Native zero
objective is original fixture scoring; no source score/trace equivalence. Java's
blocking terminateEarly and returning input for scheduled cancellation differ from
the native contract; their methods remain pending. No production source inspected.

Twelve original standalone regression methods cover queued cancellation/ID reuse,
active cancellation retaining an actual exact-search result, typed errors and
string/non-string panics, many waiters, non-Clone results, timeout semantics,
shutdown/drop/self-join, FIFO and simultaneous producers with the concurrency cap,
payload/result destructor panics, reentrant captured-input destruction and result
release. Two integration methods run all six actual solver paths, including
worker-local Rc<Cell<_>> scoring and cancellation returning valid retained state.
Backend comparisons deliberately exclude constraint-callback counts, which differ
between scoped and stream scoring; assignments, score, work counts and reasons agree.
The original batch_schedules example queues eight exact six-variable problems onto
two workers; each proves 0hard/-3soft over 729 candidates and shuts down explicitly.

The manager is a usable asynchronous execution layer, not complete lifecycle parity.
Callbacks, intermediate best events, problem changes, live statistics and automatic
CPU sizing remain pending. Existing timing cards do not measure manager throughput.

Development gate passes 1,129 Rust tests: 579 adapted, 488 original integration and
62 unit. All 46 doctests and 95 Python tests pass. Coverage is 579/4,855 (11.93%),
pending 3,994, excluded 282, failed/unverified zero; lifecycle reaches 23/379
(6.07%). The batch example independently confirms eight optimal results. Logs use
work/manager-*.log. Full rewrite remains active and incomplete; subsequent work
should fill the remaining event/problem-change lifecycle and algorithm semantics.

Release verification also passes all 1,129 tests and the eight-job batch example.
All nine comparison/performance sections were regenerated and are current; the
live /api/freshness verification_digest matches the report. Existing Great Deluge
agreement remains 275/276 with the same explicit ratio divergence; heuristic small
validation remains 128/128 valid and equal-score cases. Browser confirms 11.93%,
all three new manager rows marked Passed, lifecycle 6.07%, 23 timing cards, no
stale messages or errors, and no overflow at 1280px/390px. Browser closed; the
existing dashboard server remains running.

Current warmed Rust/Java medians in ms: exact6 0.012/4.628, exact8 0.151/6.270;
Great Deluge fixed-rain32 2.629/9.962, 96 15.269/17.210 (equal scores [0,0] and
[-44,-22], latter infeasible under the cap). Stream tabu96 is 173.986/31.960,
score [-44,-484] in both. List two-opt16 0.348/11.152 and 32 0.892/11.795 retain
quality differences [0,-1250]/[0,-1254] and [0,-1964]/[0,-2070]. Construction32
remains [0,-1080]/[0,-1062]. These existing workloads do not measure manager
parallel throughput or establish significant changes between shared-machine runs.


## Manager callbacks — latest increment

Added SolverJobCallbacks<R,E> with on_started/on_final/on_error, submitted through
submit_with_callbacks. Existing submit remains the no-callback path. Started runs
once on the solver thread before the solve closure; a caught panic records its
failure but does not skip solving. Final success callbacks and error handlers run
on a separate consumer pool. new(solver_count) creates one consumer by default;
with_consumer_count accepts explicit positive solver/consumer counts and rejects
count overflow. Both pools are registered for shutdown and self-join protection.

Started failures are delivered first, then the solve error or a final-callback
failure. SolverJobFailure borrows the original typed error or callback stage/message;
no Clone bound is added to results/errors. A failing error handler is recorded and
disabled, without recursive notifications or reuse after panic. Callback failures
never overwrite a solve's result. callback_failures() exposes diagnostics after
completion. The job remains Active and its ID reserved until delivery and callback
capture cleanup finish; wait/try_result publication includes these effects. Slow
final/error delivery consumes a consumer slot while solver workers can take more
jobs. Multiple consumers can overlap; cross-job delivery order is completion-queue
order, not submission order.

Queued cancellation skips every callback. Cancellation/shutdown of started jobs
still drains delivery and preserves any computed result. Shared state tracks
in-flight solve functions so consumer threads cannot exit before final dispatch.
The last dispatch wakes every consumer, including those that must exit without
receiving work. No user callback/destructor runs under manager locks. Common capture/
dispose helpers preserve prior panic-payload and abandoned-result cleanup behavior.
Callbacks must not await their own result or work needing a saturated consumer pool.
Started callbacks remain part of the solver thread's cooperative work. No callback
preemption, Java thread trace, live best event or throughput claim.

Three more full SolverManagerTest methods are adapted in solver_manager_cases.rs:
exceptionInSolver, exceptionInConsumer, testStartJobConsumer. Provenance/notices
include DEFAULT_PROBLEM_FINDER delegation to the already-attributed two-variable
fixture; the unused allocation is omitted in the immediate-error case. Native typed
errors/caught callback panics replace JVM exception classes. errorThrowableInSolver
remains pending because native allocation abort is not catchable OOM recovery.
Nine original callback regressions verify ordering, execution threads, blocked
callbacks freeing solver capacity, successful results despite failed callbacks,
handler disablement, panic delivery once, queued cancellation, draining multi-consumer
shutdown, concurrent consumers, reentrancy/self-join, constructor bounds and capture
release before wait completion. Existing six-backend integration now verifies final
callbacks, and batch_schedules checks eight final deliveries on consumer threads.

Development gate passes 1,141 Rust tests: 582 adapted, 497 original integration and
62 unit. All 47 doctests and 95 Python tests pass. Coverage is 582/4,855 (11.99%),
pending 3,991, excluded 282, failed/unverified zero; lifecycle is 26/379 (6.86%).
The batch example confirms all eight final callbacks. Logs use work/callbacks-*.log.
The full rewrite remains active and incomplete, with intermediate best events,
problem changes and other remaining semantics still to implement.

Release verification also passes all 1,141 Rust tests; the release batch example
confirms eight optimal results and eight final deliveries. All nine comparison
and performance evidence groups were refreshed and are current. The generated
report is verified, and its verification fingerprint matches the live server.
Desktop (1280x900) and mobile (390x844) browser checks confirm 11.99% overall,
6.86% lifecycle, all six credited SolverManager methods and 23 benchmark cards,
with no stale warning, horizontal overflow or browser errors.

Fresh illustrative medians (Rust/Java milliseconds): exact six-variable solve
0.011/5.465; eight-variable solve 0.172/8.489; Great Deluge 32 variables
2.945/14.309 and 96 variables 16.342/22.920; incremental-stream tabu 96 variables
231.151/47.562. Heuristic validation remains 128 valid/equal-score cases. List
validation remains 192 valid/189 equal-score cases, with the three existing
quality differences visible. Great Deluge remains 275/276 Java agreements;
the known ratio-rounding discrepancy is retained explicitly. These measurements
are workload comparisons, not callback/manager throughput measurements, and do
not establish a general performance advantage.


## Live best-solution delivery — latest increment

SolverManager now accepts submit_with_progress with an owned SolverProgress<R>
publisher, plus SolverJobCallbacks::on_best. Generic producers select their own
best solutions and explicitly publish snapshots. One pending snapshot per job is
replaced by newer publications; one more may be under consumption. Per-job delivery
is serial even with multiple consumers, and pending work is requeued at the back
after each callback. The latest accepted snapshot precedes final/error delivery.
A failed best callback is recorded once and disabled without replacing the solve
result. Started failure delivery is tracked so later updates/final completion do
not repeat notifications. Callbacks, snapshot disposal and capture destruction run
outside manager locks. Replaced-snapshot destructor panics are contained.

Publisher clones use weak references to the particular delivery/run. They require
no R: Clone, cannot keep a finished run alive, reject late publications and cannot
touch reused problem IDs. Cancellation does not close a still-running producer;
returning or panicking from the solve does. Accepted means eligible for delivery,
not guaranteed delivery of an update superseded while the consumer is busy.
Shutdown still drains the final pending update and completion. Empty-callback jobs
retain direct publication so existing status/cancellation behavior is preserved.

Added exhaustive_search_with_observer, exhaustive_list_search_with_observer and
exhaustive_mixed_search_with_observer. Each emits the first eligible complete
candidate and strict retained-best improvements, without rescoring. Ties and
structural rejections emit nothing. Observers borrow retained snapshots; caller
clones them for publication. Their execution is part of cooperative elapsed time
but not evaluation counts. Panics propagate to the caller/managed solve boundary.
The original live_schedules example combines exact optimization and asynchronous
best delivery for eight jobs, checking monotonic consumed scores and optimal finals.

The complete adapted SolverManagerTest.skipAhead now preserves four ordered
assignments, a blocked consumer, skipped obsolete update, initialized result, fewer
than four best callbacks, one final callback and no callback errors. Added channels
make its race deterministic. Native validated moves and explicit snapshots replace
Java custom-phase/score-director/event plumbing; the source contains no score
assertions. Provenance includes the four-entity/four-value PlannerTestUtils fixture.
Seven original progress regressions cover bounded coalescing/serialization across
three consumers, shutdown draining, late publishers/ID reuse, non-Clone results,
failure ordering, error-handler panic, destructor reentrancy and panic, actual
three-model exact search, and callback reentrant cancellation/republication.
Two original observer regressions preserve eligibility, tie retention, evaluation
counts, snapshots and cooperative cancellation across all three models.

Construction/local-search/ordered-plan automatic intermediate events,
first-initialized callbacks and problem changes remain to implement. The full
rewrite remains active; this is not completion or a manager-throughput claim.

Development verification passes 1,151 Rust tests (583 adapted, 506 original
integration, 62 unit), 48 doctests and 95 Python tests. Source coverage is
583/4,855 (12.01%), pending 3,990, excluded 282 and failed/unverified zero.
Solver lifecycle is 27/379 (7.12%). The development live_schedules example verifies
eight optimal results with live updates. Logs for this increment use work/progress-*.

Next-event specification caution: the pinned firstInitializedSolutionConsumer*
methods are not simply 'emit on the first complete snapshot'. The single-CH and
single-LS cases expect no event; CH-CH-LS attributes the event to CH index 1,
and CH-custom-LS to custom index 1. Read those tests, including early-terminated
construction cases, before adding this event. Keep them pending until native
phase sequencing and producer metadata preserve their full assertions.

Release verification passes all 1,151 Rust tests. Release live_schedules output
was checked per ID: all eight jobs emitted best updates and an optimal 0hard/-3soft
final result. All nine comparison/performance evidence groups were refreshed and
report current, with successful verification and a matching live-server fingerprint.
Browser verification at 1280x900 and 390x844 confirms overall 12.01%, lifecycle
7.12%, all seven credited manager methods (including skipAhead) and 23 benchmark
cards; no stale warning, horizontal overflow or browser errors.

Fresh illustrative medians, Rust/Java milliseconds: exact six-variable solve
0.012/4.509; eight-variable 0.149/6.254; Great Deluge32 2.502/8.259 and 96
16.025/17.924. Incremental stream tabu96 remains slower at 245.850/47.061,
with equal [-44,-484] scores. List two-opt16 0.349/10.676 and 32 1.102/15.471
retain their existing quality differences. List validation is 192 valid/189 equal;
heuristics 128 valid/128 equal; stream search 64 valid/64 equal. Great Deluge's
known ratio boundary remains 275/276 Java agreements, explicitly not credited as
full agreement. These workloads do not measure live-callback throughput or prove
a general Rust advantage; timing changes between shared-machine runs are not
established regressions or improvements.


## Construction/local-search observations — latest increment

Added public SolutionUpdate<'a,A,S> with borrowed assignment, cached score and
completeness. Twelve *_with_observer entrypoints cover construction and explicit
initial-assignment local search across basic/list/mixed and scoped/stream backends.
A private Observed director adapter delegates all scoring/selection and implements
a new internal notification hook. Existing directors use a no-op hook; completeness
is passed as a lazy closure, so unobserved paths do not add validation/Eq work.

Construction emits initial scoring and installed placements after requested
consistency checks, including partial states. Local search emits its initial best
and strict retained-best improvements after step bookkeeping/diagnostics, never
trial candidates, ties or accepted worsening moves. No rescoring or score-counter
increments occur. Ordinary validation/zero-budget/pre-cancellation/impossibility
rules remain. Observer execution counts toward elapsed and local stagnation time;
cooperative cancellation retains the committed prefix. Panics propagate to the
caller and are caught as solve panics inside managed jobs. Callbacks need not be
Send when created in a worker; publish owned snapshots for consumer delivery.

Each phase independently emits an initial score. The original live_heuristic
example filters cross-phase duplicate scores while running four actual construction
and local-search jobs; every consumer verifies it received updates and final
score 12 with six values assigned to 2. This is an original illustrative objective,
not a Timefold fixture or optimality claim from local-search termination itself.

Seven original integration methods in phase_observers.rs cover all twelve paths,
both mixed construction orders, committed-state/score/count equivalence, small
budget prefixes, initial cancellation, plateaus and accepted worsening moves,
observer panics/worker reuse, actual manager delivery, invalid acceptance/network/
foreign-stream validation at zero budget, and optional/empty initial completeness.
Two original unit methods use controlled clocks to check construction elapsed time
and local elapsed/stagnation deadlines. A new public doctest demonstrates partial
and complete construction updates. No source tests/fixtures were adapted in this
increment, so provenance credit stays 583/4,855 (12.01%).

Unified ordered-plan observation, first-initialized callbacks with correct phase
producer semantics and live problem changes remain pending. The whole rewrite
remains active and incomplete. Logs use work/phase-observers-*.

Development gate passes 1,160 Rust tests: 583 adapted, 513 original integration and
64 unit. All 49 doctests and 95 Python tests pass. Coverage remains 583/4,855
(12.01%), pending 3,990, excluded 282 and failed/unverified zero; lifecycle remains
27/379 (7.12%). These new original observer tests earn no adapted-method credit.

Release verification also passes all 1,160 Rust tests. Release live_heuristic
output confirms each of the four jobs emitted updates and finished with score 12;
its final callbacks check the six assigned values and nonzero per-job consumption.
All nine comparison/performance evidence groups were refreshed and are current.
The generated report verifies successfully and matches the live-server fingerprint.
Desktop (1280x900) and mobile (390x844) browser checks confirm 12.01% coverage,
7.12% lifecycle and 23 benchmark cards, without stale warnings, overflow or errors.

Fresh illustrative medians, Rust/Java milliseconds: exact six-variable solve
0.012/4.681, eight-variable 0.160/5.595; Great Deluge32 2.590/8.757 and 96
15.093/16.915. Incremental stream tabu96 remains slower at 169.535/24.549 with
equal [-44,-484] scores. List two-opt16 0.333/10.359 and 32 0.946/11.430 retain
the existing differing scores. List validation remains 192 valid/189 equal-score;
heuristics 128 valid/128 equal; stream search 64 valid/64 equal. Great Deluge is
still 275/276 Java agreements, retaining the known ratio boundary discrepancy.
These ordinary solver workloads do not measure observer/consumer throughput;
shared-machine timing changes are not proof of a regression or improvement.


## Observed ordered solve plans — latest increment

Added six solve*_with_observer APIs sharing private implementations with the
ordinary solve APIs. PlanObservation filters retained-best updates across all
phases, retaining first ties and preferring complete over incomplete states just
like SolverResult. SolverUpdate borrows assignment and carries cached score,
completeness, phase index and SolverPhaseKind::{Construction,LocalSearch,Exact}.
Construction is index zero; later indices follow the requested plan. Initial
construction may be partial; later phase initial scores equal to the retained best
are suppressed. No score replay or extra evaluation work is introduced.

Observed local helper entrypoints now accept the existing SearchControl reference
so whole-solve stagnation time and step limits are preserved. Public individual
phase observers still pass None. Exact phases optionally notify after recording a
strict improvement above their carried-in score. Ordinary exact execution passes
None without new score clones. Original controllable-clock helpers remain for
tests. Callback time and cooperative cancellation use existing phase/solve rules;
final exact exhaustion still wins over final-candidate cancellation. Native phase
metadata is not Java event-producer identity/first-initialized parity.

Seven original integration methods in support/solver_observer_cases.rs use the
existing six-backend solver fixture, now optionally observing. They check full
outcome/work equality, both mixed construction orders, cross-phase duplicates,
phase metadata, exact-after-partial construction, every score/move budget prefix,
shared stagnation steps, whole-plan validation before events, callback cancellation
and panic capture, graph-factory error event prefixes, managed final delivery and
exact final-candidate cancellation/exhaustion precedence. Two controlled-clock
unit methods check shared stagnation in local/exact phases and exact observer
elapsed time. A public doctest exercises the API. All are original tests; no new
source method/fixture credit is claimed.

Original live_plan example composes construction, one local-search step and exact
search in four manager jobs. One observer publishes snapshots without a manual
cross-phase filter. Every consumer checks monotonic scores, nonzero consumption,
final Exact phase index 2, four assigned values 2 and proven optimum score 8.

First-initialized callbacks, custom/repeated construction phases and live problem
changes remain pending. Read the earlier firstInitializedSolutionConsumer warning
before implementing those source semantics. The full rewrite remains active and
incomplete. Logs use work/plan-observers-*.

Development gate passes 1,169 Rust tests: 583 adapted, 520 original integration and
66 unit. All 50 doctests and 95 Python tests pass. Source coverage remains
583/4,855 (12.01%), pending 3,990, excluded 282 and failed/unverified zero;
solver lifecycle remains 27/379 (7.12%). No source-method credit was added.

Release verification also passes all 1,169 Rust tests. Release live_plan output
confirms all four jobs emitted updates and reached proven optimum 8; callbacks
verify final Exact phase index 2, assigned values and per-job consumption. All nine
comparison/performance groups were refreshed and are current. The verified report
matches the live-server fingerprint. Browser checks at 1280x900 and 390x844 show
12.01% overall, 7.12% lifecycle and 23 benchmark cards, with no stale warning,
horizontal overflow or browser errors.

Fresh illustrative Rust/Java median milliseconds: exact six-variable solve
0.011/4.132, eight-variable 0.145/5.832; Great Deluge32 2.388/9.691 and 96
13.627/14.666. Incremental stream tabu96 remains slower at 169.649/28.677 with
equal [-44,-484] scores. List two-opt16 0.273/10.079 and 32 0.893/10.797 retain
the existing score differences. List validation remains 192 valid/189 equal-score;
heuristic validation 128 valid/128 equal; stream search 64 valid/64 equal.
Great Deluge remains 275/276 Java agreements with its recorded ratio discrepancy.
These benchmarks use ordinary solve paths and do not measure observer/consumer
throughput. Timing changes between shared-machine runs are not established
regressions or improvements.


## Resumable/repeated construction — latest increment

Added `greedy_*construction_from` and `greedy_*construction_from_with_observer`
for all six model/backend paths. Original constructors now delegate to these with
the model initial assignment. Basic construction internals now accept an explicit
initial state, matching existing list/mixed internals. Public validation precedes
zero work/cancellation without scoring. The original controlled-clock callers now
pass explicit initial state; solver_time_tests uses the phase handoff assignment.

`SolverPhase` and `MixedSolverPhase` gain Construction variants. The shared runner
clips each later construction's controls, validates all tokens before work, allows
partial-state handoff into another construction phase, and otherwise preserves
existing retention/termination. All six solver implementations pass prior state to
the resumed entrypoints. Mixed plans pick component order from the current phase,
not the original construction config. Local search still cannot accept partial
state, and exhausted exact search still stops with its existing proof semantics.
Phase indices and observed duplicate suppression use the existing plan mechanism.

Four original integration methods in tests/construction_resume.rs check supplied
values, full/prefix pins, optional unassigned state, explicit malformed input,
zero budget/cancellation, every small evaluation prefix, both mixed orders and
observed/unobserved equality. Five methods in support/solver_construction_cases.rs
check repeated-phase continuation without reconstruction, every shared score/move
budget prefix, exact continuation, retained local-search state, shared stagnation
steps, token/factory failures, cancellation and each mixed phase's own ordering.
One controlled-clock method checks initial construction exclusion from stagnation,
shared deadline including graph factory cost, and later construction not resetting
an already started stagnation timer. A SolverPlan doctest shows partial completion.
No tests/fixtures were copied or adapted for this increment, so provenance stays
unchanged. Work logs use work/repeated-construction-*.

First-initialized event investigation: pinned SolverManagerTest.java lines
282–560 and helper lines1317–1336 were inspected (tests only). Important: its
InitialSolutionEvent.isInitialized() returns !event.isTerminatedEarly(), NOT a
solution-completeness predicate. Default CH/LS emits with CH0 producer; single CH
and single LS omit events; CH/CH/LS emits with CH1; custom/CH/LS emits CH1;
CH/custom/LS emits custom1; two custom phases omit events. Early CH termination
emits with terminatedEarly true. Native optional list construction skips assignment,
so the early-list fixture still has a separate behavior gap. Repeated construction
is now supported; first-initialized callbacks, custom phases, construction-free
plans and live problem changes still need work. Keep the overall goal active.

Development gate passes 1,179 Rust tests: 583 adapted, 529 original integration and
67 unit. All 51 doctests and 95 Python tests pass. Release verification passes the
same 1,179 Rust tests, and release live_plan confirms all four managed jobs reach
proven optimum 8 with final phase metadata and monotonic consumer assertions.
Coverage remains 583/4,855 (12.01%), pending 3,990, excluded 282, failed/unverified
zero; solver lifecycle stays 27/379 (7.12%). All nine evidence groups are current,
verification is successful and the live-server fingerprint matches. Browser checks
at 1280x900 and 390x844 show 23 benchmark cards, no stale messages, no horizontal
overflow and no browser errors. Filtering lifecycle by firstInitializedSolutionConsumer
shows all nine source methods still Pending. Browser session closed.

Fresh illustrative Rust/Java median milliseconds: exact six-variable 0.013/4.033,
eight-variable 0.150/6.147; Great Deluge 32 2.498/9.619 and 96 14.449/16.724;
stream tabu 96 remains slower at 176.577/25.070 with equal [-44,-484] scores.
List two-opt 16 0.336/10.506 and 32 0.947/10.513 retain score differences. List
validation remains 192 valid/189 equal; heuristic 128 valid/128 equal; stream 64
valid/64 equal. Great Deluge remains 275/276 Java agreements, with its recorded
ratio boundary discrepancy. These ordinary workloads do not measure repeated-
construction or event/manager throughput. Shared-machine timing variation alone
does not establish regressions/improvements. Upstream submodule remains unchanged.


## Initialization events and managed delivery — latest increment

Added six `solve*_with_events` functions and SolverEvent::{Best,FirstInitialized}.
New SolverInitialization metadata is Copy with native phase/kind/terminated_early;
FirstInitialized also borrows retained assignment, optional score and completeness.
These flags are independent. PlanCallback internally selects Best or Events while
ordinary solving retains None. `run` explicitly passes mutable PlanObservation to
each executing closure; the shared runner emits initialization after recording the
producer's PhaseReport, before shared stop handling. No RefCell or duplicate score
computation. The existing controlled-clock test adapter wraps run_with_clock_observed
with None and its old four-argument phase closure; a new unit test uses the observed
runner to show initialization callback elapsed time stops before local search.

The producer is the phase immediately before the first requested local-search
phase. No local search means no event. Earlier stops/errors omit it; successful
producer returns emit even if partial, zero work, globally limited, or an exhausted
native exact phase stops the solve at that boundary. Complete assignment does not
imply terminated_early=false. Construction Completed / Exact Exhausted mean false;
other producing-phase reasons mean true. The exact behavior is native, not Java
parity. Callback panic propagates; elapsed time and cancellation count toward solve
controls. Best-only observers remain unchanged.

Manager progress internally sends ProgressMessage::{Best,Initialized}. Delivery
has a separate initialized Option<(R,SolverInitialization)> and one-shot available
latch, set false atomically on the first accepted initialization. A boolean preserves
publication order relative to the retained best: best replacement moves that best
after the initialization; initialization arriving after a pending best stays after
it. The consumer takes a batch and invokes callbacks in that order without locks,
then completes or requeues. Initialization cannot be coalesced away. Existing serial
per-job delivery, weak publisher identity, shutdown and lock/destructor rules remain.
The public APIs are on_first_initialized and publish_first_initialized. A panic
records FirstInitialized stage and reports once without disabling best/final; a
later solve error still drains accepted initialization. Waits and ID reuse include
delivery. No Clone requirement on R. Manager does not infer events from final return.

Five original integration methods in support/solver_initialization_cases.rs use
the extended common fixture to check six-backend boundaries, repeated construction,
shared prefixes, completeness/early flags, exact prior retention, cancellation,
validation and graph-factory failure. Five in solver_initialization_delivery.rs
check slow-consumer coalescing/order with three consumer threads, reliable one-shot
initialization, shutdown drain, final wait/ID reuse, callback/error-handler panics,
solve error/panic and eight concurrent publishers with exactly one accepted event.
One controlled-clock unit test and one public doctest are added.

Three complete adaptations in solver_manager_cases.rs cover
firstInitializedSolutionConsumerWithDefaultPhases,
firstInitializedSolutionConsumerWithSingleCHPhase, and
firstInitializedSolutionConsumerWith2CHAndLS. Assertions preserve the helper's
non-early-termination flag and producer index, including absence for single CH.
The default fixture retains two required variables/two values, constant-zero
ordinary scoring, a ten-step local cap and source 1ms shared stagnation where
configured. Java wrappers become owned native snapshots and mutex-protected state.
Provenance adds the three methods plus helper/config/scorer fixture records; notices
now identify ten manager methods. Only tests and the public docs description in
library-integration.adoc lines182–183 were used; no production implementation read.

Original live_initialization example connects plan events directly to manager
publishers for four jobs. The first CH is limited to one candidate; CH1 fills the
remaining three variables. Consumers assert origin CH1, non-early termination,
complete [0,2,2,2] with score6, monotonic bests, final all2/score8 and exact proof.
Both final and initialization counters must be four after shutdown.

Next gaps: six remaining first-initialized source methods, custom phases,
construction-free plans and live problem changes. Early-CH source exception/step
limit and optional-list construction remain separate gaps. Overall goal stays
active; this increment does not establish all lifecycle or Java scheduling parity.
Work logs use work/initialization-*.

Development and release verification pass 1,193 Rust tests: 586 adapted, 539
original integration and 68 unit. All 52 doctests and 95 Python tests pass. Both
debug/release live_initialization runs deliver all four initialization snapshots
before completion and verify the expected phase, values, score and proven optimum.
Source coverage is now 586/4,855 (12.07%), pending 3,987, excluded 282 and
failed/unverified zero. Lifecycle is 30/379 (7.92%). All nine comparison/performance
groups were refreshed and are current, with successful verification and a matching
live-server fingerprint. Browser checks at 1280x900 and 390x844 show 23 benchmark
cards, no stale warning, no overflow and no browser errors. The lifecycle search
for firstInitializedSolutionConsumer lists three Passed and six Pending methods.
Browser session closed; upstream submodule unchanged.

Fresh illustrative Rust/Java median milliseconds: exact 6 0.011/3.966, exact 8
0.153/5.895; Great Deluge 32 2.254/9.524, 96 14.423/16.454; stream tabu 96 remains
slower at 187.903/28.787 with equal [-44,-484] scores. List two-opt 16 0.253/8.745
and 32 0.941/12.099 retain the known score differences. List validation remains
192 valid/189 equal-score, heuristic 128 valid/128 equal, stream 64 valid/64 equal.
Great Deluge retains 275/276 Java agreements and its recorded ratio boundary gap.
These ordinary workloads do not measure initialization-event/manager throughput;
shared-machine timing differences alone do not establish improvements/regressions.


## Explicit phase sequences — latest increment

Added SolverSequence/MixedSolverSequence with shared limits and an explicit full
phase vector. Defaults and From<SolverConfig/SolverPlan> preserve the existing
construction-first plan by inserting construction into that vector. All 18 public
ordinary/observer/event solve APIs now accept Into<Sequence>; existing concrete
config/plan arguments remain accepted. Mixed conversions preserve each phase's
construction order and local-search settings. Phase indices are direct vector
indices in results, events, validation and factory errors.

Shared run now walks the full vector rather than injecting construction at zero.
A lazy InitialState factory creates the model initial assignment/completeness only
after empty/acceptance/token validation. Result begins with that unscored state,
so a first zero-work/cancelled local or exact phase can return without inventing
an assignment or score. Exact candidates do not gain an extra initial evaluation;
a complete initial assignment is not assumed to beat scored candidates until it
is scored. Empty sequences return new SolverError::EmptyPlan, with no fact cloning,
scoring or graph creation. Explicit incomplete initial state is rejected by first
local-search validation even at zero budget/cancellation. The legacy partial
initialization termination remains IncompleteConstruction when no next phase can
accept partial state. Existing shared stopping precedence is unchanged.

Scoped exact phase binding now explicitly calls network.validate before enumeration,
including zero work; basic ConstraintNetwork.validate is pub(crate). Stream source
validation already occurs there. This closes the dependency on an earlier
construction phase. First-local index is direct: zero has no producer, so no
initialization event. Partial exact-to-construction handoffs and later mixed phases
use their exact sequence indices. Old controlled-clock observation helpers prepend
their construction through a test adapter; the shared time fixture now uses explicit
sequences directly. No callback evaluations are added by state setup.

Eight original integration methods in support/solver_sequence_cases.rs cover all
six paths: single local search with exactly one phase; exact-only candidate counts
and exhaustion; zero work/pre-cancellation state retention; incomplete/empty plan
errors (including a Clone implementation that panics if invalid plans touch facts);
partial exact-to-construction handoff; absence of initialization for local-first
sequences; indices/shared controls; zero-budget scoped/stream validation; and every
small shared score/move prefix across local/exact phases. A controlled-clock unit
test checks first graph-factory elapsed time and local-initial-score exclusion from
stagnation. A public SolverSequence doctest adds direct-local usage.

Adapted firstInitializedSolutionConsumerWithSingleLSPhase in solver_manager_cases.rs:
two entities initialized to first value, constant-zero objective, sole local search,
shared target zero, initialization listener plus empty final listener, final wait,
false initialization marker and absent producer. Native extra assertions check the
single phase at index zero, target stop and no callback failures. Provenance adds
one method, reusing already-attributed fixtures; notices now count eleven manager
methods. Only the pinned Community test was consulted, no production implementation.

The five remaining initialization-event methods need custom phases or early-CH
behavior (step-limit/exception and optional-list gaps). Live problem changes also
remain pending. Keep the full goal active. Work logs use work/sequence-*.

Validation completed for this increment: bash scripts/check.sh passes with 1,203
Rust tests (587 adapted, 547 original integration, 69 unit), 53 doctests and 95
Python tests. Release all-target tests pass all 1,203 cases; live_initialization
also passes in release, delivering four initialization callbacks and four proven
optima. All nine differential/performance datasets were refreshed successfully.
Report verification is successful and its verification digest matches the live
freshness endpoint. Coverage is 587/4,855 (12.09%), with 3,986 pending and 282
excluded; lifecycle is 31/379 (8.18%).

Browser verification at 1280x900 and 390x844 shows 23 benchmark cards, no horizontal
overflow, no stale/error messages and no browser errors. Filtering lifecycle for
firstInitializedSolutionConsumer shows four Passed and five Pending methods.
Browser session closed; upstream submodule remains unchanged.

Fresh illustrative Rust/Java median milliseconds: exact 6 0.011/3.928, exact 8
0.150/6.987; Great Deluge 32 2.398/8.182, 96 14.071/16.376; stream tabu 96 remains
slower at 166.569/30.248 with equal [-44,-484] scores. List two-opt 16 0.257/8.957
and 32 0.908/11.194 retain the known score differences. List validation remains
192 valid/189 equal-score, heuristic 128 valid/128 equal, stream 64 valid/64 equal.
Great Deluge retains 275/276 Java agreements and its recorded ratio boundary gap.
These workloads do not measure explicit-sequence or manager throughput, and these
shared-machine samples alone do not establish an improvement or regression.


## Construction assignment steps — latest increment

Added ConstructionConfig::max_steps: Option<u64> (default None), applied to all six
model/backend paths. One step installs a newly assigned required variable/list
value; it finishes the full candidate scan or an early pick. Existing assignment
counters expose the count. Mixed uses one shared count across component order.
The check runs after skipping supplied/optional assignments and before the next
required scan. Reaching the cap with another required item returns new
ConstructionTermination::StepLimit; completing exactly at the cap returns Completed.
Zero permits initial scoring; complete/empty/optional-only models still complete.
Each resume and each phase has a fresh count. Existing candidate/score truncation,
time/cancellation polling and validation/impossibility precedence are unchanged.
Cancellation/time precede step checks, which precede candidate-budget observation.
SolverWork remains local-search-only for completed_steps/accepted_moves and shared
stagnation. Four existing explicit test config literals now include max_steps: None (basic,
list and mixed stream comparisons plus mixed construction). No default behavioral change or new adaptation credit.

Seven original integration methods were added: three in support/construction_step_cases.rs
(included by construction_cancellation.rs) cover all six paths/both orders/early picks,
diagnostics, every small step/score/candidate cap combination, cancellation and zero
score precedence; two in construction_resume.rs cover preserved inputs/pins, optional
skips, reset counts, validation, impossible/empty/optional models and observers;
two in support/solver_construction_cases.rs cover repeated construction and local/exact
handoffs, event producers and early flags. One controlled-clock unit in
timed_construction_tests.rs checks time precedence. One ConstructionConfig doctest
shows a two-call resume. Focused checks passed before the full gate.

The full source methods for early-CH initialization stay pending: native incomplete
local-search handoff returns IncompleteConstruction rather than Java's exception,
and optional list construction still skips unassigned values. Custom phases/live
problem changes also remain pending. No source implementation was inspected.
Work logs use work/construction-step-*.

Next-step evidence: inspected only the pinned test fixtures
core/src/test/java/ai/timefold/solver/core/testdomain/list/unassignedvar/
TestdataAllowsUnassignedValuesListConstraintProvider.java and ...ListEntity.java.
The optional-list manager source test has two initially empty lists and two values,
CH step limit one then LS step limit zero. The fixture penalizes list sizes and
each unassigned value, giving total ordinary score -2 for every ownership state.
Its event still needs to report early construction termination. Native construction
currently skips optional values entirely, so step limiting alone cannot satisfy it.
A future optional-value construction policy must handle ties, unavailable insertions,
existing placements/pins, initial completeness and retained-best publication honestly;
do not award source-method credit by simulating the event. Any adapted fixtures need
new provenance records. The two inspected fixtures have not been copied or adapted.

Final verification: bash scripts/check.sh passes 1,211 Rust tests (587 adapted,
554 original integration, 70 unit), 54 doctests and 95 Python tests. Release
all-target tests pass the same 1,211 cases. live_initialization passes in release
with four callbacks and four proven optima. All nine differential/performance
datasets are current; report verification succeeds and matches the live endpoint's
verification digest. Coverage remains 587/4,855 (12.09%), 3,986 pending and 282
excluded. Native regressions add no source credit. Upstream submodule unchanged.

Browser checks at 1280x900 and 390x844 show 23 benchmark cards, correct counts,
no stale/error messages or horizontal overflow, and no browser errors. Browser
session closed. Fresh illustrative Rust/Java median milliseconds: exact 6
0.010/4.316, exact 8 0.146/5.496; Great Deluge 32 2.198/9.124, 96 13.742/15.088;
stream tabu 96 171.049/26.060 (equal [-44,-484]). List two-opt 16 0.270/7.405,
32 0.897/12.397 keep the recorded score differences. All 128 heuristic and 64
stream validation cases retain equal scores; list validation remains 192 valid,
189 equal. Great Deluge still has 275/276 Java agreement cases. These workloads
use default unlimited construction steps and do not measure the new step-limited
phase/event throughput; shared-machine timing changes alone prove no regression
or improvement. Keep the full goal active.


## Optional construction — latest increment

ConstructionConfig::optional now offers OptionalConstruction::Preserve (default)
and NonWorsening. The latter scans missing optional basic/list values in existing
model/domain/insertion order across all six paths. Supplied values, list pins and
basic variables pinned to None are preserved. Optional candidates must score at
least the cached pre-scan state; best accepted score wins, ties keep the first
candidate (including ties with unassignment). All-worse/unavailable insertions
leave None untouched. No null synthetic candidate or extra rescore is counted.
Required assignments keep their old behavior and structural priority.

Each construction result now exposes completed_steps, counting scans with at least
one evaluated candidate, including rejected optional scans and interrupted scans.
Assigned counters count installed placements only. Mixed has one scan count across
both components. max_steps now bounds that count; under default Preserve behavior
it equals the old assignment count. Empty scans count zero, but the pre-scan cap
can stop before discovering no candidates. Resume resets the count and considers
remaining missing items again, including earlier optional rejections. Native
solver work/stagnation counters still exclude construction. Standalone observers
notify initial state and committed placements; rejected scans add no events.
All 12 resume API comments now describe configured optional construction.

Three complete source methods now adapted: DefaultConstructionHeuristicPhaseTest.
solveWithAllowsUnassignedBasicVariable (first value/second value/null, score -1)
and solveWithAllowsUnassignedValuesListVariable (retain two initial values from
four, score -2), in construction_optional_cases.rs; SolverManagerTest.
firstInitializedSolutionConsumerEarlyTerminatedCHListVar in solver_manager_cases.rs
(two optional values/two empty lists, CH step one, LS step zero, successful final
wait, called callback, early flag true and producer CH0). The manager test also
requires an actual placement and both phase reports; it does not simulate events.
Explicit NonWorsening is a native configuration adaptation, not Java default-policy
parity. The existing InitialSolutionEvent helper is reused. Eleven fixture records
cover the optional basic/list model metadata and the distinct scoring rules; notices
now count twelve manager methods and the two new construction methods. No production
source was inspected. The tests use native derived list relationships instead of
explicit Java shadow synchronization.

Four original integration methods: three in support/optional_construction_cases.rs
(included by construction_cancellation.rs) check all six paths, both mixed orders,
positive/zero/negative objectives, early picks, diagnostics, all small budget
combinations and cancellation prefixes; one in construction_resume.rs checks pins,
empty domains, unavailable insertion, existing assignments, observers and resumed
mixed step budgets. The former fixture's run_problem helper takes optional model
and objective parameters; legacy run delegates with its unchanged required model.
Outcome now includes completed_steps. Four explicit legacy ConstructionConfig
literals supply optional: Preserve. Focused and full debug checks passed.

Remaining initialization source methods: early required CH (native incomplete
handoff still returns IncompleteConstruction rather than Java exception) and the
three custom-phase cases. Live problem changes and broader solver parity remain
pending. Work logs use work/optional-construction-*.

Further source evidence for future custom-phase work: read the three manager
custom-phase initialization tests and pinned public docs optimization-algorithms/
overview.adoc around lines 880–938. Commands run in sequence, must cooperate with
termination, and mutate entities through score-aware move operations; problem facts
are not mutable through this contract. The docs state non-improving commands do not
replace the best state carried into later phases/commands. The manager fixtures
assert no initialization during custom commands; CH/custom/LS attributes the event
to custom phase 1, and two custom phases without LS emit none. This needs an actual
command/phase API, not simulated producer notifications. No command implementation
was inspected or adapted in this increment.

Final verification: bash scripts/check.sh passes 1,218 Rust tests (590 adapted,
558 original integration, 70 unit), 54 doctests and 95 Python tests. Release
all-target tests pass the same 1,218 cases; live_initialization delivers four
callbacks and proves four optima. All nine differential/performance datasets are
current, report verification succeeds, and the live freshness verification digest
matches. Coverage is 590/4,855 (12.15%), 3,983 pending, 282 excluded; lifecycle
32/379 (8.44%) and search algorithms 24/158 (15.19%). Upstream unchanged.

Browser checks at 1280x900 and 390x844 show 23 benchmark cards, no horizontal
overflow, stale/error messages or browser errors. Lifecycle filtering shows five
Passed and four Pending firstInitializedSolutionConsumer methods; search-algorithm
filtering shows both optional-construction methods Passed. Browser session closed.

Fresh illustrative Rust/Java median milliseconds: exact 6 0.012/4.294, exact 8
0.137/5.577; Great Deluge 32 2.402/8.804, 96 13.996/14.477; stream tabu 96
176.778/28.934 (equal [-44,-484]). List two-opt 16 0.289/8.581, 32 1.059/15.435
retain known score differences. Validation remains 128 heuristic/64 stream equal
scores, list 192 valid/189 equal. Great Deluge remains 275/276 Java agreement with
its recorded ratio boundary difference. These default-policy workloads do not
benchmark optional-construction throughput; shared-machine samples alone establish
no performance improvement/regression. Keep the full goal active.


## Native custom commands — latest increment

New custom_phase.rs exposes CustomPhaseConfig/Termination/Work/Result/Error and
CustomPhaseContext<A,M,S,E>, plus plain and _with_observer entrypoints for all six
model/backend combinations. CommandDirector is a private object-safe transactional
adapter implemented by a macro over the six existing concrete directors. It calls
apply(), invokes a visitor with assignment/score/constraint work, then commits or
lets the existing guard roll back. Existing production scoring was reused unchanged.

Context execute commits validated moves (including worsening); evaluate visits and
rolls back scored candidates. Temporary improvements do not enter the best. Context
tracks current and retained-best state independently; completion or strict score
improvement replaces the best, ties retain the earliest best. Observers receive the
initial scored state and strict committed bests only. Explicit native partial input
is supported; commands can initialize it. Results include best assignment/optional
score/completeness, work counters and honest termination, with no optimality claim.

Config limits evaluations (initial score included), candidate slots, time and a
borrowed cancellation token. Wrappers validate input/network/source ownership and
emptiness before early zero/cancel returns. Runtime includes director initialization,
initial observer and command work. Commands cooperate by returning or polling;
operations check before acting, and a final poll catches elapsed time during the
last operation. Termination latches, with cancellation/time/score/move precedence;
zero score precedes pre-cancellation. Zero time/move allows initial scoring but no
command. Diagnostic assert_consistent excludes its callbacks from work counts.

Failures: operations charge a slot before touching the director. A successful
candidate supplies its callback count before executing a temporary visitor, so a
caught visitor panic rolls back and remains accounted. Before scoring/commit the
context latches failure; caught scoring/commit panics cannot yield success with
unknown callback work. Operation errors also latch failure; if swallowed by a
command, run returns IgnoredOperationError. Propagated errors retain typed Scoring(E),
explicit Command(String) is supported, and uncaught panics propagate. Diagnostics
cannot clear an existing failure. execute's best observer runs after commit.

Eight original methods in tests/custom_commands.rs cover all six paths, every
small score/move cap, optional observer streams, cache/candidate rollback, temporary
visitor panic recovery, scoring panic invalidation, ignored and propagated errors,
explicit command errors, partial initialization, zero/cancel/time controls and
input/stream validation. A normalized fixture maps values to list owner indices;
moves derive their source from the live assignment. Two private clock tests use
actual ScoreDirector caches and injected clocks to cover initialization/observer/
command/scoring elapsed time, backwards clock/latching and count precedence.
One public doctest demonstrates temporary scoring and retaining a committed best.
Focused tests and clock cases passed before the full gate.

This increment is the real execution foundation, NOT ordered custom-phase support.
SolverPhase still has construction/local/exact only; no new source methods are
credited. Next wire typed commands into the ordered runner so real Custom producer
indices, phase reports/shared limits, callbacks and custom-to-CH/LS handoffs can
satisfy the three source manager methods. Do not simulate producer metadata or
credit these standalone regressions as upstream passes. Four initialization-event
source methods remain pending. Deferred score APIs and Java structural error
semantics are not implemented. Read the latest solver-contract.md section.
Work logs use work/custom-command-*; full goal stays active.

Integration points inspected for the next increment: solver_plan.rs keeps phases
model-independent and derives Clone/Eq, whereas command closures depend on assignment,
move and backend-error types. Choose typed command binding without erasing those
errors or breaking existing concrete config conversions. solver.rs PhaseRequest,
PlanObservation::initialize, report/error conversion, cancellation prevalidation,
shared budget ownership, graph-factory time clipping and partial-state handoffs all
need a real Custom branch. MixedSolverPhase::common must preserve phase indices.
Shared stagnation currently uses SearchControl through local/exact execution; custom
commands need deliberate semantics and tests there, rather than just boundary polls.
Global score/feasibility stops also need to prevent later command operations after
a retained committed best reaches the target. Keep command failures and successful
zero-work phases distinct. Existing standalone APIs remain useful independently.

Final verification: bash scripts/check.sh passes 1,228 Rust tests (590 adapted,
566 original integration, 72 unit), 55 doctests and 95 Python tests. Release
all-target tests pass the same 1,228 cases; live_initialization delivers four
callbacks and proves four optima. All nine differential/performance datasets were
refreshed successfully and are current. Report verification succeeds and its digest
matches the live freshness endpoint. Coverage stays 590/4,855 (12.15%), with 3,983
pending and 282 excluded. Original command regressions add no source-method credit.

Browser checks at 1280x900 and 390x844 show the correct counts and 23 benchmark
cards, no horizontal overflow, stale/error messages or browser errors. Browser
session closed. Pinned upstream remains unchanged.

Fresh illustrative Rust/Java median milliseconds: exact 6 0.012/4.039, exact 8
0.144/4.664; Great Deluge 32 2.210/9.010, 96 13.406/15.209; stream tabu 96 remains
slower at 173.333/22.097 (equal [-44,-484]). List two-opt 16 0.312/10.136 and
32 1.006/10.819 retain known score differences. Validation remains 128 heuristic
and 64 stream equal-score cases, with list 192 valid/189 equal. Great Deluge still
has 275/276 Java agreement and its explicit ratio boundary gap. These workloads
do not benchmark custom-command throughput; shared-machine timing samples alone
establish no performance improvement or regression. Keep the full goal active.


## Ordered custom phases — latest increment

SolverPhase/MixedSolverPhase now have Custom(CustomPhaseConfig). Twelve new public
APIs (six model/backend families, plain and _and_events) bind a typed FnMut taking
phase index and CustomPhaseContext. Dispatch remains explicit; plans remain generic
only over scores, Clone/Eq and compatible with existing conversions. No callback
means UnboundCustomPhase before any cloning/scoring/factory, even for unreachable
phases. New SolverError::Custom retains phase index and mapped typed scoring or
command errors; existing factory/ordinary failures stay Phase. The internal phase
executor now returns already-located SolverError rather than losing custom identity.
Legacy controlled-clock fixtures map their errors explicitly.

All six branches validate custom initial assignment/network/source ownership, clip
time after graph factories, instantiate the native director, invoke the shared
custom engine, and convert results/reports. Shared score/candidate caps and tokens
bind in the common runner. CustomPhaseTermination adds shared stagnation and goal
reasons. Custom context polls the shared SearchControl and committed-best targets;
strict improvements notify it before observer callbacks. Initial scoring starts
stagnation afterward, preserving later phase elapsed time. Custom commits count no
local steps; strict custom improvements reset existing shared stagnation. A private
SharedControl forwarding wrapper reborrows mutable monitors at the context lifetime.
Standalone APIs still pass no shared monitor or target. Commands remain cooperative.

Custom handoffs accept partials, return retained bests, report actual indices and
emit real initialization events when immediately before the first requested local
search. Completed is non-early; failures emit none. Two custom phases without LS
emit none and can both execute despite partial states. Native final partial result
semantics remain IncompleteConstruction. Deferred-scoring APIs remain pending.

Seven original methods in solver_custom.rs exercise all six bindings, plain/event
parity, operation-prefix budgets, targets versus temporary trials, committed-best
handoffs, partial construction, producer indices, phase-local limits, cancellation,
validation/conflicts and command/scoring errors. Two real-stream controlled-clock
methods extend solver_time_tests.rs for shared factory/time/candidate prefixes and
cross-custom stagnation; four custom clock tests pass. The first attempted filter
matched zero tests; corrected filter custom_ ran four, including both new methods.
A public solve_with_commands doctest was added. Legacy owned-score/solver fixtures
handle the new error/phase variants without claiming custom coverage there.

Three source methods in solver_manager_cases.rs preserve the custom/CH/LS,
CH/custom/LS and two-custom initialization assertions. Every actual command asserts
false initialized state; managed consumers update the existing attributed helper.
Final waits include delivery, producer kind/index and absent producer assertions
are preserved, with extra actual-command counts and no-callback-failure checks.
The source 1 ms stagnation timeout is explicitly replaced by one shared unimproved
local-search step for deterministic ordering. Provenance adds three cases; existing
attributed generator/scorer/event fixtures are reused and notices count fifteen
manager methods. No production source was inspected. Only early-required-CH remains
pending among source initialization methods; its Java exception contract is distinct.

Work logs use work/ordered-custom-*. Full goal remains active; next substantive
work includes deferred-score commands, live problem changes, the required-CH error
contract and further remaining Community behavior. Run/record the full gate,
release verification, comparison refresh and browser check before committing.

Further test/public-doc evidence reviewed while release verification runs:
SolverManagerTest.firstInitializedSolutionConsumerEarlyTerminatedCH catches an
optional ExecutionException and checks its IllegalStateException root/message,
then always checks called=true, non-early=false, producer CH0. Keep that conditional
error assertion visible when deciding a future full adaptation; simply omitting
it would establish only the native successful partial-return path. Public custom
command docs distinguish uninitialized assignments from structural flaws and require
special temporary-flaw handlers plus fail-fast ordinary commands; native partial
assignment support alone establishes none of that Java structural contract. Multiple
Java commands also carry best state between commands, unlike multiple operations
inside one native callback. Read overview.adoc around 843–938 before that next work.

Final verification: bash scripts/check.sh passes 1,240 Rust tests (593 adapted,
573 original integration, 74 unit), 56 doctests and 95 Python tests. Release
all-target tests pass the same 1,240 cases; live_initialization delivers all four
callbacks and proves all four optima. All nine differential/performance datasets
are refreshed/current. Report verification succeeds and matches the live freshness
endpoint's digest. Coverage is 593/4,855 (12.21%), with 3,980 pending, 282 excluded
and no failed/unverified methods. Lifecycle is 35/379 (9.23%). Upstream unchanged.

The old dashboard server had stopped (no listener/process on 8765); the initial
freshness client waited without a timeout and was terminated after that diagnosis.
Started scripts/dashboard_server.py with output in work/ordered-custom-dashboard-server.log;
server exec session 31915, PID 2305 at verification time. Reuse if still live. A
bounded retry confirms the digest. Browser verification sees 200 responses for the
page, report and freshness API. Desktop 1280x900 and mobile 390x844 show correct
counts, 23 benchmark cards, no horizontal overflow, stale/error messages or browser
errors. Screenshots in work/ordered-custom-{desktop,mobile}.png were inspected.
Lifecycle filtering shows eight Passed and one Pending firstInitializedSolutionConsumer
methods, including all three new custom cases. Browser closed; local server retained.

Fresh illustrative Rust/Java median milliseconds: exact 6 0.009/3.382, exact 8
0.143/5.312; Great Deluge 32 2.185/7.944, 96 13.783/15.053; stream tabu 96
169.867/24.257 (equal [-44,-484]). List two-opt 16 0.255/8.350, 32 0.851/10.791
retain known score differences. Validation remains 128 heuristic/64 stream equal
scores, list 192 valid/189 equal; Great Deluge remains 275/276 Java agreement with
its explicit ratio boundary gap. These workloads do not benchmark custom-command
throughput. No broad speedup/regression claim from shared-machine samples.


## Strict incomplete handoffs — latest increment

Added IncompleteHandoffPolicy::{ReturnPartial(default),Error} to SolverLimits through
incomplete_handoff. Error rejects an incomplete retained state when the next phase
is LocalSearch, after preceding best/init notifications and before any local factory
or scoring. SolverError::IncompleteLocalSearch { phase } identifies the destination
and supplies the initialized-solution message. Its Error source is None, making it
the root beneath SolverJobError::Solve. It is an explicit native policy, not a Java
default or exception-class ABI claim. First/standalone local input validation remains
unchanged. No last-phase error and no error for custom/construction/exact handoffs.

Existing reported global stops, callback cancellation and elapsed time precede
strict errors. An additional strict-path budget gate catches coincident global
score/move exhaustion hidden by construction's earlier phase-local step check;
such a local phase cannot run, so return EvaluationLimit/MoveEvaluationLimit. Default
ReturnPartial preserves its previous boundary-reason precedence. Shared budget checks
use post-phase work. The policy remains Copy/Eq with the existing generic limits.
Legacy owned-score/solver test fixtures handle the new enum variant explicitly.

Four original integration methods: three support/solver_incomplete_handoff_cases.rs
methods included by solver.rs cover all six paths, score/step/move-zero producer
limits, zero local budgets, event-before-error, absent local factory/callback work,
plain/event parity, default partials, shared budgets/time/callback cancellation,
repeated construction and exact resumption/indices. One solver_custom.rs method
covers repeated partial custom producers and zero-work exact producers across all
six paths. A solver_observation_tests.rs controlled-clock method proves that init
callback elapsed time wins over strict rejection. A public policy doctest was added.
Focused tests, including the existing optional-list source case, pass.

The full remaining source initialization method is adapted in solver_manager_cases.rs.
Two required entities/two values start cleared, with the TestdataConstraintProvider
penalty of one per assigned entity. CH step one leaves one assigned value, followed
by local search under Error. Actual managed initialization publishes first; a blocked
consumer proves failed-job wait waits for delivery. The typed root error is checked
as IncompleteLocalSearch(phase1), with the source initialized-solution text and no
further source. Source called=true, isInitialized=false and CH0 assertions are all
preserved; extras check one assignment, structural -1/ordinary -1, no final callback
and no callback failures. Provenance adds one method and three test-fixture records
(TestdataSolution generator, TestdataEntity metadata, TestdataConstraintProvider);
notices now count sixteen manager methods. Existing attributed event helper reused.
No production implementation was inspected. All nine source initialization-event
methods now have adaptations; broad manager/live-change behavior remains incomplete.

Work logs use work/incomplete-handoff-*. Full gate/release, comparison freshness and
browser verification still need final recording before commit. Full goal stays active.

Next substantive work evidence (tests/public documentation only): pending
SolverManagerTest.addProblemChange, addProblemChangeToNonExistingProblem_failsFast
and addProblemChangeToWaitingSolver require a real change queue, including queued
jobs. The daemon test adds a fifth value to an initial four and waits on the change
receipt before reading the consumer's updated solution; the waiting-job test queues
the same change while another job blocks its only worker, then awaits a final
consumer containing the added fact. Unknown ID 999 fails fast and identifies it.
Public responding-to-change/real-time-planning.adoc says the future completes when
a user consumer accepts a best solution containing the change, not merely when the
change is applied. The solver stops, applies changes to adjusted retained best,
re-runs phases and resets normal termination, while explicit terminateEarly stays
latched. Daemon mode waits without consuming CPU after normal termination and wakes
for changes; explicit cancellation exits. Mutable fact updates require isolated
owned data rather than mutation through existing scorer captures. Current opaque
FnOnce managed jobs and immutable model/scoring sessions do not supply this yet.
Do not award change-method credit for just a queue or manually published fabricated
snapshot. Native revisions/receipt delivery, warm restart and cancellation need
independent deterministic evidence before adapting these methods.

Final verification: bash scripts/check.sh passes 1,246 Rust tests (594 adapted,
577 original integration, 75 unit), 57 doctests and 95 Python tests. Release
all-target tests pass the same 1,246 cases; live_initialization delivers all four
events and proves all four optima. All nine differential/performance datasets were
refreshed successfully and are current. Report verification succeeds, and the live
freshness endpoint matches its digest. Coverage is 594/4,855 (12.23%), 3,979 pending,
282 excluded, no failed/unverified; lifecycle is 36/379 (9.50%). Upstream unchanged.

The existing dashboard server remained live and was reused. Browser checks at
1280x900 and 390x844 show correct counts, 23 benchmark cards, no horizontal overflow,
stale/error messages or browser errors. Lifecycle filtering now shows all nine
firstInitializedSolutionConsumer methods Passed. Browser session closed; server
retained. No frontend design changes were made.

Fresh illustrative Rust/Java median milliseconds: exact 6 0.009/3.753, exact 8
0.136/6.027; Great Deluge 32 2.408/7.348, 96 13.768/14.757; stream tabu 96
171.616/26.204 (equal [-44,-484]). List two-opt 16 0.302/8.531, 32 0.843/10.807
retain known quality differences. All 128 heuristic and 64 stream validation cases
still have equal scores; list validation remains 192 valid/189 equal. Great Deluge
retains 275/276 Java agreement and the explicit ratio-boundary difference. These
workloads use default ReturnPartial and do not benchmark strict-error delivery;
shared-machine samples establish no broad performance improvement/regression.

## Live changes increment

Added live_solver.rs, a native generic live-job layer over the existing solver and
consumer pools. It owns P, serializes queued changes by revision, restarts using a
fresh token and previous R, and acknowledges receipts only after successful user
consumer return. Daemon waits use a Condvar. Best snapshots coalesce cumulatively;
Final delivery in daemon mode requires cancellation before receipts complete.
Changes can lower scores. Model/scorer rebuilding and warm reconciliation are
explicit user closures; no automatic graph repair or within-run best streaming.

Eleven original tests cover all six actual solver paths, added/removed facts and
lower scores, slow/coalesced consumers, queued changes/cancellation, stale handles,
active native scoring interruption and fresh budgets, failures, in-flight change
cancellation, final-only daemon completion, drop and concurrent shutdown. Cancellation
uses the exact job handle; a second state snapshot after pool closure covers racing
submissions. No user callback or captured change destruction runs under state locks.

Three source methods are adapted in solver_manager_cases.rs using the actual
four-entity/four-value generated fixture, constant-zero objective and ten-step local
cap. addProblemChange adds fact 5 in daemon mode and checks the acknowledged consumer;
unknown 999 rejects with its ID; waiting solver uses one worker, actual paused custom
phase then construction, and final delivery. Existing fixture records are extended;
three case records and notices added. Source production implementation untouched.
A native public doctest demonstrates actual solver rebuilding and receipts.

Verification logs use work/live-changes-*. Final results are recorded below.
Goal remains active.

Next lifecycle specifications inspected (test bodies only): terminateEarly expects
one active/two queued jobs, queued removal and synchronous NotSolving after active
termination. Native terminate_early currently signals cooperatively, so do not mark
that method covered just by inserting status assertions before completion. The
active-job method also checks an isTerminatedEarly flag; native handles do not expose
that history. The scheduled-job method requires returning the identical input
problem without solving, whereas native cancellation returns CancelledBeforeStart.
That difference needs explicit API design, not credit for an error result.
submitMoreProblemsThanCpus_allGetSolved runs 2*availableProcessors across three
consumer modes with two real custom assignment phases; its helper/assertions must
be fully read before adapting. It is a promising next bounded manager increment.

Final verification: bash scripts/check.sh passes 1,260 Rust tests (597 adapted,
588 original integration, 75 unit), 58 doctests and 95 Python tests. Release all-target
checks pass the same 1,260 tests; the initialization example delivers all four events
and proves four optima. All nine differential/performance datasets were regenerated
and report current, with successful verification and a matching live freshness digest.
Coverage is 597/4,855 (12.30%), 3,976 pending, 282 excluded and no failed/unverified
methods. Lifecycle is 39/379 (10.29%). Upstream checkout/pin remain unchanged.

Reused the existing dashboard server. Browser checks at 1280x900 and 390x844 verify
597/4,855, 23 benchmark cards, no horizontal overflow, stale/error messages or browser
errors. Lifecycle search shows the three added change methods Passed; the separate
non-daemon BasicPlumbingTerminationTest methods remain Pending. Browser session is
closed; dashboard server remains running. No frontend design changes were made.

Fresh illustrative Rust/Java median milliseconds: exact 6 0.012/3.960, exact 8
0.132/4.849; Great Deluge 32 2.455/8.207, 96 13.387/15.567; stream tabu 96
170.278/26.437 with equal [-44,-484]. List two-opt 16 0.296/8.820 and 32 0.885/10.883
retain the known quality differences. Heuristic validation is still 128/128 equal
scores, stream validation 64/64 equal; Great Deluge differential remains 275/276 with
the documented ratio-boundary discrepancy. These are existing solver workloads,
not live-change throughput measurements; shared-machine timings support no broad
speedup or regression claim. All required work for this increment is verified;
the overall solver goal remains active.

## Owned input cancellation increment

solver_manager.rs now stores an optional owned R separately from its solve closure.
submit_owned moves it to the worker on start; queued cancellation/shutdown returns
that exact R as success, with no Clone requirement or callbacks. Existing opaque
submit APIs retain CancelledBeforeStart. cancel_and_wait and manager
terminate_early_and_wait include callback completion and capture the original run
across ID reuse. is_termination_requested exposes the existing latched token. Scoped
thread-local pool identity rejects unfinished same-pool synchronous waits with
WouldBlock after cancellation; completed/queued-cancelled results remain accessible.
No unsafe code. Direct wait and asynchronous cancel semantics are unchanged.

Six original owned_cancellation tests pass, covering original Arc identity, zero
callbacks/solve on queued cancellation, shutdown with mixed submission modes,
active start-vs-cancel boundary and blocked final consumer, same-pool solver/consumer
WouldBlock, allowed completed and cross-pool waits, ID reuse/history, non-Clone inputs
and error delivery. A public doctest demonstrates exact queued-input recovery.

Four source manager methods are adapted: all three pending cancellation methods and
the CPU-scaled three-consumer-mode batch. Actual custom phases hold active jobs until
cancellation, replacing scheduling races with a deterministic cancellation boundary;
subsequent source CH(/LS) configuration is retained. Queued identity uses Arc::ptr_eq.
The batch runs 2*available_parallelism jobs per mode, two actual custom assignment
phases, checks all initialized results and final notifications, exact final-only
counts and optional first/required final best snapshots. The producer suppresses the
native initial all-unassigned score snapshot, publishing actual assignment improvements.
All source assertions are preserved with stable indices/values replacing e/v codes.
Provenance records four cases plus batch helpers, with explicit adaptations; notices
now count twenty-three manager methods. No upstream production source inspected.

Verification logs use work/owned-cancellation-*. Initial Clippy complexity warning
was fixed by naming the internal SolveTask type. Final results are recorded below.
Goal remains active.

Next lifecycle evidence (test bodies only) suggests managed statistics. The finished
score-count method requires an actual global five-score-call limit yielding exactly
five score calculations/four move evaluations, nonnegative duration and speeds.
The finished problem-size method expects two entities, two variables, two values and
formatted scale "4". The waiting-size method expects four/four/four and scale "256"
while queued behind a paused actual custom phase, then adds a fifth problem fact,
awaits consumer acknowledgement and expects four/four/five and scale "625". This
needs real pre-solve model statistics and revision-aware updates on live jobs, not
hardcoded metadata or statistics derived only after solving. Existing native work
counts may support telemetry; inspect their counting semantics before attribution.
The source errorThrowableInSolver explicitly throws a synthetic OutOfMemoryError;
Rust unwinding-panic recovery does not establish actual allocation-failure recovery.
Keep that method pending unless the adaptation's error-category limits are explicit.

Final verification: bash scripts/check.sh passes 1,270 Rust tests (601 adapted,
594 original integration, 75 unit), 59 doctests and 95 Python tests. Release all-target
checks pass the same 1,270 tests; live_initialization delivers all four events and
proves four optima. All nine differential/performance datasets were refreshed and
report current. Verification is successful; live /api/freshness matches the report
digest. Upstream checkout and pinned revision are unchanged. Coverage is 601/4,855
(12.38%), 3,972 pending, 282 excluded, zero failed/unverified; lifecycle 43/379 (11.35%).

Reused the running dashboard server. Browser checks at 1280x900 and 390x844 confirm
601/4,855, 23 benchmark cards, no overflow, stale/error messages or browser errors.
Lifecycle searches show all three cancellation cases and the CPU-scaled batch Passed.
Unrelated termination methods remain Pending. Closed the browser session; server
remains running. No frontend changes or design skills were used.

Fresh illustrative Rust/Java median milliseconds: exact 6 0.011/3.457, exact 8
0.187/5.896; Great Deluge 32 2.344/7.673 and 96 13.432/14.601; stream tabu 96
165.956/23.057 with equal [-44,-484]. List two-opt 16 0.274/8.842 and 32 0.774/9.165
retain known score differences. Heuristic validation remains 128 valid/128 equal,
streams 64/64, lists 192 valid/189 equal; Great Deluge 275/276 retains its documented
ratio-boundary difference. Existing workloads do not measure owned-job cancellation
latency. Shared-machine timings support no blanket speedup/regression claim. This
increment is verified; the overall solver goal remains active.

## Managed statistics increment

New statistics.rs derives finite/list/mixed model size and BigUint complete-assignment
space. Basic spaces are exact (pins/optional included). List formulas distribute
ordered free values over unpinned suffixes while preserving prefixes/full pins;
optional lists sum subsets. Restricting per-owner free-value ranges marks the count
as an upper bound. Mixed components multiply counts. Native slot counts and disjoint
mixed value namespaces are explicit; empty finite models have no spare fact registry.
No Java class maps, logarithmic/locale formatting or structural constraint scoring.

Job statistics record revisioned immutable size, optional actual SolverWork, overflow
flag, active timing/duration and floored/saturated unsigned speeds. Unknown work is
None. Progress can record cumulative/additive work and revisioned sizes independently
of listeners. Decreases/stale revisions/closed-run reports are rejected. Worker time
starts after started callbacks and freezes before completion delivery. Queued jobs
have zero time/no counters. Existing cancellation/result behavior is retained.

Live submit_with_statistics inspects model metadata before queueing and after each
applied change batch, and accepts LiveRun carrying real result plus SolverWork.
Completed-run work accumulates; daemon waits pause timing. Both measured and original
live jobs exclude idle waits; original jobs retain unknown counters/size. Size is
updated before publication and receipts retain the successful-consumer boundary.
Initial inspector runs on caller before submission; later failures are solve panics.

Eight original job_statistics methods test list-space enumeration, pins/optional/
restricted ranges, basic >u128 counts, mixed bound quality, unknown metrics, queued/
started/consumer/idle timing, counter monotonicity/overflow, revisions, stale publishers,
speed arithmetic and failure. Three source methods now preserve actual five/four
work counts and the finished/waiting size assertions including live scale 256 -> 625.
The existing live fixture helper now also returns native work; its original wrapper
preserves earlier methods. Provenance adds three methods; notices count 26 manager
methods. A public doctest uses real solving and telemetry. Verification logs use
work/job-statistics-*. Final results are recorded below. No upstream production
code inspected; goal remains active.

Next manager source evidence: threadFactoryIsUsed supplies CustomThreadFactory, whose
newThread wraps the runnable in a thread named "CustomThread". A real custom phase
checks the current thread name, then construction runs for four entities/four values
and the final result is non-null. Native managers currently instantiate Builder
threads directly. An injectable native spawn factory receiving worker role/index and
owned runnable would provide the capability (including naming/stack options), rather
than awarding credit for hardcoded renaming. Cover partial-spawn failure cleanup,
solver/consumer dispatch, defaults and own-pool wait protection. Ten manager methods
remain pending: create, errorThrowableInSolver, solveGenerics, three override/time-limit
methods, three builder methods and threadFactoryIsUsed. Source production was not read.
The separate ProblemSizeStatisticsTest remains pending: it additionally asserts Java
entity/variable class maps, unused value ranges on empty models and locale/log-scale
formatting; current native metadata intentionally does not imply those contracts.

Final verification: bash scripts/check.sh passes 1,281 Rust tests (604 adapted,
602 original integration, 75 unit), 60 doctests and 95 Python tests. Release all-target
checks pass the same 1,281 tests; live_initialization delivers all four events and
proves four optima. All nine differential/performance datasets were regenerated and
are current. Report verification succeeds and the live freshness digest matches.
Coverage is 604/4,855 (12.44%), 3,969 pending, 282 excluded, zero failed/unverified;
lifecycle is 46/379 (12.14%). Upstream checkout and pinned revision remain unchanged.

Reused the running dashboard server. Browser checks at 1280x900 and 390x844 verify
604/4,855, 23 benchmark cards, no overflow, stale/error messages or browser errors.
Lifecycle searches show the score-count, finished-size and waiting-size cases Passed.
Browser session closed; server retained. No frontend code or design skills used.

Fresh illustrative Rust/Java median milliseconds: exact 6 0.011/3.838, exact 8
0.147/5.206; Great Deluge 32 2.334/8.777 and 96 13.143/15.181; stream tabu 96
169.030/25.595 with equal [-44,-484]. List two-opt 16 0.358/9.473 and 32 0.997/10.692
retain known score differences. Validation remains 128/128 equal heuristic cases,
64/64 equal stream cases, 192 valid/189 equal list cases and 275/276 Great Deluge
agreement with its documented ratio-boundary difference. Existing benchmarks do not
measure telemetry/inspection overhead. Shared-machine timings support no general
speedup/regression claim. This increment is verified; the overall goal stays active.

## Configurable thread factory increment

Both ordinary and live managers now accept with_thread_factory(workers, consumers,
factory), passing SolverThreadSpec(role/index) and an owned runnable. Calls execute
synchronously on the caller, solver pool then consumer pool, with independent
zero-based indices. Factories may borrow state and configure Builder names/stacks
or wrap execution. Existing constructor names/counts are preserved. Live restarts
reuse workers. Factory errors preserve I/O details; factory-call unwinding panics
become Other errors. Both failure paths close and join already returned handles.
Factories own cleanup of threads started without returning their handles and must
spawn the runnable exactly once rather than execute it inline. Pool TLS guards
remain independent of custom thread names.

Six original thread_factory tests cover actual two-by-two dispatch, invalid counts,
errors at every partial creation position, panics before/after successful creation,
default/custom names with own-pool wait protection, and live daemon reuse. One full
source adaptation checks CustomThread inside a real custom phase followed by CH for
four entities/four values. Provenance records method and helper separately; notices
now count 27 manager methods. A public doctest demonstrates custom name/stack setup.
No production source read, upstream pin unchanged. Logs use work/thread-factory-*.

Nine manager source methods remain pending: create, errorThrowableInSolver,
solveGenerics, solveWithOverride, solveWithTerminationSpentLimit,
solveWithTerminationUnimprovedSpentLimit, testSolveBuilderForExistingSolvingMethods,
solveWithBuilder and solveAndListenWithBuilder. Existing native panic containment
must not be advertised as recovery from actual out-of-memory/process aborts. The
separate ProblemSizeStatisticsTest remains pending for the previously recorded
reflection, empty value-registry and formatting differences.

Next evidence read only from SolverManagerTest: solveWithBuilder requires a delayed
problem finder, final callback and exception callback with successful final delivery;
solveAndListenWithBuilder additionally requires a non-null best-event producer ID.
Current native best callbacks carry only the result, unlike initialization callbacks,
so builder syntax alone cannot earn the listening method. solveGenerics exercises
Java callback variance; choose and document a meaningful native typing contract.
testSolveBuilderForExistingSolvingMethods verifies mock delegation counts for three
convenience overloads; native behavioral tests alone are not that complete method.
The three config-override methods inspect time gradients at 50 ms (0.05 for 1 s,
0.5 for 100 ms), and the exact overridden termination passed to a factory. Native
SolverLimits supports max_time/max_unimproved_time but has no managed default/override
configuration or corresponding job gradient introspection yet. Keep these pending
until the full assertions have meaningful native counterparts.


Final checks: bash scripts/check.sh passes 1,288 Rust tests (605 adapted,
608 original integration, 75 unit), 61 doctests and 95 Python tests. Release all-target
checks pass the same 1,288 tests. The live_initialization example delivers all four
initialization events and proves four optima. Coverage is 605/4,855 (12.46%),
3,968 pending, 282 excluded, zero failed/unverified; lifecycle is 47/379 (12.40%).
Upstream checkout and pinned revision are unchanged.

All nine differential/performance datasets have been refreshed and report current.
Verification is successful and the live /api/freshness digest matches the report.
The running dashboard server was reused. Browser checks at 1280x900 and 390x844
confirm 605/4,855, 23 benchmark cards, no overflow, stale/error messages or browser
errors; the lifecycle search shows threadFactoryIsUsed Passed. Browser closed,
server retained. No frontend edits or design skills were used.

Fresh illustrative Rust/Java median milliseconds: exact 6 0.012/4.183 and exact 8
0.152/6.884; Great Deluge 32 2.569/14.946 and 96 14.748/20.705; stream tabu 96
209.569/28.816 with equal [-44,-484]. List two-opt 16 0.322/8.716 and 32 0.956/14.254
retain the documented score differences. Heuristic validation remains 128 valid/128
equal, streams 64/64 and lists 192 valid/189 equal. Great Deluge remains 275/276 with
its documented ratio-boundary difference. Existing benchmarks do not isolate thread
factory overhead; shared-machine results support no blanket speedup/regression claim.
This increment is verified; the overall independent solver goal remains active.


## Deferred job builder and best-event origin increment

Added SolverJobBuilder through manager.job_builder(id, solve), with owned or deferred
fallible problem source, callback bundle, optional queued model size and consuming
run. Missing source fails before reservation; rejected/unused/queued-cancelled jobs
never look up or solve. Finder executes once on worker after started callback and
inside solving telemetry. Lookup errors/panics follow existing failure delivery.
P and R may differ and need no Clone; P requires Send only. Builder-owned problems
are task captures, dropped on queued cancellation with CancelledBeforeStart; existing
submit_owned retains its separate return-input semantics. Setters replace prior
settings with ordinary Rust drop behavior.

Managed best updates now retain (snapshot, optional SolverEventProducer) together.
publish_from carries native kind/index copied from actual SolverUpdate; on_best_event
receives borrowed SolverBestEvent. Legacy publish sends None; on_best ignores origin.
The two callback setters replace one another. Coalescing, callback-panic containment,
initialization ordering and final delivery otherwise use existing machinery. No
Java producer identity or automatic origin for arbitrary closures is claimed.

Six original job_builder tests cover non-Clone distinct input/result types including
Send-but-not-Sync input, worker/order/one-lookup execution, metadata/work reporting,
missing/duplicate/closed/unused builder behavior, queued cancellation/source disposal,
lookup error/panic recovery and ID reuse, coalesced origin pairing/legacy None/stale
publishers, and attributed-callback panic with final delivery. Two source adaptations
use real CH/LS10, attributed constant scorer and DEFAULT_PROBLEM_FINDER fixture.
Both verify final callback delivery; listening additionally verifies best/origin.
Provenance records both methods and the helper; notices now count 29 manager methods.
Public builder doctest demonstrates distinct input/output and deferred finder.
Logs use work/job-builder-*. Upstream production not read; goal remains active.

Seven manager methods remain pending: create, errorThrowableInSolver, solveGenerics,
solveWithOverride, solveWithTerminationSpentLimit, solveWithTerminationUnimprovedSpentLimit
and testSolveBuilderForExistingSolvingMethods. Java generics and mock overload wiring
are not established by native builder syntax. No claim of actual OOM recovery.
Next substantial capability is managed solver defaults/per-job termination overrides:
existing native max_time/max_unimproved_time limits lack owned job config/introspection.
Pinned test evidence and gradient assertions were recorded in the prior increment.


Additional test-only configuration evidence: core/config/solver/termination/
TerminationConfigTest.java checks overwrite clearing alternative unit fields,
inheritance when a child is unset, child precedence across both matching and different
time units, plus move-count inheritance/isConfigured. Rust durations normalize units;
any future adaptation must preserve these semantics explicitly rather than claim
Java-field parity. SolverFactoryTest also contains XML/resource/class-loader contracts;
ordinary native constructors do not establish those methods. No credit added.


Full verification: bash scripts/check.sh passes 1,296 Rust tests (607 adapted,
614 original integration, 75 unit), 62 doctests and 95 Python tests. Release all-target
checks pass the same 1,296 tests. The live_initialization example delivers all four
initialization events and proves four optima. Coverage is 607/4,855 (12.50%),
3,966 pending, 282 excluded and zero failed/unverified. Lifecycle is 49/379 (12.93%).
The upstream submodule remains unchanged at the recorded pin.


All nine differential/performance datasets were refreshed and report current.
Verification succeeds and the live /api/freshness digest matches the report. Reused
the running dashboard server; browser checks at 1280x900 and 390x844 show 607/4,855,
23 benchmark cards, no overflow, stale/error messages or browser errors. Lifecycle
search finds solveWithBuilder and solveAndListenWithBuilder Passed. Browser closed;
server retained. No frontend edits or design skills used.

Illustrative refreshed Rust/Java median milliseconds: exact 6 0.012/3.955 and exact 8
0.170/5.164; Great Deluge 32 2.368/8.890 and 96 13.746/16.273; stream tabu 96
179.827/27.463 with equal [-44,-484]. List two-opt 16 0.331/9.548 and 32 1.066/10.539
retain documented score differences. Existing workloads do not isolate builder or
origin-publication overhead. Shared-machine timings support no general speedup or
regression claim. This increment is verified; the overall solver goal remains active.


## Reusable solver factory and per-job settings increment

New solver_factory.rs provides SolverFactory<P,R,E,S> with immutable native
SolverSequence defaults and a reusable concurrent solve function. build_manager
creates a native pool; with_manager accepts existing pools including custom thread
creation/consumer counts. ConfiguredSolverManager uses the existing builder/pool,
with explicit shutdown and Deref for ordinary manager methods. Configured job
builders clone settings, accept input/finder/callbacks/size, replace limits/phases,
and set/disable time or unimproved-time limits in call order. Each submission holds
an immutable effective config on ConfiguredSolverJob and injects a fresh job token
into a copy passed to its real solve function. P/R/E need no Clone; P is Send only.
Arbitrary solve closures must forward config to native solving. No Java reflection,
XML config or automatic enforcement of arbitrary code is claimed.

Empty phase vectors and cancellation references in factory/global/each local phase
are rejected before execution. Model-specific validation remains in actual solving.
Configured metadata survives failure, cancellation and ID reuse. Queued cancellation
uses the captured-input builder contract; deferred finders remain uncalled. Pure
SolverLimits::time_gradient uses explicit elapsed/unimproved durations with max/OR,
capping at one, zero -> one and no timed criteria -> None. It omits other limits and
does not infer stagnation from job duration.

Seven original solver_factory tests cover actual move/time/unimproved termination,
whole-limit/phase overrides and setter order, default isolation, validation, concurrent
job cancellation, cloned factory/custom pool reuse, queued config/size/cancellation
and ID reuse, failed reusable functions and non-Clone Send-only inputs, plus time
fractions/zero/disabled/large-duration boundaries. Two full source methods preserve
one-second/100-ms time gradients at 50 ms on real solving. Added fixture attribution
reuses PlannerTestUtils constant-zero CH/LS10 configuration. Notices count 31 manager
methods. Source adaptation waits assert success without requiring a 100-ms job to
finish construction under arbitrary scheduling. Public factory doctest runs a real
native solve. Logs use work/solver-factory-*. No upstream production read.

Five manager source methods remain pending: create, errorThrowableInSolver,
solveGenerics, solveWithTerminationUnimprovedSpentLimit and
testSolveBuilderForExistingSolvingMethods. The unimproved source method checks a raw
override with absent spentLimit and spies on TerminationFactory.create(rawOverride).
Native convenience mutation preserves other effective fields and has no raw Java
patch object/static factory interaction, so do not award that method merely because
unimproved execution works. Other native factories do not establish Java reflection,
XML, overload mock wiring or actual OOM recovery. Overall goal remains active.


Full verification: bash scripts/check.sh passes 1,305 Rust tests (609 adapted,
621 original integration, 75 unit), 63 doctests and 95 Python tests. Release all-target
checks pass the same 1,305 tests. The public factory doctest needed an explicit
String error annotation during development; the final full gate passes. The
live_initialization example delivers all four initialization events and proves four
optima. Coverage is 609/4,855 (12.54%), 3,964 pending, 282 excluded and zero failed/
unverified. Lifecycle is 51/379 (13.46%). The upstream checkout/pin is unchanged.


All nine differential/performance datasets were refreshed and report current;
verification is successful and the live /api/freshness digest matches. Reused the
running dashboard server. Browser checks at 1280x900 and 390x844 confirm 609/4,855,
23 benchmark cards, no overflow, stale/error messages or browser errors. Lifecycle
searches show solveWithOverride and solveWithTerminationSpentLimit Passed while
solveWithTerminationUnimprovedSpentLimit remains Pending. Browser closed; server
retained. No frontend edits or design skills used.

Refreshed illustrative Rust/Java median milliseconds: exact 6 0.010/4.108 and exact 8
0.137/5.943; Great Deluge 32 2.418/8.614 and 96 13.240/15.868; stream tabu 96
168.339/31.034 with equal [-44,-484]. List two-opt 16 0.273/8.193 and 32 1.098/13.143
retain documented score differences. Existing workloads do not isolate factory or
configuration-copy overhead. Shared-machine results support no general speedup or
regression claim. This increment is verified; the overall solver goal remains active.


## Pillar move and neighborhood increment

New pillar.rs implements native Pillar groups with one selected slot per member or
explicit aligned multicolumn rows. Validate nonempty dimensions, duplicate slots,
column uniformity, bounds, matching swap widths and disjointness before constructing
ordinary Composite moves; both signatures are captured before any writes. Unequal
member counts are supported. Canonical row ordering ignores member draw order while
preserving column significance. Raw snapshot construction allows no-ops/out-of-range
values; native validated application/score directors enforce ranges and pins.
Snapshots must be regenerated when the starting assignment changes.

Opt-in Neighborhood::Pillar groups all unpinned equal-valued basic slots, including
None; filters change/swap targets against every member's range and optionality; and
lazily clones only the emitted candidate's values. O(n) grouping storage and worst-
case O(n²) equality work precede candidates. Shared boxed provider dispatch serves
both basic scoring backends and mixed basic/joint enumeration. List components use
ordinary changes/optional swaps. Defaults retain existing behavior. No inferred
entity schema or subpillar enumeration is supplied.

Six original integration tests cover real group/mixed-joint traps in both backends,
seeded orders/first-best/budgets, an independent 36-state optional/ranged/pinned
successor oracle, malformed/nonuniform groups, atomic rollback/rejection/score
consistency and lazy clones. Six source methods cover four built-in PillarSwapMove
state/undo cases and two SelectorBasedPillarChangeMove range/raw-execution cases.
The raw execution source includes no-ops and a target outside the third member's
range: preserve that through raw Assignment transactions, without a safety claim.
All six methods and four helper/fixture sources have provenance/notices. No source
production code was read. Rebase, Java descriptions/getters/equality, subpillar
providers and mutated-source range-assertion cases remain pending. Whole-goal work
continues after this increment. Logs use work/pillar-*.

Full debug gate: bash scripts/check.sh passes 1,317 Rust tests (615 adapted,
627 original integration, 75 unit), 64 doctests and 95 Python tests. Coverage is
615/4,855 (12.67%), 3,958 pending, 282 excluded, no failed/unverified methods.
The upstream pin and checkout are unchanged. Initial development fixed a borrowed
iterator lifetime and a Clippy filter-map style finding before the successful gate.

Next useful follow-up: independent cross-language whole-pillar workloads and
explicit subpillar selection policies, while retaining pending credit for Java-only
metadata/identity assertions. Existing benchmarks select default neighborhoods and
do not isolate pillar-search performance or boxed-provider overhead.

Release all-target verification passes the same 1,317 tests. The live_initialization
example delivers all four events and proves four optima. Stream comparison cases
and all 512 list-move cases pass; list solving retains 192 valid/189 equal-score
cases, and stream search retains 64 valid/64 equal-score cases. These preserve the
previously documented behavioral differences; no new parity claim is added.

All nine differential/performance datasets were refreshed and report current, with
successful verification and a matching live /api/freshness digest. Browser checks
at 1280x900 and 390x844 show 615/4,855 (12.67%), 23 benchmark cards, no overflow,
stale/error messages or browser errors. Moves/selectors show 89/874 (10.18%); searches
show the six adapted pillar methods Passed and remaining pillar methods Pending.
Reused the dashboard server and closed the verification browser. No frontend edits
or design skills were used.

Illustrative refreshed Rust/Java median milliseconds: exact 6 0.010/4.063 and exact 8
0.144/5.341; Great Deluge 32 2.442/9.248 and 96 14.974/15.365; stream tabu 96
174.475/24.580 with equal [-44,-484]. Two-opt 16 0.271/9.539 and 32 0.976/10.685
retain known score differences. All measurements are workload-specific and use the
existing default neighborhoods; no pillar-specific speed claim or general speedup/
regression inference is supported. This increment is verified; the full rewrite
remains unfinished and the goal stays active.


## Subpillar selection and search increment

New subpillar.rs provides validated Copy SubPillarSelection policies for bounded
combinations and consecutive windows. Enumerate increasing size then lexicographic
positions, with max clipped to group size, undersized groups empty and O(k) current
indices. No bitmask/power-set allocation or binomial count; usize::MAX upper bounds
work. Pillar::subpillars preserves aligned columns and uses canonical member-row
order; moves still validate current uniformity. No inferred entity schema.

SubPillarConfig exposes selection, PillarChangeMode (Disabled/AssignedOnly/All/
UnassignOnly), and optional independent swap_selection. new enables All and symmetric
swaps. Native public subpillar_moves validates the model and snapshots the existing
unpinned equal-value groups. Changes scan each subset's first-member domain, not
its full group's first-member domain. None is allowed only for optional selected
members. Equal swap policies visit unordered groups; different policies visit both
orientations. Reject groups below either minimum before expanding subsets, avoiding
an exponential scan for a large group paired with an undersized singleton. Products
and cloned candidate values are lazy, with no whole-space deduplication. Filtering
otherwise may still scan many rejected subsets and is not preemptible by candidate
budgets. Finite enumeration does not promise Java random-sampler parity.

Neighborhood::SubPillar feeds both basic scoring backends and mixed basic/joint
search; list components keep ordinary changes and enable swaps iff swap_selection
is Some. Basic change mode does not restrict list moves. ShuffledVariables changes
encounter order (including consecutive membership); ShuffledMoves keeps the existing
seeded bounded shuffle. Direct provider uses index order. Group helpers were shared
with whole-pillar enumeration without changing its behavior.

Thirteen adapted source methods cover five change provider assertions, all four
unassign provider methods and four swap provider methods. Preserve all source
assertions/sample caps through finite deterministic subsets; stable slots replace
Java object identities. Four testdomain fixtures are attributed, with notices.
Pending: crossingNull constructor validation (native heterogeneous slots filter per
member), shared sampler reset/identity behavior, Java constructor variants and
automatic multivariate provider cases. Native explicit multicolumn subsets do not
justify these credits. No production upstream source was read.

Six new original methods cover bitmask subset order/bounds, independent successor
sets with pins/domains/None/asymmetric swaps, partial-group local-optimum escape in
both backends under seeded orders/selections/evaluation caps, consecutive order and
subset-local domains, large lazy candidate cloning and early undersized swap-pair
rejection. Existing mixed-joint trap also runs subpillar settings. A public doctest
shows enumeration. During development, the write helper needed an explicit shared
lifetime; review caught undersized swap-pair expansion before release. The in-flight
old-source gate rejected its stale fingerprint after the fix, as intended; final
verification reruns against frozen sources. Logs use work/subpillar-*.

Next useful follow-up: dedicated Java/Rust pillar/subpillar workloads and automatic
multivariate schema selection; existing default-workload benchmarks do not establish
pillar-specific performance. The overall solver goal remains active.

Final debug gate passes 1,336 Rust tests (628 adapted, 633 original integration,
75 unit), 65 doctests and 95 Python tests. Coverage: 628/4,855 (12.94%), 3,945 pending,
282 excluded and no failed/unverified methods. Moves/selectors: 102/874 (11.67%).
The pinned upstream checkout is unchanged.

Release all-target tests pass the same 1,336 cases. The live_initialization example
delivers all four initialization events and proves four optima. All production/source
changes were frozen before these checks and the subsequent comparison refresh.

All nine comparison datasets were refreshed and are current; verification succeeded
and the live /api/freshness digest matches. Browser checks at 1280x900 and 390x844
show 628/4,855 (12.94%), 23 benchmark cards, no overflow, stale/error messages or
browser errors. SubPillar search shows all thirteen adapted methods Passed and the
remaining config, constructor, sampler and multivariate methods Pending. Reused the
running dashboard server and closed the verification browser. No frontend edits or
design skills were used.

Existing outcomes remain: 512/512 list-move cases, 192 valid/189 equal-score list
solves, 64 valid/64 equal-score stream solves and 128 valid/128 equal-score heuristic
solves. Great Deluge retains its documented 275/276 Java agreement. Illustrative
Rust/Java median milliseconds: exact 6 0.012/4.120, exact 8 0.145/5.245; Great Deluge
32 2.392/9.180, 96 15.383/14.700; stream tabu 96 180.007/24.737 with equal [-44,-484].
List two-opt 16 0.336/9.121 and 32 1.081/13.573 retain their documented score
differences. Default-workload timings do not isolate the new subpillar provider,
and shared-machine measurements support no general speedup/regression claim.
This increment is verified; the full rewrite remains unfinished and active.

## Multivariable pillar schema increment

New pillar_schema.rs provides PillarSchema with Arc-shared canonical rows of aligned
selected slots. Reuse Pillar validation for nonempty/rectangular/disjoint shape;
FiniteProblem::with_pillar_schema validates bounds before attachment. Cloning retains
metadata; without_pillar_schema removes it, while rebuilding a new model requires
reattaching desired metadata. Ordinary construction/exact/non-pillar behavior stays
slot-based. No Java annotation/metamodel inference is introduced.

Attached schemas drive existing Pillar and SubPillar neighborhoods in both basic
scoring backends and mixed basic/joint search. Group keys compare all selected
columns, including None; unselected columns affect neither keys nor writes. A selected
pin excludes its whole row, while an unselected pin does not. Row order follows first
selected-slot encounter in variable order, including shuffles. Changes update one
selected column at a time. Swaps exchange every selected column and reject the whole
pair if any cross-value fails a domain/optionality check. A pure pillar_swap_moves
entry point emits only whole-group swaps; without metadata it uses single-slot rows.

Schema swaps retain both directed orientations even for equal subset policies. Old
no-schema pillar/subpillar paths retain prior unordered/asymmetric behavior. Bounds
and group pairing remain lazy; reject undersized partners before expanding subsets.
O(variable count + row count) grouping indices and worst-case O(rows² × columns)
comparison work precede candidates. Only emitted candidate values are cloned. No
hard responsiveness guarantee for long grouping/rejected-subset scans is claimed.

Eleven source adaptations: eight whole PillarSwapMoveProviderTest methods and three
multivariable SubPillarSwapMoveProviderTest methods. Both expected directed movements
are independently built/asserted where required; other checks preserve memberships,
pins, mixed-column illegality, singleton/unequal groups and excluded-column merging.
Six helper/testdomain fixture sources are attributed alongside all methods/notices.
Source constructor overload/empty-list message and shared sampler lifecycle assertions
remain pending. No production upstream source was read. Native explicit schemas do
not claim Java ABI, reflection or random-stream compatibility.

Six original tests cover schema validation/bounds/clone/removal/selected pins,
exhaustive 256 binary assignments under whole/subset masks, domain restriction,
selected columns and pin policies; per-column changes/optional targets; both basic
backends escaping a tuple-group trap under seeded orders with score rollback; both
mixed backends needing joint tuple/list moves; lazy clone counts and huge undersized
swap partner pruning. One public doctest shows schema attachment and pure swaps.
Focused tests and Clippy passed before freezing sources for the full gate. Logs use
work/pillar-schema-*. Next useful work: dedicated Java/Rust pillar and subpillar
workloads and broader domain/shadow-variable support. The full solver goal remains
active and unfinished.

Full debug gate passes 1,353 Rust tests (639 adapted, 639 original integration,
75 unit), 66 doctests and 95 Python tests. Coverage: 639/4,855 (13.16%), 3,934 pending,
282 excluded, no failed/unverified methods. Moves/selectors: 113/874 (12.93%).
The upstream pin and checkout are unchanged.

Release all-target checks pass the same 1,353 tests. The live_initialization example
delivers all four initialization events and proves four optima. Sources were frozen
before the full gate, release run and comparison refresh.

All nine differential/performance datasets were refreshed and report current, with
successful verification and a matching live /api/freshness digest. Browser checks
at 1280x900 and 390x844 show 639/4,855 (13.16%), 23 benchmark cards, no overflow,
stale/error messages or browser errors. Whole pillar swap provider shows eight
methods Passed, two constructor methods Pending; subpillar swap provider shows
seven Passed, three constructor/sampler methods Pending. Reused the dashboard
server and closed the verification browser. No frontend edits or design skills.

Existing comparison outcomes persist: 512/512 list-move cases, 192 valid/189 equal-
score list solves, 64 valid/64 equal-score stream solves and 128 valid/128 equal-
score heuristic solves; Great Deluge retains the known 275/276 Java agreement.
Illustrative Rust/Java median milliseconds: exact 6 0.012/4.119, exact 8 0.166/5.659;
Great Deluge 32 2.533/9.431, 96 13.949/16.505; stream tabu 96 174.402/23.887 with
equal [-44,-484]. Two-opt 16 0.336/9.587 and 32 0.904/12.433 retain known score
differences. These default-workload timings do not isolate pillar schema overhead
or establish a general speedup/regression. This increment is verified; the whole
rewrite remains unfinished and the goal stays active.

## Explicit-group pillar measurements (2026-10-03)

Added original Java public Moves/MoveTester and native Pillar/FiniteProblem
transaction adapters plus `scripts/benchmark_pillars.py`. All 128 seeded cases
match independent Python trial, restored and committed matrices and batch
checksums. Cases include optional values, no-ops, noncontiguous/unequal supplied
groups, subgroups and one to three selected columns. Seven Python regressions
check simultaneous writes, untouched cells, invalid groups, response completeness,
state/checksum integrity and deterministic generation. No upstream method credit.

Six timing workloads cover single-column change/swap and three-column swap, at
16/64 entities. Every iteration constructs groups/moves, applies temporarily,
copies/checks trial and restored states and consumes checksums. Public Java
MoveTester infrastructure and native structural/domain validation costs differ.
Automatic grouping/schema-provider enumeration, sampling, scoring and search are
outside scope. Setup and separate committed-state checks are outside the timer.
Defaults: 1,000 transactions per batch, 5 warmup batches, 7 samples, 3 fresh
processes per runtime, alternating runtime order. All raw observations retained.

Debug gate: 1,353 Rust tests, 66 doctests and 102 Python tests pass. Release all-
target run passes the same 1,353 tests; live_initialization example succeeds.
Source digest frozen before full checks and all ten dataset refreshes. Adapted
coverage stays 639/4,855 (13.16%), 3,934 pending, 282 excluded, zero failed or
unverified. Upstream unchanged. Logs use work/pillar-performance-*.

All ten datasets current; report verification successful and live freshness digest
matches. Median Rust/Java ms per transaction: change16 0.000853/0.003522,
change64 0.003638/0.009293, swap16 0.001476/0.004526, swap64 0.005852/0.021092,
tuple16 0.002506/0.011565, tuple64 0.008867/0.045663. These are batch averages with
framework/validation differences, not general solver speedups. Existing evidence
retains 512/512 list moves, 192 valid/189 equal-score list solves, 64 valid/equal-
score stream solves, 128 valid/equal-score heuristic solves and the known 275/276
Great Deluge agreement. Stream tabu96 remains slower: Rust 173.489 vs Java 24.085
ms, equal [-44,-484]. Exact8 0.177/5.768 ms. Quality differences remain visible.

Dashboard reuses existing patterns, adds six pillar cards (29 total), independent
case counts and scope/method/environment details. Browser at 1280x900 and 390x844
confirms current data, correct per-transaction caption, no overflow/errors/stale
messages. Visual inspection caught wrapping timing units; widened only pillar
numeric columns, refreshed CSS/JS asset version hashes, and confirmed 90px nowrap
columns and no clipping on both viewports. Existing server retained; verification
browser closed. No Impeccable or design delegation.

Next: broader domain/shadow-variable support or remaining pillar provider behavior
with full test assertions, without treating these explicit-group transaction
measurements as automatic schema-provider evidence. Full rewrite goal active.

## Derived reference streams (2026-10-03)

Added independent `ReferenceNode` and `StreamGraphBuilder::follow_reference`.
An optional right-row identity follows each left payload. Outputs preserve left
identity and `(left, Option<right>)`, including missing targets. Optional target
payloads distinguish Some(None) from no target. Persistent left/target/reverse
membership maps stage changes atomically and join graph checkpoints, rollback and
saved-candidate installation. Right changes emit only current dependents in ID
order. Chains derive fields such as entity.group.owner; graph topology remains
acyclic, and self/cyclic payload references have finite one-hop semantics.

Adapted the complete BasicInverseShadowVariableTest method with all five source
assertions and four fixture attributions. Native stable IDs and stream-derived
owner codes replace Java object identity, annotations and hidden inverse supply.
No reflection/internal-implementation equivalence claim. Nine original regressions
cover 2,000 generated changes against a scan, selective dependents, mapping/clone
panic safety, chained and same-source references, composite basic move undo/commit,
list previous-element views, foreign ports, construction and saved local-search
candidates compared with independent full scoring. One public doctest added.
Automatic shadow dependency discovery, recursive fixed points, Java inconsistency
rules and full declarative shadow compatibility remain unimplemented.

Full debug gate passes 1,363 Rust tests (640 adapted, 648 original integration,
75 unit), 67 doctests and 102 Python tests. Release all-target run passes the same
1,363 tests; live_initialization delivers all four events and proves four optima.
The large release solver test target took several minutes to compile; its live
compiler was observed and allowed to finish without restarting. Logs use
work/reference-stream-*. Upstream pin and checkout unchanged.

All ten differential/performance datasets refreshed and current, successful report
verification and matching live freshness digest. Coverage 640/4,855 (13.18%),
3,933 pending, 282 excluded, zero failed/unverified. Domain model is now 1/455
(0.22%). Browser at 1280x900 and 390x844 shows the new method Passed, 29 benchmark
cards, no overflow, stale messages or errors. Existing server reused; verification
browser closed. No frontend edits, design skill or delegation.

Existing comparison outcomes persist: 512/512 list moves; 192 valid/189 equal-score
list solves; 64 valid/equal-score stream solves; 128 valid/equal-score heuristic
solves; 128/128 pillar transactions; known Great Deluge agreement 275/276. Stream
tabu96 remains slower at Rust 171.985 vs Java 27.091 ms with equal [-44,-484].
These refreshes do not benchmark the new reference operator. No new Java generated
reference oracle is claimed: the new evidence is the pinned adapted source method,
native scan/transaction/search regressions and doctest.

Next: extend native derived-domain/shadow behavior using the pinned declarative
shadow tests and public APIs, including a direct Java/native generated reference
comparison where useful. Full rewrite goal remains active and unfinished.

## Derived dependency evaluation (2026-10-03)

Added independent immutable `DependencyGraph<K>` with deterministic evaluation,
fixed prerequisite validation and iterative cycle detection. Dynamic edges merge
with fixed edges; cycles and their downstream dependents produce inconsistent
values without invoking suppliers. Ordinary unavailable values remain distinct
from inconsistency. Removing a dynamic cycle recovers on the next evaluation.
Inputs are sorted and deduplicated; malformed edges fail before callbacks run.
The plan supports non-Clone outputs and is reusable after a callback panic.
Each evaluation recomputes the whole schedule: no incremental shadow cache or
mutable entity-field installation is claimed.

Adapted three complete DependencyValuesShadowVariableTest methods: testSolve,
testLoopStatusOfEntityIsUpdatedEvenIfNoVariablesOnTheEntityChanged, and
failsIfTwoValuesDependOnEachOther. Explicit native list-owner/predecessor views
and exact integer-minute scheduling replace Java annotations. Both scoped and
stream list solvers obey the native 1,000-move cap and verify all source dependency
ordering assertions. Cycle insertion and undo preserve every source null-value
assertion with additional inconsistency and score-restoration checks. Fixed cycles
fail eagerly at native configuration, rather than Java solve time; this lifecycle
deviation is explicit in provenance and documentation. All four source fixtures
and the tested diagnostic prefix are attributed. Upstream remains unchanged.

Six original regressions cover all 512 directed three-node graphs against an
independent reachability oracle for fixed cycles and dynamic propagation, missing
values, deterministic inputs, metadata errors, a 20,000-node chain, non-Clone
outputs and panic recovery. Full debug gate passes 1,372 Rust tests (643 adapted,
654 original integration, 75 unit), 68 doctests and 102 Python tests. Release
all-target verification passes the same 1,372 tests; live_initialization delivers
all four events and expected optima. The large release solver target was allowed
to finish its several-minute compilation without restarting. Logs use
work/dependency-graph-*. Source and provenance were frozen before verification.

All ten comparison datasets refreshed and current; report verification succeeds
and live freshness digest matches. Coverage is 643/4,855 (13.24%), 3,930 pending,
282 excluded, zero failed/unverified. Domain model is 4/455 (0.88%). Browser checks
at 1280x900 and 390x844 confirm all three new methods Passed, 29 benchmark cards,
no overflow, stale messages or browser errors. Existing server retained and
verification browser closed. No frontend edits or design skill this increment.

Existing comparison outcomes persist: 512/512 list moves; 192 valid/189 equal-score
list solves; 64 valid/equal-score stream solves; 128 valid/equal-score heuristic
solves; 128/128 pillar transactions; known Great Deluge agreement 275/276. Stream
tabu96 remains slower at Rust 178.873 vs Java 26.908 ms with equal [-44,-484].
These comparisons do not measure the new DependencyGraph; no direct Java generated
dependency oracle was added. Full rewrite goal remains active and unfinished.

Next: extend missing declarative domain behavior or add direct Java/native
dependency evidence. FollowerValuesShadowVariableTest was inspected, but its
fixtures have not yet been read or adapted. Its testUpdateOnce requires one
supplier call for A1 and none for A2/A3/B1/B2 after changing leader A, so matching
follower values alone is insufficient: shared supplier computation must be
preserved. AlwaysLoopedShadowVariableTest also remains pending; read its fixtures
before adapting its odd/even fixed-loop diagnostic assertions.

## Shared follower suppliers (2026-10-03)

Added `StreamGraphBuilder::follow_shared_reference`, independently composed from
native persistent keyed collections and reference indexes. One supplier result is
broadcast to all followers sharing an explicit target ID. The smallest native tuple
ID represents the group; a target update invokes that supplier once, and an
unreferenced target invokes none. Membership changes recompute affected groups.
Missing targets pass None to the supplier. Group equality is a caller promise;
Java's null alignment-key exception is not inferred (ordinary Rust None keys would
share a group). Intermediate routing can retract/reinsert outputs; callbacks must
be pure. Transaction checkpoints and saved candidates restore all caches without
supplier replay. No annotation discovery or recursive dependency inference added.

All three FollowerValuesShadowVariableTest methods and five follower fixtures are
attributed. Preserve all fifteen change/null assertions and exact A1-once,
A2/A3/B1/B2-never supplier observations after initialization. The solve fixture is
THREE leaders, EIGHT followers, TWO values, not three values/eight leaders. Verified
Random(0).nextInt(3) sequence with local JDK 21: [0,1,1,2,2,2,2,0]. Native stream
construction/search uses a shared 1,000-move limit and full score checks. Retained
assignment reconstruction verifies every source follower/leader equality; no
Java search trace or optimality claim. Null shadow followers count in scoring,
while null genuine leader variables are excluded, per pinned public forEach docs.
Additional scores are -25, -20, -25, -34 with native structural deficits -3,-2,-1,0.
An initial added score assertion omitted that structural component; it failed and
was corrected before the successful full gate.

Six original regressions check 1,500 generated membership/target changes against
full scans, representative replacement, empty groups, missing targets, exact
supplier counts, rollback without replay, panic recovery, finite same-source
cycles, foreign ports, and construction/saved local-search candidates against
independent direct scoring. These earn no method credit. Debug gate passes 1,381
Rust tests (646 adapted, 660 original integration, 75 unit), 69 doctests and 102
Python tests. Report coverage is 646/4,855 (13.31%), 3,927 pending, 282 excluded,
zero failed/unverified. Domain model is 7/455 (1.54%). Logs use
work/shared-reference-*. Source/provenance frozen before the successful gate.

Next: DynamicFollowerValuesShadowVariableTest and all five dynamic_follower
fixtures were read. Three pending methods: solve 3 leaders/8 initially unassigned
followers/2 values under 1,000 moves; leader-value changes with nine assertions;
follower leader reassignment with nine assertions including unassignment to null.
Unlike static followers, each dynamic follower has a genuine leader variable:
forEach filters an unassigned leader reference, but retains an assigned follower
whose leader's value is null. The fixture has no alignmentKey, so do not substitute
shared supplier semantics for per-follower evaluation. Native follow_reference
plus map is the relevant starting point. Consider a durable public-API Java/native
generated follower/dependency comparison, beyond the current native scans. Existing
benchmarks do not isolate this operator. Full rewrite goal remains unfinished.

Release all-target verification passes all 1,381 tests; live_initialization delivers
all four events before completion and proves four optima. The large solver release
target took several minutes to compile; its live compiler was observed and allowed
to finish. All ten comparison datasets refreshed and current, report verification
successful and live freshness digest matching. Existing outcomes persist: 512/512
list moves, 192 valid/189 equal-score list solves, 64 valid/equal-score stream solves,
128 valid/equal-score heuristic solves, 128/128 pillar transactions and the known
275/276 Great Deluge agreement. Stream tabu96 Rust/Java 168.371/25.034 ms with equal
[-44,-484]; exact8 0.175/5.756 ms. These are existing workload measurements, not a
shared-supplier benchmark or a general performance claim.

Dashboard checked at 1280x900 and 390x844: 13.31%, 646/4,855 methods, domain 7/455,
all three static follower methods Passed and the three dynamic follower methods
still Pending. All 29 benchmark cards render; no overflow, stale messages or
browser errors. Existing server retained, verification browser closed. No frontend
edits, Impeccable use or delegation. Full rewrite goal remains active.

## Dynamic followers and Java follower evidence (2026-10-03)

Three complete DynamicFollowerValuesShadowVariableTest methods now pass through
existing native reference streams: three leader-value changes (nine assertions),
two reference changes plus null unassignment (nine assertions), and a fully
unassigned three-leader/eight-follower/two-value solve with shared 1,000-move budget
and full score assertions. Final native snapshots preserve all source follower/
leader equalities. No production library changes were needed. All five fixture
attributions are recorded; stable slots replace Java object identity and explicit
graph projections replace externally stored shadow fields.

Added durable public Java/Rust follower adapters and scripts/follower_checks.py,
integrated into differential_streams.py. All 128 cases (65 dynamic, 63 fixed/shared),
2,373 observed states and 3,714 primitive operations agree with independent Python
reference values and scores. Include zero followers, null leader values, null
references, shared and unreferenced targets, repeated writes, no-ops, composite
commits and temporary moves with undo. Java public MoveTester updates annotated
shadows; SolutionManager.UPDATE_SCORE_ONLY recomputes each observed score. Rust
uses persistent follow_reference/follow_shared_reference projections. Every leader
value, reference, derived value, ordinary score and completeness flag is checked.
Shared null-reference values agree, but Java null-alignment supplier-call counts
are NOT compared or claimed. Java structural scores, incremental scoring timing,
search quality and general reflection parity are outside these generated checks.

IMPORTANT correction to the preceding handoff: the pinned fixture's interface
source forEach(HasValue.class) includes null genuine variables as well as null
shadow values. Generic concrete-entity filtering was an incorrect assumption.
Direct Java initially returned -20 where the old native/Python assumption gave -8.
Both native fixtures now group all interface rows, and all generated observations
match Java. The static source fixture scores are -64,-32,-26,-34 (structural
components remain -3,-2,-1,0), replacing the previous -25,-20,-25,-34 assertions.
The original value and shared supplier-count assertions still pass. Keep the
interface/concrete-source distinction explicit when adapting subsequent fixtures.
Initial adapter setup also required correcting its work/oracle/solver classpath
and adding normal bean setters; no upstream production implementation was read.

Seven new Python regressions check null-row scoring, fixed versus genuine-reference
completeness, sequential writes, undo without aliasing, bad indices/domains, corrupt
or incomplete responses, and deterministic generation. All fixtures used in both
adapters are attributed, as is the reused static testMove boundary in the Python
case generator; other generated inputs earn no additional method credit. Dashboard
adds one evidence entry using its existing presentation and refreshes the JS asset
hash. No design skill or delegation used.

Debug gate and release all-target verification both pass 1,384 Rust tests (649
adapted, 660 original integration, 75 unit); 69 doctests and 109 Python tests pass.
Live initialization delivers all four events before completion and proves all four
optima. Source/provenance frozen before successful full verification; logs use
work/dynamic-follower-*. Coverage 649/4,855 (13.37%), 3,924 pending, 282 excluded,
zero failed/unverified. Domain model now 10/455 (2.20%).

Next: FollowerSetValuesShadowVariableTest and both follower_set fixtures were read.
It has two pending methods: 3/8/2 solve with seed-0 distinct pairs of leader IDs,
and nine assertions on AB/AC/BC followers after changing A/B/C. Supplier selects
the minimum non-null value by Java string code, not numeric identity; all-null
returns null. Its source list is nonempty and can be represented with indexed
membership joins plus grouped minima; preserve empty/missing and duplicate behavior
explicitly if extending beyond the fixture. It reuses the fixed follower interface
constraint and leader fixture, so keep all null-valued interface rows in scoring.
A generated public Java multi-reference comparison would strengthen this next
increment. The complete rewrite remains unfinished and the goal stays active.

All ten differential/performance datasets refreshed and current, with successful
report verification and a matching live freshness digest. Existing results retain
512/512 list moves, 192 valid/189 equal-score list solves, 64 valid/equal-score stream
solves, 128 valid/equal-score heuristic solves and 128/128 pillar transactions.
Default workload timings remain scoped measurements; the new follower comparison
contains no timings. Exact8 Rust/Java was 0.168/5.658 ms in the final refresh.

Browser checked at 1280x900 and 390x844: 13.37%, 649/4,855 methods, domain 10/455,
all three dynamic follower methods Passed, 29 benchmark cards, and the separate
128/128 follower evidence entry with 2,373 states/3,714 operations and its limitations.
No overflow, stale messages or browser errors. Existing server retained; verification
browser closed. This increment is verified; the complete rewrite remains active.

Final attribution review clarified the copied static boundary in adapter/script
headers. Full checks, release tests, all ten dataset refreshes and both dashboard
viewports were repeated after that source-fingerprint change; all pass/current.

## Collection follower conformance increment

Adapt both FollowerSetValuesShadowVariableTest methods with complete original
assertions: nine change assertions and every final supplier value in the 3/8/2
solve. Exact Random(0) distinct leader pairs were verified with JDK 21. The native
solve uses a shared 1,000-move cap and full-score consistency assertions. No
production implementation changed: a reusable attributed test/example fixture
combines existing membership joins, conditional ordered collection and references.
Keys use UTF-16 code units plus first-reference position, preserving Java string
order and distinct value identities with equal codes. All-null follower rows remain
in the interface-based count-squared score. Two original native tests cover
Unicode/ties, duplicate references, null transitions, undo and search/full-scoring
agreement.

A separate public Java/Rust/Python collection comparison passes 128/128 cases,
2,368 states and 3,724 operations. Java MoveTester updates annotated fields;
SolutionManager.UPDATE_SCORE_ONLY recomputes observed ordinary scores. Independent
Python validates every leader and derived identity, completeness flag and score.
Include zero followers, repeated references, empty/duplicate/Unicode codes,
all-null groups, repeated writes, composite commits and trial/undo. Dependencies
are fixed nonempty collections; code strings are immutable. No empty/null
collection, PlanningId, supplier-count, structural-score, search-quality or timing
claim. Generated inputs are original and earn no extra upstream-method credit.
Seven Python regressions cover boundaries and corrupt/missing/reordered responses.

Debug gate and release all-target checks pass 1,388 Rust tests (651 adapted, 662
original integration, 75 unit); 69 doctests and 116 Python tests pass. Live
initialization delivers all four events before completion and proves all four
optima. Source/provenance frozen before final verification; logs use
work/collection-follower-*. Coverage is 651/4,855 (13.41%), 3,922 pending,
282 excluded, zero failed/unverified. Domain model is 12/455 (2.64%).

Next: ListElementShadowVariableTest has been fully read (fixtures not yet read).
Six methods cover list-element end-time chains and owner last-end aggregate,
empty-list aggregate fallback, composite list moves, basic duration changes
retriggering an owner aggregate without list changes, and full-assert solves from
initialized and uninitialized assignments. Read all seven fixtures in
core/testdomain/shadow/list_element before implementation. A generated public
comparison should cover list changes and basic/list coupling with independent
reference values. Preserve the interface/concrete-source null-filter distinction
from the previous increment. The full rewrite remains unfinished and active.

All ten differential/performance datasets refreshed and current, successful report
verification and matching live freshness digest. Existing comparisons retain
512/512 list moves, 192 valid/189 equal-score list solves, 64 valid/equal-score
stream solves, 128 valid/equal-score heuristic solves and 128/128 pillar cases.
Exact8 Rust/Java was 0.162/5.025 ms; stream tabu96 was 174.222/27.575 ms. These
remain scoped measurements; the new collection comparison contains no timing.
Browser checked at 1280x900 and 390x844: 13.41%, 651/4,855, domain 12/455, both
collection follower methods Passed, 29 benchmark cards, separate 128/128 evidence
with 2,368 states/3,724 operations. No overflow, stale messages or browser errors.
Existing server retained; verification browser closed. No design skill/delegation.

## List-element derived values and aggregate conformance increment

Adapt all six ListElementShadowVariableTest methods, preserving every source
assertion. Reusable attributed support in examples/list_element_support combines
existing stream operators; no production implementation changed. Ordered owner
maps rescan touched lists from fixed start times, cache element start/end pairs
and last ends, and reference projections retain every assigned/unassigned value.
Empty owners retain their fixed start; optional list semantics are preserved.
Mixed graphs map optional genuine durations to padded values and use membership
joins plus nullable grouped sums; null members produce None, empty owners Some(0).
Basic-only changes update aggregates without list events. Both source solves use
original starts/durations/initial lists, a shared 1,000-move cap and full checks.
All seven upstream fixtures were read and appropriate local adaptations attributed.

Three original Rust regressions check complete derived snapshots and scores after
remove-all/transfer/undo, null versus empty aggregates and mixed composite undo,
and bounded optional-list search against an independent full-scoring network.
Seven Python tests check explicit boundaries, sequential composites, snapshot
isolation, invalid operations, deterministic generation and corruption in every
output field, as well as missing/reordered output. No extra method credit.

Public Java/Rust/Python comparisons pass 128/128 cases (64 ordered, 64 mixed),
2,365 states and 3,675 operations. Compare ordered owner/previous/start/end and
last-end aggregates, mixed padded durations and nullable totals, lists/durations,
and ordered ordinary scores. Generated cases include zero values, empty owners,
initial unassigned values, assign/relocate/swap, ordered unassign, basic-only
changes, sequential composites, commits and trial/undo. Java rejects same-position
relocation, so generated relocations/swaps use distinct positions. Same-position
native no-ops are not a Java compatibility claim.

Java adapter lessons: automatic SolutionManager.updateShadowVariables discovery
failed for zero-value fixtures (collection source references no variables); public
MoveTester with explicit entity metadata handles these fixtures. UPDATE_SCORE_ONLY
returned zero ordinary score for this declarative list fixture even when visible
shadows were correct. A full UPDATE_ALL gives the expected score. To avoid hiding
stale MoveTester shadows, ordered scores now use a separately rebuilt solution
from genuine/fixed inputs, with no object/list aliases to the observed solution.
Original MoveTester-derived values are observed untouched. Mixed source fixtures
have no scoring provider; their zero protocol field is a placeholder, not scoring
evidence. No Java supplier-count, structural-score, reflection, search-quality or
timing parity claimed. Negative/overflow timing inputs are outside generated
comparisons, though native fixture addition uses explicit Java-style i32 wrapping.
Public score-analysis docs identify that API as Enterprise; it was not invoked.
No upstream production implementation was inspected or copied.

Debug gate and release all-target tests pass 1,397 Rust tests (657 adapted, 665
original integration, 75 unit), 69 doctests and 123 Python tests. Coverage is
657/4,855 (13.53%), 3,916 pending, 282 excluded, zero failed/unverified; domain
model is 18/455 (3.96%). Sources/provenance frozen before final verification.
Logs and focused evidence use work/list-element-*. Dashboard adds one separate
list-element entry; all 29 benchmark cards remain. No design skill/delegation.

Next: concurrent group-derived scheduling is a useful extension. The file list
under declarative was inspected, and ConcurrentValuesShadowVariableTest was
partially read (output truncated; read it fully before adapting). It covers list
neighbor/index/cascading-time updates, group-aligned start/finish values, supplier
call counts, cycle propagation/recovery and two bounded full-assert solves. Read
all concurrent fixtures, especially the assertion score calculator. Existing
DependencyGraph cycle evaluation and shared-reference grouping may supply reusable
pieces, but no equivalence should be assumed. The full rewrite stays unfinished.

Final review corrected Java Owner.lastSupplier to box its empty fallback, avoiding
conditional-expression unboxing of a nullable last end. Direct public supplier
probe in work/oracle/solver/ListElementNullableProbe.java passes both the empty
fallback and a nonempty list whose last end is null. Source-digest enforcement
correctly rejected the in-flight solver comparison after this source edit; the
full final gate/release/comparison sequence was restarted against frozen sources.

Follow-up reading during verification filled the previously truncated concurrent
method section and read all five concurrent fixtures. Seven source methods exist:
simpleChain, solutionSimpleChain, groupChain, groupChainValidToInvalid,
groupChainInvalidToValid, solveNoConcurrentValues, solveWithConcurrentValues.
Base instant is 2025-01-01 09:00; ready is previous finish +30 minutes or base for
an assigned first element; finish is shared start +30 minutes. Shared start takes
the group's maximum non-null ready value. Cascading time is base + index days.
Tests require A1/B1 to be the only initial group supplier callers and require
cycle/downstream invalidation plus recovery, while an independent C placed before
a cycle stays valid. Invalid-to-valid also checks an unassigned B3 still sharing
its group's start/finish. Do not infer this from an unassigned representative's
early-returning supplier; direct public-runtime checks should resolve alignment.

The independent fixture expectation flags a group inconsistent when two assigned
members share an owner or dependencies cycle, and propagates that to downstream
values. The provider uses forEachUnfiltered for inconsistent hard penalties and
forEach for assigned, consistent finish-minute soft penalties; easy score checks
these independently. Both solve tests require feasibility with a 1,000-move limit.
Existing DependencyGraph recomputes an immutable plan each evaluation; group start
nodes could share one evaluation, with entity-level inconsistency projected across
related fields, but this design remains unimplemented and unverified.

Final corrected-source verification succeeds: full gate, release tests and live
initialization (all four events before completion, all four optima). All ten
comparison/performance datasets are current, report verification successful, and
the live freshness digest matches. Generated list-element results remain 128/128,
2,365 states/3,675 operations. Existing list quality remains 192 valid/189
equal-score cases; stream and heuristic validation remain 64/64 and 128/128.
Exact8 Rust/Java was 0.190/5.439 ms; stream tabu96 was 182.499/29.347 ms.
These are scoped benchmark results, not whole-solver speed claims.

Browser verified at 1280x900 and 390x844: 13.53%, 657/4,855 methods, domain 18/455,
all six list-element methods Passed, separate 128/128 list-element evidence and
29 benchmark cards. No overflow, stale messages or browser errors. Existing
server retained, verification browser closed. Upstream submodule stays unmodified.

## Explicit shared dependency evaluation and concurrent scheduling

Add production DependencyGraph.evaluate_aligned: explicit key pairs form transitive
classes with the smallest key as representative. Combine fixed/dynamic dependencies,
normalize inputs to distinct canonical keys, invoke one callback per consistent
class with sorted members/inputs, clone the result to each member. Alignment-created
self cycles and downstream classes are inconsistent without supplier calls. Validate
all metadata before callbacks. Plans stay immutable/reusable. Original evaluate
still supports non-Clone values. This rebuilds scheduling; no incremental cache.

Four original evaluator tests cover 512 graphs x 5 partitions (2,560 combinations),
fixed-edge collapse cycles, transitivity/duplicates, recovery, canonical inputs,
None versus inconsistency, pre-callback validation, unwind reuse and a 10,000-node
class. New doctest documents the public API. All seven ConcurrentValuesShadowVariableTest
methods preserve full assertions, including all five initial supplier counts and
both 3-owner/6-visit 1,000-move full-assert feasible solves. Source fixtures and
assertion easy calculator are attributed. Native minute offsets replace the fixed
2025-01-01 09:00 LocalDateTime base; start classes share maximum available ready time.

The reusable concurrent_support fixture feeds previous-finish dynamic edges into
shared start evaluation. Crucial observed behavior: entity inconsistency does NOT
clear unrelated ready values. An assigned first visit retains ready=0 even if its
shared start is cyclic. The initial native projection incorrectly cleared every
field on inconsistent entities; direct Java observations corrected this before
final checks. Generated comparisons check every final field. Full stream projection
rebuilds a global list snapshot. Owner rows arrive separately, so a transfer may
transiently duplicate ownership; normalize this private intermediate snapshot,
while validated final assignments remain unique. Two original domain tests verify
complete cached field/score restoration through cycle/undo and 1,944 small partial
orders against independent closure/relaxation scoring. The independent scorer also
checks every stream projection during the adapted solves.

Generated concurrent comparisons retain ALL 128 cases, 2,385 native observations,
3,574 primitive operations. Every native state matches independent recursive
Python. Java emits 2,025 matching observations and fully agrees on 107/128 cases;
21 cases terminate with recorded runtime errors, zero successful observations
differ. Runtime errors receive NO agreement credit. Preserve their cases, native
states, observed Java prefixes and error messages in the downloadable report.
Seven Python tests enforce boundary values, snapshots, invalid metadata/moves,
reproducibility and honest error/divergence accounting; corrupt/missing/reordered
outputs cannot silently pass.

Important reference boundary: 3 values, group [0,1], lists [2,1]/empty. Group member
0 is unassigned; Java may leave assigned member 1's finish null, then throw in
scoring or a dependent ready supplier. Native semantics follow the source fixture's
independent expected-value helpers (max non-null readiness shared with all members).
Do NOT imitate an exception as the native desired behavior or delete these cases
to get 128/128. ConcurrentOriginalBoundary compiles all five original concurrent
fixtures unchanged from the pinned test tree and uses public APIs only; it observes
assigned_member_finish=null and error=java.lang.NullPointerException:temporal.
This rules out the Long-minute adapter as the cause of this minimal failure.
The diagnostic is rechecked on every concurrent comparison and grants no coverage.
The public annotation used by the pinned fixture emits a deprecation warning;
keep pinned behavior, do not switch versions or use Enterprise.

Debug gate passes 1,410 Rust tests (664 adapted, 671 original integration, 75 unit),
70 doctests and 130 Python tests. Coverage 664/4,855 (13.68%), 3,909 pending,
282 excluded, no failed/unverified adapted methods; domain model 25/455 (5.49%).
Sources/provenance were reviewed and frozen before release/report refresh.
Logs use work/concurrent-*. Dashboard adds separate 107/128 concurrent evidence
with explicit 128/128 native validation and 21 Java error cases; benchmark cards
remain at 29. No design skill/delegation.

Next candidates inspected read-only: AlwaysLoopedShadowVariableTest has one fixed
odd/even-cycle diagnostic method (full test read, fixtures not read); existing
DependencyGraph may already satisfy its native behavior. AffectedEntitiesUpdaterTest
only exposes entity-to-node grouping. More substantial work is persistent dynamic
dependency updates: AbstractTopologicalGraphTest has seven mutation/permutation
methods for loops, edge removal and joining/separating loops. Only method names
were read this turn; read the full tests before implementation. Do not inspect
production implementations. An incremental native dependency cache would address
the current full-rebuild limitation; its design and validation remain outstanding.

Additional read-only preparation: AbstractTopologicalGraphTest (all seven methods
and permutation helpers) and DefaultTopologicalGraphTest were fully read. The
source default implementation test explicitly says it is not incremental. Tests
need SCC members, relative topological ranks, batched edge additions/removals and
changed bits for cycle-membership toggles only. Splitting a four-node SCC into two
cycles must report ZERO changed bits because all four remain cyclic. Removing the
last back edge of a two-node cycle reports exactly those two members. Current
DependencyGraph inconsistent flags include downstream nodes, a different concept.
Source edge direction is prerequisite->dependent; current native evaluation edges
point node->dependency, so adapt orientation explicitly.

The all-permutation helper permutes both node labels and edge insertion orders.
The three larger cases permute all six-node labels, Java-shuffle edges with a
count-dependent Random seed and use a recursive 1,000-limit permutation helper.
Do not silently reduce those cases while claiming complete method adaptation.
A persistent topology abstraction and later incremental value cache remain design
work; source tests do not by themselves require an incremental implementation.

Final verification for this checkpoint: release all-target tests pass all 1,410
cases; the live initialization example delivers all four events before completion
and reaches all four proven optima. All ten comparison/performance datasets were
refreshed after the final source edits and report current; verification.successful
is true and the localhost freshness digest matches the generated report. The
repeated concurrent comparison retains 107 agreements, 21 Java errors and all
128 independently validated native cases, including the unchanged-fixture error.
Desktop 1280x900 and mobile 390x844 browser checks show 13.68%, all seven concurrent
methods Passed, the explicit 21-error evidence, and 29 benchmark cards. No page
errors, horizontal overflow or stale-data messages. Browser verification closed;
the existing dashboard server remains running on localhost:8765.

Current timing examples (Rust/Java): exact 8-value search 0.188/5.371 ms;
96-value stream tabu 173.031/30.907 ms. These are bounded measured workloads,
not an overall solver speed claim. Concurrent scheduling has no timing comparison.

## Persistent dependency topology (next checkpoint)

DependencyTopology<K> maintains a fixed sorted key set and persistent directed
node-to-dependency edges. Atomic update validates ALL removals/additions before
mutation, removes then adds, deduplicates and treats absent removals as no-ops.
Components are sorted SCC memberships; every component shares a deterministic
rank ordered after its prerequisite components. Return only cycle-membership
status toggles, excluding downstream nodes and SCC repartitions that stay cyclic.
Clone supports native trial snapshots. Existing DependencyGraph fixed-cycle
validation shares the original independent iterative SCC routine, now extracted
as strong_components. No Timefold production code was read or used.

All seven AbstractTopologicalGraphTest methods are adapted, with provenance for
that declaration file and DefaultTopologicalGraphTest's constructor/accessors
(empty consistency hook). Full source node/edge permutations retained: 14,400,
14,400, 86,400, 86,400, 518,400, 518,400, 362,880 = 1,601,280 total. Larger cases
preserve count-seeded Java Collections.shuffle and the recursive 1,000-limit
helper, including original node permutation order. Test-only Rust Java PRNG uses
the public Random contract. TopologyPermutationOracle.java adapts test helpers
only and uses actual JDK shuffle to output counts/FNV64 endpoint-stream checksums.
Rust verifies every generated stream against tests/fixtures/topology-permutations.txt.
This is test-input fidelity, NOT a public Java topology behavior comparison.

Three original tests compare all 512 three-node graphs and 4,608 single-edge
toggles against independent Floyd reachability; check atomic errors, no-op/conflicting
batches, clone isolation and downstream-not-cyclic; and exercise a 20,000-node
chain, one giant cycle and recovery without recursive stack use. All focused
checks pass, including the seven adapted methods (~20 seconds in debug).

Review found the evidence digest omitted non-Rust test fixture files. Add recursive
fixture contents/paths to source_digest and a Python regression for add/change/
rename/remove. The initial full gate started before this correction; allow it to
finish then rerun on frozen final sources, never credit its stale capture.

Edges persist, but SCC/rank calculation rebuilds after actual changes. This is a
foundation for an incremental derived-value cache, NOT such a cache or a new
speed claim. Next substantial work: use dependency edges to propagate changed
values through only affected nodes, preserve unavailable vs inconsistent states,
handle edge-rewiring/cycle recovery and transactional undo, and integrate with
actual score-director derived fields. Do not conflate cycle-member changed keys
with all invalidated values; downstream inconsistency still needs propagation.

Additional read-only preparation: NodeTopologicalOrderQueueTest read fully (8
methods: empty, ranked order, offered-only, deduplication, reinsertion after poll,
64-node capacity, arbitrary ties, 50x1000 randomized workload). It only requires
fixed supplied ranks. DynamicFollowerValuesShadowVariableTest read fully (3 methods)
but its fixture files not read this turn; existing dynamic-follower work already
covers this family. Avoid duplicate provenance or coverage.

Native integration preparation: ReferenceNode already owns persistent SnapshotMap
roots and exposes clear/checkpoint/restore for StreamGraph transactions. Plain
StreamGraphBuilder.map is deliberately stateless (checkpoint is unit); do NOT hide
a mutable dependency cache in a captured RefCell there, because trial rollback
would not restore it. A dependency-aware stream node must own and checkpoint its
cache. Begin with an independently checked native cache with explicit dirty keys,
then integrate via the stateful Node interface. Changed incoming edges dirty the
consumer even when cycle membership is unchanged; unavailable None is not a cycle.

Final-source debug gate passes 1,420 Rust tests (671 adapted, 674 original integration,
75 unit), 71 doctests and 131 Python tests. Report coverage is 671/4,855 (13.82%),
3,902 pending, 282 excluded, no failed/unverified cases. Domain model 32/455 (7.03%).
The stale first capture was correctly rejected after the fingerprint fix; only
work/topology-check-final.log is the successful final-source gate. Full release
and all ten benchmark/comparison refreshes run sequentially under one process;
logs are work/topology-*. Do not restart based on quiet compilation output.

Release all-target tests pass all 1,420 cases after a 6m33s optimized build. The
live initialization example delivers all four events before completion and reaches
all four proven optima. No source edits occurred during this final pipeline.
GraphStructureTest was also read completely during verification; its 12 methods
classify descriptor/entity arrangements (empty, previous-only, arbitrary) rather
than specify incremental value propagation. Supporting fixtures were not read this
turn; do not treat this inspection as adaptation or implementation evidence.

Final evidence: all ten comparison/performance datasets are current and report
verification.successful=true. Native concurrent checks remain 128/128; Java
agreement 107/128 with 21 runtime errors and zero divergent successful cases.
Stream search remains 64/64 equal-score; heuristic search 128/128; list search
192 valid / 189 equal-score. Existing Great Deluge 275/276 discrepancy remains
explicit. Current scoped timings Rust/Java: exact 8-value solve 0.174/5.971 ms;
96-variable stream tabu 173.071/25.483 ms. No topology speed comparison is claimed.

Restarted the existing task dashboard server to load the new source_digest logic
(the prior process had imported the old function). The first background launch
was not retained; a confirmed foreground managed session now serves localhost:8765
(session 95911), and its live verification digest matches the final report.
The completed validation/refresh process was session 8363, exit 0. Desktop
1280x900 and mobile 390x844 show 13.82%, 671/4,855 methods, domain 32/455, all seven
topology methods Passed and 29 benchmark cards; no overflow, page errors or stale
messages. Browser verification closed; dashboard server stays running.

## Incremental derived values and transactional stream integration

DependencyCache<K,V> now owns Arc-shared topology/reverse indexes and persistent
SnapshotMap values. Initial evaluation runs in component rank order, skips cycles
and downstream inconsistency, and preserves consistent unavailable None. Updates
validate all dirty keys and edge endpoints before callbacks, apply removals then
additions, dirty changed incoming-edge consumers and cycle-status toggles, then
schedule changed values' direct consumers. Equality covers optional value AND
inconsistency; equal results stop propagation. Changed input identity still forces
its consumer even when input values match. Value-only updates do not scan all
nodes; edge changes still rebuild topology/reverse indexes. Callers explicitly
invalidate keys reading changed external facts/formulas. Cache Clone needs no
V:Clone and shares immutable roots. Stage all new values/metadata before installing;
ordinary callback or equality unwind preserves the old cache (external side effects
and panicking destructors retain ordinary Rust limitations).

DependencyNode / StreamGraphBuilder.derive_dependencies owns and checkpoints both
source rows and cache. Fixed declared key universe; dependencies may refer to absent
rows, which supply consistent None. Present rows emit (row,DerivedValue); source
metadata changes emit even if derived equality stops dependent propagation. Source
retraction removes incoming edges and output. Unknown keys/dependencies produce
GraphError::Dependency(String) and poison the graph transaction as usual. Clear
restores an empty-row, empty-edge initial cache; saved candidates restore snapshots
without invoking suppliers. It is NOT hidden in a stateless map closure.

Five original cache tests: all 512 three-node graphs, 4,608 single-edge toggles,
1,536 external-value invalidations vs independent Floyd/relaxation; equality cutoffs
and changed input identity; invalid metadata and supplier panic; non-Clone snapshots
and 10,000-key sparse update with one supplier / <500 comparisons; equality panic
after staged value/edge changes. Five stream tests: cycles/recovery, metadata and
retraction, callback counts and no-call undo; unknown metadata, supplier unwind
and downstream structural-score rejection restore caches; all 1,296 partial parent
assignments through composite score-director commit/undo vs independent recursive
reference; greedy construction and saved local-search candidates match a separate
ConstraintNetwork at five evaluation limits; foreign port / duplicate key rejection.
No additional upstream method credit (coverage remains 671/4,855).

Public concurrent_oracle now also maintains CachedSchedule from the attributed
concurrent_support/cache.rs fixture. Fixed representative starts plus alias nodes
express shared groups. Dynamic previous-finish edges and dirty ready keys update
DependencyCache between observations. Every field must equal the existing full
native graph/projection and Python. Cached snapshots restore on trial undo and
an unchanged projection asserts zero supplier calls. Initial debug comparison
passes all 128 inputs / 2,385 states. Native alias counts are NOT Java supplier-count
parity. Three new fixture provenance entries cover ConcurrentValue/Entity/Solution.
The original concurrent stream scoring fixture still uses full list snapshot
projection; the extra cache evaluator does not establish faster whole-solver behavior.

Full gate running on final sources; logs work/cache-*. Initial focused core/stream
checks pass; clippy issues and one test score integer-type mismatch were corrected
before final verification. Sources frozen before full release/comparison refresh.

Final-source debug gate passes all 1,430 Rust tests (671 adapted, 684 original
integration, 75 unit), 73 doctests and 131 Python tests. Coverage remains 13.82%,
671/4,855, with 3,902 pending and 282 excluded; verification.successful=true.
Release/all-comparison refresh runs sequentially as session 90466, logs work/cache-*.
Do not restart quiet compilation without authoritative termination evidence.

Next concrete integration: move the concurrent scoring fixture's full list projection
onto owned/checkpointed cached state, while preserving its independent reference-score
assertion after each complete projection. CacheSchedule already batches all dirty
keys/edge changes against one final list snapshot; do not feed phase facts through
ordinary flat_map one-by-one and assert a complete schedule on intermediate mixed
states. Consider a narrowly specified cached projection operator with owned cloneable
state, per-input identity, retraction cleanup, reset for assert_consistent and saved
candidate checkpoints. Its output must depend on the current input, with state used
only to cache work; history-dependent semantics would fail fresh rebuilds. Existing
stateless map must remain stateless. Then exercise actual cached concurrent solving,
not just the second public-comparison evaluator, before making speed claims.

Release all-target verification passes all 1,430 tests after a 6m46s build. The
live initialization example again delivers all four events before completion and
reaches all four proven optima. Final-source concurrent comparisons retain all
128 native cases / 2,385 states, including both full and cached native evaluators;
Java agreement remains 107/128 with 21 explicit runtime errors. Source fixtures
reproduce the known partial-group null-finish error unchanged.

Final refresh process 90466 completed exit 0. All ten comparison/performance
datasets are current, report verification succeeds and localhost's live digest
matches. Stream search remains 64/64 equal-score; heuristics 128/128; list search
192 valid / 189 equal-score. Known Great Deluge 275/276 and concurrent Java 21-error
boundaries remain explicit. Current scoped Rust/Java timings: exact 8-value solve
0.161/5.358 ms, stream tabu96 184.638/23.272 ms. These workloads do not benchmark
the new cache and support no cache or whole-solver speed claim.

Desktop 1280x900 and mobile 390x844 browser verification shows 13.82%, 671/4,855,
domain model 32/455, all seven concurrent methods Passed, new full/cached evaluator
evidence with 128/128 native cases / 2,385 states and 21 Java error cases, and 29
benchmark cards. No overflow, page errors or stale messages. Browser verification
closed; the existing localhost:8765 server remains running (no restart needed).


## Owned cached projections in concurrent construction and search

StreamGraphBuilder::map_cached now owns a cloneable cache per input identity.
Snapshots share the persistent state-map root; an update clones only that identity's
cache and stages the callback result before installation. Retraction discards the
identity's cache; reinsertion starts fresh. Diagnostic rebuild clears caches and
replays current rows. Undo and saved candidates restore snapshots without callback
replay. Clone must isolate mutable state, and cache history must never affect the
output for a given current input and immutable facts. Stateless map is unchanged.

The concurrent scoring fixture now uses map_cached with CachedSchedule. It still
normalizes transient duplicate ownership, projects complete owner-list snapshots,
and independently asserts reference_score after each projection. The public oracle
retains its second cache evaluator and full projection: all fields must agree.
This completes the concrete integration proposed above; actual construction and
local search now use the owned cache, not merely the second comparison evaluator.

Five original operator tests cover per-identity state, warm rollback/rebuild,
retraction/reinsertion, callback panic and downstream failure, sparse cache clones,
history-dependent contract violations, and foreign ports. Two domain regressions
verify full concurrent fields/score and zero supplier calls on republishing restored
lists; construction and ten bounded hill-climbing/tabu searches match independent
scoring, including saved candidates. No new upstream method credit is claimed.

Final-source debug gate passes 1,437 Rust tests (671 adapted, 691 original
integration, 75 unit), 74 doctests, and 131 Python tests. Clippy is warning-free.
Release/comparison refresh is session 79441, logs work/cached-projection-*. Do not
restart quiet compilation without evidence that it has terminated.

Remaining limits: the concurrent fixture still builds complete normalized lists,
clones its previous-visit metadata, and rebuilds topology on edge changes. Alias
supplier counts are not Java parity. Existing benchmark cards do not isolate this
cache change; make no new speed claim. A next useful step is a scoped concurrent
scoring benchmark with equal fixed operations and fully checked output fields,
retaining the 21 known Java partial-group runtime errors in correctness reporting.

Optimized all-target verification passes all 1,437 tests after a 6m51s build.
The live initialization example delivers all four initialization events before
completion and reaches all four proven optima. A provenance wording correction
was followed by another complete captured debug run, exit 0, with matching before/
after/current verification fingerprints. No Rust sources changed after the full gate.
The public concurrent refresh validates 128 native cases / 2,385 states / 3,574
operations. Java agreement remains 107/128, with 21 explicit runtime errors and
zero divergent successful cases; all native fields match full projection/Python.

Final refresh process 79441 completed exit 0. All ten comparison/performance
datasets are current, report verification succeeds, and localhost's live digest
matches the final source/provenance fingerprint. Stream search remains 64/64
equal-score; heuristics 128/128; list search 192 valid / 189 equal-score; pillar
transactions 128/128. Known Great Deluge 275/276 and concurrent Java 21-error
boundaries remain explicit. Scoped Rust/Java timings: exact 8-value solve
0.154/4.934 ms, stream tabu96 170.489/25.910 ms. These do not isolate the new cache.

Desktop 1280x900 and mobile 390x844 verification shows 13.82%, 671/4,855,
domain model 32/455, all seven concurrent methods Passed, owned-cache evidence,
and 29 benchmark cards. No overflow, browser errors or stale messages. The browser
verification session is closed; the existing localhost:8765 server stays running.

## Concurrent shadow-update benchmark

Added original Rust/Java drivers and scripts/benchmark_concurrent.py. Native timing
uses ListProblem and CachedSchedule directly; Java uses public MoveTester with
ConcurrentOracle's attributed domain. No stream graph, score engine, independent
full rescoring, or solver search is timed. This measures the shadow evaluator used
by cached scoring, not the whole cached graph. Both runtimes decode/construct moves,
apply a temporary move, copy/check all nine shadow fields, undo, check restored
ownership, and consume checksums. Native snapshots cache and output rows explicitly.
Model/context setup, Python expected arrays, output encoding, and a separately
validated committed move are outside timing. Runtime/framework costs differ.

Scope: fully assigned 2–64 visits, disjoint fixed shared-start groups, one change
or swap between distinct positions. An initial smoke run exposed Java's rejection
of same-position changes; the generator now constructs valid distinct-position
moves, and Python rejects invalid no-op requests before either runtime is invoked.
No runtime-error sample is filtered out. Every failure aborts measurement. The
separate partial-group oracle still retains all 21 known Java errors.

64 seeded small cases validate cycle creation/recovery and all trial/restored/
committed fields. Four workloads: swap/relocate, 16/64 visits. Defaults 200
transactions per batch, five warmup batches, seven samples, three fresh processes;
runtime order alternates. Five Python regressions cover independent expected fields,
malformed counts/identities, every output column, checksums, complete batch accounting,
invalid scope, deterministic generation and timing workloads. No upstream credit.

The dashboard adds four cards under Concurrent shadow updates vs. the original,
plus methodology/environment and correctness evidence. compat_report now exposes
concurrent_performance as the eleventh dataset. Fixture reuse is noted in provenance
and third-party notices. Full debug gate passes 1,437 Rust tests, 74 doctests and
136 Python tests; release all-target tests also pass. Full refresh session 49560
runs sequentially; logs work/concurrent-benchmark-* and work/concurrent-refresh-*.

Refresh session 49560 completed exit 0. All eleven datasets are current and the
live server/report/current verification digests match. The new default benchmark
passes all 64 cases and every 3-fork x 7-sample x 200-transaction measurement.
Median Rust/Java milliseconds per transaction: swap16 0.042828/0.100922,
swap64 0.213895/0.128548, relocate16 0.033648/0.065776,
relocate64 0.125279/0.130446. The 64-visit swap is slower in Rust; the
near-1x relocate result has overlapping ranges and supports no speedup claim.
These timings establish no historical regression or whole-solver performance.
The separate concurrent comparison remains 107/128 Java agreement with 21 errors,
128 native cases / 2,385 states, and no divergent successful observations. Other
known boundaries remain explicit (Great Deluge 275/276; list search 189/192 equal).

Desktop 1280x900 and mobile 390x844 show four new cards / 33 total, warmed final
numbers, current metadata, 13.82% coverage, no overflow, browser errors or stale
messages. Browser closed; existing localhost:8765 server remains running.

Next useful backend work: profile the new 64-visit swap before choosing an
optimization. DependencyCache currently rebuilds topology/reverse indexes after
edge changes and CachedSchedule clones full visit metadata; measure which dominates
rather than assuming supplier-call reductions mean faster moves. Keep the full
three-node/exhaustive topology, panic/transaction and public field checks. Further
Community method families remain pending; do not redefine completion as benchmark
parity. GraphStructureTest and ListVariableShadowVariablesTest were inspected only
as possible future specifications this turn; no claims or adaptations were added.

## Indexed dependency-cache scheduling

Profiled the pre-change 64-visit swap in an isolated ignored copy under
work/dependency-profile, using coarse Instant timers around cache update stages.
No profiler executable was available. Across 2,004 updates, median total update
was 167,848 ns: topology clone 8,402; topology update 31,063; reverse-plan creation
29,684; value propagation 87,012. Aggregate shares approximately 5%, 19%, 18%, 52%.
Timer instrumentation and I/O make these diagnostic stage measurements, not dashboard
benchmarks. The original release executable is retained at work/concurrent-baseline.

Production DependencyCache now resolves dirty keys to stable topology indices and
uses Vec<Vec<usize>> reverse links plus (rank,index) ready entries. Rank, cycle and
dependency traversal use private indexed topology accessors, removing repeated
key searches and consumer-key clones. Public keyed SnapshotMap values, sorted
supplier inputs and changed-key results remain. Suppliers receive declared topology
keys. The fixed sorted key universe never reindexes. Edge updates still rebuild
SCC/rank topology; this increment does not claim fully dynamic SCC maintenance.

One original 1,024-consumer fanout regression uses shuffled string keys, verifies
exact callback order and equal-value cutoffs, requires fewer than ten key clones
when only the root value changes, and checks rollback when final changed-key
cloning panics after evaluation. Existing exhaustive small graphs, 20,000-node
stack-safe topology, stream transactions and saved-search checks pass. No new
upstream credit or fixture adaptation.

An alternating before/after run (three processes per runtime/workload, three warmup
batches, seven measured batches, 200 transactions each) validates every output via
benchmark_concurrent.parse_batch. Median milliseconds before/after: swap16
0.039960/0.030565; swap64 0.211345/0.168488; relocate16 0.042174/0.039749;
relocate64 0.130116/0.109233. Raw samples are in
work/indexed-cache-before-after.json. These same-machine results indicate about a
20% lower median for swap64, with the usual shared-machine/no-significance limits.

Final-source full gate passes 1,438 Rust tests (671 adapted, 692 original
integration, 75 unit), 74 doctests and 136 Python tests. Sources frozen before the
release/full comparison refresh, session 20915, logs work/indexed-cache-*. Do not
restart quiet optimized compilation without authoritative terminal evidence.

Next feature candidate researched during compilation: consecutive-sequence
collectors. Read only pinned test sources and public interface signatures:
InnerUniConstraintCollectorsTest.toConsecutiveSequences (lines ~857) and
.toConsecutiveSequencesUpdate (~1295); AbstractConstraintCollectorsTest's
buildSequenceChain helper (~78); ConsecutiveSetTreeTest's 12 methods and its helper
fixtures. No implementation/adaptation or provenance credit yet. Public Sequence,
SequenceChain and Break expose items/count/length, endpoint items, first/last flags,
and neighboring breaks. Before implementation, probe the public collector to
establish complete field semantics, especially duplicate identity vs duplicate
index, sequence length vs break length, and mutable-value retraction.

Important observed test distinctions: repeated insertion of the SAME logical
value increments multiplicity but does not increase sequence item count; distinct
values can share the same index. The testDuplicateNumbers case inserts one object
three times at index 3, counts three total distinct items [1,2,3], then removes all
three occurrences and mutates the removed object. Random tests retain exact seeded
1,000-step operations with threshold 2; the duplicate-index test uses absolute
values and identity tie-breaking. The timestamp fixture uses fixed UTC January
2000 values with one-day spacing. Adapt full assertions and helper identities;
do not replace the expected-chain helper with the native implementation under test.
Native snapshots may be immutable, but any change from Java's live views/tie order
must be explicit. A useful implementation must integrate with reversible grouping,
transactions and solver search, not only pass a standalone scaffold.

Release all-target verification passes 1,438 tests after a 6m22s build. Fresh
concurrent transactions pass all 64 generated cases and every measured sample.
Final Rust/Java median milliseconds: swap16 0.035118/0.085286, swap64
0.172297/0.126979, relocate16 0.028380/0.078023, relocate64
0.099390/0.122089. The 64-visit swap remains slower than Java despite the native
before/after improvement. Public full-field concurrent validation remains
107/128 Java agreement, 21 explicit Java errors and 2,385 native observations.

Refresh process 20915 completed exit 0. All eleven datasets are current; report
verification succeeds and source/report/live-server fingerprints match. Coverage
remains 671/4,855 (13.82%), pending 3,902, excluded 282, failed/unverified zero.
Dashboard browser verification shows all 33 cards, final concurrent timings,
no overflow, stale messages or page errors. Browser closed; existing localhost:8765
server stays running. UI code did not change in this backend-only increment.

## Consecutive-sequence collector — current increment

Independent `sequence_collector.rs` now provides SequenceCollector, immutable
SequenceChain/ConsecutiveSequence/SequenceBreak snapshots, SequenceItem, generic
SequenceMetric and exact IntegerSequenceMetric<const STEP:u64=1>. Persistent maps
track captured ID/index/value plus occurrence counts and `(index,ID)` order.
Distinct IDs at equal indices remain distinct; repeated IDs appear once. Conflicting
captured values for an outstanding identity panic atomically. Remove by captured
ID; remove all occurrences before changing that identity. Generic global/keyed,
conditional and composed aggregation uses the existing ReversibleCollector API.
Finish scans all distinct items (with captured-value lookup); chain boundaries are
not incrementally maintained. No dedicated sequence timing exists yet.

Twenty source methods now have full native adaptations: all twelve
ConsecutiveSetTreeTest methods and toConsecutiveSequences/toConsecutiveSequencesUpdate
in each of the four Inner*ConstraintCollectorsTest classes. Source fixed inputs,
all source assertions and both exact 1,000-operation Random(1) streams are retained.
`SequenceTraceFixture.java` regenerates the trace byte-for-byte; fixture/helper
attributions are recorded. Independent pairwise connectivity supplies expected
randomized results. UTC Timeslot fixtures use exact epoch seconds and SignedDuration.

Five original Rust regressions check duplicate/captured identities, failed replacement
atomicity and retained snapshots, exact i64 endpoint distances and invalid zero gap,
3,000 grouped edits through commit/undo/rebuild, mapper/metric/downstream failure
recovery, and construction plus 32 bounded searches against independent full scoring.
Six original Python tests validate independent component/multiplicity/trial semantics
and reject missing/reordered/corrupt response metadata and invalid tie ordering.

The original public Java/native/Python comparison uses public collector value handles
and native CollectNode slots. All 64 ordinary cases and 4,689 states agree, checking
items, counts, lengths, endpoints, flags and neighboring breaks after add/replace/
remove/trial/inverse update. Java chain endpoint and neighboring-break object references
are checked directly in the driver. Different tie ordering is accepted only within
same-index items; each endpoint must match its runtime's observed item order. Native
stable logical-ID order is additionally checked. A separate int MIN/MAX break
explicitly diverges: Java -1, Rust exact 4,294,967,295. Neither that boundary nor
native immutable snapshots/Java mutable aliases receive Java compatibility credit.
Generated checks earn no extra upstream method coverage.

`differential_streams.py` now includes `sequence_collector_checks`; its existing
measurement fingerprint covers the new Rust/Java/Python files and fixture. The
dashboard has a separate generated comparison card with the divergence and no-timing
scope visible. App asset hash updated; no frontend design skill was needed.

Final-source `scripts/check.sh` passes 1,463 Rust tests (691 adapted, 697 original
integration, 75 unit), 75 doctests and 142 Python tests. All 25 sequence tests also
pass release mode. Coverage 691/4,855 (14.233%); constraint streams 311/1,301
(23.905%); pending 3,882; excluded 282; failed/unverified zero. Logs:
work/sequence-final-check.log, work/sequence-release.log. Source freeze precedes
measurement refresh session 63644, with per-dataset work/sequence-refresh-*.log.

Next substantive choices: add a matched sequence-collector benchmark before
optimizing finish, or implement connected-range collectors from pinned tests and
public APIs. Connected ranges are distinct from consecutive sequences; their
consecutiveUsage/toConnectedRanges tests remain pending. Continue honoring no
production-source translation and Community-only boundaries. Goal remains active.

Refresh session 63644 completed exit 0. All eleven datasets are current, report
verification succeeds, and report/source/live-server fingerprints match. Existing
correctness totals remain: streams 128/128; stream search 64/64 equal scores;
heuristics 128/128 equal; lists 512 move cases; concurrent full-field comparison
107/128 Java agreement with 21 explicit Java errors. The new sequence comparison
is present in the stream dataset. All 33 timing cards were refreshed; sequence
collection itself has no timing card. Concurrent transaction Rust/Java milliseconds:
swap16 .032160/.113872, swap64 .158197/.135790, relocate16 .029223/.068754,
relocate64 .105561/.137646. These are noisy workload-specific timings.

Browser verification confirms 14.23% overall coverage, the 64/64 sequence result
with all 4,689 states and the explicit overflow caveat, all 33 benchmark cards,
no stale results or page errors, and no overflow at desktop or 390px width.
Screenshot work/sequence-dashboard.png. Browser closed; localhost:8765 remains
served by the existing dashboard process. No source changes after verification.

## Consecutive-sequence performance — current increment

Added original `sequence_benchmark.rs`, `SequenceBenchmark.java` and
`scripts/benchmark_sequences.py`, plus six response/reference regressions. No
production solver behavior, upstream test adaptation or provenance entry changed.
The public Java collector handle and native SequenceCollector each perform a slot
replacement, full result observation, inverse update, and second full observation.
Initialization is outside each timed batch. Every timed observation is checked
against independent Python pairwise connectivity and contributes to a consumed
checksum; a committed update is separately checked outside the timer.

Canonical observation arrays retain every logical ID's membership/index/component
rank, every sequence count/length/endpoint index/first-last flag/neighbor ordinal,
and every break length/endpoint index/first-last flag. Java neighboring-break object
references are additionally validated against the exposed break collection. Equal
index tie order is omitted only from this benchmark; the earlier differential
continues checking endpoint identities against each runtime's item ordering.

All 64 seeded original cases pass, including duplicates, tied indices, empty slots,
removals and no-op replacements. All samples of four dense/fragmented 64/512-item
workloads pass. Default configuration: 200 pairs per batch, 5 warmup batches,
7 measured samples, 3 fresh processes, alternating runtime order. Median ms/pair:
dense64 Rust .006615255 / Java .010551145 (1.59x); dense512 .024316815/.057951290
(2.38x); fragmented64 .012233405/.040087575 (3.28x); fragmented512
.080690935/.366283415 (4.54x). Raw ranges and outputs retained in
work/sequence-performance.json. These are update-plus-full-observation timings,
including result materialization, array allocation, checking and checksums.
Different native snapshots/Java live views and validation/allocation costs remain
explicit. They do not isolate update-only cost, graph rollback, scoring or search.
No general solver or statistical-significance claim.

The dashboard adds a dedicated section using its existing timing-card layout and
shared renderer. `compat_report.py` now loads twelve datasets, including
sequence_performance. App asset hash updated; CSS unchanged. Native finish still
scans all distinct items and rebuilds sequences; this increment does not optimize
it or claim incrementally maintained boundaries. Benchmark indices are bounded;
the existing Java int-overflow discrepancy remains separately visible.

Final-source `bash scripts/check.sh` passes 1,463 Rust tests, 75 doctests and
148 Python tests. Coverage remains 691/4,855 (14.233%); stream methods 311/1,301
(23.905%), pending 3,882, excluded 282, failed/unverified zero. Logs:
work/sequence-bench-final-check.log, work/sequence-bench-final-measure.log.
All benchmarked Rust code is built optimized. Refresh process session 39991 runs
the existing eleven datasets sequentially before regenerating the final report.
No benchmark-source edits after this final-source gate; documentation/UI files
are excluded from measurement fingerprints.

Next substantial work: connected-range collection from pinned Community tests and
public APIs (consecutiveUsage/toConnectedRanges), including overlapping ranges,
removal/splitting, custom distance metrics, transactional grouped use and genuine
source assertion adaptations. Keep exact Java identity/alias differences explicit.
Alternatively profile update-only sequence workloads before choosing a boundary
maintenance optimization; the full-observation benchmark cannot identify update
cost separately. The full solver rewrite remains active and incomplete.

While measurements ran, read only ConnectedRangeTrackerTest declarations, its
initial fixed-range cases and the start of the overlap case. Important difference for the next feature: repeated insertion
of the same range remains repeated in its iterable and contributes multiplicity to
minimum/maximum overlap (unlike sequence logical-ID deduplication). Touching endpoints
join a component but do not increase overlap. Removal uses captured endpoints even
after the source object's endpoints mutate. Nine tracker methods exist, including
two equality/hash tests and a randomized test; preserve their actual assertions
before assigning credit. This is research only, with no connected-range code or
coverage added in this increment.

Refresh session 39991 completed exit 0. All twelve datasets are current; final
report verification succeeds and source/report/live-server fingerprints agree.
Existing agreement boundaries remain explicit (including 21 concurrent Java errors
and the single sequence overflow discrepancy). Browser verification confirms all
37 timing cards, all four new sequence medians/ranges, the update/restore plus
observation caption, and no stale messages, page errors or horizontal overflow at
1440px and 390px. Screenshot work/sequence-benchmark-dashboard.png. Browser closed;
existing localhost:8765 server remains running. All source changes are verified.

## Connected-range collection — current increment

Independent `range_collector.rs` provides ConnectedRangeCollector, owned immutable
ConnectedRangeChain/ConnectedRange/RangeGap snapshots, RangeItem, RangeMetric and
exact IntegerRangeMetric. Captured `(ID,start,end,value)` entries retain occurrence
counts in persistent storage. Repeated IDs require identical captured fields;
remove all occurrences before changing identity or use separate occurrence IDs.
Retraction ignores supplied endpoints/value and uses captured ID. Start>end rejects
atomically. Finish sorts start/end/ID, expands occurrences, forms touching components
and sweeps endpoint events. It rebuilds boundaries; no incremental boundary or
connected-range timing claim. All generic collection operators can use this type.

All occurrences appear in output, unlike sequence deduplication. Touching spans
connect, but overlap counts measure half-open interiors. Zero-length ranges add no
overlap; empty-only components return None for both bounds. has_overlap means
maximum >1. i64 endpoints subtract exactly into i128; pure custom metrics can return
SignedDuration. Complete captured native values/IDs define equality/hash; saved
snapshots and hashes do not mutate. Java live views and mutable object aliases are
not reproduced.

Twenty source methods are adapted: eight ConnectedRangeTrackerTest methods and
consecutiveUsage, consecutiveUsageDynamic, consecutiveUsageUpdate in all four
collector arities. Full item/endpoint/count/overlap-bound/gap assertions use an
original pairwise-connectivity and midpoint-count reference. Source empty-component
MAX/MIN bounds map explicitly to native None. HashCodeConsistentWithEquals preserves
native value equality/hash consistency; HashCodeReflectsMutation remains pending,
because the source specifically requires the same live object's hash to change.
No Java numerical-hash or general equality implementation parity is claimed.

The attributed `RangeTraceFixture.java` runs the original randomized test's
split-point input-selection scaffolding against the pinned Community jar (test-only
internal types, no implementation source read or copied). Its TestRange equality/
hash semantics are retained. A logical-value approximation was caught and discarded
before credit: fresh selection identities produce all additions in the actual
source test. The final fixture has exactly 100x100 Random(1) operations and zero
removals. Native expected results are independent, not Java production outputs.
Compile with `javac -cp work/oracle/timefold-solver-core-2.7.0.jar -d work
compat/oracle/RangeTraceFixture.java` and run class
`ai.timefold.solver.core.impl.score.stream.collector.connected_ranges.RangeTraceFixture`
with that jar and work on the classpath. Provenance and notices cover fixtures/helpers.

Five original Rust regressions check multiplicities, captured endpoint removal,
failed replacement atomicity, immutable hashes, exact i64 endpoint length, empty
ranges, custom duration metrics, 3,000 generated grouped edits through commit/undo/
rebuild, mapping/metric/downstream failure restoration, construction and 32 capped
saved-candidate searches against independent full scoring. Seven Python regressions
check independent semantics, complete response identity/shape/fields, error/difference
accounting and preservation of every original comparison case.

Original public comparisons retain 197 cases / 11,583 states. Every native case
matches Python. Java fully agrees in 65 cases and differs in 132, with no errors.
All 64 distinct-identity positive-length cases agree. The initial 128 shared-identity
cases (64 positive, 64 allowing empty ranges) remain unchanged; all differ. Four of
five fixed boundaries also differ. Java's zero-only MAX/MIN bounds normalize to
native None, but other discrepancies are retained verbatim. Java iterator order is
not compared across runtimes; native stable ordering is separately checked.

Minimal duplicate-object failure: one Item [0,2), handles A.add(item), B.add(item),
B.remove(), A.remove(); Java counts 1,2,2,1, native counts 1,2,1,0. A separate pair
of equal-endpoint objects removes correctly. Native multiplicity behavior is kept;
this is a reported reference discrepancy, not hidden compatibility credit. Empty
interior/junction ranges can make Java hasOverlap true with maximumOverlap=1.
Java int MIN/MAX length overflows to -1; native length is 4,294,967,295. All inputs,
raw runtime outputs and full differing observations are in range_collector_checks,
integrated into differential_streams.py and its dashboard comparison entry.

Final `bash scripts/check.sh` passes 1,488 Rust tests (711 adapted, 702 original
integration, 75 unit), 76 doctests and 155 Python tests. All 25 range tests pass
release mode. Coverage 711/4,855 = 14.64% (report's three-decimal value 14.645),
constraint streams 331/1,301 = 25.44%, pending 3,862, excluded 282, failed/unverified
zero. Source frozen before refresh session 52390, per-dataset work/range-refresh-*.log.
Read work/range-check.log and work/range-release.log for final-source gates.
No full rewrite completion claim: goal remains active.

Refresh session 52390 completed exit 0. All twelve datasets are current, final
report verification succeeds, and report/source/live-server fingerprints match.
The stream dataset retains 65/197 connected-range agreements and all 132 differing
cases. Existing stream/solver/list/heuristic comparisons retain their explicit
boundaries. All 37 timing cards were refreshed; connected ranges add correctness
only, not a timing card. Browser checks confirm 14.64% overall, 25.44% stream coverage,
eight passed tracker methods plus the one pending live-hash method, the full range
comparison/caveats, and no stale text, page errors or desktop/mobile overflow.
Screenshot work/range-dashboard.png. Browser closed; localhost:8765 server remains.

Next useful feature: load-balance collection. Only its test location was found,
not yet implemented: InnerUniConstraintCollectorsTest.loadBalance around line 949
and assertUnfairness helper around line 1047. Read full pinned tests, companion
arities and public LoadBalance docs before implementation. Exact unfairness rounding,
load/group multiplicities, initial loads, undo and solver scoring need evidence.
A dedicated connected-range performance comparison is also still pending. Do not
claim unsupported mutable hash or Java duplicate-object bug parity.

## Load balancing — current increment

Independent load_balance_collector.rs exports LoadBalanceCollector<K> and immutable
LoadBalance<K>. Captured (key,i64 load,i64 initial) occurrences use persistent maps;
initial load is counted once per active key, every occurrence adds its load. All
active captures of a key must have equal initial load. Conflicts panic atomically;
remove all captures before changing initial load. Removal matches exact triples.
Missing remove/replace returns false. Zero and negative loads are supported; only
present keys participate, including zero-load keys. Baseline zero rows can include
idle resources. Empty standalone result is empty map/zero; empty streams emit no row.

Totals, total sum and sum of squares use BigInt. Updates maintain moments, finish
computes sqrt((n*sumSquares-sum^2)/n) using integer square root and squared midpoint
rounding. Six significant digits, half-even, canonical trailing-zero stripping;
zero is scale-zero Decimal::ZERO. No binary floats. Owned result shares load-map
root; clones share captures and key state. Java live-map and decimal-scale parity
are not claimed. Java documentation says six decimal places, but public observations
use six significant digits. Additional exact halfway probes (work/load-balance-
rounding-probe.txt) confirm half-even at roots 1234565,1234575,9999995. Production
implementation source was neither read nor copied.

Four methods adapted: loadBalance in Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest.
Every empty/add/duplicate/removal unfairness assertion remains. Native captures
replace uni record/multi-arity tuples and handles; ignored fields remain zero.
Provenance/notices updated. Five original Rust integrations cover multiplicities,
initial consistency/reset, zero keys, missing values, immutable snapshots, atomic
failures, exact full-width totals/large offsets, 1,800 grouped edits through
commit/undo/rebuild, callback/structural rejection restoration, construction and
32 bounded saved-candidate searches vs independent full scoring. One unit test
covers exact-root ties and decimal exponents -80..80. Seven Python tests protect
reference arithmetic, representation normalization, malformed/missing/reordered
responses, differences/errors and retained seeded inputs. No generated-case credit.

Public LoadBalanceOracle.java and native load_balance_oracle.rs perform captured
slot insert/replace/remove and inverse-update trials. Python load_balance_checks.py
uses independent pairwise squared load differences and a 120-digit Decimal square
root, rounded to six digits. All 101 cases / 8,290 states match native results.
Java agrees on 97 cases, including all 96 bounded cases; four large-number cases
differ, no runtime errors. All original input/output/difference records remain.
Java returns zero for loads 1000000000 and 1000000001 vs native 0.707107; full i64
boundaries expose wrapped totals and squared deviations. Decimal trailing zeros
normalize numerically; Java representation scale is not an equivalence claim.
Paired inputs require consistent initial loads. Separate initial probe shows Java
keeps first initial while any occurrence remains; native rejects inconsistent ones
for deterministic rebuild. Connected-range's existing 132 differences stay explicit.

Final bash scripts/check.sh passes: 1,498 Rust tests (715 adapted, 707 original
integration, 76 unit), 77 doctests, 162 Python tests. New nine integrations and the
rounding unit test also pass release. Coverage 715/4,855 = 14.73%, streams 335/1,301
= 25.75%; pending 3,858, excluded 282, failed/unverified zero. Gate logs are
work/load-balance-check.log and work/load-balance-release.log. Source frozen before
sequential refresh session 38165; work/load-balance-refresh-*.log tracks all twelve
datasets. Load balance adds correctness evidence only, no new timing card. The
existing 37 workloads must remain accurately described. Goal remains unfinished.

Refresh session 38165 completed exit 0: all twelve datasets are current and final
report verification succeeds. Source, report and live-server fingerprints match.
The stream dataset retains 97/101 load-balance agreements, four explicit differences,
and the existing range/concurrent boundaries. Browser checks confirm 14.73% overall,
25.75% stream coverage, four passed loadBalance methods, complete comparison caveats,
all 37 timing cards, no stale measurement text, no page errors and no horizontal
overflow at 1440px/390px. work/load-balance-dashboard.png shows the refreshed coverage.
Browser closed; existing localhost:8765 server remains running.

Next possible increments: calendar-period sum collectors or custom-order min/max,
whose source methods remain pending; read their pinned tests/public contracts first.
Dedicated connected-range and load-balance timing comparisons also remain pending.
Do not describe this milestone as a complete rewrite or full Java behavioral parity.

## Calendar-period aggregation — current increment

Independent period_collector.rs exports CalendarPeriod, PeriodOverflow and
PeriodSumCollector. Period values hold three independent signed i32 fields. Public
constructors/accessors, equality/hash, checked add/subtract and display preserve
years/months/days, including non-normalized and mixed-sign amounts. No Ord is exposed
because calendar amounts have no anchor-independent chronological order. There is
no date application, normalization, parsing or elapsed-duration/time-zone conversion.
Public JDK Period documentation is linked in the stream contract; no copied JDK code.

The collector records exact component triples/multiplicities in a persistent map,
including zero captures, and maintains a checked per-field total. Clone shares roots,
finish returns an owned Copy value. Empty standalone sum is ZERO, empty stream groups
emit no row. Missing remove/replace returns false. Try methods return PeriodOverflow
without mutation; trait methods panic for graph rollback. Replacement subtracts old
then adds new, checking both intermediates. An identical replacement is not generally
a no-op: public pinned Java reuses the same Period object and still throws replacing
-1 in the day sequence [-1,MAX,1]. Equal-but-distinct periods also throw there. Source
update-test comments suggesting an identity shortcut are not generalized. Subtracting
MIN from MIN succeeds. Keep all arithmetic prefixes representable for full rebuilds.

Eight methods adapted: sumPeriod and sumPeriodUpdate in all four Inner* collector
arities. All source values, expected results and operation ordering remain; native
captured tuples/CollectNode IDs replace Java handles, with ignored nullable fields
retained for update mappings. Ledger and notices updated. Original tests add no
upstream-method credit: six Rust regressions cover calendar identity/sign/display,
all-field widened arithmetic boundaries, zero/duplicate captures, snapshots, failed
insert/remove and both replacement stages, 1,800 grouped edits through commit/undo/
rebuild, mapping/downstream/structural/overflow recovery, construction and 32 bounded
saved-candidate searches against independent per-field full scoring. Two doctests
and seven Python reference/protocol tests cover the supporting contract and evidence.

PeriodCollectorOracle.java uses pinned public APIs and pooled Period objects.
period_oracle.rs exposes the same add/replace/remove/inverse-update protocol through
try methods. period_collector_checks.py compares independent Python checked field
arithmetic and reconstructs every successful state from active captures using
unbounded sums. All 227 cases / 8,771 observations agree, including 120 expected
arithmetic rejections. Both runtimes stop at the first rejection; all input operations
and observed prefixes remain stored. Rejection agreement is not a successful update,
and Java post-error state is not claimed. Same-object/equal-object and both update
stages have fixed boundaries for every field. work/period-probe.txt retains initial
black-box observations; work/period-probe.json retains full latest checks.

Review added explicit addition-stage replacement failures for all three fields.
The first in-flight check correctly rejected its stale log after these source edits
(work/period-initial-check.log). A fresh complete final-source gate then passed:
1,512 Rust tests (723 adapted, 713 original integration, 76 unit), 79 doctests and
169 Python tests. All 14 period integration tests pass release. Logs are
work/period-check.log and work/period-release.log. Coverage 723/4,855 = 14.89%,
streams 343/1,301 = 26.36%, pending 3,850, excluded 282, failed/unverified zero.
Source frozen before sequential refresh session 99193, with per-dataset logs at
work/period-refresh-*.log. This adds correctness only; 37 timing cards remain.
Goal is active and the complete rewrite remains unfinished.

Refresh session 99193 completed exit 0. All twelve datasets are current; final report
verification succeeds and report/source/live-server fingerprints agree. Existing
Java differences remain explicit. Browser QA confirms 14.89% overall, 26.36% streams,
eight passed sumPeriod/sumPeriodUpdate methods, the 227/227 period card with 120
expected-rejection caveat, all 37 timing cards and no stale messages, page errors or
horizontal overflow at 1440px/390px. Screenshot work/period-dashboard.png shows the
period/load-balance/range evidence entries. Browser closed; localhost:8765 remains.

Next useful feature: custom-order minimum/maximum collectors. Their source methods
minNotComparable/maxNotComparable and corresponding updates in the Inner* test
classes remain pending; read all arities and public comparator semantics before
implementation. Do not assume stable tie identities or mutable aliases. Dedicated
connected-range/load-balance/period performance measurements are still pending.

## Captured-key extrema increment (2026-10-04)

Independent extrema_collector.rs exports Extremum, ExtremumCollector, MinByCollector
and MaxByCollector. Inputs capture (stable ID, Ord key, non-Ord payload). Persistent
identity/multiplicity and key/bucket indexes stage updates before installation.
Both min and max choose smallest active ID among tied keys; finish returns an owned
snapshot. Retraction looks up captured ID only. Duplicate IDs require equal captured
key/payload until final removal; conflicts panic atomically. Missing remove/replace
returns false. Default ReversibleCollector replacement stages both halves. Pure,
consistent immutable key/ID ordering remains a precondition. Reverse/tuple/Ord newtype
keys express custom order; no arbitrary Java Comparator/ABI parity is claimed.

Read all four arities' minNotComparable/maxNotComparable and Update source methods,
plus public comparable-property min/max Javadocs/signatures (not production bodies).
Sixteen adaptations preserve all source assertions, duplicate/retraction/update
ordering and ignored nullable tuple fields. Non-Ord Text payloads with captured string
keys replace Java projections. Provenance and notices updated. Public docs explicitly
allow either tied element and changing identity with incremental history; native
stable ties provide deterministic reconstruction, not a Java history match.

Six original regressions (no method credit) cover null/non-Ord payloads, duplicates,
Reverse/composite/full-width keys, captured removal, atomic conflicting updates,
clone-failure fault injection across 64 attempts, 1,800 grouped edits with commit/
undo/full-sort reconstruction, mapping/downstream failures, construction and 32
bounded saved-candidate searches with payload-sensitive scores. One doctest and six
Python reference/protocol regressions supplement them.

ExtremaCollectorOracle.java and extrema_oracle.rs use pooled immutable items and
slot handles. Original extrema_collector_checks.py keeps all 196 cases and 16,528
states, including duplicate IDs, signed/full-width keys, replacements/removals and
inverse-update trials. Every runtime output must name an active item with its exact
captured key/payload and independent extreme key. Native additionally chooses lowest
ID. Java satisfies the public rule throughout, with 2,429 permitted tie-identity
differences retained explicitly. No exact identity agreement, extra method coverage
or dedicated timing claim. work/extrema-probe.json retains the standalone result.

Final source gate passed: 1,534 Rust tests (739 adapted, 719 original integration,
76 unit), 80 doctests and 175 Python tests; all 22 extrema integration tests also pass
release. Logs work/extrema-check.log and work/extrema-release.log. Overall method
coverage 739/4,855 = 15.22%; constraint streams 359/1,301 = 27.59%. Pending 3,834,
excluded 282, failed/unverified zero. Sequential twelve-dataset refresh is session
10464 with logs work/extrema-refresh-*.log. Source frozen throughout measurements.
Goal remains active; the complete solver rewrite is not finished.

Refresh session 10464 finished successfully. All twelve datasets are current;
report/source/live-server fingerprints match. Dashboard browser QA confirms overall
15.22%, streams 27.59%, all sixteen NotComparable methods passed, the 196/196 extrema
selection-rule card with 2,429 permitted identity differences, and all 37 timing
cards. No page errors or horizontal overflow at desktop 1440px/mobile 390px. Saved
screenshot work/extrema-dashboard.png; browser closed and localhost:8765 remains.
Existing Java discrepancies remain explicit. No new extrema timing was added.

Next useful increment: inspect remaining natural-comparable min/max Update methods
and collector-composition cases in the pinned test inventory. Dedicated extrema,
connected-range/load-balance/period performance measurements remain pending. Preserve
stable native identity and keep Java tie differences separate from invalid results.

## Collector composition and finish mapping (2026-10-04)

New original finishing_collector.rs exports CollectorFinisher<Input> (a pure static
result policy with associated Clone output) and FinishingCollector<C,F>. Default and
Clone require only C's capabilities, not F:Default/Clone. finish invokes F on the
inner's owned output, even when empty; graph empty groups still emit no row. Inner
insert/remove/replace stage cloned state before installation; false returns and
ordinary panics cannot install partial changes. replace delegates to C::replace,
retaining specialized behavior instead of recreating remove/insert. from_collector
and into_inner support existing collectors. Policies must be deterministic, pure
and preserve immutable-snapshot semantics; runtime closure captures can use graph
mapping. No callback-count/order or Java ABI guarantee.

Read full source methods in all four arities. composed_collector_cases.rs adapts
compose2/3/4, compose2Update, collectAndThen/Update, minComparableUpdate and
maxComparableUpdate: 32 methods. Preserve all empty/duplicate/add/remove/replacement
assertions and zero trailing tuple fields. Nested PairCollector and finish policies
flatten tuples or transform counts. Natural min/max updates use captured natural
keys; non-uni LocalDateTime fixtures retain ordered (2023,1,1,0,minute) fields for
source offsets 1..7, with no general date arithmetic claim. Provenance/notices updated.

Five original regressions add no upstream-method credit: isolated partial custom
inner mutations on panic/false, specialized replace dispatch, real same-value period
subtraction overflow, standalone/graph finisher-panic recovery, 1,800 grouped edits
with full independent fields and commit/undo/rebuild, construction and 32 bounded
saved-candidate searches. One doctest and five Python reference/protocol regressions.

Original CompositionCollectorOracle.java, composition_oracle.rs and
composition_collector_checks.py compare 132 cases / 11,300 observations. Two-, three-
and four-result composition plus count-result transformation agree exactly on all
fields, including f64 average bits; independent Python reconstructs every state.
Duplicate values, replacements, removals and inverse-update trials are retained.
Generated inputs keep intermediate integer sums/differences representable. No new
source-method credit, Java callback failure/recovery or dedicated timing claim.
work/composition-probe.json retains the standalone comparison.

Final source gate passed: 1,571 Rust tests (771 adapted, 724 original integration,
76 unit), 81 doctests and 180 Python tests. All 37 new integration tests pass release.
Logs work/composition-check.log and work/composition-release.log. Coverage
771/4,855 = 15.88%, streams 391/1,301 = 30.05%; pending 3,802, excluded 282,
failed/unverified zero. Sequential refresh is session 82208; source frozen throughout.
Logs work/composition-refresh-*.log. Goal remains active and the rewrite is unfinished.

Refresh session 82208 completed exit 0. All twelve datasets are current and
report/source/live-server fingerprints match. Browser QA confirms 15.88% overall,
30.05% streams, 16/16 compose methods and 8/8 collectAndThen methods passed; all 32
new adaptations are verified in report metadata. The 132/132 composition card
retains 11,300 states and exact-average-bit scope; existing Java differences remain.
All 37 timing cards render. No page errors or horizontal overflow at 1440px/390px.
Screenshot work/composition-dashboard.png. Browser closed; localhost:8765 remains.

Next inspect remaining Inner* collector methods. The uni pending list is toSet,
toSortedSet, toSortedMapMerged, countUpdate, sumUpdate, countDistinctUpdate,
toSetUpdate and toSortedSetUpdate. Read corresponding arities and compare existing
SetCollector/MapCollector behavior before adapting; toSortedMapMerged may need a
reusable merge-result policy or stronger ordering/multiplicity coverage. Dedicated
extrema/composition/connected-range/load-balance/period benchmarks remain pending.

## Checked integer sums and remaining collection updates (2026-10-04)

Read all eight methods in all four arities (re-read the truncated bi output
separately). New set_sum_collector_cases.rs adapts toSet/toSortedSet,
toSortedMapMerged, countUpdate, sumUpdate, countDistinctUpdate, toSetUpdate and
toSortedSetUpdate: 32 methods. Retain all source assertions and arity-specific tuple
sums/first-field mappings, ignored nullable string fields and uni alternating
merged-value counter. SetCollector provides memberships/ordered native keys;
MapCollector plus FinishingCollector exposes wrapping i32 sums matching Integer::sum
for captured i32 inputs. Bounded source count values use usize. All provenance and
notices are recorded. No upstream production implementation used.

Black-box work/SumBoundaryProbe.java found that public Java sum rejects insertion,
removal, replacement-difference and replacement-total overflow, but accepts a
same-value replacement even when subtracting the old value first would overflow.
New original sum_collector.rs exports SumCollector and SumOverflow. It checks
insert/remove totals, and checks new-old then total+difference on replacement.
Try methods preserve state on overflow; missing captures return false. Trait methods
panic for normal graph rollback. Captured zero/duplicates persist. It shares roots
on clone and maintains finish total without scanning. Existing IntSummary wrapping
sum is unchanged. AverageCollector's separately observed wrapping replacement-total
behavior is unchanged. Keep all insertion prefixes representable for rebuilds.

Five original regressions add no source-method credit: widened i64 boundary
arithmetic, both replacement failure stages, zero/duplicate/missing captures,
unchanged-value updates, MIN removal, immutable snapshots, graph overflow recovery,
1,800 map updates with commit/undo/rebuild and zero-key retention, construction and
32 bounded searches against independent total scores. One doctest and seven Python
reference/protocol tests accompany them.

SetSumCollectorOracle.java, set_sum_oracle.rs and set_sum_collector_checks.py retain
270 original cases / 19,484 observations. All public Java/native outputs match full
independent multiset calculations, including 39 expected sum overflow rejections.
Cases stop at the first error; rejected operations are not successful updates and
no Java post-error state is claimed. Set comparison normalizes iteration order;
native sorted iteration is separately checked by adaptations, while map key order
is retained in comparisons. Maps exercise wrapping i32 merges, duplicate captures,
zero-valued retained keys and key changes. All modes include inverse-update trials.
No arbitrary merge-function, Java ABI, null-key/value or timing claim. Standalone
results saved to work/set-sum-probe.json.

Final gate passes: 1,608 Rust tests (803 adapted, 729 original integration, 76 unit),
82 doctests and 187 Python tests. All 37 new integration tests pass release. Logs
work/set-sum-check.log and work/set-sum-release.log. Coverage 803/4,855 = 16.54%,
streams 423/1,301 = 32.51%, pending 3,770, excluded 282, failed/unverified zero.
The pinned InnerUniConstraintCollectorsTest declares 58 methods, now all passed.
Bi has 54/59, tri/quad each 53/58. Remaining five in each are count, countDistinct,
sum, minComparable and maxComparable; read their exact tuples/date fixtures next.
The overall rewrite remains unfinished; goal active. Refresh session 70903 runs
all twelve datasets sequentially on frozen source, logs work/set-sum-refresh-*.log.

Refresh session 70903 finished exit 0. All twelve datasets are current, and
report/source/live-server fingerprints match. Browser QA confirms 16.54% overall,
32.51% streams, four sorted merged-map methods and four sumUpdate methods passed,
the 270/270 comparison card with 19,484 observations and 39 expected-rejection
caveat, and all 37 timing cards. No page errors or horizontal overflow at desktop
1440px/mobile 390px. Screenshot work/set-sum-dashboard.png; browser closed and
localhost:8765 remains. Existing Java differences remain visible and uncredited.

## Complete Inner collector inventory and aggregate timings (2026-10-04)

Added tuple_collector_cases.rs with the remaining count, countDistinct, sum,
minComparable and maxComparable methods for bi/tri/quad (15 adaptations). Read all
full pinned methods. Numeric tuples, sums, duplicates, retraction order and expected
results are preserved. Fixed LocalDateTime calendar fields use a native ordered
fixture; no Java PECS variance or general date API claim. All provenance/notices
are recorded. No new production Rust implementation in this increment.

The four Inner* classes now have all 233 declared methods adapted: uni 58, bi 59,
tri 58, quad 58. This is native adapted-method coverage, not original JUnit execution
or full stream compatibility. Overall 818/4,855 = 16.85%; streams 438/1,301 = 33.67%.
Pending 3,755, excluded 282, failed/unverified zero. The rewrite remains unfinished.

New aggregate_benchmark.rs / AggregateBenchmark.java / benchmark_aggregates.py
compare checked i64 sums, naturally sorted sets and sorted maps merged by wrapping
i32 addition at 64/512 captures. Every timed pair replaces one capture, materializes
and validates the full observation, restores it, materializes/validates again, and
consumes weighted checksums. Separate committed results are checked outside timing.
Native captured-value APIs and Java handle APIs keep their own bookkeeping costs.
This includes allocation/copying/assertions, not isolated mutation, graph rollback,
scoring or solver search. Inputs and arithmetic are bounded and representable.
Sets have duplicate pairs; maps have eight contributions per key and a new-key trial.
96 generated cases pass independent Python reconstruction in both runtimes; six
Python tests reject bad protocol/observations/checksums and validate workload scope.
All benchmark cases are original and add no method credit.

Final gate: 1,623 Rust tests (818 adapted, 729 original integration, 76 unit),
82 doctests and 193 Python tests passed; all 15 new adaptations also pass release.
Logs work/aggregate-check.log and work/aggregate-release.log. Sequential refresh
session 79641 completed exit 0 on frozen source; logs work/aggregate-refresh-*.log.
All thirteen datasets are current and report/source/live-server fingerprints match.
Aggregate timing uses 200 pairs, 5 warmup batches, 7 samples, 3 fresh processes,
alternating runtime order; all 21 records per runtime/workload passed validation.
Results are mixed: Java/Rust median ratios sum64 0.17, sum512 0.12, set64 1.48,
set512 0.81, map64 3.10, map512 1.40. No overall solver-speed claim.

Browser QA: 16.85% overall, 33.67% streams, new bi methods Passed and 59-method
class filter; metadata verifies all four classes. All 43 timing cards render,
including six new aggregate cards and methodology/environment/caption. No errors
or horizontal overflow at 1440px/390px. Screenshot work/aggregate-dashboard.png.
Browser closed; localhost:8765 remains running. README and contract docs updated.

Next inspect broader remaining solver/stream behavior, rather than assuming that
finishing four collector classes closes the rewrite. Score calculation has 119
pending methods, and constraint streams retain 863 pending. Use tests/public API
specifications only. Dedicated extrema/composition/range/load-balance/period
benchmarks and generic merge functions remain possible extensions. Check inventory
and existing native contracts before choosing the next meaningful behavior gap.

## Fading variable tabu acceptance (2026-10-04)

Previous increment 21c5f36 completed the Inner collector inventory and added
aggregate benchmarks; it was concrete progress. Current source adds independent
FadingTabuMemory<K> and Acceptance::FadingVariableTabu. A selected key starts age 1;
ages <= hard_tenure reject, age hard_tenure+k gives (k+1)/(fading_tenure+1), combined
horizon accepts. Zero fading matches hard tabu; zero hard tenure supports immediate
fading. Combined zero/overflow is rejected before scoring. Refresh deduplicates
keys, empty completed steps age them, and no global step counter can overflow.
The youngest queried retained key determines probability (documented native policy).
Aspiration requires strict global-best improvement. Seeded SplitMix64 is separate
from move ordering and does not claim Java random traces. Lazy supplied-draw API
validates finite [0,1) values only if a probabilistic decision actually needs one.

All basic/list/mixed local-search engines now collect touched indices for fading,
covering scoped and graph backends. List moves retain both touched owners and mixed
basic/list keys remain separate. Best/rejected fallback steps advance memory as
before. The policy is also in owned-score, fractional and bendable-decimal matrices.

Read the complete pinned EntityTabuAcceptorTest.fadingTabuSize. The one adaptation
preserves all 7 step observations, sizes 2/4, entity keys e0/e1, zero scores, draws
0.5/0.5/0.1, acceptance decisions and empty-random expectations. Native callbacks
assert no draw or exactly one draw. Java mock/lifecycle plumbing maps to explicit
native completed-step calls. Provenance/notices updated. Multi-key fading and
zero-hard behavior remain native contracts, not Java-equivalence claims. No
value/move-tabu search or ratio sizing in this increment.

Seven original regressions cover chronological last-touch references across
29 tenure pairs x 180 steps x 81 key pairs, zero-fading agreement with TabuMemory,
large tenure arithmetic, optional/null keys, boundary/invalid draws, aspiration,
seed replay and certain-decision draw preservation, fresh sessions, early config
rejection, actual search stops, touched list owners and mixed identity separation.
Existing backend-comparison matrices additionally cover selection, fallback, pins,
budgets, shuffled ordering and joint moves. One new API doctest. Focused logs
work/fading-focused.log and work/fading-extra.log passed.

Scope audit: pinned public understanding-the-score documentation explicitly marks
Score Analysis Enterprise-only. Do not implement that API from those docs or
request Enterprise artifacts. Existing native graph match inspection is unchanged.

Full gate passed: 1,631 Rust tests (819 adapted, 736 original integration, 76 unit),
83 doctests and 193 Python tests. All 8 new tests pass release. Logs
work/fading-check.log and work/fading-release.log. Coverage 819/4,855 = 16.87%,
search algorithms 25/158 = 15.82%; EntityTabuAcceptorTest now 4/4 adapted. Pending
3,754, excluded 282, failed/unverified zero. Source frozen during sequential refresh
session 79021; thirteen datasets, logs work/fading-refresh-*.log. No dedicated
fading-tabu timing or public-Java trace comparison has been added.

Refresh session 79021 completed exit 0. All thirteen datasets current; report,
source, verification and live-server fingerprints agree (work/fading-audit.py).
Browser QA confirms 16.87% overall, 15.82% search, all four entity-tabu methods
Passed, and 43 timing cards. No page errors or horizontal overflow at 1440px/390px.
Browser closed; localhost:8765 remains running. The full rewrite remains incomplete.

Next candidates: extend Community search behavior (value/move tabu, diversified
late acceptance or composite acceptors), or inspect pending stream behavior.
Read complete pinned test methods before choosing scope. Public seeded comparison
of fading search quality and dedicated timing remain pending; do not infer those
claims from the fixed-draw method or refreshed existing algorithms' benchmarks.

## Correct HardMediumSoft late-acceptance reset levels (2026-10-04)

Previous increment 7c0e576 added and verified fading variable tabu (progress).
Read full CompositeAcceptorTest, DiversifiedLateAcceptanceAcceptorTest,
LateAcceptanceScoreBufferTest and DefaultLevelScoreStateTest while choosing next
work; only Community tests/public docs, no upstream production source. The last
class exposed a correctness gap: mediumImprovementTriggersResetForHardMediumSoftScore
requires a reset, while native LateAcceptance used hard_cmp and an original test
incorrectly asserted no medium reset. Fix this before adding another algorithm.

Original late_medium_regression.rs first ran against old code: both full-history
and basic-search tests failed (work/late-medium-before.log, exit 101). Stale history
left two old slots and search completed 3 steps instead of stopping after 1. The
new Score::late_acceptance_cmp defaults to hard_cmp for custom scores. Fixed integer
and decimal HardMediumSoft implementations compare hard plus medium; other fixed
families explicitly use their original prefixes, and bendable/custom defaults keep
hard-only behavior. Feasibility, HARD_LEVEL_COUNT, hard_cmp, full score ordering
and other acceptors are unchanged. LateAcceptance now uses this comparison only
when a completed step improves its retained global best.

Six adapted DefaultLevelScoreStateTest methods preserve complete initial/new-best
fixtures and reset/no-reset assertions: hard/soft HardSoft, medium/soft HardMediumSoft,
hard/soft Bendable(1,2). Explicit pure comparisons replace mock step-scope indices
0/1 and score definitions; three index/baseline-specific methods remain pending.
Provenance/notices updated. No Java lifecycle/cache-index equivalence claimed.

Five original regressions check complete history, four history lengths across 240
completed steps against separate best/prefix/cursor reconstruction, worsening/ties,
exact tiny decimal differences beyond f64 precision, scale equality, bendable soft
levels, structural exclusion, preserved feasibility and all six local-search paths.
All six paths now stop after the single medium improvement in bounded fixtures.
The previous incorrect original medium test was corrected from the pinned evidence.
A new Score doctest explains reset levels versus feasibility. Focused tests pass
(work/late-medium-focused.log, work/late-level-focused.log). No new Java solver-trace
or dedicated performance comparison; do not infer those from the native regression.

Contract docs now correct the earlier hard-only statement. Historical handoff text
that said medium does not reset is superseded by this increment. Source frozen for
full gate session 6437, log work/late-level-check.log.

Full gate completed exit 0: 1,642 Rust tests (825 adapted, 741 original integration,
76 unit), 84 doctests and 193 Python tests. Coverage 825/4,855 = 16.99%, search
31/158 = 19.62%; pending 3,748, excluded 282, failed/unverified zero. The six reset
methods pass; three DefaultLevelScoreStateTest lifecycle/index methods remain
pending. Release plus thirteen sequential refreshes use session 46832, logs
work/late-level-release.log and work/late-level-refresh-*.log. Source remains frozen.

Release and refresh session 46832 completed exit 0. All 11 new tests pass release;
all thirteen datasets are current and report/source/verification/live-server hashes
match (work/late-level-audit.py). Browser confirms 16.99% overall, 19.62% search,
DefaultLevelScoreStateTest six Passed and three Pending, and 43 timing cards.
No page errors or horizontal overflow at 1440px/390px. Browser closed and
localhost:8765 remains. QA tip: count status text together with the class name;
a broad Passed-text regex also counts the filter option (corrected and rerun).

The rewrite remains incomplete. Next search candidates remain composite acceptors,
diversified late acceptance and value/move tabu. Composite configuration must handle
existing Copy configuration usage without silently limiting composition; diversified
source tests mutate trial history and require a careful specification before design.
Do not implement production code by translating upstream internals. Public seeded
fading-quality comparison and dedicated timing remain pending from the prior increment.

## In progress: ordered composite acceptance

Native Acceptor::All/from_policies and LocalSearchConfig::additional_acceptance
combine an arbitrary slice of policies in order. Trials short-circuit, completed
steps update all children (including rejected fallback), and all configs validate
before state allocation/scoring/factories/events. Empty extras preserve single-policy
execution; standalone empty All accepts everything. Any child tabu activates key
tracking across all six paths. Scoped borrowed slices preserve Copy configs; static
factories require static policy slices and score types. Contract docs state these
limits and distinguish native state ordering from Java lifecycle equivalence.

One CompositeAcceptorTest.isAccepted adaptation preserves all five truth patterns.
phaseLifecycle remains pending. Provenance/notices updated. Six original composite
regressions cover short-circuit counters/randomness, step broadcasts, empty/nested/
large composition, validation-before-allocation, fresh state, basic/list/mixed tabu
keys and aspiration isolation. Existing paired backend matrices test both empty
and combined extras. A new owned-score matrix and later-phase validation test add
two more original cases. The latter initially expected the wrong Failure::Phase;
actual Failure::Acceptance was correct, and the test expectation was fixed after
inspection of existing acceptance-validation tests. Focused suites now pass.

The public Java and native heuristic drivers add late acceptance size 3 plus tabu
size 2, global-best aspiration and native hill shortcut. Smoke with 3 steps/no
warmup/one sample passed all 160 cases (32 per five algorithms), including all 32
combined equal scores and both combined timing workloads. Smoke numbers are not
performance evidence. Full default benchmark will replace them. Source now frozen
for full gate session 79875, work/composite-check.log. Expected 1,651 Rust tests
(826 adapted, 749 original integration, 76 unit), 85 doctests and 193 Python tests;
verify actual output. Final refresh should yield 13 current datasets and 45 cards.

Full gate completed exit 0 with the expected 1,651 Rust tests, 85 doctests and
193 Python tests. Coverage 826/4,855 = 17.013%, search 32/158 = 20.253%, pending
3,747, excluded 282, failed/unverified zero. Seven new standalone tests also pass
release. Thirteen sequential refreshes run under session 33710 with logs
work/composite-refresh-*.log; final report generation is included in that process.

Next search candidates remain diversified late acceptance and value/move tabu.
The complete diversified source test was inspected (tests only): it mutates history
on isAccepted, explicitly injects history/worst-occurrence state, and asserts eight
replacement branches. Pinned local-search docs describe a different acceptance/
replacement strategy. Establish a precise native trial/step contract before adding
it; do not mechanically extend current step-only LateAcceptance. MoveTabuAcceptorTest
uses mock move identities; an actual native search policy needs a stable structural
move-identity contract across generated candidates, not variable keys relabeled
as moves. Neither feature nor its source-method coverage is implemented here.

Refresh session 33710 completed exit 0. Audit work/composite-audit.py confirms
matching source/report/verification/live-server fingerprints, 13 current datasets,
45 timing cards and CompositeAcceptorTest one Passed / one Pending. Default
heuristic run used 50 steps, 5 warmups, 7 samples and 3 fresh forks (21 samples per
runtime/workload): all 160 small cases valid and equal-score, 132 optimal per runtime.
Combined 32-variable medians Rust 2.669 ms / Java 12.195 ms, both optimal 0/0;
96-variable medians Rust 17.223 ms / Java 19.000 ms, both infeasible -44/-22.
Do not generalize those ratios or omit the latter quality limitation. All existing
reference discrepancies remain retained in the refreshed datasets.

Browser QA confirms 17.01% overall, 20.25% search, 45 cards, ten heuristic cards,
both combined cards with quality/ranges/call counts, and composite filter results
isAccepted Passed / phaseLifecycle Pending. No page errors or horizontal overflow
at 1440px and 390px. Browser closed; localhost:8765 remains running. Documentation
and source/provenance were inspected before the Conventional Commit. The rewrite
is still incomplete; this increment is concrete progress, not goal completion.

## In progress: typed move tabu (2026-10-04)

Previous goal turn made concrete progress and committed 21b629d. Tree was clean.
This increment adds hard and fading move tabu, using pinned MoveTabuAcceptorTest
and the public local-search documentation (reject moves equal to recent steps).
Only the public withMoveTabuSize API signature was searched; no production logic
was copied or translated. Native moves already have Eq/Clone, including list and
mixed descriptors. Identity is structural, with order, positions and swap orientation
significant; inverse/successor-state equivalence and Java equality are not claimed.

New MoveAcceptor<S,M> stores ordinary Acceptor state, hard/fading move memory or
ordered children. accepts/step_ended require both descriptor and variable keys.
Acceptance::start_with_moves constructs it; legacy scalar start reports
MoveContextRequired for move policies. LocalSearchConfig starts typed state in all
six paths, with any extra policies; existing variable-key tracking remains separate.
No hashing or generic Hash bound. Trials never clone descriptors into memory;
selected steps clone only when move memory needs the key. All policies validate
before construction, and every child sees completed rejected-fallback steps.

Three complete source methods are adapted with stable IDs, fixed draws and all
assertions; provenance/notices updated. Seven original regressions cover structural
identity, chronological expiry/refresh/aspiration, independent memories, skipped
random draws, clone counts, invalid configs and all six search paths against an
independently retained selected-move history. Paired-backend and owned-score matrices
include both policies. All focused/integration suites passed. No scoped Java
lifecycle, general move-equality, PRNG or fading timing parity claimed.

Public Java/native heuristic drivers add move-tabu tenure 2. Three-step/no-warmup
smoke passed 192/192 valid and equal-score cases (32 per six configurations) plus
12 timing workloads. These smoke timings are not performance evidence. Full gate
session 74212 is running in work/move-tabu-check.log; source frozen. Expected
1,661 Rust tests (829 adapted, 756 original integration, 76 unit), 86 doctests and
193 Python tests. After success run focused release and all 13 sequential dataset
refreshes, final report, fingerprint/status audit and browser QA for 47 cards.

Full gate session 74212 completed exit 0: 1,661 Rust tests, 86 doctests and
193 Python tests. Coverage 829/4,855 = 17.075%, search 35/158 = 22.152%, pending
3,744, excluded 282, failed/unverified zero. All ten new tests pass release.
Thirteen sequential refreshes run in session 83711, logs work/move-tabu-refresh-*.log.
Final report generation follows in that same process. Audit work/move-tabu-audit.py
expects 13 current datasets, 47 cards and MoveTabuAcceptorTest 3/3.

Next candidates: value tabu, ratio sizing, diversified late acceptance. ValueTabu
source tests expose planning-value lists through mock moves, but actual native
search needs a clear value extraction contract (basic change/swap/composite, list
spans/positions, optional None and mixed B/L namespaces) before adopting the method
credit. Do not treat descriptor identity as value identity. Diversified source
history mutation and injected-state tests still require the earlier noted trial/
step specification. None of those features is implemented in this increment.

Refresh session 83711 completed exit 0. Fingerprint/live-server audit passes with
13 current datasets, 47 cards and MoveTabuAcceptorTest 3/3. Full default heuristic
run (50 steps, 5 warmups, 7 samples, 3 forks) validates 192/192 small cases with
equal final scores; 158 optimal per engine. Move-tabu medians: 32 variables Rust
2.212 ms / Java 10.019 ms, both optimal 0/0; 96 variables Rust 14.462 ms / Java
17.325 ms, both infeasible -44/-22. Each workload retains 21 samples per engine,
full score-call counts and ranges. These workload ratios are not general speed
claims, and the infeasible case must remain visible. Existing discrepancies persist.

Browser confirms overall 17.08%, search 22.15%, all three move-tabu methods Passed,
47 cards and 12 heuristic cards. Both new cards show quality/feasibility, known
optimum, score calls and timing ranges. No page errors or horizontal overflow at
1440px/390px. Browser closed and localhost:8765 remains running. Source/tests,
provenance and staged diff were reviewed before the Conventional Commit. The
full rewrite remains incomplete; this increment is progress, not goal completion.

## In progress: value tabu (2026-10-04)

Previous turn made concrete progress and committed da122fc; worktree was clean.
This increment adds ValueTabu/FadingValueTabu and ValueAcceptor<S,M,K>. Existing
MoveAcceptor facade and the new value facade share a private generic context state;
scalar/move-only entry points reject missing value context. Independent variable,
move and value memories compose in order with existing trial/step semantics.

Public Move.getPlanningValues Javadoc specifies assigned values, permits null,
and says it is observed after execution. Pinned tests retain null in value tabu.
Native basic metadata records actual target writes, including None and intermediate
composite writes. List metadata records moved/assigned/removed items, not shifted
neighbors. Both use private recording callbacks in existing native apply paths,
replaying snapshots only when requested. Composite children observe prior writes;
final model validation remains at the original boundary. Mixed metadata validates
the whole move and uses distinct Basic(Option<B>)/List(L) identities. This replay
cost is explicit and included in value-tabu timing; it invokes no scoring callbacks.

All six search paths capture trial and selected metadata from the same original
step snapshot, only for configured value policies. Paired-backend and owned-score
matrices now include hard/fading value policies. Seven original regressions cover
write/intermediate/null capture, ranges, list spans, shifted-neighbor exclusion,
input immutability, mixed namespaces, chronology, contexts/configs, skipped draws,
independent memories and six-backend search against independent primitive-value
extraction and selected-key histories. A composite range regression confirms that
transient cross-range values are allowed when the final lists restore valid ranges.

Five ValueTabuAcceptorTest methods are completely adapted, preserving 96 hard/
multiple/aspiration assertions, seven null assertions and eight fading decisions
plus draw-consumption checks. Stable identities and explicit global-best baseline
replace mocks; provenance/notices updated. Ephemeral work/adapt-value-tests.py
checks that every source assertion in the first three methods is represented.
The initially generated literal bool assert_eq assertions triggered Clippy; they
were converted to assert! without suppressing the lint. Full gate rerun session
56691 is now active, log work/value-tabu-check.log. Source is frozen.

Three-step/no-warmup benchmark smoke passed 224/224 valid and equal-score cases
(32 per seven configurations), including new value_tabu with Java public
withValueTabuSize(2). These are not timing evidence. Expected full gate totals:
1,673 Rust tests (834 adapted, 763 original integration, 76 unit), 87 doctests,
193 Python tests. After success run focused release and all thirteen sequential
refreshes, then final report/audit/browser checks for 49 cards and ValueTabu 5/5.

Full gate session 56691 completed exit 0 with the expected 1,673 Rust tests,
87 doctests and 193 Python tests. Coverage 834/4,855 = 17.178%, search 40/158 =
25.316%, pending 3,739, excluded 282, failed/unverified zero. Focused release passes
all 12 new cases plus seven existing move-tabu regressions. Source remains frozen.
Thirteen sequential refreshes and final report run in session 37437, logs
work/value-tabu-refresh-*.log. Audit work/value-tabu-audit.py expects 13 current
statuses, matching fingerprints, 49 cards and ValueTabuAcceptorTest five passed.

Next useful work includes public Java value-metadata comparisons for richer list/
composite operations, optimizing capture replay only with preserved invariants,
ratio-based tabu sizing and diversified late acceptance. Current Java timing/quality
checks cover complete basic change moves; they do not establish general Java list/
composite planning-value identity. Public fixed-draw fading adaptations do not
establish cross-engine seeded stochastic traces or dedicated fading performance.

Refresh session 37437 completed exit 0. Audit confirms 13 current datasets,
matching source/report/verification/live-server fingerprints, 49 timing cards and
ValueTabuAcceptorTest 5/5. Default heuristic run uses 50 steps, 5 warmups, 7 samples,
3 forks and retains 21 samples per runtime/workload. All 224 small cases validate
with equal final scores, 185 optimal per runtime. Value-tabu medians: 32 variables
Rust 2.650 ms / Java 9.508 ms, both optimal 0/0; 96 variables Rust 14.403 ms / Java
17.414 ms, both infeasible -44/-22. Capture replay is inside the native timing.
These are bounded workload results, not general speed or quality claims. Existing
cross-runtime discrepancies remain retained in the other refreshed datasets.

Browser QA confirms 17.18% overall, 25.32% search, all five source methods Passed,
49 cards and fourteen heuristic cards. New cards expose quality, feasibility,
known optimum, ranges and score calls. No page errors or horizontal overflow at
1440px/390px. Browser closed; localhost:8765 remains running. Staged implementation,
adaptations, provenance, notices and contracts were reviewed. The full solver
rewrite remains incomplete; this increment is concrete progress toward it.

## In progress: list value metadata comparison (2026-10-04)

Previous increment committed 649aa0f; clean worktree at start. Added optional
--planning-values mode to the existing Java and native list adapters. Default
list/shadow protocol unchanged. New independent differential_list_values.py
replays current child input identities and compares native first-touch order;
Java getter is called after execution and compared by identity set. Both
temporary/undo/commit paths checked; raw Java order/multiplicity retained.
Original corpus unchanged; metadata adds two original minimal reproductions.
No upstream adaptation or new method credit, no production Rust behavior changes.

First complete run: baseline 512/512 states, metadata native 514/514, Java
425 agreements / 16 differing sets / 40 errors / 33 unsupported empty composites.
Minimal swap+change includes untouched value in Java getter; swap+unassign
throws index error. Documented in list-planning-contract.md. Seven new Python
tests guard evidence protocol, wrong state/identity, native dedup/order, reference
sequential capture, Java set normalization, errors and unsupported classification.
Dashboard shows separate metadata evidence row; no design skill needed.

Source frozen. Full gate session80779 running, log work/list-values-check.log.
Expected unchanged 1,673 Rust tests / 87 doctests, Python increased193→200.
Then refresh all13 measurements, regenerate report, audit current hashes/49cards,
browser QA of new evidence at desktop/mobile, inspect staged diff and commit.

Full gate session80779 completed exit0: Rust counts unchanged, 87 doctests and
200 Python tests pass. All13 sequential dataset refreshes running session77152,
logs work/list-values-refresh-*.log. Source frozen; no implementation changes
during measurements. Focused release adapter already exercised all514 native
metadata cases twice and baseline512 full state cases before this gate.

Refresh session77152 completed exit0. Audit work/list-values-audit.py confirms
all13 current datasets, source/verification/live-server fingerprints, unchanged
834/4855 upstream coverage and49 timing cards. Refreshed metadata counts exactly
reproduce the first run, with56 retained Java error/difference records and no
native/protocol failures. Java default state/undo/commit mode independently
passes both added minimal reproductions without the metadata getter.

Browser QA at1440px/390px confirms17.18%,49 cards and new425/514 evidence row,
with514/514 native,16 differences,40 errors,33 unsupported explicitly visible.
No page errors or horizontal overflow; browser closed, server8765 remains alive.
Full gate and staged diff reviewed. This is verification progress; goal remains
incomplete. Next useful work: ratio-based tabu sizing or diversified late
acceptance from pinned Community tests/public APIs, or capture optimization
while preserving independent first-touch contracts and honest Java differences.

## In progress: diversified late acceptance (2026-10-04)

Previous turn was progress: b0c096c, clean at start. Implemented independent
DiversifiedLateAcceptance using pinned tests/public docs and the linked DLAS
paper (arxiv1806.09328), without reading production Java acceptor implementation.
Accepted candidates update current slot under replacement conditions; rejected
trials do nothing. Completed steps rotate only, including fallback; no reset on
hard-level improvement. Recompute worst/count after replacement, O(history size).
Added configuration/Acceptor routing, making policy available in all six paths.
Ordered conjunction semantics explicit. Five original regressions cover rotating
multi-level reference, boundaries/repeated trials, order/short-circuit/fallback,
validation/fresh phase, and independent basic network/stream search selection.
Existing owned-score six-backend and list-stream matrices include policy.

Both pinned source methods fully adapted in private-state module unit tests,
including all manually inconsistent worst/count fixtures. Provenance uses
crates/solver/src/lib.rs plus qualified names diversified_late::cases::...
Report parser now recognizes only timetuck library executable for those results;
run_local uses -p timetuck --lib. Existing parser regression expanded to check
passing, failed, ignored and unrelated library/example boundaries. Notices updated.

Added diversified algorithm to Java/Rust heuristic adapters and driver, public
Community preview, history3. Three-step/no-warmup smoke:256/256 valid and equal
final scores,176 optimal/runtime. Not performance evidence. Full defaults still
needed after gate, expected51 timing cards/16heuristic cards. Focused new unit,
original regressions, owned-score and list-stream suites passed. Initial original
test compile errors (untyped config, nonexistent Stop spelling, temporary borrow,
wrong grouping signature) fixed; full assignment stream uses three filtered joins.

Source frozen. Full gate session95009 running, log work/diversified-check.log.
Expected1680 Rust tests (836 adapted,768 original integration,76 original unit),
87 doctests,200 Python. Then focused release as useful, all13 sequential refreshes,
report/currentness audit, browser test new class2/2 and51cards, inspect and commit.

Initial full gate95009 passed all expected totals and reported836/4855 (17.219%),
search42/158 (26.582%). Focused release passes2adapted+5original tests. Review
then scoped the Java preview flag to algorithm=diversified instead of enabling
it for every benchmark mode. No Rust change. Active refresh42832 correctly
failed source stability during its first aggregate dataset (exit1); its remaining
steps did not run. Revalidation and all13 refreshes restarted sequentially in
session80901, logs work/diversified-final-check.log and diversified-refresh-*.
Source now frozen again. This is a verified stopped process, not a timeout restart.
Pinned AcceptorFactoryTest.diversifiedLateAcceptanceAcceptor separately confirms
that explicit DIVERSIFIED_LATE_ACCEPTANCE plus lateAcceptanceSize produces the
single diversified acceptor. Read only test assertions; no factory implementation.
Audit work/diversified-audit.py expects836 coverage,2new class passes,51cards,
all13 current datasets and unchanged explicit list-value discrepancies.

Corrected-source full gate in80901 completed successfully:1680Rust,87doctests,
200Python. Sequential measurements now running; aggregate and sequence completed.
No further source changes. New history has linear recomputation cost, explicitly
included in its measured solve. Final card/result audit still pending.

Session80901 completed exit0 with all13 refreshed datasets and regenerated report.
Audit confirms source/report/verification/live hashes,836/4855, new class2/2,
51 timing cards and preserved list-metadata distinctions. Ephemeral audit copied
an old count via overbroad string replacement (512 became212); corrected that audit
literal and reran successfully. No project/source change was involved.
Default heuristic50steps,5warmups,7samples,3forks:256/256 valid and equal-score
small cases,210 optimal/runtime. New diversified medians:32variables Rust2.497ms /
Java13.249ms, both0/0optimal;96variables Rust15.615ms / Java24.279ms, both-44/-22
infeasible.21samples/runtime/workload, ranges and score-call counts retained.
These bounded results do not establish general speed or identical Java traces.

Browser QA1440px/390px:17.22%overall,26.58%search,both new methods Passed,
51cards/16heuristiccards, quality/feasibility/ranges/score calls visible. No errors
or horizontal overflow. Browser closed; server8765 remains alive. Staged diff,
provenance, source assertions and final verification reviewed before commit.
Goal remains active/incomplete. Useful next work: ratio-based tabu sizing from
EntityRatioTabuSizeStrategyTest and FixedTabuSizeStrategyTest (already read this
turn); or richer public Java trial/step acceptance evidence. Do not infer Java
trace parity from the new final-score comparisons. No subagents were used.

## In progress: population-relative tabu sizing (2026-10-04)

Previous turn was progress, committedcc58d1e, clean at start. Added validated
TabuSize::fixed(i64)/ratio(f64), private representation preservesCopy/Eq with
canonical signed zero. Ratio interval[0,1], finite; positive nearest-half-up
binary64 product, exact final cap[1,population-1], counts0/1resolve1. Fixed positive
conversion rejects overflow on narrower targets. Endpoint/tiny population rules
are original native extensions, not claimed from the six source ratio fixtures.

SizedVariableTabu integrates through LocalSearchConfig::start_acceptance(population)
across all six scoring/search paths. Basic count=all variables, list=all owners,
mixed=sum, including pins. Additional policies are resolved in a fallibly
allocated temporary vector. No implicit missing population: standalone users call
for_population first; otherwise PopulationContextRequired. Existing ordinary
policy settings retained. Fixed/fading/value policies still available unchanged;
no automatic value/move/fading ratio expansion this increment.

Three source methods fully adapted: EntityRatioTabuSizeStrategyTest.tabuSize,
FixedTabuSizeStrategyTest.tabuSize/invalidTabuSize. Provenance/notices updated.
Five original regressions pass: exact binary-fraction integer reference rounding,
bounds/NaN/zero/huge counts, standalone context/expiry, all six backends against
explicit fixed controls withpins and additional policies, empty/reused configs.
Owned-score six-backend and list-stream matrices additionally include sizing.
Focused session62363 completed exit0. Public Java/native ratio_tabu benchmark
adds0.2ratio and aspiration. Default workloads resolve6/19 for32/96variables.
Smoke session37878 running, log work/tabu-size-smoke.log (not timing evidence).

Source frozen; full gate83095 running work/tabu-size-check.log. Expected1688Rust
(839adapted,773original integration,76original unit),87doctests,200Python.
After success, focused release, all13 sequential refreshes, regenerate/audit
report, browser verify3source methods and53cards/18heuristiccards, review/commit.

Full gate83095 completed exit0 with1688Rust,87doctests,200Python. Report credits
839/4855 (17.281%), search45/158 (28.481%), pending3734,excluded282,zero failed
or unverified. Smoke37878 finished288/288valid/equal-score cases at3steps;
not timing evidence. Focused release plus all13 sequential refreshes now running
session58813, logs work/tabu-size-release.log and tabu-size-refresh-*.
Audit prepared in work/tabu-size-audit.py for13current datasets,839coverage,
all3new methods,53cards/18heuristiccards,288validationcases and21samples per
runtime/workload. Source remains frozen; browser closed until timings finish.

Refresh58813 completed exit0. Audit confirms all13current datasets, exact source/
verification/live fingerprints,839coverage,3new method passes,53timing cards and
18heuristic cards. Default50steps/5warmups/7samples/3forks:288/288small cases valid
and equal-score,238optimal/runtime. Ratio-tabu32variables Rust2.530ms/Java12.450ms,
both0/0optimal;96variables Rust15.987ms/Java17.434ms,both-44/-22infeasible.
21samples/runtime/workload; ranges and score calls retained. These are bounded
workload comparisons, not general speed or Java trace equivalence.
Browser1440px/390px confirms17.28%,28.48%search,all3methods Passed,53cards and
18heuristiccards. Both new cards showquality/feasibility/ranges/scorecalls. No
pageerrors or horizontaloverflow; browserclosed,server8765stillrunning. Final
stagedimplementation,tests/provenance and required gate reviewed before commit.
Full goal remains active/incomplete. Next useful work: population-relative value
or fading tabu with explicit key-universe semantics; or remaining move metadata
adaptations after the public list comparison. Do not erase known Java metadata
errors/differences or claim Java trace parity from final-score equality.

## In progress: sparse basic planning-value capture (2026-10-04)

Previous increment committed d9fc36d. Basic capture now clones only distinct
outputs; composites use a sparse map of borrowed values at written indices.
Input validation order, intermediate writes, first equal payload and None remain
observable. Basic apply no longer routes through an unused recording callback.
List/mixed capture semantics and known Java metadata differences are unchanged.
Six complete mass/pillar/list/sublist metadata methods adapted with provenance
and notices. Four original regressions compare 1500 generated cases against
independent full-array execution and check 10000-variable clone bounds, identity
payloads, invalid queries and clone/equality panics without input mutation.

Required gate74391 completed exit0: expected1698Rust (845adapted,777original
integration,76original unit),87doctests,200Python. Source frozen. Focused release
and all13 sequential measurement refreshes running session15868, logs
work/value-capture-release.log and work/value-capture-refresh-*.log. Afterward:
report/audit fingerprints and845/4855coverage, browser QA53cards/18heuristics
and six new method passes, final evidence, staged review, conventional commit.
Full goal remains active/incomplete; no subagents or frontend redesign used.

During review, clarified the mass-change provenance entry: its source and native
query both run before execution (the other five retain capture across execution).
No measurement source changed. This changes the verification digest, so repeat
scripts/check.sh after session15868's measurements finish, then regenerate report.
The final report step in15868 may reject the old verification digest as expected.

Possible next implementation: native list gather builder, specified by the five
execution/undo methods in builtin/MassListChangeMoveTest.java (read fully this
turn). It gathers sampled identities in draw order, adjusts a pre-move destination
only for sampled members originally before it, supports currently unassigned
members and optional null-destination unassignment, and rolls back. No native
mass-list/gather API exists yet. Existing ListAssignment::location and ListMove
Composite/Change/Assign/Unassign can support an independently designed builder.
The source's equality/hash/rebase methods have separate semantics and must not
receive credit merely from the execution builder. Production Java was not read.

All13 measurements completed successfully in15868; its final report correctly
rejected the old provenance fingerprint. Repeated required gate14634 completed
exit0, including1698Rust,87doctests,200Python; focused release17tests also passed.
Final audit work/value-capture-audit.py verifies13current datasets and matching
source/verification/live digests,845/4855methods (17.405% report;17.40%UI from raw
counts),3728pending,282excluded,zero failed/unverified. Moves119/874(13.62%).
All six new methods Passed;53timingcards,18heuristiccards. Validation288/288valid
and equal-score,238optimal/runtime. Defaults50steps/5warmups/7samples/3forks,
21samples/runtime/workload. Value-tabu32variables Rust2.504ms/Java9.645ms,
both0/0optimal;96variables Rust15.252ms/Java15.375ms,both-44/-22infeasible.
These refreshed bounded workloads are not a controlled before/after optimization
experiment or a general speed/trace-parity claim. Clone counters prove the reduced
capture cost directly. List metadata remains514native/425Javaagreements,
16differences,40errors,33unsupportedemptycompositions.

Browser1440px/390px confirms six new methods,17.40%coverage,13.62%moves,
53cards/18heuristics, both value-tabu cards with quality/feasibility/ranges/calls.
No browser errors or horizontaloverflow. Screenshots visually inspected; browser
closed,server8765remainsalive. Staged implementation/test/provenance review before
commit; full rewrite goal remains active/incomplete. Next gather candidate above.

## In progress: selected list gathering (2026-10-04)

Previous turn was progress, committed28bcf8d; clean at start. Added independent
ListProblem::gather_move in list_gather.rs and ListGatherError. Validates before,
duplicate/unknownmembers, originalboundary; removes selected identities into a
private snapshot, inserts in sampleorder at the surviving-prefix boundary,
validates finalpins/ranges, returns ordinaryComposite bound to originalstate.
Assignedpayloadsretained;newassignedusesinputpayload. Empty validatednoops;
Noneunassigns assignedmembers (requiredmodels maybepartial as withUnassign).
No new enumvariant or solverpathdispatch needed. No Java productionread.

Five complete MassListChangeMoveTest execution/undo adaptations withprovenance.
Four original tests:29835exhaustiveorderedcases/candidate/undo/commit/metadata,
260steps pairedlist-scoringbackendsmatchingindependentfullformula, invalidinput/
pins/ranges/noops, payloadandclonepanic. Focuseddebug15601passed9tests.
New original gather comparison256cases,seed20261212 viapublic Moves.massChange.
Java/Rust adaptersacceptg onlyasnonemptysingleoperation, existing512cases intact.
Python reference emits at original boundaries; bothfullstatesandfourshadows checked.
Smoke62897exit0:256/256gather,512/512ordinary, existingmetadata425/514withunchanged
16differences/40Javaerrors/33unsupported. ThreePythonreference/generatorchecks pass.
Dashboard adds separate evidence row; no newtimingcardorupstreamcreditforgenerated.

Source now frozen. Required gate thenfocusedrelease,all13sequentialrefreshes,
finalreport/audit/browser/stagedreview/commit remain. Expected1707Rust
(850adapted,781originalintegration,76originalunit),88doctests,203Python;
850/4855=17.508%report (17.51%UI),moves124/874=14.19%,pending3723,excluded282.
Goalactive/incomplete. No subagents. Server8765alive;browserclosed.

Required gate85392 completed exit0. Full850method report generated;1707Rust,
88doctests,203Python expected/green. Focusedrelease plus13sequentialrefreshes now
running session37877, work/gather-release.log and gather-refresh-*.
Audit prepared work/gather-audit.py for850coverage,5methods,256gathercomparisons,
13currentdatasets,53cards/18heuristics andunchangedlistmetadata. Sourcefrozen.

Focusedrelease9tests passed. Refreshed list comparison again256/256gather and
512/512existing states; unchangedmetadata425/514withnative514/514. All benchmark
refreshes remain sequential; no browser/test workload overlaps timed runs.
Staged implementation/reference tests and provenance reviewed while measurements
run; finalbrowser and actualcommit await successfulrefresh/audit.

Refresh/audit37877 completed exit0. All13datasetscurrent with matching source,
verification and livefreshness digests. Summary850/4855(17.508%;17.51%UI),
3723pending,282excluded,zero failures/unverified. Moves124/874(14.19%UI).
MassListChangeMoveTest precisely5passed/3pending;256/256gathercomparison added
separately.53timingcards/18heuristics;288/288smallheuristiccasesvalid/equal-score,
238optimal/runtime atdefault50steps/5warmups/7samples/3forks. Gather itself has
no timing comparison yet; do not attribute unrelated refreshed timings to it.
Native builder currently uses private full snapshots and composite operations.

Browser1440px/390px verifies17.51%,14.19%,5gatherpasses/3pending, separate
256/256evidencerow and53cards. No pageerrors/overflow; screenshots reviewed.
Browserclosed,server8765alive. Allstagedsource/tests/provenance reviewed. Commit
this increment normally; fullgoal remains active/incomplete. Next useful work:
matched gather transaction benchmarks, or native gather search neighborhood and
mixed-backend integration with explicit selection/termination semantics. Do not
credit remaining equality/hash/rebase methods from structural Composite equality.

## Bounded gather search neighborhood (2026-10-04)

Previous turn was progress, committed 7befd0b; the worktree was clean at start.
Added list_gather_moves and Neighborhood::Gather { max_size }. Lazy permutations
follow destination, group size, then registry order. Pins, destination ranges and
the identity immediately preceding each boundary are pruned before enumeration.
This avoids scanning factorial groups that only duplicate earlier boundaries.
Unchanged results are skipped. Optional unassignments come last as canonical
assigned subsets. Zero disables list moves; oversized bounds are clipped.
Basic search retains changes/swaps, and mixed joint search combines both families.
Entity shuffling changes destinations while member permutations keep registry order.
Each yielded gather still uses the existing private-snapshot composite builder.

Five original integration tests cover 108 small models against an independent
successor-set reference, order/bounds/frozen models, 80-value clone laziness,
180 paired list/mixed search configurations, and both basic backends. A private
unit test checks permutation counts, uniqueness, ordering and fused exhaustion.
No upstream method credit or Java search-order/trace equivalence is claimed.

Focused checks passed. The initial gate found a test-only Clippy useless_vec;
that was fixed. After the earlier-boundary pruning improvement, final gate 53379
passed 1,713 Rust tests (850 adapted, 786 original integration, 77 original unit),
89 doctests and 203 Python tests. Logs: work/gather-search-final-check.log and
work/gather-search-focused.log. Focused release tests also passed.

Sequential refresh and audit 84718 completed successfully. All 13 datasets match
current source, verification and live fingerprints. Coverage remains 850/4,855
(17.51% UI), with 3,723 pending, 282 excluded and zero failed/unverified. Moves
remain 124/874 (14.19%). Existing 256 gather comparisons, 512 list cases and the
425/514 Java planning-value agreements remain unchanged. There are 53 timing
cards, including 18 heuristic cards; 288 small heuristic cases are valid and
have equal final scores, with 238 optimal per runtime. Gather search itself has
no Java search or timing comparison yet.

Concurrent host CPU activity was observed during the later heuristic/pillar runs.
work/gather-search-host-load.json records UTC 2026-10-04T06:50:59.334102+00:00,
load averages 39.58/24.33/15.65, and process names/CPU usage. Added this observation
and a methodology note to the two work measurement files without altering samples.
They explicitly disclaim an isolated-host baseline or controlled before/after
comparison. Regenerated report and audit 43249 passed; source code was unchanged.
Future measurements replace this one-run annotation. Automatic host-load recording
would improve the benchmark tools, but requires its own verification.

Browser checks at 1440 and 390 pixels confirm current coverage, 53 cards, 18
heuristic cards, no stale indicator, no errors and no horizontal overflow.
Expanding #methodology displays both host-load notes. Screenshots were reviewed.
Browser closed; server 8765 remains alive. Final staged review precedes commit.
The full goal remains active and incomplete. Useful next work: matched gather
transaction/search benchmarks, or profiling incremental-stream workloads against
Java. Do not interpret the contended-host timings as feature performance changes.

## 2026-10-04: Value-only snapshot replacement

Continued the active independent Community solver goal with native stream-search
profiling. No Timefold production implementation was read or translated. Before
binary is from 6934f43; profiler is locally extracted Valgrind 3.22.0.

SnapshotMap existing-key replacement now walks its prepared path iteratively,
copying shared nodes but skipping height/size refresh and balance checks. The tree
shape cannot change. Equal-key representatives, unchanged payload sharing and
Send/Sync remain intact. Insert/remove paths are unchanged. The existing exact-root
comparison-panic regression additionally tests replacement in shared and unique
maps. No method credit or test-count increase is claimed.

Focused five map tests passed. Full gate 35343 passed 1,713 Rust tests, 89 doctests
and 203 Python tests; work/value-replacement-check.log retains output.
The four original stream workloads (hill/tabu, 32/96 variables, 50 steps) reduced
Callgrind instruction references by 6.65%, 6.89%, 6.63% and 6.86%. Each output was
independently rescored; assignments, scores and terminal callback counts stayed
identical to the retained before binary. These are instruction counts, not elapsed
speedup, allocation improvement, Java parity or a general performance guarantee.
See docs/constraint-stream-contract.md and work/profiling/value-replacement/.

That new profiling directory includes binaries, inputs, runner, profiles and hashed
manifest. Its README notes an artifact-handling error: the first command reused
the older experiment's work/profiling/before.callgrind name before its existence
was noticed. The new file was moved into the new directory; the older raw file is
unavailable. Old logs, outputs, before binary, DHAT and original manifest remain.
Do not attribute the new profile to that old manifest.

Useful next functional increment: AcceptedLocalSearchForagerTest has seven pending
methods. Only this test source and existing native code were inspected while the
refresh ran. It checks accepted-vs-rejected priority, best-score/last-step early
picks, random/first ties, per-step accepted-candidate limit and structurally flawed
rejection. Native MoveSelection currently supports FirstAccepted/BestAccepted only;
max_accepted_moves is a phase budget, not that per-step candidate limit. Preserve
all fixture scores, intermediate isQuitEarly assertions and supplied-draw semantics
when adapting; record provenance and integrate behavior across all six backends.
StrategicOscillationByLevelFinalistPodiumTest has four further pending methods.
Only method names in that file were inspected. Full goal remains active/incomplete.

Sequential profile/refresh/audit session 93021 completed successfully. All 13
measurement datasets and report/source/verification/live fingerprints are current.
Coverage remains 850/4,855 (17.51% UI), 3,723 pending, 282 excluded, zero failed or
unverified. All 53 timing cards remain, including 18 heuristic cards. Heuristic
validation: 288 valid/equal-score cases, 238 optimal per runtime. The 256 gather
comparisons and recorded planning-value/concurrent limitations are unchanged.
Logs: work/value-replacement-refresh-*.log; audit: work/value-replacement-audit.py.

Refreshed stream medians (Rust/Java milliseconds): hill32 70.260/27.925,
hill96 256.389/40.826, tabu32 105.008/35.266, tabu96 260.767/40.294. Final scores
agree: 0/0 at32 and -44/-484 at96 (both infeasible at96 under the fixed budget).
The host is shared; these are not isolated before/after elapsed-time measurements.
Profiler metadata retains load observations; an additional observation is saved
in work/value-replacement-host-load.json. No manual timing samples were modified.

Browser verification at 1440 and 390 pixels confirmed 17.51%, 53 timing cards,
18 heuristic cards, current stream figures, no errors and no horizontal overflow.
Search algorithms drilldown/filter lists all seven forager methods as pending.
Browser is closed; server 8765 remains running. Final staged diff reviewed before
commit. No processes from this increment remain active.

## 2026-10-04: Per-step foraging and seeded ties

Continued the full active goal from clean 62d7fe1. Previous increment was verified
progress. Implemented the seven pending AcceptedLocalSearchForagerTest methods
from pinned Community tests; no upstream production implementation was inspected.
Provenance and notices record all score/acceptance/identity/quit assertions and
native substitutions. Canonical structural flaw uses -1 structural state, empty
finalists represent the asserted zero selected-move count, and supplied index 1
replaces TestRandom(1,1). No Java lifecycle/metric/PRNG trace claim.

New forager.rs exports ForagerConfig<S>, MoveForager<C,S>, TieBreaking and
InvalidFinalistIndex. Saved payloads need not be Clone. Accepted candidates rank
before rejected fallback, then maximize scores. Rejected structural flaws cannot
enter fallback. First ties retain one payload; seeded random ties retain all equal
finalists and pick in encounter order without replay. Capture is lazy and failed
capture leaves selection/counts/quit unchanged. Invalid supplied finalist indices
reject; empty, singleton and first pools do not request a draw.

LocalSearchConfig adds accepted_candidate_limit: Option<NonZeroU64> and
tie_breaking (First default). MoveSelection adds FirstBestScoreImproving and
FirstLastStepScoreImproving, both strict and accepted-only. Per-step accepted count
includes inferior accepted candidates and ties, and is separate from phase
max_accepted_moves. Random tie seeds are seed + completed_steps (wrapping),
independent of move ordering/acceptance, with a fresh count per local phase.

All six scoring backends use MoveForager for saved candidates. Internal select_best
accepts Into<ForagerConfig<S>>; bool conversion preserves construction and its
custom early-pick callbacks. Local search passes current/best references and step
number through one helper. Solution-observation wrappers forward this configuration.
Stream saved structs no longer duplicate the ranking score. Random ties can retain
many assignment/cache/graph snapshots; first mode stays one candidate. Reference
and ownership boundaries are in docs/incremental-search-contract.md.

Seven adapted tests and six original forager regressions passed. Originals cover
256 generated histories x32 configurations and every finalist, lazy/error/panic
capture, seed repetition/coverage, bounds, exact early/budget termination, and
240 configs across all six backends. One new owned-score test compares 576 paired
configurations across six backends. Six existing unit failure tests now save two
tied candidates before a later scoring/acceptance failure, then verify restoration.
Full gate 60369 passed 1,727 Rust tests, 90 doctests and 203 Python tests; logs are
work/forager-final-check.log. Focused release checks passed all 23 tests in
forager_cases, forager_regression and owned_scores; work/forager-release.log.
README no longer freezes the obsolete 723-method count; it points to the dashboard.

Next functional target: the four StrategicOscillationByLevelFinalistPodiumTest
methods remain pending. Read all four test bodies during the measurement wait.
They distinguish best vs last-step reference, two/three levels and unconditional
priority for overall improving moves. A plausible reference derived from fixtures
is: overall improvements first; otherwise prefer the earliest score level that
strictly improves its reference, then ordinary lexicographic score. This is only
an inference; verify unresolved cases against public documentation/Community oracle
before generalizing. Preserve every multi-step reference update and score fixture.
No production implementation was read. Full goal remains active and incomplete.

Sequential release/refresh/audit session 82047 finished successfully. All 13
measurement datasets and source/verification/live report fingerprints are current.
Coverage is 857/4,855 (17.65% UI), 3,716 pending, 282 excluded, zero failed or
unverified. Search is 52/158 (32.91%); moves remain 124/874. There are 53 timing
cards, 18 heuristic cards, 288 valid/equal-score small heuristic cases and 238
optimal per runtime. Existing list/gather and recorded differential limitations
remain unchanged. Logs: work/forager-refresh-*.log; audit: work/forager-audit.py.
These refreshed benchmarks keep their original selection settings; new early/tie
policies have native/fixture evidence, not new Java PRNG or timing equivalence.

Browser checks at 1440/390 confirmed current percentages, seven passed forager
methods, 53 cards, no stale indicator, no errors and no horizontal overflow.
Search now has button ref e36 in the initial snapshot because its increased count
changes the displayed order; always obtain fresh refs. Browser closed; server8765
remains alive. No test/measurement process from this increment remains active.
Final staged review precedes commit; the full goal remains active/incomplete.

## 2026-10-04: Strategic oscillation ranking

Continued the active full goal from clean 18fac3e. Added FinalistRanking with
HighestScore default, StrategicOscillationLastStep and StrategicOscillationBest.
All six native local-search paths pass immutable current/best references through
the shared forager. Accepted priority and structural score remain authoritative;
overall improvement wins, otherwise earliest ordinary numeric improvement, then
full-score order. Rejected BestScore fallback retains ordinary order. Explicit
early-pick matches override ranking; accepted-count limits keep prefix ranking.
Default FirstAccepted remains immediate; select BestAccepted to compare finalists.
No upstream production implementation was read or translated.

All four StrategicOscillationByLevelFinalistPodiumTest methods are adapted in
oscillation_cases.rs, with provenance/notices preserving complete score/order,
singleton identity and reference-update fixtures. Original regression tests cover
256 histories x3 rankings x16 rotations, rejected fallback, exact-reference order,
early-pick/count-limit interaction, decimal/full-width numeric comparisons and
failed/panicking saved captures. One original test records actual scoring callbacks
across all six backends, proving chosen worsening steps via the excluded candidate
in each subsequent neighborhood. Owned-score forager matrix now has 1,728 paired
owned/Copy configurations (all six models/backends), with no extra method credit.

Original public OscillationOracle uses documented STEP_SCORE and MOVE_COUNT_PER_STEP
Micrometer gauges, plus returned best identity/score. It stays within public
Community configuration/domain APIs and resets its isolated global registry between
solves. Bounded scores [-8,8], 2..16 choices, 1..4 steps, huge annealing temperatures
and first ties ensure complete accepted neighborhoods; gauges check those counts
without narrowing before validation. Native oscillation_oracle exercises forager
selection. Python compares 402 matrices x3 modes x4 prefixes: 4,824 native reference
passes, 4,499 Community agreements, 325 differing cases, no failures. All differences
and raw inputs/outputs remain in report/work artifacts. These are not timing or
additional source-method credits; no general annealing/PRNG/internal-trace claim.

Observed exact-reference boundary: reference (6,0,0), equal candidate and
(2,8,-2). Community selects whichever candidate occurs last in the two permutations;
native consistently selects the medium improvement, following the documented
preference for softer improvement over none. Deliberate total-order difference is
in compatibility/search docs, API docs and dashboard methodology. An ignored
work/oscillation/boundary_explanation.py sequential public-output hypothesis,
changing only handling of exact-reference candidates, reproduces all 4,824 Java
observations. It is an inference from outputs, not observation of internals.

Full final gate 97680 passed 1,736 Rust tests, 90 doctests and 208 Python tests;
work/oscillation-final-check.log. Prior full gate also passed, then final review
added explicit rejected-fallback coverage, clarified module prose and made Java
count validation exact. Measurement/release refresh session: 3483;
work/oscillation-release.log and work/oscillation-refresh-*.log. Sources frozen
before this capture. Dashboard update adds only data/caption text, no Impeccable.

Next useful increment: add both strategic modes with entity tabu to the existing
heuristic benchmark and small-case quality comparison. Native selector already
supports them; Java public LocalSearchForagerConfig.withFinalistPodiumType supports
both variants. Existing benchmark stays at nine algorithms/eighteen timing cards
in this increment. Preserve equal-score/quality distinctions because the recorded
exact-reference behavior can cause different traces or results. Then continue
pending search/solver functionality; do not equate these increments with the full
rewrite. Full goal remains active and incomplete. No subagents were used.

Final release/refresh/audit session 3483 completed successfully. All 32 focused
release tests passed. All 13 dataset statuses and report/source/verification/live
fingerprints are current. Coverage: 861/4,855 (17.73% UI), 3,712 pending, 282 excluded,
zero failed/unverified; Search 56/158 (35.44%). There are 53 timing cards, including
18 heuristic cards. Existing heuristic validation remains 288 valid/equal-score
cases and 238 optimal per runtime. The oscillation comparison again reports
4,499/4,824 agreement, 325 differences, 4,824 native reference passes and no failures.
The 256 gather comparisons and documented list metadata differences are unchanged.
Audit: work/oscillation-audit.py; original raw cases: work/oracle/oscillation-cases.json.

Current stream medians Rust/Java ms: hill32 45.333/14.598; hill96 170.440/26.708;
tabu32 49.520/16.177; tabu96 180.213/37.287. Scores agree: 0/0 at32, -44/-484 at96
(both infeasible at96 under the budget). Shared-host timings do not establish a
causal before/after speedup. These workloads retain their prior highest-score
configuration; new strategic policies have no dedicated timing claim yet.

Browser verified 1440/390 widths: 17.73%, four passed oscillation methods, all 53
benchmark cards/18 heuristic cards, visible 4,499/4,824 agreement and 325-difference
explanation, no stale warning, no errors or horizontal overflow. Desktop/phone
screenshots reviewed in work/oscillation-{desktop,phone}.png. Browser closed;
server8765 stays alive. Staged code/test/provenance/adapter diffs reviewed before
commit. No test or measurement process remains active. Full goal stays active.

## 2026-10-04: Strategic oscillation performance comparison

Previous goal turn made verified progress (b37ed5a). Continued from that clean
commit; no subagents. Added oscillation_last/oscillation_best to both original
heuristic benchmark adapters: native FinalistRanking and public Community
FinalistPodiumType, entity tabu tenure two, global-best aspiration, original-order
changes, first ties, bounded steps. Existing highest-score configurations retain
explicit HighestScore/HIGHEST_SCORE. No upstream production source inspected.

The benchmark now has eleven configurations, 22 timed workloads at 32/96 variables
and 352 independently validated small cases. New config metadata records each
strategic reference and separates native highest-score rejected fallback from
Java public-forager behavior; no identical fallback/selected-trace claim. Report
also groups validity, equal-score counts and enumerated optima by algorithm.
A new Python regression proves valid differing scores are not equality/optimality
passes, and equal scores need not imply equal assignment identity. No additional
adapted-method or solver-feature credit. Dashboard configuration label now derives
from measured data; the prior static "Seven" label was already stale for nine.
Docs/dashboard.md now describes the current eleven configurations and methodology.

Unwarmed one-sample probe session 54105 completed, only as adapter/correctness
validation. Do not use its timing values as warmed performance claims. It returned
352 valid/equal-score cases and 300 optimal per runtime; each new strategic mode
was valid/equal in 32/32 and optimal in 31/32. Timed workload scores agreed, 0/0 at 32
and -44/-22 at 96. The final default refresh overwrites this probe's artifact with
three fresh processes x seven measured solves after five warmups, 50 steps.

Full gate 75697 passed 1,736 Rust tests, 90 doctests and 209 Python tests;
work/strategic-timing-check.log. Sequential refresh/audit session 53364 runs all 13
datasets; work/strategic-timing-refresh-*.log and work/strategic-timing-audit.py.
Coverage remains 861/4,855 (17.73%), pending 3,712/excluded 282; expected workload
count 57 including 22 heuristic cards. Sources froze before the final full capture.

Next functional target considered: all seven LateAcceptanceScoreBufferTest methods
were read in full, exclusively from the pinned test tree. They specify initial
slots/current, write-and-advance, wrap, reset retaining cursor, post-reset writes,
repeated equal reset without writes, and equal reset after a write. Native
LateAcceptance in acceptance.rs currently owns Vec<S>, eagerly clones all initial
slots and fills every slot on reset. An independently designed lazy score history
could improve large/owned-score history resets and support these fixtures, but
must integrate with actual acceptance/search rather than be an unused wrapper.
Preserve the existing public history observation contract or handle any native API
change deliberately; add clone/panic/large-history checks and independent eager
reference histories. No production Timefold buffer implementation was inspected.
Full goal remains active and incomplete.

Final refresh/audit session 53364 completed successfully. All 13 datasets and
source/verification/live fingerprints are current. There are 57 benchmark cards,
22 heuristic cards and eleven configurations. Final validation remains 352 valid
and equal-score cases, 300 independently optimal per runtime; both new modes are
32/32 valid/equal and 31/32 optimal. Source-method coverage is unchanged at 861/4,855,
with no failed/unverified methods. Existing 4,499/4,824 oscillation agreements and
325 recorded differences remain separate and unchanged.

New strategic medians Rust/Java in milliseconds: last-step 32 2.611/14.723,
last-step 96 16.721/16.072, best-score 32 2.913/11.771, best-score 96 16.265/16.680.
Each has 21 measured samples/runtime after warmups across three processes.
Scores/calls agree: 0/0 and 4,801 at 32; -44/-22 and 14,401 at 96. Both 96 results are
infeasible. Shared-host timing ranges remain in the report; near 1x differences
are not established speed advantages, and no isolated before/after claim follows.

Browser checked at 1440/390: dynamic "11 heuristic configurations", 57/22 card
counts, all four new titles and current numbers, quality/infeasibility labels,
no stale warning, no errors, no horizontal overflow. Screenshots reviewed in
work/strategic-timing-{desktop,phone}.png. Browser closed; server 8765 remains alive.
Staged source/adapter/UI/documentation diffs reviewed. No test/measurement process
remains active. Full rewrite goal stays active; the next functional opportunity
is the independently designed late-acceptance history described above.

## 2026-10-04: Lazy late-acceptance history

Independent native `ScoreHistory<S>` now backs LateAcceptance. Nonempty circular
slots track writes since a shared baseline; reset preserves cursor. `get`, `current`
and slot-order `iter` borrow without cloning. `as_slice` uses OnceLock to lazily
materialize a full contiguous cache, retaining the existing borrowed `history()`
API. No new production upstream source inspected. Extra slot metadata and stale
owned values are the explicit memory tradeoff; snapshot invalidation can drop a
linear number of cached scores. u64 generation rollover clears old stamps.

LateAcceptance prepares its new best/baseline clones before mutation. Constructor
uses one score clone and a hard/medium improvement at most two, independent of
history length. A failed score clone leaves prior history, cursor and best intact.
Seven fully attributed LateAcceptanceScoreBufferTest methods retain every score,
operation and observation. Native reset return values/internal generation choices
are outside compatibility claims. Nine original integration tests plus one private
rollover unit test check generated eager histories, 100,000-slot clone work,
concurrent cache initialization, copy independence and clone/equality failures.
Existing generated hard/medium histories and six-backend search checks remain.

Focused seven fixtures/eight initial regressions passed session 88823, followed by
one additional equality-failure regression in the full gate. Initial gates caught
unused Ordering and manual-is-multiple-of lints, fixed before final capture. Full
check session 54235 passed; log work/history-check.log. Verified counts:
1,753 Rust tests, 91 doctests, 209 Python tests; 868/4,855 adapted methods
(17.88%), Search 63/158 (39.87%), pending 3,705/excluded 282; no failed/unverified
methods. No isolated wall-clock reset speedup claimed.

All nine DefaultLevelScoreStateTest bodies were read as tests only during this
increment. Six score-level methods were already adapted; three remaining cases
concern Java best-score-index lifecycle. Avoid claiming them from native comparison
alone. No next implementation target selected yet; full rewrite goal remains active.

Focused release tests passed before sequential measurement refresh in session 65111;
work/history-release.log. All 13 datasets refresh through work/history-refresh-*.log;
final report audit is work/history-audit.py (output work/history-audit.log).

Refresh/audit session 65111 finished with exit 0. All 13 datasets are current;
source, verification and live-server fingerprints agree. Coverage is 868/4,855
(17.88%), Search 63/158 (39.87%), with zero failed/unverified methods. All 57 timing
workloads remain, including 22 heuristic workloads/11 configurations. Heuristic
checks remain 352 valid/equal-score cases and 300 optimal per runtime. Recorded
oscillation boundary differences remain 325; no hidden compatibility credit.

Late-acceptance + tabu current medians Rust/Java: 32 variables 2.240/9.609 ms,
96 variables 15.573/16.495 ms; 21 samples/runtime, three processes, five warmups,
50 steps. Scores agree (0/0 and -44/-22 respectively); 96-variable result remains
infeasible. Shared-host measurements do not isolate this reset change's effect.

Browser checks at 1440/390 confirmed 17.88%, Search 39.87%, seven passing buffer
fixtures, 57/22 cards, current measurements and no errors/horizontal overflow.
Screenshots work/history-{desktop,phone}.png were visually inspected. Browser is
closed, server 8765 remains running; no test/measurement process remains active.
All staged changes inspected before commit. This turn made verified progress;
full goal remains active, not complete or blocked.

## 2026-10-04: Basic branch-and-bound search

Prior goal turn made verified progress (83e8aa7). This increment adds a substantive
exact-search capability: independent native branch-and-bound over FiniteProblem.
The callback gets partial assignment plus unresolved movable indices (a decided
optional None must not be confused with unresolved). BranchEvaluation holds its
score and optional admissible upper bound. Unknown bounds add no pruning; the
minimum full-score bound along a path carries forward and detects violations at
visited eligible leaves. Pruned branches cannot validate caller admissibility.

Four NodeExploration policies and public SearchNodeKey scheduling comparison follow
pinned comparator fixtures and public exhaustive-search.adoc, not production code.
Scheduling ignores structural incompleteness; eligible leaf retention and pruning
use full score order. Negative structural leaves are rejected. Queued partial
parents expand in priority order, generating children in domain order. Pins fixed,
movable initial values cleared, optional None last, ties first eligible evaluated
leaf. Native insertion serial gives deterministic exact-key ties. Full owned
assignments live in a BinaryHeap; no incremental or linear-memory DFS claim.

Evaluation/cancellation/time limits, frontier-size stopping, strict-best observer,
node/leaf/open-parent/pruned-branch counters and conditional is_optimal are exposed.
Remaining proven-dominated branches can be pruned despite exhausted budgets; empty
space/final completion retain exhaustion precedence. FrontierLimit cannot imply
optimality. Panics propagate with caller model unchanged. List/mixed branch search,
orchestration integration and automatic score-trend bounds remain unimplemented.

Four complete comparator methods plus shared AbstractNodeComparatorTest fixture
helpers are attributed. Thirteen original integration tests and two controlled-clock
unit tests cover independent enumeration, generated valid bounds, every evaluation
prefix, owned/decimal scores, structural states, cancellation, limits, failures,
optional None/pins/ranges and actual ConstraintNetwork scoring. Focused integration
and unit runs passed; avoid the first filtered integration output (0 tests) as
coverage evidence. Work/branch-focused-integration.log contains all 13+4 passes.
Clippy passed. Full gate session 93549 writes work/branch-check.log.

Public Java SolverOracle.branchFactory uses BRANCH_AND_BOUND, four exploration
types, no entity/value sorting and ONLY_DOWN score trend. Javap signatures only
verified public configuration methods. Native/Java benchmark adapters accept an
optional order mode; default brute force preserved. Benchmark_solver now measures
10 workloads: original brute sizes6/8 plus four branch orders at each size. Branch
fixtures negate absolute soft preferences so partial hard/soft score is admissible;
Python tests prove it against all complete states on small prefix trees. Same
inputs across branch orders; full scoring, per-run setup and result validation.
No identical Java/native bounds, trace or node-count claim. Existing visual cards
consume all workloads; exact section footer and docs now describe both algorithms.

Unwarmed one-sample probe 15914 passed all10 independently enumerated optima;
work/branch-benchmark-probe.log. Its timings are NOT warmed performance evidence.
Final refresh must replace probe artifact. Expected final counts: 1,772 Rust tests,
92 doctests, 211 Python tests; 872/4,855=17.96% coverage, Search67/158=42.41%,
pending3,701/excluded282; 13 current datasets,65 timing cards including10 exact and
22 heuristic. Work/branch-audit.py checks fingerprints, counters and warmed samples.
Full goal remains active, incomplete, and not blocked.

Full gate93549 completed successfully: 1,772 Rust tests,92 doctests,211 Python
checks; coverage872/4,855 (17.96%), Search67/158 (42.41%), zero failed/unverified.
Focused release checks also passed before sequential refresh in session24565;
work/branch-release.log. Refresh logs: work/branch-refresh-*.log; final audit output
work/branch-audit.log. No source changes after the frozen full capture.

The three InitializingScoreTrendTest bodies were read as tests only while waiting.
Native automatic bounds are not implemented. A future trend/bound integration must
handle exact numeric levels and unknown/infinite bounds honestly, including decimal
families; do not infer arbitrary finite bounds from integer maxima. Priority remains
integrating bounded search into the broader native solver, not earning isolated
parser-fixture credit. README now exposes the standalone capability and its contract.

Refresh24565 terminated exit1 in the existing Java oscillation adapter: "missing
or inexact step score". Do not treat that refresh as successful. A controlled GC
probe in work/oscillation-gc/{baseline,retained} reproduced the unfenced adapter
failure (zero output rows) and passed24 with a strong solver reference plus Java
Reference.reachabilityFence. No upstream production code was inspected. Official
Micrometer gauge docs confirm weak observation references; see compatibility.md.

Permanent OscillationOracle now holds/fences solver through metric reads, includes
metric value in failures and accepts a diagnostic --gc-check. run_checks forces GC
on24 cases in a fresh64MiB process and compares complete raw observations with the
normal Java run. GC evidence is metric_gc_checks in the report and raw
metric_gc_outputs in work/oracle/oscillation-cases.json. No compatibility credit.
Recheck46523 passed4499 agreements/325 differences/0 failures and24/24 GC checks;
work/branch-gc-recheck.log. Final capture must use the new adapter/script sources:
full gate45079 writes work/branch-final-check.log. Prior fingerprints are stale.

Final gate45079 passed on the GC-fixed sources: 1,772 Rust tests,92 doctests,
211 Python tests. Sequential final refresh is session88284, logs
work/branch-final-refresh-*.log, followed by work/branch-final-audit.log.
Re-poll that exact handle if interrupted; do not restart on observation timeout.
Old refresh24565 is terminal failure; final refresh88284 replaces every dataset so
all measurements share the new global source fingerprint. Rust release evidence
remains work/branch-release.log (no Rust changes after that check).

Final solver differential in refresh88284 passed128/128 cases,4499/4824 oscillation
agreements with325 unchanged differences,0 failures, and24/24 forced-GC metric
checks. The warm exact-search benchmark has completed all10 workloads; final
heuristic/pillar measurements and combined audit still follow in the same session.

Final refresh88284 finished exit0 and work/branch-final-audit.log passed: all13
datasets current with matching source/verification fingerprints,65 timing cards,
10 exact workloads and22 heuristic workloads. Coverage872/4,855=17.96%; Search
67/158=42.41%; pending3,701,excluded282,failed/unverified0. The final full gate
45079 passed1,772 Rust tests,92 doctests,211 Python checks. Forced-GC checks24/24
passed and the325 existing oscillation differences remain recorded.

Warmed median milliseconds per solve (Rust/Java), sizes6 then8: brute force
0.021313/7.626263 and0.238770/8.470987; depth first0.012708/4.217277 and
0.060763/6.738224; breadth first0.062455/5.932555 and1.372616/8.875124;
score first0.019462/3.596690 and0.115350/6.569077; optimistic bound first
0.014944/4.127800 and0.075903/7.188131. Independently enumerated optima match
every sample. These are small full-score framework workloads, not general speed
claims; bounds and execution traces need not match across runtimes.

Browser verification passed at1440x1000 and390x844:65 cards,10 exact,22 heuristic;
Search filtering NodeComparatorTest returns all4 passed methods. No page errors,
horizontal overflow or stale-data warning. Visually inspected screenshots:
work/branch-dashboard-desktop.png, work/branch-dashboard-phone.png and
work/branch-dashboard-phone-bound.png. Browser closed; localhost:8765 dashboard
server remains running. No measurement/test process remains live. Broader branch
integration (list/mixed/orchestration and automatic bounds) is still pending.
Full goal remains active and incomplete.

## 2026-10-04: List and mixed branch-and-bound

Previous goal turn made verified progress in commit 18f6428. This increment extends
bounded exact search across basic, list and mixed models, preparing the shared tree
and control machinery needed for ordered solver-phase integration. That integration
remains pending; do not imply these standalone APIs are already SolverPhase choices.

The original basic engine now accepts a private BranchTree and Retained result type.
BoundedSearchResult<R> contains the shared counters; BranchAndBoundResult<V,S> remains
an alias with the same fields and ScoredAssignment best. ListBranchAndBoundResult and
MixedBranchAndBoundResult use the existing corresponding scored assignment types.
No extra best/observer cloning is introduced by the common engine. Existing 13 basic
integration tests and all four adapted comparator methods pass after the refactor.

BasicTree preserves original model/domain order. ListTree starts with frozen lists
and pinned prefixes, clears movable initial values, then branches remaining registry
values in registry order over allowed entity/index insertions and optional omission.
Callback remaining indices are registry positions, not identities. Later insertions
can shift positions/neighbors: bounds must cover all completions. MixedTree first
branches lists then basics, retains both value types, and passes separate remaining
slices. Model completeness alone does not identify optional-model leaves. Impossible
components score nothing. Queue nodes own whole states; per-parent insertion choices
also allocate. No incremental scoring or linear-memory claim.

Ten original integration tests cover independent subset/permutation/partition sets
for 128 range/optionality configurations across four orders; fixed lists/prefixes,
initial resets, non-dense IDs; tight/loose/unknown table bounds; coupled mixed products;
every evaluation prefix with exact observer/best history; empty/impossible/frozen
spaces; limits/cancellation; inherited-bound violations and structural leaf eligibility;
full-width decimals, score-clone failures, scoring/observer panics and reusable models.
No upstream-method adaptations or provenance changes are claimed. A first overly
strong test assertion demanded pruning even from deliberately loose bounds; fixed
the test to require pruning only for tight depth-first cases. No algorithm workaround.

The mixed_routes example adds all four bounded policies using actual mixed network
scoring. Its optimistic distance bound chooses the best remaining depot for each
current metric tour; inserting customers cannot reduce distance or overload. Two
original example tests check every bound against every compatible complete state
and compare stream scoring at every visited partial node. The known 0hard/-4soft
optimum is attained by every order: depth 25 nodes/4 leaves/12 pruned branches;
breadth 82/1/48; score 37/2/24; optimistic bound 21/2/12. Exhaustive baseline visits
96 leaves. These are deterministic counts, not performance measurements.

Focused debug tests pass (10 joint tests, 2 example tests); example execution and
bound/backend tests are in work/joint-branch-{tests,integration,routes}.log. Clippy
passed before the full frozen-source gate. Full gate is session64664, output
work/joint-branch-check.log. Re-poll that live handle if interrupted; do not restart
on timeout. Expected counts are 1,784 Rust tests, 93 doctests, 211 Python tests;
coverage stays 872/4,855 (17.96%) because these are original native tests. All 13
measurement datasets must be refreshed after the full capture; old data is stale.
The full goal remains active and incomplete, with concrete progress this turn.

Full gate64664 completed exit0: 1,784 all-target Rust tests, 93 doctests and 211
Python tests. Focused release session1600 completed exit0 (13 basic, 10 joint,
4 comparator and 2 routing-example tests); work/joint-branch-release.log. Sources
are frozen for the sequential 13-dataset refresh; work/joint-branch-refresh.sh
writes per-job work/joint-branch-refresh-*.log and rebuilds the dashboard report.
work/joint-branch-audit.py checks the new test names/count and all existing metrics.
No implementation/provenance source changes after the full verification capture.

Next implementation priority is connecting these trees to SolverPhase::Exact across
all six model/scoring paths. Current exact orchestration uses solver_exact.rs with
shared SolveControl, fallible scoring, retained prior scores, goal/feasibility stops,
move/score budgets and observations. Standalone search_tree currently has infallible
scoring and only its own BranchAndBoundConfig controls; integrate those contracts
explicitly rather than wrapping errors into panics or dropping shared stop reasons.
Unknown bounds must remain a safe mode. A declared non-increasing-score bound would
need an explicit caller contract, including integer wrap behavior; do not infer
safe generic upper bounds from a partial score or arbitrary integer maxima.
Current refresh session79724 is live and must be re-polled if interrupted.

Refresh79724 completed exit0. Final work/joint-branch-audit.log passed: all 13
measurement datasets match the current source/verification fingerprints and live
freshness endpoint. There are 65 timing cards (10 exact, 22 heuristic), unchanged
872/4,855 adapted methods (17.96%), Search 67/158 (42.41%), pending 3,701,
excluded 282 and zero failed/unverified methods. Heuristic quality remains 352
valid/equal-score cases and 300 optimal per runtime. Existing diagnostic boundaries
remain recorded, including 325 oscillation differences and 24/24 forced-GC checks.

Browser verified at 1440x1000 and 390x844: correct 17.96% coverage, all 65 cards,
10 exact and 22 heuristic cards, and all four passed NodeComparatorTest results
when filtering Search. No page errors, stale-data warning or horizontal overflow.
Screenshots work/joint-branch-{desktop,phone}.png were visually inspected. Browser
closed; localhost:8765 server remains running. No test/measurement process remains
live. All staged implementation, example, tests and documentation diffs reviewed.
This turn made verified progress; the full goal remains active and incomplete.

## 2026-10-04: Bounded search in ordered solver phases

Previous goal turn made verified progress in b880153. This increment connects the
shared bounded tree engine to Exact phases in all six basic/list/mixed scoped/stream
entry points. ExactSearchStrategy defaults to Enumerate; BranchAndBound accepts
BoundedPhaseConfig with four orders, frontier cap and PartialScoreBound. Unknown
supplies no bound. NonIncreasing is an explicit caller assertion that each partial
score with structural state cleared dominates every eligible completion, including
optional omissions, later list insertions, coupling and actual wrapping arithmetic.
No inferred bounds or per-level InitializingScoreTrend implementation is claimed.

The engine now has a fallible internal BranchContext and retains eligible prior
phase scores for pruning/tie preservation. Scoring errors keep their typed phase
error; visited bound violations return phase index and diagnostic score/bound strings.
Every scored root/partial/leaf counts once toward both score and move budgets;
there are no local steps. Shared elapsed/stagnation, cancellation, goals and
observations retain their contracts. Final exhaustion/proven pruning wins over
controls reached by the final callback. FrontierLimit ends only the current phase.
Default enumeration and standalone APIs retain their existing behavior.

Ten original integration methods cover all six backends and four orders, budget
prefixes, prior incumbents, partial handoffs, limits, cancellation, errors, decimal
precision and final-exhaustion precedence. Five controlled-clock tests cover model
preparation, callbacks/observers, factory time, shared deadlines and sticky
stagnation across handoffs. A managed-job regression checks per-job strategy
copying/overrides. No copied tests or provenance changes; coverage stays 872/4,855.
The first observer test incorrectly expected an initialization event without a
following local phase; the fixture now includes that phase, preserving the existing
contract. An exhaustive fixture error match needed the new bound-error arm.

Focused debug checks, all-target clippy and managed-job session 4228 passed; logs
work/phase-branch-{matrix,time,global-time,managed,standalone,clippy}.log. The full
repository gate is session 98226, work/phase-branch-check.log; re-poll the live handle
rather than restarting on timeout. Expected counts are 1,800 Rust tests, 94 doctests
and 211 Python checks. Source is frozen pending final capture and measurements.
work/phase-branch-refresh.sh refreshes 13 datasets sequentially; the matching audit
checks current fingerprints, method coverage, new test names and 65 timing cards.
The full goal remains active and incomplete, with concrete progress this turn.

Full gate 98226 completed exit 0: 1,800 all-target Rust tests, 94 doctests, 211 Python
checks. Coverage 872/4,855=17.96%, Search 67/158, pending 3,701, excluded 282 and
failed/unverified 0. Focused optimized checks run sequentially in session 8104,
work/phase-branch-release.log. The complete upstream submodule remains clean at
8b815bc6b5f3eb81feedc94dea0885a93b267762. No sources changed after capture.

Optimized session 8104 completed exit 0: 85 solver integration tests, 13 basic branch,
10 joint branch, 4 comparator, 4 private bounded clock, 1 shared handoff clock and
1 managed-job test. The solver integration target compiled in 6m 32s; no timeout or
restart occurred. Work/phase-branch-release.log records all outcomes. Sequential
13-dataset refresh now runs work/phase-branch-refresh.sh with per-job logs; do not
run competing CPU work while timing. No unrelated heavy process was active at start.

Refresh handle is 67765; combined progress log work/phase-branch-refresh.log.
Solver differential passed 128/128; oscillation 4499/4824 agreements, 325 recorded
differences and 0 failures. Exact/heuristic/pillar timing and final audit remain
pending until that same refresh handle exits successfully.


Refresh 67765 completed exit 0. Final work/phase-branch-audit.log passed: all 13
measurement datasets and the live freshness endpoint match the current source and
verification fingerprints; 65 timing cards, including 10 exact and 22 heuristic
workloads. All new tests are present in the full capture. Exact samples match
independent optima; heuristic validation has 352 valid/equal-score cases and 300
optima per runtime. Oscillation differences remain 325, with 24/24 forced-GC metric
checks. Coverage remains 872/4,855 (17.96%), Search 67/158 (42.41%), pending 3,701,
excluded 282 and zero failed/unverified methods.

Browser checks passed at 1440x1000 and 390x844: correct coverage, 65 cards, ten exact
and 22 heuristic cards, four passed NodeComparatorTest methods when filtering Search,
no page errors, stale-data warning or horizontal overflow. Visually checked screenshots
work/phase-branch-{desktop,phone,phone-timing}.png. The final timing screenshot waited
for report loading before scrolling; initial immediate captures were superseded.
Browser closed; localhost:8765 server remains running. No test/measurement process
remains live. Staged implementation, tests and documentation reviewed.

This turn made verified progress. The full rewrite remains active and incomplete;
automatic per-level initializing-score-trend bounds and broader Community behavior
remain pending. Unknown bounds and the explicit caller assertion are the currently
supported ordered-phase choices; do not claim generic automatic bound inference.

## 2026-10-04: Declared score trends and symbolic bounds

Previous goal turn made verified progress in 3fd8c22. This increment adds declared
per-level initialization trends to ordered bounded phases, rather than only parser
fixtures. Read score_trend.rs and the new solver-contract section. Only pinned
Community tests/public documentation and public black-box APIs informed behavior;
no upstream production implementation was inspected or translated.

InitializingScoreTrend parses exact ANY/ONLY_UP/ONLY_DOWN tokens, repeating a single
token across the specified width. Three complete pinned predicate/parser methods
are adapted and attributed. Native bounds retain a finite numeric prefix followed
by an optional infinite boundary. Upper bounds keep OnlyDown levels; lower bounds
keep OnlyUp levels. First infinity makes suffixes irrelevant to partitions of
finite reachable scores. Structural state clears to zero; full bound comparisons
still consider structural first. Native exact decimal comparisons avoid float and
finite-extrema sentinels. Custom scores require numeric lexicographic ordering.

The common engine uses these bounds for scheduling, path minima, visited-leaf
checks and pruning. Existing finite standalone callbacks/errors retain their API;
SearchNodeKey now has an optional bound-type parameter defaulting to its score.
PartialScoreBound::Trend owns its declaration; bounded configs are Clone, not Copy.
Invalid widths return a typed phase error before scoring (stream factories can
already have run). Lower bounds never supply an incumbent without a scored witness.
Declarations remain caller obligations, including actual overflow/list/optional
semantics. This is declared-trend derivation, not automatic constraint analysis.

Five original bound regressions check parsing/widths/empty scores, every legal
small two-level completion, integer extremes, decimal exactness, total ordering
and independent three-level finite-score partitions. Five original solver methods
cover all six paths/four orders, pruning with unknown soft levels, every unknown
budget prefix, error typing, old finite-mode equivalence and decimal precision.
The first three-level reference incorrectly distinguished suffixes after equal
infinity; corrected it to compare finite-score partitions as documented. No
production algorithm workaround. One old fixture needed clone after the config
ceased to be Copy. Focused runs and all-target clippy passed.

Public Java SolverOracle optionally accepts branch order/trend, retaining existing
no-argument behavior and benchmark factory overload. Native solver_oracle invokes
real ordered scoped phases in this mode. score_trend_checks compares 64 original
homogeneous required graph cases x four orders x ANY/ONLY_DOWN-ANY declarations;
all 512 results passed independent Python optima, validity and rescoring in probe
44771, work/trend-java-probe.log. Signed soft preferences/pins are included; no
Java traversal/bound representation/list/mixed/timing claim. The standard solver
differential and existing dashboard diagnostic list now include this count.

Full gate is work/trend-check.log. Expected totals: 1,813 Rust tests, 95 doctests,
211 Python checks,875/4,855 adapted methods. Sources are frozen for final capture,
focused release and the sequential 13-dataset refresh. The full goal remains
active, incomplete and unblocked; this turn makes concrete progress.

Full gate 9717 completed exit 0: 1,813 Rust tests, 95 doctests, 211 Python tests.
Adapted methods 875/4,855=18.023%; Score calculation 3/119; Search remains 67/158;
pending 3,698 / excluded 282 / failed 0 / unverified 0. Focused release 33652 completed exit 0
including all 13 new methods, standalone 13+10, comparator 4 and controlled-clock 4;
work/trend-release.log. No source/provenance changes after full capture. The
sequential 13-dataset refresh runs work/trend-refresh.sh; re-poll its live handle
if interrupted. Work/trend-audit.py checks fingerprints, all timing/quality data,
three new adapted identities, native references and the 512-case Java diagnostic.

Refresh 85413 completed exit 0. Final work/trend-audit.log passed: 13 current
measurement datasets and live freshness match source/verification fingerprints;
65 timing cards (10 exact, 22 heuristic); all 512 trend comparisons and all 128
baseline planning cases match independent optima. Heuristic quality remains
352 valid/equal-score cases, 300 optima per runtime. Existing oscillation differences
remain 325, with 24/24 forced-GC checks. Final coverage 875/4,855=18.023%; Score
calculation 3/119=2.52%, Search 67/158=42.41%; pending 3698 / excluded 282 and zero
failed/unverified methods. These diagnostics earn no additional method credit.

Browser checks passed at 1440x1000 and 390x844: 18.02% overall, all 65 timing cards,
three passed InitializingScoreTrendTest results when filtering Score calculation,
and the 512/512 separate diagnostic with its scope explanation. No page errors,
stale-data warning or horizontal overflow. Visually inspected screenshots:
work/trend-{desktop,phone,phone-diagnostic}.png. Browser closed; localhost:8765
server remains running. No test/measurement process remains live. Upstream checkout
is unchanged/clean. Staged implementation, adapters, attribution and docs reviewed.

This goal turn made verified progress. Full Community behavior remains incomplete;
declared-trend bound derivation now exists, while automatic constraint analysis is
not implemented or claimed. Future work should advance remaining solver behavior
and its complete pinned tests, keeping native semantics and Java boundaries explicit.

## 2026-10-04: Phase-local diminished returns

Previous goal turn made verified progress in 5c2401b. This increment independently
implements sliding-window improvement-rate termination from pinned Community tests
and public documentation, without inspecting or translating production sources.
Read diminished_returns.rs and the new incremental-search-contract section.

DiminishedReturnsConfig uses a positive Duration and exact positive rational ratio;
LocalSearchConfig optionally enables it across six search paths. A native policy
retains changed best scores plus a cutoff predecessor, measures the softest level,
and resets grace on structural/hard/higher-priority improvements. Widened Decimal
and BigInt arithmetic preserve integer extremes and sub-float differences. Backward
times clamp; expiry latches; initial scoring and factory work are excluded. Later
ordered phases may continue. Existing budgets, time controls and cancellation retain
their ordering, and disabled rate checks add no clock reads. No Java timed-trace or
internal ring-buffer representation parity is claimed.

All eight DiminishedReturnsTerminationTest methods retain complete assertions with
individual provenance and notices. Five original regressions include 512 sparse
traces / 51,200 observations against untrimmed integer reference histories, decimal
and Duration extremes, retention, resets and invalid configuration. Four native
clock tests exercise all six search paths, plus one stream solver handoff test
compares zero and 50-second preparation. Focused debug tests passed in 88488 and
31127. A first test-only unsigned-negation compile error was corrected before the
successful reference run; no implementation workaround.

Full gate is work/diminished-check.log (handle 37490 while running). Expected totals
are 1,831 Rust tests, 96 doctests and 211 Python checks; do not treat estimates as
verified until exit 0. Focused release and sequential 13-dataset refresh follow;
work/diminished-refresh.sh and work/diminished-audit.py are prepared. Source and
provenance are frozen for capture. Full rewrite remains active and incomplete.

Full gate 37490 completed exit 0: 1,831 Rust tests, 96 doctests and 211 Python tests.
Coverage 883/4,855=18.187%; Solver lifecycle 59/379=15.567%; pending 3,690, excluded
282, failed/unverified zero. Release checks and then the sequential refresh run
under handle 7678 with set -e; logs work/diminished-release.log and
work/diminished-refresh.log. Re-poll that same handle if interrupted. No Rust,
Python or provenance edits after capture. The existing 65 timing workloads will
be refreshed; this feature adds no dedicated diminished-returns timing claim.

Release and refresh handle 7678 completed exit 0. All 18 new focused tests passed
in release. Final work/diminished-audit.log confirms matching source/verification
fingerprints and live freshness, all 13 current datasets, 65 timing cards (10 exact,
22 heuristic), 512 trend optimum agreements and 128 baseline solver agreements.
Heuristic validation retains 352 valid/equal-score cases and 300 optima per runtime.
Existing oscillation differences remain 325 with 24/24 forced-GC metric checks.
No new diminished-returns Java timed-trace or dedicated timing claim.

Browser verification passed at 1440x1000 and 390x844: 18.19% overall, 59/379 solver
lifecycle methods, eight passed DiminishedReturnsTerminationTest rows and all 65
timing cards, with no visible stale warning, page errors or horizontal overflow.
Visually inspected work/diminished-{desktop,phone,phone-tests}.png. Browser closed;
localhost:8765 remains running. No test/measurement process remains live. Upstream
checkout remains clean and pinned. Implementation, tests, attribution and docs
reviewed for the milestone commit.

This goal turn made verified progress. The Community rewrite remains active,
incomplete and unblocked. Continue substantive solver behavior from remaining
pinned Community tests/public docs; do not infer coverage from internal storage
checks, generated diagnostics or benchmark success.

## 2026-10-04: Pooled cheapest-insertion construction

Previous goal turn made verified progress in b803fda. This increment adds substantive
pooled construction across all six native model/scoring paths, independently from
pinned Community public documentation. No upstream production code was inspected.
Unimproved-score-threshold and composite termination tests were inspected as other
possible gaps, but no implementation or coverage claim was made for those features.

ConstructionPlacement::Pooled competes all eligible missing variables/list values
in each scan, including both components of mixed models. Existing assignments/pins
remain fixed; deterministic order handles ties and optional early-picking. Optional
non-worsening candidates are reconsidered after another item is placed; an entirely
rejected pool terminates. Saved candidate scores/caches avoid replay. Configuration
is still Copy and defaults to Sequential. See the incremental-search contract.

Seven original integration tests cover every generated basic budget prefix, coupled
quality/order changes, optional reconsideration, independent list insertion with
prefix pins/ranges, cross-component competition/ties and exact owned decimals.
Three existing backend matrices now run both strategies. Controlled clock checks
also run both, plus two new all-six-path cancellation/candidate-budget checks.
Public Java SolverOracle and native solver_oracle add an internal pooled mode.
pooled_construction_checks.py validates 66 required homogeneous graph cases; all
Java/native assignments and scores agree with Python greedy reconstruction, 48
are exhaustive optima per runtime. Four Python reference/protocol tests pass.
This diagnostic appears separately in the dashboard. No adapted-method credit: the
PooledEntityPlacerTest lifecycle/copy assertions remain pending. No new timing claim.

Initial all-target check required adding the new default field to explicit existing
config literals. Two syntax/type issues in original test drafting were corrected
before passing focused run 78946. Clock run 7044 and Java probe 34750 completed
exit 0. First full gate 15837 passed 1,839 Rust tests, 97 doctests, 215 Python tests.
Review then found empty optional pools could report StepLimit at zero steps. New
regression failed before the fix (95351, work/pooled-empty-before.log), then passed
all six paths after filtering impossible placements (39203, work/pooled-empty-after.log).

Sources are now frozen for final gate, focused release and sequential 13-dataset
refresh. Final expected count is 1,840 Rust tests, 97 doctests, 215 Python tests;
coverage remains 883/4,855=18.187%. Work/pooled-audit.py checks all fingerprints,
existing diagnostics, 65 timing records and the 66 new pooled comparisons. The
full Community goal remains active, incomplete and unblocked.

Final sequence handle is 61476 (set -e): scripts/check.sh into
work/pooled-final-check.log, focused release into work/pooled-release.log, then
work/pooled-refresh.sh into work/pooled-refresh.log. Re-poll this same handle after
an observation timeout. Do not restart from an incomplete log or infer termination.
No Rust/Python/provenance changes after the final capture began.

Final full gate completed successfully: 1,840 Rust tests, 97 doctests and 215 Python
tests, with formatting/clippy clean. Coverage remains 883/4,855=18.187%, pending
3,690 / excluded 282 / failed 0 / unverified 0. The same live handle continues
focused release and the sequential refresh; no competing own CPU work is running.

Final sequence 61476 completed exit 0. Focused release passed all seven pooled
integration tests and all nine controlled-clock construction tests. Final audit
work/pooled-audit.log passed: all 13 measurement datasets and live freshness match
current source/verification fingerprints; 65 timing cards (10 exact, 22 heuristic).
All 66 new pooled cases agree in final assignments/scores with native Python
reconstruction, 48 optima per runtime. Existing diagnostics remain unchanged,
including 512 trend optima, 352 valid/equal-score heuristic cases (300 optima),
325 oscillation differences and 24/24 forced-GC metric checks. No source edits
were made after final capture. Upstream remains clean and pinned.

Browser verification passed at 1440x1000 and 390x844: unchanged 18.19% coverage,
all 65 timing cards and the new 66/66 pooled diagnostic with 48/66 optimum count
and scope explanation. No page errors, stale warnings or horizontal overflow.
Visually inspected work/pooled-{desktop,phone,phone-diagnostic}.png. Browser closed;
localhost:8765 server remains running. No test/measurement process remains live.
The staged implementation, adapters, original tests and docs were reviewed.

This turn made verified progress toward the full rewrite. The goal remains active,
incomplete and unblocked. Pooled construction is now available; Java lifecycle/copy
method adaptations, broader construction strategies and remaining Community
behavior remain pending. Do not turn generated-case agreement into method credit.

## 2026-10-04: Minimum significant improvement within a time window

Previous goal turn made verified progress in 0c3b55e. This increment adds an
independent exact score-difference stagnation policy to all six local-search paths.
Only pinned Community tests/public documentation informed it; no upstream production
implementation was read. Read significant_improvement.rs and the new search contract.

SignificantImprovementConfig validates zero structural state, ordinary score width
and exact level arithmetic. Optional ignored suffixes use true prefix comparison,
not a finite wildcard sentinel. SignificantImprovement owns a score/time anchor,
accumulates smaller gains until a qualifying best is observed, clamps backward time
and latches expiry strictly after the deadline. A gain at the exact deadline can
reset it; a late gain remains retained by search without reviving the policy.
Initialization/factory work is excluded and later ordered phases may continue.
The option is independent of ordinary max_unimproved_time; both can be enabled.
Public progress is approximate and can be one before exact expiry.

LocalSearchConfig::significant_improvement borrows immutable settings; a running
phase clones them into independent history. The first full gate 40787 ended at
Clippy large_enum_variant because inline settings enlarged all solver phase enums.
Borrowing settings avoids a broad phase-API boxing change and preserves lightweight
configuration copies. Standalone policy still owns its configuration. The new
config/reason is integrated in the shared LocalTimeBudget without changing disabled
clock behavior. No solve-wide score-difference policy or Java lifecycle parity.

Two source scoreImproves methods retain all timestamps, scores, repeated phase/solver
queries and exact gradient assertions, with provenance and notices. Three remaining
methods stay pending (negative milliseconds and construction/shared lifecycle).
Seven original regressions cover 640 histories / 64,000 events, exact integer and
decimal differences, prefix/wildcard boundaries, zero/max durations and validation.
Four injected-clock tests cover all six search paths; one phase/factory test checks
a 50-second preparation delay and subsequent exact completion. The first native
reference run 24002 found only incorrect bitwise assertions for floating estimates;
changed those original checks to 1e-15 tolerance, leaving stop decisions exact.
Focused run 50248 passed all then-current tests.

Final full gate runs as handle 36818, work/significant-check.log; formatting and
Clippy passed. Expected totals after all tests: 1,854 Rust tests, 98 doctests and
215 Python checks; 885/4,855 adapted methods=18.229%, Solver lifecycle 61/379.
These are estimates until the gate exits successfully. Prepared
work/significant-refresh.sh and work/significant-audit.py for the subsequent
sequential 13-dataset refresh. No new dedicated timed-search comparison is claimed.
Full goal remains active, incomplete and unblocked. Re-poll live handles on timeouts.

Full gate 36818 completed exit 0: 1,854 Rust tests, 98 doctests and 215 Python
checks. Coverage 885/4,855=18.229%; pending 3,688 / excluded 282 / failed 0 /
unverified 0. Sources and provenance are frozen for release and measurements.

Release and sequential refresh run under handle 81489 with set -e. All 14 new
focused tests passed release in work/significant-release.log. Refresh progress
is work/significant-refresh.log and per-job significant-refresh-*.log files.
Re-poll that same handle if interrupted; do not restart on an observation timeout.

Release/refresh handle 81489 completed exit 0. All 13 datasets and 65 timing
workloads were regenerated successfully. The audit in work/significant-audit.log
passed current source/verification fingerprint checks and live API freshness.
Baseline solver comparisons retain 128/128 optima, trend cases 512/512, and pooled
construction 66/66 assignment/score agreement with 48 optima each. Heuristic
comparisons retain 352 valid equal outcomes with 300 optima each. These workload
results do not claim universal speed superiority or timed-policy trace parity.

Browser verification passed at 1440x1000 and 390x844: overall coverage 18.23%,
Solver lifecycle 61/379 (16.09%), two passed and three pending methods in the
threshold class, and all 65 timing cards. No page errors, visible stale warnings
or horizontal overflow. Visually inspected work/significant-desktop.png,
work/significant-phone.png and work/significant-phone-tests.png. Browser closed;
localhost:8765 server remains running. No test/measurement process remains live.

This increment made verified progress: 1,854 Rust tests, 98 doctests and 215 Python
checks passed, along with all 14 focused release tests. Coverage is 885/4,855,
with 3,688 pending, 282 excluded and no failed or unverified adaptations. The full
goal remains active, incomplete and unblocked. Shared/construction lifecycle
semantics and the remaining Community behavior still need implementation.

## 2026-10-04: Cycling value-first construction

Previous goal turn made verified progress in 9276182. This increment independently
implements ConstructionPlacement::ValueQueue from pinned Community public
construction documentation (Allocate to value from queue). Read the two
QueuedValuePlacerTest methods; lifecycle/copy assertions remain pending. No upstream
production implementation was read; no test fixtures were adapted or copied.

Basic values are deduplicated by Eq in first model/domain encounter order. Each
queued value competes for eligible missing variables in model order; the winning
receiver uses existing scoring/forager/cache behavior. A successful assignment
permits another full rotation. Empty receiver sets skip without scored steps;
scored optional rejections count a step. A full rotation without an installed
assignment stops, and later successful placements allow optional retries. Existing
values/pins and heterogeneous domains remain authoritative. Each call restarts.

A shared private ValueConstructionQueue feeds both basic and mixed construction.
List construction retains sequential value insertion. Mixed component order remains
meaningful: finish each component in sequence, without later cross-component retries.
Default sequential and pooled modes retain their prior behavior. See the new
incremental-search contract section for precise scope and allocation costs.

Seven new original tests pass: target order/ties/resume; rejected rotations;
3,200 reference comparisons (32 heterogeneous/optional models x 25 budgets x four
basic/mixed scoring paths); empty queues; list/mixed order; coupled mixed objectives;
and exact owned-decimal target ranking/early picks. Extended the existing six-path
time/cancellation/candidate-cap matrices to include the queue; all nine timed
construction test methods passed. Initial focused run 56215 stopped at test-only
integer inference and a borrow in domain rotation, both corrected; final focused
handle 18611 exited 0. Production all-target check 29663 exited 0.

Public Java adapter now selects ALLOCATE_TO_VALUE_FROM_QUEUE through public config.
Shared construction result validation and process orchestration retain the old pooled
comparison. New original value_queue_construction_checks.py reports 67/67 equal final
assignments and scores, all native references correct, 24 optima per runtime and no
failures. Initial Java handle 19580 exited 0 (work/value-queue-java.log). Three new
Python reference checks pass. Dashboard gains a separate diagnostic with these counts
and limitations. No new dedicated timing or upstream method credit is claimed.

Full gate runs under handle 64333, work/value-queue-check.log. Formatting and Clippy
passed; expected totals are 1,861 Rust tests, 98 doctests and 218 Python tests until
verified at exit. Prepared work/value-queue-refresh.sh and work/value-queue-audit.py
for the next sequential 13-dataset refresh. Sources are frozen during capture and
measurements. Re-poll the same handle on observation timeouts. Full goal remains
active, incomplete and unblocked.

Full gate 64333 completed exit 0: 1,861 Rust tests, 98 doctests and 218 Python
checks, with formatting/Clippy clean. Coverage remains 885/4,855=18.229%, with
3,688 pending, 282 excluded and zero failed/unverified. Sources are frozen for
focused release verification and the sequential 13-dataset refresh.

Release and refresh run under handle 88894 with set -e; work/value-queue-release.log
and work/value-queue-refresh.log retain output. Re-poll the same handle after an
observation timeout; do not restart solely because output is delayed.

Release/refresh handle 88894 completed exit 0. Seven new regression methods and all
nine timed construction methods passed release. All 13 datasets were regenerated,
including 65 timing workloads. work/value-queue-audit.log passed source/verification
fingerprints, live API freshness, coverage and existing comparison invariants.
The fresh queue comparison again reports 67/67 assignments/scores/native references,
24 optima each and zero failures; pooled remains 66/66 with 48 optima each. Exact
baseline 128/128, trend 512/512 and heuristic 352 valid/equal with 300 optima each.
Existing divergence/error diagnostics remain explicitly represented in the dashboard.

Browser checks passed at 1440x1000 and 390x844: 18.23% adapted-method coverage,
65 timing cards, new 67/67 value-queue diagnostic and separate 24/67 optimum count,
with no page errors, stale-result warning or horizontal overflow. Visually inspected
work/value-queue-{desktop,desktop-diagnostic,phone,phone-diagnostic}.png. Browser
closed; localhost:8765 server remains running. No test/measurement process remains
live. Staged implementation, tests, adapters and docs were reviewed.

This turn made verified progress toward the full rewrite. The goal remains active,
incomplete and unblocked. Queue lifecycle/copy method adaptations, broader construction
ordering and remaining Community behavior remain pending. New generated cases and
original regressions earn no additional upstream-method credit.

## 2026-10-04: Dedicated construction performance and quality evidence

Previous goal turn made verified progress in 19789db. This increment extends the
requested performance dashboard to the three implemented basic construction modes:
Sequential, Pooled and ValueQueue. The solver library algorithms are unchanged.
New native construction_benchmark example and public-Java ConstructionBenchmark
measure complete per-solve construction with full scoring, result validation and
independent score-call counters. Parsing, network/factory construction, process startup
and output encoding remain outside timing; per-solve setup remains inside. Only
public pinned Community configuration is used, not production implementation source.

benchmark_construction.py has 67 original small cases per mode with independent
optima and greedy references (201 comparisons), plus shared planted-optimum timing
inputs at 32 and 96 variables. Native assignments must match independent reference
reconstruction; Java results are validated independently and quality/identity equality
is counted separately. Five new Python checks reject corrupt timing/result/pin/score
evidence, distinguish validity/equality/optimality, check reference order and prove
that planted scores attain zero while all contributions are nonpositive.

Pilot handle 94835 completed exit 0: 201 valid/equal-score/equal-assignment/native
reference cases; 122 optima each. Both engines reach zero on sequential/pooled timing
inputs, but value queue yields -3hard/-17soft at 32 variables and -16hard/-7soft at 96.
These are one-warmup/one-sample/one-fork pilot observations, not final timing evidence.

The report gains construction_performance as its 14th measured dataset. Dashboard
uses existing timing cards with quality, feasibility, known optimum and score-call
ranges. Six new cards bring the expected total to 71. Metadata, refresh behavior,
methodology and original-case diagnostics include construction. Original comparisons
confer no upstream method credit. UI edits do not change the report fingerprint.

Full gate 16813 completed exit 0 (work/construction-benchmark-check.log): 1,861 Rust
tests, 98 doctests, 223 Python tests, formatting/Clippy clean. Coverage remains
885/4,855=18.229%, pending 3,688/excluded 282/failed 0/unverified 0. Sources are frozen
for the 14-job sequential refresh in work/construction-benchmark-refresh.sh; its
log is work/construction-benchmark-refresh.log, with per-job logs. The final audit is
work/construction-benchmark-audit.py. Full rewrite remains active/incomplete/unblocked.

Sequential refresh runs under handle 26926. Re-poll this same handle after observation
timeouts; do not restart a still-live measurement. The planned full construction
measurement uses five warmups, seven samples and three fresh processes per runtime.

Refresh handle 26926 completed exit 0. All 14 datasets are current; 71 timing
workloads include six new construction cards. Final construction evidence uses the
default 5 warmups / 7 samples / 3 fresh processes, with 21 samples per runtime per
workload (252 measured runtime samples). Audit 29234 exited 0 and independently
rechecked inputs, output scores/assignments, small optima and all measured call counts.
All 201 small comparisons agree in assignments/scores and native references, with
122 enumerated optima each. Existing comparison invariants remain unchanged.

Observed construction medians in ms (Rust / Java): Sequential32 0.053216 / 3.566291,
Sequential96 0.709755 / 4.869291; Pooled32 0.875619 / 6.049585, Pooled96 25.871895 /
41.708188; ValueQueue32 0.287323 / 4.192019, ValueQueue96 7.938188 / 14.065829.
These are workload-specific, end-to-end full-scoring results, not universal speedups.
Sequential/pooled reach the known zero optimum; queue returns the same infeasible
scores in both engines. All measured score-call counts retain full construction work
across repeated samples; Java calls are two above native on these selected inputs.

Browser checks passed at 1440x1000 and 390x844: all six new cards, 71 overall,
18.23% coverage, 201/201 construction diagnostic with 122/201 optima, current-state
indicator, exact returned scores, known optima and 3x7/5-warmup caption. No page errors,
undefined text or horizontal overflow. Inspected work/construction-benchmark-
{desktop,phone,phone-quality}.png; reload keeps the new cards current. Browser closed;
localhost:8765 server remains running. No test/measurement process remains live.
Staged benchmark code, validation, report wiring and documentation were reviewed.

This turn made verified progress on the explicitly requested performance dashboard.
The full solver rewrite remains active, incomplete and unblocked. No adapted-method
credit was added. Remaining Community behavior and Java lifecycle/copy semantics still
need implementation and verification; do not mark the goal complete at this milestone.

## 2026-10-04: Nested local-search stopping conditions

Previous verified increment is d24db2a. Added independently implemented borrowed
StepTermination expressions: All/Any over completed steps, stagnation steps,
accepted moves, exact score targets and feasibility. Shared boundary evaluation
integrates all six search paths without changing existing resource/cancellation
checks. Conditions do not latch; borrowed expression reuse has phase-local counters.
Empty All is true, empty Any false. No time expressions, progress gradients or
solve-wide composition. Read the new incremental-search contract section.

Inspected only pinned AndCompositeTerminationTest, OrCompositeTerminationTest and
their abstract test helper. Full mock scope/applicability/gradient assertions are
not represented by this native feature; all remain pending. No production source
or adapted fixtures were used, and no provenance credit was added.

Five new original search tests cover 98 rule/acceptance combinations across six
paths (588 solves), empty/initial conditions, resource precedence, exact decimal and
multi-level scores, accepted-vs-fallback counts and non-latching stagnation. Focused
18-test search suite passed. One additional six-path orchestrator test verifies
fresh counters and continued phases. First compile found a test-only misspelled
PlanCompleted variant, corrected. Clippy then detected phase enum size difference
crossing its 200-byte threshold after one new borrowed field. Three targeted expect
annotations explain retaining inline infrequent phase configs and the current public
API rather than adding boxing/allocation. No broad lint suppression.

Full gate runs under handle 97276, work/step-condition-check.log. Expected counts
are 1,867 Rust tests, 99 doctests, 223 Python tests until verified at exit. Prepared
work/step-condition-refresh.sh (14 sequential jobs) and work/step-condition-audit.py.
Sources are frozen for capture and subsequent measurements. Full goal remains active,
incomplete and unblocked. Re-poll the same handle after observation timeouts.

Full gate 97276 completed exit 0: 1,867 Rust tests, 99 doctests and 223 Python
checks passed, formatting/Clippy clean. Coverage remains 885/4,855=18.229%, pending
3,688 / excluded 282 / failed 0 / unverified 0. Sources remain frozen for focused
release checks and all 14 sequential comparison/performance jobs.

Release/refresh/audit handle 60539 completed exit 0. All 18 focused release tests
passed. All 14 datasets and 71 timing workloads were refreshed; audit verifies
current source and verification fingerprints, live API freshness, coverage, all
six new regression methods and existing comparison invariants. Exact baseline
128/128, queue 67/67, pooled 66/66, construction 201/201 and heuristic 352/352
comparisons remain valid and equal-score; optimum counts remain separate.

Browser DOM checks passed on the existing dashboard at desktop and 390x844:
18.23% coverage and 71 timing cards, no page errors, undefined/stale-result text
or horizontal overflow. No UI layout changes were made, so no new screenshot
review was needed. Browser closed; localhost:8765 server remains running. Upstream
is unchanged at the pinned commit. No test or measurement process remains live.
Staged changes were reviewed, including the targeted enum-size lint expectations.

This increment adds useful native phase-local composition with original correctness
regressions. The full Community solver goal remains active, incomplete and unblocked;
shared/time composition, scope applicability and gradient adaptations remain future
work. The unchanged upstream coverage percentage is intentional and truthful.

## 2026-10-04: Seeded basic ruin-and-recreate search

Previous turn made verified progress in bc75328. New ruin_recreate_search_from and
ruin_recreate_stream_search_from implement actual repeated group clearing/greedy
reconstruction, native acceptance, strict best retention, seeded count/subset sampling,
pin/null freezing and global evaluation/attempt/time/cancellation controls. Read
ruin_recreate.rs and the new search-contract section. Model/graph/cache rebuilding
is deliberate; list/mixed support and SolverPlan integration remain future work.
No dedicated Java random-trace/timing claim. Only tests/public docs were inspected.

One complete pinned testRuiningNoAssignedValues adaptation preserves all three null
assertions through both backends. Its optional solution-generator and assigned-value
penalty fixtures are recorded in provenance and notices. Other selector, rebase,
equality and metric tests remain pending. Ten original regressions include 1,600
scoped/stream solves against full-score reconstruction (32 models x 25 budgets x two
backends), local-optimum escape, exact counters, seeded subset/pin behavior, multiple
placements/acceptors, cancellation at each score call, delayed/cancelling factories
and exact owned decimals. Sequential/Pooled escape the crafted valley; ValueQueue
correctly retains the old best. Do not assume equal strategy quality.

Focused 11-test suite passed. Initial fixture fixes: one aggregate scoped penalty
avoids duplicate constraint names; stream aggregate callbacks fill not-yet-inserted
rows with None; decimal network receives explicit value type. Clippy rejected a
test-only clone on Copy, corrected. Full gate now runs as handle 34758 with output
work/ruin-check.log. Expected totals: 1,878 Rust tests, 100 doctests, 223 Python
checks, 886/4,855 methods (18.249%), pending 3,687, excluded 282, failed/unverified 0.
Until the gate exits these are estimates. Sources/provenance are frozen for capture.
Prepared work/ruin-refresh.sh (14 sequential jobs) and work/ruin-audit.py. Re-poll
live handles on observation timeouts. Full goal remains active/incomplete/unblocked.

Full gate 34758 completed exit 0: 1,878 Rust tests, 100 doctests and 223 Python
checks passed; formatting/Clippy clean. Coverage is 886/4,855=18.249%, pending
3,687 / excluded 282 / failed 0 / unverified 0. Moves & selectors becomes 125/874.
Sources/provenance remain frozen for focused release and the 14-dataset refresh.

Release/refresh/audit handle 33922 completed exit 0. All 11 focused release tests
passed, all 14 existing measurement datasets were refreshed, and work/ruin-audit.log
confirms current source/verification fingerprints and live freshness. There remain
71 existing timing workloads; no dedicated ruin/recreate Java timing was added.
Existing quality/counter invariants pass, including baseline 128/128, heuristic
352/352 and construction 201/201 agreement; optimum counts stay separate.

Browser DOM verification passed at desktop and 390x844: 18.25% overall coverage,
Moves & selectors 125/874, all 71 timing cards, and the filtered
RuinRecreateMoveSelectorTest.testRuiningNoAssignedValues explicitly Passed. No page
errors, undefined/stale-result text or horizontal overflow. No UI layout changed.
Browser closed; localhost:8765 server remains running. No test/measurement process
remains live; upstream is unchanged at the pinned commit. Implementation, tests,
provenance/notices and documentation were staged and reviewed.

This turn made verified progress toward the full independent solver rewrite. The
full goal remains active, incomplete and unblocked. Next work should integrate this
search into native plans, support list/mixed repair and build matched public-Java
quality/performance evidence without inflating method coverage.

## 2026-10-04: List ruin-and-recreate with a shared search engine

Previous verified progress is c101056. Added ruin_recreate_list_search_from and
ruin_recreate_list_stream_search_from. List samples exclude full/prefix pins and
include missing optional values with legal receivers. Selected values are removed;
unsampled owners/order and missing values remain fixed. Original registry identity
indices are preserved while temporary entity ranges restrict missing candidates.
Both native backends use existing list construction for sequential/pooled placement;
ValueQueue retains sequential list semantics. Read the new search-contract section.

Shared ruin_engine.rs owns work limits, seeded selection, acceptance and best
retention for both basic/list models. RuinRecreateOutcome is the generic public
container; existing basic result and new list result names are scored-type aliases.
The basic shuffled permutation is retained while eligibility stays unchanged.
List metadata uses changed owner indices, their final lists, and registry identities
whose owner/index changed. These are documented native identities, not Java equality.
Model/graph/cache rebuild cost remains deliberate; mixed repair/plan integration
and dedicated Java repair quality/performance evidence remain pending.

One complete ListRuinRecreateMoveSelectorTest.testRuiningOnlyOneAssignedValues
adaptation preserves the one-entity/three-value fixture, list-size penalty and final
size-one assertion in both backends. Method/fixture provenance and notices added.
Eight original regressions cover 1,584 list solves against independent insertion
(24 models x 33 budgets x two backends), sampled subsets/pins/ranges/nulls, stable
value-scoped constraints and stream shadows, multiple placements/acceptance families,
complete optional vs partial required cancellation, factories and owned decimals.
All 20 focused tests (9 list, 11 basic) passed. Test-only fixes included a closure
lifetime and redundant integer cast; no source assertions were weakened.

Full gate runs under handle 38768, work/list-ruin-check.log. Expected until exit:
1,887 Rust tests, 101 doctests, 223 Python checks; 887/4,855 methods=18.270%, pending
3,686/excluded 282/failed 0/unverified 0; Moves & selectors 126/874. Sources and
provenance are frozen. Prepared work/list-ruin-refresh.sh (14 sequential jobs) and
work/list-ruin-audit.py. Re-poll live handles after observation timeouts. Full goal
remains active, incomplete and unblocked.

Full gate 38768 completed exit 0: 1,887 Rust tests, 101 doctests and 223 Python
checks passed, formatting/Clippy clean. Coverage is 887/4,855=18.270%, pending
3,686 / excluded 282 / failed 0 / unverified 0; Moves & selectors 126/874.
Sources/provenance remain frozen for focused release and the 14-job refresh.

Release/refresh/final-audit runs as handle 91258. All 20 focused release tests passed.
Attribution review identified that the new rustdoc example also reuses the adapted
size-one list fixture. Added its own fixture ledger entry and notice, limited to
the example; no Rust implementation changed. This updates verification provenance,
not measured-source fingerprints. work/list-ruin-audit.py now runs a final full gate
into work/list-ruin-final-check.log after all 14 timed jobs finish, then checks data
freshness and invariants. This avoids competing CPU work during measurements and
recaptures the updated attribution ledger. Re-poll the same live handle.

All 14 measurement jobs in 91258 finished. Its final report command rejected the
old verification capture after the attribution ledger update, so set -e stopped
before invoking the planned final audit (exit 1). No measurement failed or needs
restart: measured-source files are unchanged. Launched work/list-ruin-audit.py
explicitly as handle 14823; it runs the final full gate/capture first, then audits
all datasets. Follow work/list-ruin-final-check.log and work/list-ruin-audit.log.

Final gate/audit handle 14823 completed exit 0. Updated attribution is captured;
1,887 Rust tests, 101 doctests and 223 Python checks pass. All 14 datasets have
current measured-source/verification fingerprints and live freshness; 71 existing
timing workloads and prior quality/counter invariants pass. No dedicated repair
Java timing was added. All 20 focused basic/list release tests also passed.

Browser DOM checks passed at desktop and 390x844: 18.27%, Moves & selectors 126/874,
71 timing cards, and filtered testRuiningOnlyOneAssignedValues explicitly Passed.
No page errors, undefined/stale-result text or horizontal overflow. No layout changed.
Browser closed; localhost:8765 server remains running. No test/measurement process
remains live. Pinned upstream is unchanged. Implementation, shared-loop refactor,
regressions, provenance (including the rustdoc fixture) and notices were reviewed.

This increment makes verified progress on list repair and shared search control.
The full Community goal remains active, incomplete and unblocked. Mixed repair,
SolverPlan integration and dedicated public-Java repair quality/performance evidence
remain substantive next work; other selector/lifecycle methods stay pending.

## Mixed repair increment (verified)

Added mixed_ruin_recreate.rs and original mixed repair checks. The shared engine
samples one basic-slot/list-value pool, then uses mixed construction under explicit
component order and coupled scoring. Acceptance namespaces distinguish basic slots
from list entities and Basic from List values; list registry identity is preserved.
No new upstream method/fixture was adapted, so provenance and method credit stay
unchanged. See the mixed section in docs/incremental-search-contract.md.

Focused checks initially caught two test expectations: ValueQueue commits its first
queued basic value even in list-first order; factory cancellation skips repair
scoring, whereas cancellation from the scoring callback retains a scored optional
repair. Tests now exercise the intended distinctions. Production assertions were
not weakened. Full goal remains active, incomplete and unblocked.

Full gate 49326 passed: 1,897 Rust tests, 101 doctests, 223 Python tests;
formatting/Clippy clean after simplifying one test closure type annotation.
Coverage remains 887/4,855=18.27%, pending 3,686, excluded 282, failed/unverified 0.
Sources are frozen. Handle 22418 runs focused release checks (basic/list/mixed),
then 14 sequential refresh jobs and work/mixed-ruin-audit.py. Logs:
work/mixed-ruin-release.log, work/mixed-ruin-verify.log, work/mixed-ruin-audit.log.
The audit expects 1,897 tests and unchanged 71 workload/compatibility invariants.
Re-poll the same live handle on observation timeouts; do not restart timed work.

Release/refresh/audit handle 22418 completed exit 0. All 30 focused repair tests
passed in release mode. All 14 measurement datasets are current with matching
source/verification fingerprints. The audit confirms unchanged coverage, 71 timing
workloads, independent quality references, work counters and known discrepancies.
No dedicated Java mixed-repair measurement is claimed.

Browser checks passed at desktop and 390x844: 18.27%, 71 timing cards, no stale or
undefined text, page errors or horizontal overflow. Browser closed; localhost:8765
server remains running. No test or measurement process remains live. Pinned upstream
and attribution ledger are unchanged; every new check is original.

Next substantive work: shared repair controls needed for SolverPlan integration
(independent move budgets, score targets, feasibility and observation/stagnation
hooks), then all six plan/backend paths and managed lifecycle. Existing repair
entry points only expose score/attempt/time/cancellation limits. Preserve existing
seeded traces and partial-repair retention when extending the shared engine.
The full Community goal remains active, incomplete and unblocked.

## Shared repair controls increment (verified)

Added independent max_move_evaluations, exact best_score_limit, and
stop_when_feasible to shared RuinRecreateConfig, with typed termination reasons.
Engine passes remaining candidate budgets into each reconstruction and checks goals
only against complete retained best at attempt boundaries. Precedence and zero-limit
semantics are documented in docs/incremental-search-contract.md. Defaults preserve
previous seeded behavior. No new upstream adaptation or provenance change.

Seven focused original tests passed, including 2,550 independent score/candidate
budget combinations and 702 placement/early-pick combinations across all six paths.
The first test harness incorrectly joined a populated source with an unused empty
source for pure basic/list models; fixed those graph definitions and added explicit
independent final-score assertions. No production assertion was weakened.
Full goal remains active; observation/stagnation hooks and phase integration follow.

Full gate 35103 passed: 1,904 Rust tests, 101 doctests, 223 Python tests;
formatting/Clippy clean. Coverage remains 887/4,855=18.27%, pending 3,686, excluded
282, failed/unverified 0. Sources frozen. Handle 96395 runs focused release tests,
14 sequential measurement refreshes, then work/repair-controls-audit.py. Follow
work/repair-controls-release.log and work/repair-controls-verify.log. Expected final
invariants remain 14 current datasets and 71 workloads. Re-poll the same live handle
on an observation timeout; do not restart measurement work.


Release/refresh/audit handle 96395 completed exit 0. All 37 focused repair tests
passed in release mode. Fourteen datasets have current measured-source/verification
fingerprints; 71 timing workloads, prior quality/counter checks and known discrepancy
counts pass the final audit. No dedicated Java repair timing or method credit added.
Browser checks passed at default desktop and 390x844: 18.27%, 71 cards, no stale or
undefined text, page errors or horizontal overflow. Browser closed; localhost:8765
server remains running. No test/measurement job remains live. Upstream unchanged.

Next: add observer/control plumbing to shared ruin_engine, preserving seed and work
counts. Reuse SolutionUpdate for initial and strict retained-best notifications;
SearchControl in time_limit.rs exposes start_search, record_improvement,
complete_step and stop. Decide/document repair step semantics explicitly (accepted
changed moves currently advance acceptance history; attempts include rejections).
Then wire SolverPhase/MixedSolverPhase, phase reports, global limits, factories and
managed lifecycle across all six backends. RuinRecreateError already wraps native
local errors; avoid introducing unboxed recursive error types when integrating.
The full goal remains active, incomplete and unblocked.


## Repair observers increment (verified)

Added `_with_observer` counterparts for all six repair APIs, using SolutionUpdate.
Shared engine emits initial complete scored state and strict retained-best updates
only; notification follows retention/acceptance bookkeeping. Callback cancellation
and time are checked at the next attempt boundary. Existing unobserved wrappers pass
None, avoiding extra score clones. Public/private wrapper split shares validation,
factory timing and reconstruction; internal RepairObserver alias avoids complex
callback signatures. No new upstream method/fixture or provenance change.

Five new original regressions passed: 1,638 observed/unobserved budget pairs (3,276
solves), exact counter/results and event equivalence, initial/improvement cancellation,
callback timing, decimals, skipped scoring, panic propagation/reuse, optional nulls,
and successive sampled improvements. Original seven control regressions still pass.
Full gate runs as handle 73739 into work/repair-observers-check.log; formatting and
Clippy already pass. Sources frozen. Expected full counts: 1,909 Rust / 101 doctests /
223 Python; coverage unchanged 887/4,855=18.27%. Goal active and incomplete.

Full gate 73739 completed exit 0 with expected counts: 1,909 Rust tests, 101 doctests,
223 Python checks and clean format/Clippy. Release/refresh/audit runs as handle 92449:
work/repair-observers-release.log, work/repair-observers-verify.log and
work/repair-observers-audit.log. Expected 42 focused release checks, 14 current
measurement datasets and 71 timing workloads. Re-poll the same handle on observation
timeouts; do not restart timed jobs. No source/provenance changes during measurement.


Release/refresh/audit handle 92449 completed exit 0. All 42 focused repair tests
passed in release mode. All 14 datasets have current measured-source/verification
fingerprints, with 71 timing workloads and prior quality/counter/discrepancy checks
passing. No new Java repair benchmark or adapted method credit is claimed.
Browser checks passed at 1440x900 and 390x844: 18.27%, 71 cards, no stale/undefined
text, page errors or horizontal overflow. Browser closed; localhost:8765 remains
running. No test or measurement process remains live. Upstream/provenance unchanged.

Next substantive work remains full repair-phase integration. Shared observer support
is now available; add SearchControl monitoring and explicit accepted-step semantics,
then all six SolverPhase/MixedSolverPhase backend paths, global work/time/goal limits,
phase reports and managed solver factories. Existing private *_observed wrappers
share optional callbacks; expose crate-level variants as needed rather than duplicate
validation/scoring logic. Avoid recursive unboxed native/repair error variants.
Keep goal active: the full Community solver remains incomplete and unblocked.


## Repair phase integration (verified)

Added SolverPhase/MixedSolverPhase::RuinRecreate, phase kind/termination metadata,
SolverError::Repair and six backend dispatch paths. Shared config clipping covers
score/candidate/time/cancellation/targets/feasibility. Repair factories run inside
repair API, never as an extra outer graph. All ruin/acceptance/token validation runs
before phase work. IncompleteHandoffPolicy and first-initialized events now recognize
repair as an improvement phase. Managed factory validation recognizes repair tokens.

Private repair adapters accept SearchControl. Monitor starts after initial scoring,
before callbacks; strict bests record improvement, accepted changed repairs complete
steps. Rejected/no-op attempts do not age steps. Shared stagnation checks at repair
boundaries, with explicit typed reasons; full contract in docs/solver-contract.md.
Public standalone observer APIs retain their signatures and pass no shared monitor.

Sixteen focused tests pass; includes 2,016 independent shared-budget prefixes with
quality, six-path events/handoffs/goals/errors, mixed ordering, decimals, nonzero
stagnation and managed config/callback delivery. Extracted existing original fixture
into tests/solver_support/fixture.rs, included by legacy solver.rs and solver_repair.rs;
legacy method names unchanged. Updated exhaustive matches in original test helpers.
No upstream production read or new adapted method/fixture. Coverage stays unchanged.

Test setup fixes: explicit SolverSequence avoids SolverPlan's implicit construction;
flat local-score stagnation fixture explicitly selects HillClimbing because default
local acceptance is StrictImprovement. Removed obsolete large_enum_variant lint
expectations (now unfulfilled). Fixed new test type annotations. No production
assertions were weakened. Clippy passes. Next run full gate then focused release,
14 measurements, audit, browser checks and commit. Full goal active and incomplete.

Full gate 7007 completed exit 0: 1,925 Rust tests, 101 doctests, 223 Python checks;
formatting/Clippy clean. Sources frozen. Handle 35700 runs 58 focused release checks,
then the 14 sequential refresh jobs and work/repair-phase-audit.py. Logs:
work/repair-phase-release.log, work/repair-phase-verify.log, work/repair-phase-audit.log.
Expected coverage remains 887/4,855=18.27% with 14 current datasets and 71 timings.
Re-poll the same live handle after observation timeouts; do not restart timed jobs.


Release/refresh/audit handle 35700 completed exit 0. All 58 focused release checks
passed (16 new phase cases plus existing repair controls/observers/basic/list/mixed).
All 14 datasets have current source/verification fingerprints, with 71 timing
workloads and prior quality/work/discrepancy invariants intact. Coverage remains
887/4,855=18.27%, pending 3,686, excluded 282, failed/unverified 0. No extra method
credit or dedicated Java repair timing is claimed.

Browser checks passed at 1440x900 and 390x844: 18.27%, 71 cards, no stale/undefined
text, page errors or horizontal overflow. Browser closed; localhost:8765 remains
running. No test/measurement process remains live. Upstream and provenance unchanged.

Repair-phase integration is now implemented across all six solver paths, with
shared controls, initialization/best events and managed configuration/callback tests.
Next substantive work: dedicated public-Java repair quality/performance evidence
and remaining complete Community selector/lifecycle adaptations. Read pinned tests
and public API docs only; never upstream production implementations. Existing repair
model/cache rebuild cost is deliberate and should be measured before optimization.
The full goal remains active, incomplete and unblocked.

## Dedicated basic-repair measurement increment (verification underway)

Added original RepairBenchmark.java and repair_benchmark.rs adapters, independent
benchmark_repair.py and six protocol/scope/quality regressions. Public Java basic
ruin selection compares native standalone basic repair; half/whole fixed groups,
seed 20261222, eight repairs, step-counting threshold period nine. Worst all-zero initial
scores guarantee every complete candidate meets the retained initial threshold.
Java public MOVE_EVALUATION_COUNT is re-registered after solving and checked against
the requested work, with reachabilityFence; native checks attempts, complete repairs
and AttemptLimit. Per-runtime score calls are separate, not asserted identical.

The new report/dashboard section shows four timing workloads, actual repair counts,
score calls and final quality. Small quality fixtures are 32 generated conflict/
preference cases per mode with exhaustive optima; timed 32/96-variable fixtures have
planted zero optima. Every result is independently validated; no new adapted fixture
or upstream credit. Full scope/methodology is in docs/dashboard.md. No production
implementation was inspected. Upstream/provenance unchanged.

Initial adapter build issues corrected before measurement: config requires clone;
Micrometer SimpleMeterRegistry needs explicit close; move gauge must be registered
again after solve. Initial simple probes passed but are not final measurements.
Full gate runs as handle 61502 into work/repair-benchmark-check.log. Expected counts:
1,925 Rust, 101 doctests, 229 Python; 887/4,855 methods unchanged. Sources frozen.
Next run new repair measurements plus all prior 14 jobs sequentially, regenerate
report, audit 15 current datasets/75 timing workloads, check browser and commit.
Full goal remains active and incomplete.

The first broader benchmark correctly failed closed: repair-quality-2 required
14 Java move evaluations for eight late-acceptance steps. Hard-score improvements
reset late history, so its initial threshold does not persist. Switched both
adapters to StepCountingHillClimbing STEP with period steps+1; unlike late acceptance,
this threshold only updates at the configured completed-step boundary. Re-running
full gate as handle 14559; discard prior probe/failed measurement data. The strict
Java move count remains required, never relaxed. Six Python benchmark checks pass.

Corrected full gate 14559 completed exit 0: 1,925 Rust tests, 101 doctests and
229 Python checks, format/Clippy clean. Sequential refresh/audit handle 79918 is
running; logs work/repair-benchmark-refresh.log and per-job siblings. New repair
measurements finished: 64 valid cases, 52 equal scores, optima Java 35/Rust 33.
Four timing workloads all reach planted zero optima. Warmed medians (Rust/Java ms):
half32 .413/7.224, half96 3.796/18.435, all32 .543/10.859, all96 6.333/24.480.
These are workload-specific observations with raw ranges, not overall solver speed.
All measured solves satisfy eight repairs; remaining prior 14 datasets refreshing.
Do not modify measured sources during refresh; re-poll the existing handle after
observation timeouts. Audit expects 15 current datasets and 75 timing workloads.

Refresh/audit handle 79918 completed exit 0. All 15 datasets have current measured
source/verification fingerprints; 75 timing workloads and existing discrepancy,
quality and counter invariants pass. New independent audit verifies all 64 quality
cases plus 168 measured runtime samples (84 per runtime) against domains, scores,
optimum bounds and eight repairs. Native score-call counts also equal the full
expected repeated reconstruction work, excluding accidental reused solved input.

Browser verification caught omitted repair methodology/environment DOM targets;
added both in the existing methodology section. A repeated same-URL navigation
retained old markup; a fresh query URL verified the corrected page. At 1440x900 and
390x844: content visible, 18.27%, 75 cards/four repair cards, eight repairs per sample,
52 equal-score quality cases and methodology present; no stale/undefined/error text,
horizontal overflow or page errors. Browser closed; localhost:8765 stays running.
No test or benchmark process remains live. Upstream/provenance unchanged.

Next substantive work: expand independent list/mixed/optional repair evidence and
remaining fully preservable Community selector/lifecycle test adaptations. The new
basic comparison explicitly retains half-group quality differences (20/32 equal;
Java 19 optima, Rust 17) while whole-group results agree in 32/32 (16 optima each).
Do not treat differing random traces or quality as method passes or hide them.
Full solver rewrite remains active, incomplete and unblocked.

## Repair work counters and remaining basic selector tests (verification underway)

SolverWork now exposes repair_attempts and complete_reconstructions. All six phase
adapters preserve standalone counts; plan totals sum them, other phases contribute
zero, and managed statistics apply monotonic absolute updates plus saturating
additions/overflow flags. Existing candidate and accepted-step counters stay distinct.

Three more RuinRecreateMoveSelectorTest methods are adapted in ruin_recreate_cases:
ruining, ruining_with_metric and ruining_allows_unassigned. Preserve five-value/
30-entity generators, required cyclic initial values and optional first-null/rest-
cyclic values, assigned-entity penalty and optional -1/null,-1000/ordered collision
score. Required cases keep 1000ms unimproved termination; optional native limit is
100 full repair attempts. Full score diagnostics and both scoring backends execute.
Successful-solve and all three metric inequalities remain; public SolverWork counts
replace Java Micrometer, explicitly not registry or numeric/trace parity. All reused
test fixtures and transformations attributed in provenance/notices; no production
implementation read. Expected coverage 890/4,855=18.33%, Moves/selectors129/874.

Three original regressions check 2,850 score/candidate prefixes across six paths,
complete/no-op/rejected work and managed counter monotonicity/additive overflow.
Existing managed repair tests compare actual job statistics to returned work.
Corrected an initial reference expectation: completing the last required slot can
produce a complete assignment before all candidates for that slot are scanned.
Focused 31 checks pass (18 phase, four adapted basic repair, nine job statistics).
Next full gate, focused release, all 15 datasets, audit/browser and commit.
Expected full counts: 1,931 Rust,101 doctests,229 Python. Full goal remains active.

Full gate 6664 completed exit 0: 1,931 Rust,101 doctests,229 Python; formatting and
Clippy clean. Coverage verified 890/4,855 (raw report percent18.332, UI18.33%),
pending3683, excluded282, failed/unverified0. Sources/provenance frozen. Sequential
release/refresh/audit now runs work/repair-metrics-verify.sh; logs repair-metrics-
release.log, -verify.log, -refresh-*.log and -audit.log. Expect 31 focused release
checks, 15 current datasets/75 timings. Re-poll the live handle; observation timeouts
are not process termination. No measurement/source mutation while this job runs.

Release/refresh/audit handle 79129 completed exit 0. All31 focused release checks
passed; all15 datasets are current,75 timing workloads. Existing quality/counter/
discrepancy invariants remain intact. New basic repair metrics and four source
methods verified, with source/verification fingerprints matching current files.

Browser verification passed at1440x900 and390x844 using a fresh query URL:
18.33%,890/4,855 methods,75 timing cards; Moves/selectors129/874(14.76%). Searching
for generic/RuinRecreateMoveSelectorTest.java shows all four named methods passed.
No horizontal overflow, stale/undefined/error text or page errors. Browser closed;
localhost:8765 remains running. No verification process is live. Upstream unchanged.

Next substantive work remains list/mixed/optional repair comparisons, applicable
list selector tests and broader native lifecycle/search behavior. Basic repair
work reporting is complete for successful solver results; failed phases retain
existing typed-error behavior, without claiming complete partial work telemetry.
The full solver rewrite remains active, incomplete and unblocked.


## Weighted change/swap/repair search (verified)

RepairMoveWeights interleaves change/swap/repair with one shared current state,
acceptor/history, best and random source across all six model/backend paths. Default
0:0:1 preserves prior repair-only RNG consumption. Zero disables families; all-zero
fails up front; empty families renormalize, all-empty yields NoAvailableMoves.
Fine selection uses reservoir sampling over existing providers (mixed basic then
lists), preserves pins/ranges and polls cooperative controls between iterator calls.
Fine scoring initializes a fresh cache/graph; costs are included, with no incremental
fine-search performance claim. Total attempts combine repair and fine; separate
fine_move_attempts/fine_move_evaluations distinguish actual work. Candidate caps
include fine scores but exclude repair initialization. Phase and managed statistics
preserve counters, monotonicity and additive overflow.

Three more ListRuinRecreateMoveSelectorTest methods adapted: required hash fixture
and metrics preserve 1000ms unimproved termination, construction and full native
score diagnostics. Optional scoring preserves squared lengths, five per missing,
unique assigned same-index pairs and 40/40/20 weights. Native 100 selected attempts
replace Java completed-step cap; no trace or Micrometer API parity. Attributed Java
Objects/String oracle verifies all 300 required fixture hash penalties. All four
list selector methods pass; UnionMoveSelectorTest lifecycle remains pending.

Original regressions cover 2,496 standalone weighted solves across backends/budgets/
acceptors, 1,014 phase score/candidate prefixes, shared tabu/stagnation, empty families,
large weights, owned decimals, strict notifications, sampling/factory cancellation,
phase aggregation, managed publication and counter monotonicity/overflow.
Full gate passed:1,945 Rust,101 doctests,229 Python; formatting/Clippy clean.
All58 focused release tests passed. Logs: work/hybrid-check.log, hybrid-release.log,
hybrid-verify.log, hybrid-refresh-*.log and hybrid-audit.log. No process remains live.

All15 datasets are current,75 timing workloads. Fingerprints and prior invariants
pass the audit. Coverage893/4,855(raw18.393/UI18.39), pending3680,excluded282,
failed/unverified0. Moves/selectors132/874(15.10%). Repair quality remains64valid/
52equal, Java35/Rust33 optima; half20/32equal,whole32/32equal. No new weighted-mode
Java performance claim. Upstream unchanged.

Browser verified1440x900 and390x844 with fresh query URL:18.39%,893/4,855 methods,
75 cards; list selector filter shows all four methods passed. No horizontal overflow,
stale/undefined/NaN text or page errors. Browser closed; localhost:8765 stays running.
Next work: independent list/mixed/optional and weighted public-Java quality/timing
comparisons, or remaining fully preservable selector/lifecycle behavior. Any fine
scoring optimization must preserve budgets, errors/cancellation and actual work.
Full independent solver rewrite remains active, incomplete and unblocked.

## Required/optional list repair comparison (verified)

Original public Java adapter ListRepairBenchmark, native example
list_repair_benchmark, Python runner benchmark_list_repair.py and six evidence
regressions. Homogeneous required/optional lists, no pins/ranges, full scoring;
penalties for owner cost, squared capacity excess, ascending neighboring IDs and
missing values. Initial ascending owner-zero list is provably worst, so long STEP
threshold accepts completed proposals. Requested groups 1..ceil(n/2) or 1..n;
eight repairs verified in both runtimes for every quality/timing sample. Native
actual constraint callbacks and Java easy-score calls are retained separately.

64 valid/56 equal; Java 64 optima/Rust 56. Both all-maximum groups 16/16 equal/optimal;
both half-maximum groups 12/16 equal/Rust optimal, Java 16/16 optimal. Larger fixtures
with 24/48 values have independent planted zero optima; optional results omit 4/9
values. Half-maximum timings differ in quality (24: Rust0/Java-2006; 48: Rust-3013/
Java-1001), all-maximum timing scores zero in both. Preserve quality beside ratios.
Fixed-minimum probes: required min1 or3 and optional min1 finish 8/8. Optional
min=max3: Java finishes one repair after retaining two assigned values; native
finishes eight including eligible missing values. This observed boundary remains
separate from matched-work measurements; no Java internals or random trace claim.
No source-method credit from these original fixtures. Assigned owner/index metadata
is validated in both adapters. Python independently enumerates owners/omissions/
permutations for small optima and checks every returned assignment and score.

Full gate passed: 1,947 Rust,101 doctests,235 Python; formatting/Clippy clean.
Two focused release example tests passed. Coverage unchanged 893/4,855 (18.39%),
pending3680/excluded282, failed/unverified0. Node syntax check clean. All 16 datasets
are current, 83 timing workloads. List repair has 336 measured runtime samples
(3 forks × 7 samples × 8 workloads × 2 runtimes), five warmups per process.
Audit validates all prior invariants, fingerprints, 64 new quality cases, four
boundary probes and every sample against independent score/work/optimum checks.
Logs: work/list-repair-check.log, -release.log, -verify.log, -refresh-*.log,
-audit.log. Handle20388 completed exit0; no verification process remains live.

Dashboard adds a separate list-repair section, eight cards, method/environment and
quality/boundary details using existing layouts. Browser verified 1440x900 and
390x844: 18.39%,83 cards/eight new cards, four quality-difference flags, 8/8 work
counts, four boundary paragraphs, no overflow/stale/undefined/NaN text or page
errors. Screenshots inspected at work/list-repair-desktop.png and -mobile.png.
Use IIFEs for repeated agent-browser evals (top-level const names persist) and
instant scrollIntoView before screenshots (default smooth scrolling can capture
the old viewport). Browser closed; localhost:8765 stays running. Upstream unchanged.

Next substantive work: mixed/weighted public-Java search comparisons and remaining
fully preservable selector/lifecycle behavior, or native search improvements with
broad quality evidence. Half-group quality gaps are retained, not evidence of
identical random sampling. Full solver rewrite remains active/incomplete/unblocked.

## Assigned-only mass-list neighborhood (verified)

ListSampleSize supports exact/ranged/all assigned movable groups. Zero/singleton
sizes yield no moves; exact populations too small yield none, maximums clip.
All uses global assigned movable count before receiving-range filtering.
ListMassChangeConfig defaults to model optionality for whole-group unassignment;
explicit true on a required model is UnassignmentNotAllowed. Missing values never
join any sample. Immutable ListMassChange metadata exposes ordered members,
original insertion destination and snapshot-bound ordinary composite movement.
Enumeration is lazy: receiver/index, size, registry permutation, then unique
unordered optional unassignments. Pins/prefixes/ranges constrain members/receivers.
General Gather behavior/order remains unchanged; shared permutation helper gains
bounded starting size. No factorial candidate collection is allocated.

Neighborhood::MassListChange integrates list/mixed scoped+stream search and joint
moves; basic-only paths retain changes/swaps. All four list/mixed solver sequence
adapters validate every configured null policy before any phase scoring/factory.
Validation errors preserve phase index. Existing acceptance, controls, observations,
transactional cache/graph and retained-best machinery is reused.

All ten MassListChangeMoveProviderTest methods adapted with original fixtures,
limits and assertions. Finite deterministic enumeration replaces sampled Java
streams, native candidate metadata/model/errors replace metamodel/constructor APIs.
No Java probability, lifecycle or internal move-equality claim. Provenance/notices
record methods, in-method fixtures and ranged fixture constructor semantics.
No production implementation inspected. Original 4,455 successor-set comparisons
cover layouts/pins/ranges/cardinality/null policy and validate candidate metadata,
rollback, completeness and missing-value exclusion. Other tests cover lazy80-member
full permutations, all-global semantics, optional search, and validation before
factories. Expanded existing gather integration matrix covers both new size modes,
three move orders/four acceptors/five score budgets across list/joint mixed paths;
basic controls verify ordinary changes/swaps still apply.

Gate14526 passed:1,962 Rust,102 doctests,235 Python; format/Clippy clean. Coverage
903/4,855(raw18.599/UI18.60%), pending3670/excluded282,failed/unverified0;
Moves/selectors142/874(16.25%). All 20 focused release tests passed. Handle23218
completed exit0: all16 datasets refreshed,83 timing workloads, evidence audit
passed with source/verification fingerprints and prior quality invariants intact.
Logs: work/mass-list-check.log, -release.log, -verify.log, -refresh-*.log,-audit.log.
Browser verified1440x900 and390x844:18.60%,903/4855,83cards, Moves142/874,
allten mass-list methods passed, eight list-repair cards retaining four explicit
quality differences and8/8 repair counts. No page errors, overflow or stale/
undefined/NaN words. Screenshots inspected: work/mass-list-desktop.png and
-mobile.png. Word-boundary checks avoid matching NaN inside "nanosecond".
Browser closed; localhost:8765 stays running. Upstream unchanged.

Next substantive work: basic mass-change grouping, general k-opt, remaining fully
preservable selector/lifecycle methods, or mixed/weighted public-Java comparisons.
Mass-list search performance remains unmeasured; existing list/gather comparisons
do not imply a new provider benchmark. Full goal remains active/incomplete.

## Arbitrary basic mass-change neighborhood (verified)

MassChangeConfig supports exact/ranged/all group sizes and optional null-crossing
policy. Arbitrary basic slots may have different starting values. Lazy combinations
reuse native subset enumeration; all uses source-eligible population before target
filtering. Targets follow first-member domain order, then null. Every member must
admit the target, pins never join, zero/singleton groups and no-op states yield none.
Default null crossing admits missing optional slots and permits null targets only
for wholly optional groups. Explicit false excludes missing slots and null targets;
explicit true rejects any required model slot, including pins. Missing required
slots stay excluded. Empty models are valid. Immutable MassChange metadata carries
all selected slots, common destination and an ordinary composite writing each slot.
PillarSchema does not alter arbitrary slot grouping.

Neighborhood::MassChange integrates basic/mixed scoped and stream local search,
including joint moves. List-only paths keep changes/swaps. All basic/mixed solver
sequences validate every local-phase null policy before any scoring/factory even
with zero shared budget; phase indices remain in errors. Existing move ordering,
acceptance, budgets, score checking, rollback and observations are reused.

All ten pinned MassChangeMoveProviderTest methods adapted with their original
fixtures, limits and assertions. Native metadata/errors and finite enumeration
replace Java metamodels, constructor exceptions and sampled streams. No Java
probability, internal move identity or lifecycle claim. Provenance records methods,
in-method fixtures and TestdataSolution initialized modulo-value fixture generation.
No production implementation inspected. Five original regressions cover 20,146
bitmask/reference successor comparisons across ranges/pins/optionality/states/sizes,
metadata and rollback, lazy128-slot/64-member groups, empty/bad bounds, group-only
plateau escape across basic and joint mixed scoped/stream backends with three move
orders/three acceptors/five budgets, optional search, list fallback, and pre-scoring
validation across standalone and planned basic/mixed paths.

All 15 focused debug and release tests passed. Full gate2951 and verification
handle39399 completed exit0. All 16 comparison datasets are current,83 timings,
and the audit passed with source/verification fingerprints and prior invariants.
Verified gate counts: 1,977 Rust,103 doctests,235 Python; 913/4855(raw18.805/UI18.81%), Moves152/874,
pending3660/excluded282. Logs: work/mass-basic-focused.log, -check.log, -release.log,
-verify.log, -refresh-*.log, -audit.log. Mass-change search performance remains
unmeasured; prior pillar timing is not a benchmark of this new provider.

Next candidates include mass assign/unassign providers (their tests were searched,
not adapted: basic unassignment explicitly permits singleton samples), general
k-opt, and mixed/weighted public Java comparisons. Profile bounded enumeration
on sparse overlapping domains as well as successful early polls before claiming
practical large-group performance. Goal remains active, incomplete and unblocked.

Browser verified at 1440x900 and390x844:18.81%,913/4855,83 timing cards,
Moves152/874(17.39%), allten new methods passed, no overflow/stale/undefined/NaN
words or page errors. Screenshots inspected: work/mass-basic-desktop.png and
-mobile.png. Browser closed; localhost:8765 stays running. Upstream unchanged.

## Basic group assignment/unassignment (verified)

MassAssignmentSize supplies exact/ranged/all source sizes. Group assignment selects
missing unpinned slots (including required partial inputs), needs at least two
members and one non-null destination admitted by every member. All uses global
missing source count before destination filtering. Unassignment selects assigned
unpinned slots, permits singleton groups, and validates that every model slot is
optional even if pinned or requested work is zero. Empty models are valid. Finite
subsets follow size/member order and assignment targets first-member domain order.
Immutable MassChange metadata and ordinary composites reuse model/scoring rollback.

sample_mass_unassign_moves adds explicitly bounded, seeded sampling with replacement
between draws, without replacement within each group. It samples a size uniformly
from clipped positive bounds, shuffles eligible indices, truncates and sorts IDs.
No Java PRNG/distribution parity. Empty/insufficient populations yield no samples;
otherwise every requested draw is returned. Allocations remain linear per poll.
Neighborhood::MassAssign and MassUnassign integrate finite groups into basic/mixed
search including joint moves; lists retain changes/swaps. Existing validation hooks
reject bad unassignment configurations before any basic/mixed phase/factory work.

All three MassAssignMoveProviderTest and four MassUnassignMoveProviderTest methods
adapted with full fixtures, original limits/assertions and provenance. The singleton
unassignment method retains exactly fifty draws through the real seeded sampler
(seed29, fifty requested draws), not finite-list cycling. TestdataAllowsUnassigned-
Solution.generateSolution(3,5) starts with entity0 missing and indices1..4 assigned
index modulo three; this fixture source is attributed. No production code inspected.
Six original regressions include10,829 independent bitmask successor/metadata
comparisons across states, pins, optionality, sizes/modes; sampler repeatability and
prefix consistency over32seeds/six size modes; large lazy128-slot/64-member groups;
partial required assignment/all-population/empty-range boundaries; basic and mixed
joint/nonjoint scoped+stream search with three orders/three acceptors/four budgets;
and early validation across zero/nonzero plans and standalone paths.

All13 focused debug tests passed. Initial full gate65722 stopped at Clippy's
filter_map_bool_then lint; fixed with explicit conditional, no behavior change.
Full gate42747 passed. Handle89825 completed exit0:13 focused release tests,
16 current datasets,83 timings and audit all passed with source/verification
fingerprints and prior quality invariants intact.
Verified gate totals:1990 Rust,104 doctests,235 Python;920/4855(raw18.95/UI18.95%), Moves159/874,
pending3653/excluded282. Logs: work/mass-assign-focused.log,-check.log,-release.log,
-verify.log,-refresh-*.log,-audit.log. No new provider performance benchmark yet.

Next substantive candidates: list mass assign/unassign providers, general k-opt,
mixed/weighted public-Java search comparisons, and profiling sparse-domain group
enumeration before making practical performance claims. Full goal remains active,
incomplete and unblocked.

Browser verified1440x900 and390x844:18.95%,920/4855,83 cards, Moves159/874(18.19%),
allthree assignment and allfour unassignment methods passed. No errors, overflow,
stale/undefined/NaN words. Screenshots inspected: work/mass-assign-desktop.png and
-mobile.png. Browser closed; localhost:8765 stays running. Upstream unchanged.

## List group assignment/unassignment (verified)

list_mass_assign_moves/list_mass_unassign_moves require optional list values and
return OptionalValuesRequired for required models even at zero requested work.
ListSampleSize now shares an internal cardinality-floor helper: existing mass change
and new assignment require two members, unassignment permits one. Old change
semantics unchanged. Assignment sources only missing values, uses global missing
population for all-sized groups, and enumerates legal receiver/index then size and
registry permutations. Range/prefix/full-pin restrictions apply to all receivers.
Unassignment selects assigned movable identities, unique subsets in registry order,
excluding missing values, full pins and pinned prefixes. Candidate metadata is
ListMassChange, composites use native gather/atomic scoring rollback.

sample_list_mass_unassign_moves provides bounded draws with an explicit seed,
size uniform over clipped positive bounds, subset uniform without internal repeats,
with replacement between draws. Members follow registry index order. Enough source
members yields every requested draw, including repeated singleton groups. Empty
populations terminate. No Java PRNG/distribution parity. Neighborhood::MassListAssign
and MassListUnassign integrate finite enumeration into list/mixed scoped/stream
local search and joint moves. Basic-only modes retain changes/swaps. Existing
list/mixed sequence validation hooks reject invalid phases before any scoring or
factory, retaining phase indices. ShuffledVariables changes assignment receivers;
unassignment stays in registry order, ShuffledMoves can reorder either stream.

All five methods each from MassListAssignMoveProviderTest and MassListUnassign-
MoveProviderTest adapted with original fixtures, limits and assertions. Singleton
unassignment retains exactly50 draws via the real seeded sampler(seed31). Ranged
fixture constructors produce empty lists, A admits both values and B only first;
fixture attribution records this. Pinned public neighborhoods.adoc “Moving scattered
values” also confirms source/optionality/minimum-two assignment semantics. No
production implementation inspected. Five original regressions cover2268 independent
successor/metadata references over27 layouts,3pin modes,2range modes,2families,7sizes;
32seed/six-size bounded sampling; lazy128missing/64selected groups; all-global/empty
boundaries; list and joint/nonjoint mixed scoped+stream searches with threeorders,
threeacceptors,fourbudgets; all required-model rejection paths before callbacks.
Existing gather basic-fallback regression expanded to both new modes/all+zero sizes.

All20 focused debug tests passed. Full gate98819 completed exit0, fmt/Clippy passed.
Handle53951 completed exit0:20 focused release tests,16 current datasets/83timings
and audit passed with source/verification fingerprints and prior quality invariants
intact. Verified gate2005 Rust,105doctests,235Python;
930/4855(raw19.156/UI19.16%), Moves169/874,pending3643/excluded282. Logs:
work/list-assign-focused.log,-check.log,-release.log,-verify.log,-refresh-*.log,
-audit.log. New group providers remain without dedicated Java/Rust benchmarks.

Browser verified at1440x900 and390x844:19.16%,930/4855,83 timing cards,
Moves169/874(19.34%), all five assignment and five unassignment methods passed.
No overflow, stale/undefined/NaN words or page errors. Screenshots inspected:
work/list-assign-desktop.png and -mobile.png. Browser closed; localhost:8765
stays running. Upstream unchanged.

Next substantive candidates: general k-opt, mixed/weighted public-Java search
comparisons, dedicated group-provider benchmarks, and profiling sparse-domain
group enumeration. Full goal remains active, incomplete and unblocked.

## Single-list cyclic k-opt (verified)

Added independent graph reconnection through ListProblem::k_opt_move. Removed and
added undirected edges use absolute indices within one movable suffix; the suffix
wraps around its pinned prefix. Degree-two and connected-cycle checks reject bad
edges and subtours, including repeated-edge two-vertex cycles. An explicit first
successor chooses direction while the first unpinned identity stays anchored.
Ordinary snapshot-bound Change composites preserve assigned payloads, pins, ranges
and atomic scoring rollback. No production implementation inspected.

ListKOptConfig validates 2 <= minimum <= maximum, without a fixed maximum k.
list_k_opt_moves/Neighborhood::KOpt enumerate path reconnections lazily by entity,
k, cut combination, permutation and orientations. Exact changed-edge filtering
removes smaller exchanges; anchored-path symmetry removes duplicates; both directed
orders are emitted. Linear active storage does not avoid factorial search size.
Missing values and other entities remain untouched. Basic search retains ordinary
changes/swaps; mixed joint search combines those with the cyclic list moves.

Pinned public move-selector-reference.adoc documents cyclic edges and arbitrary k.
SelectorBasedKOptListMoveTest.testIsFeasible has a full native success/error adaptation.
The second case creates a duplicate retained edge, so DuplicateEdge rejects it
before connectivity traversal. Eight ordered-output fixtures preserve all lists,
pins, edges and output orders; their Java notification checks remain unadapted and
receive no method credit. Provenance and notices record each fixture separately.
Multi-entity k-opt repartitioning, Java orientation, callback spans and rebase remain
pending; dedicated k-opt runtime comparisons are not yet available.

Five original regressions check2586 independent permutation successors across
suffix sizes3..7 and pinned prefixes0..2, ranges, untouched/missing values and full
pins; every graph reconnection and iterator successor uses independent edge sets;
all-k bounds, invalid inputs, two-direction outputs, subtours and rollback; a130-value
list and explicit65-edge exchange; list scoped/stream plateau escape; mixed
joint/nonjoint scoped/stream with three orders/three acceptors/five budgets.
Existing gather basic-backend fallback test includes KOpt. Seven focused tests
passed. Initial fixture run found the expected native error was DuplicateEdge rather
than Disconnected; corrected the adaptation and recorded that substitution.

Full gate handle5091 completed exit0; fmt/Clippy passed.
Handle26906 completed work/kopt-verify.sh with exit0:12 focused release tests,
16 current datasets/83 timings and all audit invariants passed.
Verified gate totals2012Rust,106doctests,
235Python;931/4855(raw19.176/UI19.18%),Moves170/874,pending3642/excluded282.
Work scripts/logs: work/kopt-focused.log,-check.log,-verify.sh,-refresh.sh,-audit.py.
Full goal remains active, incomplete and unblocked.

Browser verified1440x900 and390x844:19.18%,931/4855,83 timing cards,
Moves170/874(19.45%). K-opt class has one passed and16 pending methods, retaining
non-credit fixtures accurately. No page errors, overflow, stale/undefined/NaN words.
Screenshots inspected: work/kopt-desktop.png and -mobile.png. Browser closed;
localhost:8765 remains running. Upstream unchanged.

Next substantive candidates: multi-entity k-opt behavior and public-Java quality/time
comparisons for k-opt or mixed/weighted groups. No k-opt runtime claim has been made.
The full rewrite remains active and incomplete.

## Cyclic k-opt quality/performance comparison (verified)

Added KOptBenchmark.java and native k_opt_benchmark example using existing routing
wire fixtures, one complete unpinned entity and cyclic Manhattan scoring (no depot).
Public Java KOptListMoveSelectorConfig min=max3/4 returned initial assignments with
one score call on8/16 values. Global versus entity-provided range probe had the same
result. Ignored diagnostic SLF4J jar/logs confirm no doable selected move and zero
steps; no production implementation inspected. Public min2/max3 or4 performs real
search including higher-k moves in diagnostic logs. Exact-bound observations remain
four separate probes without timing ratios; do not claim an upstream root cause.
ListSolverBenchmark.validate now delegates to an overload accepting an independent
score, retaining all previous structural/shadow checks for cyclic scoring reuse.

Both adapters configure30steps,10000score evaluations,seed20261008,first-accepted
hill climbing. Native rolling shuffle window4096 with exact-edge enumeration; test
proves first window reaches every configured k at8/12/16 values. Native anchors its
cycle and excludes retained removed edges/equivalent edge sets; Java sampling and
orientation differ. No identical work, trace or distribution claim. Full scoring,
per-solve setup and result/shadow validation timed; parsing/model/network/factory
creation, process startup and output encoding excluded. Three warmups,5samples,
3forks by default, alternating runtime order. Every output independently checked.

benchmark_kopt.py generates32 original8-value cases per k range (64total), including
coincident and collinear points, with exhaustive anchored-permutation optima. Four
larger workloads use12/16 values × ranges2–3/2–4. Six Python evidence tests check
cyclic metrics, IDs, score/quality/range validation, reproducibility and summaries.
New kopt_performance report dataset and dashboard section reuse existing cards;
quality differences and four exact-k probes remain visible. No coverage credit.

Pilot0warmups/1sample/1fork passed:64valid57equal,Java57/Rust64 optima;
2–3:27/32equal,2–4:30/32equal. Boundary callsJava1each,Rust2805/10000/8771/10000
(k3n8,k3n16,k4n8,k4n16). Cold timings are not final evidence.
Full gate31699 completed exit0:fmt/Clippy and all tests passed.
Handle51766 completed work/kopt-bench-verify.sh with exit0:release window test,
17 current datasets/87 timings and all audit invariants passed.
Verified2013Rust,106doctests,241Python;
coverage unchanged931/4855(19.18%). Full goal remains active and incomplete.

Final warmed measurements reproduce64valid57equal cases and Java57/Rust64 optima.
Four boundary probes again record Java1call each with unchanged initial routes;
native calls2805/10000/8771/10000. No speed ratios for these probes.
Four timing workloads,15measured samples per runtime each (120total):
- 2–3/12values: Rust28.134ms score-180 calls2145; Java9.737ms score-182 calls80.
- 2–3/16values: Rust79.179ms score-202 calls325; Java13.119ms score-272 calls124.
- 2–4/12values: Rust102.448ms score-180 calls3439; Java11.816ms score-194 calls130.
- 2–4/16values: Rust95.602ms score-190 calls1720; Java9.691ms score-238 calls115.
All hard scores0; Rust slower with better returned scores on these four cases.
Optima unknown for those sizes. Independent audit replays every saved assignment,
checks all small-case optima and recomputes counts, medians, ranges and ratio labels.

Browser verified1440x900 and390x844:931/4855(19.18%),87timing cards,4k-opt cards
all labeled quality differs,4separate exact-k probes. Methodology/environment present;
no errors, overflow (including expanded notes), stale/undefined/NaN words.
Screenshots inspected: work/kopt-bench-desktop.png and -mobile.png. Browser closed;
localhost:8765 remains running. Upstream unchanged. Logs work/kopt-bench-*.log.

Next substantive candidates: profile native k-opt generation/window costs and add
bounded seeded sampling if justified, then multi-entity k-opt repartitioning and
its public-API evidence. Preserve the current exact-k Java boundary evidence.
The full rewrite remains active and incomplete.

## Bounded seeded k-opt sampling (verified)

Profile in work/kopt-profile.rs/.log measures generation of the initial window:
12values/2–3:1196moves0.837ms;12/2–4:4096moves2.836ms;
16/2–3:3408moves2.749ms;16/2–4:4096moves3.295ms. Thirty window generations
approximately account for prior solve time, motivating direct on-demand proposals.
These are local diagnostic timings, not new Java/Rust benchmark ratios.

Added ListKOptSampling and sample_list_k_opt_moves plus Neighborhood::SampledKOpt.
Each bounded attempt chooses an eligible entity, possible clipped k, distinct cuts,
random path order/directions; retained cut edges reject the proposal. Every returned
move changes exactly selected k edges. Attempts bound proposals, not yield count;
repeats allowed; no uniform-final-tour or Java distribution/PRNG claim. Small C4/C5
impossible edge counts and ineligible entities return immediately. Anchors, pins,
entity ranges, payloads and missing values remain unchanged; each move affects one
list. O(n*k) edge checking, O(n²) relocation construction, linear scratch memory.
Shared cut_paths extraction preserves exhaustive enumeration. No production code read.

ListKOptSampling::for_step mixes the base seed with completed step using wrapping
arithmetic; step0 unchanged, each phase resets counters. List and mixed search cores
apply this to both scoring backends; ordinary neighborhoods retain old behavior.
Standalone iterators use the supplied seed. Move-order shuffling is independent;
sampled benchmark uses Original to avoid another window. Basic-only fallback stays
changes/swaps. Six original sampler tests check independent small-tour sets/exact-k
membership (all possible outputs observed for4..6values;7values checked by membership),
pins/ranges/missing identities, seed/attempt-prefix repeatability, empty/max-budget
boundaries,65edge draws on130values and32manual first-improvement/seed traces through
both list backends. Existing list/mixed plateau and basic-fallback tests expanded.
All18 focused tests passed; nine Python benchmark-evidence tests passed.

Benchmark retains window baseline and adds sampled strategy. Sampled proposal budget
per step equals configured evaluation cap (10000default); score/step limits unchanged.
New rollups reject empty/duplicate paired cases and enforce real candidate work and
score-call budgets.128paired comparisons over32routes/twokranges/twostrategies,
eight timings, four retained exact-k probes. No upstream-method credit.
Pilot0warmups/1sample/1fork:128valid114equal,Java114/Rust128optima counting paired
runs; each native strategy64/64optima versus Java57/64. Native sampled cold timings
roughly1–2ms versus window31–108ms. Compared with window, sampled quality is worse
on two larger workloads, equal on one and better on one; warmed results are below.

Full gate32736 completed exit0:fmt/Clippy,2019Rust,107doctests,244Python passed.
Handle78381 completed work/kopt-sample-verify.sh with exit0:19focused release tests,
17 current datasets/91 timing workloads and the independent audit passed.
Coverage unchanged931/4855(19.18%). Full goal remains active and incomplete.

Warmed k-opt results:128valid114equal,Java114/Rust128optima counting paired runs;
each native strategy64valid57equal,Java57/Rust64optima. Four exact-k probes retain
Java1scorecall each and unchanged routes. Eight timings,15samples/runtime:

- window2–3/n12 Rust27.568ms -180 calls2145;Java9.550ms -182 calls80.
- window2–3/n16 Rust80.030ms -202 calls325;Java14.511ms -272 calls124.
- window2–4/n12 Rust105.529ms -180 calls3439;Java10.933ms -194 calls130.
- window2–4/n16 Rust106.529ms -190 calls1720;Java12.582ms -238 calls115.
- sampled2–3/n12 Rust1.100ms -182 calls445;Java10.763ms -182 calls80.
- sampled2–3/n16 Rust0.635ms -212 calls239;Java12.308ms -272 calls124.
- sampled2–4/n12 Rust2.126ms -180 calls802;Java10.842ms -194 calls130.
- sampled2–4/n16 Rust1.204ms -186 calls386;Java12.110ms -238 calls115.

All hard scores are zero. Sampling reduces native generation time, but compared
with window search returns worse quality on two, equal on one and better on one.
Java quality is equal on one sampled workload,
worse on the other three; no equal-work or general-speed claim. The independent
audit replayed all assignments, checked small-case optima and recalculated medians,
ranges, counts and ratio labels from all 240 measured k-opt runtime samples.

Browser checks passed at1440x900 and390x844:931/4855(19.18%),91 cards,8 k-opt
cards,7 quality differences,4 separate exact-k probes and both strategy rollups.
Report and freshness HTTP responses200; no browser errors or horizontal overflow,
including expanded notes. Screenshots inspected:work/kopt-sample-desktop.png and
-mobile.png. Expanded notes exposed an existing missing CPU field in list-repair
evidence; the UI now renders missing environment fields as explicitly not recorded.
Updated app.js asset hash to avoid cached old rendering. No undefined/NaN values
remain in expanded notes. Browser closed;localhost:8765 remains running.
The dashboard guide now includes all17 refresh commands and the k-opt methodology.

Next substantial capability remains multi-entity k-opt. Four ordered upstream
fixtures were read from SelectorBasedKOptListMoveTest (testMultiEntity2Opt,
testMultiEntity3Opt,testMultiEntity3OptPinned,testMultiEntity4Opt). Their repartition
and notification behavior is not yet implemented or credited; do not assume simple
per-list cyclic reconnection establishes it. Upstream production remains unread
and the pinned submodule unchanged. No new provenance entries in this increment:
all sampler tests and benchmarks are original.

## Combined-cycle k-opt repartitioning (verified)

Previous goal turn made verified progress: commit75ec4d8 added bounded single-list
sampling, all gates passed and17 datasets/91 timing workloads were current.
Current increment adds list_k_opt_repartition.rs, explicit
ListProblem::k_opt_repartition_move and sample_list_k_opt_repartitions with
Neighborhood::RepartitionKOpt. Concatenate selected movable suffixes into one
cycle, reconnect using explicit combined offsets/start/successor, partition with supplied
suffix lengths. Empty destinations/zero lengths allowed; prefixes, unselected
lists, missing values and actual assigned payloads preserved. Target ranges checked
before ordinary Change composites are constructed. Atomic undo uses existing code.
Sampler combines all unpinned entities, chooses exact-k reconnections, a uniform
cycle rotation and uniform
weak-composition lengths, rejects retained cut edges/invalid receiving ranges;
proposal count bounded, repeats possible, no Java distribution claim. Per-step seeds
integrated with list/mixed cores. Basic-only fallback remains changes/swaps.

Extracted cycle_order and draw_tour from existing native implementations without
changing their order or random draws. Java production was not inspected. Source
specifications were pinned docs and SelectorBasedKOptListMoveTest/iterator tests.
Explicit combined-cycle edges differ from implicit Java per-list wrap boundaries;
full Java partition/orientation parity remains pending. Two output fixtures
(testMultiEntity2Opt,testMultiEntity4Opt) preserve initial/edges/result order with
explicit native lengths and successor, recorded in provenance and notices but earn
no method credit. Do not adapt the other fixtures by treating their local wrap
edges as global edges. Existing public Java timings remain single-route only.

Nine original tests cover28,070 small tour/partition states (every cycle rotation),
all sampled small
successors, selected entity order/pins/ranges/missing/unselected values, invalid
length sums/entities/source state, seeded prefixes,65cuts on130values, assigned
payloads, manual per-step seed traces, cross-owner list and coupled mixed search.
A focused test initially compared internal constraint work across mixed backends;
scoped and stream graphs do different work. Corrected assertions compare all
observable search outcomes/counters except backend-specific constraint evaluations.
The15 focused tests (9original,1fixture runner,5basic/gather) passed. Earlier
single-list k-opt focused tests also passed after helper extraction.
Initial gate71499 completed exit0:2029Rust,108doctests,244Python passed.
Review then found that a fixed combined-cycle anchor would unnecessarily exclude
valid multi-route partitions. Added explicit start/successor pairs and uniformly
sampled rotations; independent references now cover all permutations/partitions.
Stopped the confirmed running verification tree16090/session86121 with SIGTERM;
terminal exit143 observed, before editing measured sources. No duplicate workflow.
Expanded28 focused tests passed including unchanged single-list tests. Added invalid
start/successor bounds to the existing validation test. Full gate61978 completed exit0:
2029Rust,108doctests,244Python plus fmt/Clippy passed. Handle33930 completed
work/kopt-repartition-verify.sh with exit0:29focused release checks,17 current
datasets/91timings and the independent audit passed. Coverage remains931/4855.

Next comparison plan: public KOptBenchmark currently validates one full route but
its original score calculator already sums independent cyclic Manhattan distances
across vehicles. Extend a separate multi-route adapter/protocol deliberately,
checking complete ownership, pins, ranges, shadows and non-regression. Rust should
use RepartitionKOpt with full-cycle rotations. Use independent small route/partition
optima (brute partitions/permutations or a separately verified subset DP), retain
Java no-work/empty-route boundary observations, and label unequal final quality.
Do not conflate per-route score edges with the native combined-cycle exchange edges.
No general Java parity or multi-route speed claim is available yet.

Browser verification passed:931/4855(19.18%),91 timing cards, current report and
freshness responses200. Moves & selectors drill-down searched testMultiEntity;
all five matching methods remain pending, including the two new output fixtures.
No browser errors, undefined/NaN values or mobile horizontal overflow, including
expanded methodology. Browser closed;localhost:8765 remains running. No UI changes
in this increment. Upstream pin unchanged and no production implementation read.
Single-route k-opt quality and score-call outcomes unchanged after shared helper
extraction:128paired/114equal,each strategy64optima versusJava57. Warmed sampled
medians0.963/0.629/2.004/1.193ms (2–3/12,2–3/16,2–4/12,2–4/16). These are
single-route measurements; multi-entity k-opt comparison remains the next task.
Full rewrite goal remains active and incomplete.

## Selective verification cache

check.sh now reuses successful per-target Rust, per-file Python, fmt, Clippy and
Doctest jobs. All17 measurement commands have separate input keys and direct-command
reuse; --force bypasses caching. Native dependencies use an intentionally selective
module/symbol scan, not compiler-proven dependencies. Use --force for final-phase
validation. See docs/dashboard.md. No Rust solver behavior changed.
Initial Rust baseline:2029 passed; unchanged follow-up:284 targets reused, zero
commands executed (39.31s hashing/log assembly). Python suite251 passed, followed by
8 focused cache checks including one added module-isolation case (252 distinct tests).
Old global-key benchmark reports remain stale; no benchmark sweep was performed.

## Solve-wide composite termination

Added borrowed TerminationExpression<C> (All/Any/Condition), exact decisions and
optional progress estimates, with scope applicability supplied by a native evaluator.
SearchTermination composes steps, elapsed time and both stagnation limits through
SolverLimits::termination. Shared counters survive local/repair phase handoffs;
only the complete expression latches, so an unfinished AND permits stagnation resets.
Polled within local/repair/exact/custom phases and at construction boundaries.
Construction does not yet poll composite conditions inside its candidate loop;
score/evaluation leaves are not yet part of this native solve-wide expression.
Twelve complete And/OrCompositeTerminationTest adaptations and the abstract
applicability helper are attributed. Six original integration tests cover all six
solve paths, repair stopping, initialization, nested progress and full-width counts;
two controlled-clock cases cover resets and exact/bounded/custom stopping.
Exact exhaustion still takes optimality precedence when a condition coincides.
check.sh passed:2049 Rust tests,109 doctests,252 Python checks,fmt/Clippy.
Coverage943/4855(19.423%); failed/unverified0. Dashboard data/freshness matched.
Benchmarks remain stale; no benchmark sweep or browser redesign. A single batched
--no-run Cargo build avoided sequential recompilation during the gate. Full goal
remains active; extend substantial missing solver behavior next.

## Composite limits inside construction

Construction now polls solve-wide expressions between candidates in all six
model/scoring paths, including observed entry points. Interrupted scans retain
the best evaluated prefix and report ConstructionTermination::Condition. Polling
does not advance search steps or start/reset stagnation tracking. Standalone
construction signatures and independent hard-limit precedence are preserved.
Original deterministic regressions compare all three placement modes, both mixed
orders and eight deadlines across all six backends against candidate-budget
prefixes. An actual SolveControl fixture checks interrupted domain scans and
suppression of later phases. No new upstream method credit for these regressions.
check.sh passed:2051 Rust tests,109 doctests,fmt/Clippy; all46 Python
file jobs reused their successful results (252 tests). Rust reused73 targets.
Coverage remains943/4855(19.423%),failed/unverified0; live dashboard freshness
matched. Benchmarks remain stale. Full rewrite goal remains active.

## Stream match justifications and score explanations

Added on-demand typed graph explanations with default ordered fact justifications,
custom justifications and separately selected blamed facts. Exact retained scores
are read without rescoring. Per-fact contributions deduplicate repeated blame
within a match while preserving equal tuple occurrences. Cross-constraint totals
qualify match identities by constraint name. into_indexed supports mixing scalar,
joined and other tuple IDs without dropping the original identity mapping.
Snapshots remain independent across commits/rollback; empty and zero-score matches
are retained. Basic/list/mixed stream directors expose these through graph().
Standalone scoped scoring explanations and automatic heterogeneous graph-wide
justification registration remain unimplemented; aggregates cover supplied reports.
Twenty-three uni-stream scoring/justification methods and four fixture/helper
records were adapted with exact provenance. Four original regression tests cover
cross-constraint blame, duplicate rejection, decimal precision, rollback, callback
panic, foreign handles, empty results and heterogeneous-ID aggregation.
The gate exposed a pre-existing live cancellation race: waking the live worker can
let it leave the pool before the pool-level cancel call, causing a successful live
request to report false. Live cancellation now retains the first accepted request's
success independently of that pool race. Two deterministic unit regressions cover
worker completion before pool cancellation and competing callers; stale completed
handles still return false.
Final check.sh passed:2080 Rust tests,110 doctests,fmt/Clippy; Python
checks reused. The cancellation follow-up rebuilt/reran only5 affected targets;
283 other Rust targets reused matching successful results. Coverage966/4855
(19.897%),failed/unverified0. Live dashboard data and freshness matched.
Benchmarks remain stale; no timing sweep. Full rewrite goal remains active.

## Weighted stream constraints and zero-branch pruning

Added score_weighted with exact signed integer match scaling, sorted explicit
ConstraintWeightOverrides and build_with_weights. Defaults are retained unless
replaced; explicit zero disables the terminal and exclusively feeding branches
before source initialization, while shared active branches continue. Disabled
constraint handles remain inspectable. Unknown names, unweighted overrides and
structural weights are typed errors. Rebuild per weight configuration; no dynamic
in-place graph reweighting. Fractional decimal weights work; fractional match
factors and standalone scoped-network overrides remain unsupported.
Five complete upstream methods adapted with provider/solution/entity fixtures and
attribution. Six original regressions cover disabled callbacks, sharing, dead joins,
source retention, override activation/freeze, rejection, exact integer/decimal
scaling, rollback/explanations and optimum changes in basic/list/mixed stream solves.
check.sh passed:2091 Rust tests,111 doctests,fmt/Clippy. Python jobs
reused matching successful results. Coverage971/4855(20.000%),failed/unverified0.
Live dashboard data/freshness matched. Benchmark datasets remain stale; no sweep.
Full rewrite goal remains active.


## Exact decimal match weights and selective facade inputs

Added Decimal::multiply_exact and DecimalWeightedScore for all four native decimal
score families; score_weighted_decimal accepts exact fractional factors. Products
retain full precision instead of flooring at the weight scale. Integer/decimal
weighted terminals share registration, overrides and zero-branch pruning. Scale
sum overflow is a typed GraphError with the underlying cause; failed transactions
remain poisoned until dropped, restoring all earlier routed changes.
Sixteen decimal ScoreInliner methods adapted with all score/undo assertions and
helper provenance. Six original regressions cover fractional products, representation,
overflow, cross-node rollback, explanations, overrides/pruning and optimal solves.
Verification keys now project lib.rs onto referenced modules/reexports while retaining
crate attributes and other root code. Unrelated public exports no longer invalidate
all targets. Three focused cache tests cover unrelated additions, alias retargeting,
and root code/attributes; this remains a development heuristic, not a Rust parser.
The dashboard server was restarted with the updated Python input logic.
check.sh passed:2113 Rust tests,112 doctests,255 Python checks,fmt/Clippy.
Coverage987/4855(20.33%),pending3586,excluded282,failed/unverified0.
Benchmark datasets remain stale; no timing sweep. Full rewrite goal remains active.


## Configurable scoped weights

All three scoped networks now support signed integer/exact decimal factor callbacks
and atomic named overrides. Missing override names retain current weights; unknown
names, direct-score overrides and structural weights are rejected before mutation.
Directors borrow their network, preventing in-session changes. Zero weights skip
callbacks and fact assembly and are removed from candidate/exact callback work counts.
Declared dependencies survive disabling, supporting reactivation between sessions.
Typed decimal errors and panic unwinding preserve model/cache rollback.
Sixteen non-overflow integer ScoreInliner methods adapted with helper provenance;
eight integer overflow methods remain pending because native arithmetic wraps.
Four original regressions exercise basic/list/mixed registration/configuration,
pruning/reactivation, errors/panics, exact and construction/local-search optima.
Full rewrite goal remains active.
check.sh passed:2133 Rust tests,113 doctests,fmt/Clippy;255 Python checks
reused.217 Rust targets ran and77 reused matching successful evidence.
Coverage1003/4855(20.659%),pending3570,excluded282,failed/unverified0.
Benchmark datasets remain stale; no timing sweep.


## Scoped score explanations and snapshot comparison

All three scoped directors now expose explain/explain_default over cached weighted
contributions and current scoped facts; scoring callbacks are not invoked. Each
active scope is one aggregate match, disabled weights have none. Custom/default
justifications and deduplicated per-fact contributions use existing explanation types.
ScopedScoreExplanation separates full structural score from ordinary matches.score.
ScoreExplanation::diff compares current against baseline, retaining only changed
constraint/indictment before/after snapshots and exact score deltas. Zero-delta
metadata/identity changes are retained; additions/removals are explicit Options.
IDs must be aligned across compared reports. One upstream ConstraintMatchTotal method
adapted; seven original regressions cover scoped snapshots and comparisons.
Full rewrite goal remains active.
check.sh passed:2141 Rust tests,114 doctests,fmt/Clippy;255 Python checks
reused.244 Rust targets ran and53 reused matching successful evidence.
Coverage1004/4855(20.680%),pending3569,excluded282,failed/unverified0.
Benchmark datasets remain stale; no timing sweep.


## Checked integer scoring

ScoringArithmetic defaults to Checked in graph/scoped scoring. All built-in integer
scores check impacts and ordinary final totals using new AdditiveScore hooks. Score
operators remain wrapping; with_arithmetic(Wrapping) selects legacy modular scoring.
Custom bounded score types must override the hooks; default hooks retain operators.
Wide i128 sums/batched adjustments avoid false intermediate-i64 overflow. Graph totals
check complete routed updates; each terminal match change remains checked. Diagnostic
replay uses modular accumulation but checked impacts and compares every retained
match, avoiding replay-order failures and rejecting modularly colliding bad impacts.
Typed ScoreOverflow propagates through all six solver paths; failed graph/scoped
moves preserve rollback. Eight upstream methods and two helper records adapted;
seven original regressions cover boundaries, rollback, replay and solving errors.
Five existing modular-scoring regressions now explicitly select Wrapping. Full rewrite
goal remains active; no benchmark or release-mode rerun.
Final check.sh passed:2156 Rust tests,115 doctests,255 Python checks,fmt/Clippy.
Rust source digest:f71c8513b92474e0f7529b98f6c0ae3f5150513260dd64a4218b1692e9de3dd1.
The broad run exposed one additional legacy snapshot test that expected wrapping;
its builder now selects Wrapping. The follow-up reran that one Rust target and reused
298 matching successful targets. Coverage1012/4855(20.844%),pending3561,
excluded282,failed/unverified0. Benchmark datasets remain stale.

## Weighted neighborhood unions

LocalSearchConfig.neighborhood_union borrows a NeighborhoodUnion and replaces the
single neighborhood when set. Positive u32 child weights support sequential or
seeded weighted random traversal; zero weights disable validation and enumeration.
Swap-only selection lets change/swap families receive independent weights. All
three models and both scoring backends share the selector, including solver-phase
prevalidation. Per-step seeds wrap-add the completed step count, independently of
existing move/tie ordering. Mixed joint moves remain within each selected child.
MoveUnion is also a public generic lazy iterator; it never polls at construction,
removes exhausted children, retains duplicates, and fuses exhaustion. Integer u128
sampling avoids floating-point weight rounding. The ordinary single-family path
still directly consumes its iterator without allocating a union.
Two upstream UniformRandomUnionMoveIterator methods and three helper substitutions
are recorded; hasNextOnlyPollsTheDrawnIterator remains pending because native Rust
has no separate polling/priming protocol. Five original regressions cover weighted
selection, laziness, disabled validation, all six solver paths and a local trap
requiring both group and single moves. Full rewrite goal remains active.
Final check.sh passed:2163 Rust tests,116 doctests,fmt/Clippy;255 Python checks
reused.222 affected Rust targets ran and79 reused matching successful evidence.
Rust source digest:bf7055038dd9cedaf56e3c0e371ce004e6e305505e5d31129406be0fda793a2a.
Coverage1014/4855(20.886%),pending3559,excluded282,failed/unverified0.
Live dashboard freshness matches the report. Benchmark datasets remain stale;
no benchmark or release-mode rerun. Logs:work/move-union-check.log,
work/move-union-test-build.log,work/move-union-lint-final.log.

## Independent-destination change products

Neighborhood::ChangeProduct(SubPillarSelection) and change_product_moves enumerate
subsets of eligible basic variables with a separate legal destination per member.
Each selected slot changes. Pins/no-alternative slots are excluded; optional null
destinations come last; required null starts can be assigned but never cleared.
Consecutive selection follows eligible variable order, including search shuffling.
Lists retain change/swap behavior; mixed joint moves combine a basic product with
one list move. Existing union composition, budgets and both scoring backends work.
Public MoveProduct lazily builds Cartesian products from stable restartable child
factories without caching all choices. Last child varies fastest; empty children
are rejected or omitted, and zero/all-empty children yield no moves. Infinite
suffixes starve preceding children; consumers need a budget. No Java mimic/lifecycle
or random-selector equivalence is claimed. Four attributed helper fixtures preserve
ordered/empty enumeration only, with no upstream-method credit. Six original tests
cover independent state-space references, laziness, immutable/required slots,
weighted union and four basic/mixed solver paths. Full rewrite goal remains active.
Final check.sh passed:2170 Rust tests,118 doctests,fmt/Clippy;255 Python checks
reused.225 affected Rust targets ran and79 reused matching successful evidence.
Rust source digest:53c25e70e401288827f84719b748f1f24ddbad9c53b65ae52f505eb4f4ac4b39.
Coverage stays1014/4855(20.886%),pending3559,excluded282,failed/unverified0.
No benchmark or release-mode rerun. Logs:work/change-product-check.log,
work/change-product-test-build.log,work/change-product-focused-final.log.

## Custom move providers

MoveProvider<P,A,M>::moves borrows a model/current committed snapshot and receives
the completed-step count. Six *_local_search_from_with_provider entry points use
the shared basic/list/mixed engines with scoped or stream scoring. Vec-returning
closures have a blanket adapter; trait implementations can return lazy borrowed
iterators. Built-in neighborhood/union configuration and mixed joint generation
are replaced; irrelevant built-in validation/order arrays are skipped. Existing
acceptance, tabu, foraging, candidate shuffling, scoring, rollback and stopping rules
remain in effect. Providers are cooperative; candidate-only budget probing pulls
one additional unevaluated item and shuffling can prefetch a bounded window.
The provider callback itself must return promptly. Invalid moves propagate existing
model/scoring errors. Source/model validation runs before a zero-budget early exit.
Two upstream custom-provider methods and TestdataSolution.generateSolution helper
are attributed. Four original tests cover all six paths, stateful snapshots,
infinite iterators, limits/prefetch/cancellation, prevalidation and invalid moves.
The subsequent integration below adds typed providers to SolverSequence phase plans.
Full rewrite goal active.
Final check.sh passed:2176 Rust tests,119 doctests,fmt/Clippy;255 Python checks
reused.227 affected Rust targets ran and80 reused matching successful evidence.
Rust source digest:94287a578742a57a41070d41f326fc25da7d375a7c7d7ce6ab0c76a03ecdb445.
Coverage1016/4855(20.927%),pending3557,excluded282,failed/unverified0.
Live dashboard freshness matches the report; benchmark datasets remain stale.
No benchmark or release-mode rerun. Logs:work/move-provider-check.log,
work/move-provider-test-build.log,work/move-provider-lint-final.log.

## Move providers in complete solver plans

SolverExtensions combines PhaseMoveProviders bindings, custom commands and events
for all six solve*_with_extensions entry points. MixedSolverExtensions is a public
convenience alias. Bindings address zero-based full-plan local-phase indices;
missing/non-local targets fail before graph factories, scoring or callbacks. Only
bound phases skip built-in neighborhood validation. Provider searches reuse plan
monitors and clipped limits, retained-best handoffs, initialization and best events.
The generic MoveProvider trait is now separate from typed provider_search entry
points so dependency fingerprints need not pull every scoring backend via the trait.
Six original integration tests exercise construction, multiple bound local phases,
a worsening custom command, built-in search, event/no-event paths, shared move and
composed step limits, event cancellation and early binding/neighborhood validation.
A move-budget boundary may instantiate one extra provider iterator for an unscored
probe, matching the existing candidate engine. No upstream method credit added.
Full rewrite goal remains active.
Final check.sh passed:2182 Rust tests,120 doctests,fmt/Clippy;46 Python jobs reused.
228 affected Rust targets ran and80 reused matching successful evidence.
Rust source digest:02447be125f83fdc5b6a427b6fc11dd38d1fa34915e6b332ce74ad620f6b29eb.
Coverage1016/4855(20.927%),pending3557,excluded282,failed/unverified0.
Live dashboard freshness matches the report. Benchmark datasets remain stale;
no benchmark, release-mode or browser rerun. Logs:work/provider-plans-check.log,
work/provider-plans-test-build.log,work/provider-plans-lint-final.log,
work/provider-plans-focused-final.log. Broad test compilation took5m56s.

## Configurable sublist neighborhoods

Added SubListSize, SubListChangeConfig and SubListSwapConfig with lazy typed APIs
and SubListChange/SubListSwap/SubListUnassign neighborhood variants. Relocation and
unassignment require minimum two; swaps allow singleton sides and independent
bounds. Reversal is configurable; singleton/singleton swaps never reverse. Default
unassignment follows the model; explicit true on required lists is rejected, as
is standalone sublist unassignment on required lists. Default native maxima are
unbounded, with bounds clamped to actual lists. No implicit sampling cap is claimed.
Asymmetric swaps retain both physical placements and deduplicate symmetric pairs
only when both orientations satisfy the bounds. Existing max-only APIs unchanged.
Twenty-one full provider-method adaptations are attributed; default-maximum remains
pending. Four original regressions cover independent state spaces, pins/ranges,
rollback, all four list/mixed search backends and early validation at zero budget.
The full rewrite goal remains active.
Final check.sh passed:2207 Rust tests,121 doctests,fmt/Clippy;46 Python jobs reused.
230 affected Rust targets ran and80 reused matching successful evidence.
Rust source digest:173c648a562bae19e1850a991857c73461bc00fb861071768add1b8696520658.
Coverage1037/4855(21.359%),pending3536,excluded282,failed/unverified0.
Sublist provider families:change9/10,swap7/7,unassign5/5. Live dashboard freshness
matches the report. Benchmark datasets remain stale; no benchmark, release-mode or
browser rerun. Logs:work/sublist-controls-check.log,
work/sublist-controls-test-build.log,work/sublist-controls-focused.log.
Broad test compilation took10m21s. The initial lint clone-on-Copy warning was fixed;
final check.sh includes the successful Clippy run.

## Explicit assignment and whole-pillar policies

Added assign_moves/unassign_moves and Assign/Unassign neighborhoods for optional
basic slots, plus PillarChangeConfig, pillar_change_moves/pillar_unassign_moves and
matching neighborhood variants. Whole-pillar changes require two members; unassignment
allows singleton groups. Default null crossing follows optionality, false excludes
null sources/destinations, and explicit true requires all model slots optional.
Pillar changes reject groups containing missing required selected slots. Schema
rows retain tuple grouping and one-column writes; selected pins exclude whole rows.
Shared schema enumeration is reused, with non-null domain scans disabled for
unassignment. Basic-only families retain ordinary list neighborhoods in mixed search.
Twenty-three full provider-method adaptations and four fixture helpers are attributed.
Four original tests check successor sets/rollback, schema member counts/columns/pins,
basic/mixed scoped/stream solving (including joint candidates), and zero-budget early
validation. Existing general pillar and mass move contracts are unchanged.
Full rewrite goal remains active.
Final check.sh passed:2234 Rust tests,122 doctests,fmt/Clippy;46 Python jobs reused.
232 affected Rust targets ran and80 reused matching successful evidence.
Rust source digest:159fc372da1994a45529adef983c0fddd705d484ab2e9dc6bb887193159895cd.
Coverage1060/4855(21.833%),pending3513,excluded282,failed/unverified0.
Assign4/4,Unassign5/5,PillarChange9/9,PillarUnassign5/5. Live dashboard freshness
matches the report. Benchmark datasets remain stale; no benchmark/release/browser
rerun. Logs:work/assignment-controls-check-final.log,
work/assignment-controls-test-build.log,work/assignment-controls-focused.log,
work/assignment-controls-regression.log. Broad compilation took6m27s.
The first gate exposed an existing observer timing test mixing a simulated solve
clock with a real 5ms construction timer. Both matching fixtures now inject the
same controlled clock into construction, preserving all assertions. The final
gate passed. That test-only edit still invalidated232 integration/lib/example job
keys; test-module dependency isolation remains an opportunity for selective reuse.

## Isolate unit-test verification inputs

Integration/example and measurement library inputs now project away exact cfg(test)
items and exclusively unit-test modules. Targets retain their own test-only code;
shared runtime support, literal contents, and unknown conditional attributes remain
inputs. Doctest keys omit unit fixtures while retaining documentation. This remains
a selective development heuristic, not Rust compiler dependency analysis.
A real dependency audit found solver_observation_tests.rs now affects only the lib
unit target out of312 targets; the prior test-only fix invalidated232 jobs.
Final check.sh passed:2234 Rust tests,122 doctests,260 Python tests,fmt/Clippy.
The key-format baseline executed312 Rust targets and46 Python jobs once.
Source digest:a652920dc03d9805191a7abb88ac0a5cba2b1c1b97b8cae90bd65180fd367939.
Coverage unchanged at1060/4855(21.833%). No benchmark/release/browser rerun.
Logs:work/unit-isolation-check.log,work/unit-isolation-audit.log.
The full rewrite remains active; reusable neighborhood samplers are the next batch.

## Reusable neighborhood samplers

Added NeighborhoodSample, SampleRandom, SizeSampler, Sampler/KeyedSampler,
PillarSampler and ordinary/keyed assembly. Fixed/bounded/draining sizes share a
nonempty minimum contract; custom rules accept, reject, accept-and-stop or stop
without accepting. Reset precedes validation; explicit stops below the floor error,
while source exhaustion below it returns None. Target sizes are hints, never eager
allocation sizes. Samples own distinct members, iterate in encounter order and
compare/hash without order. Bounded random sizes advance a caller-owned seeded RNG.
Sources supply member order and group membership; no Java PRNG or uniform-member
selection claim. Accepted duplicates skip rule evaluation. Infinite rejecting or
draining sources need an explicit caller bound.
All24 SamplersTest and9 SampleTest methods are attributed, plus2 helper fixtures.
The four decision variants were checked from pinned artifact public enum signatures
only; no production bytecode or source implementation was inspected. Mutable-alias
wrap methods remain pending. Six original regressions cover boundary sizes, exact
consumption, keyed/dynamic rule contracts, reproducibility, stop-only behavior and
sampled composite move providers with both scoped and stream solver scoring.
The cache change is committed as03f716b. The full rewrite goal remains active.
Final check.sh passed:2273 Rust tests,123 doctests,fmt/Clippy;46 Python jobs reused.
Only3 Rust targets ran (lib and two sampler suites),311 reused unchanged results.
Affected executable compilation took30s. Source digest:
d1f74d700ccafa10619ba9271c30dfac14ea4378f07a0e0188cacea91554653c.
Coverage1093/4855(22.513%),pending3480,excluded282,failed/unverified0.
SamplersTest24/24;SampleTest9/12. Live dashboard freshness matches the report.
The dashboard process was restarted to load the changed verification-input code;
backend data/freshness inspected, no UI changes or browser rerun. Saved benchmark
measurements remain stale; no benchmark sweep or release-mode rerun.
Logs:work/neighborhood-sample-check.log,work/neighborhood-sample-build.log,
work/neighborhood-sample-tests.log. Dashboard server handle46628 remains live.

## Neighborhood datasets and repeated sampling

Added lazy repeated/keyed sampling, immutable NeighborhoodDataset snapshots,
identity-indexed stores accepting TupleChange batches, seeded permutations/random
row draws, filtered joins and indexed CachedNeighborhoodJoin slices. Missing samples
or one yielded sampler error fuse repeated iteration. Source factories reopen for
each attempt; filtered slices scan exhaustively as needed. Stores cache unchanged
snapshots; held rows/samples and owned sample iterators survive updates. Batch changes
and explicit join-cache refresh are atomic on ordinary panics. Source facts must be
immutable; updates require explicit propagation/refresh, not reflective Java sessions.
23 complete upstream methods (SamplingIterator13,Dataset6,PillarDataset4) and6 fixture
helpers are attributed. The unbounded Java iterator consumer-exception method remains
pending. Six original tests cover consumption/seed behavior, atomicity, duplicate/null
rows, rare slices and lazy sampled providers in both solver scoring backends.
Final check.sh passed:2302 Rust tests,125 doctests,fmt/Clippy;46 Python jobs reused.
Three Rust targets ran,313 reused unchanged evidence. Compilation took23.86s.
Source digest:22f50a05d46d8201c660cbe29a9108b98cee2f15247bfc983e874d459119955b.
Coverage1116/4855(22.987%),pending3457,excluded282,failed/unverified0. Live dashboard
freshness matches. No UI/browser, benchmark sweep or release-mode rerun; older
benchmark measurements remain stale. Logs:work/neighborhood-dataset-check.log,
work/neighborhood-dataset-build.log,work/neighborhood-dataset-tests-final.log.
Initial focused compilation exposed test-only string dereference/result-access
mistakes; those were fixed before the final successful check. Goal remains active.

## Incremental stream dataset capture

Added typed StreamDataset handles and builder.dataset capture, shared per input port.
Captures use the graph journal for commit/undo/saved candidates; disabled score
pruning preserves live dataset branches. Consistency rebuilds compare dataset payloads
as well as scores. Persistent dataset_rows snapshots avoid materialization;
dataset_snapshot explicitly clones values for neighborhood sampling. Source facts and
payload equality must be immutable/stable/reflexive. Basic/list/mixed stream directors
expose committed and trial captures through graph(); MoveProvider inputs are unchanged.
13 full upstream methods (UniEnumerating8,BiEnumerating5) plus11 fixture helpers are
attributed. The12 uni source-discovery/null/pin methods remain pending. Seven original
checks cover transaction errors/panics, snapshots, pruning, rebuild diagnostics,
saved candidate installation and all three stream score-director model families.
Final check.sh passed:2322 Rust tests,126 doctests,fmt/Clippy;46 Python jobs reused.
264 affected Rust targets ran,54 reused unchanged evidence. Source digest:
f76a90dad53baf6629f740717366a354fb666c78ea6325ef7af7f46ae31dcf40.
Coverage1129/4855(23.254%),pending3444,excluded282,failed/unverified0. Live dashboard
freshness matches. No benchmark/release/browser rerun; older measurements stay stale.
Logs:work/stream-dataset-check.log,work/stream-dataset-build.log,
work/stream-dataset-tests-final.log. Full rewrite goal remains active.
Broad compilation took17m57s, dominated by tests/solver.rs. An isolated symbols-off
build (same source, CARGO_PROFILE_TEST_DEBUG=0, separate work/no-debug-target) passed
compilation in6m15s. Cargo invoked.timestamp to executable mtime measured the solver
target itself at730.78s with debug symbols versus328.05s without; executable sizes
1916.8MiB versus266.1MiB. These were not controlled identical build conditions, but
suggest a useful development build improvement. No profile configuration was changed.
Experiment log:work/solver-no-debug-build.log; both builds and final checks finished.
Dependency selection also rebuilds the same symbol-owner index for each target;
content-keyed reuse of that index could reduce check overhead without broadening
result reuse. Avoid repeating these measurements merely on a new turn.

## Reduce build and dependency-scan overhead

Development/test profiles now disable debug symbols while retaining debug assertions
and overflow checks. Set CARGO_PROFILE_DEV_DEBUG=2 or CARGO_PROFILE_TEST_DEBUG=2 when
symbols are needed. The measured symbols-off experiment above supports this choice;
release configuration is unchanged. Dependency owner indexes, dependency-word scans
and module/include parsing now reuse immutable results keyed by actual source text.
Files and module existence are still read on each scan; no timestamp/commit shortcut
or old-result relabeling was introduced. The existing selective dependency heuristic
is unchanged. Nineteen focused cache tests passed, including new same-mtime edits,
module addition/removal, out-of-line implementation and changed-call regressions.
Dependency selections match the previous code for solver, stream-dataset regression
and score-benchmark targets. One 318-target scan took53.03s alongside compilation
(previous observed scans were roughly3min, not a controlled benchmark). The owner
index was built once and reused319 times; word scans had31,464 hits and479 misses.
Logs:work/verification-parsing-tests.log,work/verification-parsing-timing.log.
The fresh build of all targets completed in6m04s; the solver target itself took
239.63s and produced a266.1MiB executable. This differs in build conditions from
the earlier17m57s broad build, so it is an observation, not a controlled speedup.
Final check.sh passed:2322 Rust tests,126 doctests,263 Python tests,fmt/Clippy.
All318 Rust targets and46 Python jobs ran for the changed profile/cache baseline.
Source digest:61bc4df1ba10cecf16bf339d35c909cfc59696596fb8772d55bcf15d0e0e2898.
Coverage remains1129/4855(23.254%),pending3444,excluded282,failed/unverified0.
Live dashboard freshness matches the report; server handle75340 loaded the new
verification code. No benchmark sweep, release-mode or browser rerun. Older
measurements remain stale. Logs:work/verification-speed-build.log and
work/verification-speed-check.log. Both compilation/check and timing sessions are
complete; do not restart them. The full rewrite goal remains active. Resume solver
implementation, reusing this successful baseline for unchanged inputs.

## Typed planning dataset enumeration

Added EnumeratedDataset captures with DatasetEnumerationOptions, borrowed ordered
iteration and owned sampling/join snapshots. A permanent synthetic null member is
separate from unassigned row payloads and is not inserted into the scoring graph.
Pin filtering runs on delivered tuple updates; reads do not rerun predicates.
basic_entity_dataset, list_entity_dataset and list_value_dataset bind fixed model
pins to score-director sources. Whole-entity list pins remove entities/values;
prefix pins remove only the corresponding values. Unassigned entities/registered
values remain included. Custom streams support explicit insertion/retraction/pin
payload updates; native model membership/pin metadata stays fixed during sessions.
All12 remaining UniEnumeratingStreamTest enumeration methods are adapted, with9
fixture/helper attributions. Native ascending IDs replace Java object identity/order;
reflection and arbitrary dynamic model mutation are not claimed. Six original
regressions cover null/duplicate distinctions, foreign handles, callback-free reads,
pin transitions/panics, sampling and basic/list/mixed transactional moves. Every
generated single list move (including optional unassignments/swaps) is compared with
independent assignment-derived positions/neighbors and checked after rollback.
Final check.sh passed:2340 Rust tests,127 doctests,fmt/Clippy;46 Python jobs reused.
Only3 Rust targets ran (lib and two new suites),317 reused unchanged evidence.
Affected executable compilation took19.26s. Source digest:
ebbb149956507a4f77455335656ad3e370ec614994b3c5805367ce3a8b945286.
Coverage1141/4855(23.502%),pending3432,excluded282,failed/unverified0. Live dashboard
freshness matches the report. No UI/browser, benchmark sweep or release rerun;
older measurements remain stale. Logs:work/dataset-enumeration-build.log,
work/dataset-enumeration-check.log,work/dataset-enumeration-focused.log and
work/dataset-enumeration-regression.log. The initial original-regression compile
needed its list_moves neighborhood/result arguments corrected; the final check
includes the completed ChangeAndSwap optional-list variant. All jobs are complete.
The full rewrite goal remains active.

## Bounded probes and retiring pair sampling

Added BoundedFilter with distinct Found/BudgetExhausted/SourceExhausted outcomes;
retry gets a fresh budget, successful probes have no lookahead, and true source
exhaustion fuses even non-fused inputs. RetiringRandom owns physical optional slots
and a live-index pool, draws occurrences with replacement, and retires only the
latest unretired draw. Borrowed draws need no Clone; owned iteration clones on
demand. Duplicate/null payloads remain separate from holes. RetiringPairs chooses
a live left, invokes a caller-supplied right probe up to a nonzero attempt limit,
and permanently retires repeatedly unproductive lefts. Productive pools require
bounded consumers; callbacks must return promptly. Bounded probes may miss rare
matches; callers can exhaustively scan the right side instead. Selection is fair
over live left occurrences, not over all matching pairs. Reopen on new snapshots.
16 upstream methods are adapted (DefaultRetiringRandomIterator7,FilteringIterator4,
RetiringBiWalk2,BiRandomMoveIteratorLiveness3), with6 fixture/helper attributions.
Explicit native attempt count8 substitutes for the symbolic upstream retry constant;
its Java value/internal storage thresholds and PRNG traces are not claimed. Six
original regressions cover bounded consumption/retirement, duplicate/null slots,
demand-driven cloning, deterministic distributions, exact dead-pool call counts and
pair-provider solving in scoped/stream local search. Focused cases/regressions passed.
Final check.sh passed:2362 Rust tests,128 doctests,fmt/Clippy;46 Python jobs reused.
Only3 Rust targets ran (lib and two new suites),319 reused unchanged evidence.
Affected executable prebuild took21.39s. Source digest:
8e0cea2a16ddc89b673d0f726a53062117ce5be7194d0727b8e0bef6ff34fe09.
Coverage1157/4855(23.831%),pending3416,excluded282,failed/unverified0. Live dashboard
freshness matches. No UI/browser, benchmark sweep or release rerun; old measurements
remain stale. Logs:work/retiring-sampling-build.log,work/retiring-sampling-check.log,
work/retiring-sampling-focused.log,work/retiring-sampling-regression.log. All build,
check and freshness jobs are complete. Full rewrite goal remains active.

## List destinations and custom provider unions

Added list_destinations (lazy immutable-assignment slots) and live
list_destination_dataset. Whole pins exclude entities; prefix pins retain the
boundary/end slots. Optional lists can include exactly one unassignment member.
Slots use pre-removal indices and are independent of receiving value ranges;
providers must adjust/check their own concrete moves. Live views follow journaled
list/mixed source updates and preserve held snapshots through rollback.
ProviderUnion borrows weighted providers, defers each factory until first selection,
skips zero weights entirely, and reopens children with each committed step snapshot.
Sequential/random ordering, wrapping step seeds, nested unions and duplicates work
through the existing six provider-enabled backends. Empty child lists are rejected;
all-disabled lists are valid empty neighborhoods. Infinite children need consumer
budgets. Five upstream methods and five helpers are attributed. Six original tests
cover dynamic destination/reference comparisons, validation, lazy factories/cursor
drop, disabled/nested children, weights/seeds and all six solver paths.
Final check.sh passed:2373 Rust tests,129 doctests,fmt/Clippy;46 Python jobs reused.
Three Rust targets ran (lib and two new suites);321 reused unchanged results.
Affected prebuild took13.90s. Source digest:
0c2fc51dc810d1b1e5d5d16b59cfc3206c5f392615a061bc34f17f5f9ae21dd9.
Coverage1162/4855(23.934%),pending3411,excluded282,failed/unverified0. Live dashboard
freshness matches. No UI/browser, benchmark sweep or release rerun; older measurements
remain stale. Logs:work/destination-union-build.log,work/destination-union-check.log,
work/destination-union-focused.log,work/destination-union-regression.log. All jobs
completed. Full rewrite remains active.

## Value-label exchange neighborhoods

Added value_exchange_moves and value_exchange_moves_in_order with bounded
contiguous or arbitrary subset selection, unused domain labels and explicit optional
unassignment. Custom orders are checked for duplicate/out-of-range IDs. Selection
bounds include pinned/unchanged slots; touched pins and incompatible receiving
domains reject the whole candidate. Empty/no-op exchanges are omitted, singleton
writes are Change, and multiwrites are Composite. Iterators clone only yielded
writes and integrate through existing custom providers without shared engine edits.
Three preview example methods are adapted: code-range value exchanges, restricted
block relocation and common-cut tail swaps (the latter two use existing generators).
Four fixture helpers are attributed; Java API/custom move metadata parity is not
claimed. Five original regressions cover independent exhaustive small-case states,
rollback, custom order/pins/errors, lazy cloning and escaping a single-change local
optimum in both scoring backends. Initial focused compilation required correcting
two regression API calls; all focused cases and final checks then passed.
Final check.sh passed:2381 Rust tests,130 doctests,fmt/Clippy;46 Python jobs reused.
Three Rust targets ran (lib and two new suites);323 reused unchanged results.
Affected prebuild13.63s; final check pipeline about2m42s elapsed (log timestamps),
including selection/report overhead, not just test execution. Source digest:
5ea15043affa7b47a05d171f45120462bab8a061e06281de7ef6f182d991486d.
Coverage1165/4855(23.996%),pending3408,excluded282,failed/unverified0. Live dashboard
freshness matches. No UI/browser, benchmark sweep or release rerun; older measurements
remain stale. Logs:work/value-exchange-build.log,work/value-exchange-check.log,
work/value-exchange-focused.log. All build/check/freshness jobs completed.
The full rewrite goal remains active.

## Compound list move builder

Added ListMoveBuilder with an isolated validated draft and recorded native moves.
Supports single/batch assignment, unassigned-value replacement, existing-element
replacement with index adjustment, signed shifts, relocation, block exchange and
unassignment. Each method is atomic; failed calls/input-iterator panic leave prior
edits intact. Cloning forks a draft. push(Composite) permits temporary intermediate
pin/range violations when the final state is valid. finish normalizes one step and
preserves multi/empty composites, for replay against the same starting state.
The implementation uses full model snapshots; no incremental-edit performance claim.
Fifteen complete MoveDirectorTest assignment methods retain lists, all asserted
positions/unassigned states, errors and undo through native stream-director replay.
Java notifications/lifecycle semantics are not claimed; notification-specific shift
and replacement methods remain pending. Four original tests cover exhaustive small
replacement/shift indices and rollback, failure/panic recovery, forked drafts,
compound scoring and provider solving through both list scoring backends.
Final check.sh passed:2400 Rust tests,131 doctests,fmt/Clippy;46 Python jobs reused.
Three Rust targets ran (lib/two new suites);325 reused unchanged evidence.
Affected prebuild11.32s; final pipeline about1m19s by log timestamps. Source digest:
550f506c9afa0f6062faba8476fe92d52e1eefccdbcec4f87a17e2dcdb833b73.
Coverage1180/4855(24.305%),pending3393,excluded282,failed/unverified0. Live dashboard
freshness matches. No UI/browser, benchmark sweep or release rerun; old measurements
remain stale. Logs:work/list-move-builder-focused.log,work/list-move-builder-build.log,
work/list-move-builder-check.log. All jobs completed; no failures remained.
The previous turn made verified implementation progress and committed568629c.
This batch also makes implementation progress; the full rewrite remains active.

## Frozen-model solver lifecycle

Ordinary solve plans now skip phases when no basic variable or list entity is
movable, across all six backends. Complete initial assignments score once, emit
an Initial best event, and run no user commands/providers or FirstInitialized
notification. Explicit exact-search plans remain unchanged to preserve native
exhaustion/optimality. Prefix-pinned lists remain movable; either movable mixed
component preserves normal phase execution. Incomplete frozen lists validate
score bindings and return NoCompleteAssignment unscored, ahead of evaluation and
cancellation gates. Complete frozen models honor those gates before initialization.
Initialization errors use InitialScoring rather than claiming a phase ran.
Three complete DefaultSolverTest methods and five test helpers are attributed.
Nine original regressions cover actual scores, all backends, mobility, limits,
callbacks, bindings and failures. No production implementation was consulted.
The first broad run found three existing regressions in exact empty models and
zero-budget impossible models; production behavior was corrected, preserving
optimality/reason/work assertions. Only the ordinary empty-list phase/error
metadata expectation changed to reflect the new initialization boundary.
Final check.sh exited zero:2412 Rust tests,131 doctests,fmt/Clippy;46 Python jobs
reused. Shared solver changes ran276 Rust targets;54 reused successful evidence.
The final affected executable prebuild took3m08s. Source digest:
5e6a1dd067f69ebdf2a5e3a78a5e26678e1c6d89f366435550acd7bed11fe79a.
Coverage1183/4855(24.367%),pending3390,excluded282,failed/unverified0.
Logs:work/frozen-solver-build.log,work/frozen-solver-check.log. Earlier failed runs
are superseded by this successful final state; aggregate Rust metadata exit0 was
checked explicitly. Live dashboard verification digest matches the generated report.
No browser, benchmark sweep or release rerun; older performance measurements remain
stale. Full rewrite remains active.

## Direct random list-segment neighborhoods

Added SubListSelection: per-entity count blocks index all movable nonempty ranges
without value clones or a quadratic catalog. Ranks use entity order, descending
length and ascending start. Whole pins, prefixes, optional missing values and
inclusive bounds are handled. Lookup is logarithmic in entity/length counts after
ordinary model validation. Immutable indices retain their source coordinates.
Seeded or injected rank draws repeat with replacement; empty sources do no draws.
Added sampled_sublist_change_moves and sampled_sublist_swap_moves for lazy, bounded
relocation/reversal/unassignment and unequal-length exchanges. Separate attempt
limits include range/overlap/no-op rejections; duplicates remain. Sampling is not
uniform over legal moves. Search providers rebuild these from each step snapshot.
Four complete upstream methods and four helper/partial-fixture entries are
attributed. The empty-selection method stays pending because Java never-ending
metadata has no direct native property; constructor/lifecycle methods also remain
pending. No Java multi-entity distribution or PRNG trace parity is claimed.
Five adapted local cases and seven original regressions cover exact rank spaces,
large index/no cloning, laziness/reproducibility, snapshot retention, invalid and
impossible sources, sampled-vs-enumerated neighborhoods, rollback, and actual
provider solving with scoped and stream scoring. Initial focused compilation
caught two test API mistakes; both were repaired before final verification.
Final check.sh exited zero:2424 Rust tests,132 doctests,fmt/Clippy;46 Python jobs
reused. Only3 Rust targets ran (lib/two new suites),329 reused unchanged evidence.
Affected executable prebuild took7.49s. Source digest:
b9504a08469dd6a223562b7f8da9aeffec38aacdb7178a48ea3d84cd4e3f0fc1.
Coverage1187/4855(24.449%),pending3386,excluded282,failed/unverified0. Live dashboard
verification digest matches. No browser, release run or benchmark sweep; older
performance measurements remain stale. Logs:work/sublist-sampling-focused.log,
work/sublist-sampling-build.log,work/sublist-sampling-check.log. All jobs completed.
Previous goal turn made verified progress and committed d4f26b5; this turn adds
usable sampling implementations. The full rewrite goal remains active.

## Indexed planning value ranges

Added object-safe ValueRange with u128 occurrence counts and immutable Integer,
Enumerated, Boolean, Composite and Optional range implementations. Integer ranges
use widened i64 endpoint arithmetic and positive u64 steps, including nondividing
steps under the documented native half-open contract. Composite ranges route ranks
through checked prefix counts without flattening; optional wrappers prepend None.
Lazy double-ended iterators skip by rank. Uniform occurrence draws repeat with
native seeds or injected ranks; duplicates retain their sampling weight. Stable
sorted copies preserve held snapshots, and optional present-value sorting keeps
None first. Materialization requires a caller limit before reading values.
variable_from_range provides bounded conversion to the existing finite solver;
this does not introduce lazy domains into solver storage/search.
Thirty-four complete upstream methods and three helpers are attributed. Int/long
fixtures retain distinct original boundaries using native i64; missing values and
invalid indices/exhaustion use Option. Negative-index and unsupported-sort methods
remain pending. Seven original regressions cover arithmetic extremes, counts above
usize, composite/optional overflow, lazy skipping and sampling, rejected allocations,
stable sorting and a real exact solve with combined domains/known optimum.
Focused cases/regressions passed; two initial parentheses warnings were removed.
A first check began broad reruns because an inherent Variable constructor made the
new module a dependency of the shared type. That run was explicitly terminated
(process group22479, terminal session60286 exit143) before moving the bridge to
standalone variable_from_range. No hashes/evidence were relabeled. Final check.sh
exited zero:2465 Rust tests,133 doctests,fmt/Clippy;46 Python jobs reused.25 Rust
targets ran (including22 cache records refreshed after the bridge adjustment),309
reused unchanged results. Future isolated edits need only their affected inputs.
Final affected executable prebuild21.77s. Source digest:
9558210af31ba224e25e5d65eb619b9762a0d110a39847132dac78191b56e0a5.
Coverage1221/4855(25.149%),pending3352,excluded282,failed/unverified0. Live dashboard
verification digest matches. No browser, release tests or benchmark sweep; older
performance measurements remain stale. Logs:work/value-range-focused.log,
work/value-range-build.log,work/value-range-check.log. The interrupted intermediate
check is retained in work/value-range-constructor-check.log. Final checks and
freshness jobs completed. Previous turn made progress and committed e224515;
this turn adds working range capabilities. The full rewrite goal remains active.

## Common-value selection and sampled group changes

Added CommonValueRanges over shared immutable indexed range handles. Arc identity
sets deduplicate without traversing values; equality ignores handle order. Borrowed
variable-domain construction deduplicates content-equal ordered slices and validates
member IDs. Explicit solution-wide fetch runs once. Assigned destinations must
belong to every input; absent destinations and excluded values are rejected.
Exact selection uses a uniform reservoir over legal occurrences of the smallest
source, with a preflight scan limit. Bounded rejection probes use the same source
and fall back to an exact scan; oversized scans return ScanLimit rather than a
false empty result. Ten-times-size probe budgets saturate at u64::MAX. Defaults
are64 random draws and100000 exact entries. Duplicate source occurrences retain
their weight; unique-source destinations are uniform. User range operations remain
cooperative. Native seeds do not reproduce Java random traces.
sample_common_change produces native Change/Composite moves for selected members,
including heterogeneous/unassigned starting states. It excludes whole-group no-ops,
rejects pins, and preserves member order. Model/director application and rollback
remain unchanged. Both scoped and stream provider searches escape the tested
single-change local optimum using the same sampled group change.
Nine upstream unit methods and two parameterized bias methods are attributed,
with eight helpers/fixtures. Bias tests retain both parameter rows each,200000
trials per row, complete category reachability and the original5-sigma threshold.
Root-seeded native trial draws replace Java splitFrom. Six original regressions
cover exhaustive small intersections, exact/probe budgets and fallback, valid huge
constant-value ranges, immutable identity boundaries, bad IDs, pins, rollback and
actual provider solving. Focused checks passed; no production implementation read.
Final check.sh exited zero:2482 Rust tests,134 doctests,fmt/Clippy;46 Python jobs
reused. Only3 Rust targets ran (lib/two new suites),333 reused unchanged evidence.
Affected executable prebuild20.92s. Source digest:
331eb23362fa4cf285c0c848eba09ce859d3dcc6384aa02186320ade847a3fc9.
Coverage1232/4855(25.376%),pending3341,excluded282,failed/unverified0. Live dashboard
verification digest matches. No browser, release run or benchmark sweep; older
performance measurements remain stale. Logs:work/common-values-focused.log,
work/common-values-build.log,work/common-values-check.log. All jobs completed.
Previous goal turn made verified progress and committed4f060ab. This batch adds
usable intersection/group-move behavior; the full rewrite goal remains active.

## Arbitrary-precision numeric value ranges

Added BigIntegerValueRange and DecimalValueRange over the indexed ValueRange API.
Arbitrary-precision endpoints and positive steps use exact half-open arithmetic;
counts must fit u128. Default decimal steps are one coefficient unit at the fixed
input scale. Decimal endpoints/explicit steps must share that scale and membership
preserves representation equality. Mismatches reject before any scale-padding
allocation. Bounds need not divide the stride under the documented native contract.
Existing composition, optional wrappers, common-destination selection and bounded
finite-domain conversion work with both new types. Solver domains remain explicit.
Ten upstream methods and two iterator helpers are attributed. Every default/explicit
step, count, indexed value, membership/null assertion, prefix versus complete
iteration and scripted draw is retained, including exact decimal scales. Two
unsupported-sort methods and broader Java constructor/scale behavior remain pending.
Seven original regressions cover independent small progressions, thousand-digit
offsets/huge steps, maximum/overflowing counts, extreme i32 scales, common-value
intersection, optional/composite ranges and an exact decimal solve with optimum0.06.
Focused tests and first Rust pass succeeded. The new doctest initially assumed
plain Decimal Display output; the existing type prints102e-2. The example was
corrected to compare exact Decimal values, leaving production behavior unchanged.
Final check.sh exited zero:2499 Rust tests,135 doctests,fmt/Clippy;46 Python jobs
reused. First run executed3 Rust targets and reused335; after the documentation fix,
only lib ran and337 targets reused successful results. Initial affected executable
prebuild43.38s. Final source digest:
1fddb8abdb431d980a864722815d7e5acdab9e7f957d6dc510d94cc1ab61cb1e.
Coverage1242/4855(25.582%),pending3331,excluded282,failed/unverified0. Live dashboard
freshness is verified (same digest reuses its earlier successful response). No
browser, release suite or benchmark sweep; older measurements remain stale.
Logs:work/exact-range-focused.log,work/exact-range-build.log,
work/exact-range-first-check.log (doctest failure),work/exact-range-check.log
(successful final gate). All jobs completed. Previous goal turn made progress and
committed4874128; this batch implements numeric ranges. Full rewrite remains active.

## Gregorian calendar and temporal value ranges

Added validated CivilDate, CivilTime, CivilDateTime and CalendarYear values plus
TemporalValueRange over the existing indexed range API. Independent Gregorian
arithmetic supports year zero and ±999999999; bounded binary search inverts day
numbers. Local clock values retain nanosecond precision. Supported units span
nanoseconds through millennia where meaningful for the value type. Positive steps,
ordered bounds, supported units and exactly reachable endpoints are enforced.
Month/year increments anchor to the original start and clamp month ends without
cumulative drift. Wide counts/indexes, membership, sampling and reverse iteration
stay lazy; bounded materialization integrates date domains into finite solving.
No timezone/DST/Java API parity or new performance result is claimed.
Twenty-one complete TemporalValueRangeTest methods and two iterator helpers are
attributed. Four Java mock-unit/negative-index/unsupported-sort cases remain pending.
Seven original regressions cover three complete 400-year calendar cycles including
negative years, leap-century/day boundaries, huge nanosecond counts, checked
arithmetic, unsupported units, invalid dates/clocks, intersections/optional ranges
and a real date-domain appointment solve. No upstream production implementation read.
Initial test-adaptation script stopped on multiline whitespace before writing the
first ten methods; corrected that test-only converter. A focused regression compile
then caught an i32 cast against native i64 scores; removed the unnecessary cast.
Final focused suites passed. Final check.sh exited zero: 2527 Rust tests,
136 doctests,fmt/Clippy; 46 Python jobs reused. Only 3 Rust targets ran
(lib/two new suites), 337 reused unchanged successful results. Affected
executable prebuild45.09s. Verified aggregate exit0 and matching before/after source:
0fd45ac5408a2cd4929586229d25706adfb61187122ac0eb56a7bf026e589b64.
Coverage1263/4855(26.014%),pending3310,excluded282,failed/unverified0. Live dashboard
verification digest matches report. No browser/release/benchmark sweep; older
performance evidence stays stale. Logs: work/temporal-range-focused-final.log,
work/temporal-range-build.log,work/temporal-range-check.log. Gate session60086 and
freshness session46025 finished successfully; no batch process remains running.
Previous turn answered a timing question without implementation progress; this
turn resumed the next safe implementation and delivered verified calendar behavior.
The full independent solver rewrite remains active and incomplete.

## Cascading list shadows and atomic move trials

Added ListCascadeCache with explicit callback inputs for value identity, owner/index,
previous/next identity, updated previous shadow and current cached shadow. Structural
metadata changes and explicit dirty facts trigger calculation in list order, at most
once per element; equal Option<S> results stop forward propagation without suppressing
later dirty roots. Unassigned values have no list dependencies. Independent callbacks
use independent caches; one tuple/struct can group jointly updated fields. Callbacks
must be idempotent for unchanged logical inputs and must explicitly invalidate changed
external facts/formulas. from_values deliberately permits imported partial shadows.
Clones share immutable model/assignment/position maps and persistent shadow roots,
including non-Clone payloads. update validates metadata before suppliers and installs
state only after successful calculation. Native apply returns a trial that restores
lists and shadows on drop, or commits both; errors and callback unwinding preserve
cache state, though external callback side effects are not reversible. Validation
and structural detection inspect full assignments; only callback work is selective.
Four complete single/distinct cascading-shadow methods and seven fixture helpers
are attributed. Native seeded shadows plus an explicit first dirty value substitute
for Java partial owner/previous notifications; reflection/lifecycle parity is not
claimed. Six original regressions cover all generated small partial assignments,
independent full-fold comparison, early stopping/later roots, external facts, list
move trial/commit, unassignment, pins/invalid metadata, callback unwinding, grouped
non-Clone payloads and an exact arrival-time route solve choosing[1,2,3] over[3,2,1].
Focused compilation caught a test-only read of a context after move; capture the
fact before consuming the context. Final focused suites passed. Affected executable
prebuild38.00s. Initial gate session46236 terminated with exit143 during lib tests;
no test failure was reported and process inspection confirmed no live gate/children.
Resumed check reused completed fmt/Clippy and finished exit0 (session15620):
2537 Rust tests,137 doctests;3 Rust targets ran,339 reused unchanged
successful evidence;46 Python jobs reused. Aggregate exit0 and equal before/after
source digest verified: ff43104dae3cb8db30e552187a35425d723f4e53e7ec4b77e2f87e152288ca12.
Coverage1267/4855(26.097%),pending3306,excluded282,failed/unverified0. Live freshness
session48131 completed and matched report verification digest. No browser/release/
benchmark sweep; older performance measurements stay stale. Logs:work/list-cascade-
focused-final.log,work/list-cascade-build.log,work/list-cascade-check.log(interrupted),
work/list-cascade-resumed-check.log(success). All batch jobs are terminal.
Previous goal turn made progress via f2635e1; this turn adds usable incremental
cascade callbacks and atomic list trials. Full solver rewrite remains active.

## Dependency-order work queue integrated with derived-value propagation

Added NodeOrderQueue over shared immutable ranks: offer deduplicates pending IDs,
pop permits reoffering, equal ranks break ties by ID, and invalid IDs fail without
mutation. Peek, clear, independent clones and partial fused drains are supported.
Heap/membership storage grows with offered work; rank snapshots can be shared.
DependencyCache now caches ranks in each immutable plan and uses the queue for
value propagation. Value-only updates still avoid scanning the node universe;
edge edits build new rank snapshots before propagation. No performance claim made.
Eight complete NodeTopologicalOrderQueueTest methods and two helpers are attributed.
The native random workload retains 50x1000 operations,200 nodes,50 possible ranks,
seed0; an ordered-set reference replaces Java PriorityQueue with deterministic ties.
Three original regressions cover snapshots/partial drains/invalid IDs, topology
snapshot isolation and converging dependencies with edge reversal/cycle recovery.
Existing exhaustive dependency-cache tests also passed in focused development.
Diff review caught a collision with the existing node_order_cases.rs comparator
suite. Stopped prebuild group20256/session20627 (exit143), restored that file exactly
from HEAD and moved new cases/regressions to dependency_queue_*.rs. No old tests
were removed. Corrected prebuild36.11s. Shared propagation changed broad job inputs;
batched remaining executable compilation completed in5m51s (session26003 exit0).
Final check.sh session59752 exited0: 2548 Rust tests,138 doctests,fmt/Clippy;
291 Rust targets ran,53 reused;46 Python jobs reused. Verified aggregate exit0 and
matching before/after source: 63889fb33a0e90f28d99acff3b73b2f77c5212bae3ce1b5ea20d46ccd3d95416.
Coverage1275/4855(26.262%),pending3298,excluded282,failed/unverified0. Live freshness
session36474 exited0 with digest matching report. No browser/release/benchmark sweep;
older performance evidence remains stale. Logs:work/node-order-focused.log,
work/node-order-regression.log,work/dependency-queue-build.log,
work/dependency-queue-remaining-build.log,work/dependency-queue-check.log. All jobs
are terminal. Previous goal turn made progress via6803562; this turn adds a reusable
scheduler and verifies its shared-cache integration. Full rewrite remains active.

## Weighted candidate snapshots and provider sampling

Added WeightedSelection over immutable shared occurrence snapshots. Fractional f64
weights define cumulative intervals, zero weights disable occurrences, duplicates
keep separate weights, and empty/all-zero snapshots exhaust without drawing.
Power-of-two rescaling handles maximum/subnormal totals. Invalid weights/draws,
candidate limits and positive weights lost to precision return explicit errors.
Sampling is approximate f64 arithmetic on native53-bit draws, not Java PRNG parity.
Bounded collection polls at most maximum+1 items to detect overflow. Reweighting
captures a new distribution once per occurrence without cloning items or changing
held distributions. Borrowed sampling does not clone; owned sampling clones only
selected items and integrates with existing provider budgets. User callback side
effects are not reversible. Provider tests solve through both basic backends.
One full ProbabilityEntitySelectorTest.getSize method is credited. Both entity/move
scripted random sequences are attributed partial fixtures and remain pending because
Java lifecycle forwarding/child iterator-call checks are absent. Seven original
regressions cover intervals/duplicates/zero weights, invalid/extreme numbers, finite
bounds over infinite sources, non-Clone snapshots, selected-only cloning, seeded
frequency/reproducibility and actual weighted candidate solving. Focused checks passed.
Final check.sh session27213 exited0: 2558 Rust tests,139 doctests,fmt/Clippy;
only3 Rust targets ran,343 reused;46 Python jobs reused. Prebuild17.37s. Aggregate
exit0 and unchanged before/after source verified:62af0877fc9afb2f998fcf30f55c2cde5a4f900e7857c3cc7cc7da85372dec52.
Coverage1276/4855(26.282%),pending3297,excluded282,failed/unverified0. Freshness
session33867 exited0 and matched report digest. No browser/release/benchmark refresh.
Logs:work/weighted-selection-focused-final.log,work/weighted-selection-build.log,
work/weighted-selection-check.log. All jobs terminal. Previous turn made progress
via63f4761; this turn adds usable weighted candidate exploration. Full goal active.

## Scoped selection caches and lifecycle forwarding

SelectionCache now captures finite candidates and optional weights at solve, phase
or step start, forwards every balanced native lifecycle event once, and releases
its own snapshot at the configured scope end. Original traversal preserves order;
random traversal supports uniform/weighted injected or seeded draws and metadata.
Held snapshots retain non-Clone items after release. Explicit source sizes are
checked; unknown finite sources remain collection-bounded. Invalid transitions
forward nothing. Failed builds/unwinding leave the scope active without a snapshot;
end that scope before retrying. External callback side effects are not reversible.
Parent lifecycle delivery remains explicit, with no automatic solver-listener or
Java runtime-class parity claim. Solve/phase caches intentionally retain old facts
until their next configured boundary; choose scope accordingly.
Twenty complete methods from entity/value/move caching and entity/move probability
selectors now preserve sequence, size, repetition, rebuild and forwarding counts.
Fourteen helper fixtures are attributed. The earlier immutable weighted-sequence
fixtures remain partial in their original file; full method credit now comes from
selection_cache_cases.rs. Six original regressions cover context refresh/release,
invalid transitions/retry, bounded sources, callback unwinding/invalid weights,
non-Clone ownership and actual weighted step-provider solving in both backends.
Focused suites passed (20 adapted +6 original). Development corrected explicit
lifetime bounds in test source/provider implementations. Prebuild14.24s; first
final gate31257 exited101 for Clippy collapsible_if. Collapsed the condition and
reran affected checks; final gate64457 exited0:2584 Rust tests,139 doctests,
fmt/Clippy;3 Rust targets ran,345 reused;46 Python jobs reused. Aggregate exit0
and equal before/after source verified:
45e37b5ffb8c370015a9a7f5af37bda51b84661d7e905b8ffa1dee11b3cc9e94.
Coverage1296/4855(26.694%),pending3277,excluded282,failed/unverified0.
No browser/release/benchmark sweep. Older performance evidence remains stale.
Logs:work/selection-cache-focused-final.log,work/selection-cache-build.log,
work/selection-cache-check.log(lint failure),work/selection-cache-final-check.log.
The preceding timing-answer turn made no repository progress; this continuation
revalidated the worktree and completed the pending cache implementation batch.
Full independent solver rewrite remains active and incomplete.
Dashboard freshness session53699 exited0 and matched the current report digest.
All batch process handles are terminal.

## Stable sorted and step-shuffled selection snapshots

OrderedSelectionCache wraps native scoped candidate caching with stable sorting
at source rebuild or a fresh finite permutation at each step. Shuffles retain
previous order until the source scope rebuilds. Immutable indexed snapshots share
non-Clone candidates, retain duplicates and outlive cache release; owned traversal
clones only yielded items. Invalid events preserve current order; failed ordering
or comparator unwinding makes order unavailable until the next configured rebuild.
Callbacks are owned, parent events explicit, and callback side effects irreversible.
Thirteen sorting/shuffling selector methods and eleven helpers are attributed.
Java comparator class/factory configuration (applySorting) remains pending.
Six original regressions cover scope/context, stable ties, all bounded draw
combinations through six items, failure/recovery, ownership/lazy cloning, seeded
repeatability, collection limits and actual provider solving in both basic backends.
A sorted provider selects the optimum with only one candidate evaluation.
Focused session61295 exited0 (13 adapted+6 original,compile15.12s). Final prebuild
14.40s and check session61001 exited0:2603 Rust tests,139 doctests,fmt/Clippy;
3 Rust targets ran,347 reused;46 Python jobs reused. Aggregate exit0 and equal
before/after source verified:e815df52baa03d426e1e9a5a3ea13b46d713441d8bb333724f1795237129526a.
Coverage1309/4855(26.962%),pending3264,excluded282,failed/unverified0. Logs:
work/ordered-selection-focused.log,work/ordered-selection-build.log,
work/ordered-selection-check.log. No browser/release/benchmark sweep; previous
performance measurements remain stale. Previous goal turn made progress via4471dca;
this turn adds usable ordered candidate exploration. Full rewrite remains active.
Dashboard freshness session77074 exited0 with matching report digest. All batch
process handles are terminal.

## Composable filtering and independent candidate work caps

Added FilteredSelectionSource and LimitedSelectionSource. Both forward lifecycle
events, and lazily preserve accepted occurrence order. Filtering reports unknown
exact count and a separately labelled child upper bound; scoped caching materializes
matches. Limits before filtering cap inspected inputs, limits after filtering cap
accepted outputs; the latter alone cannot bound infinitely rejected work.
FilteredMoveProvider works through the native generic provider API, opening its
child lazily and applying separate per-step scan/selection caps. Stats distinguish
scanned/rejected/yielded counts and source exhaustion/scan/selection stops. A cursor
dropped early has no false exhaustion reason. Work inside an individual child or
predicate callback remains cooperative. Existing BoundedFilter retains retryable
probe semantics; no duplicate probing implementation was added.
Twenty-one complete upstream filtering/count-limit methods and eleven helpers are
attributed, retaining cache scope rebuilds, entity contexts and lifecycle counts.
Ordered list-iterator forwarding, random ending traversal and Java termination
polling cadence remain pending. Six original regressions exercise zero budgets,
early drop, changing contexts/stat reset, finite exhaustion, bounded rejection,
source composition/cache refresh and one-evaluation solving in both basic backends.
Focused sessions54543/58289 exited0 (final21 adapted+6 original). Prebuild17.11s;
first gate58250 exited101 for test-only manual_is_multiple_of lint. Corrected the
predicate and final gate74954 exited0:2630 Rust tests,139 doctests,fmt/Clippy;
3 Rust targets ran,349 reused;46 Python jobs reused. Aggregate exit0 and identical
before/after source verified:3598d10e4d79596eae696a76cdadea152b849ddc09c442a0eb493bc17ef9cfa2.
Coverage1330/4855(27.394%),pending3243,excluded282,failed/unverified0. Freshness
session77599 exited0 and matched report digest. All process handles are terminal.
Logs:work/selection-filter-focused-final.log,work/selection-filter-build.log,
work/selection-filter-check.log(lint failure),work/selection-filter-final-check.log.
No browser/release/benchmark sweep; older performance evidence remains stale.
Previous goal turn progressed via8e138a7; this batch adds usable filtered candidate
exploration with bounded work. Full independent solver rewrite remains active.

## Recording and replay for dependent selections

RecordingSelection now owns a source and publishes selected occurrences through
shared Arc payloads to independent ReplayingSelection cursors. Availability
prefetches one Rust item but does not publish it; next publishes, and each reader
consumes that publication once. Repeated availability probes do not advance the
source. Slow readers skip superseded choices. New recording traversals and every
explicit lifecycle event invalidate old cursors; stale and unrecorded choices
have distinct errors. One owner forwards events, avoiding duplicate child calls.
Channels are thread-local. Non-Clone payloads may outlive the owner, while dropping
unpublished lookahead clears availability. Source callback effects are not undone.
Two complete entity/value mimic methods and four helpers are attributed, retaining
all interleaved assertions, metadata, lifecycle counts and child iterator openings.
Six original regressions cover passive probes, independent readers/duplicates,
shared ownership/stale scopes, lookahead abandonment, source unwinding and two
replay consumers constructing a composite move that solves through both backends.
Focused session36002 exited0 (2 adapted+6 original,compile16.03s). Prebuild16.95s;
final gate3520 exited0:2638 Rust tests,139 doctests,fmt/Clippy;3 Rust targets ran,
351 reused;46 Python jobs reused. Aggregate exit0 and unchanged source verified:
06a1102e9e83297faca1be7b0e013e4da09ddc1fc67fe6948082d7248262487b.
Coverage1332/4855(27.436%),pending3241,excluded282,failed/unverified0. Logs:
work/selection-replay-focused.log,work/selection-replay-build.log,
work/selection-replay-check.log. No browser/release/benchmark sweep; older performance
measurements remain stale. Previous goal turn progressed via53c728c; this batch
adds shared dependent-choice coordination. Full solver rewrite remains active.
Dashboard freshness session88709 exited0 and matched report digest. All batch
process handles are terminal.

## Bounded independent selection products and random composite rows

SelectionProduct now forwards child lifecycle events and captures each independent
finite child under an explicit per-child limit. ProductSnapshot shares non-Clone
occurrences, preserves duplicate identity and lazily enumerates/indexes combinations
with checked u128 cardinality. Seeded or injected sampling draws independently from
each nonempty child; injected callbacks retain original child indices. Empty-child
policies include zero/all-empty products, and repeating metadata remains true for
empty random iteration. Source collection/metadata errors publish no snapshot;
callback side effects are not rolled back. Existing MoveProduct remains the lazy
restartable-factory API. Dependent suffixes/four mimic-product methods stay pending.
Twenty independent upstream product methods and eighteen helpers are attributed;
earlier enumeration fixtures keep their partial attribution without duplicate credit.
Six original regressions compare all small products with the independent iterator,
index 2^100 combinations, reject cardinality overflow and source bounds/mismatches,
verify non-Clone ownership and seeded/scripted sampling, and solve composite moves
through both basic backends in ordered/random modes. The first focused integration
run selected the first improving move under default policy; set BestAccepted for
the test's best-of-product assertion. No production correction was required.
Focused cases passed in session51463 (aggregate101 due to that assertion); corrected
regressions passed in session35765. Prebuild14.41s; final gate27816 exited0:
2664 Rust tests,139 doctests,fmt/Clippy;3 Rust targets ran,353 reused;46 Python jobs
reused. Aggregate exit0 and equal before/after source verified:
77f7b736b2d92ae1dce1de79df5cd74c1279d8cc50a3a5f17a209dc46d4d9196.
Coverage1352/4855(27.848%),pending3221,excluded282,failed/unverified0. Logs:
work/selection-product-focused.log,work/selection-product-regression-final.log,
work/selection-product-build.log,work/selection-product-check.log. No browser/release/
benchmark sweep; older performance evidence remains stale. Previous goal turn
progressed viad6dafec; this batch adds bounded independent product composition.
Full independent solver rewrite remains active.
Dashboard freshness session67483 exited0 and matched report digest. All batch
process handles are terminal.

## Prefix-dependent traversal and bounded conditional sampling

DependentSelectionIter now traverses prefix-dependent candidate trees depth-first,
reopening suffixes after earlier choices change. Empty suffixes prune only their
branch; ignored empty levels retain explicit None slots. Total child polls bound
work, including None/exhaustion probes, with distinct Exhausted/PollLimit stats.
Zero budgets do not open factories. Non-Clone prefix Arcs can be captured by suffix
cursors and retained in completed rows. DependentSelection supplies current context
and an explicit lifecycle hook; sample_with conditionally samples one fresh-prefix
path under a total collection budget. Empty samples reject only that attempt, not
the tree; conditional uniform sampling is not necessarily uniform over complete rows.
Two complete original-order mimic-product methods and six helpers are attributed.
The random mimic fixture expects b->1+a->9 on one row plus size24; those two methods
remain pending rather than claiming fresh-prefix sampling matches stale replay.
Six original regressions cover recursive reference trees, empty-branch recovery,
infinite rejection/zero budgets, ownership, conditional errors/budgets and correlated
composite solving in both backends. Factory/next callbacks remain cooperative.
Initial focused session35734 exited101 for test-only expression attributes; explicit
panic-closure return types fixed that. Focused session36219 exited0 (2 adapted+6
original). Prebuild20.04s; gate69941 exited101 for public signature type complexity.
Added DependentPrefix alias; final gate43530 exited0:2672 Rust tests,139 doctests,
fmt/Clippy;3 Rust targets ran,355 reused;46 Python jobs reused. Aggregate exit0
and equal before/after source verified:
12c51f6c4e10c6df4a2575da17195b74e72799be2a7c26965f561849b72ad739.
Coverage1354/4855(27.889%),pending3219,excluded282,failed/unverified0. Logs:
work/dependent-selection-focused-final.log,work/dependent-selection-build.log,
work/dependent-selection-check.log(lint failure),work/dependent-selection-final-check.log.
No browser/release/benchmark sweep; old performance evidence remains stale.
Previous goal turn progressed via391c26d; this batch adds bounded dependent choices.
Full independent solver rewrite remains active and incomplete.
Dashboard freshness session30947 exited0 and matched report digest. All batch
process handles are terminal.

## Immutable domain reachability indexes

ReachableValues captures inverse value-to-owner membership, shared-domain co-values,
optional transitions and independent sorted views. Equal ranges share storage but
retain every owner. Basic/list adapters preserve structural domains independently
of pins/current assignments; candidate providers still enforce complete legality.
Five complete upstream ReachableValuesTest methods are attributed, plus one partial
entity-membership fixture and the TestdataObjectSorter comparator helper. Runtime
Java type discrimination and multipleSorters identity/cache assertions stay pending.
Six original regressions include all 4,096 small domain/optionality configurations,
snapshot ownership, unknown/duplicate/empty cases and basic/list solving through
both scoring backends. Focused session49631 failed on test helper string lifetimes;
explicit static fixture arguments fixed it; session96089 passed all twelve tests.
Prebuild7.44s; final gate49190 exited0:2684 Rust tests,139 doctests,fmt/Clippy;
3 Rust targets ran,357 reused;46 Python jobs reused. Aggregate exit0 and equal
before/after source verified:
9ca1146b34962e6f735ce94bca0f9fde4b8bb172efa437ba0439db8f9603d1ae.
Coverage1359/4855(27.992%),pending3214,excluded282,failed/unverified0. Logs:
work/reachable-values-focused-final.log,work/reachable-values-build.log,
work/reachable-values-check.log. No browser/release/benchmark sweep; older performance
measurements remain stale. Previous goal turn progressed via06667ef; this batch adds
usable inverse-domain lookups. Full solver rewrite remains active and unfinished.

User asked time spent building/testing versus implementation. App command timings
for five completed batches (19:04–19:54 Vienna, Oct5) total49.905min; build/test/check
commands11.259min (22.56%). Remaining38.646min includes implementation, spec reading,
test authoring, debugging, docs and commits; not a measured pure-coding figure.
Some work overlaps commands, so this is approximate process time versus turn time,
not an exclusive effort accounting. No timing infrastructure was added.
Dashboard freshness session70682 exited0 and matched the report digest. All batch
process handles are terminal.

## Lifecycle unions and fractional stream interleaving

SelectionUnion forwards explicit lifecycle events to all children and sums enabled
size metadata with unknown/overflow represented as None. Ordered cursors open
children lazily; WeightedInterleave buffers at most one item per enabled child,
preserves each child order and retires exhausted children before drawing again.
Fractional weights use existing WeightedSelection validation/normalization. Zero
weights disable opening/polling; invalid draws produce one error then terminate.
Finite unions terminate even in random mode; repeating children require consumer
limits. Non-Clone items work; each next polls at most once per active child, but
individual callbacks are cooperative. Integer MoveUnion/ProviderUnion are unchanged.
Six complete UnionMoveSelector methods and five PlannerAssert helpers attributed;
nearby mixed-model configuration and Java PRNG parity remain unclaimed. Six original
regressions cover 200 finite schedules, fractional boundaries, disabled/non-fused/
infinite sources, non-Clone ownership, errors, lifecycle metadata, early drop,
seeded repeatability and basic solving in both backends. Initial focused66749
passed six adaptations but failed one incorrect precision-loss fixture; putting the
huge cumulative weight before the tiny one corrected that fixture. Focused60643
passed all six regressions. Prebuild10.47s; final gate66935 exited0:2696 Rust tests,
139 doctests,fmt/Clippy;3 Rust targets ran,359 reused;46 Python jobs reused.
Aggregate exit0 and equal before/after source verified:
7b3e8332c5b6dc8b2d87b4eb5dccced705f2ad67e09ab0ed30883a012e799f6c.
Coverage1365/4855(28.115%),pending3208,excluded282,failed/unverified0. Logs:
work/selection-union-focused.log,work/selection-union-regression-final.log,
work/selection-union-build.log,work/selection-union-check.log. No browser/release/
benchmark sweep; older performance evidence stays stale. Previous goal turn made
progress viaa710019; this batch adds usable lifecycle-aware candidate composition.
Full solver rewrite remains active and unfinished.
Dashboard freshness session40675 exited0 and matched the report digest. All batch
process handles are terminal.

## Scoped domain caches and related-value selection

ReachabilityCache composes SelectionCache and ReachableValues, rebuilding only at
its solve/phase/step start. BasicRangeSource/ListRangeSource capture native domains;
All/Entity/CoValues queries return independent sorted distinct WeightedSelection
views. source_size is full distinct universe size, separate from filtered length.
Optional transitions remain available on index().destinations(); source range
collection is bounded by maximum_entities, with finite materialized per-row vectors.
Failed/duplicate/panicking builds expose no old index; held snapshots remain usable.
Pins/current assignment/list positions remain provider responsibilities. Explicit
origin queries replace the Java manual mimic recorder in filtering adaptations.
Six complete upstream methods (three iterable entity-range + three filtering-range)
and seven helper attributions preserve all repeated five-step/two-phase sequences.
Six original regressions cover actual rebuild boundaries, held snapshots, failed
and panicking source recovery, invalid events, bounded collection, shared/optional
ranges, seeded views, native model changes and both basic scoring backends.
Focused34115 exited0 (twelve tests); prebuild10.52s; final gate86034 exited0:
2708 Rust tests,139 doctests,fmt/Clippy;3 Rust targets ran,361 reused;46 Python jobs
reused. Aggregate exit0 and equal before/after source verified:
3c27f5d3b4d877f481b5715e0e1ec923166e88d5a034128b54907dca65670a6a.
Coverage1371/4855(28.239%),pending3202,excluded282,failed/unverified0. Logs:
work/reachability-cache-focused.log,work/reachability-cache-build.log,
work/reachability-cache-check.log. No browser/release/benchmark sweep; old performance
evidence remains stale. Previous goal turn made progress via99ef767; this batch
adds lifecycle-scoped domain selection. Full solver rewrite remains active.
Dashboard freshness session68398 exited0 and matched the report digest. All batch
process handles are terminal.

## Assignment-aware sources and initialization gates

GatedSelectionSource checks a context predicate before child size/open calls and
forwards all lifecycle events. BasicSelectionContext/ListSelectionContext validate
current assignments before borrowing them. BasicEntitySource selects all/movable/
uninitialized indices; BasicValueSource retains domain order and optional None.
Required, unassigned and unpinned variables need initialization; optional None is
complete. ListValueSource selects all/assigned/unassigned/movable values in registry
order, honoring whole-list/prefix pins for the movable mode. Destinations and no-op
legality remain provider responsibilities. Four complete upstream reinitialization/
unassigned-list methods and six PlannerAssert helpers are attributed. Six original
regressions cover assignment changes, invalid contexts, unknown variables, disabled
child calls and scored basic/list initialization in both backends. Generic construction
providers, automatic lifecycle wiring and revision-aware entity-list caching remain
separate work; local search still requires a complete starting assignment.
Focused26813 failed for a missing native list Clone bound;81055 failed on a test
ListFact tuple variant. Focused12958 passed four adaptations but integration tests
incorrectly used local search with incomplete assignments. Reworked those tests to
apply initialization moves through both score directors;34277 exposed missing
structural penalties in expected intermediate scores. Corrected expectations;
40866 passed all six regressions. No local-search semantics changed.
Prebuild11.23s; final gate71740 exited0:2718 Rust tests,139 doctests,fmt/Clippy;
3 Rust targets ran,363 reused;46 Python jobs reused. Aggregate exit0 and equal
before/after source verified:
2306a77a42191d49b669973392cd08ca57eb6aba04089453f5d87f99eb297ccd.
Coverage1375/4855(28.321%),pending3198,excluded282,failed/unverified0. Logs:
work/assignment-selection-focused-passed.log(adaptations pass, old integrations fail),
work/assignment-selection-regression-passed.log,work/assignment-selection-build.log,
work/assignment-selection-check.log. No browser/release/benchmark sweep; previous
performance measurements stay stale. Previous goal turn progressed via62dd282;
this batch adds usable current-assignment sources. Full rewrite remains active.
Dashboard freshness session5524 exited0 and matched the report digest. All batch
process handles are terminal.

## Revision-aware candidate snapshots

RevisionedSelection captures a bounded finite source once per phase and optionally
checks an Eq token at each step start. Unchanged tokens reuse the snapshot; changed
tokens rebuild. Phase policy ignores mid-phase changes. Callers own revision
correctness, including changes made through source_mut; no automatic model counter.
Original/indexed list traversal and seeded/injected random cursors preserve metadata;
passive has_next does not draw. Invalid draws emit one error and fuse that cursor.
Random list access rejects even before lifecycle initialization. Failed/panicking
builds expose no snapshot; held non-Clone snapshots survive rebuild/release. Valid
lifecycle events advance before callbacks; close failed scopes normally; step policy
can rebuild at a later step. Ten complete FromSolutionEntitySelectorTest methods and
nine helpers preserve all five-step/two-phase sequences and exact extraction counts:
two for unchanged sources, four for dirty sources. Java step/JIT cases map to native
step-boundary checks. Six original regressions cover scope policy, metadata/limit/
panic failures and recovery, invalid events, non-Clone snapshots, empty/unbounded
sources, passive/invalid/seeded draws, indexed views and both basic scoring backends.
Focused34917 exited0 (10 adaptations+6 originals); prebuild12.03s. Gate74476 exited101
for an unnecessary explicit drop on a test cursor; removed it, with no implementation
change. Final gate74698 exited0:2734 Rust tests,139 doctests,fmt/Clippy;3 Rust targets
ran,365 reused;46 Python jobs reused. Aggregate exit0 and equal before/after source:
ec48f5c3ce6ef62e2597c46448f7207a26a029c57ec33142bab5afae8085b6be.
Coverage1385/4855(28.527%),pending3188,excluded282,failed/unverified0. Logs:
work/revision-selection-focused.log,work/revision-selection-build.log,
work/revision-selection-check.log(lint failure),work/revision-selection-final-check.log.
No browser/release/benchmark sweep; previous performance measurements stay stale.
Previous goal turn progressed via214aaec; this batch adds revision-aware candidate
reuse. Full solver rewrite remains active and unfinished.
Dashboard freshness session33231 exited0 and matched the report digest. All batch
process handles are terminal.

## Custom provider construction across all six scoring/model paths

ProviderConstructionConfig and six *_construction_from_with_provider entry points
now build partial assignments using the existing MoveProvider interface. Pools are
reopened on the committed snapshot each step; composites can initialize multiple
basic/list items jointly. A shared loop delegates to existing select_best engines,
keeping stable ties and installing cached winners without rescoring. Existing basic
values and same-owner list subsequences are preserved. Domain/pin/ownership-invalid,
non-progressing and preservation-violating moves are counted and skipped before
scoring. Optional values default to Preserve; optional-only additions need a
non-worsening score. Required progress wins structurally, including atomic mixed
moves. Explicit raw-scan bounds stop infinite unusable providers; score/move/step/
time/cancellation bounds preserve the evaluated-prefix winner. Exact limits avoid
extra lookahead, so a bounded result can be complete. NoCandidates never claims
infeasibility. Zero score/time/pre-cancellation avoids all score/provider callbacks;
zero move/step/scan permits initial scoring. All entry points validate first.

Fourteen original regressions cover actual selection-source-driven construction,
local-search handoff, joint/composite candidates, stable ties/no winner rescoring,
existing values/list order/owners/pins/ranges, optional policies, empty providers,
infinite invalid sources, partial resume, limits across all six paths, cancellation
and no early-pick prefetch. One new usage doctest. No upstream methods adapted or
provenance credit added. This closes the standalone provider-construction gap;
SolverPlan provider construction phases and automatic SelectionSource lifecycle
wiring remain open. Legality uses clone/preview validation, not optimized deltas.

Final focused94207 exited0 (14 tests); prebuild11.00s; final gate99191 exited0:
2748 Rust tests,140 doctests,fmt/Clippy;2 Rust targets ran,367 reused;46 Python jobs
reused. Aggregate exit0 and equal before/after source verified:
6fe473b0d3c470c8cc96592ef246d9cf3551a8c82d32d9f1f89262eba2084b33.
Coverage remains1385/4855(28.527%),pending3188,excluded282,failed/unverified0.
Logs: work/provider-construction-final-focused.log,provider-construction-build.log,
provider-construction-check.log (all under work/). Test compilation initially
needed native ListPosition fields, correct step-limit type and explicit graph score
type; fixed before final tests. No implementation/gate failures. No repeated browser,
release or benchmark sweep; previous performance evidence remains stale. The prior
question-only turn made no implementation progress; current clean state was inspected
before this implementation. The full rewrite remains active and unfinished.
Dashboard freshness10991 exited0 and matched report digest
111e190201ac667413835654bb52b3718281ad2b7d67d4dc5f194d544f20bae7.
All batch processes are terminal.

## Construction providers in complete solver plans

PhaseMoveProviders now accepts construction and local-search bindings in all six
solver paths. Bound construction replaces placement and mixed component order with
provider pools, preserving other construction controls. max_construction_scans
(default 100,000) resets per bound construction phase. Scoring adapters now support
Observed directors and shared construction-condition polling. They publish initial
scores and committed winners only; construction neither advances shared search steps
nor starts/resets stagnation. New ConstructionTermination::NoCandidates/ScanLimit
retain honest phase-local reasons. Partial results can continue through construction
or exact phases; incomplete local/repair handoff policy remains enforced. Existing
provider state persists and step indices restart at zero in every entered phase.

Seven new integration tests exercise all six paths, including repeated bindings,
partial/complete handoffs, budgets, event cancellation, initialization boundaries,
raw-scan limits, empty-pool exact completion and invalid binding preflight. A new
controlled shared-condition unit test verifies prefix retention and no search-only
monitor calls. One usage doctest added. Existing provider plans now reject custom
and missing bindings instead of rejecting newly-supported construction bindings.
First focused run63199 had one wrong test expectation: an incomplete model still
prepares/validates its graph at zero score budget. Corrected the expectation without
changing that solver contract. Focused7983 passed 7 new + 14 standalone + 6 existing
provider tests; controlled unit65608 passed. Initial broad prebuild took3m54s;
gate1223 stopped at two mixed-context type-complexity lints. Added a local type alias;
final prebuild took37.34s. No new upstream adaptation credit or performance claim.
Explicit SelectionSource lifecycle integration remains open. Previous goal turn
made progress via cc95ebc; this increment integrates that feature into full solves.
The full rewrite remains active and unfinished.
Final gate55431 exited0: 2756 Rust tests,141 doctests,fmt/Clippy;314 Rust targets ran,
56 reused;46 Python jobs reused. Broad coverage reflects shared solver changes.
Aggregate exit0 and equal before/after source verified:
9b1bfc43849a8a99c4b9016a4a4491bfc95fd63575e92c0acd400bdedb727657.
Coverage remains1385/4855(28.527%),pending3188,excluded282,failed/unverified0.
Logs: work/construction-provider-plan-focused-passed.log, -control.log,
-final-build.log and -final-check.log (same construction-provider-plan prefix).
No release, benchmark or browser sweep; existing performance measurements remain
visibly stale. The broader final gate passed after the lint repair.
Dashboard freshness52822 exited0 and matched report digest
461a73115537818e037dd4b57e89a0278a92251505f8267ff6bcf70d45c5e042.
All batch processes are terminal.

## Queued and pooled construction placements

PlacementQueue captures bounded phase keys, visits children in key-major order
once or cyclically, and builds each finite candidate pool against current context.
PlacementSource supports lazy sources; PlacementFn wraps vector callbacks.
pooled_placements adapts a regular source to repeated unit-key placements.
PlacementSnapshot shares immutable non-Clone keys/moves and survives later phases.
Passive has_next does not open sources or promise a nonempty candidate pool. Empty
keys/children exhaust; empty pools still count as placements. A caller scanning a
cycle for nonempty pools must impose its own placement bound. Composite candidates
can use SelectionProduct. Lifecycle remains explicit, not automatic solver wiring.
Invalid events forward nothing; failures/panics invalidate the phase until normal
closure/restart. Ordinary key-build errors still notify child scopes; partial panic
side effects remain the caller's responsibility. Held snapshots stay usable.

Five full upstream queued-entity/value/pooled methods plus seven assertion helper
attributions preserve every pool, finite/cyclic availability, phase restart and
lifecycle callback count. Java factory-copy methods remain pending. Seven original
regressions cover live context versus phase keys, duplicates, empty collections,
non-Clone ownership, bounded/unbounded/mismatched/panicking sources, recovery, and
actual two-variable joint placements through full plans in both basic backends.
Focused cases49578 passed five adaptations. Initial native test compilation6512
needed separate SolverExtensions values for scoped/stream error types; fixed;
84261 passed six regressions. Initial gate76256 passed. Final review then found a
child-lifecycle imbalance after an ordinary key-build error; fixed event forwarding
and added the seventh targeted regression before final verification. Previous goal
turn progressed via14461c2; this increment adds reusable placement scheduling.
No benchmark, release or browser sweep; old performance evidence remains stale.
The full rewrite remains active and unfinished.
Final gate99949 exited0: 2768 Rust tests,141 doctests,fmt/Clippy;3 Rust targets ran,
369 reused;46 Python jobs reused. Aggregate exit0 and equal before/after source:
ec91cfa330d4f2d5d381b20b2db2930ab0c21f996398080ad6d50af4b73b02be.
Coverage1390/4855(28.63%),pending3183,excluded282,failed/unverified0. Logs:
work/placement-queue-cases.log, -regression-passed.log, -recovery.log,
-final-build.log and -final-check.log (same placement-queue prefix).
Dashboard freshness26608 exited0 and matched report digest
35f4ac99e22b1b12c77fd6cd7810fef8bbe27e4c54562ec5febe2b5cca9e959e.
All batch processes are terminal.

## Lazy unique sampling over indexed occurrences

UniqueIndices uses a sparse shrinking-prefix remapping table to sample without
replacement, with u128 ranks/counts and storage proportional to consumed draws.
unique_indices/unique_slice_samples/unique_range_samples provide seeded cursors;
injected-draw constructors support deterministic rank choices. Slice sampling
borrows non-Clone payloads; range sampling fetches only chosen values and supports
ValueRange trait objects. Duplicates at separate positions stay separate occurrences.
Empty/singleton pools do not draw; availability is passive. Invalid draws/missing
values yield one error then exhaust. Draw panics precede mapping mutation; callback
side effects remain external. Fallible size hints use only a one-item lower bound,
and exhausted tables may retain allocated capacity. No Java random trace parity.

Three unique and two repeating upstream methods plus toEntries helper attribution
preserve exact membership, repeated draining/exhaustion, 300 repeating draws and ten
singleton repeats. Java remove/unsupported bulk-drain methods remain pending. Seven
original regressions exhaust all draw paths through length six (bijective permutation
coverage), test u128::MAX bounds/storage, non-Clone/optional/duplicate payloads,
seed repeatability, invalid/panicking draws, indexed failures and lazy candidate
construction with both scoring backends (128 distinct candidates,129 evaluations).
Focused44575 and14710 exited0 (five adaptations/seven originals). Final gate20984
exited0:2780 Rust tests,142 doctests,fmt/Clippy;3 Rust targets ran,371 reused;46 Python
jobs reused. Aggregate exit0 and equal before/after source verified:
4655008aaf3698d4bba43191bd08975fa722ac73ce468d39553b560917776173.
Coverage1395/4855(28.733%),pending3178,excluded282,failed/unverified0. Logs:
work/unique-sampling-cases.log, -regression.log, -build.log and -check.log (same
unique-sampling prefix). No benchmark/release/browser sweep or new timing claim.
Previous goal turn progressed via380080c. The full rewrite remains active.
Dashboard freshness96637 exited0 and matched report digest
3344c179a74b2bf1c49a4ab812a07ee1b2129c8baddb674741fd777538e30172.
All batch processes are terminal.

## Indexed unique-pair scoring

UniquePairNode retains persistent source rows and equality buckets, emitting each
pair once in stable ID order (smaller, larger). No self pairs/reversed duplicates;
equal payloads at distinct IDs still pair. Updates visit only old/new bucket peers;
same-key replacement refreshes surviving payloads, key changes retract then put,
and removals use captured keys without projecting. Linear retained index state;
dense output/downstream matches can remain quadratic. Candidate roots/output are
prepared before installation, preserving standalone state on ordinary panics.
StreamGraphBuilder::unique_pairs and unique_pairs_equal route each source update
once and use the normal checkpoint journal, shared branches and rebuild machinery.
Tuple equality keys and downstream filters compose; unassigned filtering stays
explicit. Native ID orientation differs from eager stream position orientation.

Two full upstream methods preserve B/A/C insertion, all three unordered pairs,
equality keys 2/2/10, AB-to-BC update, exact match payloads and unit/total penalties.
Assertion and zero-entity fixture helpers are attributed. Six original regressions
cover 2,000 indexed edits vs complete enumeration, sparse bucket updates/callback
counts, duplicate payloads, panic recovery, 500 multi-edit graph trials with shared
score/filter/dataset branches, foreign ports, rebuilds and composite planning
transactions. No Java reflection, event-order or throughput parity claimed.

Initial focused47641 failed to compile a test helper's elided string lifetimes;
explicit static occurrence IDs fixed it. Focused66103 passed all eight tests.
Broad prebuild20839 exited0 (2m22s). Final gate96288 exited0:2788 Rust tests,
143 doctests,fmt/Clippy;320 Rust targets ran,56 reused;46 Python jobs reused.
Aggregate exit0 and equal before/after source verified:
3655c46b545cca3d4c3078b443c7ecc36164bd88c2f00d8ea80a573a4161d4f0.
Coverage1397/4855(28.774%),pending3176,excluded282,failed/unverified0.
Dashboard freshness39050 exited0 and matched report digest
98e9d9bc40e84597f3eab0a1ab566f33a94f5ff416f195532e39621e282e0ae0.
Logs:work/unique-pair-focused.log,-build.log,-check.log (unique-pair prefix).
All processes terminal. No benchmark/release/browser sweep; old timing evidence
remains stale. Previous goal turn progressed via8a9c5b8; full rewrite stays active.

## Flat collector tuples and compound grouping

Native tuples of two,three,four ReversibleCollectors now aggregate corresponding
flat inputs to flat outputs; nested tuples remain supported. Each mutation stages
all delegates before installation; failed later removal/replacement or ordinary
panic preserves prior components. Specialized replace delegates unchanged, including
SumCollector's checked intermediate difference. No input Clone bound is added.
DistinctCountCollector<V> retains persistent multiplicities, releases a key on final
removal and finishes in constant time to native usize; Option keys count null
explicitly. Graph grouping/conditional/finishing adapters use these without new
builder methods. Direct mutation inputs must identify actual captured occurrences.

Eight unary and ten binary upstream methods now preserve compound keys, multiple
aggregates, all exact match fields, codes/set identities, count-weighted penalties,
MAX/MIN extrema, filtered counts, removals, group changes and duplicate-group
survival. Binary cases use live unique pairs from c810df5. Fixture/assertion/code
helpers are attributed; unused reflection/value-group/random properties omitted.
Five original regressions cover duplicate/null counts and snapshots, atomic late
failure, specialized overflow, non-Clone/nested tuples, four-collector groups in
500 graph trials and downstream overflow rollback. No extra method credit.

Focused93702 passed18 adaptations. Initial regression16614 failed to compile a
snapshot equality assertion; changed to comparing snapshot rows, and corrected a
positive replacement fixture to respect checked difference arithmetic. Regression
43638 passed5 tests. Prebuild74242 exited0. Initial gate14606 passed Rust/lint but
failed the new doctest's generic counter input inference; explicit Input fixed it.
Final gate52215 exited0:2811 Rust tests,144 doctests,fmt/Clippy;3 Rust targets ran,
375 reused;46 Python jobs reused. Aggregate exit0 and equal before/after source:
c998e9633c16e8ab44b92d24a318f1c6927078c8cfa3ca6199e790fe1b84a6ac.
Coverage1415/4855(29.145%),pending3158,excluded282,failed/unverified0.
Dashboard freshness93368 exited0 and matched report digest
a2a188dd7b611124896ac88e0081535f45db8dc99306cb4a9d79f49dbc52be7d.
Logs:work/tuple-group-cases.log,-regression.log,-build.log,-check.log and
-final-check.log (tuple-group prefix). All processes terminal. No benchmarks,
release tests or browser sweep; old timings remain stale. Previous turn progressed
via c810df5. Full solver rewrite remains active and unfinished.

## Grouped three- and four-fact join coverage

Twenty full upstream Tri/Quad grouping methods now run through actual native join
chains: explicit entity/group/value sources, identity joins back to unique-pair
members, distinct-third-entity filtering and a same-group fourth-entity join.
Preserve all source fixture cardinalities/modulo membership, aggregate fields,
count penalties, MAX/MIN extrema, entity-set identities and specified removals.
All passed against the existing graph; no engine behavior change was necessary.
Code(id) represents Generated Entity id bijectively. Unused value-group facts and
random long fields are omitted. Native counters retain independent collector state.

The source AssertableMatch.isEqualTo helper compares equal scores/field counts and
expected.containsAll(actual), ignoring positions and duplicate-field multiplicity.
The adapter preserves that helper, its missing/excess checks, score/count and
literal source expectations. This is material to Tri groupBy_4Mapping0Collector
and Quad groupBy_3Mapping0Collector; do not reinterpret these fixtures as exact
ordered tuple assertions. Helper/provenance entries explicitly record the scope.
Earlier grouped_tuple_cases use stronger exact equality and already pass.

Focused82897 passed20 cases. Final gate70013 exited0:2831 Rust tests;one Rust target
ran,378 reused;144-doctest evidence and46 Python jobs reused;fmt/Clippy passed.
Aggregate exit0 and equal before/after source verified:
2168a253dd760928eb1801498dca749ad7cd0e1189479f9fd1f20fbbb4ddbc28.
Coverage1435/4855(29.557%),pending3138,excluded282,failed/unverified0.
Dashboard freshness94975 exited0 and matched report digest
bf86f6f8f8fb537486f52f395e31bb7ffcb75e56055dfd1ab666c0640d543c42.
Logs:work/grouped-multiway-cases.log and work/grouped-multiway-check.log.
All processes terminal. No failure, benchmark, release or browser sweep in this
batch. Previous goal turn progressed via67b29a0; full rewrite remains active.

## Shared reusable stream transforms

SharedFilter/SharedMap/SharedFlatMap retain callback identity and weak registrations
of builder owner,parent node,output node. Reapplying a definition/clone to the same
parent returns the same node; fresh definitions, parents and builders remain separate.
Ordinary builder calls remain independent. Foreign input rejects before lookup;
construction/application runs no callback. Graph nodes capture only the callback,
so definitions can be dropped after building; registrations retain no graph state
and prune dead identities on next application. Lookup is linear in registrations.
FlatMap accepts finite IntoIterator, preserving positional duplicate occurrences.
Callbacks remain pure/deterministic; no cross-thread/global registry is added.

Eighteen full upstream node-sharing methods cover all four filter arities plus
unary mapping/flatten-last, with explicit source/self-join setup attribution.
Canonical dataset identity checks node identity. Five original regressions verify
one filter/map/expansion callback per change across multiple scoring consumers,
distinct definitions/parents, cross-builder and cross-score isolation, callback
lifetime, downstream panic rollback, rebuild and duplicate/shrinking expansions.
One private unit checks weak owner cleanup across100 builder lifecycles. No timing
benchmark or automatic Java lambda-equivalence claim accompanies this increment.

Focused79925 passed18 cases;20877 exited0 with one pruning unit and five runtime
regressions (the unit-filtered regression invocation had zero selected tests; the
following unfiltered invocation ran all five). Final gate93494 exited0. Broad
build5454 exited0 in2m28s while the same gate waited on Cargo's build lock; it was
not restarted. Shared graph export edits invalidated325 targets; prebuild affected
shared-API batches before starting their final gate to avoid serial compilation.
Final verification:2855 Rust tests,145 doctests,fmt/Clippy;325 Rust targets ran,
56 reused;46 Python jobs reused. Aggregate exit0 and equal before/after source:
8bfd71ea4c0c425db52c3438deb49b557930090d6c401ccb38fb1c4e00d33094.
Coverage1453/4855(29.928%),pending3120,excluded282,failed/unverified0.
Dashboard freshness85681 exited0 and matched report digest
00110d4f061b9d9f8945f81595510d0681705140c6f119b1ac1c084f291483e8.
Logs:work/shared-transform-cases.log,-pruning.log,-regression.log,-build.log and
-check.log (shared-transform prefix). All processes terminal. No failure, release,
benchmark or browser sweep. Previous turn progressed via d9d3d9e; goal stays active.

## Shared indexed relation definitions

SharedJoin/SharedExistence reuse nodes by definition identity, ordered parent pair
and builder. Clones share registrations; changed parents and fresh definitions
remain distinct. Joins support all five key comparisons and optional post-filtering;
existence/absence uses equality counts unless a predicate requires indexed matching.
with_filter replaces the predicate with a fresh definition identity and preserves
prior clones/nodes. Both parents validate before lookup; construction runs no
callback. Weak owner registrations release graph state and prune on application.
Ordinary builder calls remain independent; unassigned filtering stays explicit.

Forty-five full upstream node-sharing methods cover Uni/Bi/Tri joins and
Uni/Bi/Tri/Quad existence/absence. Native definition and canonical dataset identity
replace Java callback/node identity; exact source methods and setup helpers are
recorded in provenance. Five original regressions compare 2,000 ordered join edits
and 800 existence edits with independent enumeration, plus callback counts, changed
filters/parent order, self-exclusion, panic recovery and rollback. One private unit
checks weak cleanup across 100 builders and foreign-parent validation.
Focused cases passed45; runtime regressions passed5; pruning unit passed1.
No benchmark, release or browser sweep; no new timing ratio claimed.
Final broad build50969 exited0 in2m31s. Gate55802 exited0:2906 Rust tests and
146 doctests;327 Rust targets ran,56 reused;46 Python jobs reused;fmt/Clippy passed.
Aggregate exit0 and equal before/after source verified:
aaf8cd90b36ece12c915601f804674a30f1e2d423387c56769a7b8a9e9235a7c.
Coverage1498/4855(30.855%),pending3075,excluded282,failed/unverified0.
Dashboard freshness24467 exited0 and matched report verification digest
8a765c052736efb60fd1e9dcc57d57ca532cf565b16ceee2d594e2546d615638.
Logs:work/shared-relation-cases.log,-regression.log,-pruning.log,-build.log and
-check.log (shared-relation prefix). All processes terminal. Previous goal turn
progressed via8526c2a; the full rewrite remains active.

## Shared grouping and collection definitions

SharedGroupKeys/SharedGroup/SharedCollect/SharedGroupList/SharedCollectList reuse
complete pipelines by definition identity and parent within a builder. Group keys
may be tuples; collectors may be flat tuples. Lists retain duplicate occurrences
and input-ID order, with persistent immutable snapshots. Empty input emits no row;
final retraction removes a group. The existing weak registration helper is reused;
all five definitions validate foreign parents and retain callbacks without graph
state. StreamPort.same_node exposes cross-payload graph/node identity.

Twenty-four full Uni/Bi/Tri/Quad grouping-identity methods preserve changed parents,
different/same key mappers and collector callbacks, distinct collector kinds and
Quad's two different zero-returning functions. Four setup helpers retain actual
Cartesian self-join topology. Native IDs replace entities; Java reflection and
automatic callback interning are not claimed. Provenance/notices record each case.
Four original regressions cover 600 full-enumeration update/rollback trials across
all five operators with two consumers each, exact callback counts, tuple collectors,
empty removal, mapping/terminal panic recovery, definition/parent/builder isolation,
callback lifetime and retained ordered snapshots. One private unit covers foreign
parents and weak owner cleanup over 100 builders for all five definitions.
Focused cases48937 passed24; regressions54732 passed4. Two regression-only compile
inference errors were corrected before that focused pass.
No benchmark, release or browser sweep; performance ratios are unchanged.
Broad build19202 exited0 in5m04s; other local development processes were consuming
CPU during compilation. First gate85489 exited101 on the new weak-cleanup unit:
its macro retained each output handle until after checking owner release. The
runtime ownership was correct. Added explicit drop(first) inside the test macro;
production code did not change. Other completed targets remain reusable because
the selective resolver excludes cfg(test) items from their inputs.
Final gate68476 exited0:2935 Rust tests,147 doctests,fmt/Clippy passed. One Rust
unit target reran;384 Rust targets and46 Python jobs reused successfully.
Aggregate exit0 and equal before/after source verified:
ad4ed19ac49974e8e26a58031ea41fa4f90cc4df6b294d9a8144e80c5acff036.
Coverage1522/4855(31.349%),pending3051,excluded282,failed/unverified0.
Dashboard freshness43042 exited0 and matched report verification digest
541231edbed8b6fe4baac30b4ccc01632ff2532388f70898a6aa9084f7d94a42.
Logs:work/shared-aggregation-cases.log,-regression.log,-build.log,-check.log and
-final-check.log (shared-aggregation prefix). All processes terminal. Previous
turn progressed via37627db; the full goal remains active.

## Shared distinct and concatenation pipelines

SharedDistinct reuses the distinct occurrence index per definition/parent;
SharedConcat reuses a routing node per definition/ordered parent pair while
preserving branch tags, duplicate rows and both sides of self-concatenation.
SharedPaddedConcat composes two SharedMap definitions with SharedConcat: each
side's mapping remains shared even when paired with different opposite parents.
Both parents validate before any topology changes. Pure mappings run on puts;
retractions use captured outputs. Existing weak registration helpers are reused.
Built nodes own callbacks; definitions retain no graph state and prune dead owners.

Fifty-seven full upstream node-sharing methods add Uni/Bi/Tri expansion,
Bi/Tri/Quad mapping/flatten-last, Bi/Tri flatten and all four arities' distinct,
concatenation and padding assertions. Preserve Tri's two first-fact mappers and
Tri/Quad true/false source-filter choices. Actual Cartesian self-join bases and
adjacent-arity padding sources remain; native IDs/Option/ordered field vectors
replace entities/null/Java tuple overloads. Four setup helpers and each method
have provenance. Precompute remains pending; no automatic interning claim.
Five original runtime tests cover 800 multiset/set differential trials, 200
prefix-preserving tuple expansion/flattening trials, duplicate positional IDs,
one padding call across opposite parents, self-concat, representative updates,
callback failure rollback and shared scoring consumers. One private unit checks
foreign inputs before mutation and weak cleanup over 100 builders.
Focused cases89822 passed57; regressions74262 passed5. A lifetime-elision edit
initially left two undeclared lifetime names; corrected before the regression pass.
No benchmark, release or browser sweep; existing performance evidence unchanged.
Broad build68559 exited0 in2m36s. Gate75468 stopped at Clippy's type_complexity
warning on the three-parameter PhantomData marker; introduced a private type alias
without changing behavior. For subsequent new shared APIs, checking Clippy before
the all-target compilation can catch such declaration issues before code generation.
Final build47769 exited0 in1m32s. Gate71398 exited0:2998 Rust tests,148 doctests,
fmt/Clippy passed;331 Rust targets ran,56 reused;46 Python jobs reused.
Aggregate exit0 and equal before/after source verified:
772be5f1d0f7424f7dae694bec6a8804356e6ec9a39ee060651f9426f2fe2477.
Coverage1579/4855(32.523%),pending2994,excluded282,failed/unverified0.
Dashboard freshness48471 exited0 and matched report verification digest
31b03d4f6ff4ab5b437843b88a9ee9d2f684deb0351e65572cd59a9842e1c758.
Logs:work/shared-set-cases.log,-regression.log,-build.log,-check.log,
-final-build.log and-final-check.log (shared-set prefix). All processes terminal.
Previous goal turn progressed via8ec1ece; the full rewrite remains active.

## Existence sharing with assignment scope and self-exclusion

Seventy-two full upstream node-sharing methods now cover the remaining existence,
absence, Other and IncludingUnassigned identity assertions across four arities.
All execute against existing native operators; no production source changed.
Preserve identity/code and first/second-fact indexers, equality/inequality predicates,
true parent filters, explicit assigned-left and assigned/all-right scopes, and
self-exclusion independent of equality keys. Higher-arity predicates compare the
first two left facts. The literal unary ifNotExistsOtherDifferentParent fixture
starts with ExistsOther and compares a filtered NotExistsOther; preserve both
mode and parent differences rather than silently correcting its name/body mismatch.
Four setup helpers retain Cartesian self-join topology and explicit native assignment
filtering; provenance records each adaptation. Precompute/retrieval cases remain.

Two original regressions exercise all four arities with 400 source edits each,
including assignment/unassignment, code changes, removal and rollback. Each update
compares complete output payloads, cardinalities and scores against independent
Cartesian enumeration for assigned/all right scopes, existence/absence and optional
self-exclusion (eight combinations), with two scoring consumers per shared relation.
A concrete same-definition/different-right-scope case checks exact scores through
assignment changes. Focused cases94951 passed72; regressions40353 passed2 in13.29s.
The runtime helper initially lacked the PartialEq bound required by dataset(); it
was added before the passing run. No engine failure, benchmark or browser sweep.
Final gate70797 exited0:3072 Rust tests;two new Rust targets ran and387 reused;
148-doctest evidence and46 Python jobs reused;fmt/Clippy passed. Aggregate exit0
and equal before/after source verified:
e384cafc49b75ba14752838bfb8ac5ecba58a0436086bcca149b507a91c0cb20.
Coverage1651/4855(34.006%),pending2922,excluded282,failed/unverified0.
Dashboard freshness80382 exited0 and matched report verification digest
b1b00d4d2a7bdd4ee851a1db0de9f69a70433d1d10f5b9b2fbc0f9cdf07eb632.
Logs:work/shared-existence-scope-cases.log,-regression.log and-check.log
(shared-existence-scope prefix). All processes terminal. Previous goal turn
progressed via bbbb157; the full rewrite remains active.

## Fact-key precomputation and live value resolution

PrecomputeNode and builder.precompute cache one output per input ID and explicit
fact key. The cheap key selector runs on every put; computation and downstream
fact-pipeline updates occur only on a new ID or changed key. Equal keys retain the
same cached output object. Retraction releases its entry without callbacks;
reinsertion computes again. Keys at different IDs do not merge rows. Keys must
include all computation dependencies and omit no mutable captured facts. Cached
outputs are immutable fact data/IDs; join back to a live source for current planning
values. No reflection, global invalidation or property-event classification is added.
SharedPrecompute definitions/clones reuse one cache per parent/builder, using the
existing weak registry. Graph checkpoints and source replay integrate the cache
with rollback and diagnostic rebuilds; rebuilds intentionally rerun computation.

Thirteen full upstream methods preserve pure-fact removal, every filter entity
position across four arities, and both unique-pair changed-variable positions.
Real Cartesian fact joins and live identity joins retain all score/match states
through variable changes, fact changes, removal and reinsertion. Generated fixture
cardinalities/modulo membership are retained where consumed; unique-pair fixtures
omit unused fact sources. Typed IDs replace entity/fact identities. Mockito group-
getter assertions become precomputed predicate/index at-least-once/never counters;
the cheap native fact-key selector still runs on each put. Provenance states this
boundary explicitly, with Lavish generation and Tri/Quad helper attribution.
Five original regressions cover600 standalone cache edits and600 graph edits,
retained output identity/release, key/computation/terminal panics, rollback, shared
identity/lifetime, and native basic-variable move scoring with no fact recomputation.

Focused cases41298 passed13; regressions23001 passed5. An initial use of is_some
on SnapshotMap.remove's bool result was corrected. The unary fixture's unused value
and value-group source counts were subsequently aligned to its original defaults.
Early Clippy90502 passed in20.35s before the all-target build. No performance,
release or browser sweep; cached performance evidence remains unchanged.
Final broad build23468 exited0 in1m35s. Gate44967 exited0:3090 Rust tests,
149 doctests,fmt/Clippy;335 Rust targets ran,56 reused;46 Python jobs reused.
Aggregate exit0 and equal before/after source verified:
2069346010e2489aae9f3a29d42bec154cf927757e06238a1310894af4594e64.
Coverage1664/4855(34.274%),pending2909,excluded282,failed/unverified0.
Dashboard freshness64131 exited0 and matched report verification digest
c33eece686e0f3b89e33fd042d74976f9c5bbc2010f5a4ebe702c1693a29ac65.
Logs:work/precompute-filter-cases.log,work/precompute-regression.log,
work/precompute-clippy.log,work/precompute-build.log and work/precompute-check.log.
All processes terminal. Previous turn progressed via4e649e7; full goal stays active.

## Precomputed collection and tuple operators

Forty-two complete upstream precompute methods now exercise existence/absence,
grouping, flatten/flattenLast with fresh ValueHolder allocations, map, concat,
distinct and complement across arities one through four. Together with the
previous thirteen cases, this completes all 55 methods in the four abstract
precompute operator classes. Separate node-sharing/retrieval and Bavet lifecycle
cases remain pending; this is not whole-precompute or whole-solver completion.
Native fact-key caches feed actual joins/collectors/operators; live assigned
entities drive the helper's final existence gate. Preserve initial empty scores,
all final matches, duplicate mapped rows, padded nulls and Tri complement's
trailing distinct. Bi/Tri flatten literally expect entityWithoutGroup in the
second field of the entityWithGroup row. Preserve that source fixture and its
AssertableMatch expected.containsAll(actual) semantics, including missing/excess
checks, field counts and total penalties; do not silently rewrite the expected
row or claim positional/multiplicity equality. All cases and helpers attributed.
Additional native checks deactivate/reactivate assignment gates without fact
recomputation and retain identical Rc holder allocations. One original 600-update
regression compares grouped counts, list flattening and live complement rows
against independent maps after fact/group edits, assignment changes, removal and
trial rollback. No production source changed.

Focused cases 79148 passed 42 after an explicit collect::<Vec<_>>() type annotation
fixed initial inference failure; expanded target 70039 passed 43 including runtime
regression. Final gate 46090 exited 0: 3133 Rust tests, 149-doctest evidence reused,
fmt/Clippy passed; one new Rust target ran, 391 reused; 46 Python jobs reused.
No benchmark/release/browser sweep. Performance evidence remains unchanged.
Aggregate exit 0 and equal before/after source verified:
8052ac917ef02a6b8a50ce2202b73cd6d8a9184cf90df3f2ea96f3ce88f227d2.
Coverage 1706/4855(35.139%), pending 2867, excluded 282, failed/unverified 0.
Dashboard freshness 65485 exited 0 and matched report verification digest
2e002b94730ea8856489c221448a56da086894578ee5b562548d3887c5513568.
Logs:work/precompute-operator-cases.log and work/precompute-operator-check.log.
All processes terminal. Previous timing-only reply made no implementation change;
this goal turn advances verified compatibility. Full goal stays active.

## Precompute sharing and unequal-depth relation updates

Ten complete upstream node-sharing methods preserve same/different precomputed
prefix identity across all four arities and the two unary retrieval-scope checks.
Native explicit shared definitions replace Java factory/callback interning; a
fact-only source cache stays separate from its live source. True/false filters and
Cartesian self-joins retain source topology. Additional runtime assertions insert
two facts and confirm both consumer scores with one computation per Cartesian
tuple. All methods in the four abstract node-sharing classes now have adaptations.
No automatic interning, class discovery or reflection equivalence is claimed.

Six BavetRegression methods preserve indexed/unindexed/mapped predicate flips,
existence/absence counter changes and self-joins with null keys. Immutable entity
snapshots replace mutable aliases; constant-key joins cover unindexed candidates,
and explicit post-join filters retain predicates and downstream value guards.
Identity-map depths and all fixture score states are preserved. Complete row
multisets strengthen source match membership checks, including mapped duplicates.
Every specified predicate flip is compared with a separately built graph and
replayed current sources. One original regression adds 1,800 edits across six
shapes, varied groups/assignments, removal and rollback versus direct enumeration.
This does not claim JVM deferral scheduling or mutable-alias semantics.

Read BavetNodeDeactivationTest from upstream tests only. It asserts whole-network
activity, including backwards deactivation from empty joins and shared live
consumers, not merely empty scores. Native graph currently prunes zero-weight
constraints statically but has no settled activity API or dynamic dead-branch
tracking. All these lifecycle cases remain pending; do not credit empty datasets
as activity assertions. The source test also asserts FILTER-node absence because
Java compiles filters into wrappers; native filters are actual graph nodes.

No production source changed. Initial SharedJoin::equal constructor typo corrected
to new; initial static graph lifetime in the row-reader callback corrected to a
higher-ranked borrowed graph. Focused sharing 46703 passed 10; deep update 95680
passed 7 (including original regression). Final gate 69792 exited 0: 3150 Rust
tests, 149-doctest evidence reused, fmt/Clippy passed; 2 new Rust targets ran,
392 reused; 46 Python jobs reused. No benchmark/release/browser sweep.
Aggregate exit 0 and equal source-before/after verified:
4ab491a85a0a39655313a44989febd0e99fbd004b9b2b08c082fc420304d2d75.
Coverage 1722/4855 (35.469%), pending 2851, excluded 282, failed/unverified 0.
Dashboard freshness 18229 exited 0 and matched report verification digest
e3df33c4fc4f8b0c96c6ba2656e0bb3f5ab4353a6e1232fa7ae8808ebde04f5b.
Logs: work/shared-precompute-cases.log, work/deep-join-update-cases.log and
work/precompute-deep-update-check.log. All processes terminal. Previous goal turn
progressed via f470c69; full goal remains active.

## Opt-in dynamic source-emptiness activity pruning

Added with_activity_pruning, graph.activity and typed immutable StreamActivity /
NodeActivity snapshots. Forward structural input rules and backward observer
reachability distinguish ordinary joins, concat, anti-joins and optional references.
Sources retain facts even when inactive; datasets and shared live consumers keep
branches alive. Routing skips inactive operator callbacks. When source emptiness
changes the active plan, all operator caches rebuild from retained facts. Other
updates remain incremental. Rollback and saved candidates restore the routing plan
with source/index/score state. Default execution is unchanged. See the updated
constraint-stream contract for callback purity, checked rebuild arithmetic,
conservative predicate/key treatment and allocation-identity/rebuild-cost limits.

Ten BavetNodeDeactivationTest methods now have adaptations and helper provenance.
chainOverEmptyClassFullyDeactivates stays pending because its FILTER-node-absence
assertion describes Java compilation; native filters are nodes. Native empty-chain
activity is independently checked. Five original integration tests cover 100 skipped
updates, reactivation, snapshot retention, foreign handles, datasets/disabled weights,
panic and checked-overflow rollback, plus 800 eager/pruned updates across grouped,
joined, anti-joined, concatenated, optional-reference and precomputed branches.
One private unit checks saved candidate installation after activation rollback.
Focused 86132 passed 10 adapted cases and the first 4 regressions; the final added
conservative-filter regression and private unit passed in the full gate.
Final Clippy 54505 passed; broad build 6906 passed in 8m07s. Gate 42879 exited 0:
3166 Rust tests and 150 doctests; fmt/Clippy passed; 340 Rust targets ran, 56 reused;
46 Python jobs reused. No benchmark/release/browser sweep or throughput claim.
Equal source-before/after and aggregate exit 0 verified:
7d31313d5288147e884f63c30a66640a7dea69aff112764920dc07a8668db1f9.
Coverage 1732/4855 (35.675%), pending 2841, excluded 282, failed/unverified 0.
Freshness check 7256 exited 0 and matched the dashboard report digest:
b3e06dc752ecc5a632bc649f9a2b2eb622ac7ec46e6aafb08746465091fb4466.
Logs: work/activity-focused.log, work/activity-final-clippy.log,
work/activity-build.log and work/activity-check.log. Previous goal turn progressed
via 0441d06; full rewrite remains active. All processes are terminal.

## List ownership transitions through deep streams

Twenty-four complete BavetRegression methods now use real optional ListProblem
models and ListStreamScoreDirector binding. Native assign, composite replacement,
unassign and swap moves drive owner/index/previous/next updates through indexed
and unindexed joins, existence/absence, unequal-depth maps, four-way joins, identity
projection/flattening, stable value grouping and owner grouping. Preserve every
explicit assignment and match multiset, mapped duplicates, ten-contributor groups,
and the two-group collected-list fixture. Empty initial assignments, full current
source shadow views and final diagnostic rebuilds add native evidence. Immutable
snapshots replace mutable Java aliases; reverse after-event ordering and JVM
settling/deferral internals are not claimed. All cases/fixtures/helpers attributed.
One original regression checks 2,400 cross-owner moves across 20 shapes, pruning
on/off and trial rollback versus independently maintained lists and enumeration.

No engine source changed. Initial focused 53309 passed 24 cases; explicit independent
assignment expectations and the original regression were then added. Focused 71815
passed 25. Final gate 77130 exited 0: 3191 Rust tests; one new Rust target ran and
396 were reused; 150-doctest evidence and 46 Python jobs reused; fmt/Clippy passed.
Aggregate exit 0 and equal source-before/after verified:
3f9cd59c52477e801333fefd07601beaa4f85dfd571b59972e0b45f35b93f3ed.
Coverage 1756/4855 (36.169%), pending 2817, excluded 282, failed/unverified 0.
Freshness check 17573 exited 0 and matched the report verification digest:
24b1b95c8f8ec344df13357d701b7d337bf91216e8530a451b8b2b2940cfb43b.
Logs: work/list-stream-null-cases.log and work/list-stream-null-check.log.
No benchmark/release/browser sweep. Previous turn progressed via eb3b943;
full rewrite remains active. Remaining BavetRegression methods include six basic
variable null-conflict cases plus mapPlanningEntityChanges, concatSameTupleDeadAndAlive
and clearEvents. Their sources have not been adapted in this batch.
All processes are terminal.

## Basic-variable null transitions and concat recovery

Eight BavetRegression methods now execute actual required-variable moves through
self joins, existence/absence, separate assigned-filter paths, identity mapping
and concat. All original states, predicates, non-null guards, score/match sets and
duplicate occurrences are preserved. Current payloads and native structural scores
are checked separately. The concat undo state remains unobserved as upstream;
native eager updates do not claim Java deferred queue or mutable-alias semantics.
An original regression enumerates all 64 three-entity assignments forward/backward
across eight shapes and both activity modes (2048 trials), with rollback and
independent enumeration. Composite move input order reverses on the second pass;
internal native delivery remains sorted by touched variable.

Focused 69611 passed 9 tests. Final gate 84639 exited 0: 3200 Rust tests; one new
Rust target ran, 397 reused; 150-doctest and 46 Python job evidence reused; fmt and
Clippy passed. Equal source-before/after and aggregate exit 0 verified:
059d5dbe6da01ce9a915ff934448854c0ddaae74f5a0748adb01d62c627f0dbf.
Coverage 1764/4855 (36.334%), pending 2809, excluded 282, failed/unverified 0.
Dashboard freshness matched report digest:
9de382c6ed2d698d7fb0f1b821adf8e7a328c1fd1c091637783bc2f75c39c571.
Logs: work/basic-stream-null-cases.log, work/basic-stream-null-check.log.
No engine changes, benchmark/release/browser sweep. All processes terminal.
The preceding timing-answer turn made no implementation progress; this turn
revalidated the worktree and completed the next cases. Full goal remains active.

BavetRegression clearEvents remains pending. Read its test and testdomain helpers:
TestdataListMultipleShadowVariableSolution.generateSolution(2,1) assigns only
value0 to entity0; value1 remains unassigned. Both cascade fields start null;
getCascadeValue returns 2 for null. updateCascadeValue sets index+10. The test
loads without shadow initialization, clears pending shadow updates, and checks
both getters remain 2. ListCascadeCache.from_values can import uninitialized
shadows but has no deferred update queue; new/update/apply evaluate eagerly.
NodeOrderQueue.clear exists as a generic scheduling primitive, not a score-director
shadow cancellation operation. Do not credit static getters or eager execution
as clearEvents compatibility. No upstream production implementation was read.

## Deferred cascade initialization, batching and cancellation

ListCascadeCache now supports new_deferred, queue_update, queue_move,
pending_updates, clear_pending_updates and flush_pending. Queueing installs
validated latest assignment/position metadata without running callbacks; roots
accumulate and deduplicate across edits. Flushing evaluates latest list order
with existing change-sensitive propagation. Eager update/apply consume pending
roots too. Clone and move rollback retain queue, assignment and shadows, without
requiring Clone on stored shadows. Invalid input and callback panic retain state
for retry. Clearing preserves latest metadata and stored values, which may stay
stale/uninitialized; explicitly dirty all values to repair. Standard list shadows
and stream updates remain eager. Separate cache/graph updates are not automatically
one transaction; the contract and integration regression make this explicit.

BavetRegression clearEvents now has a real deferred initialization/cancellation
adaptation preserving both null fields and getter fallback 2. Actual pending
roots are checked before cancellation; a forbidden-callback flush proves it.
Source constructs but never evaluates/asserts a score; native cascade cache
replaces listener lifecycle without claiming general Java event ordering. Test
and fixture provenance added. All 39 BavetRegression methods now pass adaptations.
Seven original regressions cover 512 queued structural states in 128 flush batches
against independent list folds, cancellation/repair, retry, deduplication, snapshots,
non-Clone values, dropped/committed deferred and eager trials, and feeding changed
shadows into incremental scoring. All prior cascade cases/regressions pass.

Focused 81015 passed 4 existing cases/6 regressions; 91467 passed 1 new adaptation
and 7 originals. Clippy 43970 passed (15.27s). Gate 40139 exited 0:
3208 Rust tests and 151 doctests; 5 Rust targets ran, 395 reused; 46 Python jobs reused.
No broad prebuild: only cascade implementation changed, and the selective gate
reran lib plus all four cascade targets. No benchmark/release/browser sweep.
Aggregate exit 0 and equal source-before/after verified:
1bab5ea512318395063a2fafb94f23148bf02cf1d1dcdddf854ac899ff75d67a.
Coverage 1765/4855 (36.354%), pending 2808, excluded 282, failed/unverified 0.
Dashboard freshness matched verification digest:
5704600fb14767132599a82274ec2dd32f485c317fc26df74b87871f28c6ba02.
Logs: work/deferred-cascade-existing.log, work/deferred-cascade-focused.log,
work/deferred-cascade-clippy.log, work/deferred-cascade-check.log.
All processes terminal. Previous goal turn progressed via 1a9bc74; full goal active.

Next useful implementation batch: all 20 pending methods in Community
AbstractAdvancedGroupByConstraintStreamTest. Only test-source lines 1-225 have been
read so far: nested group/collect/filter, unique pairs followed by group counts
and recollection into lists/maps/sets, incremental entity removals. Finish reading
the methods and fixture/helper specifications before adapting. Also pending uni
stream methods include joinAfterGroupBy, filtered grouping, mapToBi/Tri/Quad and
flattenLastWithDuplicates; unsupported Java descriptor cases need separate scope.

## Advanced grouping, recollection and shared joins

All 20 AbstractAdvancedGroupByConstraintStreamTest methods now have complete
native adaptations, preserving fixture counts, codes/group strings, all weighted
matches, nested collection contents and incremental removals. Actual native
unique pairs expand through three/four fields, with all distinct-entity guards
and original multiplicities (3 tri or 12 quad per full group). Key-only grouping
is map/distinct. Global list/map/set/sum collectors and successive groups operate
on live output updates. Existence-before/after grouping removes group facts
without removing entities. PairExists intentionally pairs across groups, then gates
by first-entity group. SharedJoin reuses one filtered port as both inputs.
The construction-only case remains a real build assertion; extra regression
executes it on populated streams. Typed match fields normalize unordered source
justifications while preserving duplicates and nested list order. FilteredPairs
keeps the source literal (MyEntity3, GeneratedEntity0) expectation; its membership
helper accepts native reverse pair field order, while the first-entity predicate
is preserved. Fixture, getter and assertion-helper provenance recorded.

One original 3600-trial check spans all 20 shapes in both activity modes, checks
insertions/removals, group/value/code changes, group-fact gating, empty sources and
rollback against independent counts/sets/combination formulas (no rebuilt oracle).
Focused 46160 passed the first 20 cases; 41267 passed final 21 including original
regression (0.58s). Gate 53634 exited 0: 3229 Rust tests; 1 new Rust target ran,
400 reused; 151-doctest and 46 Python job evidence reused; fmt/Clippy passed.
No production changes, benchmark/release/browser sweep. Equal source-before/after
and aggregate exit 0 verified:
26608dc10b199f47a12b0be5b95ef4cf46bc146b83ec9d107a99aede1bdeb33a.
Coverage 1785/4855 (36.766%), pending 2788, excluded 282, failed/unverified 0.
Dashboard freshness matched report digest:
2f26d2b5154ec52db979090ab16dd74d83b7f62a5e75bf056f8420d6a228e3b0.
Logs: work/advanced-grouping-focused.log, work/advanced-grouping-check.log.
All processes terminal. Previous goal turn progressed via 2e12204; full goal active.

Next batch: remaining uni stream operator cases. Located source methods in
AbstractUniConstraintStreamTest: joinAfterGroupBy line390; filtered group cases
1390,1414,1440 (read 1386-1478 already); primitive grouping1712; mapToBi/Tri/Quad
2145,2177,2212; flattenLastWithDuplicates2344. Read full cases before adapting.
The three filtered methods have initial-only assertions: groups0/1 counts8/2;
filter group1 yields one group or two joined entities. No incremental assertions
in those source methods. Corresponding bi/tri/quad cases may support a wider batch.

## Canonical expansion and cross-arity projection coverage

Added FlatMapNode::new_canonical and StreamGraphBuilder::flat_map_canonical in
canonical_flat_map.rs. They expand using an explicit equivalence key, replacing
each equal-key payload by a clone of its expansion's first representative while
retaining all positional occurrences. Rc/Arc clones retain actual pointer identity.
Selection is per input expansion and refreshed on each replacement; no shared
cache or cross-update allocation identity. Ordinary flat_map behavior is unchanged.
A local BTreeMap gathers representatives inside the callback; existing flat-map
state and graph checkpoints handle atomicity/rollback. Keys must include any
varying retained prefixes. O(n log d) mapping work; no timing claim.

Twenty-one Community methods adapted: mapToBi/Tri/Quad for all four input arities
(12); joinAfterGroupBy for uni/bi/tri collectors (3); three filtered-group methods
and primitive-key grouping (4); uni/bi flattenLastWithDuplicates (2). Real joins,
unique pairs and all three distinct-count collectors preserve the original graph.
Native exact match fields retain duplicates, normalized for source unordered
justification membership. Deterministic long properties 11/23/37 replace original
random values within their allowed range; expected sums 34/48/60/71 are literal.

Uni duplicate fixture retains separate equal-value Rc allocations and actual
Rc::ptr_eq guards; canonical expansion is necessary for all 5 initial matches.
Bi duplicate fixture has an important source-helper limitation: literal expected
counts by entity/group are 4/2 and 1/2, while expansion of b.group produces 3/3
and 2/1. Source assertScore checks bidirectional justification membership and total
score/count, not per-justification frequency. Preserve its literal lists and all
helper assertions; separately check exact actual native multiset, including after
removal. Do not claim equality of those literal duplicate frequencies or rewrite
the mapper to force them. This nuance is in provenance and compatibility docs.

Four original canonical regressions cover per-input/per-update representative
identity, positional reordering, no callbacks on retract, key panic before state
installation, checked-overflow rollback and 320 graph trials with both activity
modes. Focused 2005 passed 4. Initial projection compile 96246 failed because an
imported Field::Entity shadowed the Entity struct; qualify self::Entity. Focused
87752 passed all 21 cases. Clippy 43032 passed in 27.61s. Gate 3710 exited 0. Adding the
public StreamGraphBuilder extension invalidated broad stream inputs; build 83804
was launched after this became visible and completed in 1m38s, allowing the existing
gate to use compiled targets. No gate restart or repeat test execution.
Final evidence: 3254 Rust tests and 152 doctests; 347 Rust targets ran,
56 reused; 46 Python jobs reused. Aggregate exit 0 and equal source-before/after:
cf5118c9c5c1dc7983a0bdb0449612a9763d854f67ce84d23d4e3f331255638d.
Coverage 1806/4855 (37.199%), pending 2767, excluded 282, failed/unverified 0.
Dashboard freshness matched report digest:
fdfc55adb5f72df7da4880014f5a5b7b1f090eacaf68af2bb6445b00ec745ad3.
Logs: work/stream-projection-canonical.log, work/stream-projection-focused.log,
work/stream-projection-clippy.log, work/stream-projection-build.log,
work/stream-projection-check.log. No benchmark/release/browser sweep.
All processes terminal. Previous goal turn progressed via 8237b22; full goal active.

Next useful broad batch: pending weighted/unweighted penalty/reward/impact tests
and default/custom justification cases across bi/tri/quad (and any uni pending).
Read only Bi source 2738-2821 and 2970-3058 so far. Bi default 7 entities generate 21
unique pairs: penalties -21 or -42. Helpers assert a sole named constraint, 21 matches
and justification type/two facts for the first 7 matches. Custom justification retains
score and pair facts. Existing score_explanation.rs exposes explain_constraint and
explain_constraint_default (lines 142-225 read), and graph score_weighted/decimal
exist; use actual native explanation APIs, not stand-in constants. Read all chosen
methods/helpers/decimal fixtures first. Multiple-registration errors or lifecycle
requirements may need new support and must not be silently reduced to snapshots.

## Cross-arity weighted scoring and explanations

Added tuple_score_explanation_cases.rs: 69 Community methods (23 each Bi/Tri/Quad)
cover integer/exact-decimal penalties, rewards and signed impacts with default and
custom justifications. Actual weighted terminals use weight ONE and signed match
factors. All 21 Bi unique pairs are retained; Tri/Quad select the two equal-value
pairs (0,5)/(1,6) and join actual value facts once/twice. Literal totals are passed
independently (-21/-42/+21/+42 Bi, -2/-4/+2/+4 Tri/Quad), never derived from graph
output. Every explanation's score and ordered facts are checked, strengthening
upstream type/count helpers. Quad retains repeated values in justification but
counts each once in indictments. All fixture/helper provenance recorded. Native
on-demand explanation differs from Java build-time justifyWith registration;
failWithMultipleJustifications remains pending. No production source read/ported.

Focused process19113 passed all69. Final selective gate39553 exited0: one Rust
integration target ran,403 reused;46 Python jobs reused;152 doctests reused.
3323 Rust tests in aggregate. Equal source-before/after and exit0:
3e7094516efc276d826d7a99360bd1045fd75276ce904beadefcaea195628a02.
Dashboard1875/4855 (38.62%),2698 pending,282 excluded,0 failed/unverified.
Freshness process5483 exited0 with matching report digest:
c7c20f421f4a391de2dd971530d7fda28d5f6d8aef0b1aa7971a169f25c87d50.
Logs: work/tuple-score-focused.log, work/tuple-score-check.log.
No engine changes, benchmark refresh, release rerun or browser sweep. All processes
terminal; reuse evidence until relevant inputs change. Full goal remains active.

Next useful shift: end-to-end DefaultSolverTest adaptations, not more scoring
wrappers. 82 methods still pending; native solver already supports basic/list/mixed
phase plans, repeated solves, pins and optional assignment. Read test sources and
fixtures before adapting. Inspected solveRepeatedlyBasicVariable749-779 (5 required
variables,2 values,construction max2steps per solve; remaining5/3/1 ->0), and
solveRepeatedlyListVariable782-817 (24values,8owners,max7construction steps per solve;
4 resumes to full assignment). Source initial score is nonnull; reproduce actual
resumed assignments, not fresh restarts. Mixed source starts1269; its shared helper
needs further reading from1290 onward. PinnedMixed1494-1513 keeps first of3entities'
two basic variables null and list empty with2+2values. UnassignedMixed1516-1539 blocks
all2+2values, expecting optional basic null, required second basic assigned, list
empty. PinnedAndUnassigned1542 onward has two scenarios; read remainder after1586
and all corresponding test score calculators/fixtures. Do not infer solver quality
or replace source constraints with easy stand-ins. Deterministic seeds/limits needed.


## Resuming solutions and required unassigned pins

Added with_initial_assignment to finite/list/mixed models. It validates against
original domains/pins before returning an independent model with the new initial
assignment. All metadata survives (including entity ranges and pillar schema).
Search history, random generators and budgets restart; only solution state resumes.
Required null pins are now legal partial states. Removed obsolete PinnedUnassigned
error/constructor rejection; revised the original boundary assertion accordingly.
search_space_size returns zero, including before overflow elsewhere. Construction
fills other movable variables, keeping structural penalties and incomplete status;
empty movable required domains still stop before scoring. Exact search cannot
report a complete or optimal result for a required null pin.

Five DefaultSolverTest methods adapted in solver_resume_cases.rs: repeated basic
(5variables/2values/max2construction steps) and list (24values/8owners/max7steps)
solves, pinned mixed, optional mixed, and both pinned-and-optional mixed scenarios.
Actual test calculator formulas and best-event helper assertions retained. Mixed
optional construction uses NonWorsening; local search uses deterministic original
move order and max16steps. This is native behavioral coverage, not reflection,
shadow lifecycle, default-phase or identical stochastic-trace parity. Three original
regressions cover metadata preservation, rejected pins/ranges/duplicates, required
null pin construction in scoped/stream backends, exact-search rejection, optional
null pins and empty movable domains. No production Timefold source inspected.

Environment changed to Windows during the focused build. Continue via
wsl.exe -d Ralph -u ralph, repo /home/ralph/ralph-repos/TimeTuck; the displayed
C:/home/... path does not exist. Prior linker failed with I/O error during the
transition and cargo/rustc were no longer live. Retried focused suite successfully:
8 boundary +5 adapted +3 original tests passed. rg is unavailable in this resumed
WSL environment; use grep/find fallback. Dashboard server restored at localhost8765
(Python PID25340), same existing static UI, no rendering changes.
Clippy session35053 passed (11.12s). Broad no-run build5038 passed (1m57s).
Final gate85083 is recorded below after completion; do not restart a live handle.

Gate85083 completed with three obsolete-contract failures: two independent move
enumerations passed every successor comparison but their old total counts excluded
required null pins; the protocol test expected constructor rejection. Both enumeration
loops now require every model to construct and assert independently derived counts
13608/25704; all existing successor/rollback comparisons remain. The protocol now
explicitly checks accepted partial input has no complete/exhaustive result. No engine
change was needed after the focused pass. An earlier commentary prematurely said all
Rust checks passed; corrected immediately after inspecting the terminal aggregate.
Final gate56742 exited0. Only the three changed Rust targets reran;403 successful
Rust targets reused.46 Python jobs and doctests ran. Aggregate3332 Rust tests and
152 doctests passed; source-before/after equal:
ec180c8a6059f9f6f47debcb256d9fa7e064accbe7c3bc37a4a8f06f5a3179dd.
Dashboard1880/4855 (38.723%),2693 pending,282 excluded,0 failed/unverified.
Live freshness matched report digest:
fc9ab07cbbabe5ea733ef4f2baeffd5d3486c0a4b482cb8b868aa4a02422af09.
Logs: work/solver-resume-focused.log, work/solver-resume-clippy.log,
work/solver-resume-build.log, work/solver-resume-check.log (initial failures),
work/solver-resume-final-check.log (successful). All verification processes terminal.
No benchmark refresh/release rerun/UI rendering change. Whole goal remains active.

Next: remaining end-to-end mixed solve cases. DefaultSolverTest.solveMixedModel
requires real shadow integration, not just assignment/score assertions: its helper
1290-1332 checks list cascade=index+1, basic-entity declarative=strength,
inverse basic-value declarative=entityCount+2, list declarative=index+2. Read actual
TestdataMixedEntity/Value/OtherValue fixtures before implementing. Default-provider
mixed case240-265 uses TestingMixedEasyScoreCalculator, reverse basic assignment and
two values initially on owner0, target ordinary score0; calculator still unread.
Do not credit either method using only the easier pinned/optional fixtures above.


## End-to-end mixed solves and derived outputs

Added mixed_solver_shadow_cases.rs with four DefaultSolverTest adaptations:
solveMixedModel (all four fixture sizes), solveWithDefaultNeighborhoodProviderMixedModel,
solveMixedModelCustomMove and solveMixedModelCustomPhase. Exact test objectives,
initial assignments, best-event helper, actual custom callbacks/proposals, complete
basics/list assertions and all four derived outputs preserved. Dataset maps compute
first-basic strength and list index+1/index+2; grouped counts plus complement retain
inverse-count+2 for every basic value, including unused ones. This fixture's cascade
is only an index formula, not recursive. Solve results remain assignment+score;
final output is materialized in a fresh director with the same factory. Do not claim
Java mutable-solution shadow listener lifecycle or automatic metadata discovery.

The original regression performs480 combined basic/list trials with pruning on/off,
including required null transitions, whole-list unassignment and every third trial
rolled back. Independent assignment arithmetic checks all scores and shadow outputs.
Custom source factory emits entity0 first-basic changes to0 then1; its random argument
is unused. The source command executes only its first proposal. Native callbacks are
asserted invoked; construction combines basic-then-list into one native phase.
No engine changes or performance claims. Provenance includes all source helpers.
Focused5tests passed. Initial final gate stopped at Clippy field-reassign-with-default;
replaced it with direct initialization. Successful final evidence follows below.

Final gate95377 exited0:3337 Rust tests;152 doctests reused. One Rust target ran,
406 reused;46 Python jobs reused. Equal source-before/after and aggregate exit0:
5e5ee1ef9c34c53ecfff6492e918fc6f3933f37ee99bf9f7ec895fdd245a0806.
Dashboard1884/4855 (38.805%),2689 pending,282 excluded,0 failed/unverified.
Freshness42136 exited0 with matching report digest:
b7e9fbe4f11dd6359b1f5db8495f96ce67dd7abe3736721962c1f71d89c5497f.
Logs: work/mixed-solver-shadow-focused.log, work/mixed-solver-shadow-check.log
(Clippy failure only), work/mixed-solver-shadow-final-check.log (success).
All processes terminal; no new benchmark/release/browser verification needed.
Full goal active. Next pending mixed cases include multientity, multiple value
providers and sorter manners; their fixtures/assertions need full reading before
adapting. Do not reuse these single-entity tests as credit for those methods.


## Entity range registries and multiple entity types

Added ListProblem::from_entity_ranges: requires every entity to provide an explicit
range, builds a first-encounter Eq union, and delegates assignment/pin validation
to the existing constructor. Empty ranges are valid; equal values must represent
the same planning identity. No new Hash/Ord requirement or performance claim.
Original regressions cover malformed inputs, pinned/resumed metadata and all256
pairs of subsets of a four-value universe, including actual construction solves.

Four DefaultSolverTest adaptations cover solveMultiEntityMixedModel (all four
fixture sizes, distinct basic/list types, actual ascending-strength first-basic
ordering), solveMixedModelWithMultipleProviders (entity-range union), solveMultiEntity
(lead references and ordinary values in separate enum namespaces) and
solveListVarEntityRangeModelWithTreeSet (actual BTreeSet ranges, optional lists).
Exact source objectives and result/event assertions retained. Native explicit
models replace reflection; deterministic bounded search replaces source defaults.
No production implementation inspected. Provenance adds four cases and16 helpers.
Focused7tests, Clippy and all-target no-run build passed. Final gate8965 exited0.
3344 Rust tests and153 doctests passed.353 Rust jobs ran,56 reused;46 Python
jobs reused. Source-before/after equal:
1321f28ba6d1da4d13f4634742f46049a99afd4cac0744084043d7aabef24a94.
Dashboard1888/4855 (38.888%),2685pending,282excluded,0failed/unverified.
Live freshness matched verification digest:
f787d7a27af417cc93cbcab4d028e2c43501405bf5e15c09cb0d94aee2c480a4.
Logs work/entity-registry-{focused,clippy,build,check}.log; all processes terminal.
No benchmark/release/UI reruns. Whole goal remains active.

Next: entity-range parameterized solves need complete selector coverage. The list
methods use generateMovesForListVarEntityRangeModel, NOT generateMovesForListVar:
nine variants (change random/original, swap random/original, sublist change/swap,
k-opt, ruin/recreate, union of all). Helper2134-2179 and source calculator/entity/
solution fixtures fully read. Optional empty lists, ranges[v1,v2],[v2,v3],[v4,v5];
score per owner=2 for length>=2 else1. Source requires every final list nonempty,
owner ranges respected and best events. One/two local phases, each1000move limit.
Native NeighborhoodUnion presently covers ordinary Neighborhood variants; inspect
actual ruin/recreate integration before claiming union fidelity. Leave these two
methods pending until all variants are implemented/exercised.


## Rich neighborhoods in repair search

RuinRecreateConfig::additional_neighborhoods adds weighted existing move families
alongside legacy change/swap/repair. Shared current state, acceptance history, best
and budgets apply to all families. Zero weights skip validation/enumeration; empty
families retire for the attempt. Reservoir sampling preserves duplicate candidates;
sampled k-opt seeds advance by total attempt index. Empty additions preserve legacy
seeded traces. Basic/list/mixed adapters dispatch existing neighborhood rules; mixed
moves are independent components, not joint products. Enabled settings validate
before scoring both directly and in later explicit solver phases. No production
implementation read and no performance claim.

entity_range_search_cases.rs covers four DefaultSolverTest methods: single/multiple
local-search phases for basic/list entity ranges. All seven basic and nine list
configurations are run (32 configuration/phase-count combinations). Exact source
ranges, optional null/empty initial states, score formulas, excluded-value/nonempty
list assertions and actual best events retained. Pillar changes and swaps use
separate native subpillar configurations, explicitly disabling the other move type.
Original/random variants are distinct standalone phases; union order duplicates
are separately weighted families with uniform within-family sampling, not Java
selector trace parity. Multi-list k-opt uses bounded seeded proposals. Some families
can be empty for these small source fixtures; original regression forces richer
list families on a larger pinned model.

repair_neighborhoods.rs checks exact legacy swap results across all six model/backend
paths, rich-family successor legality and pins, budget prefixes0..23, repeatability,
disabled/invalid/empty families and pre-score sequence validation. Initial test used
unlike constraint granularities; its scoped constraints now match graph rows. Five
focused tests passed before adding the two basic cases. The final gate covers the
new basic cases. Initial Clippy found needless_range_loop in the nine-variant harness;
fixed the loop without removing cases. Clippy retry passed; broad no-run build92590
exited0 (3m27s). Final gate89710 exited0, log work/repair-neighborhood-final-check.log. Other logs:
work/repair-neighborhood-{focused,focused2,clippy,clippy2,build,check}.log.
Full goal remains active. Larger mixed-selector configurations remain pending.

Final evidence: 3351 Rust tests and153 doctests passed. Rust jobs {'RUN': 355, 'REUSE': 56};
Python jobs {'REUSE': 46}. Source-before/after equal:
890cdf3e99fff1626d4c41ac7e517e2c50ff7dca47896dc1cf28ce92d113c995.
Dashboard {'total': 4855, 'passed': 1892, 'failed': 0, 'pending': 2681, 'excluded': 282, 'unverified': 0, 'percent': 38.97}; live freshness matched digest:
5d7cb8dd76d1e11f28d2a6782c34083b60d2739f5ef3453858f8dd6ed1eb49f0.
All verification processes terminal; no benchmark/release/browser reruns.


## Independent mixed neighborhoods

Added MixedNeighborhood, MixedNeighborhoodError, mixed_moves_configured and
mixed_moves_configured_union in mixed_neighborhood.rs, exported at crate root.
Basic/list families independently enabled; selected basic slots define order and
filter all touched slots, preserving whole-row pillar semantics. Joint products
follow singles. Weighted union children independently scope/family-select; zero
weights skip configuration validation and enumeration, duplicate occurrences stay.
Assignment validation always runs. Supplied step advances union and sampled k-opt
seeds. API feeds existing lazy MoveProvider bindings, no search-engine changes.

Original mixed_families.rs checks7,128 small states/configurations against separately
constructed basic moves plus existing list enumeration; applies every move and
validates successor. Rich-family pin/excluded-slot checks, typed selection errors,
disabled/empty families, multiplicity/reproducibility, and a coupled scoring trap
where only a joint basic-change/list-swap improves. Both scoped/stream local search
and full solver plans preserve an excluded basic slot. Initial focused compile
caught missing trait lifetime bound and wrong graph join name; fixed. Four focused
tests passed including the expanded full-plan checks; broad build36002 exited0.
Final gate76253 exited0: work/mixed-families-final-check.log. Logs work/mixed-families-{check,focused,focused2,focused3,build}.log.
No provenance additions: all tests original, no new upstream credit yet.

Next actual gaps for DefaultSolverTest.solveMoveConfigMixedModel and
solveMultiEntityMoveConfigMixedModel: their twelve configurations include
basicValue-only ruin/recreate and list-only ruin/recreate in the combined union.
Current repair engine samples a joint movable pool and applies fine Neighborhoods
to both components. Need explicit scoped repair pools and independently scoped fine
families, not just whole-component filtering after repair. Read generators1858-1960
and multi-entity helper1963 onward; first generator/helper fully read this turn.
Source pillar-change selects basicValue only; default swap may use both basics.
Use native pillar schemas where two basic columns are one entity row. Do not
credit these methods using generic unscoped repair or missing configurations.

Final evidence: 3355 Rust tests and154 doctests passed. Rust jobs{'RUN': 356, 'REUSE': 56};
Python jobs{'REUSE': 46}. Source-before/after equal:
7bc35219fb8b462c45a21d6788a13fa1a4a49cbd280fac7631c78fcd07ae2caa.
Upstream coverage unchanged1892/4855 (38.97%); live freshness matched digest:
1753dd7a3b6719eb544aa6ffada996db042c326bb8150cd4b431af02654a976b.
All processes terminal. No benchmark, release or browser rerun. Goal remains active.


## Scoped repair pools and independent mixed fine families

Added RuinRecreateConfig.ruin_candidates and repair_pools. Identity indices are
basic slots / list registry indices / mixed basic slots followed by offset list
registry indices. Explicit pool weights multiply repair weight using u128 totals.
One repair chooses one pool; global candidate filter intersects each pool. Eligible
pins/ranges still apply, unselected basics freeze and unselected list owners/relative
order persist. Zero-weight pools skip validation. Empty candidate list disables
repair; empty pool list yields no repair candidates. Fine moves remain independent.
Default None preserves original random trace and legacy change/swap families.

additional_neighborhoods now accepts RepairNeighborhood (Standard via .into(), or
Mixed(MixedNeighborhood)). Updated the two existing test callers; all previous11
focused cases passed. Mixed families reject explicitly on nonmixed models. Model-aware
phase validation checks scopes/families before construction or graph callbacks.
MixedNeighborhood validation and at_step are now pub(crate) for shared dispatch.
No production source read, no new upstream provenance/coverage claims.

Original repair_scopes.rs covers all16 mixed subsets against exact expected changed
identities, scoped/stream result/counter equality, budget prefixes, reversed scope
order, weighted basic/list pools that never cross scopes within one repair, separate
fine families, invalid indices before scoring, full phase/global-budget handoffs,
standalone basic/list backends, empty/disabled/intersected pools. Focused6tests and all-target no-run build57922 passed. Added a seventh regression
for maximum pool-weight products (u128 totals) without crossing repair scopes;
final gate60527 covers it, work/repair-scope-final-check.log.
Logs work/repair-scope-{check,families-check,focused,focused2,build}.log. Initial gate60527 exited1: the new maximum-weight test incorrectly required list
order[10,20], while both[10,20] and[20,10] have equal score. Corrected to assert sorted
membership and model validity; scope checks retained. No engine fix needed. The
selective follow-up gate reuses other successful jobs. Only tests/implementation changed; dashboard rendering unchanged.

Remaining before crediting DefaultSolverTest.solveMoveConfigMixedModel and
solveMultiEntityMoveConfigMixedModel: per-family pillar schemas. Source pillar
change targets basicValue only (group by that column); pillar swap selects both
basic columns of an entity. Current model-global PillarSchema cannot represent both
schemas in one mixed union. MixedNeighborhood.basic_variables filters moves but
does not override the grouping schema, so do not claim these methods by using one
shared schema. Source generators/helpers already read; multi-entity fixture initial
strength/comparator and objectives available in multi_entity_solver_cases.rs with
provenance. Native pillar_schema::ordered currently takes its schema from model;
consider explicit schema input for configured families instead of cloning models.
Whole rewrite goal remains active.

Implementation pointer for next grouping work: pillar_schema::ordered currently
clones Arc rows from p.pillar_schema() at entry (line136). An internal explicit-schema
entry can reuse the same iterator body without cloning models. SubPillarConfig
already independently disables changes/swaps and bounds selected entity subsets.
Also verify ordinary source SwapMoveSelector defaults for multiple basic variables:
native flat-slot Swap differs from swapping aligned two-column entity rows. A
schema with two columns plus swap-only singleton subpillars can express row swaps.
Do not silently substitute cross-column slot swaps for an entity-row selector.

Final gate54459 exited0: 3362 Rust tests and154 doctests passed.
Rust jobs{'REUSE': 412, 'RUN': 1}; Python jobs{'REUSE': 46}. All other successful Rust results reused after
the failed order assertion was corrected; no engine change in that retry.
Equal source-before/after: ad6c9fd995541c52ae4ae9c61235bd2fe327400a62059daa9f462ecbb8deba15.
Dashboard1892/4855 (38.97%); live freshness matched digest:
aa45604712a64180b88b540e1f4976423e530f4d951739cafac4ca73d00ae8fd.
Logs work/repair-scope-final-check.log (one test assertion failure),
work/repair-scope-final-check2.log (success). All processes terminal.
No benchmark/release/browser reruns; full rewrite goal remains active.


## Per-family pillar schemas and complete mixed selector adaptations

MixedNeighborhood.pillar_schema now overrides the model schema for one enabled
Pillar/SubPillar basic family. Other enabled basic families reject it; disabled
basic skips override validation. Refactored internal pillar enumeration to accept
a borrowed schema directly; existing model-based entry retains behavior. Both
configured unions and RepairNeighborhood::Mixed carry it through shared search.

mixed_pillar_schemas.rs has 3 original tests:256 full binary states/pin/range variants
with independent column-change/aligned-row-swap successors, model-schema retention,
union occurrence counts, invalid schemas/kinds before scoring/graph setup, and
full scoped/stream plans where a first-column whole pillar crosses a local barrier
that model-global row grouping cannot cross.

mixed_selector_solver_cases.rs adapts DefaultSolverTest.solveMoveConfigMixedModel
and solveMultiEntityMoveConfigMixedModel, each with all 12 source configurations.
Basic change allslots; ordinary swap = aligned row singleton subpillars; pillar
change = first-column schema and no swaps; pillar swap = two-column schema and
no changes; basic repair = first-column candidates; list change/swap/sublistchange/
sublist swap/repartition kopt; list repair = offset list IDs; union =9 fine children
and 2 distinct repair pools. Equal weights, seed 20261006, search 40 attempts/candidate
scores. Native bounded sampling/default acceptance differs from Java default
traces, disclosed. Source tiny fixtures may have empty swap/kopt families.
Exact source score formulas/initialstates, basic-then-list vslist-then-basic order,
all result assertions and best events retained. Multientity ascending strength
firstdomain versus encounter seconddomain produces required differing values.
Combined native construction phase replaces two source phases; local budget is
phase-specific, not construction-inclusive. Added 2 case and 13 fixture/helper records.

Source test methods/generators 1854-1999 read; multivariable SwapMoveSelectorTest
multiVarRandomSelectionWithEntityValueRange read to confirm aligned swaps and all
selected-column domain checks. No production implementation read or adapted.
Focused first compile failed due to moving a non-Copy MixedFact pair; replaced by
separate pattern matches. Focused 65178 exit 1 then 81252 exit 0, all 5 new tests pass.
Logs work/mixed-pillars-focused{,2}.log. All-target build 49925 pending at writing,
work/mixed-pillars-build.log. Full check/dashboard verification follows.

Next candidates read from DefaultSolverTest 1088-1270: solveSingleVarMoveConfig
(6 variants: change/swap/pillarchange/pillarswap/repair/union), solveListVarMoveConfig
(6 variants: listchange/listswap/sublistchange/sublist swap/repair/union),
solveMultiVarMoveConfig(5 variants: change/alignedswap/primary-column pillarchange/
rowpillarswap/union), solveMultiEntityMoveConfig(5 variants: change/swap/lead-only
pillarchange/lead-onlypillarswap/union). Source basic/list 2 x 2 fixtures; multiVar needs
read actual entity/solution fixture (not yet read). Last case has lead then herd
construction and scoretarget 0, so search may never execute; preserve actual source
behavior and don't pretend that case alone proves its selectors execute. All use
search move limit 40 and dummyzeroobjective, actual helper nonemptybest events;
multivar TRACKED_FULL_ASSERT has native score assertions but no Java environment
mode parity. These methods are still pending. Do not inspect production sources.


Final evidence: all-target build49925 exited0. Gate56091 exited0, log
work/mixed-pillars-final-check.log:3367Rust tests,154doctests passed. Rust jobs359ran,
56reused; all46Python jobs reused. Test metadata exit0 and equal before/after source:
7af8e29e48ddd433d5aad78e5a1c47f70a891c4e8385ff668aa2eaf9a126edbe.
Report1894/4855 (39.011%),2679pending,282excluded,0failed/unverified.
Live /api/freshness matched verification digest:
747a12fc5db818136b86a5bd39f1a5ee2e955b73063b858bf1c7f22f28b02a29.
All processes terminal. No performance/release/browser reruns. Goal remains active.


## Configurable basic families and four selector solver adaptations

Previous goal turn was progress: committed710d57b, clean worktree at start.
New BasicNeighborhood{neighborhood,variables,pillar_schema} exposes scoped ordered
basic move enumeration and independently configured weighted unions. Factored
mixed basic validation/enumeration through this shared implementation, preserving
MixedNeighborhoodError variants. BasicNeighborhoodError chains model/schema errors.
RepairNeighborhood::Basic works on basic and mixed models (basic-only movements),
rejects list-only models before scoring, and derives per-attempt seeds. Existing
Standard/Mixed behavior remains unchanged. No model clones for schema overrides.

Original basic_configured_families.rs:432 independent optional-state/scoped-order
change/swap comparisons, duplicate union membership and seeded repeatability,
complete basic scoped/stream plans crossing a column-group barrier with budgets0..4,
invalid scope/schema/kind before scoring/graphcallbacks, list model rejection,
Basic versus Mixed configured repair equivalence on mixed models in both backends.

selector_solver_cases.rs adapts four DefaultSolverTest methods with all22variants:
solveSingleVarMoveConfig6, solveListVarMoveConfig6, solveMultiVarMoveConfig5,
solveMultiEntityMoveConfig5. Actual multivar test fixtures read this turn: THREE
fields per entity, primary/secondary required shared Value domain, tertiary optional
Other domain. Enum namespaces, null initial fields, aligned3column swaps and
primary-only column grouping retained. MultiEntity twolead/twoherd fixture preserves
value versus lead-reference types and lead-before-herd construction. Its source
goal0 stops at completed construction; search configurations still validate, but
no search-execution claim. Other variants run native search40candidate/attempt cap,
seed20261007, exact dummyzeroobjective, score diagnostics and actual best events.
Native sampling/tracking/combinedconstructionphase substitutions disclosed.
Added4case records and15fixture/helper records. No production source inspected.

Initial check17774 failed due to nonexistent MixedError->MixedLocalSearchError
conversion; now retains MixedNeighborhoodError::Model. Initial focused96055 failed
on guessed FactSource type; corrected to existing StreamSource. Focused81626 exited0:
all4new original tests,3existing mixed schema tests,4new source methods pass.
Logs work/basic-families-{check,focused,focused2}.log. All-target build15248 currently
running in work/basic-families-build.log; final gate/dashboard validation follows.

All-target build15248 exited0. First gate38018 exited1 at clippy: the new test's
graph factory return type triggered type_complexity. Introduced a BasicGraph type
alias only; no runtime change. Follow-up gate83885 runs on the final Rust state,
work/basic-families-final-check2.log. No broad build repeated for this test-only fix.

Next concrete selector gap: per-family value order. Existing BasicNeighborhood
controls slot order/grouping but always uses the model's domain order. Pending
DefaultSolverTest.solveWithMultipleSorterManners (source~1444) combines ascending
and descending value selection as distinct children; solveMixedModelWithSortManner
(source~1410) exercises entity/value ordering combinations. Do not credit either
using two identical fixed-domain families. Their source methods were read in the
previous increment; comparator-sortable fixture details and exact enum alternatives
still need test/public-doc evidence. Existing generic OrderedSelection handles
sorting but is not automatically wired into configured solver move families.

Final gate83885 exited0. Rust3375 and doctests155 passed; source before/after equal:
9d258fe98146d6aea34e0c93b00b0490ba3d7242f93ddf295d22a66c8694a84c.
Report {'total': 4855, 'passed': 1898, 'failed': 0, 'pending': 2675, 'excluded': 282, 'unverified': 0, 'percent': 39.094}
Verification digest 4c3fd8781956c2d43b6b2a288e04af5110728ec67ba9f48efe16d78a0dd89890.
Live /api/freshness matched. All processes terminal; no benchmark/release/browser reruns.
Goal remains active. Logs work/basic-families-final-check{,2}.log.
Final selective accounting:361Rust jobs ran,56reused;46Python jobs reused.


## Per-family value ordering and sorted solver adaptations

Previous goal turn was progress (e3c0865). BasicValueOrder now owns a complete
per-variable domain-index/optional-None permutation, with sorted_by over borrowed
actual values (stable ties, no Clone required). BasicNeighborhood.value_orders and
MixedNeighborhood.basic_value_orders are borrowed slices. Validate duplicates,
unknown slots/indices, exact cardinality and required-null before callbacks.
Only Change/ChangeAndSwap/Assign/Unassign/Pillar/SubPillar/PillarChange/PillarUnassign
support nonempty orders; other families explicitly reject. Ordered generators now
share lazy target lookup while preserving default ordering and swap orientations.
Flat and schema pillars use the first selected row/column's target order. Static
orders need rebuilding for changed domains/sort keys; no automatic metadata cache.

Original value_ordering.rs4tests:288 optional-target/family/schema configurations,
non-Clone stable sorting, pins/domain intersection, malformed configs before score/
graph callbacks, disabled families, union occurrence/order distinctions, and full
basic scoped/stream plans with one move budget selecting1ascending versus3descending.
Also compare actual mixed configured union targets against the basic union.

sorted_solver_cases.rs adapts2DefaultSolver methods. solveMixedModelWithSortManner
runs all10combinations (2descending entity modes x5value modes). Public enum constants
confirmed via javap -public on work/oracle/timefold-solver-core-2.7.0.jar; no bytecode
or production implementation read. Actual test comparators/fixtures read. Required
basic strengths[2,1], entity difficulties[0,1], selected slots[2,0], first basic only;
compare full initial candidate order and preserve all final assignment/list asserts.
Native basic-then-list combined construction and HillClimbing16steps replace Java
default factory behavior. solveWithMultipleSorterManners preserves source5x5 shuffled
encounter orders (entities[4,2,1,3,0], values[3,4,2,1,0]) from public Collections.shuffle
Random(0), work/SortedFixtureProbe.java. HardSoftscore=(-(assigned-distinct),-distinct).
Two native construction phases (second assertedno-op), ascending/descending change
union via provider, seed17, local10candidate cap, real bestevents and provider calls.
Explicit assertions distinguish both sorted candidate prefixes. No equal Java trace/
factory/lifecycle claim. Added2case/14fixture-helper records.

Check39898 failed only missing new fields in complete existing config literals;
updated4tests. Check7795exit0. Focused3873exit0(all4original tests), sorted-focused
commandexit0(bothsource tests). Logs work/value-order-{check,check2,focused,sorted-focused}.log.
All-target build2515 currently running, work/value-order-build.log. Rust final state
includes new BasicValueOrder documentation example. Full check/dashboard follows.

All-target build2515 and final gate22388 exited0. 3381 Rust tests and 156 doctests passed.
Rust jobs{'RUN': 363, 'REUSE': 56}; Python jobs{'REUSE': 46}.
Equal before/after source:0cdc42f87ab7c193803b30926c392b5fdb4b1282a64c67c0bb50523f58628467.
Report {'total': 4855, 'passed': 1900, 'failed': 0, 'pending': 2673, 'excluded': 282, 'unverified': 0, 'percent': 39.135}
Live freshness matched verification digest:7524767aa6b2c5f76bc0d74a967e8f7d2623c05ec9e004704266e1d85f00b3ca.
All processes terminal. No benchmark/release/browser rerun; full goal remains active.
Final log work/value-order-final-check.log.

Final staged review caught an unassignment-only performance regression in the new
shared targets helper: filtering over all domain indices before yielding None.
Fixed before commit by using at most one null candidate and skipping both domain
and ordering lookup when assigned=false. Existing Unassign/PillarUnassign tests
cover the result semantics; this preserves original constant-time target setup.
Prior gate22388 evidence above predates this final helper change. Reverification
is required on the changed source state; do not relabel that previous result.

Final post-review evidence: build18553 and gate48719 exited0. 3381Rust tests, 156doctests passed.
Jobs {'RUN': 363, 'REUSE': 56} Rust; {'REUSE': 46} Python.
Equal source-before/after: 63039898dd1e5eb632c8f088a52ad2d07ada9cf5ef6f4fd956e34f8a831c7f5c.
Dashboard {'total': 4855, 'passed': 1900, 'failed': 0, 'pending': 2673, 'excluded': 282, 'unverified': 0, 'percent': 39.135}; live freshness matched digest
aa414dcf0179aa6e5042d2d0a96515dc1fc8f725076f149a2a8b0bcb9b82374a.
Final log work/value-order-final-check2.log. All handles terminal. Goal remains active.


## Atomic construction and optional/pinned list solver adaptations

ConstructionInterruption::DiscardIncompleteStep retains the previous assignment,
contribution cache and graph after a truncated scan. Default KeepBestPrefix stays.
OptionalConstruction::EvaluateUnassigned scores no-change last. All basic/list/mixed
placements and both scoring engines support it, including custom/bound providers.
Discarded scans count evaluations but no completed steps or best events. Explicit
early picks complete a step. Provider exact limits do not pull an extra proposal:
atomic mode discards at that boundary even if the next pull would end the pool.
Provider synthetic no-change follows natural exhaustion and costs a score/move
trial, not a raw proposal. Stop reasons latch; no-change is not queue progress.

Added8original regressions for budgets, cancellation, controlled phase/shared
stops, early picking, providers, retained prior steps and actual best-event visibility.
Added7DefaultSolverTest methods/16fixture-helper records in provenance, exercised
with both scoring engines. Optional fixtures: unassign all, assign remaining values
using explicit early pick in place of ONLY_DOWN inference, interrupted first step.
Four full/prefix pinned fixtures preserve every source membership/count/score assert.
Native ten-step HillClimbing, diagnostics and derived list views replace Java factory,
TRACKED_FULL_ASSERT and SolutionManager lifecycle; substitutions are documented.
No upstream production implementation was read.

Final gate74805 exited0:3396Rust tests and 157doctests passed. Rust jobs364ran/56reused;
46Python jobs reused. Source before/after identical:
0dbd8a876d23ecbe891fae6f0dd1277c1d743a85b5c8f2ef0b0b139da63712e3.
Dashboard summary:{"total":4855,"passed":1907,"failed":0,"pending":2666,"excluded":282,"unverified":0,"percent":39.279}
Live /api/freshness matched verification digest:
29f16d2e6843586e99dd0e68582161b474be5c38d7b3beb3e8e23ec80c1d07da.
All process handles terminal. Final log work/construction-atomic-final-check2.log.
Build14601 exited0; focused41632 exited0. Gate66346 found only a redundant test
initializer (fixed); final gate covers the final source. No benchmark/release/browser
reruns. Goal remains active; this is an increment, not full rewrite completion.

Next gap: DefaultSolverTest.solveCustomConfigListVariable (~1062), inspected.
Source2values/2owners, queued recording selector valueList replayed by list-change
selection; construction then16search steps; allownersnonempty and actualbestevents.
TestdataListVarEasyScoreCalculator scores each owner2ifsize1else1. Native
RecordingSelection (selection_replay.rs) opens traversal before replaycursor,
recorder.next publishes Arc value and next_replayed consumes latest occurrence.
Adapt through a real recording/replay bound construction provider, not plain greedy
construction. No method credit added for that pending case.


## Queued placement providers

QueuedMoveProvider adds Once/Cycle queues, assignment-aware lazy factories,
non-Clone keys, bounded empty-pool skips, retry after atomic discard, and explicit
reset for new phases. Construction completion notifications allow optional rejected
placements to advance and stop a full unproductive cycle. Seven original tests
cover queue behavior and integration with both scoring engines.
DefaultSolverTest.solveCustomConfigListVariable preserves the source fixture,
objective,16-step search and actual best events using real recording/replay.
Added1source case and6fixture/helper records. Placer lifecycle/copy cases remain
pending; no upstream production implementation was read.

Verification complete: all-target prebuild59641 and final check93070 exited0.
3404Rust tests and158doctests passed.366Rust jobs ran,56reused;46Python jobs reused.
Dashboard:1908/4855passed(39.3%),2665pending,282excluded,0failed/unverified.
Rust source before/after matched:
e239b0c550c742da3e9a2960173cb89c5b2a422103c240746708a81b5aa62a95
Live /api/freshness matched verification digest:
f46670d69b3bcc28c3a683e370953962ac89b6853550c4cfc3e10766ecb2953f.
Final log:work/queued-placement-final-check.log. Focused56326 passed all new tests;
52790 also passed16provider regressions. No benchmark/release/browser reruns.
Goal confirmed active. Next feature below remains unimplemented.

Next substantial gap inspected during compilation: DefaultSolverTest1593-1849,
especially no-inconsistent-field dependency handling (methods1671,1729,1767,1816).
Read all four testdomain/shadow/no_inconsistent_field fixtures. Four required list
values a1,a2,b1,b2 each60min, owner starts0, a2dependsa1/b2dependsb1. Previous list
value also supplies start; assigned start=max(previous end or owner start, dependency
ends), missing dependency end=>None. Objective soft negative sum(end-owner start).
Initial owner0[b2,b1]/owner1[a1,a2] must clear inconsistent b-values, retain a-values
and correct owner/previous shadows, with no bestevents in custom noop-only plan.
Pinning owner0prefix1 while both pairs are reversed must give a typed pinned-cycle
error. A custom phase assigning[b2,b1] after cleanup must reject structurally flawed
move. From empty lists, solving must reach HardSoft(0,-360) and dependency ordering.
Existing DependencyGraph/derive_dependencies detect cycles/downstream inconsistency
but native startup repair and solver-wide inconsistency rejection need investigation.
Do not substitute an extra hard-score penalty for the source's implicit structural
policy. No credit or code changes yet for this next feature.


## Structural list dependencies

ListProblem::with_dependencies declares fixed registry-index edges plus assigned
predecessor edges. New list_dependencies module prepares list/mixed initial states,
clears cycle members/downstream assigned values and rejects affected full/prefix pins.
Model apply rejects cyclic final states atomically before scoring; both list/mixed
selectors skip these trials. Exact enumeration/bounded children exclude cycles.
Startup repair establishes a silent baseline; later and exact bests publish normally.
No extra hard penalty; this explicit opt-in does not infer arbitrary shadow metadata.

Six original tests cover metadata/missing inputs, cleanup/pins/downstream, rollback,
callback counts, exact/bounded scoring, mixed integration and later events. Four
DefaultSolverTest no_inconsistent_field cases preserve cleanup/noevents, pinned b2
failure, custom cyclic composite failure and exact HardSoft(0,-360)/dependency times.
Native typed errors substitute Java classes/messages; provenance records all4source
cases,4fixtures and2helpers. No upstream production source was read.

Prebuild35520 and final check86513 exited0.3414Rust tests and159doctests passed;
368Rust jobs ran,56reused;46Python jobs reused. Dashboard1912/4855(39.382%),
2661pending,282excluded,0failed/unverified. Source before/after matched:
bb3b5e7c78911bec4192bcb8604512f9432dc6552f255c61b8ac3cd7daed6318
Live freshness matched verification digest:
4aae4edf2814f23493431b7858cc1158c43ce688c5d04bab23d18f2ec91b7e0f.
Final log work/list-dependencies-final-check.log; concise evidence at
work/list-dependencies-evidence.json. All process handles terminal. No benchmark,
release-mode or browser reruns. Goal remains active, full rewrite incomplete.

Next source gaps inspected while compiling: DefaultSolverTest.solveStaleBuiltinShadows
and solveStaleDeclarativeShadows(1593-1667), and all files in testdomain/shadow/
inverserelation and concurrent. Builtin fixture:2optional basic entities initially
v1,v1 with stale empty inverse collections; score negative sum(value entity count^2)
and-100pernull entity; solver must return v1,v2 unordered,score-2,besteventsnonempty.
Declarative fixture:2lists[a1,b1]/[b2,a2], alignmentgroups[a1,a2]/[b1,b2], local-only
search until feasible (source60sectimeout), exact final[b1,a1]/[b2,a2],score(0,-240),
besteventsnonempty. Concurrent provider explicitly penalizes inconsistency hard and
assigned finish times soft; unlike no_inconsistent_field, do not enable auto cleanup
policy for this fixture. Start0,ready previousfinish+30,alignedstart=max(group ready),
finish=start+30,cascade=indexdays. Native examples/concurrent_support already models
this fixture through aligned dependency projections, with source/independent closure
regressions. Still pending: actual full solver startup adapters preserving original
assertions. Do not weaken exact list ordering to any feasible equivalent.

Next-case draft is saved outside the frozen source tree at
work/stale_shadow_solver_cases.rs (not compiled, no new credit). Move into tests only
after this increment is committed, inspect it, then focus-test it. It preserves exact
stale-shadow source fixtures and initial rebuilt scores(-4 and HardSoft(-4,0)); stream
concurrent case uses existing cached Schedule support. Check max_steps type if compile
fails. Source claims still require new provenance fixtures/helpers before credit.
Broader subsequent candidate: MoveDirectorTest ValueAssignment tests181-817 use real
list score directors and assert complete forward/undo list+position state, without
mock listener-call assertions. Native ListMoveBuilder+scored transactions already
provide these operations. Other MoveDirector methods using mock before/after events
must remain pending unless all assertions have a real native counterpart. Read whole
methods/fixtures before adaptation; only181-225 was read in full this turn.


## Observed list edits and stale-shadow startup (current batch)

Previous goal turn completed1bd5968. Current increment adds list_observation.rs,
exported through lib.rs, without changing existing solver paths. apply_observed
validates an isolated candidate, emits actual before/after whole-state snapshots
and minimal changed ranges, and returns an undo-on-drop guard. Commit skips undo;
normal rollback emits reversed ranges. Observer panics at all4boundaries restore
assignments; unrelated unwinding rolls back silently. No-op/invalid edits emit none.
Convenience relocate/shift/span helpers return moved values in insertion order.
This is explicit model-level observation, not automatic score-director hooks.

32new MoveDirectorTest methods preserve list mutation/undo, returned identities,
exact before/after ranges and invalid-coordinate/no-interaction assertions.15older
assignment cases were discovered already credited and were not duplicated. Methods
for cross-list-overload same-entity rejection remain pending (native operations are
generalized).2DefaultSolverTest stale builtin/declarative cases preserve original
scores, exact assignments, source helper10steps/60sectime guard and actual events.
Native initial scores-4/HardSoft(-4,0) additionally prove fresh projection at startup.
Source tests and fixtures only; no upstream production implementation was read.

Focused3404 exited0:15initial source move cases+3original panic/lifecycle tests+2stale
cases.98407 exited0:expanded32source move cases. Final source/provenance is staged.
Direct selective gate72039 passed fmt/clippy and started tests, but compilation was
serial across dependent targets: verification_inputs follows out-of-line impls for
ListProblem, so this new impl correctly expands the affected target set. Intentionally
terminated the known gate process tree (terminal exit1), then started parallel
all-target prebuild11672 (work/observed-list-build-all.log), which exited0. Resumed
selective gate38859 exited0: 3451Rust tests and160doctests passed.
Rust jobs:{'REUSE': 97, 'RUN': 330};Python jobs:{'REUSE': 46}. Dashboard1946/4855(40.082%),
2627pending,282excluded,0failed/unverified. Source before/after matched:
1291dade9f10316753cf7cfa81addd81bef5750c7dc9a23d47bb4f5307e3979d
Live freshness matched verification digest:
2848c3ad20d10245b2c70b0201eab50f64aa45f0d2d263f5adeef92e2a0c5a5e.
Final log work/observed-list-final-check-resumed.log; concise evidence at
work/observed-list-evidence.json. All process handles terminal. No benchmark,
release-mode or browser reruns. Goal remains active; full rewrite incomplete.

Next source gaps inspected: full MoveDirectorTest ValueReplace, SubListUnassign and
MassMove blocks1900-2208. Replacement requires value unassigned/assigned hooks around
list-range brackets (including undo), plus displaced identity return. Unassignment
asserts ordered per-value before hooks, one range bracket, ordered after hooks.
Mass gathering asserts separate source removal brackets, destination assignment
bracket and exactly one shadow update; two members of one source must batch into
one bracket. Current whole-edit range callbacks do not satisfy these assertions;
keep cases pending until a real native event/lifecycle counterpart exists. Existing
ListMoveBuilder supplies mutation construction but not these callbacks. No new
source credits or production implementation reads during this follow-up inspection.
Read later MoveDirectorTest2440-2730: temporary-score postprocessing, score restore,
replayable undo and flawed-solution processor cases. Native scored guards already
restore assignment/cache, but source failure-leaves-applied semantics differ; do
not credit those failure cases based on native rollback guarantees.


## Prepared list lifecycle plans (current increment)

Previous goal turn committed2a47c19. New independent list_lifecycle module exports
ListLifecyclePlan, ListLifecycleEvent/Change, AppliedListLifecycle and StaleListPlan.
Constructors prepare/replace/unassign/assign/gather use existing validated model
moves; plans borrow the model and own initial/result/stage snapshots. Ordered real
value-before/range-before/mutate/range-after/value-after callbacks see each working
snapshot. Gather batches per source, then destination insertion; one Flush per full
apply/undo. Drop reverses stages; callback/unrelated panics restore original lists.
No callbacks for preparation failure, stale preimage or net no-op. movement() works
with scoped and stream score directors. Equality-sensitive stale checks require
payload fields in Eq or immutable payloads. Explicit hooks, not JVM shadow discovery.

Seven source methods from MoveDirectorTest ValueReplace/SubListUnassign/MassMove
read in full and adapted with3list fixture records. Negative-index method pending.
Five original regressions cover real intermediate snapshots, full callback panic
boundaries, stale/no-op/pin/dependency rejection, both scoring caches, ordered assign
and compound membership undo. Focused23047 passed initial4original+7source cases;
expanded50528 passed5original cases including both scoring backends. Only subsequent
source edits renamed private Stage to LifecycleStage and collapsed one nested if
reported by clippy. First gate25812 exited1 on that lint; fixed gate10818 exited0.
3463Rust tests and161doctests passed. Rust jobs:{'RUN': 3, 'REUSE': 426};
Python jobs:{'REUSE': 46}. No broad prebuild: no shared-type impl changed.
Dashboard1953/4855(40.227%),2620pending,282excluded,0failed/unverified.
Source before/after matched:
6b8a06cf42ef21540dcc1baa2afd84ef5d45fecf3ab571b700d2a22b179de884
Live freshness matched verification digest:
6b8f0fd0cbd910e630d8f96ebe3c1ee377553ee78a224a27c8fe95e1e741aaf8.
Log work/list-lifecycle-final-check-fixed.log; evidence work/list-lifecycle-evidence.json.
All process handles terminal. No benchmark/browser/release reruns. Full goal active.

Next missing MoveDirector behaviors include replayable recorded undo and temporary
postprocessors with explicit score freshness/failure handling (source2440-2730 read
in the preceding increment). Native scored guards already support scope rollback;
do not repeat existing mutation tests or credit failure-leaves-applied Java cases
with native rollback behavior. Full source methods/fixture helpers must be checked
before adaptation. This batch's seven source methods now have real value/range hooks;
previous notes saying they were blocked by missing callbacks are historical.


## Temporary processing and recorded replay (current increment)

Previous goal turn committedb49557c. New temporary_moves module adds TemporaryMoves
wrapper for all6native score directors, options for callback failure rollback/retain
and cached/fresh-checked restoration, typed phase errors and owned RecordedMove
snapshots/moves with direction-specific stale checks. Each callback sees the applied
candidate and score. Success undoes; KeepApplied commits validated state on callback
Err/panic before propagating. Apply failure always rolls back without callback.
recorded returns after undo, builds basic/list/mixed inverse moves, preserves repeated
basic writes, unassigned values, cross-owner transfers and pins on replay. Fresh mode
runs real contribution recalculation/graph replay and reports cache mismatch.
No existing director implementation changed; wrapper methods are on the new type.

Read full MoveDirectorTest2429-2730 including buildScoreDirector helper and all4basic
Testdata fixtures. Six source adapters preserve actual callback results/state, scores,
kept-applied panic and redo/undo, both basic backends. Five original cases exercise
all6backends, multiple replay cycles, pinned prefixes, both Result/panic policies,
stale checks, validation failure and measured fresh callback counts/corruption.
Focused84913 failed only on test enum spelling (MixedScope/MixedFact::ListEntity);
fixed focused run exited0 (work/temporary-moves-focused-fixed.log), all11tests passed.
Final gate22610 exited0: 3474Rust tests and162doctests passed.
Rust jobs:{'RUN': 3, 'REUSE': 428};Python jobs:{'REUSE': 46}. No broad prebuild.
Dashboard1959/4855(40.35%),2614pending,282excluded,0failed/unverified.
Source before/after matched:
5ada33cf8ff964f860086137be72b44bf1bb803cb3e3a6ab23ae008ba80cdc15
Live freshness matched verification digest:
d12ca693ace5faa6ead19ab239de9e955ac4e177cda31c219d31b15a63da0f53.
Log work/temporary-moves-final-check.log; evidence work/temporary-moves-evidence.json.
All process handles terminal. No benchmark/browser/release reruns. Full goal active.

Remaining source cases: moveFailureLeavesTheMoveApplied needs a move-construction
callback that may fail after writing; current native moves validate atomically.
restoreWorkingScoreWhenHandlingStructurallyFlawedSolutions needs an explicit flawed
candidate processor, not a generic validation error relabeled as source behavior.
Do not credit those based on the new postprocessor handling alone.


## Live move sessions and construction failures (current increment)

Previous goal turn committed0c1f900. New MoveSession guard for all6directors stores
an opaque per-director restoration closure: scoped snapshots capture assignment,
contributions,total; stream snapshots capture assignment and full GraphState.
Each apply immediately commits one scored primitive/composite; errors preserve the
successful prefix. Nested scopes support independent commit/rollback; outer rollback
restores everything including committed inner changes, without scoring callbacks.
temporary() runs TemporaryMoves inside an outer scope. TemporaryMoves::construct
adds live construction then postprocessing with distinct Construction error and
shared rollback/KeepApplied policy for errors/panics. Kept prefixes remain scored.

Initial focused run exposed a real snapshot bug: GraphTransaction::capture copies
only its touched-node delta. Replaced that use with new StreamGraph::capture_full,
leaving transaction capture unchanged. Fixed focused log work/move-sessions-focused-fixed.log
passed4original session regressions and7temporary source methods. Added grouped-stream
replay regression passed separately (work/move-sessions-group-focused.log). All6
engines cover nesting, prefix errors/panics, callback counts, pinned lists and mixed
partial-step rejection. Source moveFailureLeavesTheMoveApplied read in full and
adapted in existing temporary_move_cases with prior fixture/helper attribution.
One additional source credit; no upstream production implementation read.

Broad prebuild76504 and final gate24376 exited0; shared scoring/graph modules changed.
3480Rust tests and163doctests passed. Rust jobs:{'RUN': 376, 'REUSE': 56};
Python jobs:{'REUSE': 46}. Dashboard1960/4855(40.371%),
2613pending,282excluded,0failed/unverified. Source before/after matched:
dc462b65bca52c9da06301e311d91c861340141fa60e5f0d686cdca17af7ca7e
Live freshness matched verification digest:
783abf0d1496758aefbd901b6989aeb58f7351f7f8642b51fbc96313e1342329.
Log work/move-sessions-final-check.log; evidence work/move-sessions-evidence.json.
All process handles terminal. No benchmarks/UI/release reruns. Full goal active.
Next missing source behavior remains explicit structurally flawed candidate
processing; do not credit a generic model error as that processor's behavior.


## Structurally flawed candidate inspection (current increment)

Previous goal turn revalidated active goal only (no implementation progress).
This increment finishes the inherited uncommitted inspection implementation.
MoveInspection owns the proposed assignment and assigned cycle/downstream registry
IDs. List/mixed standalone helpers and four director inspect_move methods validate
starting state, domains, identity/layout and pins while deferring dependency checks.
Mixed previews include basic writes. TemporaryMoves::process_inspected routes valid
moves through ordinary scoring/rollback, and flawed snapshots to a distinct callback.
Flawed candidates never enter working state/scoring, even with KeepApplied; errors
and panics preserve baseline caches. CheckFresh checks original cache after success.

Focused work/move-inspection-focused.log passed one attributed source method and
three original tests across all four affected directors. Source MoveDirectorTest
restoreWorkingScoreWhenHandlingStructurallyFlawedSolutions read in full, plus prior
full reads of four no_inconsistent_field fixtures. Attribution records exact pinned
source and snapshot/native-shadow adaptations. No production implementation read.
New source and both new test files reviewed before broad prebuild.

Next-source reconnaissance (read-only while compiling): read MoveDirectorTest
2212-2429 fully: undoNestedPhaseMove asserts recorded before-range actions for an
unexpected destination, testSolverScopeNestedPhase asserts a distinct nested scope,
undoCascadingUpdateShadowVariable requires actual cascade reset on undo, twoUnassigns
requires two live sequential edits, twoChanges requires inverse relation of the
FIRST basic variable only. Do not credit nested/action-log cases merely from generic
session rollback. Native ListCascadeCache already supports separate transactional
shadow storage; automatic director/session integration remains worth investigating.
No additional source methods were credited from this reconnaissance.

Broad prebuild59921 and required gate69245 exited0. 3484Rust tests and164doctests passed.
Rust jobs:{'RUN': 378, 'REUSE': 56}; Python jobs:{'REUSE': 46}.
Dashboard:{'total': 4855, 'passed': 1961, 'failed': 0, 'pending': 2612, 'excluded': 282, 'unverified': 0, 'percent': 40.391}.
Source before/after matched:b7b01ce2c9a6029dfdcfad6281f574572d237dbde975e5326397246575ecfe72
Live freshness matched verification digest:1ac00702486599a78b800645f6286fefbf1622fab68a591bb32b2566a889e680.
Logs work/move-inspection-{build-all,final-check}.log; evidence
work/move-inspection-evidence.json. All process handles terminal.
No benchmark/browser/release reruns. Full rewrite goal remains active.


## Cascading shadows integrated with scoring (current increment)

Previous goal turn made progress and committed65443cd. This increment adds
list_cascade_stream and ListCascadeStep/ListCascadeStream in a new module. It binds
ListStreamSources::values to the existing incremental dependency node, resolving
predecessor identities to registry indices and supplying prior derived values.
Stable output rows include original owner/index/neighbor metadata and DerivedValue.
Unassigned values yield None without user supplier calls. Mixed sources use the
same adapter through .lists. Scoring, move undo, saved search candidates and nested
session snapshots now include real cascade state. No changes to core graph/director
modules, no shadow annotation discovery, imported partial fields or performance claim.

Read full four TestdataSingleCascading fixtures and MoveDirectorTest source method
(previous full source read retained). One attributed adapter uses real value+1
shadows and nonnull-count score, asserting actual shadow creation then undo to None.
Four original cases cover162partial order/placement trials against independent
arrival sums, registry identities/metadata, cross-owner transfers, mixed moves,
supplier and terminal panics, nested commit/undo, callback-free rollback, unaffected
list supplier reuse, dependency-index reuse and actual local search at three limits.
First focused run failed compiling test result field access only; changed it to
ListLocalSearchResult.best. Fixed focused work/list-cascade-stream-focused-fixed.log
passed all5tests. Added an exported port alias before the final gate. New files
reviewed. Required gate44610 started on frozen Rust/provenance; no broad prebuild
because shared implementations are unchanged.

Required gate44610 exited0. 3489Rust tests and165doctests passed.
Rust jobs:{'RUN': 3, 'REUSE': 433}; Python jobs:{'REUSE': 46}.
Dashboard:{'total': 4855, 'passed': 1962, 'failed': 0, 'pending': 2611, 'excluded': 282, 'unverified': 0, 'percent': 40.412}.
Source before/after matched:d3316f64e5b520470949989855437796f1102ea66f3bb8da82c852de3cfc4537
Live freshness matched verification digest:e7061ab267c2523494e252eb8ee219d31df8fec07e15738caab537ede7640bf8.
Log work/list-cascade-stream-final-check.log; evidence
work/list-cascade-stream-evidence.json. All process handles terminal.
No benchmark/browser/release reruns. Full rewrite goal remains active.

Committed511fd6c. A final attribution-only wording correction clarified one
entity with an empty value list. Report-only refresh reused the verified Rust
evidence; summary stayed1962passed,0unverified. Current live/report digest:
8a3f2d0cb0aea5d0f177a9d6ed96ea114f5b3bde7966b9d8ff8f354fe3a94991.
The preceding digest records the gate state before that wording correction.


## Inverse relationship streams and live inspection (current increment)

Previous goal turn made progress (511fd6c/219b8a9). New basic_inverse_stream adapts
selected basic-variable indices to stable entity identities and groups inverse
membership with the existing counted SetCollector. A value-universe reference join
retains empty collections and stable universe IDs; equal payloads share membership.
Basic and mixed source bindings work, with ordinary scoring/session rollback.
MoveSession::director adds read-only access to graph rows during a live scope.

Read full MoveDirectorTest.twoChangesInARow and twoUnassignsInARow, four mixed
singleentity fixtures, three list fixtures and TestdataObject. Two source adapters
retain the first-field-only inverse relationship, both-null restoration, sequential
list removals at updated indices, restored list order, and actual inverse/owner/index
rows. Attribution explicitly replaces reflection and Java object IDs with native
mappings/strings. Four original tests cover81partial basic assignments, counted
membership, universe replacement/duplicate payloads, ignored field callback reuse,
scoring panics, nested commit/undo and local search matching a reference scorer.
Focused work/basic-inverse-stream-focused.log passed5tests, then added unassign
source adapter passed work/basic-inverse-unassign-focused.log. No focused failures.
All new files reviewed. Broad prebuild29441 exited0; shared MoveSession changed.
Required gate48598 started on frozen Rust/provenance.

Next broader area: working-object lookup is absent. Read full TEST sources only:
core/.../impl/domain/common/{LookupManagerTest,LookupStrategyIdOrFailTest,
LookupStrategyIdOrNoneTest,AbstractLookupTest}. Same-ID lookup must return the actual
stored working object, not an equal clone. Duplicates fail; remove requires a known
working object; missing lookups fail; reset discards/repopulates. Null object lookup
returns null. A declared-null ID differs from absent ID metadata. ID_OR_NONE permits
no-ID add/remove as no-ops but still rejects lookup; it does NOT mean identity lookup.
ID_OR_FAIL rejects absent-ID operations. Annotation multiplicity, inheritance and
interface discovery are explicit source expectations: do not credit them with an
unrelated typed-key lookup. Test fixtures remain to read. No production lookup code
was read and no lookup case credited yet. Consider native typed schemas/Arc identity
for a larger next increment; keep Java reflective discovery limits explicit.

Required gate48598 exited0. 3495Rust tests and166doctests passed.
Rust jobs:{'RUN': 382, 'REUSE': 56}; Python jobs:{'REUSE': 46}.
Dashboard:{'total': 4855, 'passed': 1964, 'failed': 0, 'pending': 2609, 'excluded': 282, 'unverified': 0, 'percent': 40.453}.
Source before/after matched:6ef33037e54b3b418d098118bd473ee62e5724534fe21620aaf659c58bde6f86
Live freshness matched verification digest:add90183bc59795260fea2c4659a474e135d4c0ad8eeac1d5a1f7ce2eee27c18.
Logs work/basic-inverse-stream-{build-all,final-check}.log; evidence
work/basic-inverse-stream-evidence.json. All process handles terminal.
No benchmark/browser/release reruns. Full rewrite goal remains active.


## Working-object lookup (current increment)

Previous goal turn committed9b348c6 (progress). New WorkingObjectLookup<T,K> owns
planning-ID -> Arc<T> registrations with explicit WorkingLookupSchema members.
IdOrFail, IdOrNone and Disabled distinguish absent metadata from declared-null ID.
Actual Arc identity is returned on same-ID external lookup; duplicate registrations
and nonidentical removals fail. Optional null returns None. Immutable schemas pass
through. reset_with prepares the entire replacement before installation, preserving
old registrations on errors/panics. IDs and ordering must stay immutable; registries
are per-type/per-model namespaces. No production implementation was read.

Read full source None/Immutable tests plus all six clone/lookup fixtures and both
interface fixtures; prior full IdOrFail/IdOrNone/Manager/AbstractLookupTest reads
retained. Read MoveDirectorTest.lookUpWorkingObject but did not credit its mocked
director delegation. Adapted41methods: IdOrFail14, IdOrNone12, Disabled13, Manager2.
Explicit native schemas replace annotations; typed diagnostics replace Java text.
Three-member fixture preserves all heterogeneous fields/getters. Inherited-null-ID,
interface discovery and complete immutable parameter matrix remain pending. Five
fixtures and AbstractLookupTest helper attributed. Source fixture subclass/interface
files were read but not adapted. No reflection compatibility is claimed.

Three original tests exercise failed atomic resets (duplicate/null/panicking getter),
actual identity, namespace isolation, disabled metadata skipping, and external-value
resolution into real scoped/stream scored moves. Initial focused19553 failed solely
on test ConstraintNetwork type inference; explicit Arc<Fact> fixed it. Fixed focused
work/working-lookup-focused-fixed.log passed44tests. New source and tests reviewed;
final minor test iterator cleanup included before required gate86753. No broad
prebuild: this batch adds a new independent module and exports only.

Possible next substantive area: ProblemSizeStatisticsTest read in full from tests
only (core/src/test/java/ai/timefold/solver/core/api/solver/ProblemSizeStatisticsTest.java).
Its11cases include counts by entity type/variable, empty solution preserving declared
range size, multiple types summing per-type range sizes, list/cascade/record models,
fixed-point log conversion and US grouped/scientific formatting (6 decimals), with
nonfinite logs formatting as0. Test fixtures not yet read except previously attributed
basic/single-cascade ones. Do not use the imported production MathUtils/ValueRangeManager
implementations; derive behavior from tests/public APIs and native model metadata.

Required gate86753 exited0. 3539Rust tests and167doctests passed.
Rust jobs:{'RUN': 3, 'REUSE': 437}; Python jobs:{'REUSE': 46}.
Dashboard:{'total': 4855, 'passed': 2005, 'failed': 0, 'pending': 2568, 'excluded': 282, 'unverified': 0, 'percent': 41.298}.
Source before/after matched:8548b5c02eb9f86919c0627255aaa0d9d538972e089b924dfaace966b362ff5c
Live freshness matched verification digest:c2f3a656e033cd5f16aae71890efa12d43b160bb8f929f44f0bce961ebccb936.
Log work/working-lookup-final-check.log; evidence work/working-lookup-evidence.json.
All process handles terminal. No benchmark/browser/release reruns. Goal active.


## Named model statistics (current increment)

Previous user-facing turn only confirmed active goal status (no progress); resumed
live prebuild30606 without restart. Existing uncommitted statistics implementation
and focused tests retained. NamedProblemSize binds native slots to logical entity
types/fields, validates complete unique bindings and preserves declared range counts
for empty models. Mixed fields share logical entities. ProblemScale handles huge
BigUint spaces and grouped/scientific formatting. Corrected required-null pinned
variables to zero complete assignments; dependency list counts are upper bounds.

Ten ProblemSizeStatisticsTest methods and21fixture/helper records adapted at the
pinned revision. Explicit metadata replaces reflective discovery. The JVM default
fixed-point precision case remains pending; native scaled_log takes explicit
precision. Read MathUtilsTest only, not its production implementation/constant.
Focused named_statistics/named_statistics_cases/job_statistics passed; new files
reviewed. One new doctest. No benchmark/browser/release reruns planned.

Next area scouted while compiling: full ValueRangeManagerTest read, including
both sorting helpers (tests only). Solution/entity/composite extraction adds None
only for optional basic variables, deduplicates unions and retains per-entity
ranges. Equal ranges share identity. Sorting replaces the active/default range;
repeating the same sorter reuses identity, a new sorter replaces it. List optional
ranges never contain None. Initialization counters refresh on reset. Scale cases
also distinguish solution ranges from per-entity sums (including optional None);
restricted-list example approximates24*0.5=12. Keep these scale estimates separate
from our exact-count/upper-bound statistics. Existing value_range.rs supplies
immutable ranges/materialization/sorting; a managed per-entity registry is useful.
Full basic entity-provided fixture reads completed: TestdataEntityProviding{Solution,Entity},
TestdataCompositeEntityProviding{Solution,Entity},
TestdataAllowsUnassignedEntityProviding{Solution,Entity}, and
TestdataAllowsUnassignedCompositeEntityProviding{Solution,Entity} under
core/testdomain/valuerange/entityproviding (tests tree). Simple required factory
uses codes1/2/3, optional uses v1/v2/v3, ranges[1,2]/[1,3]. Composite factories use
[v1,v2]+[v1,v4] and[v1,v3]+[v1,v5]. Sorting fixtures use6values/2entities: simple
uninitialized disjoint3-value ranges; composite each entity combines two disjoint
three-value halves. All start unassigned for the referenced sorting cases.
Other range fixtures remain unread; no cases credited or implementation started.

Required prebuild30606 and gate70323 exited0. 3553Rust tests and168doctests passed.
Rust jobs:{'RUN': 385, 'REUSE': 57}; Python jobs:{'REUSE': 46}.
Dashboard:{'total': 4855, 'passed': 2015, 'failed': 0, 'pending': 2558, 'excluded': 282, 'unverified': 0, 'percent': 41.504}.
Source before/after matched:718245c26d067d6c4a383dd9c567de75721d34f233952353352db7b3394d2e4c
Live freshness matched verification digest:24503c91863f447621cfcf98363ea35bf25c4e203064b49cd617b8cd669e5380.
Logs work/named-statistics-{build-all,final-check}.log; evidence
work/named-statistics-evidence.json. All handles terminal. No verification failures.
No benchmark/browser/release reruns. Goal remains active.


## Managed planning value ranges (current increment)

Previous goal turn committed6b0075c (progress); starting tree clean. Added
ManagedValueRanges for one field with explicit Basic/List optionality and shared
or per-entity composite providers. Stable distinct unions; equal ordered ranges
intern together, sharing active sort state and snapshots. ManagedRangeOrder clone
retains comparator identity; same current sorter reuses without comparator calls.
New sorter replaces default order; raw views remain unsorted; previous snapshots
and already-built native models remain immutable. None exists only for optional
basic choices and sorts first. Resets retain kind and discard all caches. Captured
fact changes require new sorter identity or reset. Native variable/list_problem
adapters preserve range restrictions and supplied list assignment/pin metadata.

34source ValueRangeManagerTest methods:16basic+16list extraction/sorting and2
identity deduplication.39fixture/helper records attributed. Full test/helper read
retained from previous turn. This turn read all list-specific entity/solution/value
fixtures referenced by these cases, plus optional shared basic/composite fixtures
and PlannerAssert non-null/reversed iterator helpers (tests only). Previously read
basic and entity-provided fixtures retained at unchanged pin. Unique code strings
represent fixture identities; unused assignment/shadow fields omitted explicitly.
No production implementation read. Initialization/reset counters and10approximate
scale methods remain pending (13methods total). Source approximate scales differ
from native exact-count/upper-bound statistics; do not relabel those as matching.

Original4tests exercise comparator panic rollback, identity/cache callbacks, reset
snapshots, empty ranges, 256union pairs, actual basic/list solves with ordered choices
and pins, and model error propagation. Focused99942 exited1 for missing type on
empty original test; explicit i32 fixed it. Fixed focused log passed38tests. Source
and new tests reviewed before gate25728. One new doctest. Independent new module
and exports only: no all-target prebuild or benchmark/browser/release reruns.

Required gate25728 exited0. 3591Rust tests and169doctests passed.
Rust jobs:{'RUN': 3, 'REUSE': 441}; Python jobs:{'REUSE': 46}.
Dashboard:{'total': 4855, 'passed': 2049, 'failed': 0, 'pending': 2524, 'excluded': 282, 'unverified': 0, 'percent': 42.204}.
Source before/after matched:aa194a0adcf667b4cbb8dc5853257294507a92ee2440406c176987d378c9a398
Live freshness matched verification digest:14aca93c407845865d673738e7fe364c662db268125c8e21da96b7d2d1372064.
Log work/managed-ranges-final-check.log; evidence work/managed-ranges-evidence.json.
All handles terminal. No benchmark/browser/release reruns. Full goal active.


## Managed range statistics (current increment)

Previous turn committed6ed3f3a (progress); starting tree clean. Managed ranges now
retain whether providers are shared/entity-specific. New statistics() reports
per-field counts, optional-basic choices and base10 estimate; combine_fields
counts logical entity rows once. Required empty spaces retain source zero-log
convention with explicit flag. No assignments/pins enter metadata estimates.
List sizes use ordered arrangements, independent eligible-owner fractions for
restricted ranges and logarithmic elementary-symmetric subset sums for optional
lists. Estimates may be fractional and are NOT exact counts/upper bounds. Existing
ProblemStatistics unchanged. Fresh basic/list initialization validates assignments
and counts missing required basics/unassigned list values; native list registry
values count as shadow entities. Resets/recomputation preserve previous snapshots.

13remaining ValueRangeManagerTest methods adapted (47total for the class).20new
fixture/helper records. Full TestdataValueRangeSolution/Entity read; other fixtures
retained at same pin. Eight-type fixture preserves source literal values as text in
separate field managers. MathUtilsTest arrangement helper/assertions read again:
permutation/partition counts justify independent BigUint20*...*519 reference for
500values/20owners; no production helper/constant read and no default JVM fixed
precision claim. ProblemSizeStatistics fixed-point-default case still pending.

Original3tests independently enumerate required/optional ordered spaces for0..4
values/0..3owners, exercise500optional values, approximation distinction, metadata
count overflow, resets and invalid/duplicate assignments. Focused47842 exited0:
54tests across two new and two existing managed-range targets. New source/tests
reviewed and frozen before gate22266. No all-target prebuild, benchmarks, browser
or release reruns.

Next potential substantive feature: typed planning model metadata integrated with
range/statistics construction. Read full PlanningSolutionMetaModelTest (29methods,
all pending at last inventory): basic solution/entity/value type identities; list
solution genuine owner plus shadow value entity ordered membership; exact canonical
parent identity; genuine/shadow classification; ordered variables; wrong entity and
missing field diagnostics containing names. Basic fixture fieldvalue; list fixture
valueList, value shadowsentity thenindex. Relevant basic/list fixtures already read.
Use idiomatic native typed declarations and stable canonical references; do not
claim Java annotation discovery. No implementation started, no case credited.
DefaultSolverTest pending method names inspected only; no new bodies read.

Required gate22266 exited0. 3607Rust tests and169doctests passed.
Rust jobs:{'RUN': 5, 'REUSE': 441}; Python jobs:{'REUSE': 46}.
Dashboard:{'total': 4855, 'passed': 2062, 'failed': 0, 'pending': 2511, 'excluded': 282, 'unverified': 0, 'percent': 42.472}.
Source before/after matched:4ac6247921f9641ea81c21c45ff06f0c2b84ccd37e322ee3045e6ee8a48e1b82
Live freshness matched verification digest:0259bcdd1fef5b8b16bf8a880e0aa65be0eaa6df2bc566145748a43455afa335.
ValueRangeManagerTest report confirms47/47passed.
Log work/managed-statistics-final-check.log; evidence work/managed-statistics-evidence.json.
All handles terminal. No benchmark/browser/release reruns. Goal active.


## Typed planning metadata (current increment)

Previous turn committed8f06b57 (progress); starting tree clean. Added PlanningSchema
with native TypeId-backed PlanningType, typed entity/variable declarations and
basic/list/shadow kinds. Canonical immutable metadata retains declaration order;
borrowed handles preserve parent identity and cannot outlive the schema. Duplicate
types/field names, empty field names, wrong kinds/types and incomplete statistics
bindings fail with named diagnostics. Genuine/shadow queries are filtered views.

Variable handles construct typed ManagedValueRanges, infer optionality and reject
shadow-as-choice misuse. Entity statistics bindings require each genuine field
once and restore declaration order; NamedProblemSize validates actual slots. An
original test constructs and solves a mixed model and obtains named statistics
from the same declarations (2logical entities,4variables,6values,96assignments).
Other original tests cover bad declarations/bindings, distinct same-short-name
types, cross-schema identity, canonical addresses, optional and empty-model ranges.
One new doctest. No object traversal/getter/setter/inheritance or automatic shadow
evaluation claim. No production implementation read.

29nested PlanningSolutionMetaModelTest methods and6metadata fixtures adapted from
previous full test/basic/list fixture reads. Reporter now accepts optional integer
upstream_line for exact pinned declaration disambiguation. Both validation and
result application resolve it; missing/stale/ambiguous targets fail. Existing
ledger mappings without lines remain unchanged. Added Python regression checks
line/type/path/name failures and distinct nested target identity;8tests passed.

Focused74161 failed because new mixed-model test used SolverConfig; fixed to
MixedSolverConfig. Fixed71767 exited0 with32tests. New files reviewed before
gate68525, which includes required checks and selective reuse. No all-target
prebuild, benchmark/browser/release reruns.

Potential next area: shadow-source path compilation and accessors on typed metadata.
Scouted VariableSupportTest lines1-220 (incomplete first graph-events case), and
RootVariableSourceTest lines1-210 (first builtin/declarative sources and group
paths; fourth case incomplete). Source references distinguish builtin/declarative
shadows, parent/group paths, canonical target/downstream metadata and actual
visited entity identity. Remaining bodies and invalid/declarative fixtures are
unread. No implementation or test credit for these sources yet.

Required gate68525 exited0. 3639 Rust tests and 170 doctests passed.
Rust jobs: {'RUN': 3, 'REUSE': 445}; Python jobs: {'REUSE': 45, 'RUN': 1}.
Dashboard: {'total': 4855, 'passed': 2091, 'failed': 0, 'pending': 2482, 'excluded': 282, 'unverified': 0, 'percent': 43.069}.
Source before/after matched: 30ec93206cfdbe6d698abeddc422e2bba2b50ad8fa113aa61674fe686167a771
Live freshness matched verification digest: a6d508c7ddfb0727736740b90b3babef232d4335527bf720b79e16171bf48eb3.
Log work/planning-schema-final-check.log; evidence work/planning-schema-evidence.json.
All handles terminal. No benchmark/browser/release reruns. Goal active.


## Typed shadow source paths (current increment)

Previous turn committed64ddc51 and was progress. Starting tree clean. Added
source_path.rs: SourcePathRegistry with typed HRTB borrowed getters; scalar and
collection registration validates metadata before mutation. Immutable compiled
paths preserve canonical schema handles and typed accessors; no unsafe code or
object clones. Native SourceMemberRole adds explicit previous/declarative/fact/
inconsistent roles without changing PlanningSchema. Collection kind follows
metadata; duplicate accessors/types/cardinality misuse fail.

CompiledSourcePath references expose root/target/source metadata, parent kind,
chains (relative to group/list element), top/bottom/root/declarative flags and
downstream declarative source. Traversal reads prefixes only, never terminal
values; Option nulls/empty groups yield no objects, references preserve identity.
SourceReference maps a variable owner through its remaining path to terminal
owner(s). dependency_keys maps actual references to stable application keys,
sorted/deduplicated for existing dependency APIs. No automatic suppliers/setters
or Java reflection claim. No production implementation read.

Read full RootVariableSourceTest in sections (including missing truncated
328-378 segment), plus complete TestdataInvalidDeclarativeValue/Entity/Solution.
23 path cases adapted; 3 fixtures and 2 assertion helpers attributed. Reflection/
parameter/variable-identification cases remain pending. Original 3 tests exercise
incremental DependencyCache edge updates, cycle callbacks suppressed, snapshot
restore, typed misuse, nulls, duplicate groups and fresh snapshots.
Focused54735 exited0 with26tests; then strengthened source target mapping length
assertion before final gate. No all-target prebuild needed (new independent module
and lib exports). No benchmarks/browser/release reruns.

Gate76733 stopped at clippy for two needless explicit traversal lifetimes;
elided only those annotations. Revised gate30994 runs against final source.
Potential next feature: metadata-keyed variable state/notification registry. Read
VariableSupportTest remainder240-762 and gap210-245 after earlier0-220 read. Tests
require one actual state per canonical field, separate state/tracker instances,
immediate basic/list updates, latest-snapshot initialization on late lookup,
identity retained across resets, per-field separation even when value types match,
and no working-solution read during linking. Existing basic_inverse_stream offers
transactional inverse relation updates, but no native field-state registry found.
Mixed/inverse/multivar fixture bodies for this next area not newly read; no new
case credits or implementation for VariableSupportTest this turn.

Required revised gate30994 exited0. 3665 Rust tests and 171 doctests passed.
Rust jobs: {'RUN': 3, 'REUSE': 447}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2114, 'failed': 0, 'pending': 2459, 'excluded': 282, 'unverified': 0, 'percent': 43.543}.
Source before/after matched: 40f78fb572d48add5ecafee459f7dce82cc7905fe5d6ff3b8776dd3b13e4f6f6
Live freshness matched verification digest: 441043b8a110250617a00c947129b780a0a1018becf33c106cd6216324d2cebb.
Existing provenance prefixes unchanged; appended23cases and5fixtures/helpers.
Log work/source-path-final-check.log; evidence work/source-path-evidence.json.
All handles terminal. No benchmarks/browser/release reruns. Goal active.


## Shared variable state and event tracking (current increment)

Previous turn baa736e was progress; starting tree clean. Added variable_state.rs
and exports only: VariableStateRegistry binds canonical schema fields to typed
stable entity/value keys. Rc/RefCell state handles and separate event tracker
cores retain identity across repeated lookup, registration order and field resets.
Basic inverse collections and list owner/index/neighbors publish immediately.
Explicit per-field reset clears tracker window; update records both event kinds
for changed/inserted/removed entities. Snapshot methods return detached data.
Typed misuse/duplicates and duplicate list values fail before existing data/events
change. Indexes currently rebuild per field; registry is single-threaded and not
an implicit object scanner or shadow-supplier runner. No performance claim.

Native synchronize_basic/lists binds entity IDs to assignment slots. Original
3 tests compare against real scored inverse stream updates across27assignments,
list observed move apply/undo, shared identity, atomic bad input, field isolation,
null assignment, wrong schema/type and duplicate slot bindings.

16 VariableSupportTest methods adapted (concurrent shadow graph case pending),
plus5each BasicVariableTrackerTest/ListVariableTrackerTest. Trackers return typed
Before/After violation records, not JVM diagnostic strings; canonical field
filtering/reset preserves source behavior. Source state/tracker fixtures are
metadata/row projections with explicit setup instead of Java reflection. The
tracking coexistence source case also constructs native StreamScoreDirector and
inverse stream, performs a scored edit, synchronizes state and asserts consistency.

Read full BasicVariableStateTest, BasicVariableTrackerTest and ListVariableTrackerTest.
BasicVariableState notifier/externalization cases not implemented or credited.
Read complete10basicinverse/mixed/multivar fixtures and3inverse fixtures; existing
basic/list/optional fixture reads reused. Source helpers remain independently
implemented. No upstream production implementation read.

Focused4701 failed at empty-list type inference; added explicit i32 and usize
identity annotations. Revised focused command exited0 with29tests. Strengthened
source tracking coexistence with actual score director before final gate.
No all-target prebuild; new independent module. No benchmarks/browser/release reruns.

Next-area reconnaissance: read full SingleDirectionalParentVariableReferenceGraphTest
(two currently pending cases): one supplier invocation per dirty element even
when multiple parent fields change; batch unassignment updates every removed
value plus downstream counts, including default index collision0. Counting
fixtures not yet read this turn. DefaultListVariableStateTest method names only
inspected (418lines; 11methods pending); they cover initialization/assignment,
external shadow notifications and pin boundaries. No implementation/credits for
either class. Existing list_cascade and dependency caches may supply execution;
future work should integrate them with metadata and state rather than duplicate
the evaluator. Gate74807 runs against final source.

Required gate74807 exited0. 3694 Rust tests and 172 doctests passed.
Rust jobs: {'RUN': 4, 'REUSE': 449}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2140, 'failed': 0, 'pending': 2433, 'excluded': 282, 'unverified': 0, 'percent': 44.078}.
Source before/after matched: 1a100717d01cb5ccf5f596e614f00ad3f85738e200ff3e371db5ed31f2b59b60
Live freshness matched verification digest: f4723c2d2c1ec402591e0d70270569c6fa5d51056ee835b76811bba8147ecc76.
Existing provenance prefixes unchanged; appended26cases and23fixture records.
Log work/variable-state-final-check.log; evidence work/variable-state-evidence.json.
All handles terminal. No benchmarks/browser/release reruns. Goal active.


## Deferred list shadow sessions (current increment)

Previous turn5b3a9a0 was progress; starting tree clean. Added ListShadowSession
with typed list-state/target binding, fixed stable value universe, deferred
initialization, coalesced parent notifications and latest-whole-snapshot flush.
Uses existing DependencyCache, not a new evaluator. Structural owner/index/
neighbor changes dirty values; changed predecessor edges rewire the graph.
Suppliers run at most once per flush, including distinct unassigned values.
Equal results stop propagation. Captures may read immutable facts; changed
external inputs require mark_dirty. No arbitrary path inference/setter reflection.

values()/assert_current reject stale/pending state; cached_values explicitly
permits diagnostics. Checkpoints share cached non-Clone results, retain pending
work and applied/working snapshots, and restore without suppliers after the
registry field matches saved working rows. Session identity prevents foreign
restore. Metadata/universe validation and supplier panics preserve existing cache
and pending work; callback side effects remain external. Registry state must not
be mutated from a supplier. Owner identities can change; universe is fixed.

Read full3Counting fixtures; reused full SingleDirectionalParentVariableReferenceGraphTest
read from prior turn. Adapted both methods with actual typed state snapshots in
place of mocked index/inverse/next and explicit previous-chain binding in place
of reflective graph classification. Retain guarded count supplier and all initial/
final outputs. 4original tests cover actual native moves/fresh cascade agreement,
no-callback restore, latest snapshot/coalescing/equal stopping, owner replacement,
empty lists, non-Clone results, supplier panic retry and metadata/universe errors.

Focused67328 exited1: explicit non-Clone Shadow reference type needed. Second
focused command exited1: integer default inferredi32 for i64 target; fixed test
and doctest constants. Third focused command exited0:6tests. No production
implementation read. Final source/provenance reviewed and frozen before gate.
No all-target prebuild, benchmarks/browser/release reruns.

Next-area read: DefaultListVariableStateTest lines1-210 (initialize/assign cases
and first external-shadow notification case complete; second incomplete), with
notifier count semantics and double-unassignment rejection. Remaining body and
new pinned fixture bodies unread. No credits for that class.

Read remaining DefaultListVariableStateTest205-418 during gate, completing the
class: fully externalized index/inverse/prev/next suppress fallback notifications
even on unchanged rescans; partial externalization still notifies touched suffix.
Pin queries follow current owner/index, full pin overrides prefix, unassigned
values are unpinned, unsupported pinning requires no solution read, and external
list edits change which value lies below pin boundary. Pinned fixtures still
unread this turn. No credits. Gate43509 running against frozen source.

Required gate43509 exited0. 3700 Rust tests and 173 doctests passed.
Rust jobs: {'RUN': 3, 'REUSE': 452}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2142, 'failed': 0, 'pending': 2431, 'excluded': 282, 'unverified': 0, 'percent': 44.119}.
Source before/after matched: 08e5cadf4a33b5a067333403d3637a735c3e8402ad09a38c8537229a19cf4ae8
Live freshness matched verification digest: 920507133b610ee52e3bb41613c5c02b016af952e49b82e2e608b174bacce2d6.
Existing provenance prefixes unchanged; appended2cases and3fixtures.
Log work/list-shadow-session-final-check.log; evidence work/list-shadow-session-evidence.json.
DefaultListVariableStateTest has10@Test methods; prior11method count included a helper.
All handles terminal. No benchmarks/browser/release reruns. Goal active.


## List state supplies (current increment)

Recovered interrupted focused run: both targets completed successfully, 13 tests.
No rerun for the environment change. Added ListStateSupply live assignment/pin
queries and typed owner/index/previous/next publication, metadata validation,
seeded projections, suffix refresh and registry unassignment. Partial bindings
retain fallback notifications; full bindings use only changed property events.
Original integration applies events to caller-owned fields, notifies deferred
cascade suppliers, synchronizes real native moves and restores materialized fields
and cached shadows after undo. Three original and ten source methods.

Read full DefaultListVariableStateTest and relevant basic/optional/pinned fixture
bodies; added ten case and eleven fixture/helper records with existing prefixes
unchanged. No upstream production implementation read. Constructor captures current
positions so unassignment before first refresh clears seeded fields. Model bindings
validate stable owner/slot identity. Pin observations do not authorize invalid moves.
Final source/provenance frozen for the required selective gate. No benchmark claim.

The app moved command execution into native WSL. Initial sandbox runner failed
to launch; after permissions changed, normal exec worked. The current app PATH,
RUST_LOG and LC_ALL changed verification environment keys, causing unrelated
reruns. Verified that an unchanged target's old cache key exactly matches the
previous dashboard process environment after its normal bash login setup. Stopped
only gate32719's live process tree (terminal143), preserving its interrupted log.
Restarted gate45622 in that original environment without changing records/hashes.
For subsequent checks use the same environment to avoid another broad invalidation:
`env -u RUST_LOG -u LC_ALL PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/usr/games:/usr/local/games:/usr/lib/wsl/lib bash -lc 'bash scripts/check.sh'`.
The login profile adds native tool locations. Logs preserve actual reused timestamps.

Next area fully read: BasicVariableStateTest (externalizedMode, mapMode,
cannotExternalizeTwice) and basic/inverse relation Solution/Entity/Value fixtures.
Map initialization notifies once per membership, preserving multiplicity (val1
twice, val3 twice); changing c from val3 to val2 notifies old/new values once each.
Externalized inverse collections preserve members and reject duplicate binding.
Native shared BasicVariableState already supplies live inverse views; publication
events, initialized caller-owned collections and fallback notifications are next.
No source credits or implementation for those three cases yet.

Required gate45622 exited0. 3713 Rust tests and 174 doctests passed.
Rust jobs: {'RUN': 44, 'REUSE': 413}; Python jobs: {'REUSE': 46}.
The44reruns include targets whose records the interrupted changed-environment run
had replaced; 413 remaining targets reused their existing verified results.
Dashboard: {'total': 4855, 'passed': 2152, 'failed': 0, 'pending': 2421, 'excluded': 282, 'unverified': 0, 'percent': 44.325}.
Source before/after matched: a446117092c52d19a8565bdf066cef2d9c3247230a57708464dca3edfdb66681
Live freshness matched verification digest: 0e901ed5970243048590aa1e55cc180745c3135551785141e2541cf975f423cd.
Existing provenance prefixes unchanged; appended10cases and11fixtures/helpers.
Log work/list-state-supply-final-check.log; evidence work/list-state-supply-evidence.json.
No benchmark/browser/release reruns. All handles terminal; goal remains active.


## Basic inverse publication (current increment)

Previous turn7ad5c99 was progress; starting tree clean. Added BasicStateSupply
over existing shared BasicVariableState. Explicit typed Vec<E> shadow binding
validates canonical schema/value entity/types and rejects a second binding.
Seeded published collections use stable identity order and reject duplicates
atomically. Live inverse queries follow registry state; refresh produces complete
before/after collections or unbound per-membership fallback notifications.
Initial notifications retain multiplicity; later refreshes coalesce interim edits.
Registry changes validate fixed value universe and state identity. Native models
still enforce pins/domains; callers synchronize assignment and apply returned events.

Reused full BasicVariableStateTest and six fixture reads from previous turn.
Added three source methods and six fixture records; no production implementation
read. Three original tests cover actual caller-owned collections and scored stream
rows for27partial assignments plus undo, invalid metadata/seeds/unknown state and
recovery, membership multiplicity, owner additions/removals and null assignments.

Focused79317 exited0: six tests. First required gate53196 failed clippy on a
collapsible conditional; simplified the condition without changing behavior.
Final gate rerun uses the matching login environment recorded above.

Next substantive batch: all20methods of ListVariableShadowVariablesTest remain
pending. Read full999-line test class this turn, plus all three shadowhistory
fixtures (TestdataListEntityWithShadowHistory, TestdataListValueWithShadowHistory,
TestdataListSolutionWithShadowHistory). All four field setters append every assigned
value, including null, to separate histories. Assertions require exact history and
final field value, and empty histories for untouched null neighbors; checking only
final assignment is insufficient. BuildSolution initializes shadows once; resetting
the same solution must not add duplicate events. Cases cover owner removal, value
assign/unassign, six relocations, four swaps including neighbors, five sublist moves
and three sublist swaps with reversal. Execute actual native ListProblem moves and
consume typed supply events into the fixture histories. Preserve unassigned count
and zero-score assertion in addAndRemoveElement. Helper provenance is also needed.
No credits/adaptations for these20cases yet. BasicInverseShadowVariableTest and
ListElementShadowVariableTest are already credited through existing native streams;
do not duplicate credits for the recently inspected tests. AffectedEntitiesUpdaterTest
was read fully but remains pending (entity-to-graph-node index grouping).

Required final gate93681 exited0. 3719 Rust tests and 175 doctests passed.
Rust jobs: {'RUN': 3, 'REUSE': 456}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2155, 'failed': 0, 'pending': 2418, 'excluded': 282, 'unverified': 0, 'percent': 44.387}.
Source before/after matched: 67b0bc4f346d9968890f1b591a2b5b7e3357e84f89391ac817c69a66ca0ad6c2
Live freshness matched verification digest: 709bb9fd630382f6da614afff6c4509a3a6119d0784966ba572aa9135443ed20.
Existing provenance prefixes unchanged; appended3cases and6fixtures.
Log work/basic-state-supply-final-check.log; evidence work/basic-state-supply-evidence.json.
No benchmark/browser/release reruns. All handles terminal; goal remains active.


## Exact list-shadow histories (current increment)

Previous turn354dcc8 was progress; starting tree clean. Adapted all20methods of
ListVariableShadowVariablesTest through an actual native ListProblem and shared
ListStateSupply. The fixture applies before/after records into owned fields and
appends every changed value to per-field history vectors. Every source history
and final-field assertion is preserved; unchanged null neighbors require empty
histories. Required partial-list score is calculated by the native score network,
including structural unassigned count. Owner removal clears registry rows while
retaining shadow state so removal events are observable. No automatic solver-wide
entity removal or cross-field event order claim. Existing production code suffices.

Source class and all3shadowhistory fixtures were fully read in the previous turn.
A temporary strict adapter recognized every source statement and failed on unknown
syntax; it generated explicit Rust source assertions, not runtime-derived expected
values. First focused67709 failed because fixture local h shadowed source value h;
renamed fixture context and preserved all literal value names. Second focused59332
running. Added20case and14fixture/helper provenance records; prefixes unchanged.
No upstream production implementation read.

Focused59332 exited0:20tests. Required gate73870 exited0. 3739 Rust tests and 175 reused doctests passed.
Rust jobs: {'REUSE': 459, 'RUN': 1}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2175, 'failed': 0, 'pending': 2398, 'excluded': 282, 'unverified': 0, 'percent': 44.799}.
Source before/after matched: 33e965522d05fe6c9fa4e476dc49463d565992c0bffa30f6b68a42c9d1c5ddbc
Live freshness matched verification digest: d1459d899b90e309aa6e97fddb8a90b7979ed63b52a3da5c616c0ef5f7bd5370.
Reviewed generated assertions and counted every source history assertion against
its Rust counterpart for all20methods; no assertions dropped. Existing provenance
prefixes unchanged; appended20cases and14fixtures/helpers.
Log work/list-shadow-history-final-check.log; evidence work/list-shadow-history-evidence.json.
No benchmark/browser/release reruns. All handles terminal; goal remains active.

Next-area reads this turn: full ListModelTest (one bounded-quality solve with
start/end cascade), MixedModelTest (temporary basic delay change propagates to
next list element), and ShadowVariableUpdateTest (six methods: entity-array/solution
updates, inverse membership and two derived time fields, parameterized suppliers,
invalid metadata/empty input). Their newly relevant simple_list/basic/parameter
fixture bodies have not yet been read. Metadata, source paths, dependency caches
and typed basic/list supplies exist; generic coordination of typed multi-field
shadow suppliers remains an integration gap. Extend actual native execution here;
avoid re-crediting already adapted basic-reference/list-element stream tests.


## Heterogeneous shadow sessions (current increment)

Previous turnee12836 was progress; starting tree clean. Added ShadowSessionBuilder
and ShadowSession over DependencyCache. Typed basic/list/external-shadow readers
and derived nodes use canonical field metadata and stable entity keys, with
heterogeneous erased Eq values behind Rc. Typed handles enforce result types and
session ownership; duplicate field/entity registrations fail. Suppliers access
only declared dependencies and immutable world facts. Update polls registered
readers, propagates changed values once, stops on equality, and handles dynamic
edges/cycles/inconsistent descendants. Source None remains consistent.

Checkpoint shares non-Clone results; restore validates sampled registered inputs
and restores edges/cache without derived callbacks. Reads are last-published
snapshot reads; explicit update after world edits. Changed external mutable facts
require mark_dirty; those facts and callback side effects are not automatically
restored. Generic source-path binding and automatic setters remain separate work.

Read full six basic/parameter fixtures this turn; reused full ShadowVariableUpdateTest
read. Four methods adapted through real BasicStateSupply inverse publication and
multi-field scheduling. Two Java null-array/collection diagnostics remain pending.
Four original tests cover mixed result types, actual native trial/undo, equality
stopping, rewiring/cycle recovery, foreign/undeclared metadata, non-Clone snapshots,
None results, supplier and reader panic retry. One public API doctest added.

Library compile33836 exited0. Focused47222 and7611 failed because test suppliers
borrowed handles with insufficient lifetimes; move closures now own cloned handles
and borrow stable counter Cells. Focused20726 exited0: seven tests. Added one more
reader-failure/None regression afterward; final gate will run all eight. Appended
four case and six fixture provenance records; existing prefixes unchanged.
No upstream production implementation read; source/provenance now frozen for gate.

Next integration opportunity: compiled SourcePathRegistry plans already expose
canonical variable/entity references and dependency_keys over typed borrowed
objects. ShadowSession currently takes explicit typed handles/edges. Bind compiled
paths to registered field/entity nodes so rewiring follows changing references
and groups automatically; preserve missing/foreign metadata validation and atomic
updates. Inspected CompiledSourcePath339-430 this turn. GraphStructureTest1-110
was read (classification assertions only, remainder unread); avoid substituting
classification labels for operational source-path evaluation. ListModelTest,
MixedModelTest and other pending shadow updates remain useful end-to-end cases.

Required gate84879 exited0. 3747 Rust tests and 176 doctests passed.
Rust jobs: {'RUN': 3, 'REUSE': 459}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2179, 'failed': 0, 'pending': 2394, 'excluded': 282, 'unverified': 0, 'percent': 44.882}.
Source before/after matched: 1923f376e42fba83531487256b87db211af29a4911bcab8745f165fba051c80c
Live freshness matched verification digest: b2efce35e16a291c30944f46b35867fe67e27555fb94fc854c51ca0e16b522cb.
Existing provenance prefixes unchanged; appended4cases and6fixtures.
Log work/shadow-session-final-check.log; evidence work/shadow-session-evidence.json.
No benchmark/browser/release reruns. All handles terminal; goal remains active.


## Automatic compiled source bindings (current increment)

Previous turn4e8b0db was progress; starting tree clean. ShadowSessionBuilder now
binds compiled paths and typed root/identity readers to derived targets. Update
resolves every path before input sampling, validates canonical field/root identity
and registered source nodes, and automatically computes dependency edge changes.
Ordered source-node signatures preserve duplicates and group order separately
from deduplicated graph edges, invalidating targets on meaningful traversal changes.
Manual and automatic targets coexist; one target cannot use both. Source-bound
roots must remain present and map to their registered key.

ShadowInputs.for_field reads dynamically selected dependencies by canonical metadata,
with foreign-schema/type validation. Values are unique-node registration order,
not raw group multiplicity. Restore validates signatures plus sampled inputs and
restores cache, manual edges and resolved edges without suppliers. No automatic
field setters, Java reflection or changing-node-universe claim.

Read all three TestdataMixed fixtures; reused full MixedModelTest read. Adapted
its parent-delay method using actual native mixed temporary change and compiled
previous/previous.delay paths, with extra initial/undo checks. Three original tests
cover chain rewiring/cycles/restore/null, group order/duplicate/empty transitions,
missing sources, root identity and conflicting bindings. Existing session tests
cover manual-edge and panic behavior; added for_field metadata/type checks there.

Compile94704 failed on missing imports from an exact-text edit; fixed imports.
Focused35875 exited0: eight tests (four session, three source binding, one source
case). Added for_field metadata checks afterward; final gate verifies final source.
Provenance appends one case and three fixtures; prefixes unchanged. No production
implementation read. Source and provenance frozen for the selective gate.

Required gate22842 exited0. 3751 Rust tests and 176 doctests passed.
Rust jobs: {'RUN': 5, 'REUSE': 459}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2180, 'failed': 0, 'pending': 2393, 'excluded': 282, 'unverified': 0, 'percent': 44.902}.
Source before/after matched: c1af6b795d2e5023db15b18c63c33b4b8e356566eb9f083749b8c3a159a41302
Live freshness matched verification digest: 34ad2547131e2ae16f8564ef797a34bf7ac6920c05d103731e9c5ccc0d81500c.
Existing provenance prefixes unchanged; appended1case and3fixtures.
Log work/shadow-source-bindings-final-check.log; evidence work/shadow-source-bindings-evidence.json.
No benchmark/browser/release reruns. Goal remains active.

## Typed shadow sessions inside stream scoring (current increment)

Previous goal turn bba6576 made progress; starting tree clean. Added graph-native
map_shadows in stream_graph/shadows.rs. It consumes a ShadowSessionBuilder and
projects complete world snapshots into normal stream tuples. Per-input identity
stores an independent session checkpoint in SnapshotMap. Scratch session installs
that identity's state before update. Graph checkpoints contain only owned cache
state; restore/clear install the initial scratch state without callbacks. Unknown
sources/projection errors become GraphError::Dependency. External callback side
effects remain outside rollback; suppliers must depend only on registered inputs,
compiled path signatures and immutable facts. No automatic model setter claim.

ShadowSession checkpoints now clone persistent caches and retain pending/requested
edges internally. Public checkpoint still requires current state; public restore
still validates source signatures and sampled inputs. Internal capture/install
support graph rollback without resampling readers. Checkpoints retain identity.

Read ListModelTest and all four simple_list fixtures in full. Adapted exact empty
required list, positions/durations, compiled entity/previous.endTime/startTime
sources, null behavior, total-end-time penalty, -344 goal, final v3/v1/v2 order
and six time assertions. Real solve_list_stream constructs and optimizes; final
fields come from the same graph runtime bound to the returned solution. Native
assert_scores checks both phases; deterministic bounded search (1000 evaluations,
32 local steps) replaces Java solver configuration. Four fixtures appended with
one case; existing provenance prefixes unchanged. No production source read.

Compile16998 and focused25249 exited0: source case and four existing session tests.
Original70056 failed because the saved-winner test used default FirstAccepted with
one step but expected the best candidate. Set BestAccepted explicitly; focused4508
exited0 (three original tests). Added projection-count assertion after that focused
pass; final gate will verify final code. Tests cover independent identity caches,
non-Clone fields, retraction/reinsertion, reader/supplier/projection/score panics,
projection error, nested list moves and saved candidate installation. No benchmark
or UI work; source/provenance frozen for required selective verification.

Next specifications read while the gate runs: full AffectedEntitiesUpdaterTest
(still pending) and full DeclarativeShadowVariablesMethodVariablesTest plus all
five shadow/method_variables fixtures (still pending, not adapted). Method-variable
case: two initially empty owners A/B; subclass values codes 1,2,31. Getter-declared
previous exists on base and covariant override on subclass. codeSum supplier is
previous.codeSum + code, or own code for no predecessor; initialized codeSum=code.
Hard penalty counts duplicate characters per sum; soft reward sums lengths.
TRACKED_FULL_ASSERT, score calculation limit1000, final HardSoftScore(0,6).
Native metadata/getter registration and true solver execution are necessary;
merely asserting a hard-coded score or classification does not establish behavior.

Required gate80610 exited0. 3755 Rust tests and 177 doctests passed.
Rust jobs: {'RUN': 409, 'REUSE': 57}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2181, 'failed': 0, 'pending': 2392, 'excluded': 282, 'unverified': 0, 'percent': 44.923}.
Source before/after matched: bca7f572925aba7527dd6798042995e807dcbf2103077a2e3f64d72d45d058a3
Live freshness matched verification digest: 3fea31ed1b0bf4bf7109810ff6f76e5ecac4974a60e401984ed7d275ffae640e.
Existing provenance prefixes unchanged; appended1case and4fixtures.
Log work/shadow-stream-final-check.log; evidence work/shadow-stream-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Fixed shadow cycles and accessor-model solves (current increment)

Previous turn d01a7c2 was progress; starting tree clean. ShadowSessionBuilder retains
compiled on-root source fields at bind time and validates their registered same-key
nodes using DependencyTopology. Only actual cycle members contribute sorted unique
field names to FixedDependencyCycle. Missing root sources reject before callbacks.
New validate_fixed_dependencies and try_build perform no readers/root getters or
suppliers. Existing low-level build stays permissive, manual edges remain mutable,
and previous/group/object references stay dynamic. map_shadows now uses try_build,
so real solver graph initialization rejects permanent source-path loops.

Read full AlwaysLoopedShadowVariableTest, ExtendedValuesTest and their eight fixtures.
Reused last turn's complete MethodVariablesTest and five fixtures. Three source cases:
- Real solve_stream rejects odd/even root cycle with both names, source 3 entities,
  required range1..5 and target0; original null/fallback suppliers/parity penalties
  retained, plus no-callback assertion.
- Two real list solves preserve A/B, codes1/2/31, previous.codeSum concatenation,
  duplicate hard penalty and length soft reward; native score assertions and1000
  evaluation limit reach0hard/6soft. Getter-list versus field-list adapters differ.
  Rust BaseValue composition and explicit getter registration replace Java class
  inheritance/annotation discovery; no broad inheritance/reflection claim. All
  source instances are subclass instances. Independent final objective checks added.
Provenance appended3cases and13fixtures; original prefixes preserved.

Focused50009 exited0 (accessor cases before diagnostic text change); focused56384
exited0 (all3source cases on updated code). Original86411 exited0 (2tests): exact
cycle members excluding downstream, self loop, missing registration, no callbacks,
and map_shadows permits dynamic predecessor cycles, restores trial cache and recovers
from committed cycles. No upstream production implementation read. Source/provenance
frozen after formatting for required gate; no benchmark/browser/release reruns.

Read full GraphStructureTest (144 lines) during final gate: 12 pending metadata
classification cases. Empty/base-only roots are EMPTY; previous-only list roots
are SINGLE_DIRECTIONAL_PARENT/PREVIOUS; actual nonempty concurrent groups and
followers require arbitrary single-entity handling; multi-entity/list-element or
multiple active directional parents are ARBITRARY. Empty groups do not force the
arbitrary strategy. Any future classification work should drive actual execution
strategy selection, not just introduce labels to claim these cases.

Continuation checkpoint: required final gate is LIVE as session97019, launched with
matching login environment and logging to work/fixed-shadow-final-check.log.
Last authoritative poll returned session97019 running; log reached explanation_diff_regression.
Do not restart, rerun focused tests or change frozen source merely for a new turn.
Poll97019 until terminal, then inspect report/source metadata and live /api/freshness,
append evidence, review/stage the handoff tail and commit this increment. Eight
intended files, including this handoff, are staged. All
focused handles50009,56384,86411 are terminal0. No final-gate success claim yet.

While gate97019 continued, read full VariableChangeRecordingScoreDirectorTest
(564 lines, tests only), a useful next implementation target: 21 pending methods.
Each list before/after bracket records one undo range plus one before/after undo
notification pair. Interleaved entities pair independently by stable identity;
sequential same-entity brackets remain separate and undo in reverse. Exact tests
cover same range replacement, removal/insertion across owners, swap, nested cross
owner brackets, batched3-owner rotation, identical/overlapping/touching sequential
ranges. Non-delegating recorder handles share pending records. Undo handles are
live: created before any before or during an open bracket, they see later closure
and changes. Undo clears pending state so later brackets can open. Reject duplicate
before, orphan/duplicate after, mismatched from-index (also across shared handles),
reported length delta differing from actual, and undo with an unclosed bracket.
Preserve exact restored lists, notification ranges/counts and one shadow-update
notification for tests that assert those. Only the test was read this turn; its
three core/testdomain/list fixtures need inspection before adapting. Existing
native RAII move sessions are not automatically proof of this lower-level event
recording contract. Do not copy referenced production classes from comments.

Verification breadth observed: scripts/verification_inputs.py selects all children
of a selected Rust module and associates every out-of-line impl with its target
type. Therefore stream_graph/shadows.rs brings ShadowSession dependencies into
unrelated StreamGraphBuilder jobs. This explains the broad reruns on these recent
batches; separate job keys exist but this dependency scan remains conservative.
Any selective extension-method refinement needs actual cache tests and must retain
original evidence dates/checksums; do not merely rewrite keys on old results.

Required gate97019 is now TERMINAL0 (supersedes earlier live checkpoint).
3760 Rust tests and 177 doctests passed.
Rust jobs: {'RUN': 412, 'REUSE': 57}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2184, 'failed': 0, 'pending': 2389, 'excluded': 282, 'unverified': 0, 'percent': 44.985}.
Source before/after matched: 15c00df8320704b13523d244a540e8b78acb89eb37aafb24f812f4a5db8d4997
Live freshness matched verification digest: a100b2e7c0c77341e0ce4911fdad0f5ba5d0935c5ff76adf1b11381fac4a69f0.
Existing provenance prefixes unchanged; appended3cases and13fixtures.
Log work/fixed-shadow-final-check.log; evidence work/fixed-shadow-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## List change recording (current increment)

Previous user-facing goal activation turn only confirmed goal status; revalidated
worktree and resumed existing implementation. Initial compile64293 terminal0.
Added ListChangeRecorder and live ListRecordingUndo for canonical list fields,
shared/non-delegating handles, independent entity brackets, reverse undo and exact
range notifications. Entire reverse replay validated before callbacks; snapshots
publish and shared recording clears only after callbacks succeed. Ordinary panic
preserves caller data/records; external callback effects not rolled back. Explicit
observer API, whole-list snapshots; no automatic setters or performance claims.

Read full VariableChangeRecordingScoreDirectorTest and reused fully read three list
fixtures. Adapted all21 methods preserving lists, ranges, counts, event timing and
errors; native Result variants/diagnostics replace Java exceptions. Stronger exact
reverse notification order also asserted. Added three original tests: actual
committed ListProblem move restores owner/index shadows and stream score through
one final registry publication; invalid complete replay emits no callbacks; panic
at every event leaves lists/records recoverable; invalid ranges/outside edits can
be repaired. Forward suppression and Busy during replay covered.
Focused18520 terminal0:21source tests. Native28658 terminal0:3tests. Provenance
appended21cases and3fixtures, existing prefixes preserved. No upstream production
implementation read. No benchmark/browser/release reruns. Source now frozen for
one required final gate; do not repeat focused tests after a context reset.

While final gate runs, read MoveDirectorTest lines2195..2322 (three complete
methods) as possible next targets, not yet credited: lookUpWorkingObject delegates
a value to backing lookup, requiring a distinct object with equal code;
undoNestedPhaseMove mocks ruin/recreate where initially selected owner e1 losesv1
and previously unselected e2 gainsv1. It asserts recorded before-actions for e1
[v1] and e2[v2], each range0..1, despite mocked final duplicate v1 setup. A faithful
native adaptation needs real dynamic owner discovery/recording, not just old RAII
undo success. testSolverScopeNestedPhase asserts nested reconstruction uses a
fresh solver scope exactlyonce and never passes main scope. These are individual
methods, not a full-file review; do not read production classes they reference.

Required gate2899 TERMINAL0. 3784 Rust tests and 178 doctests passed.
Rust jobs: {'RUN': 3, 'REUSE': 468}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2205, 'failed': 0, 'pending': 2368, 'excluded': 282, 'unverified': 0, 'percent': 45.417}.
Source before/after matched: 193e7988d1bed0fda9a2c343854e22de7ee4c8b32c81817c3e399cab8661c4c7
Live freshness matched verification digest: e3463bcda5542cf99f712e794c2384658b1fad6c7c84b5461916b6166c086679.
Existing provenance prefixes unchanged; appended21cases and3fixtures.
Log work/list-recording-final-check.log; evidence work/list-recording-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Solution tracking diagnostics (current increment)

Previous goal turn afd152b was progress; tree clean. Read full SolutionTrackerTest
and four TestdataSolution/Entity/Value/Object fixtures (tests only). Implemented
SolutionTracker with explicit heterogeneous typed scalar/list readers, owned
snapshots, exact Eq comparison (not formatted text), canonical schema validation,
separate move/undo event windows, and before/undo/from-scratch reports. Native
registration returns independent VariableEventTracker; new public tracker.new
supports shadow fields; use_events attaches an existing registry tracker. Only
registered fields covered; caller supplies full recomputation snapshots. No
automatic score interception or whole-solver verification claim.

All seven source cases preserve exact code strings, null/value edits, capture
sequence, diagnostic sections/messages and formatter truncation. Four fixtures
attributed. Original three tests cover real ListProblem move and registry events,
stale shadow totals on undo, new-move reset, heterogeneous and non-Clone values,
identical display labels, removed/added versus null entities, foreign snapshots,
registration metadata and duplicate-stage errors, reader panic/event recovery.
Compile88165 terminal0; initial focused93874 and native64478 passed7+3 cases.
Added stage-order guards and API doctest after those runs; final-focused70498 is
pending terminal inspection. Source will freeze after formatting/provenance for
one final gate. No benchmark/browser/release runs requested or warranted.

Also read full DiminishedReturnsScoreRingBufferTest (18methods) and existing native
DiminishedReturns implementation. Source tests specify internal array slots/read
and write indices, including stale slots; native policy uses VecDeque already.
Do not create Java-shaped buffer storage just to claim these tests. RootVariableSource
remaining cases mainly require reflection/accessor preference; full output was
truncated, so no full-file read claim. Existing adapted source-path cases remain
credited. Prefer missing functional solver behavior over internal-label adapters.

Final-focused70498 terminal0: all10 tests pass with stage-order guards. Format62395
terminal0. No other live sessions before final gate. Source/provenance frozen.

During final gate, inspected DefaultConstructionHeuristicPhaseTest as next useful
functional target (24 pending methods). Read lines1..245 (header + first5 complete
cases, start of empty-entity case),373..455 (PART of common configuration),586..799
(complete advanced-basic config + four basic comparator/factory methods),997..1055
(two list comparator/factory methods),1225..1250(entity-factory method),1316..1349
(value-factory method + start of optional penalty case). Not full-file read.
Comparator/factory cases require ALL generated configurations and fixtures before
credit: expected per-entity comparator ranks either[0,1,2] or[2,1,0]; all populated,
list owners exactlyone each, deterministic shuffled-order independence when both
axes sorted. Advanced basic configs use queued entity/mimic selection and phase
cache, descending entities, ascending/descending values with early feasible/hard
non-deteriorating choice. Current ConstructionConfig has placements and early-pick,
but no direct entity/value order field; check provider/selection integration before
choosing an API. Do not merely reorder a fixture's model to emulate dynamic sorting.

Required gate10507 TERMINAL0. 3794 Rust tests and 179 doctests passed.
Rust jobs: {'RUN': 15, 'REUSE': 458}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2212, 'failed': 0, 'pending': 2361, 'excluded': 282, 'unverified': 0, 'percent': 45.561}.
Source before/after matched: 37d5d6fb3707a3425fca190c44f147daf21ba7dbf871749109eed773e049ada0
Live freshness matched verification digest: f60e834c1d2e3c95d9b0854028663faa2d1cb31a70a8b25e1deff208a553ae88.
Existing provenance prefixes unchanged; appended7cases and4fixtures.
Log work/solution-tracker-final-check.log; evidence work/solution-tracker-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Sorted construction providers (current increment)

Previous goal turn73aebe1 was progress; starting tree clean. Added phase-scoped
SortedBasicConstruction/ListConstruction providers on existing QueuedMoveProvider;
no parallel sorting engine or model reordering. Stable comparator ordering, actual
initial-state factory, sequential/valuequeue/pooled placement, live eligibility,
optional-policy handling, pins, receiver ranges and list prefix insertion bounds.
Domains stored as Arc slices and move values cloned on consumption. reset rewinds
same order; reconstruct provider for new phase-dependent priorities. Bound model
identity asserted on use. Preparation outside search time budget. Per-step dynamic
sort/mixed adapters remain future work; no broad config or performance claim.

Read complete source generateEntityFactorySortingConfiguration and
 generateValueFactorySortingConfiguration plus both corresponding methods and
nested easy score calculators. Reused/read seven basic/list fixtures, read both
TestdataObjectSortableDescendingComparator and Factory. Adapted both methods with
all4alternatives each(two factory/two comparator) on both actual solver backends.
Exact[2,1,0] assignments, original duplicate and list-size objectives, extra score
and factory-once checks. Provenance2cases13fixtures; existing prefixes unchanged.

Initial compile57952 terminal101 (moved value in list iterator); replaced filter
and map with one closure. Initial focused67600 terminal101 (test extension type
shared across incompatible backends); each branch now owns typed extensions.
Focused21133 terminal0: both source cases pass across16runs. Native8190 terminal101:
a test expected cyclic value ordering to resume after explicitly rewinding it;
reset correctly restarts values. Corrected reset test to use fresh initial state,
keeping separate atomic-interruption retry test. Native36808 pending inspection.
No production upstream sources read. Final gate follows formatting/source freeze.

Native36808 terminal0: all3regressions pass. Format14963 terminal0. Source and
provenance frozen for one required gate. No other live processes from this batch.
No benchmark/browser/release reruns.

Gate20016 TERMINAL101 at clippy: two filter_map_bool_then warnings in new list
candidate cursors. Replaced those bool::then expressions with explicit conditional
Options (same behavior, avoids splitting ownership across two closures). Original
log preserved as work/sorted-construction-check-attempt1.log. Final gate rerun is
required for failed check/changed source; focused behavior suites need no extra
standalone rerun because the required gate will execute the changed jobs.

Next-source preparation: read remaining common configurations throughline586 and
advanced list configuration800..996 (small truncated section recovered920..951),
plus list entity-range generator1056..1091 and its two methods1095..1146. Existing
common/advanced-basic configurations were already read. Advanced-list methods
compare ascending/descending values against descending or explicitly unsorted
owners. Under STEP owner cache, an unspecified owner sorter defaults descending;
under PHASE an explicit NONE hack keeps encounter order. Entity-range configurations
use STEP sorting and one extra sorted destination-value/range-filter case. These
remain uncredited: per-step comparator evaluation should be supported before
claiming those configurations. Their sortable fixtures still need full inspection.

Required gate44188 TERMINAL0. 3799 Rust tests and 180 doctests passed.
Rust jobs: {'RUN': 3, 'REUSE': 472}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2214, 'failed': 0, 'pending': 2359, 'excluded': 282, 'unverified': 0, 'percent': 45.602}.
Source before/after matched: 27b962b9e26705ea3f63b839eae7ed02014b49ee541972903916e3eb3deba339
Live freshness matched verification digest: b4c456e2161e420854827083ee5df82ed6a3f000d79c2998edefb7c4d61816b5.
Existing provenance prefixes unchanged; appended2cases and13fixtures.
Log work/sorted-construction-final-check.log; evidence work/sorted-construction-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

Read full next-target score fixtures while waiting: sort/comparator/
OneValuePerEntityComparatorEasyScoreCalculator is HardSoft(-repeated,-distinct),
where assigned excludesnull and repeated=assigned-distinct. list/sort/comparator/
ListOneValuePerEntityEasyScoreCalculator subtracts10soft per exactly-one owner,
10hard otherwise, plus1hard per owner regardless of size. These differ from this
increment's factory-sorting SimpleScore objectives; do not reuse the wrong score
when adapting remaining comparator tests. Other sortable fixtures still unread.

## Step-scoped construction sorting (current increment)

Previous activation-only goal turn was no progress; revalidated active goal and
existing uncommitted sorter changes. Continued those changes rather than restart.
Added StepSortedBasicConstruction/ListConstruction wrappers and with_step_sorting;
phase keys fixed, candidate comparators refreshed from current state per step,
retry cache, reset, stable original encounter ties and atomic order publication.
Added usage doctest. Native tests previously completed66712 TERMINAL0 (3 tests).
Source harness43493 TERMINAL0 (8 tests, 404 full solves over both backends).
Generator first failed before writing a source file (f-string brace), corrected;
no solver failure. Format53123 TERMINAL0. No production upstream code read.

Read all eight target queue comparator/factory methods and six configuration
helpers in DefaultConstructionHeuristicPhaseTest, plus all24 model/score fixtures
under testdomain/{sort,list/sort,valuerange/sort,list/valuerange/sort}/{comparator,
factory}, four shared sortable interfaces/comparator/factory/value fixtures and
TestdataObject. Source generators retain19 common +4 basic or8 list alternatives;
entity-range-list adds one corner configuration and uses STEP destinations. Java
stdlib-only seed-zero shuffle harness95832 terminal0 supplied exact permutations.
All score functions retain HardSoft objectives (distinct/repeated basic; size-one
list hard/soft penalties). Immutable ranks replace Generated codes, assertions use
stable entity identity despite shuffle. All alternatives retained, including
repeated selector/mimic variants expressed by native provider wiring.

Appended8 source cases and35 fixture/helper records, existing prefixes preserved.
Focused source tests and native regressions passed; final required gate next after
source freeze. No benchmark/browser/release reruns. Goal remains active.

Next-source preparation during gate: read complete DefaultConstructionHeuristicPhaseTest
methods117..372 (initialized/pinned/empty/optional/range cases) and1342..1564
(optional penalties, invalid selector/reflection configurations and nested score
providers). These remain pending, not credited. Initialized-solution case requires
an independent output assignment plus shadow recomputation; optional cases require
exact source easy-score fixtures before adaptation. Invalid Java reflective cache/
annotation combinations need native semantic rationale, not manufactured errors.
Potential implementation gap: component construction providers cannot yet compose
into a mixed-model provider with independently tracked queue/step state; sorted
basic/list providers are reusable building blocks. No mixed adapter implemented.

Required gate26722 TERMINAL0. 3810 Rust tests and 181 doctests passed.
Rust jobs: {'RUN': 5, 'REUSE': 472}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2222, 'failed': 0, 'pending': 2351, 'excluded': 282, 'unverified': 0, 'percent': 45.767}.
Source before/after matched: 7fa1264e6145cb555ccfea0ed3a4859544b6130fb556c2042aaa9d9182934891
Live freshness matched verification digest: e3343fac6e53c948f81550e82a5df9eed72b9f70d5c396454ff57852c7bb7d5d.
Existing provenance prefixes unchanged; appended8cases and35fixtures.
Log work/step-sorting-final-check.log; evidence work/step-sorting-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

Also read four pending-case easy score fixtures completely: unassignedvar/
TestdataAllowsUnassignedEasyScoreCalculator (-1 per null, -1000 per ordered pair
of distinct entities with equal nonnull values); list/unassignedvar/
TestdataAllowsUnassignedValuesListEasyScoreCalculator (negative total list length);
valuerange/entityproviding/unassignedvar/TestdataAllowsUnassignedEntityProvidingScoreCalculator
(-1 null, +1 assigned); list/valuerange/TestdataListEntityProvidingScoreCalculator
(+2 owner size>=2, +1 otherwise). Their model fixtures still need attribution/read
before adapting those cases. No credit added from these preparatory reads.

## Mixed component construction providers (current increment)

Previous goal turn6733904 was progress; starting tree clean. Added
MixedConstructionProvider owning basic/list providers and using existing mixed
construction scoring. BasicThenLists/ListsThenBasic, separate child counters driven
by completion notifications, empty/fully-assigned skip, optional-rejection handoff,
lazy one-item prefetch, pointer binding, reset_with explicit child resets.
Resuming from returned assignment retains child counters although outer solve
counter restarts. Direct callers must notify completed scans. Cross-component
atomic moves remain custom providers; exhausted optional components aren't revisited.

Compile14661 terminal0. First focused70487 terminal101: test passed a basic
SolverSequence where MixedSolverSequence is required; corrected test configuration.
Focused73433 terminal0: first3integration tests pass over both backends/orders,
including heterogeneous types, actual joint objective, dynamic sorted factories,
atomic retry, partial solve resume/reset and optional rejection policies. Added
2more native tests for lazy/zero-budget/foreign-model behavior and pins/ranges.
No upstream cases adapted; provenance unchanged. Final required gate next.
No benchmark/browser/release reruns. Goal remains active.

Format12494 terminal0. Gate66758 terminal101 while compiling the new lazy test:
CountingBasic's moves signature omitted the trait's Move<i32>:'a bound. Corrected
the test helper signature; production implementation unchanged. First gate log
retained as work/mixed-provider-check-attempt1.log. Rerun final gate for failed
compile/changed test input; no redundant standalone full-suite run.

Prepared next source adaptations during gate: read complete optional-sort fixtures
unassignedvar/sort/TestdataAllowsUnassignedSortable{Entity,Solution} and
list/unassignedvar/sort/TestdataAllowsUnassignedListSortable{Entity,Solution}, plus
unassignedvar/TestdataAllowsUnassigned{Entity,Solution}. Single-value penalty cases
use generated rank0, null basic/empty list, HardSoft -1 hard per assigned basic or
nonempty list owner, and step limit3. These six fixtures are read but not adapted
or credited. Their Java descriptor imports were not followed into production.

Required gate76892 TERMINAL0. 3815 Rust tests and 182 doctests passed.
Rust jobs: {'RUN': 2, 'REUSE': 476}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2222, 'failed': 0, 'pending': 2351, 'excluded': 282, 'unverified': 0, 'percent': 45.767}.
Source before/after matched: 2cd51970ce90c4c117fbac5072b08c8a5ec3334182a6e283574f332677fca80f
Live freshness matched verification digest: fbb91ed0538ad294118288556b540a9620b472d703534e1bc59324ebce3afdbf.
Provenance unchanged; native capability only, no additional source-case credit.
Log work/mixed-provider-final-check.log; evidence work/mixed-provider-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Construction phase functional cases (current increment)

Previous goal turn6f72da2 was progress; starting tree clean. Adapted9 remaining
functional DefaultConstructionHeuristicPhaseTest methods with full construction
solves on both backends (18 runs): initialized values/independent output storage,
pins/optional-null pin, empty entities,
entity ranges and hard-score optional penalties. Preserve source helper best-event
assertion. No production solver change was necessary; these source cases close
compatibility evidence gaps. Required pinned-null exception case remains pending:
existing native solver_resume_regression explicitly permits partial construction
around it and returns no complete solution. Do not manufacture an exception test.

Read full PlannerTestUtils (adapt buildSolverConfig's Dummy zero calculator and
solve's best-event-nonempty assertion only); DummySimpleScoreEasyScoreCalculator;
pinned entity/solution and pinned/unassignedvar entity/solution; optional list
entity/solution/value; entity-provided optional basic entity/solution; entity-provided
list entity/solution/value; reread basic TestdataEntity (no shadow variables).
Other basic/common and optional-sort fixtures and four easy-score fixtures already
read in previous turns. No production implementation imports followed.

First focused compile failed: collect_list emits ListSnapshot instead of slice and
untyped observer closure did not satisfy higher-ranked event lifetimes. Corrected
native harness collection and observer parameter types. Focused68113 TERMINAL0:
11 focused cases passed, then provenance duplicate detection found two optional
cases already credited in construction_optional_cases.rs. Removed those duplicate
tests and their unused fixture records; retain existing coverage/evidence. Final
batch adds9cases26fixtures; existing entries unchanged.
No benchmark/browser/release reruns. Final required gate after source freeze.

Next-target investigation: queried current report pending identities before reading
DefaultSolverTest;43 pending methods include score corruption/environment-mode cases.
Read file1..265 (imports and first4methods),386..551 (corrupted easy cases plus
empty/pinned/custom/partial initialization),1400..1544 (sort/mixed cases mostly
already credited), and2559..2577 (CorruptedEasyScoreCalculator creates Random and
returns random0..999 each calculation; TestingNeighborhoodProvider throws).
Do not claim full-file read. ScoringError currently identifies corrupted constraint
index or total only; assert_consistent recomputes contributions but discards cached/
actual score values. Rich typed score-consistency diagnostics across basic/list/
mixed directors are a useful next implementation; source full-assert cases expect
working/recomputed score diagnostics. Deterministic changing-score callbacks should
replace random corruption in adapted tests, documented explicitly. Native event
semantics differ for incomplete best snapshots; don't copy JVM event filtering
implicitly. Existing successful jobs remain reusable until relevant source changes.

Required gate57148 TERMINAL0. 3824 Rust tests and 182 doctests passed.
Rust jobs: {'REUSE': 478, 'RUN': 1}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2231, 'failed': 0, 'pending': 2342, 'excluded': 282, 'unverified': 0, 'percent': 45.953}.
Source before/after matched: fecc913d52911913106e34fb724199a4705e74193534902791d5f6bef95b14cd
Live freshness matched verification digest: d478d2640efc529a6f91ac933cbea1e35dcfe19d53f13ac33d561b93ddeaa60f.
Existing provenance prefixes unchanged; appended9cases and26fixtures.
Log work/construction-phase-final-check.log; evidence work/construction-phase-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Scoped score consistency diagnostics (current increment)

Previous goal turn41818da was progress; starting tree clean. Added typed
ScoreConsistencyReport/ConstraintScoreDifference and boxed ScoreCorruption error
evidence. Basic/list/mixed directors recompute all scoped contributions exactly once,
retain all differences and structural totals. assert_consistent uses the report.
Candidate selection assertions overlay sparse updates on the cached contribution
vector then compare every constraint; previously only totals were checked, missing
cancelling scope errors. Normal unchecked candidate scoring has no new work.
Report/error formatting bounded5rows; structured data complete; equality typed.
Legacy CorruptedConstraint variant replaced by ScoreCorruption, updated4 native
regressions. Stream graph error format/match checking remains unchanged.

Compile73199 terminal0. First focused3340 terminal101: candidate select_best is
crate-private; moved candidate regression to internal unit tests. Combined92585
finished0 but its first lib command failed (used bool instead of early-pick closure);
its 3 integration report tests passed. Corrected policy closure; focused59355
terminal0 for all-three-model cancelling-scope candidate detection/rollback test.
Source10487 terminal0: three corrupted easy-score cases pass. Format42750 terminal0.
No production upstream implementation read. Source methods386..455 and nested
calculator2559..2567 read previously; actual Random corruption replaced with
counter%1000 for deterministic tests. Full-assert diagnostic wording adapted to
native actual named constraint evidence; PHASE_ASSERT case remains pending.
Appended3cases6fixtures; prefixes unchanged. Frozen source ready for final gate.
No benchmark/browser/release reruns. Goal remains active.

Gate54929 terminal101 at clippy: needless_question_mark on mixed report helper's
Ok(with_structure(...)?) return. Replaced with direct Result return. No behavior
change; retain failed log as work/score-consistency-check-attempt1.log. Required
gate rerun follows failed lint/changed source. Focused behavior results still valid.

Prepared stream diagnostic follow-up while gate runs: existing StreamGraph
assert_consistent uses a rollback transaction, checkpoints all nodes, clears/replays
source rows, compares total plus Node::scores_match (terminal match identities/
scores and dataset payloads), and restores on error/panic. GraphTransaction asserts
poison its caller on failure while preserving score_evaluations. Read complete
native regressions stream_search.rs fresh_graph_assertions_detect_cancelling_match_changes_and_restore_on_failure
and stream_dataset_regression.rs diagnostic_rebuild_compares_dataset_payloads_even_without_scores.
Graph errors currently discard changed-node identities and cached/rebuilt totals.
A future typed stream report must retain dataset-only mismatches and cancelling
per-match changes even when constraint totals equal; an aggregate-score-only report
would lose existing guarantees. Node IDs and named constraints can be reported
without adding Debug bounds to generic tuple identities. No stream changes yet.

Prepared next batch without changing frozen gate inputs: work/solver_neighborhood_cases.rs
contains six pending DefaultSolverTest adaptations (solveWithNeighborhoods,
solveWithDefaultNeighborhoodProvider, solveWithNeighborhoodsAndMoveSelectors,
solveWithDefaultNeighborhoodProviderMultiVar/MultiEntity/ListVar). Both backends,
full source objectives/fixtures, seed20261007, 200steps/10000moves, target0, events,
validity and input preservation. work/add_solver_neighborhood_provenance.py is
prepared but NOT run (6cases18fixtures). Install only after current gate/evidence
and commit so reporting inputs remain stable. Draft compiled directly against
current already-built rlib; session71667 terminal101 exposed partial stream replay
rows causing indexed-objective panic. Fix materializes a full None-filled variable
vector by stable index; session44009 terminal0 all6methods/12backend solves.
These are development checks only, not yet dashboard coverage. No source fixture
credit before provenance/install and final gate. Read source DefaultSolverTest
184..386,2578..2685 and full multivar/multientity fixtures. Basic TestdataSolution
generateSolution takes value count FIRST; use3values/2entities. Source easy objectives
penalize distinct forbidden identities, not multiplicity. Native bounded seeded
hill climbing crosses plateau in multi-entity fixture. Java preview enable/rejection
and variable-neighborhood-descent cases remain pending. PlannerTestUtils.solve
asserts nonempty Best events; all adapted helpers retain this assertion.

Gate37746 terminal101 only at the new score_consistency doctest: default i32 inference conflicts with i64 SimpleScore multiplication. All3831 Rust tests passed;182of183doctests passed. Make both example inputs explicitly i64; production implementation unchanged. Retain attempt2 log. Rerun required gate using matching successful Rust jobs; do not repeat broad successful tests manually.

Required gate26023 TERMINAL0. 3831 Rust tests and 183 doctests passed.
Rust jobs: {'RUN': 1, 'REUSE': 480}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2234, 'failed': 0, 'pending': 2339, 'excluded': 282, 'unverified': 0, 'percent': 46.014}.
Source before/after matched: d25d0e80c99c74e01f2c0b7f57bbde2d9a0562061e5d80e310d0506e34a06588
Live freshness matched verification digest: 227158ed5f9d14fbe914e5ba0137f6f338f203e37f31413a6f74974309ba57a7.
Existing provenance prefixes unchanged; appended3cases and6fixtures.
Log work/score-consistency-final-check.log; evidence work/score-consistency-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Full-solve neighborhoods

Scoring diagnostics committed as0e6a31a after gate26023 and evidence71819 terminal0.
Installed previously checked work/solver_neighborhood_cases.rs into tests; appended
6sourcecases18fixtures with prepared provenance script. Original objective and every
source helper assertion retained in both backends. No solver implementation changes.
Draft focused44009 passed all6methods/12solves against current compiled implementation;
formatting-only changes since. Full gate follows on final test/provenance state.
Java preview-mode rejection and VND remain pending. No benchmark or UI changes.

Required gate61390 TERMINAL0. 3837 Rust tests and 183 doctests passed.
Rust jobs: {'REUSE': 481, 'RUN': 1}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2240, 'failed': 0, 'pending': 2333, 'excluded': 282, 'unverified': 0, 'percent': 46.138}.
Source before/after matched: c8bf273bb959adf39256b795dde6229907c76ee87ce1f86cfa6e764eee246c99
Live freshness matched verification digest: 6d059b5d4bffef31e96de9cf6d343c890f5b326abb451332af4803177468db45.
Existing provenance prefixes unchanged; appended6cases and18fixtures.
Log work/solver-neighborhood-final-check.log; evidence work/solver-neighborhood-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Variable-neighborhood descent (in progress)

Prior turn99b2005 completed two commits; starting tree clean. Added separate
variable_neighborhood_descent module implementing NeighborhoodUnion::descent_config
and DescentRequiresSequentialOrder. This composes existing sequential union,
StrictImprovement, FirstAccepted, Original order and Stop rejection; no second
search loop or core scoring changes. Zero children disabled, empty union valid;
random ordering errors. Config remains mutable; overrides/custom providers can
change algorithm. Read pinned PUBLIC docs local-search.adoc709..762 (not production):
first improving move, original family order, restart first after improvement.

Existing move_union_regression trap case uses named config for sequential branch;
random branch preserved. New native tests exercise priority against a higher score
in a later family, disabled child, one-step/zero-candidate limits, tie/empty stop,
and random rejection. Existing solver_neighborhood_cases helper now takes mode;
six prior cases preserve their exact configuration, seventh uses named descent.
Appended1sourcecase6fixtures, preserving previous provenance prefixes. Source
DefaultSolverTest301..316 and basic source fixtures already read. Focused79313
is running for3testtargets. Full gate/commit still required after focused success.

Focused79313 terminal0: all14 tests across3targets passed. This includes source
VND solve in both backends and the existing six-model/backend union regression.
Final source frozen; required gate follows. No benchmark/browser/release reruns.

Gate68360 deliberately terminated143 (verified process group29693) to fix avoidable broad invalidation before wasting another full rebuild. Input scanner attaches out-of-line impl blocks to every reference of their type; adding an impl for NeighborhoodUnion pulled the new helper into unrelated search targets. Expose the same policy through standalone variable_neighborhood_descent(&union) instead; matches existing free-function solver API and keeps dependency graph narrow. No cache records or old hashes rewritten. Preserve attempt1 log; previously completed matching jobs remain reusable. Updated docs/provenance to final free-function API. Required final gate still pending.

Free-function focused38031 terminal0:14tests passed. Explicitly import
variable_neighborhood_descent in the3consumer tests: the current heuristic skips
unqualified calls directly after struct-update '..', whereas named imports are
tracked. Inspection21850 terminal0 confirms helper included for all3affectedtargets
and excluded from solver.rs/assignment_controls_regression.rs. This changes neither
cache plumbing nor hashes of old evidence. Imports/formatting finalized after
focused compilation; full gate will verify final state. No further source edits.

Required gate50985 TERMINAL0. 3840 Rust tests and 184 doctests passed.
Rust jobs: {'RUN': 30, 'REUSE': 453}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2241, 'failed': 0, 'pending': 2332, 'excluded': 282, 'unverified': 0, 'percent': 46.159}.
Source before/after matched: fa573b17f0011cfa4a922d49ed33a40ec5dc55ac9488df5f2218ea9f67ef437d
Live freshness matched verification digest: e4b93e440c0ba4127d4bb8e6b8f5121464aebc8743ac7b85bf8e4d61d816ea24.
Existing provenance prefixes unchanged; appended1case and6fixtures.
Log work/descent-final-check.log; evidence work/descent-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Constraint assertion API

Starting HEAD a0fd397. Added separate constraint_assertion module with tracked integer/
decimal terminals and detached assertion snapshots. Retains raw factors through map
nodes; cached weighted totals/matches supply snapshots without scoring callback reruns.
Typed count/magnitude comparisons, numeric decimal equality, no-impact checks and
occurrence-aware justification matching include structured/custom-context errors.
Existing graph/search implementation unchanged. No out-of-line impl on shared types.

Focused76013 TERMINAL0:24 adapted source methods (84 scalar chains plus relative,
unique-pair and justification cases). Focused63643 TERMINAL0:4 original regressions
for rollback/detachment, callbacks, fractional scales/duplicates/zeros, minimum i64
magnitude, disabled pruning and foreign handles. Earlier draft94535 failed compile:
removed unsupported Debug derive and borrowed count pattern to avoid partial move.
No production upstream implementation read. Appended24cases11fixtures; existing
provenance prefixes retained. Final gate follows on frozen source. No benchmarks,
browser or release reruns. Goal remains active.

Read remaining SingleConstraintAssertionTest specs while frozen gate runs:
96..150 shadow cascade and missing referenced entities;721..877 three list-unassigned
cases (including no inverse shadow);1168..end five nullable/explicit inconsistency
cases. They remain pending. Source tests preserve supplied nullable invalid flags
and times unless explicit shadow-setting is requested; ordinary assertions must not
silently repair facts. Existing native ShadowSession/ListShadowSession offer explicit
updates but no full verifier fixture-preparation facade. A future integration must
keep supplied-vs-computed data semantics distinct. No production source was read.

Required gate68474 TERMINAL0. 3868 Rust tests and 185 doctests passed.
Rust jobs: {'RUN': 3, 'REUSE': 482}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2265, 'failed': 0, 'pending': 2308, 'excluded': 282, 'unverified': 0, 'percent': 46.653}.
Source before/after matched: 74b23db53fc789c5b185772b050af14a22bd4390ec868e8019501bf3002b12dd
Live freshness matched verification digest: 53357f60cfd71078a6974637d0359b2a87dcde6c7ffe41f9b4b9a5c553fd8af1.
Existing provenance prefixes unchanged; appended24cases and11fixtures.
Log work/constraint-assertion-final-check.log; evidence work/constraint-assertion-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Whole-score assertions

Starting HEAD32c2909 clean. Added separate score_assertion module: detached cached
whole-score snapshots, named contributions, typed exact/relative checks and boxed
structured errors with bounded Display. Graph snapshots cover ordinary constraints;
from_scores supports full director structural totals without resumming contributions.
No existing engine/type impl changed. Ten upstream methods adapted:6MultiConstraint,
3SingleConstraint list cases,1DuplicateConstraint. Read full Multi/Duplicate tests,
all four provider formulas and generated extended fixtures; nine list fixture files.
Java class/inverse representations use typed graph sources/native list membership;
no reflection fallback or automatic shadow repair claim. Appended10cases23fixtures.

Focused55550 terminal0:10source cases. Focused52064 terminal101:3native pass, decimal
regression incorrectly expected scale-sensitive score equality (Decimal itself is
scale-sensitive, DecimalScore equality is numeric). Corrected test to require numeric
scale equivalence; implementation unchanged. Focused81745 follows. Final gate pending.
No benchmarks/browser/release reruns. Goal remains active.
Focused81745 TERMINAL0: all4native regressions passed. List helper return uses a type
alias after focused source check; final gate covers this formatting/type-alias-only
change. Final source/provenance frozen for required gate.

Prepared next coverage direction while frozen gate runs: remaining Bi stream methods
include incremental entity/value joins, unchanged join-key predicate updates, and
assigned-list filtering/existence. Read AbstractBiConstraintStreamTest64..135,
267..371,534..687; complete expected match lists retained there. Existing
incremental_stream_cases covers Uni methods; deep_join_update_cases covers related
Bavet regressions but not these methods. No new credit or production edits made.
A coherent Bi/Tri adaptation batch can exercise these paths without changing the
shared solver, unless an actual behavior failure requires a fix.

Required gate91113 TERMINAL0. 3882 Rust tests and 186 doctests passed.
Rust jobs: {'RUN': 3, 'REUSE': 484}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2275, 'failed': 0, 'pending': 2298, 'excluded': 282, 'unverified': 0, 'percent': 46.859}.
Source before/after matched: e261f1f58f5f52984a507d0f4c8fa99ad9201423ca5ea31fb8722a865147e4b1
Live freshness matched verification digest: 6a02961c4bed8f92e8de9490d4658004c83f93d7a09caabd16349a9fcaa79bd5.
Existing provenance prefixes unchanged; appended10cases and23fixtures.
Log work/score-assertion-final-check.log; evidence work/score-assertion-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Incremental arity join/existence cases

Starting HEAD1a6c301 clean. Previous goal turn made verified committed progress.
Added arity_join_cases.rs (17source methods:10Bi/5Tri/2Quad) and
arity_existence_cases.rs (16source methods:8Tri/8Quad). No production changes.
Read all selected source methods and lavish fixture/helper definitions. Existing
native graph passes unchanged-key predicate flips, nullable/composite equality,
Cartesian/filtered joins, entity retractions and shared-source group removals.
Matches and individual scores are asserted at every original transition.

Source AssertableMatch compares justification facts without order; existence cases
normalize tagged IDs while retaining multiplicity. Unique pair ordering is explicit
native ID order. Two source Tri zero-join cases notify entity removal but remove it
from the entity-group list; native adaptation delivers the explicit retract event.
Read Bi64..135,267..687 (excluding unimplemented joiner ordering rejection),
Tri60..315,366..502,546..686; Quad59..142,160..298,322..457. Disabled empty methods
and reflection/configuration cases remain pending. No upstream production read.

Focused62211 TERMINAL0:17join cases. Focused41582 TERMINAL0:16existence cases.
Appended33cases15fixtures. Source and provenance frozen for one required final gate.
No benchmark/browser/release reruns. Goal remains active.

Required gate52546 TERMINAL0. 3915 Rust tests and 186 doctests passed.
Rust jobs: {'REUSE': 487, 'RUN': 2}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2308, 'failed': 0, 'pending': 2265, 'excluded': 282, 'unverified': 0, 'percent': 47.539}.
Source before/after matched: becd9496d38fbd517fe23a1cb5c99d3e9d60289f48b59fd63fdb0bf07d9a33e6
Live freshness matched verification digest: 7429f5321673c973ffe991c576fa114cc81f6baefe7801a0d89cf25d65a0251a.
Existing provenance prefixes unchanged; appended33cases and15fixtures.
Log work/arity-stream-final-check.log; evidence work/arity-stream-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Stream corruption evidence

Starting HEAD05a4bb0 clean; previous goal turn made verified committed progress.
Added StreamConsistencyReport/StreamNodeDifference typed evidence and detached
StreamScoreCorruption. Graph consistency_report reuses the existing journaled rebuild,
compares every original checked terminal/dataset and restores state on return/error/
panic. assert_consistent now returns boxed evidence in GraphError::CorruptedScore;
Error::source exposes it. Named constraints plus dataset node IDs retain cancelled
per-match/per-constraint differences even with equal totals. No intermediate-cache
or uncaptured terminal-payload comparison claim. Formatting bounded5nodes, structured
records complete. Existing five error-category assertions updated for payload variant.

Compile66361 terminal0. First focused67255 terminal101: adapted solver test used
nonexistent planning_source helper/result.best; corrected to typed source tuple and
SolverResult fields. Two immediate compile retries exposed score Option and assignment
(non-Option) expectations; corrected from compiler/type evidence. Focused21252
TERMINAL0:31tests across6targets, including3source solver cases and3newnative reports.
Existing cancelling-match, dataset, history-dependent projection/decimal, panic and
search accounting regressions pass. Added one report doctest after focused success.

Read DefaultSolverTest2737..2804 and nested calculator2906..end; basic testdomain
fixtures reused from prior full reads. Adapted initialized count2 and initialized/
uninitialized corruption with deterministic changing scores. Actual native match
names/differences replace Java registry match b/diagnostic labels. Appended3cases6fixtures.
Source/provenance frozen for final gate. Shared graph changes require affected broad
regression coverage; reuse remains enabled for unchanged inputs. No benchmarks/browser/
release reruns. Goal remains active.

While gate66018 runs, prepared (NOT installed, NOT credited, NOT yet compiled)
work/list_assigned_join_cases.rs:4Bi assigned-list join/existence cases, and
work/bi_existence_filter_cases.rs:4Bi indexed/unindexed filtered existence cases.
Read complete Bi136..266,746..877,899..1022,1061..1197; current native list source/
director/move APIs inspected. Source assertion semantics and assigned row callbacks
retained; native Composite list replacements replace Java before/after notifications.
Install/adapt provenance after this gate/commit so frozen verification inputs stay
unchanged. Work drafts formatted only. Source basic unassigned existence specs also
read; they remain pending. No additional solver implementation changes in preparation.
Draft check26869 TERMINAL0: all8prepared cases passed when compiled directly against
the current built native rlib. This is development evidence only; files remain in
work/, with no provenance/dashboard credit until installed and gated separately.
Added two basic unassigned-existence cases to the second work draft (complete source
specs already read), now6tests; direct compile/run terminal0 in chunk71d3b9. Prepared
work/add_assigned_stream_provenance.py for10cases12fixtures, NOT executed. To install
next batch: copy both work/*.rs drafts to same basenames under crates/solver/tests,
then run that provenance script, document and run the final selective gate. Original
4list cases remain covered by successful direct check26869; second draft6passed.

Required gate66018 TERMINAL0. 3921 Rust tests and 187 doctests passed.
Rust jobs: {'RUN': 434, 'REUSE': 57}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2311, 'failed': 0, 'pending': 2262, 'excluded': 282, 'unverified': 0, 'percent': 47.6}.
Source before/after matched: e8c1baaf3ad7588815c1b0095e6a62c75dceac8b35d30df1ec703672798d4edd
Live freshness matched verification digest: dc0901f348a044a789390822ac0294668035af6740ba7f9fd0717eec68d10dc9.
Existing provenance prefixes unchanged; appended3cases and6fixtures.
Log work/stream-consistency-final-check.log; evidence work/stream-consistency-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Assigned-value stream cases

Stream diagnostics committed as ae52478; gate66018 and evidence17816 terminal0.
Installed the two prepared, directly checked work drafts into tests; updated notice
comments only. Ran prepared provenance script:10cases12fixtures appended. Covers
four assigned-list join/existence transition cases, four filtered group-existence
cases and two basic unassigned-exclusion cases. No production changes. Prior draft
checks already established behavior; final selective gate follows on installed files.
No benchmarks/browser/release reruns. Goal remains active.

Required gate8456 TERMINAL0. 3931 Rust tests and 187 doctests passed.
Rust jobs: {'REUSE': 491, 'RUN': 2}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2321, 'failed': 0, 'pending': 2252, 'excluded': 282, 'unverified': 0, 'percent': 47.806}.
Source before/after matched: 1589cfea9b29398d2ea2f49262e3c7df6cd33c00443c5ccbcc9084d4cd9d0634
Live freshness matched verification digest: 9a6d850993c26ac15c52d1165971b954a993df42ce75885d3d6f0718b5b408e8.
Existing provenance prefixes unchanged; appended10cases and12fixtures.
Log work/assigned-stream-final-check.log; evidence work/assigned-stream-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Grouped existence and tuple flattening

Starting HEAD9c393dd clean. Previous goal turn completed evidence and committed the
assigned-value batch, so it made progress. Added stream_composition_cases.rs with
seven complete source methods: Bi/Tri/Quad ifExistsAfterGroupBy, Bi/Tri flatten,
Tri ifExists/ifNotExistsDoesNotIncludeUnassigned. Read complete selected source
methods and used previously inspected lavish fixtures/helper. Quad unassigned
methods are empty and remain pending. No upstream production read.
Native two/three/four distinct-count collectors feed existence; flatten retains
prefixes and duplicate occurrences. Exact match tuples, per-match and total scores
checked across every original transition. Additional native grouped assertions:
one of two witnesses removed, rollback of final removal, committed removal,
left update without witnesses, witness reactivation. No production changes.
Initial focused compile failed on mistyped exists_equal; corrected to the existing
existence_equal API. Focused chunk69d70e TERMINAL0: all7tests passed. Appended7cases
and8fixtures. Source frozen for one final selective gate. No benchmark/UI/release
reruns. Goal remains active.

Required gate87261 TERMINAL0. 3938 Rust tests and 187 doctests passed.
Rust jobs: {'REUSE': 493, 'RUN': 1}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2328, 'failed': 0, 'pending': 2245, 'excluded': 282, 'unverified': 0, 'percent': 47.951}.
Source before/after matched: 0492d93411db6abff84aab6adc369e9978359848b17c6e40bcdc5d982ef05d0b
Live freshness matched verification digest: 6a7e9f9d8f82eda75ba9a02ee9641ebee157276c1c1f73916bb37c338c784be3.
Existing provenance prefixes unchanged; appended7cases and8fixtures.
Log work/stream-composition-final-check.log; evidence work/stream-composition-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Read-only verifier consistency preparation

Starting HEAD51d926d clean. Previous turn committed7adapted stream cases with
verified evidence; progress. Added independent verification_consistency module:
explicit entity/field references, selected given IDs, inferred missing flags,
explicit overrides, detached results, complete transitive non-given diagnostics,
unknown-field/ID and duplicate-definition validation. Uses native DependencyGraph
with dynamic field edges; no existing core type/implementation changed. No shadow
supplier execution, mutation or automatic solution extraction. Explicit flags do
not suppress reference validation or remove topology edges.

Read full four testdomain shadow/dependency fixtures and six selected
SingleConstraintAssertionTest methods (full methods already read in preceding
turn). Original nullable/explicit flags and all180/120/0/1 score/count assertions
retained. Native minute offsets replace Java date origins. Source references are
reported as IDs/field names, preserving both indirect relationships rather than
Java object-value strings. Appended6cases4fixtures. Five native regressions cover
field-level false cycles, downstream propagation/overrides, detached snapshots,
complete/deduplicated/bounded diagnostics, malformed metadata and4000-nodechains.

Compile20752 TERMINAL0. Initial focused81454 terminal101: missing-reference error
included internal field links on omitted entities. Restricted diagnostics to
cross-entity references (internal dependencies still participate in inference).
Focused32143 TERMINAL0: all11tests passed. One subsequent test-only clone-to-slice
cleanup and formatting before final gate. No benchmarks/browser/release reruns.
Additional Uni inconsistency tests were read but NOT adapted; they include list
reset/lifecycle behavior and remain pending. Goal remains active.
First gate62724 TERMINAL101 at clippy: clone-to-slice replacement missed compact
macro spacing. Fixed actual expression with std::slice::from_ref; gate retry reuses
successful jobs where applicable. No behavior failure in this gate.
Retry73825 TERMINAL1 at fmt only: standalone rustfmt edition differed from workspace
format. Applied cargo fmt --all and restored canonical verification environment.

Required gate37895 TERMINAL0. 3949 Rust tests and 188 doctests passed.
Rust jobs: {'RUN': 3, 'REUSE': 493}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2334, 'failed': 0, 'pending': 2239, 'excluded': 282, 'unverified': 0, 'percent': 48.074}.
Source before/after matched: aad88a2a3a614b4630547adf5982c95e32668da6f239ba4092f59fb32f37be8e
Live freshness matched verification digest: 58b85fe0eaef7fb7a121e968fc25ba6483e96fa7ac16297eca90b3235d811b60.
Existing provenance prefixes unchanged; appended6cases and4fixtures.
Log work/verification-consistency-final-check.log; evidence work/verification-consistency-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Explicit list-shadow verifier and selection cases

Starting HEAD7e1ded5 clean. Previous turn added verifier-consistency production
behavior and committed after gate37895; progress. Added verification_list_shadow_cases
with5complete upstream methods: Single/Multi updateShadowVariablesListSingleSolution
and Uni forEachIncludingUnassigned_excludeInconsistentEntities,
forEach_excludeInconsistentEntities, forEachUnfiltered_includeUnassignedAndInconsistentEntities.
Read all selected methods and full multiplelistener Value/Solution/Entity/Provider,
TestdataObject and dependency SimpleSolution; dependency Value/Entity already read.
Preserved every original score/match/justification. Native list registry/session
explicitly prepares structural/cascade fields, retaining one unassigned value in
the single case. Native field metadata/consistency and stream filters handle all
list-cycle transitions. Two Uni methods' final duplicate e1 writes are preserved
as e1=[dependency],e2=[] (dependent unassigned). No source production read or native
production changes. Java setters/history/reflection not claimed. Appended5cases9fixtures.

Focused68396 TERMINAL101: empty initial list element type needed explicit generic.
Focused24767 TERMINAL101: helper closure's registry lifetime annotation overgeneral;
changed closure to capture this registry. Focused48508 TERMINAL0: all5tests pass.
Source frozen for final selective gate. No benchmark/browser/release reruns.
Goal remains active; remaining methods elsewhere still substantial.

Required gate29461 TERMINAL0. 3954 Rust tests and 188 doctests passed.
Rust jobs: {'REUSE': 496, 'RUN': 1}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2339, 'failed': 0, 'pending': 2234, 'excluded': 282, 'unverified': 0, 'percent': 48.177}.
Source before/after matched: d78ac6b78bb61b9d46eda4408787aedfa7b5a7a603fb65cc8c904f4c5e0bf37a
Live freshness matched verification digest: 3a181e0cf09e5c2031f7fdcc24fd5a7f8a8ae81ff999584033aacd431c05cef3.
Existing provenance prefixes unchanged; appended5cases and9fixtures.
Log work/verification-list-shadow-final-check.log; evidence work/verification-list-shadow-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Lazy indexed candidate groups

Starting HEAD0147146 clean. Previous turn committed5cases after gate29461; progress.
Inspected remaining inventory and several test-only specifications before choosing
missing lazy group composition (no native counterpart). No upstream production read.
Added independent lazy_concat module: LazyConcat with eager owned/borrowed groups,
deferred known lengths, OnceCell cached results, binary-searched cumulative indices,
unsigned bounds Result errors, overflow rejection, lazy/reversible exact-size
iteration, sparse seeded sampling through existing unique_indices. No payload Clone
requirement. Mismatched supplier lengths cache errors; panic leaves cache retryable;
zero-length suppliers never run. Single-threaded cache; recursive access unsupported.
Existing solver internals unchanged; no performance claim/benchmark refresh.

Adapted17complete ComposingListTest methods; negative-index case stays pending
(usize API). Full test source read. Original lengths/content/index/callback/error
message assertions retained; native Cell replaces Java mutable counter/boolean.
No fixture adaptation. Four native regressions cover sampling/caching/multiplicity,
borrowed non-Clone payloads, reverse iterator, invalid length, panic, bounds, overflow.
Focused55224 TERMINAL0 for17cases. Follow-up55030 TERMINAL101 compiling native test
because consuming calls array conflicted with captured borrows; use each_ref().map.
Focused13844 TERMINAL0 for4native regressions. Final source frozen for selective gate.
No benchmark/browser/release reruns. Goal remains active.
Gate4101 TERMINAL101 at clippy: samples return type redundantly specified Iterator
alongside its FusedIterator supertrait. Removed redundant bound; behavior unchanged.

Required gate39966 TERMINAL0. 3975 Rust tests and 189 doctests passed.
Rust jobs: {'RUN': 3, 'REUSE': 496}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2356, 'failed': 0, 'pending': 2217, 'excluded': 282, 'unverified': 0, 'percent': 48.527}.
Source before/after matched: a37a0ecb741de1296bf22045568b5b0a0b0f9a4e0882456ac454d61ce0721cb2
Live freshness matched verification digest: b0f4a31297e2c847b7d443c69d554988e51fd62559941406ee005c8d88c4826f.
Existing provenance prefixes unchanged; appended17cases and0fixtures.
Log work/lazy-concat-final-check.log; evidence work/lazy-concat-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Mutable indexed candidates and range sampling

Starting HEADf7d4e4d clean. Previous turn implemented LazyConcat and committed17cases
with verified gate39966; progress. Added independent candidate_index module:
CandidateIndex<K,I,T>, stable IDs/dense key buckets/position map, insert/remove/
replace/get/clear, borrowed CandidateBucket iteration/indexing/unique/repeated
sampling, multi-key deduplicated and ordered-range LazyConcat borrowed query views.
Added repeating_candidates free function for composed queries. Native payloads need
no Clone/Eq; queries prevent mutation by borrow. Removal uses swap and repairs moved
position, cleans empty bucket. No existing stream-node rewiring or performance claim.

Read full EqualIndexerTest, EqualsAndComparisonIndexerTest and AbstractIndexerTest
helper. Adapted15+10pendingmethods, preserving all source assertions including
finite exhaustion, predicates, first membership, full drains, seed0 repetition30/60
and flipped >= range drain with seeds0..9. Previously passed comparison cases remain
unchanged. Source handles/exceptions become stable IDs/typed errors/Iterator None;
no Java trace/storage identity claim. Appended25cases2fixture helper adaptations.

Focused4706 TERMINAL0:15equalitycases. Focused5832 TERMINAL0:3native regressions,
including2000mutations against BTreeMap reference. Focused80233 TERMINAL0:10rangecases.
Before final gate added fourth native regression for empty/lazy repeated query and
cached supplier errors; collapsed test else-if and used find instead of filter.next.
Source frozen for final selective gate. No benchmark/browser/release reruns.
Goal remains active.
Gate1450 TERMINAL101 at clippy: find(...).is_none() in empty-filter tests should use
!any(...). Updated both occurrences; no production/behavior change.

Required gate54867 TERMINAL0. 4004 Rust tests and 190 doctests passed.
Rust jobs: {'RUN': 4, 'REUSE': 498}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2381, 'failed': 0, 'pending': 2192, 'excluded': 282, 'unverified': 0, 'percent': 49.042}.
Source before/after matched: 3478c43dab06bbc49369be9982bd2818d19a23625991f1b7ec75b865fd3a2aed
Live freshness matched verification digest: ee99455865e3ebd6d377dc9bcd1ce760f5e00989734e14253be79b76dfa156ff.
Existing provenance prefixes unchanged; appended25cases and2fixtures.
Log work/candidate-index-final-check.log; evidence work/candidate-index-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Deduplicated membership candidates

Starting HEADcc23dc9 with membership module/cases already in progress. Previous
status turn inspected timing only; resumed concrete implementation here. Added
MembershipCandidateIndex with captured multi-key rows, inverted ID postings,
upsert/remove/get/clear, empty-row retention, tuple-key composite partitions,
and MembershipMatches borrowed iteration/unique/repeating sampling. Query eagerly
deduplicates references in ID order; no payload Clone or Java lazy allocation claim.
Independent public component requires explicit caller synchronization.

Read all three membership IndexerTest classes and AbstractIndexerTest helper;
appended25cases and1helper fixture. Retained all source assertions, native seed0/2
order difference, ten-seed exhaustion and thirty-draw repeating coverage. Native
root-derived seeds replace Java nextLong for twenty first-draw trials. Existing
source-case provenance unchanged. No upstream production implementation read.

Focused69089 TERMINAL101: test helper needed explicit index lifetime. Fixed;
focused fixed command TERMINAL0 with25cases. Regression28741 TERMINAL101: function
keyword typo in new test; fixed before final gate. Added four native regressions
covering2000mutation scan reference, null versus empty memberships/non-Clone payload,
input iterator panic preserving old row, and overlap-independent sample sequences.
Final source formatted with cargo fmt; final selective gate follows.
No benchmark/browser/release reruns. Goal remains active.

Required gate45874 TERMINAL0. 4033 Rust tests and 191 doctests passed.
Rust jobs: {'RUN': 3, 'REUSE': 501}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2406, 'failed': 0, 'pending': 2167, 'excluded': 282, 'unverified': 0, 'percent': 49.557}.
Source before/after matched: a798f1f59ba441cc9da379865b36c20516adf3b3d15b1d2885c6495585c8b2cc
Live freshness matched verification digest: cf1cdfd41e4b4a9400da935cf40db041cb9f5b29aea9bbf54de4f2cc512f9d70.
Existing provenance prefixes unchanged; appended25cases and1fixture.
Log work/membership-candidate-final-check.log; evidence work/membership-candidate-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Live neighborhood testing context

Starting HEAD104d005 clean; previous goal turn committed deduplicated membership
queries with successful gate45874: progress. Read NeighborhoodTesterTest and complete
TestdataSolution/TestdataEntity fixtures; no upstream production reads. Existing
native change enumeration and move guards already implement move behavior.
Added generic NeighborhoodTester state/provider context using shared Rc/RefCell,
fresh lazy iterators per assertion, independent within limits, inspect/update
callbacks, and typed provider/missing/unexpected errors. Presence checks stop early;
absence checks observe only bounded prefixes. Duplicate expected moves mean presence,
not multiplicity. Native default10000; empty requests skip provider. No iterator
cache, reflection or independent move execution implementation.

Five source cases/twofixtures appended. Three native regressions exercise exact
infinite-provider pull budgets, provider errors, empty requests, and real
ScoreDirector shared-state commits and temporary panic rollback of assignment/score.
Focused8268 TERMINAL101: borrowed external counter could not outlive arbitrary
provider iterator lifetime. Test now clones Rc counter into each iterator.
Focused47334 TERMINAL0:5adapted+3native tests. Final formatted source frozen for gate.
No benchmark/browser/release reruns. Goal remains active.

Required gate98260 TERMINAL0. 4041 Rust tests and 192 doctests passed.
Rust jobs: {'RUN': 3, 'REUSE': 503}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2411, 'failed': 0, 'pending': 2162, 'excluded': 282, 'unverified': 0, 'percent': 49.66}.
Source before/after matched: 58459517d911aac17dc15b898d3ee70cbe30cdd985eb747ae70a4123a76e38c5
Live freshness matched verification digest: b5de61e8247a390a83f4db88e555175eb678c3a15508592963b823a88138ce6f.
Existing provenance prefixes unchanged; appended5cases and2fixtures.
Log work/neighborhood-tester-final-check.log; evidence work/neighborhood-tester-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Captured collector sessions

Starting HEAD29f64e8 clean. Previous turn committed live neighborhood tester and
verified gate98260: progress. Read full NeighborhoodsCollectorsTest. Existing list,
finishing and pair collectors already provide core aggregation; new CollectorSession
adds owned captured contributions, session-scoped explicit handles, insert/insert_with,
remove/replace_with, contribution lookup, clear, len and empty finish. Monotonic u64
ordinals support standalone encounter order. Rc owner tokens prevent foreign-handle
aliasing; removed/cleared IDs are never reused. Delegate mutations stage isolated
clones before installing both aggregate and captured row. No V Clone/Eq requirement.
Mapper and delegate failures leave prior state intact under collector contract.
No graph-node changes or Java internal/layout equivalence claim.

Six source methods appended, no external fixture adaptations. Bi tests project
(integer,string) input to integer; native PairCollector and FinishingCollector
preserve every source output before/after each handle removal. Added three native
regressions for mutable source capture, duplicate slots, snapshot persistence,
foreign/stale handles, clear/reuse, non-Clone payloads, mapper/delegate panic and
rejected removal/replacement preservation. Focused6319 TERMINAL0 initial six cases;
focused88273 TERMINAL0 final six cases plus three regressions. Final state formatted
and frozen for selective gate. No benchmark/browser/release reruns; goal active.

Required gate70446 TERMINAL0. 4050 Rust tests and 193 doctests passed.
Rust jobs: {'RUN': 3, 'REUSE': 505}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2417, 'failed': 0, 'pending': 2156, 'excluded': 282, 'unverified': 0, 'percent': 49.784}.
Source before/after matched: 7bdbffb71593c7c6f73d707897400a64b0dee9ece70cb2d3eee744fcc2407e9a
Live freshness matched verification digest: 846f1b0bb3bba2b26a5b1799e74a1285edbfbe0ca6606c86af9f8732c46fbf19.
Existing provenance prefixes unchanged; appended6cases and0fixtures.
Log work/collector-session-final-check.log; evidence work/collector-session-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## K-opt edge orientation and full-suffix lifecycle brackets

Starting HEAD187dcaa clean; prior captured collector session batch was verified and
committed: progress. Read complete SelectorBasedKOptListMoveTest (including test
helpers) and TestdataListEntity/Solution/Value fixture sources. No upstream production
reads. Existing native k-opt tests had output-only fixtures and explicit successor
selected from expected output, so full method credit remained pending.

Added list_k_opt_edge_move: direction comes from first removed endpoint through
its first incident added edge, then rotate validated cycle to unpinned anchor.
Eight source single-entity cases now construct solely from original edges. Added
ListNotificationScope::{ChangedRange,MovableSuffix} and prepare_with_scope to native
ListLifecyclePlan. Default preserves minimal intervals; suffix policy widens only
changed lists in an atomic plan. Existing gather path, score directors and transaction
semantics unchanged. Eight source cases/threefixtures appended with callback checks;
earlier partial fixtures untouched. No broad Java orientation/partition claim.

Focused96495 TERMINAL0 initial eight cases. Regression56745 TERMINAL101: local
observer closure needed binding to outlive rollback guard; corrected. Focused99169
TERMINAL0 final eight edge-derived cases plus three notification regressions.
Added fourth original edge-order/error regression before final gate. Native coverage
includes multi-entity old/new lengths, exact undo ranges, prefix pins, default/no-op,
stale plan, callback panic, edge direction and invalid inputs. Shared list modules
changed, so final gate must run affected existing list regressions as selected.
No benchmark/browser/release reruns. Goal remains active.

Required gate37631 TERMINAL0. 4062 Rust tests and 193 doctests passed.
Rust jobs: {'RUN': 450, 'REUSE': 60}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2425, 'failed': 0, 'pending': 2148, 'excluded': 282, 'unverified': 0, 'percent': 49.949}.
Source before/after matched: 626ee4ccd9274699fbb90e14fc1b593be4bf6e54795ff4ca42beae50c2499405
Live freshness matched verification digest: 5f551e1e64cdf946031d708b1f3b77f07d8f2d1b4e6032fc40a2bc927cd5e871.
Existing provenance prefixes unchanged; appended8cases and3fixtures.
Log work/kopt-lifecycle-final-check.log; evidence work/kopt-lifecycle-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Relation-oriented candidate queries

Starting HEAD67b6355 clean. Previous goal turn committed edge-oriented k-opt and
full-suffix notification support with broad verified gate37631: progress. Read full
ComparisonIndexerTest; AbstractIndexerTest helper was read in preceding index batches.
Added independent comparison_candidates module/function and CandidateSide enum,
reusing existing JoinComparison reversal and CandidateIndex ordered bounds. Equal,
LT/LTE/GT/GTE queries borrow buckets with no payload Clone and no key arithmetic.
No existing implementation module changed; exports and two new test targets added.

Eleven observable methods appended with one helper fixture. Preserve original
30/100repeated draws, seeds0..9 drains/exhaustion and twenty native-root-derived
first draws. ExtremeKey preserves original comparator signs via Ordering. Source
reverse-array comment claims key20 matches >25, but its actual predicate/helper
assertions exclude20; adaptation preserves actual behavior. Java internal storage
and eligibility methods remain pending. Two original tests cover all five relations
and both stored sides against direct scans at i64 extremes before/after rekeying,
plus Option keys and borrowed non-Clone payloads. Focused37763 TERMINAL0:11+2cases.
Final source formatted/frozen for selective gate. No benchmark/browser/release
reruns. Goal remains active.

Required gate17334 TERMINAL0. 4075 Rust tests and 194 doctests passed.
Rust jobs: {'RUN': 3, 'REUSE': 509}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2436, 'failed': 0, 'pending': 2137, 'excluded': 282, 'unverified': 0, 'percent': 50.175}.
Source before/after matched: d9f15c4058eafa2a5a5db86642a682f2b743a68561199f20e9ad94fa6a9d4d31
Live freshness matched verification digest: 4945a4a3271752879aa6fa76d73f96a9e5568aee32c7cb2ae73f3a9c6e2ea67d.
Existing provenance prefixes unchanged; appended11cases and1fixture.
Log work/comparison-candidate-final-check.log; evidence work/comparison-candidate-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

## Phase-level retained best delivery

Starting HEADb6c10eb clean; previous relation-query batch committed after gate17334:
progress. Inspected DefaultSolverTest solve/uninitialized/partial methods and runtime
problem-change cases. Runtime changes remain pending; no changes there this batch.
Read relevant PlannerTestUtils helpers, BestScoreChangedEvent, NoChangeCustomPhaseCommand,
DummySimpleScoreEasyScoreCalculator and Testdata fixtures (Entity/Solution read in
preceding batches; TestdataValue read here). No production upstream reads.

Added independent PhaseBestBuffer/BufferedSolverUpdate with explicit initial-score
baseline, per-origin latest pending snapshot, observe returning preceding pending
snapshot, explicit/idempotent flush and has_pending. Raw solver observation unchanged.
Accepted updates clone before consuming old pending state; ordinary clone failure
preserves it. Needs fresh instance per solve/revision; native callback timing is
explicitly different from Java. Three source cases/sevenfixtures appended, exercised
on both dependency and graph backends. Full construction preserves exact event score,
phase and initialized flag; no-op custom preserves scored partial result and noevents;
two-step construction preserves partial result and at least one event.

Focused31304 TERMINAL101: observer closure needed explicit borrowed SolverUpdate
argument for higher-ranked lifetime. Fixed. Focused30532 TERMINAL0 for3sourcecases
on both backends; regression40286 TERMINAL0 for4original buffer cases. Final formatted
state frozen for selective gate. No benchmark/browser/release reruns; goal active.

Required gate26710 TERMINAL0. 4082 Rust tests and 195 doctests passed.
Rust jobs: {'RUN': 3, 'REUSE': 511}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2439, 'failed': 0, 'pending': 2134, 'excluded': 282, 'unverified': 0, 'percent': 50.237}.
Source before/after matched: 816f9be70993957f089c2da7de530f05f024683cfe9277654479c3ae7206e6ea
Live freshness matched verification digest: 5c31c695704bf4d01def7b9c2edb87a1abf685f106291a5cd12e807cd7c1bb16.
Existing provenance prefixes unchanged; appended3cases and7fixtures.
Log work/phase-best-final-check.log; evidence work/phase-best-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.

Gate9053 TERMINAL101: clippy rejected test-helper return type complexity.
Added PhaseDeliveryResult alias; formatted and gate26710 passed as recorded above.
User timing answer: last five completed batches 06:46:48–07:24:39 Vienna,
37m51s elapsed. Recorded build/lint498s and testexecution74s across focused/final
logs: at least25% verification; remaining75% includes implementation/specs/tests/docs
and unmeasured overhead. K-opt accounts for417s. Not a precise coding-time split.

## Identity-based reconciliation for runtime problem changes

Starting HEADcfd78af clean; previous batch committed verified phase best delivery:
progress. Read current live solver, model resume APIs and original live regression.
Read DefaultSolverTest.solveWithProblemChange and relevant next method; second
method director identity/close assertions remain pending. Read TestdataSolution,
Entity, Value, PlannerTestUtils configuration helper and dummy zero scorer tests.
No upstream production reads. Added AssignmentSnapshot/reconcile_problem independent
module with explicit ID validation, canonical new-domain values, current pins,
new/missing/removed-value fallbacks and preserved optionality/pillar model metadata.
No previous score copied. Live generic manager and existing model code unchanged.

Three original reconciliation cases cover ID reorder/add/remove, invalid IDs,
domain removal, optional/null/new pins, changed facts behind stable value equality,
no previous snapshot and empty models. One source method/five fixtures appended;
source addition case plus original three-revision integration run both scorers and
verify immutable delivered snapshots. Focused57597 TERMINAL0 for3 native cases.
Focused80878 TERMINAL0 for adapted live change and original daemon reconciliation
on both scoring backends. Source formatted/frozen for final selective gate.
No benchmark/browser/release reruns; no performance claim. Goal remains active.

Required gate11679 TERMINAL0. 4087 Rust tests and 196 doctests passed.
Rust jobs: {'RUN': 3, 'REUSE': 513}; Python jobs: {'REUSE': 46}.
Dashboard: {'total': 4855, 'passed': 2440, 'failed': 0, 'pending': 2133, 'excluded': 282, 'unverified': 0, 'percent': 50.257}.
Source before/after matched: 84b92377aa3914ce2de814c1e2ca71bd6978374f58823f4d442420746c5b7abf
Live freshness matched verification digest: 5794a530921a05e39b0a5d85994cce0c5555fbc5ac84236e244a22e911e89b0b.
Existing provenance prefixes unchanged; appended1case and5fixtures.
Log work/problem-reconciliation-final-check.log; evidence work/problem-reconciliation-evidence.json.
No benchmark/browser/release reruns. All sessions terminal; goal remains active.
