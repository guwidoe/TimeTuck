# Compatibility testing

## Target

Initial reference: Timefold Solver Community 2.7.0, exact commit recorded in
`compat/upstream.json` and the Git submodule. The target is planning/optimization
behavior implemented in native Rust. Java API, binary compatibility, reflection,
framework integration, and all Enterprise functionality are outside the initial scope.

Two adapted termination methods additionally preserve all step-count and
unimproved-step-count threshold/progress assertions. Native search uses these
counters across basic, list and mixed models with both scoring backends. Optional
score-target and feasibility stops have original integration checks. An additional
219 original deterministic basic-search cases now compare final solutions and
scores with Community and independent Python search. Both native scoring backends
also match Python step/acceptance/stagnation counts and termination reasons; Java
internal counters and traces are not compared. These cases earn no method credit;
nine integer score-target/feasibility methods now have complete adaptations.
The two decimal gradient methods now also have complete adaptations. Native progress objects replace
mock initialized phase/solver scopes and preserve every score, weight, repeated
observation and floating comparison; no Java lifecycle equivalence is claimed.

Two elapsed-time methods additionally preserve every millisecond threshold and
progress assertion. Cooperative native phase timing is exercised with controlled
clocks across all models/backends. This does not establish wall-clock trace parity,
Java whole-solve time scope, preemption or unimproved-time compatibility. Native
unimproved-time stopping additionally has six original controlled-clock regressions
covering all model/backend paths; upstream lifecycle methods remain pending.

## Why the existing suite needs an adapter

The upstream suite is Java/JUnit and exercises Java objects, internal types,
reflection, and JVM integrations. Running it unchanged tests Timefold itself,
not this Rust implementation. Its cases must be classified and adapted.

1. Inventory upstream test-source paths with `scripts/upstream_inventory.py`.
2. Select a small score-semantics test family and write an observable specification.
3. Adapt those cases into Rust tests or neutral fixtures, preserving attribution
   and recording provenance. Do not copy production implementations.
4. For differential tests, build a separate Java adapter using the pinned Community
   artifact and public APIs. Export versioned inputs/expected results as JSON.
5. Feed the same cases into a Rust adapter. Compare deterministic results exactly
   where the agreed contract permits it. The Java runtime must stay out of the
   Rust library's production dependency graph.
6. Expand to incremental scoring, move application/undo, small planning problems,
   then heuristic solving. Check incremental scores against full recomputation.

Thirty-four list-move methods additionally preserve ordered assignment, relocation,
swap, sublist relocation/reversal/exchange and span-unassignment states/undo, plus three
sublist touched-entity membership assertions. Native derived owner/index/neighbor
views and exact list enumeration are described in `docs/list-planning-contract.md`.
Thirty-one list-provider methods additionally preserve move membership and
exclusion assertions, including per-entity ranges. Native greedy list construction and bounded local search
are verified separately with original regressions. Java move metadata, rebase,
custom listeners, optional-list solving and matched mixed Java local-search
comparisons remain pending.
Original generated list-move and required-list-solving comparisons are separate
from coverage; the latter check complete valid solutions, final quality and
independently enumerated small-case optima.

The first three integer score families have 47 adapted tests; BendableScore adds
27, using native compile-time dimensions in place of Java score definitions; basic change/swap
moves add ten adapted cases; provider behavior and hill-climbing acceptance add
nine more. Four late-acceptance methods validate history behavior and hard-score
resets. Four construction-forager methods preserve early-pick decisions. Three
entity-tabu methods preserve 96 assertions for expiry and aspiration, using native
variable IDs as stable entity keys. Two annealing methods preserve all 12 scalar
acceptance decisions and negative-temperature rejection. Native multi-level
annealing energy and seeded traces are not covered by those upstream cases. A
fifth late-acceptance method checks BendableScore history resets with nine
assertions. See
`docs/incremental-search-contract.md` for native incremental scoring
and local-search evidence. Original finite-domain and exact-search regression
tests are kept separate from upstream coverage. See `docs/planning-contract.md`. Separate Java
and Rust development adapters compare generated operations through a tab-delimited
line protocol; versioned JSON records the inputs and results. See
`docs/score-contract.md` for the current contract. The unchanged upstream JUnit
suite has not been run against Rust, and whole-solver compatibility is not claimed.

Five integer collector methods additionally preserve every count, distinct-count,
sum, minimum and maximum assertion across duplicate insertions and retractions.
Seven additional stream methods preserve all initial/update/removal/reinsertion
scores and match sets through native persistent operators. They cover filtering,
Cartesian/equality joins and grouping. Two further methods preserve named
mirrored joins and two-key equality self-joins through automatic graph routing.
Native graph transactions and basic-variable move binding are verified by
separate original regressions. Descriptors and custom justifications remain pending.
Native stream construction/local search reuse the
existing algorithms and have separate full-scoring cross-checks; these original
regressions grant no additional upstream method coverage. See
`docs/constraint-stream-contract.md` for this distinction.
Original public-API Community stream comparisons additionally check 128 cases
and 2,899 score observations, with independent Python validation of native match
identities and group sums. Java score updates expose no Community match-analysis
API. Four matched incremental-stream search benchmarks and 64 small quality cases
are recorded separately; they grant no additional upstream method coverage.

Thirteen additional existence methods preserve initial/update scores and exact
left matches for indexed and predicate-only matching, self exclusion, assigned
filtering and existence after grouping. Native transactional existence relations
replace notification/descriptor plumbing; unsupported-class exceptions remain
pending. See the stream contract and provenance ledger.

Twenty mapping/distinct methods additionally preserve all initial/removal scores
and exact match multiplicities across uni/bi/tri/quad streams. Stable tagged IDs
and native tuple/set equality replace Java object references. A persistent
distinct node integrates with typed graphs; representative and panic/rollback
behavior have separate original regressions. See the stream contract and ledger.

