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
