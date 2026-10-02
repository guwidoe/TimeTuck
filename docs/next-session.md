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
- 153 fully adapted source test methods (74 score + 10 basic move + 22 list move + 27 provider + 11 acceptor + 4 construction + 5 collector), recorded in
  `compat/provenance.json`.
- Finite domains, initial values, pinning, optional values and reversible
  change/swap/composite transactions. Read `docs/planning-contract.md`.
- Iterative exact search with explicit evaluation budgets and truthful
  exhaustion/optimality reporting. Original `tiny_schedule` example.
- Native ordered-list model with unique ownership, optional unassignment, prefix/full
  pins and derived owner/index/previous/next views. Atomic assign/change/swap/span
  unassign/composite transactions, plus iterative bounded exact list enumeration.
  Read `docs/list-planning-contract.md`. `tiny_routes` proves `0hard/-12soft`
  after 5,040 complete states.
- 256 original Java/Rust/Python list-move cases (907 operations) check candidates,
  undo, commit and all four built-in shadow relationships; seed 20261003.
- Scoped incremental basic/list score networks and atomic scored-move transactions.
  List scopes cover entity lists and value identities/shadows, with conservative
  neighbor invalidation and deferred cache updates.
- Lazy change/swap neighborhoods and bounded deterministic local search,
  strict-improvement, hill-climbing, late acceptance, variable tabu or seeded simulated annealing; explicit
  evaluation/step limits, first/best accepted candidate selection and original or
  seeded shuffled-variable ordering.
  Read `docs/incremental-search-contract.md` before modifying this layer.
- 148 original integration regression methods, two original unit tests, and four public API documentation tests.
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
  heuristic cases are independently checked. No incremental-scoring or
  large-instance performance claim.
- Local visual dashboard: `python3 scripts/dashboard_server.py`, port 8765.
  Server was restarted after the reporting-code update and may still be running.
  Reuse it; restart after changing imported fingerprint/reporting Python code.
- Coverage denominator: 4,855 statically discovered annotated Java methods, not
  expanded runtime JUnit invocations. 153 passed; 282 JVM integration exclusions.
  Everything else is pending. `docs/dashboard.md` explains caveats and commands.
- Pinned upstream remains unmodified at 8b815bc6b5f3eb81feedc94dea0885a93b267762.

Read `docs/score-contract.md` before changing scores. The Java oracle showed that
score arithmetic CLEARS structural values, constructors retain positive values,
and full strings suppress positive structural prefixes. Do not reintroduce older
initialization-score assumptions. No production implementation has been ported.

## Next substantive work

0. Extend typed snapshot constraint streams to incremental tuple propagation,
   retractable grouped state and public-API Java stream comparison. Current joins,
   filters, existence checks, grouping and scoring rebuild per affected callback;
   do not credit upstream incremental graph tests for snapshot-only behavior. Read
   `docs/constraint-stream-contract.md`.

1. Extend native exact-search/model coverage to more applicable upstream tests.
   Java trace/descriptor-specific assertions must not be credited by looser tests.
2. Extend list-solving evidence to optional lists and mixed models. Entity-specific
   ranges now apply across moves, construction, exact and local search. Required-list
   construction/hill-climbing now have 128 Java/Rust checks
   and four full-scoring routing benchmarks; exact optima are independently
   enumerated for small cases. Native greedy list construction, lazy neighborhoods
   and bounded heuristic search now exist.
   Native exact list enumeration and four derived shadows now exist; custom
   shadows, cascading/declarative listeners and mixed basic/list solving remain pending. Optional basic variables and per-variable ranges have 64 additional
   exact differential cases.
3. Extend construction, tabu (fading and entity grouping) and extend annealing with
   deterministic seeds, limits, quality checks and applicable
   upstream acceptance tests. Keep exact Java trace tests pending when not preserved.
4. Extend matched Java heuristic evidence to incremental scoring, more workloads
   and randomized search. Current heuristic runs are deterministic and fully
   recompute scores. Scoped graph-example callbacks are not runtime benchmarks.
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

Latest verification: `scripts/check.sh` passed (301 Rust integration methods,
two unit tests, four doctests, 31 Python regressions). Incremental-scoring, neighborhood, acceptance and local-search regressions
also passed in release mode; the complete all-target and documentation suites passed
after the list-range increment. Desktop 1440px and mobile 390px browser QA passed,
including move filtering and focus preservation on refresh. Current saved Java
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