Sixteen global aggregation methods across uni/bi/tri/quad streams additionally
preserve all initial/removal scores and collector result tuples for one through
four collectors. Native composed collectors retain count, distinct count, min/max
and exact entity-ID sets. Empty global groups emit no row, as separately checked
through the Community public API; this original boundary probe grants no additional
upstream method credit.

Four list-value matching methods preserve every score and exact match assertion
for assigned-only and including-unassigned streams, in required and optional
list models. Native list stream binding derives owned shadow views and explicitly
filters assigned values; descriptor-specific noInverseVar remains pending.
Original regressions independently check moves, full-score search equivalence and
rollback. Existing Java list-move comparisons now observe Rust's cached stream
views. No list-stream solver timing comparison is claimed.

Twenty-five additional collector methods preserve all map/set, sorted-map,
merged-sum, duplicate and replacement assertions across four tuple arities.
Native captured values and stable IDs replace Java retractors and update slots.
MapCollector composes existing reversible collectors; integer summaries expose
sum results for the small Integer inputs in these tests. Arbitrary merge-function
and Java int-overflow behavior are not claimed. Original randomized graph and
search checks remain separate from upstream credit.

Twelve additional methods preserve exact list insertion, duplicate removal and
list/ArrayList replacement results across four arities. Native stable occurrence
IDs reproduce the tested order; the general Rust order is ascending ID, independent
of event delivery. No general Java encounter-order or custom collection-factory
API parity is claimed. Full-rebuild, snapshot and order-sensitive search regressions
are original checks and grant no further upstream credit.

Eight additional average methods preserve empty, insertion, duplicate, replacement
and removal results across four tuple arities. Native nullable floating results
use Option<f64>. Separate original public-API comparisons check exact result bits
and operation-specific overflow behavior; they grant no extra upstream credit.
Decimal score families are described in the subsequent increments below.

Sixteen duration sum/average methods preserve all exact nanosecond, empty,
duplicate, replacement and removal assertions across four tuple arities.
SignedDuration replaces java.time.Duration; no general date/time or Java object
identity API parity is claimed. Original public-API checks distinguish update
overflow from average-finisher overflow, including narrowing the total to i64
nanoseconds before division. See the stream contract for boundary behavior.

Thirty-two arbitrary-precision sum/average methods preserve all exact integer
values, decimal coefficients/scales, duplicates, replacements and empty results
across four arities. Native BigInt and Decimal replace Java numeric objects;
number storage/arithmetic uses num-bigint. Decimal averages round half-even at
the historical sum scale. Original comparisons validate large inputs and precision
history separately. Decimal Ord breaks numeric ties by scale to remain consistent
with scale-sensitive Eq; numeric_cmp omits that tie-break. No general BigDecimal
API or arbitrary-precision score-family compatibility is claimed. See the stream
contract for mixed-scale rebuild limitations.

Native mixed basic/list planning now supports atomic component edits and exact
optimization of one coupled objective over the full product space. Eight original
regressions and a joint depot/route example provide native evidence only; no new
upstream method credit, mixed Java solver comparison, or mixed timing claim follows.
Native scoped incremental mixed scoring additionally passes six original
regressions, including 1,200 joint edits against an independent direct formula.
Mixed local search now has seven original integration regressions and one internal
rollback regression, with optional joint moves and shared native acceptance policies.
Mixed greedy construction adds seven original regressions for component ordering,
shared budgets, preservation of initial values/pins and local-search handoff.
Mixed transactional stream binding adds seven original integration regressions and
one internal rollback test. Shared construction/search engines agree with scoped
scoring across tested policies and budgets. These native checks grant no upstream
method credit. Separate original public-API comparisons now check 128 mixed cases,
2,378 states and 3,611 basic/list operations against Java and independent Python
scores/assignments/shadows. Java phase-trace and mixed Java local-search comparisons remain pending. Separate
original mixed BRUTE_FORCE quality comparisons are described below.
See `docs/mixed-planning-contract.md`.

Fifteen additional flatten/flattenLast methods preserve every initial/removal score
and exact match multiset across four tuple arities. Native positional occurrence
IDs retain duplicates; explicit distinct collapses equal tuples. Generic expansion
closures preserve the source tuple prefixes. Uni/bi flattenLastWithDuplicates
remain pending; no Java equal-object representative choice is claimed. Six
original regressions cover updates, rollback, composition and search separately.

Forty-four concatenation methods preserve all score/match assertions across every
pairing of one- through four-element tuples, duplicate/distinct variants and
concatenated group counts. Explicit Option padding replaces nulls; branch-tagged
IDs retain independent occurrences. Compound group keys retain exact value-code
strings. Original shared-source, rollback and search regressions provide separate
native evidence. Node-sharing internals and general Java padding API parity remain
outside this evidence.

Four complement methods preserve both initial/update scores and exact retained/
padded tuple matches across four arities, including index extraction and custom
numeric padding. Explicit universes and stable identity keys replace class-based
fact discovery. Six expandTo methods additionally preserve all appended identities,
sums, exact code strings and removal assertions through native map closures; fixture
random long properties use documented deterministic values in the original range.
Original complement rollback/search regressions remain separate from method credit.

Twenty-seven comparison methods preserve five scalar truth tables, twenty mapped
numeric bi/tri/quad/penta joiner cases, and two index-orientation match/size cases.
Native persistent ordered joins replace Java internal joiner/index classes. Checked
numeric projections cover only the small source inputs; no general longValue,
null-ordering or internal index-selection parity is claimed. Original range,
transaction and search regressions grant no additional upstream method credit.

Ten equality-plus-comparison index methods preserve every empty, membership,
cardinality, ordinary iterator and boundary assertion through native partitioned
joins. Multiple equality components may use tuple keys; one ordered relation is
indexed inside each equality partition. Random neighborhood iterator and repeated
removal exception methods remain pending. Native failure/rollback/search evidence
is separate from these source-method credits.

Five overlap-join methods preserve all 65 literal endpoint assertions, including
containment, equal endpoints and nonmatching touches, through native interval
indexes. Different BigInt/Decimal projections and higher-arity ignored arguments
are retained. Independent randomized index invariants and transaction regressions
provide additional evidence without upstream credit.

Original public-API ordered/overlap join comparisons additionally check 153 cases /
10,155 states across five relations and zero/one/two equality keys, plus overlap
joins with and without equality partitions. Independent Python
pairs verify both runtime scores and exact native match IDs/payloads through edits
and undo. These are separate from upstream method coverage and timing; Java
recalculates each score via Community SolutionManager.update.

Four binary-stream existence methods additionally preserve all zero/one-equality
initial/removal scores and match assertions. Native pure equality uses right-key
counts and linear retained state; complement composes this specialization. Generic
filtered existence remains available. Original randomized, fault-injection and
dense-bucket regressions and public-API Java comparisons are separate evidence.

## Case records and coverage

Every adapted/copied case belongs in `compat/provenance.json`. Include a stable ID,
source commit/path/test method, local path, license, preserved notice paths, and a
plain description of changes. Record pending and excluded cases with reasons.
Implemented cases live in the provenance ledger. Pending methods and explicit
exclusions are listed in the generated dashboard report with reasons. Passing
a selected family never implies that the full upstream suite passes.

The inventory lists all tracked Java files under `src/test/java`; that includes
fixtures, base classes, and utilities, not only executable tests. Do not use its
file count as a count of tests or coverage denominator.

For heuristic search, compare feasibility, independently recomputed scores, and
agreed quality thresholds. Equal random seeds across JVM and Rust do not guarantee
identical PRNG streams or search traces. Record the seed, algorithm/configuration,
termination limits, reference revision, and environment with every result.


## Original mixed-search quality evidence

64 coupled depot/route cases additionally compare Community BRUTE_FORCE results,
native joint exact search and native scoped/stream local search against independent
Python optima over 32,116 assignments. All returned solutions and scores are valid;
optimality counts differ: Java 39/64, native exact 64/64, both native local backends
63/64. The dashboard reports this quality gap explicitly; 64 valid cases do not
mean 64 equal-score results. These original cases add no upstream coverage. See
`docs/mixed-planning-contract.md` for configurations, starts and limitations.


Seven SubListChangeMoveTest methods additionally preserve all ordered-list,
rollback and touched-entity membership assertions through native span moves. The
provenance ledger records each adaptation and notices. Five original native
regressions and 128 additional Community span-move cases verify neighborhoods,
score-cache updates, pins/ranges and candidate/undo/commit shadows separately.
Java span-search configuration, other move metadata, rebase and listener-event
parity remain pending. Supporting fixtures are separately attributed; these
checks do not add upstream method credit.


Five SubListSwapMoveTest methods preserve all ordered-state, undo and entity
membership assertions using native pre-move spans. Original tests additionally
exercise unequal lengths, reversed exchanges, neighborhood completeness, cached
scores and list/joint-mixed search. Another 128 original Community public API
comparisons cover candidate/undo/commit and built-in shadows without extra
method credit. Overlap/empty-span errors are a native contract; no Java invalid
move or internal listener protocol equivalence is claimed.


Twelve ListTailSwapMoveProviderTest/TwoOptListMoveProviderTest methods additionally
preserve all asserted membership/exclusion, nonemptiness, move-property and
ordered-result checks. Explicit native flags replace Java default constructors;
finite iterator sampling retains the source caps where present. No assertion of
random-stream ordering, listener protocol or Java two-opt solving equivalence is
made. Native two-opt integrates both list/mixed scoring backends and joint moves;
original successor-set and cache/search regressions remain outside source credit.


Original required-list solving evidence now includes 64 two-opt cases alongside
64 construction and 64 relocation cases. Identical complete inputs, pins/ranges,
step limits and full-scoring objectives are checked; Java's seeded preview
provider and native finite enumeration have different candidate sampling. Final
scores, optima and timings are reported separately, including disagreements.
This adds no adapted-method coverage and no equal-trace/equal-work claim.


Seeded bounded candidate-window ordering is a native opt-in search feature, with
original conservation/laziness and all-backend search regressions. It changes no
adapted-method count and makes no Java PRNG/trace claim. The two-opt comparison
now explicitly uses it with seed 20261006/window1024; Java keeps its seeded public
provider. Numeric equality of seeds does not imply matching random sequences.


Cooperative external cancellation is now an original native local-search feature
across all six model/backend paths. Six regression methods verify cross-thread
requests, scored-prefix retention, validation and fallback. This does not adapt
the complete Java plumbing/lifecycle tests, which also involve daemon execution,
problem-change queues and listeners. Those source methods remain pending and the
adapted-method count is unchanged.


The shared native cancellation token now also controls all six construction paths
and all three exact-search model types. Construction retains partial scored
progress; exact search distinguishes an interrupted prefix from fully exhausted
enumeration. Eight original regression methods and one API doctest establish
these native contracts. The complete upstream Java solver lifecycle cases remain
pending; cancellation support alone does not satisfy their other assertions.


Five additional source methods fully adapt MoveCountTerminationTest and
ScoreCalculationCountTerminationTest: four preserve all 48 threshold/progress
assertions and one preserves negative-count rejection. Native CountLimit replaces
mocked Java scope/director counters. Construction and local search now separately
limit candidate evaluations, excluding initial scoring; accepted moves and callback
counts remain separate. Original integration/clock tests verify all native paths,
without claiming Java move traces, identical counters or whole-solver lifecycle.


Native multi-phase entry points now compose construction and local-search phases
under shared count budgets, cancellation and score/feasibility goals. Eleven
original integration methods, one API doctest and the mixed routing example verify
handoffs, retained results and accounting across all model/backend combinations.
These do not adapt complete upstream solver lifecycle methods. Whole-solver Java
compatibility, daemon/problem-change behavior and shared stagnation
limits remain pending; see `docs/solver-contract.md`.


Native construction and multi-phase solving now have cooperative elapsed-time
controls, independently checked with controlled clocks and real zero-time calls.
The shared timer includes graph preparation and spans construction and improvement
phases; it does not establish Java timer/lifecycle equivalence or hard preemption.
All fourteen new regressions are original and add no upstream method coverage.
See `solver-contract.md` and `incremental-search-contract.md` for scope and precedence.


Eight conditional-collector methods now preserve all empty, insertion, duplicate,
retraction and replacement assertions across uni/bi/tri/quad inputs. Native captured
Option values and stable IDs replace Java predicate/retractor/update plumbing,
using the original min/max projections and zero-valued tuple suffixes. Six original
regressions and 34 original Community/Rust/Python stream comparisons separately
check group retention, changes, failures, rollback and solver integration; they add
no method credit. See `constraint-stream-contract.md` for scope and limitations.

Collection membership adds eight complete deterministic indexer-method adaptations.
The native API deduplicates pairs and treats Option::None as an ordinary explicit
key. Public Community agreement is limited to the 54-case distinct/non-null-member
matrix; eighteen separate boundary probes retain eight observed Java/Rust
mismatches (duplicate-member multiplicity and unpartitioned null matching).
These probes earn no method credit and are displayed separately on the dashboard.
See the collection membership section of `docs/constraint-stream-contract.md` for
exact inputs, counts and native-only regression scope.

Native basic/list/mixed exact search now has optional cooperative elapsed-time
limits. Six controlled-clock unit methods, two public integration methods and a
doctest check candidate-prefix retention, competing controls, preparation costs
and actual exhaustion. These original checks add no upstream-method coverage and
do not establish Java timed phase traces or hard preemption. See the exact-search
elapsed-time section of `planning-contract.md` for scope and precedence.

Native score storage now supports Clone-only owned values throughout scoring,
construction, local/exact search and orchestration. Six original integration
regressions compare a heap-owned integer score with the established integer family,
including clone-failure rollback, and one doctest demonstrates the contract. This
ownership groundwork initially retained i64 ordinary levels and added no decimal
method credit. The subsequent numeric-level and fixed-family changes are below.
See `score-contract.md`.


Native score levels now support i64 and Decimal through ScoreLevel. Original
rational arithmetic checks and 2,160 decimal solver configurations exercise this
numeric contract, including estimates at extreme scales. Exact score predicates
remain separate. That numeric-level groundwork added no BigDecimal family or decimal-gradient
method credit; the subsequent adaptations are described below.


Sixty decimal score methods now have complete native adaptations across Simple,
HardSoft and HardMediumSoft. Literal multiply/divide factors map to exact Decimal
inputs, tested integral powers to u32, and raw constructors to from_parts. All
assertions are retained, with numeric Rust equality/hash checks and native ZERO.
Both decimal gradient methods now have
complete adaptations preserving the original zero tolerances and split weights.
Original generated comparisons separately check coefficients/scales, text, ordering,
feasibility and arithmetic against Community public APIs and independent Python
Fraction calculations. See score-contract.md for input-scope and rounding limits.


All 26 BendableBigDecimalScoreTest methods now have complete native adaptations.
Const-generic hard/soft dimensions replace Java score definitions and parsing checks
those dimensions. The independent generator adds 4,780 cases across five shapes
(seed 20261214), including no hard levels, no soft levels and a completely empty
shape. Combined with the unchanged fixed-family corpus: 8,170 comparisons and
225 expected errors. Source-method credit remains separate from these original
checks. Native factors/powers retain the fixed-family input limits; no decimal
performance or general Java floating-input parity claim. See score-contract.md.


Native solver orchestration now supports a shared unimproved-time limit across all
six basic/list/mixed entry points and both backends. It excludes construction and
first search initialization, carries through later phases and resets only on a
strict retained-best improvement before expiry. Original deterministic clock and
public-entry-point regressions establish this native contract. No source-method
credit or Java timed lifecycle/trace parity is added; the six upstream
UnimprovedTimeMillisSpentTerminationTest methods remain pending. See solver-contract.md.


Native shared unimproved-step limits now carry completed-step progress across local
search phases in all six solver entry points. Original plateau, fallback/recovery,
phase-boundary and precedence regressions verify this behavior. No new source
methods were adapted: the existing UnimprovedStepCountTerminationTest adaptation
still covers only its recorded phase assertions. See solver-contract.md.


Native ordered SolverPlan/MixedSolverPlan now mix local and exact phases across all
six model/backend entry points. Exact candidates share solve budgets, cancellation,
time/stagnation and goals; only full enumeration with a complete scored result can
report optimality. Scoped full scoring and transactional stream rebinding have
original regression coverage, including partial construction and owned decimals.
These original orchestration tests earn no upstream method credit. They do not
establish Java phase/trace equivalence or benchmark the new exact orchestration;
existing timed workloads retain their standalone algorithms and integer scores.


Step-counting hill climbing adds four full adapted methods from the pinned
StepCountingHillClimbingAcceptorTest (109 acceptance assertions across three
step-counting sequences, plus zero-size rejection). Its negative-size method remains
pending because native periods are u64. All five modes integrate with basic/list/
mixed local search in scoped and stream scoring. Six hundred additional original
public-Java/native/Python cases verify deterministic final assignments and scores;
native counters/reasons are checked separately. These add no further source-method
credit or Java internal trace claim. See incremental-search-contract.md.


Great Deluge adds all three pinned GreatDelugeAcceptorTest methods (36 acceptance
assertions). Its exact native arithmetic supports fixed score rain and rational
rain across six model/backend paths. Separate original comparisons verify all
276 native cases against exact Python enumeration; 275 final assignments/scores
agree with public Community solving. One observed ratio boundary differs, and a
25-prefix probe records two differing outputs separately from agreement passes.
Floating accumulation reproduces the observed delay but is not a claim about Java
internals. General floating/overflow and decimal search parity remain unverified.
The new full-scoring timing workload uses fixed rain only. See
incremental-search-contract.md for the contract and evidence limits.


Native SolverManager adds bounded concurrent asynchronous solving, queued/active/
finished statuses, result handles, duplicate-ID validation and cooperative
cancellation. Three complete SolverManagerTest methods are adapted with their
construction and synchronization fixtures. Native queued cancellation returns an
explicit error, and active cancellation requests do not wait; Java methods asserting
input-return or blocking termination remain pending. Original regressions exercise
all six actual solver paths inside manager jobs, along with ownership, errors,
panics and shutdown. Callbacks, live problem changes and manager performance are
not established by these tests. See solver-contract.md.


Started/final/error callbacks now add three complete SolverManagerTest adaptations:
exceptionInSolver, exceptionInConsumer and testStartJobConsumer. Final/error delivery
uses separate consumer threads, waits include delivery, and callback failures do not
replace successful solve results. Native original regressions establish ordering,
failure containment, cancellation, shutdown and concurrency; all six solver paths
exercise final delivery. Intermediate best events, live changes, Java consumer
scheduling and native allocation-failure recovery remain unverified. See
solver-contract.md for the callback contract.


The complete SolverManagerTest.skipAhead method now has an attributed adaptation:
four model-validated assignments, a blocked consumer skipping obsolete snapshots,
initialized final state, fewer than four best events, exactly one final event and
no callback failure. Explicit producer/consumer channels make its race deterministic.
Native publishers and closures replace Java custom-phase/event plumbing. The new
exact-search observers are independently tested across basic, list and mixed models;
those original tests add no source-method credit. Construction/local-search event
automation, first-initialized events and problem changes remain pending.

Native synchronous construction/local-search observers now cover basic, list and
mixed models with both scoring backends. They expose cached committed states and
connect directly to the managed best publisher. Original cross-backend, budget,
validation, cancellation, plateau, panic and controlled-time regressions establish
the native contract without additional upstream method credit. Unified ordered-plan
observation and Java first-initialized/producer event semantics remain pending;
independent per-phase initial notifications are not claimed to match Java event counts.

Observed ordered-plan APIs now cover all six solver paths, including exact phases.
Native phase indices/kinds accompany retained-best updates, with duplicate initial
scores suppressed across phase boundaries. Original tests establish work/result
parity, shared limits, cancellation, failure prefixes and managed delivery. This
adds no source-method credit; Java producer IDs, first-initialized semantics and
custom/repeated construction phase behavior remain unverified.


Resumable construction APIs and repeated construction phases now work across all
six native model/backend paths, preserving prior assignments and shared controls.
Nine original integration regressions, one controlled-clock unit regression and
a public doctest establish this native contract; they add no source-method credit.
The pinned manager first-initialized tests remain pending: their helper records
non-early-termination, and event boundaries/producer identity still need an explicit
implementation. Custom phases and construction-free plans are not yet supported.


Initialization boundary events and reliable managed initialization delivery now
add three complete SolverManagerTest adaptations: default phases, a single
construction phase with no event, and two construction phases followed by local
search identifying the second construction phase. Provenance and notices include
the event helper, default phase configuration and constant-score test fixture.
The six other initialization-event methods remain pending; native regression
coverage does not replace their custom-phase, construction-free or early-termination
source assertions. Native exact boundaries and consumer scheduling add no Java
trace claims. See solver-contract.md for the event and delivery contract.


Explicit construction-free sequences now add the complete
SolverManagerTest.firstInitializedSolutionConsumerWithSingleLSPhase adaptation.
The two entities retain their initialized first values, a sole local-search phase
uses target score zero, the final consumer is present and initialization remains
absent after waiting. Existing plans/configurations convert without changing their
construction-first behavior. Native direct-exact boundaries and zero-work handling
have original regression coverage only. Five source initialization-event methods
remain pending, including custom phases and early-construction behavior.

Native construction now has an optional per-phase assignment-step limit across
basic/list/mixed models and both scoring backends. Original tests cover partial
initialization events and resumable phase handoffs. No source-method credit is
added: the Java early-CH exception path and optional-list construction behavior
remain pending, as do the custom-phase initialization cases.


Score-aware optional construction adds three fully adapted methods: the basic and
list optional-construction cases in DefaultConstructionHeuristicPhaseTest and the
early-terminated optional-list initialization case in SolverManagerTest. Explicit
native NonWorsening configuration preserves every source result assertion; it is
not a Java default-policy or internal evaluation-trace claim. The optional-list
fixture preserves both entity-size and unassigned-value penalties. Four source
initialization-event methods remain pending (early required construction and three
custom-phase cases); original optional regressions add no further method credit.


Standalone custom commands now use the six native transactional scoring backends,
with committed-best observation, temporary rollback, limits and explicit failures.
This is execution infrastructure for future ordered custom phases. It adds no source
method credit: custom producer IDs, ordered command/phase integration and the four
remaining initialization-event methods are still pending. See solver-contract.md.

Ordered custom phases now add the three complete SolverManagerTest custom-phase
initialization adaptations. Actual typed commands run inside solver phases and
assert initialization has not yet happened. Custom/CH/LS attributes the event to
CH1; CH/custom/LS to Custom1; two custom phases emit none. The adapted tests replace
a 1 ms timeout with one deterministic shared unimproved local-search step, explicitly
recorded in provenance; timing/trace parity is not claimed. Early-required-CH remains
pending because native incomplete handoff and Java exception contracts differ.

The required-variable early-construction initialization method is now fully adapted,
including the error branch. Explicit IncompleteHandoffPolicy::Error rejects the next
local phase after publishing the real early initialization event. A managed typed
error chain preserves the source invalid-start category and initialized-solution
message; Rust enums replace Java exception classes. The fixture penalizes assigned
entities only. All nine pinned firstInitializedSolutionConsumer methods now have
full adaptations. Broader lifecycle behavior, live changes and deferred-score
custom commands remain pending; this does not establish full manager compatibility.

Three pinned SolverManagerTest live-change methods are now adapted: addProblemChange,
addProblemChangeToNonExistingProblem_failsFast and addProblemChangeToWaitingSolver.
Each preserves the four-entity/four-value fixture and every source result assertion.
Changes use a real owned input queue and native solving; their receipts wait for
successful best/final consumer return. The waiting case runs the source custom
pause followed by construction on a single worker. Rust ownership, revision receipts
and explicit rebuilding replace Java reflection and ProblemChangeDirector. This is
method-level behavioral evidence, not complete live-planning/API compatibility;
within-run intermediate publication and automatic domain reconciliation remain out
of scope for this increment. Original live_changes regressions earn no source credit.

Four more SolverManagerTest methods are now adapted: terminateEarly,
terminateSolverJobEarly_stillReturnsBestSolution,
terminateScheduledSolverJobEarly_returnsInputProblem and
submitMoreProblemsThanCpus_allGetSolved. Explicit synchronous native cancellation
preserves the source completion/status assertions; submit_owned preserves exact
queued-input identity and a latched termination-request flag. Active custom phases
wait for cancellation to remove fixture scheduling races. The batch uses twice
Rust's detected available CPU count, two real custom assignment phases and all
three source consumer modes, including optional coalescing of the first improvement.
Every source result assertion is retained with stable integer entity/value identities.
Java signatures, default pool sizing, arbitrary cross-phase timing and native initial
unassigned-score notification parity are not claimed. Native-only regressions receive
no additional source-method credit.

Three further manager statistics methods are fully adapted: the finished score and
move counts, finished problem size, and queued problem size updated after a live
change. Real solver output supplies the exact five/four work counts; native model
metadata supplies scales 4, 256 and 625. All source assertions are retained, with
unsigned speed/Duration types preserving nonnegative checks. Metadata has an explicit
revision and is updated before publishing its solution. Native exact/upper-bound
BigUint assignment spaces and decimal output do not claim Java logarithmic formatting,
class maps or unused fact-registry behavior; the separate ProblemSizeStatisticsTest
methods remain pending. Native-only statistics checks add no source-method credit.

SolverManagerTest.threadFactoryIsUsed is now fully adapted through an actual native
spawn hook. The supplied factory creates the worker named CustomThread; an actual
custom solver phase checks that name before construction solves the four-entity,
four-value fixture. The successful final result assertion is preserved. Native
Builder/JoinHandle replace Java ThreadFactory and reflective configuration. Factory
helper attribution is recorded separately. Additional native pool dispatch, cleanup,
wait-protection and live-restart tests do not earn source-method credit.


SolverManagerTest.solveWithBuilder and solveAndListenWithBuilder now have complete
adaptations using native deferred lookup and explicit solve/callback configuration.
Both run the attributed two-entity/two-value fixture with constant-zero scoring,
construction and ten-step local search. Final-consumer synchronization and non-null
result assertions are retained; the listening case also checks its best solution
and origin, copied from actual native solver updates. Timed channels replace the
source final-callback barrier; extra completed-job/failure checks prevent swallowed
callback errors from receiving credit. Native phase kind/index replaces Java producer
identity, with no Java API or ABI claim. Six original infrastructure tests receive
no source-method credit. solveGenerics and mocked overload delegation remain pending.


SolverManagerTest.solveWithOverride and solveWithTerminationSpentLimit now have
complete adaptations. A reusable native factory runs real construction and ten-step
local search with per-job effective settings on the attributed two-entity/two-value
fixture. Job settings preserve the one-second default's 0.05 gradient at 50 ms and
the 100-ms override's 0.5 gradient, including the convenience setter. Explicit Duration
values replace mocked SolverScope clock values. Native tests additionally establish
actual limit enforcement, independent defaults/cancellation and metadata persistence.
The source solveWithTerminationUnimprovedSpentLimit remains pending: native field
mutation supports the functionality but does not reproduce its raw override-object
and static TerminationFactory invocation assertions. No extra source credit is added
for native-only tests, factory naming or Java reflection/XML/generic behavior.

## Pillar change and swap increment

Native Pillar rows support atomic single-/multicolumn basic-variable changes and
swaps with unequal member counts. Opt-in whole-equal-value grouping is integrated
with scoped/stream basic and mixed local search, including mixed joint moves.
Domains, optional values and pins filter the automatic provider; raw move snapshots
use the existing transactional primitives. No subpillar enumeration, inferred entity
schemas or Java descriptor/rebase behavior is implemented by this increment.

Six complete source methods are adapted: PillarSwapMoveTest's bothOldValuesAreReadBeforeEitherPillarIsMutated,
unequalSizedPillarsSwapEveryMember, multiVariateSwapsEveryDifferingVariable and
undoRestoresEveryMembersOriginalValue; SelectorBasedPillarChangeMoveTest's
isMoveDoableValueRangeProviderOnEntity and doMove. The latter preserves all raw
execution assertions including no-ops and an out-of-range third member. The native
candidate generator performs doability filtering, while validated application checks
the model separately. Fixture provenance includes source variable-selection helpers,
basic/multivar generators and the optional entity-range fixture. Source rebase,
Java descriptions/getters/equality, provider-specific subpillar enumeration and
caller-misuse range-assertion behavior remain pending. Six independent native tests
cover additional validation, scoring, search and laziness with no source credit.

Existing differential/performance workloads exercise default neighborhoods. Their
refresh checks continued correctness and provides current default-workload timings;
it does not measure pillar-specific speed or establish Java pillar-search parity.

## Subpillar selection increment

Native bounded combination and consecutive-member selection now extends Pillar rows
and basic/mixed local-search neighborhoods. Independent change/null-target and swap-
side settings filter every selected member's range and pins. Existing whole-pillar
APIs and default neighborhoods retain their behavior. Finite enumeration replaces
random resampling; no Java sampler distribution, sequence, aliasing/reset lifecycle,
custom comparator identity or reflection behavior is claimed.

Thirteen complete provider test methods are adapted: five from
SubPillarChangeMoveProviderTest (membership, diversity, pin exclusion and both null
policies), all four SubPillarUnassignMoveProviderTest methods (including twenty
undersized singleton slices), and four SubPillarSwapMoveProviderTest methods
(membership, diversity, independent side sizes and pins). All source assertions and
sample caps are preserved over native finite iterators with attributed fixtures.
The explicit crossingNull constructor rejection remains pending because native
heterogeneous slots filter None per member rather than reject a whole schema.
Shared-sampler identity/reset, Java constructor variants and multivariate automatic
provider behavior also remain pending. Explicit multicolumn Pillar::subpillars is
native functionality, not evidence for those methods. Six new original tests and
an expanded mixed regression grant no additional source credit.

Current default-workload benchmarks do not isolate subpillar selection/search cost.

## Multivariable pillar schema increment

PillarSchema attaches explicit selected entity rows to a finite basic model. Whole
and subpillar providers group on complete selected tuples, respect selected row pins
and reject a swap if any column/member cannot receive its cross-value. Changes are
one selected column at a time. Both basic scoring backends and mixed joint search
use the schema. Pure pillar_swap_moves enumerates both directed whole-group pairs;
existing no-schema provider behavior remains unchanged.

Eleven complete source methods are newly adapted: eight PillarSwapMoveProviderTest
methods (equal-values exhaustion, at-least-one differing column, cross-range and
mixed-column rejection, pins, three-versus-five members, singleton groups and
excluded-column grouping), plus SubPillarSwapMoveProviderTest's
multivariateSwapUsesEveryListedVariable, excludedVariableIsIgnoredForKeyAndSwap and
mixedLegalityAcrossVariablesRejectsWholePair. Directed expected moves are independently
constructed and both orientations asserted where required. Fixture provenance covers
basic generators, multivar/optional/entity-range/pinned fields and selection helpers.
Finite enumeration supplies the observed behavior without random-stream equivalence.
Java single-variable/list constructor overload assertions, empty-list constructor
message assertions and shared stateful sampler identity/reset remain pending. Native
schema validation alone grants no credit for them. Six original tests grant no extra
upstream credit. Existing search benchmarks do not isolate schema-provider overhead.

## Explicit-group pillar transaction comparison

`scripts/benchmark_pillars.py` checks 128 original seeded cases against independent
Python simultaneous writes and the pinned Community public `Moves`/`MoveTester`
APIs. Inputs select members and one to three columns explicitly, including optional
values, no-ops, unequal groups and subgroups. Every response must match the trial,
restored and committed states and repeated-state checksum. These original cases
add no adapted upstream method credit.

Six workloads measure single-column change, single-column swap and three-column
swap at 16 and 64 entities. Timing includes group/move construction, temporary
application, full state copies/checks, undo and checksum consumption. Native
structural/domain validation and Java test-framework costs differ. Parsing,
model/context setup, a separate commit check and output encoding are excluded.
The defaults use 1,000 transactions per batch, five warmup batches, seven measured
batches and three fresh processes per runtime, alternating runtime order. Reported
ranges and medians are batch averages per transaction, not latency percentiles.
Raw observations, inputs, checksums, artifact hashes and environments are retained
in the downloadable report. The dashboard labels these transaction measurements
separately: they do not measure automatic grouping/schema providers, random
sampling, scoring, solver search, memory or overall solver performance.


## Derived basic reference values

One complete method from `BasicInverseShadowVariableTest` now checks the initial
owner code, entity-to-group reassignment, referenced-group owner changes, and
changes to an unreferenced group's owner. Stable IDs replace Java object identity;
`follow_reference` streams replace annotated shadow fields and their hidden inverse
supply. All source assertions and four fixture attributions are recorded. This
establishes the observed derived-value behavior, not annotation/reflection or Java
internal state equivalence.

Nine original regressions cover scan equivalence across 2,000 generated changes,
selective ordered dependent updates, optional payloads and missing targets,
callback/clone panic safety, chained references, self/cyclic payload references
with finite one-hop semantics, composite basic moves, list previous-element views,
foreign ports, construction and saved local-search candidates. These add no
upstream credit. Graph dependencies remain explicitly constructed and acyclic;
recursive shadow fixed points, automatic field dependency discovery, Java shadow
inconsistency rules and full declarative shadow compatibility remain unimplemented.


## Dependency schedules and cycles

All three methods from `DependencyValuesShadowVariableTest` are adapted with the
four dependency testdomain fixtures. A twelve-task/four-list schedule preserves
all source start times, durations and fixed dependencies, solves through both
native scoring backends under a 1,000 move-evaluation cap, and checks every source
finish/start inequality. Temporary insertion of C before B,A preserves every null
start/end assertion through undo and additionally checks the inconsistency states
described in source comments. The fixed B/C loop is rejected with all requested
variable/value names and without the unrelated Value A. Native configuration
validation rejects it before solving; Java exception class and lifecycle stage
are not reproduced. All attributions, including the specified error prefix, are
recorded. Whole-minute fixture times are represented as exact minutes from epoch.

The independently implemented `DependencyGraph` is an immutable plan with explicit
fixed and per-evaluation dependency edges. Iterative SCC discovery reports exact
fixed-cycle members; a topological evaluation marks dynamic cycles and every
transitively blocked value inconsistent, without evaluating those callbacks.
Unavailable values outside cycles remain consistent. Six original tests check all
512 directed three-node graphs as fixed graphs and as dynamic graphs against an
independent reachability oracle, cycle recovery, sorted/deduplicated dependencies,
validation before callbacks, callback unwind, non-Clone values and a 20,000-node
chain. They earn no extra upstream credit.

Both scoped and stream fixture scoring invoke a full pure dependency projection;
this is not an incremental shadow cache or Java update-order implementation.
Automatic field discovery, arbitrary annotation semantics, recursive fixed points,
general Java inconsistency parity and dedicated dependency performance comparisons
remain unimplemented or unverified. Existing refreshed timings do not measure this
new evaluator.

## Shared follower suppliers

All three FollowerValuesShadowVariableTest methods are adapted with five source
fixtures. The solve fixture has three leaders, eight followers and two values;
its exact Random(0) follower-to-leader sequence was checked using JDK 21. A native
1,000-move cap and full-score checks accompany every source follower/leader equality
assertion. All fifteen move assertions preserve null followers and unchanged
followers after changing an unrelated leader. Supplier observations are reset after
initialization: changing leader A invokes A1 exactly once and A2/A3/B1/B2 never.
Stable IDs and stream snapshots replace objects, Mockito and annotated fields.

`follow_shared_reference` groups explicit references and evaluates a supplier using
the smallest native tuple ID, broadcasting its result to every group member.
The caller promises that members share the same intended result. General Java
representative choice, automatic alignment/dependency discovery and annotation
semantics are not claimed. Native membership changes recompute affected groups;
updates are not coalesced across separate source changes. Six original regressions
cover 1,500 generated changes against a scan, exact target-update supplier counts,
representative changes, empty/missing groups, rollback, supplier panic, finite
same-source cycles, foreign ports and construction/saved search candidates against
independent full scoring. They add no upstream method credit. Existing benchmarks
do not isolate this operator and no direct Java generated follower comparison is
claimed by this increment.

## Dynamic followers and direct public-runtime evidence

All three DynamicFollowerValuesShadowVariableTest methods now preserve their
complete source assertions. Native reference streams cover leader-value changes,
follower leader reassignment and null unassignment. The three-leader/eight-follower/
two-value solve begins unassigned, uses a 1,000-move cap and full-score checks,
and verifies every final follower/leader equality. Numeric slots replace Java
objects; derived graph snapshots replace externally stored shadow fields. Search
traces, annotation discovery and Java lifecycle identity are not claimed.

`follower_checks.py`, integrated into stream differential reporting, compares 128
cases, 2,373 states and 3,714 operations against the pinned Java public runtime and
independent Python calculations. It covers both fixed shared and dynamic follower
references, zero followers, null/missing references, null leader values, repeated
writes, composite commits and temporary moves with undo. Every ordinary score,
completeness flag, leader value, reference and derived value must agree. Java
MoveTester maintains shadows; SolutionManager.UPDATE_SCORE_ONLY computes each
observed score. Rust uses persistent reference/shared-supplier graphs. Fixtures
and the reused static boundary are attributed; generated cases grant no method
credit. Seven Python regressions reject corrupt, missing or reordered responses
and verify independent null, sequential-write, completeness and undo boundaries.

This evidence corrected a fixture scoring error in the previous increment:
`forEach(HasValue.class)` in this pinned interface-based model includes all
implementing entities, including null genuine variables. Generic concrete-entity
filtering was an incorrect assumption here. Both native follower fixtures now count
all interface rows; the static fixture scores are -64, -32, -26 and -34 across its
initial/three-change states. Existing source value and supplier-count assertions
still pass. This is observed behavior of this interface query, not a general rule
for all concrete-entity sources. Supplier counts, Java structural-score fields,
Java incremental scoring performance and follower solve quality are not compared
by these generated checks. Java's null alignment-key supplier-call behavior remains
outside the native shared-reference compatibility claim.

## Collection follower conformance

Both FollowerSetValuesShadowVariableTest methods preserve all source assertions:
AB/AC/BC derived values after three changes, and all eight final follower values
against direct supplier evaluation in the 3/8/2 solve. The exact seed-0 distinct
leader pairs were verified with JDK 21. Native full-score checks and a shared
1,000-move cap accompany the adapted solve. Explicit membership joins and
conditional ordered collections replace annotated fields. All fixture, inherited
code/identity, and solution-generator attributions are recorded.

The supplier selects the first non-null value with the minimum code. Native keys
use UTF-16 code units and reference-list position, preserving Java string ordering
and first-reference ties even for distinct identities with equal codes. All-null
groups produce None, and explicit follower projections retain each row in the
interface-based score, including null values. Two original native regressions
check Unicode/tie identities, duplicate references, all-null transitions, undo and
search agreement with independent full scoring; they earn no method credit.

`collection_follower_checks.py` adds 128 separate public-runtime cases, 2,368 states
and 3,724 operations. Both runtimes and independent Python agree on every leader
value, derived value identity, ordinary score and completeness flag through
composite commits and trial/undo. Include zero followers, duplicate references,
empty and duplicate code strings, BMP/non-BMP order, repeated writes and nulls.
Java MoveTester updates shadows; scores are rebuilt with UPDATE_SCORE_ONLY.
Seven Python regressions validate independent boundaries and reject corrupted,
missing and reordered observations. The dashboard reports this evidence separately
from adapted method coverage and performance. No supplier-call counts, mutable
code/dependency lists, empty/null dependency collections, Java PlanningId behavior,
structural-score fields, search quality or timing comparison is claimed.

## List-element derived values and owner aggregates

All six ListElementShadowVariableTest methods preserve their source assertions:
ordered element end times and owner last-end values, empty-list fallback and
unassigned nulls, a composite relocation/swap, a basic-only duration change, and
two 1,000-move full-assert solves with the original generated fixtures. Optional
list values remain optional in the ordered fixture. Native stream projections
replace annotations; changed owners rescan their ordered chains, while membership
joins and nullable grouped sums update mixed aggregates from basic changes.
Three original Rust regressions cover null versus empty aggregates, removal,
transfer, undo and bounded search agreement with independent full scoring.

A separate 128-case Java/Rust/Python comparison (64 ordered, 64 mixed) checks
2,365 states across 3,675 operations. Independent Python validates every ordered
owner, previous element, start/end time and last-end aggregate, as well as mixed
padded durations and nullable totals. Cases include zero values, empty owners,
initial unassigned values, assign/relocate/swap, ordered unassignment, basic-only
changes, sequential composites and undo. Generated inputs earn no method credit.

Java uses public MoveTester with explicit entity metadata, which also handles
zero-value fixtures where automatic entity discovery failed. The ordered score
is computed by a full UPDATE_ALL on a separate solution rebuilt from genuine and
fixed inputs; the original move-driven shadows are observed unchanged. In this
fixture UPDATE_SCORE_ONLY returned zero even with correct visible shadows, so it
is not used as a scoring oracle. Mixed fixtures have no score provider; their
zero protocol field is not scoring evidence. Seven Python tests cover independent
boundaries and reject corrupt/missing/reordered observations. Supplier counts,
reflection, fixed-property mutation, search quality and timing are not compared.
