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
are verified separately with original regressions. Complete Java move metadata coverage, rebase,
custom listeners, optional-list solving and matched mixed Java local-search
comparisons remain pending.
Five additional mass-list-change methods preserve ordered gathering, original
destination adjustment, assignment of missing members, unassignment and undo.
Native composite equality is separate from Java mass-move equality/hash/rebase.
The generated public-API gather comparison earns no method credit.
Six planning-value methods additionally preserve complete mass/pillar/list/sublist
metadata assertions, with native pre-state capture retained across execution.
The public Java getter lifecycle remains distinct from the native API.
An original list planning-value comparison additionally checks 514 native cases
against independently replayed identities: 425 Java membership agreements,
16 differing sets, 40 Java errors and 33 unsupported empty compositions.
Those limitations remain visible; the generated checks add no adapted methods.
See the list-planning contract for two small composite reproductions.
Original generated list-move and required-list-solving comparisons are separate
from coverage; the latter check complete valid solutions, final quality and
independently enumerated small-case optima.

The first three integer score families have 47 adapted tests; BendableScore adds
27, using native compile-time dimensions in place of Java score definitions; basic change/swap
moves add ten adapted cases; provider behavior and hill-climbing acceptance add
nine more. Four late-acceptance methods validate history behavior and hard-score
resets. Three tabu-sizing methods preserve all fixed-size validation and population-ratio
rounding/boundary fixtures. Native phase integration and broader ratio boundaries
have separate original regressions and do not add upstream method credit.
Two diversified-late-acceptance methods additionally preserve all trial acceptance
and private-history replacement assertions. Native phase/candidate behavior and
original backend/search checks are specified in the incremental-search contract.
Four construction-forager methods preserve early-pick decisions. Three
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

## Shared concurrent scheduling and reference-runtime failures

Seven ConcurrentValuesShadowVariableTest methods retain every neighbor/index,
cascading-time, shared start/finish, inconsistency and recovery assertion. Initial
supplier-count assertions retain A1/B1 as the only group callers. Both source
3-owner/6-value solves retain the 1,000-move limit, full-score checks and feasibility
assertion; an independent closure/relaxation score checks every native graph
projection. Minute offsets replace LocalDateTime from the fixed 2025-01-01 09:00
base. Native explicit dependency alignment replaces annotated group suppliers.
No general Java supplier-count or search-trace equivalence follows.

Original production-evaluator tests cover all 512 three-node directed graphs under
all five partitions (2,560 combinations), alignment-created fixed-edge cycles,
recovery, canonical inputs, invalid metadata before callbacks, unavailable values,
unwind reuse and a 10,000-node group. Two domain regressions check transactional
cycle/undo snapshots and 1,944 small partial orderings against independent scores.

Generated public comparisons retain all 128 cases and 2,385 native states across
3,574 operations. Every native field and score matches independent recursive
Python expectations. Java emits 2,025 matching states, fully agrees on 107 cases,
and throws in 21 cases; none of those error cases receives agreement credit.
The report preserves case inputs, native observations and Java error diagnostics.
No differing successful Java observations were seen. Seven Python tests enforce
reference boundaries, failure accounting and response integrity.

The minimal failure has three values, group [0,1], lists [2,1]/empty: first group
member 0 is unassigned. Java can leave assigned member 1's finish null, then throw
while calculating finish-time penalties. ConcurrentOriginalBoundary compiles the
original pinned concurrent fixtures unchanged and reproduces both null finish and
a scoring NullPointerException through public APIs. The native model follows the
fixture's independent expected-value semantics: a partially assigned group shares
the maximum available ready time, including with unassigned members. This known
reference-runtime boundary remains a reported discrepancy, not a compatibility
pass or justification to remove cases. Mutating groups, general date/time APIs,
Java structural scores, search quality and performance remain outside this evidence.

## Persistent dependency topology

Seven AbstractTopologicalGraphTest methods preserve every ordering, SCC membership
and cycle-status change assertion. The concrete DefaultTopologicalGraphTest has an
empty consistency hook and explicitly does not require incremental maintenance.
Native DependencyTopology keeps a fixed key set and persistent edges; an atomic
update validates all endpoints, applies removals before additions and returns only
keys whose cycle membership changed. Edges point node-to-dependency (reversed from
the source tests). Sorted SCC members and shared component ranks are queryable.
Ranks order dependencies before their consumers, including condensed cycles.
Downstream dependencies of cycles do not themselves become cycle members.

All node and edge permutations are retained: 14,400 each for the two acyclic
methods, 86,400 each for the two five-node cycle methods, 518,400 each for the first
two six-node methods, and 362,880 for the last. Test-only Java-sequence shuffling
and the original recursive limit preserve the larger source families. A standalone
Java helper emits complete endpoint-stream checksums; Rust verifies all seven
counts and hashes. This validates test-input fidelity, not Java topology behavior.
The reference contracts are [Java Random](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/util/Random.html)
and [Collections.shuffle](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/util/Collections.html#shuffle(java.util.List,java.util.Random)).

Three original regressions compare all 512 three-node graphs and their 4,608
single-edge toggles against independent transitive closure, verify atomic failure,
idempotent/conflicting batches and snapshot isolation, and exercise a 20,000-node
chain/cycle/recovery without recursion. The existing fixed-cycle validator shares
native SCC discovery. Topology currently rebuilds components/ranks after actual
edge changes; it has no incremental value cache or performance comparison.

## Incremental derived-value cache

`DependencyCache<K,V>` now uses persistent topology and value snapshots to evaluate
explicitly dirty keys and propagate changed derived values to their dependents.
Equality includes both optional value and inconsistency; consistent None and a
cycle are distinct. Changed incoming edges invalidate the consumer even if input
values are equal. Cycle-membership changes are propagated through values, including
downstream recovery. No-op edges trigger no suppliers. Value-only updates do not
scan the universe. Edge changes still rebuild topology and reverse indexes.

`StreamGraphBuilder::derive_dependencies` owns this cache in a stateful graph node.
Rows and derived fields participate in normal rollback, saved-candidate installation
and diagnostic rebuilds. Absent rows supply consistent unavailable values; unknown
keys/dependencies fail the transaction. Payloads include original row metadata, so
metadata changes are emitted even if the derived value remains equal. Closures
must read immutable captures; external side effects cannot be rolled back.

Five original cache tests cover every three-node graph with all single-edge changes
and external-value invalidations, equality cutoffs, changed input identities,
metadata, callback and equality failure atomicity, cycle/recovery, and shared snapshots of
non-Clone values. A 10,000-key sparse update is checked to invoke one supplier and
fewer than 500 key comparisons. Five stream/solver regressions check 1,296 partial
parent assignments, composite commit/undo, cache recovery, retraction, changed
metadata, downstream score failure and supplier unwind, foreign/duplicate keys,
construction and saved local-search candidates against independent reference
scoring. This work earns no additional upstream method credit.

The public concurrent comparison additionally drives a second persistent native
cache through every state and restores its snapshot for trial undo without supplier
calls. Fixed representative starts and aliases express the fixture's shared groups.
Every field must agree with the existing full native projection and independent
Python. Java runtime errors remain explicit discrepancies, not native failures or
agreement credit. Native alias supplier counts are not Java supplier-count parity.
The concurrent stream fixture now uses the same cache through an owned projection
operator, with a separate full projection and independent score reference retained.
This does not establish a new timing or whole-solver speed claim.

## Cached concurrent scoring in actual search

The concurrent fixture now uses map_cached during construction, local search,
saved-candidate installation and trial undo. Every projection still checks its
ordinary score against independent group-closure scoring; full native projection
is retained for field comparisons. Five original operator tests cover per-identity
isolation, retraction/reinsertion, cold diagnostic rebuilds, warm undo, callback
unwind and downstream score failure, foreign ports, cloning only one of 1,000
per-identity caches, and rejection of deliberately history-dependent output by
score rebuilding. Two original domain regressions compare cached construction and
ten bounded local searches (hill climbing and variable tabu) with independent
ListConstraintNetwork scoring, and prove warm cached state survives a cyclic trial.
These extend implementation evidence without adding upstream method credit.


## Concurrent shadow performance evidence

An original benchmark driver reuses the attributed concurrent fixtures to measure
fully assigned list moves and cached shadow maintenance. It compares all nine
fields against independent Python through trial, undo and commit, in 64 seeded
small cases and every sample of four larger workloads. It calls public Java
MoveTester and native ListProblem/CachedSchedule, with no scoring/search timing.
Source and destination positions must differ because Java rejects same-position
moves. This is an explicit input contract, checked before either runtime starts.
The existing partial-group error cases remain in the separate correctness report;
benchmark failures abort rather than discarding cases. No upstream method credit.


The dependency cache's internal scheduler now uses dense stable indices for
reverse links and queue entries. Public keys, callback inputs and changed-key
ordering remain key-based. The new string-key fanout/late-clone-panic regression
is original and earns no additional upstream method credit. Value storage still
uses persistent snapshots, and changed edges still rebuild the topology.

## Consecutive-sequence collection

Twenty methods are adapted: all twelve ConsecutiveSetTreeTest methods and the
add/remove plus update methods in all four Inner*ConstraintCollectorsTest arities.
Both seeded random tests replay their exact 1,000-operation inputs, verified by a
standalone attributed JDK fixture generator. Independent pairwise connectivity
replaces expected results rebuilt with the implementation under test. Source item,
count, endpoint, break and chain assertions are preserved with explicit native
snapshots; the UTC Timeslot fixture uses exact seconds and SignedDuration.

Native logical IDs replace Java object identity, stable ID order replaces identity
hash ties, and fresh immutable snapshots replace live views. Captured triples must
stay equal while an identity has outstanding occurrences. Java mutable-object
aliasing is not claimed. Integer distances widen exactly instead of reproducing
int overflow. All of these adjustments are recorded in provenance; connected-range
collectors and Java API/ABI compatibility remain outside this increment.

The public Java/native/Python comparison validates 64 original cases and 4,689
states, including duplicate identities, duplicate indices, slot replacement,
removal and trial/undo. It checks all item membership and observable metadata;
Java tie ordering is allowed only within equal indices, with its endpoint identity
checked against its own items. A separate MIN/MAX int break differs (-1 in Java,
4,294,967,295 in Rust) and receives no agreement credit. These generated checks
and five original native regression methods add no upstream-method credit.

The original consecutive-sequence benchmark adds 64 independently checked
replacement/inverse-update/commit cases and four full-observation timing workloads.
Its bounded indices exclude Java overflow, already retained as a separate
correctness discrepancy. Benchmark observations canonicalize equal-index identities
into per-ID membership/index/component arrays; sequence and break numeric fields
remain complete. Exact endpoint identity/tie-order correctness remains covered by
the earlier differential driver. These measurements are not solver performance
or additional upstream test-method coverage.

## Connected-range collection and reference discrepancies

Twenty additional methods have adaptations: eight ConnectedRangeTrackerTest methods
and consecutiveUsage, consecutiveUsageDynamic and consecutiveUsageUpdate in each
of the four collector arities. Source items, endpoint changes, counts, overlap
bounds and gaps are preserved through idiomatic captured native values. Independent
pairwise connectivity and midpoint counts replace self-derived expected trees.
Java's empty-only MAX/MIN overlap sentinels map explicitly to absent native bounds.
The source hash-equality method checks equal native snapshots/hashes; the separate
live-object hash-mutation method remains pending because native snapshots are
immutable. Java API/ABI, object-alias and numerical hash parity are not claimed.

The exact Random(1) fixture runs 100 trials of 100 operations. A first logical-value
approximation was rejected when checked against source input-selection scaffolding.
The final attributed generator executes that scaffolding with the pinned Community
jar. Fresh range identities make its containsRangeStarting branch false: the exact
trace has 10,000 additions and no removals. This limitation is not hidden. Original
native regressions separately cover 3,000 grouped edits with removals/replacements,
undo/rebuild, invalid inputs/callback unwind, immutable hashes, custom metrics,
empty/full-width boundaries, construction and 32 bounded saved-candidate searches.

Public collector comparison retains 197 original cases/11,583 states: 197 native
reference passes, 65 Java agreements, 132 differing cases, no Java errors. All 64
distinct-identity positive-length cases agree. The initially generated 128 cases
with shared identities remain intact after adding that separate cohort. The minimal
shared-object discrepancy inserts one [0,2) object through two handles, then removes
both: Java reports counts 1,2,2,1; native multiplicity semantics report 1,2,1,0.
Distinct equal-endpoint objects remove correctly in Java. Zero-length flags and
int-overflow lengths also differ. Raw inputs/outputs and every differing state are
retained; these cases do not receive agreement credit. Empty-only Java sentinels
normalize to None; item ordering is not compared across runtimes, while native
stable order is checked. Seven Python regressions enforce reference behavior,
response identity/shape, complete observations and truthful discrepancy accounting.
No new generated-case method credit or connected-range timing claim follows.

## Load-balance collection

The four Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest.loadBalance methods are
adapted with all source empty/add/duplicate/retraction unfairness assertions.
The uni record and tuple mappings become captured native key/load/initial triples;
tri/quad ignored inputs stay zero. Provenance and notices record these changes.
No Java API/ABI claim follows. Five original integration tests, one rounding unit
test, a doctest and seven Python tests add no upstream-method coverage.

Native exact totals/moments, initial-load consistency, missing captures, zero keys,
immutable snapshots, failed updates, 1,800 grouped edits with trial/commit/rebuild,
construction and 32 bounded searches have independent reference checks. Unfairness
rounds to six significant digits with half-even ties. This matches observed public
Java numeric precision, despite its six-decimal-place Javadoc. Native canonical
trailing-zero representation differs from Java scale; comparison is numeric.

Original public comparisons retain 101 cases / 8,290 states. Native matches
independent Python pairwise distances and high-precision Decimal roots in every
case. Java agrees in 97 cases, including all 96 bounded cases; four large-number
cases differ, with no runtime errors. At loads 1,000,000,000 and 1,000,000,001 Java
reports zero, native 0.707107. Full-width signed totals/squared values also expose
Java overflow. All raw inputs, outputs and differences remain in load_balance_checks;
none receive agreement credit. Initial loads are constant per active key in paired
inputs; conflicting native initial loads reject atomically instead of inheriting
Java's first-initial-load history. No live-map, general decimal-scale or timing claim.

## Calendar-period aggregation

Eight source methods are adapted: sumPeriod and sumPeriodUpdate in all four
Inner*ConstraintCollectorsTest arities. All default, add, duplicate, retract,
replacement and repeated-replacement assertions are retained with exact native
calendar components. Day mappings preserve each arity's original inputs; update
mappings keep ignored nullable trailing fields. Provenance and notices record them.
No full java.time API/ABI compatibility is implied.

CalendarPeriod and PeriodSumCollector are independent Rust implementations. Six
original integration tests cover field identity/sign/display, widened arithmetic
boundaries, multiplicity/zero capture, immutable snapshots, insertion/removal/both
replacement intermediate failures, 1,800 grouped edits with commit/undo/rebuild,
callback/structural rejection restoration, construction and 32 bounded searches
against independently summed calendar score levels. Two doctests and seven Python
protocol/reference tests add no source-method credit.

Public collector comparison retains 227 original cases and 8,771 observations.
Every year/month/day field or overflow marker agrees with independent Python and
Java. All 120 expected overflow rejections are identified as such; they are not
successful updates. Both runtimes stop at their first error, retaining complete input
and observed prefix; no Java post-error state is claimed. Period replacement checks
subtraction then addition even for the same Java object, as separately probed on
cancelling boundary inputs. Arithmetic never normalizes calendar fields or converts
them to fixed durations. Date application, time zones and dedicated timing remain
outside this increment.

## Captured-key minimum and maximum

Sixteen methods are adapted: minNotComparable, maxNotComparable and their Update
variants in all four Inner*ConstraintCollectorsTest arities. Every source assertion
and add/remove/repeated-replacement sequence is retained. Non-Ord native payloads,
captured keys and explicit handle identities replace Java comparable projections.
Nullable trailing tuple fields remain represented. Provenance and notices record
these test adaptations; no production implementation is translated.

MinByCollector and MaxByCollector select payloads by captured Ord keys. Both break
equal-key ties by lowest active stable ID. The public Java contract permits any
of the tied values and expressly allows that choice to vary across calculations.
Native deterministic identity supports full reconstruction and graph transaction
undo; it is not a claim of matching Java's tie history or mutable object aliases.

Six original Rust regressions cover duplicate IDs, nullable/non-Ord payloads,
Reverse/composite/full-width keys, captured removal, atomic conflicting updates and
injected clone failures, 1,800 grouped edits with undo/reconstruction, callback
failures, construction and 32 bounded saved-candidate searches. A doctest and six
Python protocol/reference tests add no source-method credit.

The public comparison has 196 cases and 16,528 states. Both Java and Rust satisfy
independent extreme-key, active-identity and captured-payload checks throughout.
The 2,429 differing tie choices are permitted by the contract and retained in raw
outputs; this is selection-contract agreement, not exact selected-ID agreement.
Duplicate item handles, signed/full-width keys, replacements, removals and inverse
update trials are included. No generated-case method credit or new timing claim.

## Collector composition and result transformations

Thirty-two additional methods are adapted from all four Inner*ConstraintCollectorsTest
arities: compose2, compose3, compose4, compose2Update, collectAndThen,
collectAndThenUpdate, minComparableUpdate and maxComparableUpdate. All source result
fields, operation sequences and zero trailing tuple mappings remain. Nullable tuple
results become Option fields. Non-uni natural-order update tests retain their fixed
2023-01-01 local date/time plus 1..7 minute fixtures with explicit ordered native
calendar fields; this does not implement a general date/time API. Source provenance
and notices record each adaptation. No production implementation was translated.

FinishingCollector adds a reusable pure result transformation around any reversible
collector. Nested PairCollector handles independently mapped inputs; custom finishers
flatten or derive results, including empty standalone outputs. Specialized inner
replacement semantics remain intact. Five original Rust regressions cover partial
inner mutation failures, specialized replacement delegation, real calendar-overflow
rejection on same-value replacement, finisher panic recovery, 1,800 grouped edits
with commit/undo/full recomputation, construction and 32 bounded saved-candidate
searches. One doctest and five Python reference/protocol tests add no method credit.

Original public comparisons retain 132 cases and 11,300 states across composition
of two, three and four collectors and count-result mapping. Java and Rust match all
independent min/max/count values and f64 average bits through duplicate inputs,
replacement, removal and inverse-update trials. Inputs keep intermediate integer
arithmetic representable. Callback count/order, live aliases, Java ABI and dedicated
performance measurements are not claimed.

## Sets, merged sorted maps and checked integer sums

Thirty-two source methods are adapted: toSet, toSortedSet, toSortedMapMerged,
countUpdate, sumUpdate, countDistinctUpdate, toSetUpdate and toSortedSetUpdate in
all four Inner*ConstraintCollectorsTest arities. All assertions, arity-specific
numeric tuple inputs/projections, null ignored string fields and operation sequences
remain. The uni merged-map fixture retains its alternating value counter, captured
once per insertion. Native sets check membership and natural sorted iteration;
map results use wrapping i32 addition as in Integer::sum. Count uses native usize
for these bounded fixtures. Provenance and notices record every adaptation.

Independent SumCollector implements checked i64 summation based on public black-box
observations. It leaves the existing IntSummary wrapping contract unchanged.
Replacement checks difference then final total, unlike AverageCollector's observed
wrapping replacement total. Five original regressions cover widened boundary
arithmetic, duplicate/zero/missing captures, atomic failure at both replacement
stages, unchanged-value replacement, MIN removal, snapshots, graph failure/rollback,
1,800 grouped map edits, construction and 32 bounded searches against independent
scores. A doctest and seven Python protocol/reference tests add no method credit.

Public set/sorted-set/merged-map/sum checks retain 270 cases / 19,484 observations.
Every successful result or expected overflow agrees with independent Python and
Java. There are 39 expected sum-overflow rejections; both runtimes stop at their
first error, so rejection agreement does not claim a successful update or Java
post-error recovery. Merged-map zero keys and i32 wrap boundaries remain explicit;
unordered set output normalizes order. No generic merge-function, Java API/ABI,
null-key/value, callback-order or dedicated performance claim.

## Remaining tuple collector methods and aggregate timings

Fifteen additional methods are adapted: count, countDistinct, sum, minComparable
and maxComparable in InnerBi/Tri/QuadConstraintCollectorsTest. All numeric tuple
inputs, mapped sums, duplicate/retraction sequences and expected results remain.
Date-valued extrema retain the fixed 2023-01-01 local calendar fields and 1/2 minute
positions; native Ord checks do not reproduce Java PECS type-system variance or
implement general date arithmetic. Provenance and notices state this distinction.

These finish the declared method inventory in the four Inner* collector classes:
58 uni, 59 bi, 58 tri and 58 quad methods. This is adapted native method coverage,
not execution of the original JUnit suite, full constraint-stream compatibility,
or closure of the independently documented Java/native boundary differences.

The aggregate benchmark adds checked i64 sum, naturally sorted set and additive
merged-map workloads at 64 and 512 captured occurrences. Every timed replacement
and inverse restoration materializes the complete scalar/set/map observation,
checks it against independent Python reconstruction and consumes a checksum.
Committed results are separately checked outside timing. Sets retain duplicate
pairs; maps start with eight contributions per key and replace one with a new key.
All arithmetic is bounded and representable. The 96 generated validation cases add
no upstream-method credit; six Python tests reject corrupt fields, checksums,
identities, batch counts and invalid input scope. The report retains raw samples,
warmups, fresh-process order and environment. This measures update plus complete
observation, not isolated mutation, graph rollback, solver speed, memory or overflow.


## Fading tabu acceptance

One additional Community method, EntityTabuAcceptorTest.fadingTabuSize, preserves
all acceptance and random-consumption assertions through seven step observations.
A native FadingTabuMemory and FadingVariableTabu acceptance policy integrate with
basic, list and mixed search using both scoring backends. Independent regressions
check chronological reference probabilities, duplicates, refresh, empty steps,
aspiration, extreme tenures, invalid supplied draws, reproducible native randomness,
actual stopped neighborhoods and mixed variable/entity index separation. Existing
backend and owned-score matrices now include this policy. No Java PRNG trace,
multiple-key fading, value/move tabu or new performance comparison is claimed.

Pinned documentation marks Score Analysis as Enterprise-only; that API is not an
authorized implementation specification for this Community rewrite. Existing native
per-constraint totals and typed match inspection remain independent diagnostics.


## Late-acceptance reset levels

Six DefaultLevelScoreStateTest methods are adapted with unchanged score fixtures
and reset assertions. They expose and fix the previous HardMediumSoft medium-level
history bug; integer and decimal families now reset on a new best in hard/medium,
while feasibility remains hard-only. Explicit score comparisons replace mocked
Java scope/index plumbing. Three index-cache/lifecycle methods remain pending.
Original failing-before/fixed-after regressions and complete history/search checks
are recorded in docs/incremental-search-contract.md. No source-method credit is
claimed for generated histories, decimal extensions or six-backend search checks.

## Composite acceptance

One CompositeAcceptorTest method is adapted with all five truth configurations;
the separate mocked lifecycle method remains pending. Native ordered conjunctions
combine primary and additional policies across all six search paths. Short-circuit
trial state, completed-step broadcasts, validation, owned scores and touched-key
tracking have original regressions, without extra upstream-method credit.
Two combined late-acceptance/tabu timing workloads and thirty-two small quality cases
extend the existing public Community comparison. Validity, equal final scores and
performance remain distinct; Java lifecycle and internal trace parity are not claimed.

## Typed move tabu

All three MoveTabuAcceptorTest methods preserve complete hard-tenure, aspiration
and supplied-draw fading fixtures with native stable IDs. The new typed acceptance
state integrates hard and fading move tabu across six native search paths using
full structural descriptors. Native Eq (including order and positions), Java
mock-identity adaptations and random-generator boundaries are documented separately;
no Java move-equality or lifecycle parity is inferred. Original chronological and
search regressions, plus two move-tabu benchmark workloads and 32 small quality
cases, earn no extra upstream-method credit.

## Value tabu

Five ValueTabuAcceptorTest methods are adapted in full, including multi-value steps,
repeated queries, unassigned/null identity and supplied fading draws. Typed value
acceptance integrates with basic/list/mixed search in both scoring backends. Native
capture observes actual write targets and moved list items through sequential
composites, with distinct mixed value types; its semantics and replay cost are in
the search contract. These helpers, independent chronology/search checks and two
new timing workloads plus 32 small Java/native quality cases earn no additional
source-method credit. Java mutable identity/lifecycle and fading PRNG parity remain
unclaimed; input validation, score quality and timing stay separate measurements.

## Local-search foraging

All seven AcceptedLocalSearchForagerTest methods now preserve their candidate
identities, scores, acceptance flags, every intermediate early-stop assertion and
chosen/empty result. Native typed saved candidates replace mocked Java scopes;
explicit references preserve best -10/current -100, and supplied finalist index 1
preserves the random-tie choice without asserting Java PRNG equivalence. Native
selection supports strict improvement over the best/current score, a per-step
accepted-candidate limit, and first or seeded random ties across all six backends.
Construction retains its existing selection behavior. Independent prefix references,
owned-score comparisons, retained-tie failure checks and search integration grant
no additional upstream-method credit. See docs/incremental-search-contract.md.

## Strategic oscillation

All four StrategicOscillationByLevelFinalistPodiumTest methods preserve every
candidate score/order, singleton identity and last-step/best-reference update.
Native immutable references replace Java phase scopes. This establishes those
fixtures, not Java lifecycle or whole-podium equivalence.

FinalistRanking adds last-step and retained-best strategic modes across all six
native search paths. Overall improvements outrank non-improvements; otherwise the
earliest ordinary level strictly above its reference wins, then full score breaks
ties. Accepted priority and structural ordering remain authoritative. Rejected
BestScore fallback remains ordinary score order. Early-pick matches bypass ranking;
accepted-candidate limits stop scanning without overriding ranking.

Original public-API comparisons cover 402 bounded score tables, three ranking
modes and four step prefixes (4,824 cases). Rust agrees with independent Python
throughout. Community agrees in 4,499 cases; 325 differ and are excluded from
agreement counts. Public STEP_SCORE and MOVE_COUNT_PER_STEP metrics observe the
last step and complete accepted candidate set; returned best identity/score is
checked separately. Very large annealing temperatures plus bounded levels make
all candidates accepted, confirmed at each prefix. No general annealing or PRNG
trace, internal API, additional source-method coverage or new timing claim follows.

A minimal boundary has reference (6,0,0), a candidate equal to it and candidate
(2,8,-2). Community chooses whichever occurs last in these two permutations.
Native consistently chooses the medium improvement, following the documented
preference over a candidate improving no level. This intentional total ordering
avoids encounter-order dependence except genuine tied finalists. All generated
inputs, both outputs and differences remain available in the report/work artifacts.


The full-scoring heuristic comparison additionally measures both strategic modes
with entity tabu tenure two, original-order changes, first ties and fixed step
budgets. Four new timed workloads use 32/96 variables; 64 new original small-case
checks independently validate assignments, rescored quality and exhaustive optima.
These generated cases earn no adapted-method credit. Observed equal final scores
do not establish identical traces or erase the separate exact-reference boundary.
Raw scores, calls, samples, per-algorithm quality counts and methodology are retained.

All seven pinned `LateAcceptanceScoreBufferTest` methods now have complete native
adaptations. `ScoreHistory` is integrated into late acceptance; score fixtures,
write/wrap/reset sequences and slot/current assertions are retained. Java internal
epoch commentary and native reset return values are not compatibility claims.
Independent eager-reference, owned-score failure and clone-count checks receive
no additional upstream-method credit.

Four exhaustive-search comparator methods now have complete native adaptations:
depth first, breadth first, score first and optimistic bound first. Every fixture
and all-pairs bidirectional comparison is retained, including unassigned counts,
explicit/default optimistic bounds and breadth metadata. SearchNodeKey replaces
mocked nodes; these fixtures do not claim Java traversal or lifecycle equivalence.
The independent basic-variable branch-and-bound engine is tested against exhaustive
enumeration and admissible prefix tables. Separate benchmark cases compare all four
public Java exploration configurations with native search on nonpositive-score
fixtures, validating results against Python optima. Their timings include setup,
search and result validation; node counts and traces are not asserted equal. List
and mixed bounded search now share the same independent engine, with original
subset/permutation/product reference tests and a metric routing example. Those
extensions add no adapted-method coverage or new Java timing workloads. Ordered
solver phases now use the same engine across all six model/scoring paths, retaining
shared controls, typed errors, prior incumbents and observations. Unknown bounds
remain safe; non-increasing partial scores require an explicit caller assertion.
Declared per-level trends now derive bounds with symbolic infinity; constraint
analysis does not infer the declarations. The native phase tests and managed-job
checks add no source-method credit or phase-trace parity claim.

The oscillation diagnostic keeps the Java solver strongly reachable through all
STEP_SCORE and move-count reads using Reference.reachabilityFence. A forced-GC
probe reproduced missing gauge observations without this lifetime protection;
24 representative cases now force GC between solve and observation and must match
the normal run byte-for-byte. This is an adapter reliability check, not additional
native-method or Java-behavior coverage. Micrometer documents weak gauge references
and missing/NaN observations after collection:
https://docs.micrometer.io/micrometer/reference/1.16/concepts/gauges.html.


All three pinned `InitializingScoreTrendTest` methods have complete native
adaptations, retaining every token and predicate assertion with provenance.
InitializingScoreTrend supports uniform/per-level declarations and derives upper
and lower bounds over lexicographic numeric scores. Unknown upper/lower levels use
symbolic infinity, represented by a finite prefix and its first infinite boundary;
no integer extrema or floating-point approximations substitute for infinity.
Ordered bounded phases use the derived upper bound in all six model/scoring paths.
The declaration remains a caller obligation, including optional choices, later
list insertions, constraints becoming unmatched and actual arithmetic/overflow.

Original public-API comparisons cover 512 configurations/cases: the 64 homogeneous
required graph fixtures, four exploration orders and ANY / ONLY_DOWN/ANY trends.
Both Java results and native ordered phases match independent Python optima with
assignment validation and score recomputation. Soft weights have both signs; pins
remain fixed. This establishes neither internal bound representation nor traversal,
list/mixed Java trend support, timing equivalence or additional adapted-method credit.
Raw inputs and results are in work/oracle/score-trend-cases.json; differential_solver
includes this check, and the dashboard displays its separate agreement count.


All eight pinned DiminishedReturnsTerminationTest methods now have complete native
adaptations, retaining every score, query and grace/reset assertion. Explicit
Duration inputs and native lifecycle calls replace Java nanosecond constants and
mock scopes; structural -1 represents one unassigned variable. Provenance records
the substitutions. Backward source timestamps remain in order with native monotonic
clamping. No internal ring-buffer storage tests are credited.

An independently implemented exact rational-rate policy integrates into all six
local-search paths and ordered phases. Sparse-history references, arithmetic
boundaries, injected clocks and delayed factory/handoff tests are original evidence.
Native multi-level reset rules, sticky expiry and late-grace behavior are documented
in the incremental-search contract; Java timed traces and a dedicated rate-policy
performance comparison are not claimed.


Pooled construction independently implements the pinned public cheapest-insertion
algorithm across native basic/list/mixed models and both scoring backends. Original
references, budget/clock checks and backend comparisons support its native contract.
PooledEntityPlacerTest remains pending because its mock lifecycle/copy assertions
are not adapted. A separate public-Java comparison retains 66 original graph cases
with independently validated greedy results and quality counts; it earns no
additional upstream-method credit. Java list/mixed pooled construction and dedicated
performance measurements remain outside that comparison.


Two scoreImproves methods from the pinned
UnimprovedTimeMillisSpentScoreDifferenceThresholdTerminationTest now have complete
native policy adaptations with every score, timestamp, termination query and
gradient assertion retained. Mock clock/scope plumbing becomes explicit observations;
provenance records the distinction. Construction suppression/shared lifecycle and
negative-millisecond constructor methods remain pending. The new phase-local native
limit uses exact differences, optional ignored score suffixes and a strict deadline;
original generated/clock/integration checks confer no additional method coverage.
Java boundary arithmetic, timed traces and dedicated timings are not claimed.

Value-first construction now cycles distinct basic values and selects their best
eligible receivers in both scoring backends, with component-ordered mixed integration.
Pinned Community public construction documentation supplies the behavioral specification.
QueuedValuePlacerTest was read, but its lifecycle/copy assertions remain pending;
no upstream methods or fixtures were adapted in this increment. Original native
regressions and 67 public-Java/Python graph comparisons establish separate evidence.
Final assignments and scores agree in all 67 cases, with 24 enumerated optima each.
Native heterogeneous, optional, mixed and budget semantics have original regressions;
the Java comparison covers homogeneous required basic variables only.

Dedicated basic-construction performance evidence is now produced separately by
scripts/benchmark_construction.py for sequential, pooled and cycling-value placement.
It validates all returned assignments and scores, checks each native result against
independent greedy reconstruction, and records small-case enumerated optima separately
from equality. Larger timing cases have independently justified planted optima. The
scope is full-scoring homogeneous required basic variables with pins; it does not
extend the earlier comparisons to Java list/mixed/optional or incremental-stream
construction. Original benchmark cases confer no adapted-method credit.

Native local search now supports borrowed nested All/Any expressions over completed
steps, stagnation steps, accepted moves, exact best scores and feasibility, across
all six model/backend paths. Original reference and integration regressions cover
these phase-local expressions. AndCompositeTerminationTest and
OrCompositeTerminationTest were inspected as behavioral context; their scope and
gradient assertions remain pending. No new upstream methods or fixtures were adapted,
and no Java trace or progress-gradient equivalence is claimed for this option.

One RuinRecreateMoveSelectorTest method, testRuiningNoAssignedValues, now preserves
all final-null and successful-solve assertions using native seeded basic repair in
both scoring backends. Three optional entities, three shared values, distinct initial
assignments, a group of all three and one accepted reconstruction retain the source
fixture. The assigned-entity penalty and solution-generator fixture are attributed
in provenance/notices. Native APIs replace Java configuration/descriptor plumbing;
no random trace, move equality, rebase, meter or lifecycle equivalence is claimed.
Original reference/budget/pin/cancellation/decimal regressions support the broader
native search contract without additional method credit. List/mixed repair, integrated
plan phases and dedicated Java performance comparisons remain pending.

ListRuinRecreateMoveSelectorTest.testRuiningOnlyOneAssignedValues now has a complete
native adaptation: one entity initially owns three optional values, one repair
samples all three, and the list-size penalty yields the asserted final size one.
The same assertions run in scoped and stream backends. Method, embedded scoring
and optional list solution-generator fixtures are attributed in provenance/notices.
The other list selector methods remain pending because their union selection,
metrics, timed lifecycle and fixtures are not fully adapted. Original list-repair
references and boundary tests earn no extra method credit. Mixed/plan integration
and dedicated public-Java quality/performance evidence remain future work.

Original bounded basic-repair comparisons now have their own runner and dashboard
section. Fixed half/whole groups and long step-counting threshold periods isolate a specified
number of repairs; public Java move counts and native complete-attempt counts are
checked for every solve. Independent assignment, score and enumerated/planted-optimum
checks remain separate from equal quality and timing. Sampling and reconstruction
orders differ. This adds no source-method coverage or Java trace, list/mixed/optional
repair equivalence. See docs/dashboard.md for scope and measurement boundaries.

The remaining three RuinRecreateMoveSelectorTest methods now preserve their
successful-solve and metric assertions using native ordered construction/repair
phases in scoped and stream backends. Five values/30 entities retain required cyclic
and optional first-null/rest-cyclic initial assignments. Required scoring penalizes
assigned entities; optional scoring preserves -1 per null and -1000 per ordered
collision pair. Required tests retain the 1000ms unimproved-time limit and full score
assertions. The optional 100-step cap becomes 100 complete reconstruction attempts;
no Java accepted-step or random trace parity is inferred. Public native work counts
replace Micrometer publication/registration, preserving all three metric inequalities
without claiming Java registry integration or identical numeric counters. Fixtures,
API substitutions and notices are recorded in provenance. This finishes the four
methods in that basic selector class, not the remaining list selector/lifecycle work.

The remaining three ListRuinRecreateMoveSelectorTest methods are now adapted in
both scoring backends. Required cases preserve ten generated values, three empty
entities, the generated-name Java hash penalty, construction followed by repair,
full score diagnostics and 1000ms unimproved termination. Public Java Objects/String
calls independently verify all 300 fixture penalties. Native score/repair counters
preserve metric inequalities while replacing Micrometer integration. The optional
case preserves all three scoring terms and the 40/40/20 change/swap/repair union
through shared native weighted search. Its 100 completed-step cap is represented by
100 selected native attempts, including rejected/unchanged attempts; random traces
and acceptance histories are not claimed equivalent. Provenance records all fixture
and API substitutions. UnionMoveSelectorTest lifecycle assertions remain pending.

Original required/optional list-repair benchmarks now independently check validity,
quality and actual work against the pinned public Java API. Group ranges begin at
one for eight-repair timing comparisons. Separate fixed-minimum probes expose early
Java termination after optional omissions, while native repair can include missing
values and finish the requested work. Equality is not presumed, and timing cases
with differing quality remain labelled. These original cases add no upstream-method
credit; methodology, scope and exclusions are documented in docs/dashboard.md.

All ten MassListChangeMoveProviderTest methods now have native adaptations. Their
required/optional, missing-value, ranged-receiver, prefix-pinned and fully pinned
fixtures, sample sizes, stream limits and assertions are retained. Native assigned-
only group enumeration, immutable sample/destination metadata and typed errors
replace Java sampled streams, metamodels and constructor exceptions. No Java random
probabilities, infinite-stream order, lifecycle or internal move identity is claimed.
Original exhaustive successor checks cover 4,455 model/configuration combinations;
additional laziness, optional behavior and four-path plan validation checks add no
method credit. General gather behavior and its earlier evidence remain unchanged.

All ten MassChangeMoveProviderTest methods now have native adaptations, retaining
all sample sizes, stream limits, fixtures and assertions. Arbitrary basic slots
may have different starting values; native metadata exposes the full selected set
and common destination. Finite subsets and legal targets replace sampled streams;
no probability or Java metamodel/lifecycle equivalence is claimed. Independent
bitmask checks cover 20,146 model/configuration combinations; these, lazy subset
checks and search/early-validation integrations add no upstream-method credit.

All three MassAssignMoveProviderTest and four MassUnassignMoveProviderTest methods
now have native adaptations. Group assignment selects only missing unpinned slots;
unassignment selects only assigned unpinned slots and accepts singleton groups.
The singleton method retains its exact fifty-move assertion through bounded seeded
sampling with replacement. Its generated fixture keeps the first entity missing.
Other methods use finite enumeration, with original stream limits and assertions.
No Java sampling distribution or lifecycle claim. Independent checks cover 10,829
model/size/mode successor and metadata comparisons, sampled reproducibility,
large lazy groups, required partial inputs, search and validation before factories.

All five MassListAssignMoveProviderTest and five MassListUnassignMoveProviderTest
methods now have complete native adaptations. Optional-list requirements, missing
versus assigned sources, receiving ranges, prefixes and whole-entity pins are
preserved. The singleton method still requires fifty moves, provided by a bounded
seeded sampler. Finite enumeration serves the other methods; no Java randomness,
metamodel or lifecycle parity is claimed. Original successor/metadata references
cover 2,268 combinations of layouts, pin modes, ranges and sample sizes. They and
seeded sampling, lazy groups, search, early-validation and basic-fallback checks
earn no additional upstream-method credit.


General single-entity cyclic k-opt now has native edge reconnection and lazy
bounded neighborhoods, exercised in list and mixed scoped/stream search. One
upstream feasibility method is adapted. Eight additional attributed ordered-output
fixtures intentionally earn no method credit: their Java notification assertions
remain pending, along with Java's implicit multi-entity partitioning, rebase and orientation.
Original exhaustive permutation checks cover 2,586 successor states, repeated with
pin/range models and independently checked edge-based construction. See the list
planning contract for scope, ordering and combinatorial cost.

The cyclic k-opt benchmark adds original single-route Manhattan-cycle cases and
separate public Java/Rust adapters. Both configure k ranges 2–3 or 2–4, seeded
first-accepted hill climbing, 30 steps and 10,000 score evaluations. Native exact
edge-difference enumeration uses a rolling shuffle window of 4,096 candidates;
Java uses its public random KOptListMoveSelectorConfig. Tests establish that the
first native window reaches every configured k at the measured sizes. Search work,
PRNGs, anchor conventions and equivalent-move handling differ. Every result is
independently validated; 64 small cases have exhaustive cyclic optima. Four larger
workloads report final quality and score calls alongside times. Exact-3/exact-4
probes are kept separate because pinned Java returned after only its initial score
calculation. No elapsed-time ratio is reported for those probes. These original
comparisons add no adapted-method credit and establish no multi-entity k-opt parity.

Optional bounded native k-opt sampling now avoids building an exhaustive move
window. Six original regressions check independent edge/successor sets, exact-k
membership, small impossible cases, reproducibility, bounded attempt prefixes,
65-edge samples, pins/ranges/missing identities and per-step seed integration.
Existing list/mixed scoped/stream plateau tests and basic fallback controls cover
both exhaustive and sampled neighborhoods. No upstream method credit is added.
The k-opt comparison retains the window baseline and adds sampled runs against
the same Java configurations: 128 paired small-case comparisons (64 per native
strategy), eight timing workloads and the four separate exact-k Java probes.

Native combined-cycle k-opt now supports explicit repartitioning across selected
entities, and bounded sampling of both reconnections and destination lengths in
list/mixed search. Prefixes, receiving ranges, missing identities and rollback
remain enforced. Two additional upstream output fixtures are attributed without
coverage credit: their explicit native partitions do not adapt Java's implicit
partition/orientation choice or notification assertions. Nine original tests and
independent small successor references add no upstream-method credit. Public Java
multi-entity solving comparisons and implicit-boundary compatibility remain pending;
the existing performance dashboard still measures single-route k-opt only.

All twelve AndCompositeTerminationTest/OrCompositeTerminationTest methods now have
native adaptations preserving every boolean/gradient assertion and phase-applicability
case. Native evaluators replace Java mocks and solver/phase scope objects; the
shared applicability helper is attributed. This establishes expression behavior,
not Java lifecycle or object API equivalence. Independent six-path orchestration
and controlled-clock checks cover the new solve-wide count/time expressions.

Weighted neighborhood unions now compose existing move families in basic, list and
mixed local search with both scoped and stream scoring. Sequential selection drains
children in order; seeded random selection draws a child per move in proportion to
positive integer weights. Zero weights disable children, including validation.
Swap-only neighborhoods allow changes and swaps to be weighted independently.
Unions preserve each child's order and duplicates and exhaust finite children once;
consumer budgets bound infinite generic iterators. Seeds vary per search step.
Two UniformRandomUnionMoveIterator methods retain their full observable assertions.
The Java polling/priming method remains pending: native unions deliberately do no
polling at construction and Rust iterators have no separate hasNext protocol.
Original regressions exercise weight proportions, laziness, exhaustion, disabled
validation, all six solver paths and escaping a single-move local optimum using
combined single/group changes. No Java lifecycle/PRNG trace parity is claimed.

Independent-destination basic change products now enumerate bounded variable subsets
and all combinations of their alternative values, preserving pins, domains and null
optionality. Each selected slot changes, so redundant writes do not duplicate
successor states. Neighborhood::ChangeProduct works with both basic scoring backends,
weighted unions, mixed joint list moves and existing search budgets/order policies.
MoveProduct exposes the underlying lazy generic product with restartable independent
child factories, last-child-fastest ordering and explicit empty-child policy. It
retains only current child items/iterators; input sequences are not materialized.
Four CartesianProductMoveSelector test helpers supply attributed partial enumeration
fixtures, which earn no method credit by themselves. Independent-product lifecycle,
size and random contracts are completed by the later SelectionProduct adaptation
below; original dependent mimic behavior is adapted separately, while random mimic
contracts remain pending. Original state-space and
solver-trap checks establish native behavior separately.

Custom MoveProvider implementations now supply candidates to six local-search
entry points (basic/list/mixed, scoped/stream scoring). Providers receive a read-only
committed snapshot and completed-step index and may return lazy borrowed iterators
or use the Vec-returning closure adapter. They replace built-in neighborhoods/unions
and mixed joint generation; candidate shuffling and existing acceptance, tabu,
foraging, score checks and termination remain in the shared engine. Model/source
prevalidation still runs with zero budgets. Invalid candidates retain typed errors.
Generation is cooperative and unscored; a bounded window may prefetch candidates.
PickGreatestValueMoveProviderTest and SwapNearbyValuesMoveProviderTest assertions
are fully adapted with native slot/value identities and explicit range preservation.
Original checks cover snapshot progression, infinite iterators/budgets/prefetch,
prestart cancellation, early validation and invalid proposals across all six paths.
Provider bindings now also attach to local-search phases in complete SolverSequence
and MixedSolverSequence plans. SolverExtensions combines those bindings with custom
commands and progress/initialization events across all six scoring paths. Bindings
use full-plan indices; missing or non-local targets fail before callbacks or scoring.
Unbound phases retain built-in validation and neighborhoods. Shared limits, retained
best handoffs and cancellation use the existing plan controls. Six original full-plan
regressions cover composition and stopping behavior; no additional upstream-method
credit is claimed for this integration.

Configurable sublist neighborhoods add independently bounded swap sides, minimum
relocation/unassignment lengths, reversal controls and model-aware unassignment.
SubListChange, SubListSwap and SubListUnassign integrate with list/mixed scoped and
stream search and weighted unions. Generators are lazy and deterministic, preserve
pins and destination ranges, omit forward relocation no-ops and symmetric swap
repeats, and never reverse a singleton/singleton swap. Twenty-one Community provider
methods retain their observable assertions using native configuration and move
metadata. Random distributions/lifecycle are not established. The default-maximum
method remains pending: native defaults have no implicit sampling cap. Existing
max-length-only sublist APIs retain their original enumeration contracts.

Explicit Assign and Unassign neighborhoods now select only missing or assigned
optional basic slots, respectively. PillarChange applies configurable null crossing
to whole groups of at least two members; PillarUnassign clears assigned groups,
including singletons. All four preserve pins and entity-dependent ranges and work
with basic/mixed scoped and stream search. An attached PillarSchema retains tuple
homogeneity and changes one selected column at a time. Twenty-three Community
provider methods retain their membership, validation, homogeneity and rollback
assertions. Enumeration is finite and deterministic; Java random traces and provider
retirement mechanics are not replicated. The retirement regression retains its
observable distinct legal move set. Existing general pillar APIs remain unchanged.

Reusable neighborhood sampling now supports fixed sizes, inclusive random bounds,
source-draining rules, keyed resets and custom accept/reject/stop decisions.
NeighborhoodSample owns distinct members, preserves encounter order for iteration,
and compares/hashes without regard to order. Option represents nullable members.
The assembler resets before validation/evaluation, reports explicit premature stops
as errors, and returns None on source exhaustion below the declared minimum.
Target size is a hint/contract; all-with-expected-size still drains the source.
No hint-sized allocation or lookahead after a stop is performed. Accepted duplicates
are ignored before evaluation; custom rejecting/draining rules require a bounded
source when termination is required. Native seeded randomness is repeatable but
neither Java PRNG traces nor unbiased member selection from ordered sources is claimed.
Twenty-four SamplersTest and nine SampleTest methods retain all assertions using
native APIs. Java mutable-alias wrap methods remain pending. Original checks cover
boundary sizes, duplicate consumption, keyed lifecycle, rejection bounds, stop-only
semantics and sampled composite providers in scoped and stream local search.

Neighborhood datasets now provide immutable shared snapshots, identity-indexed input
stores, seeded exhaustive permutations, with-replacement draws and lazy repeated
sampling. Repeated sampling opens a fresh source per attempt, yields sampler errors
once, and permanently ends on an absent sample. Filtered joins exhaust their right
source as needed, so rare matching members are not dropped by a retry limit.
Materialized joins index per-left slices; explicit refresh from new source snapshots
replaces all indexes atomically. Store batches are likewise atomic, and held samples,
dataset snapshots and owned sampling iterators remain unchanged by later updates.
Native ownership requires immutable fact values; cloning shared mutable interiors
is not an isolation guarantee. Java session/settle is represented by explicit tuple
change propagation and cache refresh, not automatic reflective synchronization.
Thirteen SamplingIteratorTest, six DatasetTest and four PillarDatasetTest methods
retain their assertions. The Java random iterator's unsupported forEachRemaining
exception remains pending; native random iterators use normal Rust consumers and
therefore require a bound. Six original regressions check lazy/fused consumption,
repeatability, duplicate identities, rollback on panics, cache refresh and sampled
custom providers in both scoped and stream local search.

StreamGraphBuilder.dataset now captures any typed stream port as a live incremental
terminal. Repeated capture of the same port shares one node; distinct ports remain
separate. Captured rows follow transactions, rollback, saved-candidate installation
and committed basic/list/mixed score-director moves. Dataset branches remain live
when adjacent weighted scoring constraints are disabled. Consistency diagnostics
rebuild and compare complete captured rows, including graphs without scores.
StreamGraph.dataset_rows exposes persistent snapshots; dataset_snapshot materializes
owned values for neighborhood sampling. Capturing data does not count as a scoring
callback. Captured payload equality must be stable/reflexive, and facts immutable.
Unlike the standalone snapshot join cache, graph captures need no manual refresh
once source changes are delivered. Existing MoveProvider inputs remain unchanged;
score-director graph access exposes these datasets to callers managing search steps.
Eight UniEnumeratingStreamTest and five BiEnumeratingStreamTest methods preserve
concatenation (including self-duplicates and padding), grouping, custom collector,
reassignment/retraction and mapper-call assertions. The twelve uni enumeration
methods involving source discovery, optional null rows and pin filtering are
covered by the typed views below. Seven original checks cover sharing/foreign handles, disabled branches,
failures/rebuild diagnostics, held snapshots, saved candidates and all three stream
score-director model families.

Typed EnumeratedDataset views now optionally prepend one permanent null member and
filter pinned rows before capture. Native basic/list entity and list-value adapters
bind those views to existing score-director sources. Unassigned entities/registered
values remain present; a null member is distinct from an entity with a null value.
Whole list pins exclude entities, while prefix pins exclude only the corresponding
values. Borrowed iteration uses ascending native identity order without cloning;
owned snapshots support existing sampling and joins. Null is a dataset-view member,
not a scoring-graph tuple. Native problem membership/pin metadata is fixed during a
session; custom streams can explicitly insert/retract/update typed payloads. This
does not add Java reflection or arbitrary dynamic model mutation. All twelve remaining
UniEnumeratingStreamTest enumeration methods retain their full ordered assertions.
Original regressions cover null/duplicate distinctions, callback-free reads, foreign
graphs, pin updates, panics/rollback, sampling and basic/list/mixed director moves.

BoundedFilter now distinguishes a retryable per-probe budget miss from permanent
source exhaustion. RetiringRandom draws live occurrences with replacement and
removes only the latest drawn occurrence on explicit retirement, including sparse
slots and duplicate/null payloads. RetiringPairs selects a live left occurrence,
retries a caller-supplied right probe up to an explicit nonzero budget, and retires
that left after repeated failure. Productive lefts remain available; consumers must
bound potentially infinite productive streams. Empty/all-rejected pools terminate
within left-count times attempt-count callback calls. Callbacks must themselves
finish promptly. Bounded probes can miss rare matches; exhaustive scans remain
available when necessary. Left-row fairness is distinct from uniform pair sampling.
Seven retiring-iterator, four filtering, two retiring-walk and three paired-iterator
liveness methods retain their assertions. Native attempt configuration replaces the
upstream symbolic retry constant; its Java value and PRNG traces are not claimed.
Six original checks cover exact consumption, duplicates/holes, demand-driven clones,
seeded distributions, bounded dead-pool work and provider solving in both backends.

List insertion destinations now have direct lazy enumeration and live captured
dataset forms. Each unpinned entity contributes its insertion slots at/after the
pinned prefix, including the end slot; an optional-list view can prepend exactly
one unassignment member. Ranges do not multiply/filter value-independent slots;
providers must check their own receiving values and adjust same-list destinations
after source removal. The stream form follows ordinary director trials/rollback.
ProviderUnion composes borrowed custom providers with sequential or weighted seeded
selection. Factories open only when selected, disabled children remain inactive,
and each step gets fresh child cursors over its committed snapshot. Nested unions
and duplicates are preserved; empty provider lists are rejected. The three
DefaultMoveStreamFactoryTest and two DefaultNeighborhoodTest methods are adapted.
Six original checks cover destination states/validation, lazy factories and cursor
drop, disabled/nested providers, seeded weights and all six solver backend paths.

Value-label exchanges now operate over bounded contiguous windows or arbitrary
subsets in model/custom variable order. Unused domain values participate, optional
unassignment is explicit, and touched pins or disallowed destinations reject the
whole candidate. Unchanged slots remain part of selection bounds. Empty exchanges
are omitted; single writes are plain Change moves. Enumeration clones only yielded
writes and can be supplied through the existing custom-provider search API.
Three preview example methods now retain their exact move membership/exclusion
assertions: value swaps within a code range, restricted-range block relocation,
and same-cut tail swaps. Native ordered slots/ranges replace Java metadata and
custom move objects; full example-provider/API equivalence is not claimed. Five
original regressions compare independent small-case successor sets and rollback,
order validation, pins, clone counts, and escape from a single-change local optimum
in both scoped and stream search. They earn no additional upstream method credit.

ListMoveBuilder now constructs compound list moves using a validated isolated draft.
It supports assignment, ordered batch insertion, assignment with displacement,
replacement of an existing element, signed shifts, block relocation/exchange and
unassignment. Each edit is atomic; cloning forks the draft and finish returns a
native move usable through either scoring backend and existing providers. Full
snapshot editing is a correctness baseline, without an incremental cost claim.
Fifteen MoveDirectorTest assignment methods preserve all list/position/unassignment,
already-assigned rejection and rollback assertions. Java descriptors and ephemeral
directors are replaced by draft construction plus stream-director replay/rollback.
Notification-specific replacement/shift methods remain pending. Four original
regressions cover exhaustive small indices, failed/panicking edits, draft forks,
compound score consistency and local-search solving in both scoring backends.

Ordinary empty and fully pinned solves now score their initial state without
entering configured phases, commands or providers, across all six scoring paths.
Explicit exact-search plans retain their phase/exhaustion/optimality contract. Mixed
models require both components to be frozen; prefix-pinned lists remain movable.
Initial events, work counts, global limits and initialization errors are explicit;
incomplete frozen lists validate bindings and report NoCompleteAssignment before
budget gates, without scoring or claiming a complete/optimal solution. Three
DefaultSolverTest empty/pinned methods retain every
assertion and helper-required event check, mapping solvingStarted to native Initial.
Nine original checks exercise model/backend coverage and lifecycle boundaries.
See solver-contract.md for validation, limits and event semantics.

Four RandomSubListSelectorTest methods now retain all span sequences, counts,
prefix/unassigned fixtures and injected random bounds. The native index orders
spans by entity, descending length and ascending start; repeated draws retain the
source snapshot. Original checks cover multiple entities, exhaustive small rank
spaces, large catalogs without value cloning, sampled relocation/exchange validity,
rollback and provider solving with both list scoring backends. Native random
sampling is uniform over eligible spans; multi-entity Java sampling probabilities
and PRNG traces are not claimed. Empty-selection assertions are adapted as a
fixture but that upstream method remains pending because Java never-ending metadata
has no direct native property. Constructor-message and lifecycle methods remain
pending. This batch adds no new performance measurements.

Thirty-four indexed value-range methods now retain integer/long boundary fixtures,
explicit list/composite/optional/boolean values, counts, membership, exact iteration,
injected random draws and supported sorting assertions. Native signed i64 arithmetic
covers both int and long fixtures with their distinct original limits. None represents
missing values and out-of-range lookup; negative indices are unrepresentable. Native
stable copies and captured comparators replace sorter/factory wrappers. Unsupported
sort and negative-index exception methods remain pending. Huge lazy ranges support
lookup/sampling; variable_from_range explicitly materializes under a caller
limit because the current solver still stores finite domains. Seven original checks
cover overflow, wide double-ended skipping, rejected allocations, nested optional
layers, stable snapshots, occurrence sampling and actual exact solving.

Nine SampleValueRangesTest methods and two SampleValueRangesBiasIT methods now
preserve membership across every selected domain, absence/exclusion rules,
shared-range single lookup, identity-set equality, overflow-safe probe budgeting,
exact empty-intersection results and random target validity. Both bias methods
retain both parameter sets, 200000 trials per set and the original 5-sigma
binomial threshold using root-seeded native randomness; Java traces are not
compared. Six original regressions add exhaustive small intersections, operation
budgets, exact fallback, stable identities, pin/no-op validation and group-change
provider solving through both basic scoring backends. Scan exhaustion is a distinct
error, never an empty-intersection claim. No new performance result is claimed.

Ten BigIntegerValueRangeTest/BigDecimalValueRangeTest methods now retain all default
and explicit step fixtures, counts, exact indexed values, membership, complete and
prefix-only traversal, and scripted random draws. Decimal comparisons retain every
asserted scale. Native BigIntegerValueRange and DecimalValueRange integrate with
indexed composition, optional ranges, common-destination selection and bounded
finite-domain conversion. Seven original regressions cover huge offsets/steps,
u128 count overflow, nondividing steps, extreme scales and an exact decimal solve.
Decimal scales must match explicitly in this native API; unsupported-sort exception
methods and broader Java scale/constructor-validation parity remain pending.

Twenty-one TemporalValueRangeTest methods now retain local date/date-time/year
bounds, units and strides, counts, indexed values, membership, exact iterators,
scripted random prefixes, extreme ranges and invalid/alignment cases. Native civil
values use the proleptic Gregorian calendar including year zero; month/year values
are anchored to the original start and clamp at month end without cumulative drift.
Temporal ends must be exactly reachable by whole positive steps. Seven original
regressions cover complete positive/negative leap cycles, clock precision, huge
nanosecond ranges, invalid inputs/overflow, range intersection and calendar-domain
solving. Java mock-unit, negative-index and unsupported-sort methods remain pending.
Timezones, daylight-saving transitions and Java temporal API parity are not claimed.
Indexed ranges stay lazy; solver domains still use bounded explicit materialization.
No performance comparison is added by this batch.

Four single/distinct cascading-shadow methods now preserve complete asserted values
and callback counts, including preseeded early stopping and independent propagation
of two cascades. Native explicit invalidation/imported shadows substitute for the
Java partial owner/previous notifications; reflection validation remains pending.
ListCascadeCache detects owner/index/neighbor changes, evaluates in list order,
and stops each cascade when its result stays equal. Native list-move trials restore
both lists and derived values unless committed. Structural validation/diffing still
inspects the whole assignment; only callback propagation is selective. Six original
regressions cover every small partial assignment, native moves/undo, explicit facts,
pins/errors/unwinding, non-Clone snapshots, grouped fields and an arrival-time solve.
This adds no Java lifecycle or new performance equivalence claim.

All eight NodeTopologicalOrderQueueTest methods now retain order, subset selection,
deduplication/reoffering, capacity workload and equal-rank assertions. The randomized
priority-reference check preserves workload sizes with native seeded draws and
node-ID tie breaking. NodeOrderQueue captures shared immutable ranks and stores only
offered work. DependencyCache now uses this queue, caching rank snapshots with each
topology plan; value-only invalidation does not scan the node universe. Existing
exhaustive dependency regressions and new fan-in/edge-reversal/cycle-recovery tests
verify integration. No new timing or Java allocation-strategy equivalence is claimed.

WeightedSelection adds immutable fractional occurrence weights, disabled zero entries,
bounded candidate collection, seeded/injected draws, and reweighting that shares
non-Clone items while retaining old distributions. Power-of-two rescaling handles
large and subnormal totals; invalid weights/draws and positive weights lost to
floating-point precision produce explicit errors. This remains f64 sampling on a
native 53-bit draw grid. Provider tests use weighted proposals through both basic
scoring backends. ProbabilityEntitySelectorTest.getSize earns full method credit;
the immutable sampler file retains partial entity/move sequence fixtures, now
completed by the lifecycle adaptations below. Seven original regressions cover
intervals, numeric extremes, bounds, snapshots, cloning, distribution and solving.
No new performance or Java lifecycle equivalence is claimed.

Twenty selection-cache methods now retain solve/phase/step rebuild counts, complete
ordered traversals, scripted uniform/weighted draws, size/repeating metadata,
constructor/list-iteration errors and all child lifecycle forwarding assertions.
SelectionCache explicitly receives native lifecycle events; it captures candidates
and weights at its configured boundary and releases them at scope end. Held
snapshots survive release. Failed builds leave the active scope without a cache;
callers must end that scope before retrying. Source callback side effects cannot
be rolled back. Six original regressions cover changing contexts, invalid events,
collection bounds, unwinding, non-Clone snapshots and step-scoped move-provider
solving with both basic backends. Parent lifecycle delivery remains explicit;
automatic global solver-listener wiring and Java runtime class identity are not
claimed. No benchmark refresh or new performance comparison accompanies this batch.

Thirteen sorting/shuffling selector methods now preserve ordered fixtures,
exhaustion, size/finite metadata, cache rejection, all child lifecycle counts and
exact source rebuild counts. OrderedSelectionCache sorts stably once per source
cache boundary, or shuffles each step without replacement, retaining the prior
permutation until that source cache rebuilds. Immutable indexed snapshots share
non-Clone candidates and survive future reorders/releases. Six original regressions
cover stable ties/context capture, every draw combination through six items,
invalid events/draws and comparator unwinding, duplicate occurrences/lazy cloning,
seeded repeatability/bounds and sorted/shuffled providers in both basic backends.
Native lifecycle events remain explicit. SortingMoveSelectorTest.applySorting
remains pending because Java comparator-class/factory configuration is not adapted.
No Java PRNG trace, automatic parent lifecycle, or new performance claim is made.

Twenty-one filtering/count-limit selector methods now retain ordered accepted
prefixes, exhaustion, finite metadata, lifecycle forwarding and exact child
open/size counts, including entity-dependent values and cached filtering.
FilteredSelectionSource reports unknown exact size and an explicit child upper
bound; a surrounding SelectionCache knows the materialized match count.
LimitedSelectionSource composes before filtering to bound inspected work or after
filtering to bound accepted items. A post-filter limit alone cannot bound an
endlessly rejected source. FilteredMoveProvider supplies separate total scan and
selection limits per step, lazy factory opening and inspected/rejected/yielded
counts with distinct stop reasons. Six original regressions cover empty budgets,
early drop, per-step context/count reset, exhaustion, cache refresh, composition
and actual one-scored-move solving in both basic backends. Existing BoundedFilter
continues to provide retryable probes. Java termination polling cadence, ordered
list-iterator forwarding and random ending-iterator behavior remain pending.
No benchmark or automatic Java lifecycle equivalence is claimed.

Both entity/value mimic-replay original-selection methods now retain every
record/replay availability and selection assertion, temporary/final exhaustion,
size/finite metadata, lifecycle counts and child iterator openings.
RecordingSelection publishes shared non-Clone items to independent replay cursors.
Availability prefetches one Rust item; replay cannot consume it until recording
next publishes it. Each reader consumes each publication once; slow readers see
the latest choice rather than a backlog. Opening a new recording or delivering a
lifecycle event invalidates old cursors, with explicit stale/not-recorded errors.
One owner forwards lifecycle events; replay views observe shared invalidation.
Six original regressions cover passive probes, multiple readers/duplicates,
ownership and scope invalidation, abandoned lookahead, source unwinding and replay-
coordinated composite moves in both basic backends. Channels are thread-local and
parent lifecycle remains explicit. No Java PRNG, threading or performance claim
is introduced.

Twenty independent Cartesian-product selector methods now preserve all ordered
and scripted random sequences, zero/empty-child policies, cardinality and every
child lifecycle count. This completes the behavior left partial in the earlier
MoveProduct enumeration fixtures. SelectionProduct bounds finite child collection
and returns immutable shared occurrence pools; combinations stay lazy, with u128
cardinality/indexing and explicit overflow errors. Random rows independently sample
each nonempty child, retaining original child indices in injected draw callbacks.
Six original regressions cover exhaustive small products, enormous indexed counts,
collection/metadata failures, non-Clone sharing, seeded/scripted sampling and
composite-move solving in both basic backends. Captured child pools are independent;
context-dependent suffixes are supported by DependentSelection below. The two
random mimic-product methods remain pending.
No Java PRNG trace or new performance comparison is claimed.

The two original-order mimic-product methods now preserve both complete twelve-row
sequences, finite exhaustion and all three source lifecycle counts across two steps.
DependentSelection exposes prefix-aware factories and a lifecycle hook. Its lazy
traversal backtracks after empty suffixes, preserves omitted level positions and
bounds total source polls, reporting exhausted versus work-limited termination.
Shared non-Clone prefix items may be retained by returned suffix cursors or rows.
Conditional sampling uses the current prefix and bounded child collection; an empty
sample rejects only that attempted path, and conditional uniform choices are not
necessarily uniform over complete rows. Six original regressions cover recursive
reference trees, empty-branch recovery, infinite rejection bounds, ownership,
conditional sampling/errors and correlated composite moves in both basic backends.
The upstream random mimic fixture includes a stale secondary entity on one row
(b->1+a->9). These two methods remain pending; native conditional sampling always
uses its current prefix and does not claim that legacy interleaving or size24.
No Java factory/threading/PRNG or new performance equivalence is claimed.

Five ReachableValues methods now preserve shared-range owner membership, co-value
sets, optional unassignment and ascending/descending views. Native immutable
ReachableValues indexes use ordered value/entity identities and retain every owner
when equal domains share storage. Basic and list adapters capture structural ranges;
providers still enforce pins, current assignment and list position legality. Six
original regressions compare all 4,096 small domain/optionality combinations with
direct scans, exercise shared snapshot ownership and use the indexes to generate
candidates in both scoring backends for basic and list solving. One entity-membership
fixture is partial because Java runtime type discrimination is omitted; multipleSorters
identity/cache behavior also remains pending. No dynamic domain update or new
performance comparison is claimed.

Six UnionMoveSelector methods now retain finite concatenation, weighted/uniform
interleaving, empty behavior, size/exhaustion and child lifecycle forwarding.
SelectionUnion forwards explicit events and lazily opens ordered sources;
WeightedInterleave mixes finite/infinite child streams with fractional weights,
one buffered item per enabled child and permanent exhaustion retirement. Zero
weights disable opening/polling; events still reach every child. Random family
weights do not change internal child order. Invalid weights/draws are explicit
errors; consumers must bound infinite streams. Six original regressions check
occurrence/order preservation, fractional boundaries, non-Clone ownership, disabled
and non-fused sources, lifecycle/size/early-drop behavior, seeded repeatability and
basic solving through both backends. Nearby mixed-model configuration and Java
PRNG traces remain unclaimed. Existing integer MoveUnion/ProviderUnion paths are
unchanged; no new benchmark comparison is claimed.

Six iterable entity-range/filtering-range selector methods now preserve every
sorted candidate sequence across five steps/two phases at solve, phase and step
cache scopes. ReachabilityCache composes the existing lifecycle/collection cache
with an inverse-domain index; BasicRangeSource and ListRangeSource supply native
model ranges. All/entity/co-value queries produce independent immutable finite
views with explicit random sampling available on the returned WeightedSelection.
Full source cardinality remains separate from a filtered view's length. Failed
builds expose no old index; externally held snapshots survive release/rebuild.
Six original regressions exercise actual scope refreshes, duplicate/error/panic
recovery, bounded collection, optionality/shared domains, native model changes,
seeded view sampling and basic solving in both backends. Java manual mimic state
is replaced by explicit current-origin queries. Pins/positions and dynamic model
changes still require normal provider checks and an appropriate cache scope.
No new performance comparison is claimed.

Four reinitialization/unassigned-list selector methods now preserve all target-only
initialization checks, finite values and sizes, lifecycle forwarding, child call
bounds and rejection of repeating children for a finite snapshot. GatedSelectionSource
suppresses child size/open calls when its context predicate is false. Validated
BasicSelectionContext/ListSelectionContext expose current assignments; native
sources select all/movable/uninitialized basic entities, ordered basic domain
values (including optional None), and all/assigned/unassigned/movable list values.
Uninitialized means required, unassigned and unpinned; optional None is complete.
Whole-list/prefix pins affect movable list values. Six original regressions check
current-state changes, invalid contexts, disabled children and initialization via
scoring transactions in both backends for basic/list models, including structural
penalties until completion. Local search still requires complete initial assignments;
these sources do not add a generic construction-provider entry point or automatic
solver lifecycle wiring. Revision-aware entity-list caching remains separate work.
No new performance comparison is claimed.

Ten entity-selector methods now preserve ordered/random sequences, finite/repeating
metadata, indexed-list restrictions and exact extraction counts across phases and
revision changes. RevisionedSelection captures a finite phase snapshot under a
collection bound and optionally checks an Eq revision at each step start. Equal
revisions reuse the snapshot; each phase start always extracts again. Callers must
change revisions for every fact affecting candidates. Step/just-in-time source
fixtures map to native step-boundary checks; phase policy retains its phase snapshot.
Failed or panicking builds publish no current snapshot, while held non-Clone
snapshots remain valid. Six original regressions cover refresh policy, mismatch/
limit/panic recovery, invalid transitions, empty and unbounded sources, passive
random availability, invalid draws, indexed views, snapshot ownership and both
basic scoring backends. No automatic model revision counter, Java PRNG trace parity
or new timing comparison is claimed.

Custom move providers can now construct basic, list and mixed assignments with
both scoring backends, including atomic multi-item initialization, preservation of
existing assignments, bounded scans and useful partial results. This is native
integration coverage only: no additional pinned methods or Java construction-trace
parity are claimed. The contract is in `incremental-search-contract.md`.

Full solver plans now support construction-provider bindings across all six
model/scoring paths, retaining shared controls, events and partial handoffs. This
is additional native integration evidence, not new upstream method coverage.

Five complete queued-entity, queued-value and pooled-placement methods now preserve
all ordered candidate pools, finite/cyclic behavior, phase restarts and lifecycle
counts. Native PlacementQueue/PlacementSource and immutable snapshots replace Java
mimic selectors and runtime move casts. Factory copy methods remain pending. The
native construction-provider integration is tested separately; automatic source
lifecycle dispatch is not claimed.

Three unique-random iterator methods and two repeating-random methods now preserve
all occurrence membership and exhaustion assertions. Unique sampling is lazy over
u128 indices and integrates with native construction providers. Java PRNG traces
are not matched; remove/unsupported bulk-drain exception tests remain pending.

Two additional `AbstractUniConstraintStreamTest` unique-pair methods preserve
B/A/C insertion, ID-oriented exact pair sets, unit penalties, total scores and
an incremental equality-key change from AB to BC. Native IDs replace Java
planning-ID discovery. Unused fixture groups/values are omitted, all source
entities are assigned, and native stream filtering remains explicit. Indexed
updates, shared graph branches, rollback, callback failures and composite
planning moves have original regressions with no additional method credit.

Eighteen additional unary/binary grouping methods preserve one through four keys,
zero through three collectors, complete match fields/sets, count-based penalties,
filtered counts, MAX/MIN extrema, duplicate-group survival, removals and key changes.
Binary cases use live unique-pair streams. Flat native tuple collectors and
DistinctCountCollector replace Java arity overloads; integer count widths are
native usize. Generated group/value membership and entity codes remain exact;
unused value-group facts, reflection metadata and random unused properties are
omitted. Five original regressions cover four-field and nested composition,
non-Clone inputs, duplicate/null counting, specialized arithmetic, atomic failure
and 500 incremental graph trials without additional upstream method credit.

Twenty additional three-/four-fact grouping methods now exercise actual chained
joins over explicit entity/group/value sources, unique-pair self-reference joins,
distinct-third-entity filtering and extra same-group joins. They preserve every
source match assertion, count/extremum/set aggregate, penalty and specified removal.
The existing Rust graph supports these combinations without an engine change.
Match comparison follows the source helper's score/length/containsAll semantics:
justification positions and duplicate-field multiplicities are not asserted by that
helper. This distinction matters for the three-fact four-key and four-fact three-key
no-collector fixtures; their literal expected fields are retained. Native code IDs
represent generated entity codes bijectively; group/value fact IDs remain typed.
These are additional composition coverage, not a Java reflection/event-order claim.

Eighteen additional node-sharing methods preserve unary through quaternary filter
identity assertions, plus unary mapping/flatten-last identity assertions. Native
reusable definitions replace Java callback object identity; canonical dataset
handles verify graph node identity. Base stream topology retains Cartesian joins,
true/false predicates, identity/None mappings and empty list/set expansions. Ordinary
builder operations are not automatically interned. Runtime regressions separately
verify one callback per incoming change across consumers, rollback, duplicate
expansions, different definitions/parents/builders and release of graph state.

Forty-five additional node-sharing methods cover joins from unary/binary/ternary
parents and existence/absence from all four arities. They retain changed-left/right
parent assertions, different/same indexers and filters, no-joiner reuse, scalar
code/toString equivalence and first-vs-second fact projections. Native reusable
definitions and canonical dataset handles express callback and node identities;
ordinary builder calls remain independent. Original generated runtime comparisons
cover ordered relations, filters, right-side changes, rollback, self-exclusion and
callback counts without adding upstream method credit.

Twenty-four additional grouping node-sharing methods preserve all four arities'
parent/key/collector identity assertions, including the Quad fixture's different
callbacks that both return zero. Native reusable definitions and same_node replace
Java callback and stream identity. Count/distinct-count and list/distinct-count
remain distinct collector implementations. Original runtime regressions separately
cover grouped tuple collectors, global sums, distinct keys, ordered lists, updates,
empty removal, snapshot retention and rollback; they add no upstream method credit.

Fifty-seven additional node-sharing methods preserve unary/binary/ternary expansion,
binary/ternary/quaternary mapping and flatten-last, binary/ternary flatten, and all
four arities' distinct/concatenation identities and padding differences. Tri's two
same-result mapping callbacks remain distinct, and source filter true/false choices
are retained. Native explicit definitions, scalar entity IDs, Option nulls and ordered
field vectors replace Java callbacks/reflection/tuple overloads. Precompute remains
pending. Original runtime comparisons separately cover multiset/set updates, sharing
padding across different opposite parents, representative changes, duplicate positions,
prefix preservation, self-concatenation and rollback without upstream method credit.

Seventy-two additional existence node-sharing methods preserve unassigned-inclusive
right scopes, self-excluding Other relations, changed parents, different/same indexes
and predicates across all four arities. Assignment discovery is replaced by explicit
Option filters over native fact sources. The unary ifNotExistsOtherDifferentParent
fixture literally compares an Exists stream with a filtered NotExists stream; both
mode and parent differences are preserved. These methods add identity coverage to
existing operators. Original generated assignment/update comparisons separately
verify complete tuple payloads, visibility, absence, scores and rollback across all
arities; no new engine behavior or precompute compatibility is claimed.

Thirteen precompute methods now retain all filter variable positions across four
arities, both unique-pair variable positions, and pure-fact removal. They preserve
initial, variable-only, fact-change, remove and reinsert scores/matches. Explicit
native fact keys and identity joins replace Java mutable entity references and
property notifications; precomputed predicate/index callback counts represent the
source Mockito group-getter assertions. Cheap fact-key selection still runs on each
put, so no zero-metadata-read or reflection equivalence is claimed. Generated fixture
cardinalities and memberships are retained; unused random properties are omitted.
Five original regressions additionally verify invalidation, cached object retention,
transaction failure/rollback, shared definitions and native basic move integration.
Factory/retrieval identity methods remain pending; further operator coverage follows below.

Forty-two additional precompute operator methods preserve assigned-entity gating,
grouping/counts, list flattening, fresh value-equal holders, mapping multiplicity,
concatenation, distinct and complement padding across four tuple arities. All 55
methods in the four abstract precompute operator classes now have adaptations;
separate precompute node-sharing/retrieval/lifecycle cases remain pending.
Bi/Tri flatten's literal expected fields rely on the source match helper's
containsAll semantics (equal field count but no field-order or duplicate-field
multiplicity comparison); this behavior and its helper attribution are retained.
Native allocation-retention checks and 600 changing-fact/rollback states supplement
these source fixtures without earning additional upstream method credit.

Ten precompute/retrieval identity methods complete adaptations for the four
abstract stream node-sharing classes. Native explicit definitions and source
handles replace implicit Java factory/callback interning. Six Bavet regression
methods additionally cover unequal-depth predicate flips, mapped duplicates,
existence/absence changes and null-key self-joins, with independent graph rebuilds
at the specified states. Native immutable snapshots replace mutable object aliases;
Java deferral scheduling is not claimed. An original 1,800-update enumeration check
covers all six graph shapes through changing facts, removal and rollback.
Ten Bavet network activity methods now have complete native adaptations using
opt-in dynamic source-emptiness pruning. Source facts remain available while dead
branches skip work; changed activity plans rebuild caches transactionally. Joins,
anti-joins, concat, optional references and captured datasets have distinct input
and observer rules. An 800-update eager/pruned comparison, panic/overflow rollback
and saved-candidate tests supplement the source cases. The chain method asserting
Java FILTER-node absence remains pending: native filters are explicit graph nodes.
Predicates and key matches are not used to infer emptiness. Whole-cache rebuild cost
and recreated precomputed objects make pruning opt-in; no throughput claim is made.

Twenty-four list-ownership Bavet regressions now run through native optional list
models and the bound stream score director, preserving every assignment and match
state through replacements, removals, reordering, deep joins, existence checks,
flattening and surviving/disappearing groups. Typed IDs and immutable shadow
snapshots replace Java object references; lifecycle notification ordering and JVM
deferral are not claimed. An original 2,400-move comparison checks two-owner lists,
rollback and pruning on/off against independent assignments and match enumeration.

Eight additional BavetRegression methods preserve basic-variable null transitions
through self-joins, semi/anti joins, distinct assigned-filter paths, identity maps
and concatenation. Real required-variable moves preserve the source update order,
all score/match observations and duplicate occurrences; complete current payloads
and native structural penalties are checked separately. An original 2,048-trial
regression enumerates three-entity assignments across eight shapes and both activity
modes with rollback. Native eager immutable updates do not establish Java deferred
event or mutable-alias behavior. The subsequent deferred cascade increment below
adds the missing shadow-update cancellation behavior.

The remaining BavetRegression clearEvents method now has a complete native
cancellation adaptation: deferred list-cascade loading schedules initialization,
clearing the queue retains both null fields and their getter fallback of 2, and
a forbidden-callback flush proves pending work was actually discarded. Native
cascade scheduling replaces the Java score-director listener lifecycle; the
source method contains no score calculation/assertion. General deferred stream
event propagation and reflection parity are not claimed. Seven original cases
cover queue accumulation, cancellation/repair, eager updates, move rollback,
retry after panic, immutable snapshots and incremental scoring integration.

All twenty AbstractAdvancedGroupByConstraintStreamTest methods now have native
adaptations. Nested grouping/collection, unique-pair expansion through four tuple
positions, regrouping by exact group names, existence before/after grouping and
shared-input joins preserve the source scores, match contents and removals. Native
map/distinct expresses grouping without collectors. Typed stable identities replace
Java references; top-level match fields follow the source unordered-membership
contract, with stricter duplicate multiplicity checks. Nested collection contents
and the two-group list order remain exact. The construction-only method retains
its actual build assertion; an original 3,600-trial comparison additionally executes
all twenty shapes through incremental changes and rollback with both activity modes,
using independently calculated results. No new Java lifecycle or throughput claim.

Twenty-one more stream methods now preserve mapping to two/three/four fields from
all four input arities, joins after one/two/three distinct-count collectors,
filtered and primitive-key grouping, and uni/bi duplicate flattening. Native
canonical expansion preserves equal-value representative identity without removing
occurrences; the uni fixture retains its actual pointer-identity filter. Random
properties use deterministic values within their original range.

The bi duplicate-flatten fixture's literal expected frequencies differ from the
pipeline's actual expansion. Its assertScore helper checks total score/count and
bidirectional justification membership, so it does not distinguish those frequency
distributions. The adaptation preserves those exact source assertions and literals,
and separately checks the actual native multiset. It does not claim equality of
the literal per-justification frequencies. Four original canonical-expansion tests
cover pointer identity, per-input/per-update scope, key panic, checked-overflow
rollback and 320 incremental graph trials in both activity modes.

Sixty-nine bi/tri/quad scoring and explanation methods now have native adaptations.
They exercise weight ONE with integer or exact decimal match factors for weighted
and unweighted penalties/rewards and signed impacts. Bi streams enumerate all 21
unique pairs; tri/quad streams select the two equal-value pairs and actually join
one/two value fact sources. All literal upstream totals and helper assertions are
preserved. Default/custom justification snapshots additionally check every ordered
fact and per-match score; quad explanations retain both occurrences of the value
fact while indictments count it once per match. Native on-demand explanation
callbacks replace Java build-time justification registration and policy retrieval;
multiple-registration error behavior and general Java lifecycle equivalence remain
outside this adaptation. No production implementation was copied or translated.

Five DefaultSolverTest methods now exercise actual repeated and mixed solver runs.
Basic/list construction resumes over multiple bounded solves while retaining earlier
assignments. Mixed cases preserve whole-entity pins, pinned list prefixes, optional
first basics/lists and required second basics, using the exact upstream test scoring
formulas. Required unassigned pins remain valid partial states with structural
penalties; other movable variables can initialize. Default Java phases map to
explicit native deterministic construction/local search; no identical stochastic
trace or general reflective shadow lifecycle is claimed. Model resumption retains
metadata and checks original pins before accepting a new initial assignment.

Four more DefaultSolverTest methods exercise complete mixed stream solves, default
mixed neighborhoods, custom move providers and a custom phase. The four mixed
fixture sizes retain basic/list assignment assertions and all four derived outputs:
selected-value strength, list index+1/index+2 and inverse reference count+2, including
unused values. Real graph datasets compute these during trial updates; final output
is materialized through the same factory from the returned assignment. This is not
Java mutable-solution listener lifecycle parity. The source custom command executes
the first factory proposal; the custom provider produces both original proposals.
An independent 480-trial regression checks scores and all four derived outputs across
joint moves, required-variable clearing, list unassignment, commits and rollback,
with activity pruning enabled and disabled. No new throughput claim.

Four more DefaultSolverTest adaptations cover distinct basic/list entity groups,
solution-level and entity-level value providers, lead-entity reference domains and
ordered-set ranges. Native typed values/enum namespaces retain domain distinctions.
The multientity fixture preserves ascending strength order for one basic variable
and encounter order for the other, with exact source scoring and all four fixture
sizes. Entity-only list ranges use the native stable registry-union constructor;
this is explicit configuration, not reflective provider discovery. Three original
regressions cover metadata/pins, malformed ranges and256 overlapping range pairs.

Two entity-range DefaultSolverTest methods now exercise all nine supplied list
move configurations with one/two search phases, exact scoring, optional values,
nonempty final lists, legal owner ranges and retained-best events. Repair unions
include change, swap, sublist and multi-list k-opt families in a shared search.
Native deterministic seeds, bounded proposals and uniform within-family sampling
replace Java selector traces; original-order variants remain distinct in isolated
phases, while union order duplicates are separate weighted children. Small fixture
ranges can leave a configured family empty; original larger-model checks force
sublist/k-opt execution and verify pins, successors, budgets and reproducibility.

The matching two basic entity-range methods exercise all seven configurations,
including distinct pillar-change-only and pillar-swap-only selectors, with one/two
phases. Actual optional null initial values, per-entity domains, +/-1 score formula
and excluded-value assertions are retained. Native subpillar subsets and seeded
move sampling replace default Java selectors; no identical trace is claimed.

Independent mixed-neighborhood configuration now supports basic-only/list-only
families, selected basic slots, independent joint products and weighted unions.
Original regressions cover 7,128 small-state configurations, pins and rich move
families, union multiplicities and complete scoped/stream solver plans. No upstream
method is newly credited by this API increment: the mixed parameterized methods
also require variable-specific repair pools and mixed repair unions, still pending.

Scoped repair candidates, separate weighted repair pools and independently scoped
mixed fine families now run in the shared repair engine. Original regressions cover
all16 mixed identity subsets, budget prefixes, both scoring backends and complete
phase plans; source suite credit remains unchanged. The larger mixed selector cases
still need accurate per-family pillar grouping: pillar change selects one basic
column, while pillar swap can select an entire two-column entity row. Do not count
those methods until all twelve source configurations preserve those distinctions.


The two mixed-selector DefaultSolverTest methods now preserve all twelve source
configurations apiece. Per-family pillar schemas distinguish first-basic-column
grouping from aligned two-column entity grouping. Singleton row swaps, subpillar
changes/swaps, independently scoped basic/list repair, five list move families
and the eleven-child union run through real bounded solver plans. Initial fixture
sizes, component construction order, exact objectives, nonempty best events and
all final source assertions remain intact, including comparator-induced differing
basics in the multientity fixture. Native deterministic sampling and explicit
schemas replace Java factory/PRNG behavior; reflection/shadow lifecycle and equal
proposal distributions are not claimed. Three original regressions cover 256
state/pin/range combinations, invalid configuration before callbacks and crossing
a local barrier through both complete scoring backends.


Four additional DefaultSolverTest selector methods now cover all 22 supplied
configurations: six single-basic, six list, five multivariable and five multientity.
The multivariable fixture retains both required fields and its optional third
field with a distinct value type. Row swaps align all three fields; pillar change
selects only the primary field. Multientity pillar families target lead entities,
with distinct value/lead-reference namespaces. All methods preserve actual solver
execution, the dummy objective, result assertions and nonempty best events.

The multientity source sets a best-score limit of zero, reached by construction;
its adaptation preserves that early stop and does not claim that search ran.
Native sequential construction retains lead-before-herd order within one phase.
Native bounded proposals and score diagnostics replace Java sampling and tracking
internals. Empty families on tiny fixtures and optional null fields remain valid.
Independent runtime regressions demonstrate actual scoped/grouped move execution.


Two DefaultSolverTest sorting methods now have complete native adaptations.
The mixed method preserves all ten entity/value sorter combinations, actual
integer difficulty/strength comparisons, first-basic-only selection and every
final assertion. The other method preserves both ascending/descending families,
the exact sequential Random(0) shuffled fixture encounter orders, the distinct-
value HardSoft objective, local candidate limit ten and actual nonempty best events.
Native target permutations and bound providers replace reflective selector caches;
Java default search/random traces and listener lifecycle are not claimed. Public
enum signatures were inspected with javap -public, without implementation bytecode
or production source. Original regressions additionally show ordering changes the
retained solution under a one-candidate budget in both scoring backends.


## Optional list construction and pinned list solver cases

Seven additional DefaultSolverTest methods preserve optional-list and pinned-list
solver assertions using both scoped and stream scoring. The optional fixtures
cover removing all assignments (score0), assigning remaining values with explicit
early picking (score4), and a one-candidate interruption before the final no-change
candidate (score1, original empty owner retained). The four pinned fixtures retain
full-list pins and prefix pins while minimizing/maximizing empty owners, with every
source final membership/count/score assertion. Provenance records the fixtures,
calculators and helper substitutions. Java ONLY_DOWN inference is replaced by an
explicit native early-pick policy; native model-derived list views and initial
scoring replace SolutionManager update. Deterministic HillClimbing, ten search
steps and native score diagnostics substitute for factory search/TRACKED_FULL_ASSERT.
These adaptations do not claim JVM listener, reflection, index-supply lifecycle
or identical search traces.

Native ConstructionInterruption::DiscardIncompleteStep applies to basic, list and
mixed construction, both scoring engines and custom provider construction. It
retains the previous assignment/cache/graph on an interrupted scan without replaying
scoring callbacks; discarded trials remain in evaluation counts but do not consume
completed steps or publish solution events. Default KeepBestPrefix behavior stays
available. OptionalConstruction::EvaluateUnassigned appends a scored no-change
candidate last; explicit early picking may select an insertion before that candidate.
Sequential, pooled and queued-value placements implement this policy. Provider
construction appends it after natural proposal exhaustion; synthetic candidates
count toward score/move limits, not raw-proposal limits. Providers are never pulled
past an exact limit just to prove exhaustion, so atomic mode discards at that bound
unless an intentional early pick completed the step. Original regressions separately
check improving-trial rollback, callback counts, observer visibility, score/move/scan
limits, cancellation, controlled phase clocks and transient shared termination.


## Queued placement providers

Native QueuedMoveProvider supplies one assignment-aware candidate pool per ordered
key, with finite or cycling queues, skipped empty pools, non-Clone keys and lazy
factories borrowing the current problem/assignment. Increased completed-step counts
advance the queue; repeated counts retry the same key; decreasing counts rewind it.
Explicit reset handles a new phase with the same counter. Construction notifications
let rejected optional placements advance; a full cycle of rejection/empty pools
stops, and a successful installation resets that cycle. Ordinary pooled providers
retain their stop-on-rejection default. Atomic discarded scans issue no completed
placement notification. Candidate validation/scoring stay in the existing engine.

DefaultSolverTest.solveCustomConfigListVariable now preserves the source2value/
2owner fixture, exact singleton-owner reward,16search-step configuration, nonempty
owner assertion and actual best events with both scoring engines. A bound queued
provider uses a real RecordingSelection publication and replay cursor to choose
insertion moves; identity and publication order are also asserted. Native per-step
value sources and explicit bindings replace Java selector configuration; no factory
parser, automatic lifecycle forwarding or identical-trace claim. The queued/pooled
placer lifecycle and copy source methods remain pending, not partially credited.


## Structural list dependency policy

ListProblem::with_dependencies declares fixed value dependencies by registry index,
plus a dynamic edge to each assigned value's previous list element. This explicit
opt-in complements derived-value scoring; it does not infer dependencies from score
callbacks or implement arbitrary annotated shadow discovery. Fixed cycles/unknown
indices are configuration errors. Missing dependency values alone are not cycles.
List/mixed complete solvers clear assigned cycle members and downstream dependents
before scoring, aborting on affected full/prefix pins. The cleaned initial snapshot
sets an event baseline without publishing a best event. Model metadata and caller
assignments remain unchanged; standalone phase callers can prepare_assignment.

Model validation and reversible moves reject cyclic final states before scoring.
Automatic list/mixed selection skips these trials in both engines; explicit custom
commands return a typed error and roll back. Exact enumeration skips invalid leaves;
bounded search prunes cyclic insertions (later insertions cannot remove the existing
precedence reachability). No additional hard score penalty is introduced.

Four DefaultSolverTest methods using no_inconsistent_field fixtures are adapted:
automatic initial cleanup with zero best events; pinned b2 rejection; a custom
composite recreating a dependency loop; and solving empty lists to HardSoft(0,-360)
with both dependency-time inequalities and actual best events. All run both engines.
Native typed errors identify registry values/positions instead of Java exception
classes/message strings. Minute offsets replace LocalDateTime; native dependency
projections and built-in owner/previous views replace shadow listeners. Provenance
records all four fixtures and both PlannerTestUtils helpers. No lifecycle or trace
compatibility claim accompanies these behavioral adaptations.


## Observed reversible list edits and startup shadows

ListProblem::apply_observed validates a private candidate before exposing bracketed
before/after notifications. Events contain the complete immutable assignment and
minimal changed ranges per entity; insertion/removal excludes an unchanged suffix,
whose indices listeners can shift by the range-size delta. Commit retains the edit;
scope exit reports reverse ranges and restores the original assignment. Invalid and
unchanged moves emit no events. Observer panics restore assignment state; unrelated
unwinding rolls back silently. External observer side effects remain caller-owned.
Typed observed relocation/shift/span helpers also return the moved identities.
These explicit model-level notifications do not replace automatic scoring or imply
Java score-director lifecycle compatibility; callers opt into the observed API.

32MoveDirectorTest methods now retain full forward/undo lists, returned identities
and every asserted notification range through real native callback events. Native
atomic brackets replace Mockito per-entity hooks; typed coordinate/offset errors
replace Java exception text. Same-entity rejection in the specifically cross-list
Java overloads remains pending: native generalized operations accept both kinds.
No repeated credit is added for the15assignment cases already implemented earlier.

Two DefaultSolverTest startup cases also retain exact source objectives/results and
actual best events in both engines. Inverse memberships rebuild from the initial
basic assignment (score-4), reaching distinct values/score-2. Concurrent cached
shadows rebuild to HardSoft(-4,0) before local-only feasible termination; the exact
source final lists and HardSoft(0,-240) are preserved. Native derived state replaces
mutable stale Java shadow fields; no JVM discovery/listener or random-trace claim.


## Prepared list lifecycle plans

ListLifecyclePlan validates an owned snapshot-bound plan for replacement, assignment,
unassignment, arbitrary native moves and bulk gathering. ListLifecycleEvent reports
individual value assignment/removal and list-range brackets against the actual
working snapshot. Gather removes one batch per source in first-member order and
inserts the selected block in input order, adjusting original destination boundaries.
A single Flush callback runs after all stages so a listener can refresh derived state
once. Undo reverses stages, value roles and ranges and flushes the restored state.
Commit retains changes. Stale assignments (under value equality) are rejected; errors
while preparing publish nothing. Observer panics restore assignment state; external
observer side effects require caller recovery. Plans use full snapshot copies.

These explicit hooks do not discover JVM shadows or attach observers automatically
to solver instances. Intermediate gather snapshots may be partial; score/validate
at Flush. Ordinary movement() integrates with both native scoring backends. Seven
MoveDirectorTest methods retain exact replacement/unassignment/gather results,
returned identities, notification ranges and ordered value hooks; gather tests
refresh real owner/index projections once and verify undo. Same-position/empty-span
checks use native typed errors. The negative Java index overload remains pending;
unsigned Rust coordinates do not exercise that source failure.


## Temporary scored processing and recorded undo

TemporaryMoves wraps each of the six basic/list/mixed scoring directors. A callback
sees the validated candidate assignment and its calculated score; success restores
both working assignment and scoring cache. Callback Result errors and unwind panics
obey explicit Rollback or KeepApplied policy. Validation/scoring failures restore
state without invoking the callback. Cached restoration avoids recalculation;
CheckFresh recalculates contributions or replays the graph after successful undo,
reporting corruption if fresh evidence disagrees with the restored cache.

recorded() returns owned forward/undo moves with both expected assignments. A
successful temporary record has already been undone. Forward/undo access checks the
expected preimage under value equality; callers then apply through the same native
model/director. Basic inverses restore changed variables, list inverses clear all
changed lists before reinserting original contents, and mixed inverses do both.
Pins/ranges/dependencies are validated by ordinary atomic model application.

Six MoveDirectorTest methods retain postprocessor results, callback-time state,
restored scores, forward/undo replay and failure-leaves-applied behavior. The latter
uses an actual panic with explicit KeepApplied, preserving the source assertion.
Fresh-score mode checks a genuine recalculation; external mutable scoring facts are
reported as inconsistent rather than silently overwriting a previously cached score.
The later sections cover construction failure and structurally flawed processing.
No Java reflective discovery or recorded-action-list claim.


## Live multi-move edit sessions

All six native score directors now provide edit_session. MoveSession applies each
validated move immediately, exposing its working assignment and score. A failed
step preserves the preceding successful edits. Nested sessions restore or commit
independently; outer rollback also undoes committed inner changes. Snapshot restore
performs no scoring callbacks and restores contribution caches or the entire stream
graph, including group indexes and active-node state. Session storage scales with
assignment/cache size; ordinary single-move guards remain the hot-loop primitive.

TemporaryMoves::construct accepts a live construction callback followed by scored
postprocessing. Either callback's error/panic uses the selected rollback/keep-prefix
policy; construction failure skips postprocessing. A retained prefix remains scored
and consistent. One additional MoveDirectorTest source method verifies actual
write-then-panic retention in both basic backends. No nested Java phase
factory/action-log claim is made.


## Structurally flawed candidate inspection

List and mixed directors expose inspect_move; standalone inspect_list_move and
inspect_mixed_move return the actual proposed assignment and assigned registry IDs
in dependency cycles or downstream of them. Only dependency consistency is deferred.
Pins, domains, duplicate identities, coordinates and the initial assignment remain
validated. Mixed snapshots include simultaneous basic-variable changes.

TemporaryMoves::process_inspected routes sound candidates through ordinary temporary
scoring and flawed candidates to a separate snapshot callback. A flawed candidate is
never installed or scored, including when KeepApplied is selected. Its callback
errors/panics leave working assignments and caches intact. CheckFresh recalculates
or replays the original score after either successful branch. Snapshot storage and
model cloning scale with input size; this is an explicit inspection API.

One additional MoveDirectorTest source case preserves the cyclic swap, callback
result, original order and previous score in both list backends. Original regressions
cover all four list/mixed backends, downstream diagnostics, callback errors/panics,
fresh recalculation, valid candidate scoring and strict nonstructural validation.
No Java mutable-solution callback or scoring of inconsistent shadows is claimed.


## Cascading shadows in scored list streams

list_cascade_stream connects list-value rows to the incremental dependency engine,
using dynamic predecessor edges and a deterministic supplier. Constraints consume
stable registry-keyed (ListValueRow, DerivedValue) rows, including unassigned values
with absent shadows. The supplier receives owner/index, neighbors and the previous
shadow, allowing arrival times or cumulative quantities. The same adapter accepts
MixedStreamSources::lists; basic and list changes share one graph transaction.

Shadows now participate in ordinary scored moves, saved search candidates and nested
edit sessions. Undo restores their values and dependency indexes without supplier
callbacks. Errors and supplier/scoring panics restore graph and model. Changed list
value notifications recalculate their roots; shadow changes propagate downstream.
Intermediate notifications may expose missing/inconsistent derived rows until the
transaction finishes. Suppliers must be deterministic over immutable facts and the
provided inputs. This does not import stale shadow fields or discover Java metadata.

One MoveDirectorTest method preserves cascade initialization on assign and null
restoration on undo using the source single-cascade fixture and score formula.
Original tests compare 162 partial-list/order trials to an independent schedule
calculation, exercise list/mixed rollback and nested sessions, and compare actual
local-search selections with an independently scored network at three work limits.
No callback-count equivalence or new runtime performance claim is made.


## Inverse relationships in basic and mixed streams

basic_inverse_stream selects basic-variable slots through an explicit entity-ID
mapping and emits each value-universe row with its referencing entities. Nonselected
fields and unassigned variables contribute no membership. Inverse collections are
ordered sets with counted removal: removing one of several references from the same
entity retains membership until the last reference disappears. Empty values remain
visible, and equal payloads at different universe IDs share the same membership.

The adapter composes existing transactional expansion, collectors and reference
nodes. Inverse collections and downstream scores participate in ordinary move undo,
saved search candidates and nested sessions. MoveSession::director provides read-only
access to live derived rows. Metadata mappings and identity ordering must remain
stable. This is explicit native metadata, not reflective Java field discovery.

Two MoveDirectorTest cases preserve sequential changes to distinct basic fields with
an inverse relationship on the first field only, and two consecutive list removals
whose current indices shift. Actual inverse/owner/index rows are checked before
scope exit and after rollback. Original tests cover all81partial basic assignments,
counted duplicate removal, empty/replaced universe rows, excluded-field callback
reuse, scoring panics, nested edits and reference-model local-search results.


## Working-object identity lookup

WorkingObjectLookup registers Arc instances under planning IDs described by explicit
native schemas. Lookup of an external same-ID object returns the stored working Arc;
removing an external clone is rejected. Duplicate IDs, missing registrations, absent
ID metadata, declared-null IDs and multiple declared members have distinct errors.
IdOrNone ignores objects without ID metadata when adding/removing, but still rejects
their lookup. Disabled skips registration/metadata validation and rejects non-null
lookup. Explicitly immutable schemas return the external Arc without registration.
Optional lookup of None returns None. Each registry owns a separate ID namespace.

Reset removes registrations; reset_with validates an entire replacement before
installing it, preserving the old registry on validation errors or ID-reader panics.
Registered IDs and their ordering must remain stable. Registries store identities,
not deep clones, and do not discover Java annotations, inherited/interface metadata
or immutable platform types automatically. Native schemas are part of model setup.

Forty-one upstream lookup methods are adapted across IdOrFail, IdOrNone, Disabled
and manager tests with exact fixture attribution. The inherited-null-ID and interface
metadata cases remain pending, as does the full parameterized immutable-type matrix.
Original tests cover identity-preserving resolution, namespaces, atomic replacement,
callback failures and using resolved external values in both native scoring backends.


## Named problem-size statistics and scale formatting

NamedProblemSize extends existing native statistics with explicit entity-type and
field bindings. Every basic/list slot must be bound exactly once, every field must
have one slot per entity, and duplicate names, missing slots and invalid indices
are rejected. Multiple basic/list fields can describe the same logical entity.
Distinct values are counted per type/field and summed across those namespaces.
Empty basic fields require a separately declared range size; list registries retain
this information even without owners. Assignment-space counts come from the model.

ProblemScale extracts a finite logarithm from arbitrarily large BigUint counts and
formats US-grouped integers or six-decimal scientific notation. Imported nonfinite
logs format as zero. Native fixed-point conversion accepts explicit precision; the
source case requiring the JVM default LOG_PRECISION remains pending. No production
MathUtils implementation or constant was read. Named metadata is explicit, without
Java descriptor/annotation/record discovery.

Ten ProblemSizeStatisticsTest methods preserve source fixtures, entity/variable and
range counts, per-type/per-variable maps and formatting assertions. Original tests
cover mixed multi-field entity grouping, invalid bindings, empty-model metadata,
large logarithms, pins and dependency restrictions. Existing native statistics now
count an uninitialized required pinned slot as zero complete assignments and label
list counts with dependency metadata as upper bounds. Existing job telemetry uses
these corrected native counts; its flattened-slot fields remain backward compatible.


## Managed planning value ranges

ManagedValueRanges maintains the range providers for one native planning field.
Shared providers survive empty entity sets; per-entity providers produce an ordered
distinct union. Composite providers deduplicate repeated values. Equal ordered
ranges share Arc snapshots and active sort state. Only optional basic ranges add
None; optional list ranges contain ordinary values. Raw views remain unsorted.

Reusable ManagedRangeOrder handles cache the current stable sort. A new sorter
replaces the active/default range; old snapshots stay valid. Cloned sorter handles
reuse the same result. Captured mutable facts require a new handle or a provider
reset. Sorting prepares replacement state before publishing it, so comparator or
value-clone panics retain the previous active range. Provider resets replace all
range state. Equality must denote stable planning identity.

Basic slots and list models can be built from the current ranges; list construction
retains supplied initial assignments and pin metadata. Models are snapshots: later
range edits do not mutate an in-progress solver. Large lazy providers must be
materialized by the caller; native equality-based deduplication is not a large-range
performance claim. Multiple fields use separate managers and explicit metadata.

Thirty-four ValueRangeManagerTest methods cover basic/list, shared/entity,
required/optional, single/composite extraction and sorting, plus both identity
deduplication cases. Fixtures and source sorting/assertion helpers are attributed.
Initialization counters and scale estimates are described below;
Java descriptor discovery and dynamic solver problem changes are separate concerns.
Original regressions cover comparator failures, immutable snapshots, resets, cached
comparator reuse, 256 independent range pairs, and actual basic/list solving with pins.


## Managed range statistics and initialization

Managed range statistics now distinguish shared value counts from the sum of
entity-provided counts. Optional basic ranges include the unassigned choice.
Multiple fields on the same logical rows combine entity counts once, variable
and value counts additively, and logarithmic sizes by addition. Empty required
ranges use the source zero-log reporting convention with a separate flag; this
value is not proof of feasibility. Estimates ignore assignments and pins.

Unrestricted list estimates count ordered assignments; optional lists sum over
assigned subsets without assigning an order to unassigned values. Logarithmic
arithmetic avoids floating-point overflow. Restricted ranges use each value's
fraction of eligible owners as an independent weighting approximation. This may
produce fractional estimates and is neither an exact count nor an upper bound.
The existing exact/upper-bound ProblemStatistics API is unchanged.

Fresh basic/list initialization snapshots validate supplied assignments, report
required missing basic values or unassigned registry values, and count native
list values as shadow entities. Recomputing after provider reset or assignment
changes cannot reuse stale counters. Optional basic None is initialized. Snapshot
validation checks ranges and duplicate list assignments; pin metadata belongs to
the separate solver models.

Thirteen remaining ValueRangeManagerTest methods are adapted, bringing that class
to 47 methods. The eight-type range fixture uses its actual values as textual native
identities in separate fields. The 500-value scale reference uses exact combinatorics
from the behavior specified by MathUtilsTest, not the JVM production log helper or
its fixed-point precision. Original enumeration checks both required and optional
unrestricted spaces, and explicit restricted examples demonstrate approximation.


## Typed planning metadata

PlanningSchema validates explicit solution/entity/field declarations and retains
canonical metadata in declaration order. PlanningType uses native TypeId identity
and type names for diagnostics. Entities with genuine fields are distinguished
from shadow-only entities; variables retain basic/list/shadow kind, optionality
and value type (the element type for lists). Borrowed entity/variable handles
preserve parent identity without owning cycles or outliving their schema.

Variable declarations construct shared or entity-provided managed ranges with
value-type checks and inferred basic/list optionality. Shadow fields cannot create
planning ranges. Statistics bindings require all genuine fields exactly once,
check lengths/kinds, retain declaration order and omit shadow fields. Applying
bindings to NamedProblemSize also validates actual native slot bounds/uniqueness.
An original integration test builds and solves a mixed model and obtains named
statistics from the same declarations.

All 29 nested PlanningSolutionMetaModelTest methods are adapted with exact pinned
source lines to disambiguate repeated names. Six basic/list metadata fixtures are
attributed. Explicit Rust declarations replace Java descriptor discovery; this
does not implement object traversal, reflection, inheritance, field accessors or
automatic shadow evaluation. Existing range/scoring APIs supply those operational
concerns explicitly.


## Typed shadow source paths

SourcePathRegistry binds native typed accessors to PlanningSchema metadata.
Registration checks owner/value types, variable roles, scalar/list cardinality
and duplicate names before changing the registry. CompiledSourcePath resolves
direct fields, previous-element chains, fact links, groups and planning-list
elements. References retain canonical source/target metadata, ordered member
chains, parent classification and downstream declarative identity. Runtime
traversal borrows actual snapshot objects, skips null links and empty groups,
and never reads the terminal variable value. Wrong root types fail explicitly.

Dependency keys can be mapped from resolved variable/entity pairs to the existing
DependencyGraph/Topology/Cache APIs. Keys are sorted and deduplicated while raw
traversal retains group multiplicity. An original operational test derives
predecessor dependencies, evaluates them, changes an edge incrementally, detects
a dynamic cycle without invoking cyclic suppliers, and restores a saved cache.

23 RootVariableSourceTest cases and three invalid-declarative fixtures plus two
assertion helpers are adapted. Path validation rejects missing members, more than
two source variables, nested groups, bare lists, unsupported list access, chained
declarative sources, paths ending in facts and consistency flags. Explicit typed
registration replaces Java field/getter discovery; inheritance, supplier method
parameter discovery and automatic shadow setters remain separate work. The
remaining RootVariableSourceTest reflection cases are not credited.


## Shared variable state and event tracking

VariableStateRegistry keeps one shared state and a separate tracker per canonical
planning field. Basic states expose current values and inverse collections; list
states expose assignment, owner, index and neighbors. Rc-backed state handles
survive lookups, tracker registration order, field reset and assignment updates.
Value types and stable entity-key types are checked at the field boundary; fields
with matching types remain distinct. Linking never reads a solution provider.
Snapshots loaded before handle lookup are immediately visible to later consumers.

Explicit reset replaces one field and clears its tracking window. Updates record
before/after presence for each changed entity and publish its inverse/index view
immediately. VariableEventTracker audits missing events for its canonical field,
ignores other fields, and resets the audit window. It tracks event presence, not
nested bracket counts. Duplicate keys/list assignments and invalid native slot
bindings fail before existing state or events change. Detached snapshots can be
used to restore a field; cloned handles continue observing the live state.

Native basic/list synchronization binds stable entity identities to assignment
slots. Original integration tests compare inverse state against scored stream
moves across 27 partial assignments and restore list neighbors through observed
move undo. This registry is single-threaded and explicitly synchronized; it does
not discover objects, enforce model pins/ranges or automatically execute shadow
suppliers. Each update currently rebuilds that field's indexes.

16 VariableSupportTest cases and five cases each from BasicVariableTrackerTest
and ListVariableTrackerTest are adapted. Structured missing-event records replace
JVM diagnostic strings. The tracked inverse case uses a native score director
and consistency check. Concurrent shadow graph event orchestration, embedded
Java inverse-field mutation and automatic shadow-session construction remain
separate work; no credit is claimed for their tests.


## Deferred list shadow sessions

ListShadowSession binds a shared ListVariableState to one typed shadow field on
the list value entity. It reuses DependencyCache for predecessor scheduling,
equal-result propagation stopping, and atomic supplier failure behavior. Parent
notifications coalesce by stable value identity; flushing reads the latest whole
list snapshot and detects structural changes without explicit notifications.
Unassigned values remain separate nodes and their suppliers can clear results.
Owner additions/removals are supported; value-universe changes need a new session.

Current-value reads reject pending or stale state. Initialization may be deferred.
Checkpoints share non-Clone results and restore without callbacks, after the bound
field is restored to the matching snapshot; foreign/mismatched restores fail.
Metadata/universe errors invoke no suppliers. Captured mutable external facts and
callback side effects remain the caller's responsibility. This is an explicit
previous-chain execution API, not arbitrary source-path inference or Java setter
reflection; multiple independently calculated fields use separate sessions.

Both SingleDirectionalParentVariableReferenceGraphTest methods and their three
counting fixtures are adapted. Original tests compare real native list moves
against fresh cascade calculations, restore caches without supplier work, cover
latest-snapshot notification batching, equal-result stopping, unassignment,
non-Clone results, supplier panic recovery and invalid metadata/restore errors.


## List state supplies and materialized shadows

ListStateSupply wraps shared list state with a fixed value universe, live assignment/
position/pin queries, and explicit refresh of materialized built-in shadows. Native
model bindings import full and prefix pins by stable owner identity. Pin queries
observe current positions; actual moves still require ListProblem validation.

Any subset of owner/index/previous/next fields can be bound to typed canonical
shadow metadata. Refresh emits changed before/after values; callers can apply them
to owned storage and notify ListShadowSession. Fully bound supplies suppress fallback
notifications. Partial bindings notify rescanned assigned values even when unchanged.
Range refresh covers shifted suffixes and affected predecessors; removed values clear
their fields. Imported consistent shadows avoid redundant events. Unassignment
updates registry observation rows, rejects repeat removals, and clears seeded shadows.

All ten DefaultListVariableStateTest methods and their fixtures/helpers are adapted.
Three original tests exercise native move/undo, actual materialized field updates,
deferred supplier integration, pin enforcement and invalid-input atomicity. Registry
updates and publication remain explicit; automatic setters and solver-wide wiring
are separate work. Refresh currently rebuilds maps and carries no performance claim.


## Basic inverse collection publication

BasicStateSupply adds explicit publication to shared BasicVariableState. Live
inverse queries follow registry updates immediately; a bound Vec<E> shadow on
the value entity receives complete before/after collections in stable native
identity order. Callers apply these records to owned fields. Existing collections
can be seeded, avoiding redundant publication when already consistent. Without a
binding, each removed/added membership contributes a notification, including
repeated value identities. Refresh coalesces intermediate registry edits.

Metadata, unknown values and invalid seed collections are rejected before existing
publication changes. Registry resets remove old memberships; retry after correcting
unknown values preserves the last successful observation. Native move validation
and registry synchronization remain explicit. The supply currently rebuilds its
projection and does not claim automatic setters or a performance improvement.

All three BasicVariableStateTest cases and six fixtures are adapted. Original tests
compare actual materialized collections against independent membership calculations
and scored inverse stream rows across 27 partial assignments, then restore them on
undo. Additional regressions cover seed validation, metadata isolation, empty/reset
state, unassignment, notification multiplicity and coalesced updates.


## Exact list-shadow histories during moves

All twenty ListVariableShadowVariablesTest methods now execute native list moves
and apply typed state-supply events to owned shadow fields and history vectors.
The adaptations retain every exact owner/index/previous/next history and final
field assertion, including unchanged fields and untouched null neighbors. Coverage
includes initialization/reset, owner removal, assign/unassign, relocation, swaps,
neighbor swaps, sublist relocation/swaps and reversal. The required-list partial
score assertion uses the native score network with its structural unassigned count.

These cases establish per-field change histories for the tested moves; they do not
assert event ordering between different fields, automatic setter discovery, or
solver-wide dynamic entity removal. Removing an owner resets explicit registry
rows and publishes cleared shadows. Existing production APIs satisfy the cases
without additional solver changes. Fixtures and eleven assertion/move helpers
are attributed separately. No new performance measurements are claimed.


## Heterogeneous typed shadow sessions

ShadowSessionBuilder registers typed basic/list/external-shadow readers and derived
fields against canonical schema metadata and stable native entity keys. One session
can contain different result types. Typed handles reject foreign sessions, duplicate
field/entity registration and metadata mismatches. Suppliers can read only declared
dependencies, plus immutable world facts; the supplied world reference supports
solution-parameter calculations without reflection.

Update samples readers, compares typed values, and reuses DependencyCache to schedule
affected suppliers once in dependency order. Equal results stop propagation. Dynamic
edges can be rewired; cycles and their descendants become absent/inconsistent without
supplier calls, then recover when the cycle is removed. None alone is consistent.
Reader/supplier failures retain cached values and pending work. Checkpoints share
non-Clone outputs; restore checks registered inputs and runs no derived suppliers.

Reads expose the last published snapshot. World edits require update, and mutable
facts outside registered readers require explicit invalidation of direct consumers.
Pending edits reject reads. Restore validates registered inputs, not arbitrary world
facts or callback side effects. Callers still apply field outputs and synchronize
model assignments explicitly; automatic source-path binding/setters remain separate.
Input sampling scans all registered readers; no performance claim is made.

Four ShadowVariableUpdateTest methods and six fixtures are adapted, including actual
inverse publication, dependent time fields, entity-only registration and solution
parameters. Native time fixtures retain the source origin and exact second offsets.
Java empty entity-array and null inverse-collection errors remain pending. Original
tests cover heterogeneous chains, native list trial/undo, equality stopping, cycles,
non-Clone snapshots, undeclared/foreign handles, absent inputs and panic recovery.


## Source paths bound to shadow execution

ShadowSessionBuilder can bind compiled source paths to derived nodes. Each update
traverses current typed objects, validates root identity and resolves canonical
field/entity pairs to registered nodes before readers or suppliers run. Changing
references, null parents and group membership rewire the dependency graph
automatically. Manual edges remain available for separate targets. Unknown sources
and conflicting bindings fail before publication changes.

Ordered reference signatures retain group order and repeated members, so a changed
traversal invalidates its target even when the deduplicated edge set stays the same.
Suppliers can read dependencies by canonical field; returned values represent unique
registered nodes in registration order. Group cardinality/order can be read from
the current world and are covered by the traversal signature. Restore validates
both signatures and sampled inputs, then restores cached values without suppliers.

The MixedModelTest parent-delay case uses actual temporary mixed moves and compiled
previous/previous.delay paths. Three fixtures are attributed. Original regressions
cover live rewiring, nulls, cycle recovery, duplicate/reordered groups, mismatched
restore, missing sources/root identity and explicit/automatic binding conflicts.
Node registration and root access remain explicit; node universe changes and Java
reflection are separate concerns. Resolution currently scans registered nodes.

## Typed shadow sessions in transactional scoring

`StreamGraphBuilder::map_shadows` evaluates registered heterogeneous shadow fields
from complete world snapshots and projects typed results into scored or joined
tuples. Each input identity has its own persistent dependency cache. Explicit
edges and compiled source paths both work. Trial undo, saved winners and nested
move scopes restore caches without readers, suppliers or projections; retraction
removes a cache and reinsertion starts fresh. Diagnostic rebuilds replay current
world snapshots from fresh caches. Cached field values need not implement Clone.

World snapshots must include every registered root. Mutable facts need registered
input readers or compiled path signatures; captured facts remain immutable.
Callback history and external side effects are not transactional. This is an
explicit native integration, not automatic Java field discovery or setters.

ListModelTest.simpleList now runs actual construction and local search through
compiled owner/previous/start/end dependencies and this scoring integration. It
starts with an empty required list, minimizes all end times, reaches -344, and
preserves the exact order and six time assertions. Native score assertions check
rebuild consistency; deterministic bounded search replaces Java configuration.
Four source fixtures are attributed. Original tests exercise independent row
caches, non-Clone values, retraction, rollback, callback failures, nested list
moves and saved-winner installation without replay. No performance claim is made.

## Permanent shadow cycles and accessor models

ShadowSessionBuilder validates compiled references owned by their root before
building a checked session. These dependencies always address the same registered
entity and cannot disappear when the world changes. `try_build` rejects their
cycles with the sorted, deduplicated field names of actual cycle members, excluding
downstream dependents. Missing root-field registrations also fail before callbacks.
`map_shadows` uses this validation automatically. Links through other objects stay
dynamic and may become inconsistent and recover normally. Explicit manual edges
remain mutable; low-level `build` retains its permissive cache semantics.

AlwaysLoopedShadowVariableTest now exercises real solver initialization and rejects
the odd/even fixed loop before readers or suppliers run. MethodVariables and
ExtendedValues cases now solve the two-owner concatenated-code models to 0hard/6soft
through getter-based source paths and transactional shadow scoring. Rust composition
models shared base storage; explicit registration replaces Java annotations and
covariant accessor discovery. This does not claim Java reflection or inheritance
support. Three source cases and thirteen fixtures are attributed. Original tests
cover precise cycle membership, self loops, missing registrations, dynamic cycle
recovery and score/cache rollback.

## List edit recording and notification-aware undo

`ListChangeRecorder` observes bounded edits to one canonical list variable and
records separate before/after brackets by stable entity identity. Interleaved
owners, nested brackets across owners, batched swaps and repeated/overlapping
ranges undo in reverse recording order with one notification pair per edit and
one final shadow refresh. Shared non-delegating handles retain the same pending
records while suppressing forward notifications. Live undo handles see edits
recorded after their creation and remain reusable after successful execution.

Malformed ranges, duplicate/orphan events, mismatched starts, incorrect length
changes and edits outside the reported range reject without changing records.
Undo validates all saved post-edit snapshots before invoking callbacks. Validation
errors and ordinary callback panics preserve caller lists and recording; external
callback side effects remain outside rollback. Callbacks receive the working
snapshot at each event. Whole-field registry/shadow publication belongs at the
final refresh because cross-owner replay can temporarily duplicate values.

All 21 VariableChangeRecordingScoreDirectorTest methods are adapted, including
exact list restoration and notification ranges/counts. Native tests additionally
exercise a committed legal list move, restoration of published owner/index
shadows and stream score, malformed replay, callback failure and recovery.
This is explicit observation of caller-owned lists; native model validation and
pin/value-range enforcement remain required. Automatic mutation interception,
Java setters, basic-variable recording and allocation-efficient range snapshots
are not implemented by this API. No performance claim is made.

## Move/undo value and event diagnostics

`SolutionTracker` coordinates detached typed snapshots for explicitly registered
basic, list and shadow fields. It compares before-move versus after-undo values,
and independent full recomputations versus each corresponding stored state.
Equality uses the actual value type, not rendered text. Missing entities remain
distinct from present entities with null values. Values need Eq, but not Clone or
Ord, because readers supply owned snapshots. Mutable shared values are unsuitable.

Forward and undo notification windows are audited separately with existing
VariableEventTracker handles. Registry trackers can be connected directly;
independent trackers cover manual mutations and shadow fields. Only registered
fields are covered. An empty report is not evidence for unregistered fields or
score correctness. The caller supplies recomputation snapshots explicitly; the
tracker does not rebuild scores or automatically intercept solver mutations.

All seven SolutionTrackerTest cases preserve source diagnostic text, null/value
mutations, capture order and formatter truncation. Structured reports retain every
violation and repeated report reads do not consume events. Native regressions
exercise a committed list move with registry events and stale shadow totals after
undo, heterogeneous field types, non-Clone values, identical printed labels,
missing entities, foreign snapshots, duplicate stages and failed reader recovery.
Registration freezes at the first before-move capture. Starting a new move resets
all prior snapshots and event windows; event audits must have a single consumer.

## Sorted construction providers

`SortedBasicConstruction` and `SortedListConstruction` add stable, phase-scoped
entity and value ordering to the existing queued construction engine. They bind
to actual construction phases on network and stream backends without rearranging
model IDs. Basic sequential, cyclic value-queue and pooled placement are supported;
list sequential/value queues and pooled insertion honor sorted destination owners,
pinned prefixes and receiver ranges. Equal comparisons preserve encounter order.

Comparators run when preparing the provider. Comparator factories receive the
initial problem and assignment once; rebuild the provider when those priorities
change. `reset` rewinds the existing order. Eligibility is checked against current
assignments at each placement. Use the bound model and matching optional policy.
The existing engine handles scores, early choice, limits, interruption and undo.
Sorting is outside the search call's time budget; dynamic per-step sorting and
mixed-model ordering adapters are not included. No performance claim is made.

Two DefaultConstructionHeuristicPhaseTest methods now cover all eight generated
comparator/factory alternatives on both scoring backends, preserving [2,1,0]
assignments and exact source objectives. Thirteen fixtures/helpers are attributed.
Original tests cover all placement modes, stable IDs, fresh-run reset, bounded
candidate cloning, atomic interrupted-scan retry, optional rejection, pins and
receiver ranges. Sorted orders are shared; unconsumed basic-domain tails are not
cloned into move values.

## Step-scoped construction priorities

Sorted basic/list providers now accept `with_step_sorting`, whose factory derives
candidate comparators from the current assignment once per completed-step counter.
Sequential basic entity keys and queued value keys retain phase order; destination
owners, basic candidate domains and pooled values refresh as applicable. Retrying
an interrupted scan with the same counter reuses its order. `reset` explicitly
rewinds the phase queue and clears the step cache, including when a new phase
starts at the same counter. Equal comparisons start from original encounter order.

Refresh prepares all orders before publication, so a comparator failure does not
advance the queue or partially replace its ordering. External callback effects
are not rolled back. Refresh time counts inside the cooperative search budget;
callbacks cannot be interrupted mid-call. Phase preparation still occurs outside
that budget. This API does not automatically choose cache scopes from annotations.

Eight additional DefaultConstructionHeuristicPhaseTest methods preserve every
configuration alternative: 23 per basic/global-or-entity-range method, 27 per
list/global-range method and 28 per list/entity-range method. Comparator/factory
variants run on both network and stream backends (404 complete construction runs).
They retain the source score arithmetic, seed-zero shuffle permutations, stable
entity/value rank identities, receiver ranges, early-pick settings and exact
per-entity assignment assertions. Step destination sorting covers entity-range
list configurations. Native tests additionally prove changing priorities affect
actual solver choices, and cover retries, resets, stable ties and failed refresh.
No Java configuration/reflection parity or new performance result is claimed.

## Mixed component construction providers

`MixedConstructionProvider` composes a basic provider and a list provider in either
existing mixed construction order. Each candidate changes only its originating
component while the complete mixed objective scores it. Sorted and step-sorted
providers can therefore share a construction phase without duplicating scoring,
validation, pin enforcement or rollback. This is sequential component scheduling;
atomic candidates that change both components still use a custom mixed provider.

The adapter owns its children and tracks separate completed-placement counters.
Only actual completion notifications advance them; discarded atomic scans retry
unchanged. Partial solves can resume with the same adapter even when the enclosing
solve counter restarts. For an independent phase, `reset_with` requires an explicit
child reset callback and then clears scheduling state. Fully assigned or exhausted
components are skipped; optional rejection passes control to the remaining child
when the rejecting child stops. Exhausted components are not reopened automatically
if another component's changes would make an optional move more attractive.

One candidate is prefetched to detect an empty child; remaining proposals are lazy.
Direct users must deliver completion notifications, and the adapter binds to one
problem. Native tests cover cross-component objectives on both solver backends,
heterogeneous value types, state-dependent sort factories, resumable budgets,
retries/reset, optional rejection, zero budget, lazy cursors, foreign-model rejection,
pinned initial values, prefixes and receiver ranges. This increment adds native
capability without claiming new upstream case coverage or performance results.

## Construction phase behavior cases

Nine more DefaultConstructionHeuristicPhaseTest methods exercise complete
construction-only solves on both network and stream backends: initialized values
and independent result storage, ordinary and optional-null pins, no entities,
entity-provided basic/list ranges and hard-score
penalties for optional assignment. Source arithmetic and final assignments are
preserved. The shared source helper's nonempty best-event assertion is retained,
including already initialized and empty solutions. The default helper uses a
constant-zero objective, not a uniqueness penalty.

Native entity indices and string fact identities replace Java entity/value objects.
Optional scored decisions explicitly use EvaluateUnassigned. List owner/index and
neighbor views derive from the assignment instead of annotated setters. No automatic reflection or annotation discovery is claimed.

The required-null pin exception method remains pending: native models deliberately
permit a pinned required null and return an incomplete partial construction result,
with no complete solution available. Existing native tests specify that behavior.
This batch does not relabel it as a passing exception test or alter partial solves.

## Scoped score consistency diagnostics

Basic, list and mixed ScoreDirectors expose `consistency_report`, returning typed
cached/recomputed totals and every differing named constraint contribution. Each
constraint is evaluated exactly once in that report; cancelling differences remain
visible. Totals retain structural state and the network's arithmetic policy.
Arithmetic or callback errors abort reporting without repairing caches. Callback
side effects are not rolled back. Equality compares actual score values, including
custom scores whose Debug text is indistinguishable.

`assert_consistent` now returns a boxed ScoreCorruption diagnostic through each
scoped scoring error type. Display includes working/recomputed scores and up to
five changed constraints; structured fields retain all differences. Candidate
checks also compare all individual contributions against the candidate's effective
cache, closing a total-only check that missed cancelling scope errors. Failing
candidate checks roll back assignment/cache transactions. Diagnostic evaluations
remain outside reported search work counts and run only when requested/enabled.
This change covers scoped networks; stream graph match/dataset diagnostics retain
their existing behavior. No performance claim or JVM environment-mode API is added.

Three DefaultSolverTest corrupted-easy-score cases cover initialized/uninitialized
assertion failures and unchecked successful execution. A deterministic changing
score replaces random corruption. Native labels and actual named-constraint evidence
replace Java labels and its analysis-unavailable fallback. Existing cancellation
corruption regressions now inspect complete structured differences. Original tests
cover report completeness, formatting truncation, structural totals, exact callback
counts, overflow recovery, typed equality, and candidate rollback in all three models.

## Full-solve neighborhood cases

Six additional DefaultSolverTest methods exercise default basic/list neighborhoods,
default candidates combined with an explicit swap selector, multiple fields per
entity, and two entity types with distinct value/reference domains. Each runs with
scoped and stream scoring. Native default change/swap candidates and neighborhood
unions replace Java preview configuration. The fixtures preserve their original
objectives: distinct forbidden identities and squared excess list length. The
multi-variable fixture retains the optional tertiary field and its separate domain.

Bounded hill climbing uses seed 20261007, 200 steps and 10,000 move evaluations,
with the source score target of zero. Checks retain successful solution and best-event
assertions and additionally require target attainment, valid assignments and unchanged
inputs. Stream full-objective adapters preserve variable positions during incremental
source replay. No Java default algorithm, random trace, preview-flag validation or
variable-neighborhood-descent claim is made by these cases.

## Variable-neighborhood descent configuration

`variable_neighborhood_descent` exposes first-improvement descent through the
existing incremental search engine. Enabled families are searched in their supplied
order; the first strict improvement is installed and scanning restarts from the
first family. Equal-score moves are rejected. Positive weight magnitudes do not
change priority; zero disables a family. Random union order is rejected. Empty
unions and exhausted non-improving neighborhoods stop without a selected move.
Ordinary search limits, targets, cancellation and assertion options remain available.
Overriding the returned policy fields or supplying a custom provider changes these
semantics. This is a named configuration of existing machinery, not a second loop.

The behavior follows the pinned public local-search documentation's VND algorithm
description. One additional DefaultSolverTest method preserves the ordinary VND
successful solve and best-event assertion with both scoring backends. JVM preview
configuration is not reproduced. Native regressions distinguish first-family
priority from highest-score selection, zero-weight disabling, equal-score stopping,
limits and return to single changes after a group move escapes a local trap.

## Constraint assertions

Opt-in `track_constraint` and `track_decimal_constraint` terminals retain signed
unweighted factors alongside tuples. `constraint_assertion` creates a detached
snapshot with separate penalty/reward counts, unweighted magnitudes, cached weighted
score, and caller-defined justifications. Factor callbacks run on tuple propagation,
not inspection. Numeric impact expectations ignore decimal scale. Cancelling impacts
still fail `NoImpact`; zero-factor matches retain justifications but affect neither
count. Contains/exact justification checks preserve occurrence multiplicity and allow
arbitrary order. An empty expectation requires no justifications in either mode.

Tracking adds a graph map node and uses ordinary graph weight overrides, pruning,
arithmetic and rollback. Disabled constraints have no matches. Raw factor signs
classify penalties/rewards even for negative weights. Foreign handles fail before
justification callbacks. Multi-node registration errors require discarding the builder.
This is a native assertion API, not Java ConstraintVerifier or shadow repair parity.

24 SingleConstraintAssertionTest methods cover 84 scalar assertion chains, relative
impact/score comparisons, String planning-ID unique pairs and six justification
methods, preserving failure messages and custom contexts. Fixtures retain entity
class filtering and givenSolution generation behavior. Original regressions cover
rollback, detached snapshots, callback counts, exact decimals, zero/duplicate matches,
minimum integer magnitude, disabled constraints and foreign handles. Shadow/list
variable consistency cases remain pending; benchmarks are not refreshed by this batch.

## Whole-score assertions

`score_assertion` captures a graph's cached total and all named constraint scores,
including disabled constraints. `ScoreAssertion::from_scores` accepts a director's
cached total and contributions, preserving structural penalties separately rather
than reconstructing a total by summing constraints. Duplicate names are rejected.
Snapshots are detached; capture both inputs from the same state. These APIs do not
recompute, repair facts or infer omitted constraints.

Exact/greater/less assertions use the score type's equality and order. Failure
objects retain typed expected/actual scores, custom context and every contribution;
Display limits contribution details to five. Decimal scores retain exact precision
and numeric equality across scales. Empty graphs produce a zero score.

Six MultiConstraintAssertionTest methods preserve combined scores, typed/relative
comparisons and assigned/all list-value filtering. Three additional single-constraint
methods cover the same list cases, and DuplicateConstraintAssertionTest covers the
original duplicate-name diagnostic. Native list membership replaces both inverse
shadow and no-inverse Java representations. Source verifier expectations use graph
constraint totals, excluding outer structural penalties. Original regressions cover
structural totals from a scoped director, callback-free detached graph snapshots,
disabled constraints, complete/truncated diagnostics, typed comparisons, duplicates,
empty graphs and exact fractional scores. Automatic shadow preparation is
still pending; no JVM verifier or reflection parity is claimed.

## Incremental joins and existence across tuple arities

33 additional stream methods cover entity/consecutive filters in Bi/Tri/Quad,
Cartesian/equality/composite/filtering joins, unchanged-key predicate changes, and
Tri/Quad existence and nonexistence with zero/one equality and filter conditions.
Each adaptation retains every source match and score assertion before and after
changes. Group removals can affect both base joins and existence witnesses in the
same graph transaction. Unique-pair identity order and optional string keys remain
explicit; null join keys retain their source equality behavior.

Tests use the existing graph implementation. No engine fix was needed. Native
retractions replace Java notifications, including the two Tri zero-join source cases
whose explicit removal notification accompanies removal from a different collection.
Existence match assertions preserve unordered source justification semantics and
occurrence counts, using tagged identities to distinguish fact classes. Join tests
additionally require ordered tuple payloads. Inapplicable Java reflection/selector
configuration and disabled methods receive no new credit. No benchmark refresh or
performance claim is attached to this test-only batch.

## Stream corruption evidence

`StreamGraph::consistency_report` rebuilds from source rows, compares the graph
score and every terminal's match identities/scores plus every captured dataset's
identities/payloads, then restores the original state. Reports retain typed cached
and recomputed totals and all changed node IDs; terminal differences include their
constraint names. Equal totals do not hide cancelling changes within a terminal or
across constraints, and dataset-only changes remain visible. Intermediate operator
caches and uncaptured terminal payloads are not directly compared, as before.

`assert_consistent` uses this same report and returns
`GraphError::CorruptedScore(Box<StreamScoreCorruption>)` with detached evidence.
The error source exposes the diagnostic. Display includes five node differences;
structured fields retain all of them. Failed transaction assertions still poison
the trial and restore its original state on drop. Callback panics and arithmetic
errors retain the original rollback behavior. Diagnostic evaluations stay outside
search counters; ordinary unchecked scoring adds no report work. External callback
side effects remain the caller's responsibility.

Three DefaultSolverTest incremental-calculator methods now have adaptations:
initialized count scoring and initialized/uninitialized corruption. Deterministic
changing scores replace random corruption, and named native match differences
replace Java diagnostic labels/registry details. Native regressions verify complete
and bounded evidence, cancellation in both totals and individual matches, dataset
payload changes, detached reports, rollback on corruption/panic/overflow, and trial
poisoning without diagnostic work inflating evaluation counts. This is diagnostic
functionality, not a benchmark or proof that an application objective is correct.

## Assigned-value join and existence updates

Ten additional Bi stream methods cover assigning, unassigning and atomically
replacing list values through joins and existence checks; indexed/unindexed group
predicates; and basic unassigned-value exclusion. Both sides of assigned-list joins
filter out unassigned values before owner-dependent callbacks. Native list sources
supply derived ownership and positions; composite moves replace paired Java list
notifications. All original match and score assertions remain, including equal-owner
cross-products and reassigning a previously removed value. Null basic values are
excluded from pair generation and existence witnesses; removing such a value leaves
the original nonexistence match intact. The existing graph implementation passes
these cases without further production changes.

## Grouped existence and tuple flattening

Seven additional Bi/Tri/Quad stream methods cover existence after two, three and
four distinct-count collectors, tuple-prefix-preserving flattening with duplicate
occurrences, and exclusion of unassigned facts from three-way existence witnesses.
Every original match and score assertion is retained through value changes and
removals. Additional native checks cover witness multiplicity, rollback, grouped
changes while no witness exists, and reactivation. Existing engine behavior passes
without production changes. Empty upstream Quad unassigned methods earn no credit.

## Read-only verification consistency

`verification_consistency` prepares effective inconsistency flags from explicit
entity/shadow-field metadata and a selected set of given IDs. Field-level cycles
and their downstream dependents infer missing flags; supplied flags override the
result only for their entity. Preparation neither evaluates suppliers nor writes
facts, shadows, assignments or flags. Independent fields on the same entities do
not create false cycles. Consumers apply assignment/consistency filtering when
feeding a scoring graph; unfiltered constraints may inspect all prepared flags.

A metadata census can include omitted referenced entities for diagnostics. Every
reachable cross-entity reference to a non-given entity is reported, deduplicated,
including indirect references; traversal stops where metadata is unavailable.
Structured errors retain every reference while display lists at most five.
Unknown given IDs, duplicate entity/field definitions and unknown given shadow
fields are rejected. Repeated given IDs are harmless. Explicit flags do not bypass
reference validation. The API provides native explicit metadata preparation, not
Java reflection, automatic solution extraction or shadow-variable repair.

Six complete SingleConstraintAssertion methods now preserve missing-reference,
nullable/explicit consistency, given-fact/given-solution, and unfiltered scoring
expectations. Native tests additionally cover false entity cycles, downstream
inference, detached snapshots, bounded diagnostics, invalid metadata and long
chains. Whole-list shadow-setting verifier methods remain pending.

## Explicit list-shadow preparation in verifier cases

The two remaining Single/MultiConstraintAssertion shadow-setting methods now use
VariableStateRegistry and ListShadowSession for explicit structural/cascade
preparation before scoring. Native fields receive owner, index, previous, next
and cascade results; all source penalty10, reward20, justification(-10,value0) and
combined score10 assertions are preserved. Ordinary verification still leaves
supplied values untouched. This adapter uses explicit native field assignments;
it does not provide reflective setter discovery or Java setter-history parity.

Three Uni stream-selection methods also preserve every match/score observation
before, during and after list-induced dependency cycles. Actual list positions
supply shadow dependencies, verification_consistency computes flags, and typed
stream filters distinguish assigned, including-unassigned and unfiltered facts.
Two source methods write e1 twice at the last transition; the native cases retain
that final state (dependency assigned, dependent unassigned) instead of changing
the source intent. Native atomic snapshot updates replace notifications followed
by working-solution resets. No additional production changes were necessary.

## Lazy indexed candidate groups

`LazyConcat` combines borrowed slices, owned vectors and deferred candidate groups
with advertised lengths. Length and bounds checks do not load groups. Indexed or
iterated access loads only the selected group, caches its contents, and borrows
items without requiring Clone. Seeded sampling without replacement composes the
native sparse index sampler and preserves duplicate occurrences. Length overflow
leaves the collection unchanged. A supplier length mismatch is cached as a typed
error without shifting later indices; a panicking supplier can be retried.

Seventeen ComposingList methods preserve complete ordered-content, indexing,
iteration, laziness, caching and bounds assertions. Negative Java indexing remains
pending because the native API takes usize. Four native regressions additionally
cover selective sampling, duplicate occurrences, non-Clone borrowed values,
reverse iteration, malformed suppliers, panic retry and total-length overflow.
This adds an indexed candidate collection; no join-node rewiring or speedup claim
is made, and no previous benchmark measurement has been refreshed.

## Mutable indexed candidate queries

CandidateIndex stores stable IDs in dense keyed buckets and supports insertion,
removal, replacement/rekeying, borrowed lookup and bucket queries. A position map
keeps removals valid after swapping in a bucket's final entry. Empty buckets are
removed. Duplicate insertion and unknown removal/replacement are typed errors.
Payloads require neither Clone nor Eq. Keys are captured explicitly; callers
synchronize this standalone neighborhood component with working assignments.

Bucket queries support finite sampling without replacement and indefinite seeded
sampling with replacement. Multi-key queries deduplicate repeated keys and compose
borrowed LazyConcat groups; range queries use native ordered-key bounds. Both
support global occurrence sampling. repeating_candidates also samples composed
queries without flattening, preserving lazy supplier/error behavior. Borrowing
prevents index mutation while query results remain live. Removal may change bucket
order, and no Java PRNG trace or storage-layout parity is claimed.

Fifteen EqualIndexer and ten remaining EqualsAndComparisonIndexer methods now
preserve every original match/size/exhaustion/filter/repetition assertion, including
inclusive comparison boundaries and flipped >= queries. Four native regressions
cover 2000 mutations against a reference map, non-Clone duplicate payloads, range
and multi-key membership, failure preservation, cleanup and repeated lazy errors.
Existing stream nodes have not been rewired; no solver speedup is claimed.

The native MembershipCandidateIndex adds mutable captured multi-key candidate
rows, explicit tuple-key partitions, removal/replacement, and borrowed query views.
Queries deduplicate candidate identities across overlapping and repeated keys;
unique and repeating seeded sampling use the resulting identity set. Empty-key
rows remain retained without matching. Payloads require neither Clone nor Eq.
Queries eagerly store deduplicated references; this does not establish Java lazy
first-draw allocation behavior or automatic stream-node maintenance. Callers must
update the index when their working assignments change.

Twenty-five additional Containing/ContainedIn/ContainingAnyOfIndexerTest methods
preserve their complete observable assertions, including composite queries,
duplicate insertion identity, missing-key exhaustion, seeds 0..9 drains, and 30
repeated draws. Native upsert replaces the same ID rather than retaining duplicate
insertion handles. Native Iterator None replaces Java exhaustion exceptions;
PRNG traces are not compared. The twenty first-draw trials use seeds derived from
native root 0. The source single-draw method checks membership, not allocation;
its name is retained without claiming equivalent internal removedSet behavior.
Earlier credited membership methods remain unchanged. Native regressions separately
cover 2,000 updates against a scan model, empty versus null keys, non-Clone payloads,
panicking key input, and overlap-independent sampling. No performance claim is made.

NeighborhoodTester now supplies reusable bounded membership assertions over shared
working state. Every assertion asks its provider for a fresh lazy iterator, so
committed changes are visible and temporary rollback leaves enumeration unchanged.
Derived within contexts retain independent limits while sharing state. The helper
accepts arbitrary native state/providers and delegates execution to existing move
and score-director guards through synchronous update callbacks. It does not infer
Java metamodels, build providers, or implement move execution itself.

All five NeighborhoodTesterTest methods have complete adaptations with two test
fixtures. Native typed Missing/Unexpected errors replace AssertionError; the native
default limit is 10000, without claiming Java default equivalence. No-op exclusions
and initialized/uninitialized source states are preserved. Three original cases
add exact pull-budget/early-stop checks on infinite providers, provider failure
propagation, empty requests, and shared score-director commit/panic rollback.
A successful produces_none_of checks only the bounded prefix. Reentrant mutable
state access is unsupported; this is a single-threaded testing helper.

CollectorSession now owns captured reversible-collector contributions behind
session-scoped explicit removal handles. Duplicate values remain separate slots;
insert_with and replace_with can map working facts once using a stable ordinal.
Removal uses the captured old contribution, never a recomputed mutable fact.
Monotonic ordinals are suitable for standalone list encounter order and are never
reused after clear. Foreign/removed handles and delegate rejection are typed errors.
Mutations stage the delegate collector, preserving both captured rows and aggregate
on ordinary mapping/collector panic or rejected operations, under the existing
ReversibleCollector isolation contract. Inputs need neither Clone nor Eq.

All six NeighborhoodsCollectorsTest methods preserve their ordered list, finish
length, paired-list and full removal assertions. Existing list, finishing and pair
collectors are reused; explicit Bi input projection replaces the source null-view
mapper. This does not change graph collection's stable-ID ordering. Three native
regressions exercise duplicate slots and immutable output snapshots, changed source
facts, scoped/stale handles, clear/reuse, non-Clone payloads, and failure isolation.

Prepared atomic list edits now offer ListNotificationScope::MovableSuffix, emitting
[pinned_prefix, old_length) before and [pinned_prefix, new_length) after each changed
list. The default keeps minimal differing intervals. No-op and unaffected lists
remain silent. Undo reverses the extents; existing stale-plan validation, pin/range
checks, one flush, and observer-panic restoration remain in force. This explicit
observer option does not alter score-director propagation or staged gather behavior.

list_k_opt_edge_move adds a deterministic edge-order constructor: start at the first
removed edge's first endpoint, traverse its first incident added edge, then rotate
the validated cycle to the first unpinned identity. Eight single-entity source k-opt
methods now preserve all edge inputs, exact results, feasibility success, pinned
prefix and complete before/after notification assertions. No expected output feeds
the constructor. This convention matches these source cases; general Java implicit
orientation and multi-entity partition choices remain unclaimed. Earlier output-only
fixtures are unchanged. Four native regressions cover multi-entity length-changing
notifications, rollback, no-ops, default scope, stale/pinned errors, callback panic,
and opposite edge-order orientations. No benchmark speedup is claimed.

comparison_candidates adds relation-oriented borrowed CandidateIndex queries using
JoinComparison and an explicit CandidateSide. Right-side storage reverses the
relation automatically. Equality and inclusive/exclusive ranges use native bounds,
without comparator negation, key arithmetic or payload copies. Occurrences remain
distinct and use the existing unique/repeating sampling over composed buckets.

Eleven remaining observable ComparisonIndexerTest methods now preserve removal,
boundary, empty/singleton exhaustion, ten-seed drains, repeated draw membership and
coverage, right-side flipping, and first-draw reachability. Native Ord replaces Java
integer comparator results (the extreme-sign fixture maps MIN/MAX to Ordering).
Native random traces differ; twenty first-draw seeds derive from root0. Tests that
assert Java storage modes or factory eligibility remain pending. The source comment
claiming key20 matches >25 is not its assertion; the actual strict predicate is
preserved. Two native regressions check all five relations on both sides over i64
extremes before/after rekeying, plus borrowed non-Clone payloads and explicit Option
ordering. Existing graph join behavior is unchanged; no speedup is claimed.

PhaseBestBuffer provides optional phase-level owned result delivery over native
SolverUpdate. An explicit scored-initial baseline suppresses unchanged initial
snapshots. The buffer keeps only the latest improvement in the observed phase,
returns a pending previous-phase snapshot on origin change, and requires a final
flush after a successful solve. It preserves native phase/completeness metadata;
raw immediate solver observers and scoring remain unchanged. Reset by constructing
a new buffer for each solve/problem revision. Ordinary snapshot-clone failure
preserves pending delivery. No asynchronous timing or Java default trace is claimed.

DefaultSolverTest.solve, solveStopsWhenUninitialized and
solveStopsWhenPartiallyInitialized now preserve all result and helper event-content
assertions through this explicit adapter, on both scoring backends. They retain the
source zero objective, two/five unassigned variables, two-value domains, no-op custom
phase, two construction steps and ten-step local bound. Complete construction
publishes exactly score0/initialized/construction-phase0; no-op uninitialized solving
publishes no improvement; interrupted construction retains a scored partial result
and nonempty delivery. Four native regressions cover coalescing, origin changes,
flush idempotence, ignored baselines, completeness and clone-failure preservation.

AssignmentSnapshot and reconcile_problem provide basic-variable warm starts across
changed domains and entity membership/order, using explicit stable variable IDs.
New pins take precedence; valid previous assignments survive by identity, using the
new domain's object for equal values. Missing entities/removed values use the new
initial state; old null assignments remain null. Snapshots and inputs stay owned
and unchanged. Duplicate IDs and wrong lengths fail explicitly. The new model keeps
optionality and pillar metadata; scoring is rebuilt, never copied from a prior score.

DefaultSolverTest.solveWithProblemChange now has an adaptation on both scoring
backends: four initialized entities/facts, zero score, bounded local search, daemon,
immediate queued fifth-fact addition, successful changed-solution consumption and
termination. Native receipt completion substitutes for the processed-change event
flag. This claims observed changed-result delivery, not Java director identity or
callback timing. Original integration also changes entity order/membership, domains
and pins across three consumed revisions while checking held snapshots remain intact.

ListAssignmentSnapshot/reconcile_list_problem add identity-based warm starts for
changed list models. Current full/prefix pins reserve ownership before any retained
rows; existing entities keep eligible surviving values in previous order. New
entities use current initial rows after retained ownership, independently of entity
order. Existing empty rows stay empty. Deleted owners, removed/ineligible values
and new unclaimed values remain unassigned for construction. Current registry objects
replace stale equal identities; optionality, ranges and dependencies are preserved.
Existing dependency repair clears cycles/downstream values and rejects pinned cycles.

Five original reconciliation tests and one live three-revision integration cover
these contracts. The integration runs list and mixed models with both scoring
backends, including reordered/deleted owners, new full/prefix pins, entity ranges,
missing-value construction and retained independent snapshots. Mixed models compose
the basic/list helpers before rebuilding scores. No new upstream method credit or
performance claim; Java score-director lifecycle parity remains pending.

list_directed_two_opt_plan adds explicit boundary-selected list reconnection.
Ascending cuts reverse a span; descending cuts reverse a cyclic movable span,
retaining the original first value as anchor only when no prefix is pinned.
Cross-entity cuts exchange suffixes, including empty suffix transfers. Plans validate
pins, ranges and dependencies before observation and preserve ordinary atomic undo.
Wrapped edits notify the whole movable suffix; others use changed intervals. Scoring
directors can consume the plan's movement; existing two-opt enumeration is unchanged.

Nine SelectorBasedTwoOptListMoveTest cases now preserve all output, doability and
before/after interval assertions, including unequal wrapped segments, pinned prefixes
and entity-provided ranges. Native ID/pin/range/lifecycle substitutions are recorded;
Java rebase and default callback ordering remain unclaimed. Four native regressions
cover all small cut pairs, empty tails, invalid/pinned/dependency rejection, observer
panic rollback and scoped/stream score-cache apply/undo/commit consistency. No speedup
claim or benchmark refresh.

DirectedTwoOptProvider connects wrapped two-opt plans to native local search via
MoveProvider for list and mixed models on both backends. list_directed_two_opt_moves
validates its starting state, lazily scans directed cut pairs followed by unordered
owner-pair tail exchanges, includes empty receiving tails, and omits no-op or invalid
final states. Distinct cuts may have the same successor. Temporary preparation uses
owned snapshots; no constant-time sampling or speedup is claimed. Mixed candidates
leave basic variables unchanged; callers may combine additional providers explicitly.

Four original tests establish a wrapped-only single-step improvement for list and
mixed search, both backends, retained basic assignments, pins/range/dependency filtering,
empty receivers, lazy opening and bounded scoring. These are native integration
checks and add no upstream-method credit. Built-in neighborhoods retain their prior
behavior; custom-provider selection remains explicit.

ListDestinationIndex caches value-to-receiver membership and insertion intervals
for a borrowed immutable list model/assignment. Single/group queries intersect
ranges, exclude full pins, respect prefix insertion bounds and include optional
unassignment exactly once. Owned ListDestinationSet results provide exact counts,
ordinal lookup, forward/reverse traversal and seeded uniform slot sampling. No value
clones or flattened per-slot storage are needed. Query results are snapshots, and
positions refer to the pre-removal assignment; source pin/dependency checks remain
with move construction. Empty groups select all movable entities; unknown values fail.

ElementDestinationSelectorTest.originalWithEntityValueRange now preserves all five
receiver sets/orders and both traversal directions; native rev over the same snapshot
replaces Java cursor rewind. Five native regressions cover group/range intersections,
optional/pinned/empty boundaries, uniform slot counts and singleton RNG stability,
no value clones, retained old snapshots, all small two-value range masks and indexed
group-gather local search on both scoring backends. Other source selector lifecycle,
random scheduling and mutable replay cases remain pending. No speedup claim.


All ten DefaultLocalSearchPhaseTest methods now exercise real basic/list solver phases
on both scoring backends: initialized/pinned/optional-null/empty inputs, explicit
tabu search, six-value/two-owner list construction followed by local search, and
local-only rejection of uninitialized required basics. All source result assertions
and the helper's nonempty-best-event check are retained. Native fixed slots carry
entity identities; zero objectives replace the dummy Java easy calculator. Native
HillClimbing and one-step VariableTabu settings make the unspecified/default phase
choices explicit; Java default acceptors, random traces and exact counters are not
claimed equivalent. Required pinned-null starts now identify the offending native
basic-variable slot through PinnedUnassignedVariable; the fixture maps this slot to
the source entity e3 and its value variable. Structured identity replaces Java
exception wording. This adds specific startup diagnostics, without changing the
search algorithm or adding a performance measurement.

Custom incremental calculators now have an owned working-solution session with
reset and before/after change callbacks. An optional registry retains distinct
match occurrences, per-constraint/overall ordinary scores, justifications and
explicit cancellation handles. Ten IncrementalScoreDirectorTest methods cover
policy fallback, registration, cancellation and reset. Native regressions compare
400 changes (including inverse updates for rejected trials) against full sums,
with matching enabled/disabled, and cover expired/foreign handles, atomic score
overflow and callback failure recovery. Calculator scores remain independent of
explanation totals. Callers supply complete change descriptors; dependency
inference and integration of this calculator backend into built-in heuristic
engines remain pending. The registry does not yet aggregate fact indictments.

The custom calculator backend now runs basic-variable greedy construction, local
search (including lazy custom providers) and custom commands through the existing
engines. CalculatorScoreDirector validates native moves and brackets whole moves
and inverse updates with sorted unique variable IDs. Dropped trials restore
assignments and notify the calculator; failed inverse callbacks invalidate the
backend until reset. Consistency checks use a separate factory-created calculator.
Original integration regressions compare construction and local-search results,
limits, randomized ties and step counts with scoped scoring, excluding opaque
constraint work counts. Each calculator score is one work unit; callbacks and
optional diagnostic recomputation are not separately counted. No upstream methods
are credited for these original tests. List/mixed models and direct solver-sequence
configuration for this backend remain pending. Candidate preparation copies the
assignment; no performance improvement is claimed or measured in this increment.

Custom calculators now also drive list and mixed construction, local search,
lazy move providers and custom commands. All three native calculator backends
share the same transaction/rollback implementation. List change descriptors are
sorted touched-entity indices; mixed descriptors keep basic variables and list
entities in separate vectors and callbacks observe both complete components.
Calculators tracking list membership/owner/index/neighbor facts inspect the
before/after contents of touched entities. They receive whole-move notifications,
not Java list-listener event sequences. Required unassigned basic/list values
supply native structural scores. Two AbstractScoreDirectorSemanticsTest methods
now preserve full/prefix list pinning scores through the incremental calculator
fixtures. Original regressions additionally compare 36 construction and36 local
search configurations with scoped backends, and exercise joint rollback, native
pin validation, match totals, custom commands/providers and failure recovery.
No performance claim is made.

Calculator-backed complete solves now accept SolverSequence/MixedSolverSequence
and SolverExtensions across basic, list and mixed models. They share orchestration
for construction, local search, repair, custom commands, enumeration and bounded
exact search, including provider-bound construction/local phases. Retained-best
handoffs, initialization events, shared limits and frozen-model handling follow
the existing engines. Each phase or exact/repair scoring session gets an independent
factory-created calculator; incremental state is reused within construction/local
candidate scans. Match collection is disabled in solve entry points. Original
regressions compare results, phase/work counters (excluding opaque constraint work)
and events across 75 plan/budget/strategy combinations, 15 shared-limit comparisons,
three provider/command compositions and factory-skipping/observer-cancellation cases.
These add no upstream method credit and establish no performance comparison.

Calculator registrations now expose owned ScoreExplanation snapshots through
explain_calculator_matches/default and basic/list/mixed calculator director
explain/default methods. These retain constraint names, occurrence IDs, empty
constraints and zero-score matches. Arbitrary stored justifications can map to
owned custom explanations without Clone; default explanations preserve fact order
and duplicates while per-fact indictments count each occurrence once. Checked
per-constraint and cross-constraint fact totals report overflow without changing
the registry. Snapshots include the cached structured director score separately
from registered ordinary contributions, with matches_agree explicitly reporting
partial explanation totals. No calculator scoring/reset callback runs to explain.
Match IDs remain suitable for diffs only within one registry generation; callers
must not diff across resets or separate registries. Solve-time match collection
remains disabled. Five original regressions exercise all model adapters, rollback,
reset, unhealthy/disabled access, partial totals, snapshots, diffs, callback panic
and checked aggregation. No additional upstream methods are credited.

IncrementalCalculator now has an optional fallible calculate_score_with_matches
hook, used by generic sessions and all calculator search/solve adapters. The
default delegates to calculate_score; an override can discover/register matches
while scoring or return an error. Matching-disabled calls still invoke the hook.
Calculators own earlier-match cancellation; no implicit clearing is performed.
Errors or panics invalidate the session until reset, including partially registered
explanations. Cached native director reads do not invoke scoring again.
Three additional AbstractScoreDirectorSemanticsTest methods now preserve the
incremental-calculator instantiation's independent sessions with entity removal/
addition, replacement solution weights1/2/0, and notified mutable weight1-to2.
The fixture retains its unweighted registration/weighted-return behavior through
the new hook. Owned generic solutions and native change descriptors replace Java
working-object mutation and notifications; these cases do not establish dynamic
native finite-domain model reconfiguration or factory descriptor identity.
Five original regressions cover all three native calculator directors, complete
plans, error/panic rollback, reset recovery and repeated-match cancellation.

Value-driven list swaps now accept independent caller-ordered value streams with
Cartesian or finite paired selection, optional explicit no-ops, assigned/unassigned
replacement, pin filtering, bidirectional receiving ranges and final dependency
validation. A bounded sampler draws ordered pairs with replacement; rejected pairs
consume attempts. It is not uniform over valid moves and does not reproduce Java
random sequences. ListValueSwapProvider reopens selection at each list/mixed search
step. Opening caches locations and filtered orders, without constructing moves;
candidate validation uses temporary assignments and makes no performance claim.
Six deterministic ListSwapMoveSelectorTest methods preserve all ordered move codes,
including positions, reverse duplicates, no-ops, pins, ranges and optional values.
Three additional source random-prefix fixtures preserve supplied mocked sequences
and output prefixes but add no method credit; never-ending metadata remains pending.
Four original regressions cover 27 partial assignments against existing swap
successors, sampler bounds/reproducibility/no opening randomness, dependency rejection,
invalid values and provider-driven list/mixed search improvements.

Value-driven list changes now cover assignment of missing required/optional values,
relocation and optional unassignment. ListValueChange snapshots preserve selected
pre-removal destinations while exposing native moves with adjusted post-removal
indices. Explicit Cartesian/paired selection, generated entity-start/after-anchor
destinations and bounded pair sampling respect full/prefix pins, receiving ranges
and final dependency constraints. Generated anchors omit the moved value itself;
optional unassignment follows anchored positions. ListValueChangeProvider works
with list/mixed construction and local-search provider bindings, rebuilding against
each current assignment. Opening validates inputs; candidates prepare temporary
assignments lazily. No performance or Java random-selector parity is claimed.
Six deterministic ListChangeMoveSelectorTest methods preserve every ordered source
move code and finite output count, including ephemeral same-list destinations.
Five original regressions compare both enumerators over54 required/optional partial
states, check actual execution/undo and retained pre-removal metadata, validation,
sampling reproducibility/bounds and complete list/mixed provider-driven solves.

SampledSubListProvider now binds bounded sublist relocation/reversal/unassignment
and asymmetric sublist exchange sampling to complete list and mixed solver plans.
It rebuilds indexes from each current assignment, seeds by completed step, preserves
sample order and duplicates, and rejects dependency violations and unchanged states
before scoring. Native attempt limits bound rejected work; an empty sample is not
proof of local optimality. Missing values require a separate construction phase.
Five original regressions cover sampler replay including seed overflow, partial
consumption and changed snapshots, pins/ranges/dependencies, empty/invalid inputs,
complete list/mixed plans with both policies, and clone costs. Validation now creates
a working assignment lazily on the first proposal and reuses it until traversal
ends. It saves one full-assignment clone per proposal after the first; ordinary
move-guard undo snapshots remain. No additional upstream method credit, Java
random trace parity or general timing improvement is claimed.

SlotReservationMap supplies bounded index remapping with implicit identities,
small sparse logs and one-way dense promotion. The test-specified transition is
min(16, slot_count/2) nonidentity entries; updates and identities consume no extra
log entry. Native Result errors validate bounds before mutation, and failed dense
allocation preserves existing mappings. Display sorts reservations by slot.
UniqueIndices now uses this representation for pools of at most64 occurrences;
larger/u128 virtual ranges keep sparse hashing. Sampling order and random draws
remain unchanged; no measured performance claim is made.
Twelve SlotReservationMapTest methods are fully adapted, including10,000 oracle
operations with all200 slots compared after each operation (native seeded RNG
replaces Java Random). Two signed-negative/exception source methods remain pending;
positive out-of-range assertions are recorded as a partial fixture only.
Additional tests cover counts/previous values/clear in both representations,
failed allocation, error atomicity and unique-sampling equivalence to swap-remove
across the small-pool boundary. Existing huge-range and solver sampling regressions
remain applicable. The map does not enforce unique logical indices by itself.

CandidateIndex equality-key buckets now use ScalingMap: sorted vector storage
promotes permanently to a tree after configured churn at scale or a size ceiling.
Lookup/mutation and borrowed range traversal retain native candidate behavior;
values and keys need no Clone bound in the map itself. Supplier callbacks run only
for absent keys and before promotion/mutation. Native defaults are threshold64,
churn tolerance8 and array ceiling1024; only64 is explicitly established by the
read source-test comments. The other tuning values are independent native choices.
Nine ScalingNavigableMapTest methods preserve all behavioral assertions with those
symbolic threshold bindings and a native seeded100,000-operation tree-map oracle.
The source array-reverse quirk remains pending: native iterators honor reversal in
both representations. Three original regressions cover range boundaries/errors,
double-ended iteration, supplier panic atomicity, clear and live candidate-index
rekey/removal/query/sample behavior across promotion. No performance measurement
or parity with unread upstream tuning constants is claimed.

EntitySwapProvider now exchanges aligned basic-variable columns of individual
entities, independently of equal-valued pillar grouping. Validated disjoint rows
retain caller identity/order. Unordered, Cartesian and paired orders support pins,
receiving domains, optional null transfer and explicit no-op metadata. Selected
pinned slots exclude their complete row; unselected variables are untouched.
Bounded sampling retains pair multiplicity and replays seed+step; rejected proposals
consume attempts. Basic/mixed provider bindings reuse ordinary transaction/scoring
engines; mixed list assignments are unchanged. Upper-bound pair counts deliberately
include candidates later rejected by self/range/no-op checks.
One SwapMoveSelectorTest method fully preserves optional-null entity ranges, ordered
outputs and advertised pair-space size. Four lifecycle-bearing source methods have
partial ordered/reopening fixtures only and remain pending. Five original tests
cover97 small optional/required states with execution/undo, multi-column range/pin
checks, malformed configurations, replay and both complete solver model families.
Java lifecycle and random no-change fallback policies are not claimed.

Six DefaultExhaustiveSearchPhaseTest methods now cover completing initialized
basic/list entities, full/prefix pins and per-entity list ranges, and empty entity
collections. Adaptations preserve the PlannerTestUtils best-event presence/absence
assertions and its zero objective. Native immutable entity slots replace named Java
beans; successful Results replace non-null solutions. Internal node restoration and
both pinned-null source methods remain pending. Completion is an explicit native
strategy; restricted completion exhaustion is not reported as global optimality.
Original regressions cover optional values, supplied phase state, insertion ordering,
dependencies, callback stop, controls, phase continuation and all nine scoring routes.

Eleven MultipleDelegateListTest methods now cover logical concatenation, ordered
iteration/collection, membership and duplicate positions, mutable subranges, and
repartitioning with all source offsets/sizes retained. Rust borrowed slices replace
Java list aliases; explicit lengths replace inclusive partition ends. Signed Java
exception messages, supplied-array identity, Java cursor conventions and unsupported
operations remain pending. One power-set helper fixture is recorded separately.
The borrowed view now supplies K-opt's combined values without an intermediate
payload clone. Native regressions verify borrowing, empty segments, invalid ranges,
non-Clone repartitioning and K-opt dependency-cycle rejection. No runtime performance
claim is made; previous benchmark measurements have not been refreshed.

Five partial FusedEqualIndexTest fixtures now exercise shared-partition row
membership/counts, composite equality keys, ordered suffix direction, cleanup and
lazy opposite-side observations. No additional full-method credit is claimed:
Java bucket-handle identity, explicit empty-bucket creation/cleanup and factory
invocation protocols remain pending. Native PartitionedJoinNode now co-locates both
sides in one persistent equality map and exposes borrowed prospective-row queries.
Existing full-pair oracle, panic, graph-rollback and solver integration regressions
continue to apply; new tests compare both query directions against full scans.

Six SetValueRangeTest methods now preserve every size, lookup, membership, ordered
iteration, scripted sampling and comparator-order assertion using native ordered
set domains. Three helper records retain attribution. Hash-indexed membership,
first-representative deduplication, shared payload set operations, snapshot sorting
and finite-model integration have independent regressions. EmptyValueRangeTest
remains pending; no Java singleton or unsupported-operation claim is made.

All70methods across Simple/HardSoft/HardMediumSoft/Bendable integer and decimal
ScoreDefinitionTest classes now have complete adaptations. The native definition
trait preserves zero/unit scores, labels, counts, bendable shape validation and
construction, all integer finite bound components, sanitized per-level division
and decimal negative-scale cases. All16decimal bendable construction shapes are
covered. The integer bendable source's repeated unit divisor is retained verbatim
in the test intent. General negative rounding, overflow errors and zero-level
shapes are documented native behavior with independent regressions. Finite integer
extrema are separate from the solver's existing infinite bounds.

Five additional ElementDestinationSelectorTest methods now preserve original and
scripted-random ordering/counts, empty selection and pinned/unpinned entity starts.
Six PlannerAssert helper records retain attribution. Native finite/repeating APIs
replace selector mode metadata. Receiver-group filtering, duplicate weighting,
seeded bounded sampling and complete list/mixed plans have independent regressions.
Java fallback-stream, mutable mimic-recorder and lifecycle callback cases remain
pending. The previously adapted originalWithEntityValueRange method is unchanged.

Eight PendingListChangeTrackerTest methods and four ListVariableBeforeChangeActionTest
methods now have complete adaptations. Native pending storage preserves action
identity, duplicate rejection, interleaved removal, reuse and clearing. Rebased
recordings preserve captured values, range notifications, insertion undo and open
bracket failures. Separate working Arc allocations verify actual rebasing through
WorkingObjectLookup. Java exception classes/callback names become native errors;
batch undo additionally emits a shadow refresh. Original regressions exercise
non-Clone action churn, interleaved open edits, memoized/type-changing lookups,
collisions, lookup failures/panics and independent source/target replay.

Five MoveTesterTest methods now preserve successful construction/binding, committed
swap, temporary swap/rotation observations and full restoration. Native model
references and isolated draft callbacks replace reflection metadata and mutation
observed through Java entity aliases. Four runtime-null argument cases remain
pending; Rust references/closures are non-null. One initialized cyclic solution
helper retains attribution. Original regressions additionally cover basic/list/mixed
composition, pins, partial lists, independent branches, rollback on errors/panics,
foreign-draft rejection and replay through incremental list scoring.

Thirty ScalingSequencedSetTest methods now preserve mutation results, membership,
ordered traversal, first/last, threshold transitions, null elements, order-neutral
equality/hash behavior and live reverse-view identity. Native Arc handles replace
object references; vectors/fixed arrays replace Java arrays, Option replaces empty
access exceptions, and Debug supplies printable contents. Equality comparisons use
differently ordered native sets; Java hash values are not reproduced. Six Java
unsupported-operation methods and signed/zero capacity rejection remain pending.
Native capacity zero is valid. Borrowed iterators reject edits until dropped.

The set additionally powers OrderedSetCollector over explicit occurrence IDs.
Distinct output order and representatives use the lowest current occurrence ID,
so graph rollback and full recomputation agree independently of event delivery.
This collector has original native coverage and earns no upstream method credit.

Eighty-five ElementAwareArrayList methods now have native EntryList adaptations.
Stable entry identities survive insertion and compaction; removed or foreign handles
cannot remove another entry. Gapped slots distinguish holes from live Option::None
payloads. Detached bidirectional cursors support insertion, removal and replacement;
other structural edits invalidate them, while replacement remains visible to other
cursors. Handles use local Rc ownership, need no payload Clone, and retain their last
value after removal; values already returned by reads remain snapshots. Slot count
is bounded to 1.25 times live length after edits, but allocation capacity may retain
its high-water mark. Indexed reads scan holes; insertion/compaction are linear.
Ten deterministic 10000-operation reference runs and a 20000-round growth guard
use native random streams, not identical Java traces. Two methods involving signed
negative indexes remain pending. These utility cases do not establish new solver
quality or performance results.

Indexed candidate pair views now borrow Cartesian products of matching key buckets
without materializing all pairs. Equality and all four ordered relations use exact
u128 pair counts; repeating draws and sparse unique permutations are uniform over
pair occurrences, including unequal bucket sizes. Duplicate identities on opposite
sides remain separate roles. Repeated blocks added manually intentionally add weight.
Construction metadata scales with matching bucket pairs, which can be quadratic in
key count; this is not a performance measurement. Overflow rejects an append without
changing the view. Mutable indexes require dropping and rebuilding borrowed views.
PairMoveProvider maps pairs into native moves with current assignment/step context
and a cap on inspected pairs, including rejected proposals. Its captured keys remain
immutable; assignment-dependent keys require rebuilding at the appropriate boundary.

Three PairFairnessBiasIT methods preserve both skewed fixtures, 620000draws per
indexed/cached case and the original five-sigma bound, plus complete unique-pair
drain. Native pair views, move providers and a materialized pair cache replace Java
stream/session/dataset wiring; no Java lifecycle equivalence is claimed. The
filtering-only deliberately biased case remains pending. One FilteredUniqueIteration
method additionally covers every rejecting-filter seed/count combination. Native
PRNG traces differ. Original regressions exercise Cartesian blocks with multiple
left rows, unique-draw position distributions, all ordered relations after mutations,
u128-scale virtual products, bounded filtering and solving through both scoring
backends. These checks make no runtime speed claim.

Four DefaultNeighborhoodProvider methods now preserve the exact ordered default
family sets for required/optional basic and list models. Native DefaultNeighborhood
implements the move-provider extension for basic, list and mixed models; its checked
opening methods expose errors. Explicit model optionality and DefaultMoveFamily
replace Java reflection metadata and runtime provider classes. The pinned public
neighborhoods documentation additionally specifies the behavioral defaults: changes
stay between assigned values, and 2-opt stays within one list. Separate assign and
unassign families handle optional values; tail swaps include reversing exchanges.

Native enumeration is finite, lazy and ordered by family. Optional seeded random
interleaving weights each live family equally and retains within-family order;
Java sampling rates/traces are not claimed. Basic models may mix required/optional
slots. Explicit aligned entity rows enable atomic multicolumn swaps; row pins and
receiving domains restrict these swaps. Mixed providers flatten component families.
List dependency cycles are rejected during generation when dependencies are enabled.
Required missing values still need construction initialization. Use this provider
through existing *_with_provider or custom-phase provider APIs. Original regressions
exercise actual moves and successful basic/list/mixed solves under both scoring
backends; no runtime speed claim is made.

Four EqualsFirstReorderTest methods now preserve stable equality-first ordering,
left/right mapping identities, unchanged-definition identity and normalized
equality/hash semantics. Native JoinProjection handles support heterogeneous Ord
key types in immutable JoinConditions. Applying conditions builds an indexed join
with all equalities as one partition key, the first inequality as an ordered key,
and additional inequalities as independent pair filters. Equivalent normalized
conditions using identical projection handles share nodes for identical ordered
parent ports in one builder. Stable order within each group is preserved; this is
not arbitrary commutative expression canonicalization. Empty conditions express a
Cartesian join. Native original regressions cover graph integration and transactions;
these four credits do not establish all upstream join-combiner semantics. Projection
callbacks must remain pure, as with existing graph APIs. Type-erased owned keys use
Rc storage and dynamic comparisons; no performance advantage is claimed or measured.

Compound conditions additionally support native existence and absence queries.
All equalities form a partition key and the first inequality uses the ordered join
index; remaining comparisons filter its candidates. The graph emits each qualifying
left identity once, preserves payload updates and changes right-driven visibility
only across zero matches. Equality-only and empty conjunctions use linear retained
count state. Ordered queries retain accepted pair IDs and can require quadratic
space for dense matches. Join, existence and absence registrations are distinct.
Original regressions cover 4,000 updates and rollbacks with activity pruning enabled
and disabled, repeated right matches, self joins, projection capture, all four
inequalities and callback panic recovery. This API increment adds no upstream
method credit or performance measurement.

Ten KOptUtilsTest methods now preserve removed/added edge identities, one/two/three
component label arrays, circular reversal/restoration and pure k-opt counts for
k=2..8. Native ListKOptTopology validates a degree-two reconnection, exposes all
components, and supplies the connected traversal used by single-list and repartition
move construction. Labels describe absolute positions within a range; the adapter
adds the source's unused zero sentinel. Pure topology counts use arbitrary precision
and distinct path endpoints; they do not count legal moves under pins, singleton
segments or model constraints. Circular reversal is shared with directed two-opt,
whose complete move plan still applies its existing anchor rotation and pin rules.

Nine selector-based rebase methods now preserve basic and list primitive/composite
identity mappings. Native basic/list/mixed rebase functions cache referenced identities,
reject collisions within independent namespaces, and retain composition shape and
list-relative coordinates. Destination model validation remains with normal apply;
list rebasing requires position-for-position corresponding starting assignments.
Six builtin composite methods preserve execution and temporary undo assertions using
the existing native MoveTester. Java reflective metadata is not reproduced.

Native SolutionManager now refreshes owned solution snapshots without running a
search. Everything, ShadowsOnly and ScoreOnly policies independently control
materialized shadows and stored scores; errors and callback panics publish neither
part of a requested update. Basic/list/mixed constructors use existing native
constraint networks, including initialization penalties. List constructors refresh
owner/index/previous/next fields in value-universe order and reject stale structural
shadows on score-only updates. Custom adapters can use the existing typed shadow
scheduler. They must explicitly define any additional declarative shadow validation.
This capability adds no complete upstream method credits: Java factory/manager
construction variants and full declarative diagnostic fixtures remain pending.

Runtime shadow diagnostics now expose owned entity-type/key/field identities for
strongly connected loops and their inconsistent downstream dependents separately.
Strict shadow refresh rejects inconsistent updates and restores prior cached values,
requested/applied dependencies and pending notifications. It integrates with native
SolutionManager adapters; permissive scoring updates remain available. This adds
no upstream method credits: complete SolutionManager fixture/factory and diagnostic
assertions have not been adapted.

Missing-source preflight now returns owned target/source field identities for all
missing direct references from registered shadow targets, deduplicated in source
order. Strict refresh uses this check automatically and resolves each path once
before evaluating any input or derived field. Missing entities are not recursively
expanded; getter/identity callback failures propagate without an incomplete report.
Ordinary permissive shadow updates preserve their existing error behavior. Native
regressions add no complete upstream method credits.

Mass-change moves can now be constructed from explicit slot groups and rebased
with validated target identity mappings. Duplicate input slots collapse in first
appearance order; empty/singleton groups are supported. MassChange equality and
hashing ignore member draw order, while executable composites retain it. Six more
Community MassChangeMoveTest methods preserve execution, required unassignment,
temporary undo, member metadata, equality/hash and rebasing assertions. Native
slots replace entity identities; the Java description method remains pending.

ListMassChangePlan now represents an identity-based list gather and resolves source
positions against the current assignment before applying. Insertion identity keeps
member order and destination; unassignment identity ignores member draw order.
Rebasing maps each distinct value and optional owner once, rejects value collisions,
and preserves the insertion boundary. Existing snapshot-bound ListMassChange stays
unchanged. Three Community MassListChangeMoveTest methods now preserve equality,
hashing and exact working-copy identity assertions. Original regressions cover
changed source positions, colliding hashes and lookup failure/null behavior.

Optional detailed assignment validators now preserve basic/list/mixed validation
errors and enrich range failures with the exact borrowed payload, basic slot or
list position, allowed values and unassignment policy. Global list membership and
per-entity ranges remain distinct. Shape, pin, duplicate and dependency errors
retain their original priority. Applications may supply entity/variable labels
and value renderers; sparse ranges display every member. Existing constructors
and search error types are unchanged. No complete upstream method credits are
added: the DefaultSolver invalid-initial-value fixtures and automatic named
solver-entry diagnostics remain pending.

Borrowed-specification constructors now build owned basic/list/mixed models with
initial-value range diagnostics. Failed construction retains references to the
original offending payload and range; successful models outlive their input.
Metadata validation and error priority remain those of native constructors. List
optionality, entity ranges and pins are retained. Four DefaultSolverTest invalid-
range methods are adapted through this explicit construction entry point, before
search can accept a model. Native slot/position identities plus application labels
replace reflective field identities; exact allowed members and nullable policy
replace Java endpoint message notation. Both mixed failure paths are preserved.
Existing constructors and automatic error formatting in other solver APIs remain
unchanged; these credits do not claim Java factory/reflection parity.

UnimprovedTimeLimit now exposes an explicit inactive/start/reset lifecycle and
monotonic retained-improvement timing. Inactive timers never expire, even at zero;
active timers expire at the deadline and late improvements cannot revive them.
The shared phase time budget uses this timer, so initialization-time polling no
longer arms stagnation implicitly. Total elapsed limits still include initialization.
Six UnimprovedTimeMillisSpentTerminationTest methods preserve all timestamp,
progress, lifecycle and negative-input assertions using explicit native events
instead of mocked Java scopes. Solve-wide cross-phase policy remains independent.

Development verification no longer treats an unrelated Box/Rc/Arc payload plus a
common method name as a dependency on every generic forwarding implementation.
The refinement applies only when the wrapped type parameter is directly bounded
by the same public trait. Named trait dependencies, factory return types, concrete
wrappers and universal implementations remain tracked. In the measured range-
collector target this reduces selected modules from164to11 and removes an unrelated
search-timer dependency; solver targets still select the timer. This is a selective
development heuristic, not compiler-level dependency analysis.

Diminished-returns defaults now provide the Community thirty-second window and
exact1/10000rate ratio. Ordinary and mixed sequence helpers fill only missing
local-search policies, preserving explicit overrides, other phase kinds and shared
limits. Each phase uses its existing local timer; this introduces no cross-phase
rate measurement. Three TerminationFactoryTest methods preserve default, additive
unit-duration and explicit-duration settings using typed native policies. Native
regressions cover idempotency, later-added phases and non-Clone score parameters.

Live problem changes can now be grouped into a `ProblemChangeBatch`: heterogeneous
owned callbacks run in order as one submitted revision, with no intervening queued
edit or solve. One receipt acknowledges delivery of that revision or a later one.
Errors and panics stop remaining edits with a one-based position; earlier mutations
are retained, so this is not transactional rollback. Cancellation is checked around
the group. Empty batches request another solve without editing the model. Pending
callbacks are disposed individually, including on failure, cancellation or rejection.
Two BasicPlumbingTerminationTest adaptations preserve deferred callbacks and
non-daemon restart behavior with real native jobs and bounded channel coordination.
Original tests cover concurrent submission, delivery acknowledgement, empty batches,
failure/panic receipts and disposal. No Java queue or score-director API parity claimed.

Variables and list entities now accept optional labels/value renderers for runtime
range diagnostics. Rejected assignments and moves retain an owned ValueRangeReport
through rollback, custom-phase errors, local-search errors and both scoring backends
for basic/list/mixed models. The original ModelError/ListError classification and
error source remain accessible. No Debug/Display/Clone/'static bound is added to
basic planning values. Formatters execute only for range errors; cloned/resumed
models retain metadata. Metadata errors, pins, duplicates and index checks keep
existing priority. Sparse allowed ranges are listed exactly. K-opt still filters
range-invalid candidates regardless of attached diagnostics. Two DefaultSolverTest
methods now exercise invalid moves after construction through actual native custom
and local-search phases; three nested test fixtures are attributed. Native checks
are always active, rather than gated by Java environment modes. Initial model-build
and borrowed assignment diagnostic helpers continue to work with enriched causes.

Custom phases now expose evaluate_inspected using existing list/mixed dependency
previews. Dependency-flawed candidates go to a separate callback without installation
or scoring; valid candidates use the ordinary transactional evaluator. Basic models
use ordinary evaluation, including partial states. Every candidate attempt consumes
one evaluation/move slot; unscored previews add zero constraint work. Domain/pin/index
errors still invalidate the phase, while caught handler panics allow continuation
with restored state. Existing phase limits apply before preview or callback execution.
Four original regressions cover all six backends, both handlers, budgets, rollback,
work counters and caught errors/panics. This is not a port of Java command-context
score-call counts, and earns no additional upstream method credit.

PillarSelectionPolicy now combines whole-pillar, arbitrary-subset and comparator-
ordered consecutive selection. Comparators receive aligned variable-slot rows;
stable sorting defines windows while each resulting pillar keeps canonical row
metadata. Selection stays lazy after sorting and never clones planning values.
Comparator handles compare by identity, and disabled bounds use None. The Community
unlimited convenience retains its exact 1..=2147483647 default; explicit policies
support native usize bounds. Seven SubPillarConfigPolicyTest methods are adapted.
Native tests verify aligned rows, stable ties, bounds, input preservation on comparator
panic, lazy large combinations, and comparator-driven local search with both basic
scoring backends. Policies compose with ordinary pillar change/swap constructors and
custom move providers; existing built-in selector configurations remain available.

Native PillarMoveProvider rebuilds value-tuple groups at every step and applies
independent comparator-aware policies to both sides of swaps. It also produces
single-column changes from selected subpillars. Model schemas, pinned rows,
optional null targets and every destination range are respected; mixed bindings
preserve list assignments. Group metadata is eager, but subsets/candidates are lazy;
impossible group-size combinations are rejected before subset enumeration. Both
swap orientations are retained. Public pillar_groups exposes checked current-state
groups without cloning planning values. Six original regressions cover four solver
paths, schema/pin/regrouping behavior, an independent successor oracle with custom
row order and asymmetric policies, and large lazy spaces/value-clone boundaries.
TEST-ONLY DefaultPillarSelectorTest1-340 informed gap investigation, but no source
method is credited: native per-step grouping does not reproduce Java lifecycle
notification counts or its sampled random traces. No copied test fixtures.

Native seeded pillar sampling draws directly from prepared row metadata without
enumerating combinations. Sizes are uniform within clipped bounds; subsets or
comparator-ordered windows are uniform conditional on size. Sparse partial
Fisher-Yates uses O(k) scratch entries for k selected rows. Public policy.sample
owns its metadata, retains duplicate draws and does not reproduce Java traces.
SampledPillarMoveProvider regroups each basic/mixed step using seed+step (wrapping),
with a bounded number of lazy attempts. Enabled families and eligible groups are
sampled uniformly; changes sample a column and first-row target, swaps sample
independent groups. Same-group swaps, unchanged targets and range-invalid draws
consume attempts without scoring. Mixed lists remain unchanged. An empty sample
can stop search without proving a local optimum. Six original tests verify seeded
replay, sampling support/distribution, sorted-row preparation, large subsets,
range/pin/schema filtering and actual two-step solving on four backend paths.
No additional upstream method/fixture credit; no benchmark speed claim.

BasicEntityRows and basic_entity_stream now project selected basic planning slots
into stable whole-entity facts. Rows may have different widths and caller-defined
column order; slots must be disjoint, in range and each row nonempty. Unselected
variables are ignored and pins do not remove facts. All source fields must exist;
assigned-only mode additionally requires every selected value, even for optional
variables. Including-unassigned mode retains null fields. This composes with basic
and mixed planning sources and existing graph transactions/rollback.
Two complete AbstractUniConstraintStreamTest methods preserve score and entity
matches for assigned-only and including-unassigned optional basic variables.
Four original regressions cover all partial multi-field assignments, commits/undo,
missing-source retraction/reinsertion, schema errors, ignored-variable callback
work, pinned facts and mixed joint-trial/panic rollback. No reflection or Java
notification-order equivalence claimed; no extra fixture files copied.

Twenty-two additional complete structural-activity methods now have adaptations:
all six ForEachUniNodeActivityTest, all seven IfExistsUniNodeActivityTest, all five
ConcatUniUniNodeActivityTest, three state-only SingleInputNodeActivityTest methods,
and JoinBiNodeActivityTest.inactiveWhenDownstreamInactive. Existing native source
emptiness/operator rules and enabled scoring paths already support these outcomes;
no solver implementation changed. Actual source facts and enabled/zero-weight sinks
replace mocked upstream capability/downstream activity, while assertions use settled
StreamActivity. Filter predicates do not erase structural source capability. The four
remaining methods in the latter two classes assert lifecycle notifications and stay
pending; this batch does not claim those notifications. No fixtures copied.

Native benchmark result analysis now supports owned exact cross-family numeric
scores (missing trailing levels treated as zero), ordinary-level totals without
integer overflow, failure/incomplete counts, total-score and worst-score rankings,
and floating per-level relative score changes. Decimal scale does not break
numeric ties. Percentages preserve cancellation before conversion; zero baseline
changes yield signed infinity and equal zeroes yield zero. Arithmetic dimensions
are checked, and IEEE infinity/NaN behavior is explicit. Run scheduling, dataset
identity alignment, timing collection and HTML reports remain separate concerns.
Seventeen complete Community benchmark methods have adaptations: all four each
from TotalScoreSolverRankingComparatorTest and WorstScoreSolverRankingComparatorTest,
ResilientScoreComparatorTest.compareTo, and all eight ScoreDifferencePercentageTest
methods. Native result reconstruction replaces Java mutation and report mocks;
unasserted helper metadata is omitted and the helper adaptation is attributed.
The source method named divide actually multiplies; its assertions are preserved.
Four original regressions additionally exercise exact overflowing integer totals,
failed/partial/empty outcomes, decimal equality and sums, extreme decimal scales,
tiny relative changes, zero/IEEE arithmetic, and real exhaustive-search results
under different evaluation budgets. No solver speed or runner parity claim.

The native serial benchmark runner now schedules warm-ups separately from measured
repetitions, matches deterministic seeds across configurations, clones each problem
before timing, records typed failures, and supports cancellation between trials.
Measured summaries feed the existing exact ranking API; timing summaries exclude
warm-ups and failed callbacks. Reports distinguish completion from success and
retain the exact trials completed before cancellation. Six original checks cover
measurement boundaries, repeatability, errors, cancellation, validation, and real
solver outcomes. The benchmark_configs example demonstrates three search budgets
on two problems. This native harness adds no upstream method credit and does not
claim Java benchmark/report timing or a measured Timefold speed comparison.

Relative benchmark standings now compare outcomes only within matching problem
keys, excluding comparisons inside one configuration. Equal-score bands and
per-configuration prefix counts compute wins/ties/losses without enumerating all
pairs. Quality order is wins, ties, fewer losses, then the existing exact summary
tie-breaker. Display ranks are one-based competition ranks and ties retain config
order. Repeated keys compare all cross-configuration pairs; missing keys add no
synthetic losses. Disjoint sets expose zero comparisons and use the numeric
summary fallback, which is not a matched performance comparison. Runner standings
require a complete schedule and match both problem and repetition, excluding warm-ups.
Ten complete upstream methods now have adaptations: all eight TotalRankSolverRankingWeightFactoryTest
methods and ScoreSubSingleBenchmarkRankingComparatorTest.compareTo plus
TotalScoreSingleBenchmarkRankingComparatorTest.compareTo. All score values,
initialization updates, group relations and assertions are preserved. Native
snapshot reconstruction replaces live Java ranking-weight references. Four original
checks cover an independent quadratic oracle, differing score scales, ties/invalid
configurations, runner integration/cancellation, and a 20,000-row tie group with
100 million comparisons per configuration. No statistical significance claim.

Three SolutionManager update-policy methods now use a native manager that owns a
declarative shadow session. All initial/result score and first-shadow nullability
assertions and generated seven-entity/five-value fixture are preserved; explicit
registration replaces Java descriptors and factory variants. Original regressions
exercise cache/output rollback after callback errors and panics, pending graph
edits, cycles, retries and policy callback exclusion. No Java lifecycle parity
or additional inconsistent/list method credit is claimed.

The SolutionManager list-refresh method additionally preserves all four built-in
shadows for six values across four lists, including one empty list. Native list
and mixed managers now report the first incorrect shadow field and owned expected/
actual contents, even for neighbors outside the model. Original checks cover
field precedence and rejection before scoring. The Java score-only error-message
method remains pending; native diagnostics use model indices, not Java object paths.

Native list and mixed declarative solution managers now combine built-in list
fields with custom graph projections in one atomic refresh. Assignment validation
precedes readers; score-only checks built-in fields and the supplied domain
validator without refreshing the graph. Original tests cover both model types,
changed rows/basic variables, callback failure rollback, invalid assignments,
cycles and recovery. This native API integration grants no upstream method credit.

ShadowCalculator now bridges declarative solution managers to native calculator
search for basic, list and mixed assignments. Reset/forward/inverse callbacks
refresh shadows, while scoring remains deferred; selected-move replay does not
rescore. The bridge clones its owned assignment per change, preserves graph caches,
requires reset after failures/panics, rejects adapter structural contributions and
does not advertise constraint-match explanations. Original search/rollback tests
verify small optima, cache/score consistency and callback accounting; no new
upstream method credit or performance claim is attached to this integration.

Declarative refresh now supports explicit AllowInconsistent scoring while strict
rejection remains default. Missing-source diagnostics and rollback still apply.
Native dependency callbacks bind list predecessor relations directly from current
assignments. One complete SolutionManager score-only declarative method preserves
the concurrent-visit fixture and HardSoft(-4,0) result. Original integration checks
show local search escaping the crossed cyclic start and exact search finding
HardSoft(0,-240), plus strict/tolerant transitions and failure rollback. This does
not claim Java alignment scheduling, supplier counts or annotation lifecycle parity.

Two strict inconsistent-solution update methods additionally preserve reversed
list/dependency fixtures and exact four-field loop membership. Caller-owned Rust
solution identity and unchanged contents replace the Java exception solution
property; typed errors replace class/message fragments. Both source methods use
ordinary update despite the shadow-only name. Java exception API parity is not
claimed; adapted assertions and fixture substitutions are recorded in provenance.

Native dependency bindings now share an immutable node catalog when created
after the same registrations. Adding nodes invalidates the builder cache for
future bindings only. Original tests verify linear key-copy count for a fixed
registered graph and retention of earlier bindings' node boundary; no wall-clock
performance or upstream method credit is claimed for this memory improvement.

Transitive entity preflight follows compiled declarative source paths even through
entities omitted from the supplied set. It reports owned field relationships,
deduplicates paths, terminates cycles and rejects ambiguous identities or mixed
schemas. It composes with solution-manager preflight before any refresh callback;
session field registration is still checked independently. One complete upstream
missing-referenced-entities case preserves the four-visit/two-owner graph and all
three required diagnostic relationships. Native typed errors replace Java text;
no static Java API or annotation lifecycle parity is claimed.

The complete score-only wrong-inverse-owner SolutionManager case preserves the
four-visit concurrent fixture and rejects b1 pointing at e2 while stored in e1.
Typed value/expected-owner/actual-owner fields replace Java diagnostic text;
caller data remains unchanged and no shadow reader executes.

Shadow managers can append preflight checks while retaining the list/mixed model
validation installed by their constructors. Checks run in order under every
update policy and stop before graph/score callbacks on failure. Explicit
replacement remains available. Original tests cover callback order, rejection,
panic recovery and preservation of initialized and uninitialized state.

Phase inspection exposes kind, initialization capability and the last phase
before the first improvement phase, for basic/list and mixed sequences. The
runtime initialization event uses the same boundary calculation. Metadata is
configuration-only: skipped, failed or budget-limited solves need not emit an
event. The complete seven-scenario SolverFactory initialization case is adapted;
original cases additionally cover native exact/repair phases, sequence edits,
all781 short sequences, runtime alignment and pinned-model skips. Native typed
metadata replaces Java phase-list/interface introspection.

Independent phase-score validation is now available through SolverExtensions for
all nine network, stream and calculator solve routes. A caller supplies a separate
score calculation per selected phase; mismatches and calculation errors abort
before handoff/initialization events. Initial-only and partial scored outputs are
covered; unscored outputs are skipped. Validation time consumes the shared deadline
without increasing search work counters. Prior best notifications are not undone.
This is a native runtime building block, not a claim that all Timefold environment
modes or their exact assertion schedules are implemented; those remain pending.

StableFlatMapNode and StreamGraphBuilder.flat_map_stable add keyed occurrence
matching across reorderings. Equal keys retain separate IDs in previous occurrence
order; new payloads update survivors, unmatched old occurrences retract, and new
occurrences receive fresh IDs within the input lifetime. Standalone enqueue/
propagate defers mapping and delivery and prepares whole batches before publishing.
Graph integration uses ordinary immediate routing, transactional rollback and
activity pruning. Both complete FlattenLastUniNodeTest methods now preserve all
deferred lifecycle and duplicate event assertions through a native consumer.
Existing positional expansion remains available; stable IDs do not encode order.

Verification input selection now follows referenced inherent members instead of
attaching every extension implementation to every use of its type. Plain child
module declarations do not themselves invalidate a job; referenced child symbols
and methods do. Attributed modules and literal includes remain dependencies, as
do implicit foreign-trait behavior and unresolved macro-generated members.
This is still a selective development heuristic, not Rust compiler dependency
analysis. Cache keys include the scanner, so this change requires fresh baselines
at the next relevant verification run; old records and benchmark dates are not
rewritten. No solver compatibility credits are added by this tooling change.

Compound JoinConditions now compose containing, contained-in and shared-key
collection joins with every declared equality and ordered comparison. Equalities
partition the membership index, and residual ordered predicates inspect matching
key/partition pairs only. Duplicate/shared keys retain the existing native
one-pair membership semantics. Callback-based collection variants build independent
nodes; ordinary projection-identity sharing still applies to the existing apply
method. The final three JoinerTypeTest methods now preserve all scalar/collection
inputs and boolean results through graph pair presence. Native regressions cover
all three directions, null elements, duplicate keys, replacements, removals,
rollback, pruning and projection failures against an independent full pair scan.
Existing documented Java membership multiplicity/null divergences remain unchanged.

Compound collection joins now also support Exists/NotExists in containing,
contained-in and shared-key directions. Equality partitions and collection postings
limit candidates; ordered residual predicates qualify witnesses. Each left row is
emitted once, regardless of duplicate keys or witness count. Left replacements
refresh visible payloads; removing the last qualifying witness changes visibility.
Empty collections survive NotExists. Accepted pair identities consume space
proportional to matching pairs; callback variants do not automatically share nodes.
Native regressions cover incremental updates, dropped transactions, self-joins,
activity pruning and projection/residual/downstream panic recovery. The existing
ordered existence adapter shares the witness reducer and its regressions pass.
No new upstream test methods are claimed by these original regression tests.

BatchedExistsNode adds explicit round boundaries to the native existence index.
Eager evaluation stages private roots during enqueue; deferred evaluation retains
inputs for propagation. Both evaluate in input order and emit at most one explicit
TupleLifecycleEvent per affected left identity, reflecting final round visibility.
Insert/update/retract distinctions are produced by the solver, not inferred by a
test adapter. Committed rows remain observable until propagation succeeds; pending
work can be discarded or retried after callback/clone failures. Transient changes
are coalesced downstream; this does not reduce underlying pair evaluation work or
change the immediate stream-graph scheduler. Equality buckets still prune work;
matching-pair storage can be quadratic. The complete six-round upstream existence
case is adapted for both native timing modes with strict lifecycle balance checks.
Java layer distances/internal scheduling are not reproduced.

BatchedJoinNode adds equality-indexed filtered join rounds with production
Insert/Update/Retract events. Deferred rounds coalesce each input ID to its last
value, update both sides, and deduplicate candidate pairs before predicates.
Eager rounds stage each input immediately; both publish only final pair payloads.
Preload temporarily selects eager evaluation on an empty node and restores the
configured mode after propagation. Pending rounds can be discarded; preparation
failures preserve committed state and pending inputs for retry. Accepted pairs
are stored explicitly, so dense joins require quadratic space. This standalone
policy does not change immediate stream-graph scheduling.
Three UnindexedJoinBiNodeTest methods are fully adapted: both payload refreshes
and preload/control behavior. The separate no-preload test explicitly requires
two predicate walks per pair; native deferred rounds intentionally evaluate each
candidate once. That method remains pending, with no compatibility credit.

Batched joins now integrate with transactional stream graphs and all three native
stream score directors. StreamBatch/apply_batch coalesce source identities and
schedule nodes topologically; join_batched settles both inputs together. Basic,
list and mixed initial/move updates opt into batches when an observed batched join
is declared. Activity transitions trigger one final-source rebuild. Checkpoint,
rollback, checked-score errors, scope restoration and saved-winner installation
include batched join state. Ordinary graphs keep sequential delivery; other
operators retain per-event semantics. Seven original regressions exercise this
runtime integration; no additional upstream method credit is claimed.

Batched existence now has a native transactional graph adapter, including
semi/anti modes, automatic score-director batching, final witness visibility,
source-activity rebuilds and rollback. This extends previously adapted standalone
existence behavior into the solver graph. Native integration regressions confer
no additional upstream-method credit; provenance and source-suite totals stay
unchanged.

Explicit coalesce_updates boundaries also publish only final per-identity derived
updates, allowing aggregate constraints to score a changed group once per batch.
Batch-enabled consistency checks now use batched source replay. Five native
integration regressions across these two extensions add no upstream-method credit.

Native assertion-enabled move selection now verifies restored scoring state after
every candidate rollback, including early selection and the final candidate before
winner installation. This applies to network, stream and calculator backends for
basic, list and mixed models. Broken calculator undo callbacks can no longer be
hidden by replaying a winner that repairs their cache. Diagnostic work remains
outside search evaluation counters. This strengthens `assert_scores`; it does not
claim full Java environment-mode scheduling or tracked-variable parity. Three
native fault-injection regressions add no upstream-method credit.

The incremental-stream performance adapter now supports native batched joins and
coalesced group scoring, with explicit immediate mode available for comparison.
Matched Java/Rust swap workloads complement the original change workloads.
Swap results must preserve pins and value multiplicities; small-case optimum
checks enumerate only assignments reachable by that neighborhood. Timed swap
workloads use seeded balanced colors, so the planted zero-score optimum remains
reachable. These original benchmark cases add no upstream-method credit.
The native `Neighborhood::Swap` currently enumerates both directed orders of each
pair. The pinned `SwapMoveSelectorTest.originalLeftEqualsRight` specifies one
unordered pair per same-selector pass. The initial directed-mode timings compare
engines under equal step limits, not identical candidate counts. The unique mode
below removes reverse pairs; do not infer pure propagation cost from solve timings.

`UniqueSwap` and `ChangeAndUniqueSwap` now provide lazy first-orientation pair
selection for basic variables and assigned list positions, including mixed models
and neighborhood unions. They retain pins, mutual ranges, selected variable order,
change-value ordering, and optional list replacements. Existing `Swap` and
`ChangeAndSwap` retain directed pairs. This complements the existing unordered
entity-row provider without building row schemas for ordinary single-slot swaps.
Removing reverse duplicates changes finite-budget and sampling behavior; the new
modes are explicit opt-ins. Four native regressions cover enumeration and all nine
model/scoring combinations with score assertions. They add no upstream-method
credit. Stream benchmark `swap` now uses unique pairs; `swap-directed` preserves
the prior adapter mode. A fresh comparison validates 128 generated cases and all 8
timed workloads with matching Java/Rust scores. Observed swap solve times are
roughly half those in the preceding directed run under the same step/warmup/sample
settings; Java remains faster. These limited-sample development measurements are
not a general speed guarantee or additional upstream-method credit.

`JoinNode::apply_batch` now stages both input indexes and publishes one final
change per affected pair for equality and all four ordered comparisons. The join
retains input rows/indexes without a persistent expanded-pair cache; dense rounds
can still require quadratic temporary candidate/output buffers. Repeated IDs use
the last input before projection. Callback, clone, ordering and input-iterator
panics leave committed indexes unchanged, excluding panicking destructors and
external callback side effects.
`join_equal_batched` and `join_compare_batched` integrate this path with graph
activity, checkpoints, rollback and score-director batching. The filtered
`join_batched` operator retains its residual-predicate behavior and now uses a
transient ordered candidate set. The stream benchmark uses compact equality joins.
Six original regressions cover independent scans, immediate/batched interoperation,
self joins, rollback, faults and payload retention; no upstream-method credit.
A fresh stream comparison retains matching Java/Rust scores in all 128 generated
validation cases and all 8 timed workloads. Scoring-callback counts match the prior
Rust run, while observed solve times are 28–44% lower with compact indexed batches.
These development measurements use limited warmup/samples; Java remains faster.

Native solve-wide ConstructionSteps/CombinedSteps conditions now bound construction
placements across phases and optionally combine them with local/repair steps. All
three models and all three scoring backends, including custom construction providers,
share the controls. Solve/phase/job work exposes construction_steps separately from
search stagnation. Original regressions cover these behaviors; no additional upstream
method credit or Java step/lifecycle equivalence is claimed.

Native EntityConstruction now provides separate-column or atomic joint-row
initialization without caller-written candidate factories. It supports all three
basic/mixed scoring backends and composes with existing mixed list construction.
Original regressions cover cross-column objectives, nullable subsets, pins,
interruption, ordering and lazy products; no additional upstream method credit.

Native SolverLimits::assert_phase_scores now automatically rebuilds scoring for
each scored phase output in all nine model/backend paths, before handoff and
initialization notification. Original corruption, budget, event, failure and
controlled-time regressions verify the behavior. It remains opt-in and is not
full Timefold environment-mode compatibility; no additional source method credit.

Native move providers now receive automatic phase and candidate-scan boundaries
from construction and local search in all nine solver backends. Notifications
forward through filters, enabled union children, mixed component providers,
construction queues and placement factories. Optional phase resets preserve the
existing default of resumable provider state. Returned errors close open scopes;
panic unwinding skips user callbacks. These are native events and do not claim Java selector lifecycle equivalence;
they confer no additional upstream test credit.

CachedMoveProvider now connects native selection caches to real construction and
local search, with solve/phase/step scope, bounded weighted draws and current-assignment
move mapping. Shared bindings let a single borrowed provider retain a solve cache
across phases. All nine backends own one solve lifecycle per distinct registered
provider, closing on returned errors; standalone engines use one-phase sessions.
Native integration tests cover these paths; upstream method coverage is unchanged.

Four additional SwapMoveSelectorTest methods now have full adaptations: original
shared and separate entity sources, empty shared source, and empty right source.
They preserve all ordered codes, finite size/exhaustion and six lifecycle counters
across two phases with two then three scans. Native PairSelectionSource and
CachedMoveProvider replace Java selector identity and mocked scopes. The older
entity_swap_cases partial fixtures remain as separate enumeration checks; their
provenance now points to the full adaptations. Random/mimic selector cases remain
pending.

Three further SwapMoveSelectorTest methods preserve finite dependent partner
traversal (including backward cursor movement), filtered random self fallback and
disjoint singleton-domain fallback identity. ReachableEntitySwapProvider offers
a separate bounded dependent sampling mode for basic and mixed solving. It checks
all incoming values and at least one aligned domain overlap, excludes pinned rows
and retains duplicate input occurrences. Native seeds, explicit probe/output budgets
and immutable left-bound views replace Java random/lifecycle APIs; these cases do
not establish infinite iterator or PRNG trace equivalence. The two remaining
random entity-range source methods are still pending.

PlacementQueueProvider now connects the existing phase-cached placement sources
to actual basic, list and mixed construction engines. It owns lazy source lifecycle
scopes, maps tokens using current assignments, skips empty/rejected pools, replays
within a scan and stops cyclic scheduling after a full pass without installation.
Every phase rebuilds keys; this adapter does not resume the old queue cursor across
phase boundaries. Seven original integration/regression tests cover the bridge.
These native solver integrations add no upstream method credits.

Three further construction/exhaustive phase methods now preserve required pinned
null rejection with the offending entity/variable identity, and optional pinned
null acceptance with exact zero score and best-event presence. Adaptations enable
`SolverLimits::reject_unassigned_pins` explicitly; its native default remains false
for partial workflows. All nine solver model/backend combinations share the startup
policy. Typed basic-slot diagnostics replace Java exception text. Exact-search
internal node replay and expansion-step trace methods remain pending.

Two DefaultPillarSelectorTest methods now preserve all ordered whole-pillar groups,
assignment changes between steps, finite sizes/exhaustion and six child lifecycle
event counts across two phases. GroupedSelectionSource adds bounded stable grouping
and snapshot reuse for selector composition; CachedMoveProvider can use it for
current-assignment pillar moves. Two additional methods preserve shared demand
counts, including phase cleanup when StepEnded is skipped. SharedSelectionPool provides explicit keyed leases, and
SharedStepSelectionSource shares bounded collected snapshots across step-scoped
selectors. Each view forwards its own child lifecycle; callers must include source
configuration and immutable input revision in shared keys. Native regressions
exercise independent cursors, ownership, failure recovery and two actual solver
providers sharing one source collection per scan. Existing built-in pillar
providers are unchanged.

Four further DefaultPillarSelectorTest methods preserve scripted random subgroup
selection, fixed sizes, ordered consecutive members and empty random sources.
Together with the previous four adaptations, all eight methods in this source
class now have complete native adaptations. Injected bounded draws preserve the
asserted prefixes and metadata; native seeded streams do not promise Java PRNG
identity. SubgroupSelectionSource caches bounded finite group inputs and samples
directly without enumerating subsets. LimitedSelectionSource bounds candidate
materialization for CachedMoveProvider and now preserves zero size for an empty
never-ending source. Native tests include full small-subset draw multiplicities,
invalid draw/cap handling, non-Clone occurrence ownership, seeded continuation,
and actual scored solver execution including an empty source. No performance
comparison was refreshed for this increment.

Four ChangeMoveSelectorTest methods now preserve all finite and random output
prefixes, empty dimensions, current-assignment changes without dropping non-doable
targets, and both children's lifecycle counts across two phases/five steps.
KeyedSelectionSource collects bounded per-key value pools through PlacementSource,
with original key-major traversal or direct random key/value draws. Original-mode
snapshots can feed phase-cached move providers while mapping against current
assignments. Native random sampling chooses eligible keys then their values;
the source adaptation explicitly supplies the mocked cyclic entity/first-value
policies and does not claim Java seeded traces. Separate native tests cover unequal
per-key pools, duplicate occurrence identity, caps/errors, held non-Clone snapshots,
seed replay and actual scored search with entity-specific value domains.

List move transactions now snapshot only referenced entities, once each across
nested composites. They restore saved lists without value cloning on rejection,
validation error or ordinary panic unwinding. The full pre-move `original()` view
is materialized lazily and cached; `original_list(entity)` borrows pre-move contents
without copying unrelated entities. Both scoped and stream list scoring discover
value dependencies from the touched lists before/after the move, avoiding the full
historical assignment. External model validation remains complete; validated move
layout checks now inspect touched entities. Observed edits and mixed-model outer
transactions may still take full snapshots. Five original regressions cover all move kinds, commit/rollback,
late errors, cloning panics, lazy snapshot recovery, Send/Sync preservation and
both list scoring backends. These checks add no upstream method credit. Native
microbenchmarks are separate from the existing Java performance comparisons.

List mutation now preserves registry membership and uniqueness through primitive
operation checks, validates final pins/receiving ranges on touched entities and
retains global dependency-cycle validation. Public apply/initialization/reset
boundaries still validate the full input. All six list/mixed scoring backends
reuse their exclusively owned validated assignments between moves; bounded sublist
sampling reuses its validated scratch assignment. Mixed value-dependency discovery
also inspects only touched old/new lists. Three original regressions confirm no
unrelated value comparisons across all six backends, invalid external input remains
rejected, and final pin/range/global-cycle violations roll back atomically while
restored intermediate violations remain permitted. Existing transaction, calculator,
dependency and sampled-provider regressions also pass. No upstream credits added.

The remaining seven ScoreUtil decimal utility methods now have complete native
adaptations: nonnegative output scale and floor-rounded multiplication, division,
and integral powers, including negative scientific-notation scales. Exact decimal
operands replace terminating doubles; exponents are nonnegative integers. This
establishes these cases, not general floating-point conversion or fractional powers.
Six EmptyValueRange methods use an empty native enumerated range. Missing indexed
values return None; sorting returns an immutable empty copy without invoking its
comparator. Java exception types and singleton identity are not native API promises.

EntryList now exposes stable first/last/neighbor handles, insertion before the
first entry or after a live handle, borrowed entry traversal, and clearing with
optional callbacks. Native neighbor queries skip gaps and reject removed/foreign
handles. Callback clearing snapshots original identities, visits each once even
if callbacks remove entries, and retains newly inserted entries. Callback panics
leave completed removals and other callback mutations in place. Ten TupleList
and five ElementAwareLinkedList methods preserve all their observable assertions;
Java intrusive storage slots and randomized-iterator traces remain pending.
This extends the native collection API; it does not change stream-graph storage.

Basic-variable transactions now validate public input once and validate only
written slots in the final state. Owning basic and mixed scoped/stream/calculator
score directors reuse their validated state. Change/swap leaves avoid allocating
a touched-slot set; composites deduplicate journaled slots and preserve ascending
validation-error priority. Raw move index checks, rollback, temporary unassignment,
and final-only domain/pin rules remain intact. Constructors and calculator resets
still validate all external values. Three original regressions cover six backends,
unrelated comparison work, invalid input, final failures and repaired intermediate
composites. Stable Clone/Eq value identity remains required.
Three additional SwapMoveProvider methods preserve complete multivariable move
inclusion/exclusion cases, including a forbidden whole swap with one legal column
and optional missing third values. Explicit selected rows replace Java metadata.

Batched indexed joins coalesce inputs and deduplicate candidate pairs using sorted
buffers, retaining first identity representatives and last input values. Stable
index roots avoid repeating old/new candidate enumeration when membership is
unchanged. Equality can reuse one bucket even when unrelated buckets change;
ordered comparisons require the opposing index to be unchanged. Final rows still
refresh payloads and changed memberships retain old/new enumeration. New native
regressions cover identity clone work and repeated updates/retractions; existing
join-round, panic, snapshot and graph rollback cases apply. Sorted buffers can
retain duplicate pair IDs until deduplication; dense rounds remain quadratic.
No upstream credits.

A native configured-selection pipeline now resolves inherited order and minimum
cache scopes, validates sorting/weighting requirements before constructing its
source, and executes original, random, sorted, shuffled and probabilistic modes.
Source and order caches may have different lifetimes: stable phase values can be
reshuffled, reweighted or sorted with a new comparator each step. Source callbacks
receive every valid lifecycle event once; failed cache/order builds leave output
unavailable, with explicit scope closure and later rebuilding for recovery.
Filtering composes before caching. Uncached selection delegates to the source
factory's requested mode; cached sources must enumerate finite occurrences.
The pipeline implements SelectionSource and integrates with CachedMoveProvider;
cache/order builds share payload storage and only yielded owned items are cloned.

All 43 Entity/Value/MoveSelectorFactory methods now have native adaptations.
Explicit source policies and executable plans replace reflective descriptor
configuration and Java wrapper-type assertions. Both shared solution ranges and
separate entity-context value sources support phase sorting. Optional list value
filtering selects assigned/unassigned/all modes for optional lists and bypasses
those filters for required lists. Reflective Java instantiation remains outside
the native API; callers supply the source type and cache policy explicitly.

ConfiguredSelectionPolicy is the distinct public cache policy name; it avoids
colliding with the private construction selection-policy trait in verification
input discovery. Configured selection now supports per-opening yielded-item
limits, including zero without opening a direct source, while ending_iter visits
all finite occurrences independent of that cap or random weights. Unknown source
cardinality remains unknown; capped random caches report exact draw counts only
when availability is known. Ending traversal rejects uncached repeating sources.

SelectionRegistry provides typed named recording and replay handles. The recording
owner alone receives lifecycle events; replay inherits its behavior and rejects
configuration overrides/callbacks. Duplicate/empty names fail before configured
source construction; failed construction does not publish a name. Removing a name
leaves previously acquired replay handles attached to their original owner.
SubListSource connects native bounded span indexes to configured/recorded sources
without materializing the full span catalog in uncached mode. Bounds, whole pins
and pinned prefixes apply. Three sublist factory cases and the finite random-limit
ending-iterator case have complete adaptations. The sublist factory rejection of
an explicit maximum-size property combined with a mimic reference remains pending;
that combined reflective configuration shape has no native counterpart here.

Sublist selection sources now compose independent span and destination streams,
with entity-conditioned span draws, separate left/right bounds, optional reversal,
raw PRE-removal proposal metadata, and conversion to native POST-removal moves.
Lifecycle events propagate once per child; bounded entity reads include rejected
owners. Sources reopen against the current validated assignment. Recording/replay
and list/mixed local-search integration have original regressions. Catalog counts
are separate from traversal sizes: owner restrictions and finite budgets can yield
fewer proposals, and invalid/no-op combinations remain subject to move validation.
Six additional RandomSubListChange/SwapMoveSelector methods preserve cardinality,
repeating metadata and empty selection assertions. The earlier ten sequence-only
fixtures remain partial; this batch does not establish full Java lifecycle or
random-trace equivalence. No new performance measurement is claimed.

Five RandomAccessLeafIndexer methods now have complete adaptations using a
single unit-key CandidateIndex bucket. They preserve cardinality, tuple membership,
removal failure, and the shared 30-draw seeded sampling assertion. Native occurrence
IDs replace Java tuple entry handles; random traces are not required to match.
Batch propagation additionally reuses per-node routing buffers for batches with
multiple source changes. Single-change batches keep compact local queues. Both
process only queued nodes in topological order. Buffer reuse does not retain event payloads
between batches. Original regressions cover commit/rollback reuse, failed batches,
panic recovery, ordering and payload release. This optimization earns no additional
upstream-method credit.


The remaining three preview ChangeMoveProvider methods now preserve all move
membership/no-op assertions for incomplete solution ranges, per-entity ranges and
optional per-entity ranges with crossing disabled. The two entity-range generators
append each current value if it is absent from the range; their comments claiming
a current value outside the range are stale. Native domains preserve the actual
fixture values instead of weakening model validation. Ten methods in this class
now have complete adaptations; no production behavior needed changing for these
three cases. The unrelated fact in the incomplete-range case stays outside the
native variable domains. No Java reflection or registration API parity is claimed.

Graph transactions now reuse visited-node and checkpoint-journal buffers after
commit or rollback. Nested consistency checks use separate workspaces; retained
capacity follows maximum nesting depth, and checkpoint payloads are released.
Batched integer grouping passes its already coalesced rows directly to the
collector, preserving public batch ordering and last-update behavior. Original
regressions cover nested rebuilds, saved candidates, callback failure and payload
release. These changes add no upstream-method credit. Native timing samples are
mixed across workloads; no general speedup or fresh Java comparison is claimed.

Standalone list cascades, metadata-bound shadow sessions and list/mixed scoring
streams now support next-element dependencies as well as previous-element
dependencies. Next mode propagates from tail to head and exposes the updated
next_shadow; existing constructors keep previous mode. Deferred updates, imported
seeds, equality stopping, snapshot restoration and solver candidate rollback use
the selected direction. Native prefix/suffix reference regressions add no upstream
method credits, reflective graph-classification claim or performance measurement.

Diversified late acceptance now updates its cached minimum and multiplicity
without scanning history when a replacement preserves a minimum or supplies a
new lower one. Removing the final minimum triggers one scan for its replacement.
All score comparisons/clones finish before state publication, retaining unwind
behavior. Existing pinned acceptance/replacement cases remain unchanged. Original
regressions compare full history against independent scans and exercise every
update branch under injected comparison/clone failures. No new method credits.

## Checked compound basic moves

`BasicMoveBuilder` prepares changes, swaps, ordered write groups and value cycles
on a private validated assignment. Each operation reads earlier edits, validates
its final domains/pins and rolls back on error or unwind. Groups may temporarily
leave domains or alter pins if their final state restores validity. Required
variables may be cleared during preparation; completeness is checked separately.
Cloning forks a draft, and `finish` yields an ordinary `Move` for scoring/search.
Replay requires the original starting state; it does not rebase or detect stale
assignments. Preparation clones the assignment once and cycles clone selected
values. No performance improvement is claimed for this new API.

Six additional `SelectorBasedCompositeMoveTest` methods preserve construction,
ordered equality, child execution multiplicity and interdependent change/swap
behavior. Native variants and write journals substitute for Java types and mocks,
as recorded in provenance. Arbitrary child doability predicates remain pending.
Three original regressions exercise draft forks, both scoring backends, commit/
rollback, model validation, temporary invalid writes, missing values, and injected
clone failures during cycle preparation/application. These earn no method credit.

Range-conditioned `SampledListValueSwapProvider` now integrates bounded sampling
with list/mixed search. Pins, two-way receiving ranges, optional replacements,
no-op policy and dependency rejection remain enforced. Original regressions
compare native conditional draws over 27 partial assignments, duplicate weights,
seed wrapping/replay, changed-state reopening and both scoring backends. Native
random traces and bounded attempts differ from Java's never-ending selector
contract. Existing random-prefix fixture credits remain partial; no new upstream
methods or fixtures are claimed. Value-swap validation reuses a private working
copy; exact clone counts are documented in the list planning contract. No new
wall-clock or Java performance measurement is claimed.

All six `IteratorBiasIT` methods now have native adaptations, preserving the
original one-million/200000 trial counts, physical gap fixtures, bucket weights,
all parameter combinations and five-sigma binomial limits. Coverage includes
repeated draws at eleven positions, retirement of two interior identities,
all 120 five-element permutations, comparison queries in both directions,
overlapping membership deduplication and multi-bucket unique draws at positions
1/2/10. Native seed-zero PRNG streams replace Java random traces; query ordering
and Java iterator classes are not parity claims. Repeated-draw checkpoints share
one traversal per trial, preserving each requested marginal. These tests passed
without a solver implementation change; they establish previously missing bias
evidence, not a newly fixed bias or a performance improvement.

`RetiringIndices` adds selective uniform retirement over virtual `u128` rank
spaces with sparse remapping. `LazyConcat::retiring` and
`CandidatePairs::retiring` expose it on borrowed indexed queries without copying
payloads or materializing Cartesian products. Draws without retirement allocate
no remapping entries; storage grows with retirements. Cloning a rank sampler forks
its seed and retirement state. Supplier errors/panics leave the selected occurrence
available for explicit retirement, while the underlying query remains unchanged.
Original regressions compare against dense swap-removal, exercise huge spaces,
independent forks, duplicate pair occurrences and lazy supplier failures.

One additional `DatasetBucketBiasIT` method preserves all three 200000-trial
parameter cases and five-sigma 20%/30%/50% bucket shares after successive draws.
Native indexed greater-or-equal joins and retirement replace Java dataset/session
iteration. Provenance records test/helper attribution and native PRNG substitution.
This establishes query sampling behavior, not Java lifecycle or timing parity.

Dataset sampling now chooses between eager and lazy permutations. Small samples
use `lazy_exhaustive_iter`, which retains the snapshot, clones payloads on demand
and starts with sparse rank bookkeeping. Full-drain rules and minimum sizes at
least one thirty-second of the dataset retain eager Fisher-Yates ordering.
`Sampler::exhausts_source` is a stable optimization hint (false for custom rules
unless overridden); `SizeSampler::all` and its keyed wrapper forward the hint.
Sample decisions, distinct membership, minimum failures and snapshot isolation
remain unchanged. Native small-sample order changes, without Java trace parity.

Retiring rank storage now promotes to dense remaining indices after at least
1/32 of its original space has retired and the remainder fits usize. Promotion
preserves active-rank order and consumes no randomness; failed allocation keeps
sparse storage. Dense capacity is bounded by the consumed fraction. Huge virtual
spaces and untouched/prefix queries retain sparse state.

Both `SamplingIteratorBiasIT` methods additionally preserve all original200000
trial cases, unary uniformity, binary draw positions1/2/5 and five-sigma20/30/50%
shares through actual sample assembly. Original regressions cover100000row
prefixes, eager full-drain selection, clones, replay, cached/uncached keyed
filters, minimum handling and held snapshots after source updates.

An isolated optimized native ordering benchmark on the final source measured a
10-row prefix of100000rows at0.957ms eager versus0.00128ms lazy (construction outside
timing). Explicit10000row full drains were0.0855ms eager versus0.290ms lazy; normal
full-drain sample rules retain eager order. Raw results include three processes,
warmups, alternating samples and source/executable identities in
`work/lazy-dataset-performance.json`. These are ordering costs, not whole-solver
or Java speedups. Old dashboard performance records retain their dates/freshness.

`ValueSeededSubListSelection` provides an indexed, borrowed value-to-owner lookup
feeding uniform entity-local sublist ranks. It avoids a linear assignment search
for each selected value and does not clone value payloads. After validation,
construction stores O(n) borrowed entries in O(n log n) time; lookup is O(log n)
before rank selection. The borrow ties the index to its assignment snapshot.
Unknown, unassigned, pinned and too-short seeds consume no rank draw. Excluding
pinned-prefix seeds is an explicit native policy, not an additional upstream claim.
Bounded iterators preserve repeated seed weighting and include rejections in their
read limits. Rebuild the index after a move or model change.

Both `SubListSamplingBiasIT` methods now preserve200000draws and five-sigma bounds:
all18spans on7values with lengths2..5, and entity shares2:3:5 from independent,
uniform assigned-value seeds. The new index performs the actual seed-to-owner
mapping. Native PRNG streams replace Java traces; these cases add no solver-wide
performance claim. Original regressions cover pins, every rank, borrowing,
comparison counts, bounded lazy iteration, replay, relocation and undo.

`FilteredPairSampler` adds explicit left-uniform filtered pair selection: choose
an entity occurrence uniformly, then a matching value occurrence uniformly. This
retains the filtering-only neighborhood's per-entity weighting and differs from
existing indexed pair-uniform queries. The first visit scans the right side fully;
only a proven empty row is retired. Rare matches cannot disappear due to an unlucky
probe. The immutable-input predicate must stay stable for the traversal.

A caller-specified global match-cache budget bounds retained right indices. Rows
that exceed the budget retain only a count and rescan to the selected match.
Caching changes predicate work, not the seeded trace. No payload cloning or full
Cartesian pair catalog is required. Predicate panics do not install partial rows
or retire the selected left. Empty joins fuse; nonempty sampling needs a bound.

Two `LeftRetirementBiasIT` methods preserve interior retirement fairness and
hard/easy reachability across400000draws; the filtering-only `PairFairnessBiasIT`
method preserves the620000draw, size-normalized share-ratio assertion. Native
change moves retain fixture identities and observable entity/value membership.
Exact cached scans replace Java random probing; PRNG traces differ. Original
checks cover cache bounds, trace equivalence, duplicate occurrences, panic retry,
empty joins and provider-backed search with both scoring backends. Predicate-call
counts establish caching behavior; no wall-clock or Java performance claim.

`MoveProviderGroup` compiles explicit nested provider groups into a flat union of
unit-weight leaves. Random selection therefore weights groups by their leaf
counts, without manually assigning group weights. Sequential traversal preserves
depth-first leaf order. Factories remain lazy, lifecycle events reach the leaves,
and exhausted leaves leave the current scan. Counting and compilation are
iterative; empty trees return the existing empty-provider error.

Three `MixedMoveSelectorTest` methods now preserve plain/flat/nested leaf counts;
`MixedMoveSelectorSplitBiasIT` preserves100000draws from3classic and2neighborhood
leaves with five-sigma3:2shares. Actual provider compilation determines the split.
A wrapped opaque provider counts as one leaf; arbitrary Java decorators are not
introspected and their methods remain pending. Keep groups explicit until union
compilation. Original regressions verify deep nesting, empty groups, lazy factory
calls, grouping-independent traces, finite-leaf exhaustion, lifecycle forwarding,
and provider-backed solving with both scoring backends. No timing claim is made.

Integer stream collectors now store up to two distinct values inline and promote
to persistent frequency trees on the third. Trees demote at one distinct value,
avoiding rebuilds when repeatedly crossing two/three values. Equality compares
logical frequencies across representations; held snapshots, wrapping sums,
duplicate counts and extrema retain their semantics. Two original regressions
cover repeated transitions and independently computed summaries; no upstream
coverage credit is added.

An isolated before/after benchmark against dc2f71f measured eight complete stream
search workloads (32/96 variables, change/swap, hill climbing/tabu,20steps,
three processes and five measured samples per process). Medians improved2–9%;
96-variable tabu swap decreased426.4ms to389.1ms. Scores, assignments and callback
counts matched. Small standalone tree-churn workloads still showed up to7%
overhead; this is not a universal speedup. Raw evidence and source identities:
work/inline-collector-performance.json and work/collector-threshold-final.tsv.

A separate fresh Java comparison validated128runs and128equal-score cases.
At96variables Rust/Java medians were30.47/52.12ms for hill change,
29.65/56.70ms for tabu change,411.35/181.66ms for hill swap, and
379.53/198.14ms for tabu swap. Thus larger swap workloads remain slower than
Timefold. This refresh updates only the stream comparison; other measurements
retain their existing dates and calculated freshness.

`BestSolutionTracker` retains an owned assignment, score and explicit count of
required unassigned values. Fewer missing required values outrank a better raw
score; equal counts use normal score ordering and ties retain the first snapshot.
Optional unassignment does not contribute to the count. Callers supply the score
and count from the same evaluated state; the tracker does not rescore or mutate it.
The lazy snapshot factory supports non-Clone saved states and runs only for strict
improvements. The borrowed variant clones neither assignment nor score on rejection.
Snapshot construction completes before replacement, preserving the old best on panic.

All eight `BestSolutionRecallerTest` methods preserve original score/count inputs,
improvement decisions and retained solution/score assertions. One native operation
handles both candidate-move and completed-step observations. This reusable tracker
can consume solver observers, but does not change automatic solver-wide partial
retention, score-director lifecycle or Java event timing. Native regressions exercise
partial and complete construction observers with both scoring backends, deterministic
ties, missing required values, snapshot independence and panic recovery. No timing
claim is made; clone/factory counts directly verify work avoided for rejected states.

Value-tabu capture inside list and mixed local search now uses the already-scored
candidate's validation. Primitive metadata reads the pre-move lists and copies only
distinct moved values, preserving first-touch order and separate basic/list keys.
Empty composites capture nothing; single-child wrappers are unwrapped iteratively.
Multi-child composites replay coordinates on a draft of borrowed values, copying
only touched list rows and preserving later children’s view of earlier edits.
Only distinct captured values are cloned into the result. Public planning_values
still performs full model, pin, range and dependency validation. The internal shortcut is used only after a
candidate has successfully applied and scored from the corresponding snapshot.

Original comparisons cover valid primitive moves across pins/ranges, sequential
composites, namespaces, and all six existing value-tabu solver backends. A primitive
one-value move on1000values clones1value for internal capture instead of1021in the
public validation/replay path. No upstream methods are newly claimed.

An optimized native before/after benchmark against1914ab5 measured complete list
searches with four lists, value-tabu tenure3/aspiration and best-accepted swap scans.
For32/96values at10steps, median runtimes were14.106->7.406ms and548.163->201.445ms;
at30steps they were51.898->24.949ms and1859.768->702.775ms. Three fresh-process forks
alternated baseline/candidate, with one warmup and three samples each. Scores,
full best assignments, callback counts and completed steps matched; each result was
independently rescored and validated. This47–63% reduction concerns these workloads,
not Java performance or every solver configuration. Raw results/source identities:
work/scored-values-performance.json. Existing dashboard benchmarks retain their
measurement dates and computed freshness; no Java benchmark was refreshed.

Compound metadata capture now handles assignment, relocation, swaps, reversed
sublist changes, unequal sublist swaps and unassignment without replaying model
validation or cloning the whole assignment. The draft borrows values from the
original assignment and move payloads; untouched rows are never copied. Nested
children execute iteratively in order. First-touch payload identity survives a
later unassign/reassign of an equal value. Snapshot clone failures cannot mutate
the original assignment. The scored-candidate precondition remains internal.

Original regressions compare200varied nested sequences and their final rows with
validated application, cover every primitive, and check sparse clone counts and
panic safety. The public validation path remains unchanged. A native full-solve
benchmark against116ca39 uses a two-swap compound provider, four lists, value tabu,
and32/96values at10/30steps. Medians before->after(ms):12.027->7.124,
335.891->157.504,27.334->19.293,999.466->467.132. Three alternating fresh-process
forks with warmup and three samples preserve identical scores, best assignments,
callback counts and step counts; every result is independently rescored/validated.
The29–53% runtime reduction is specific to these workloads. It is not a Java
comparison. Raw evidence: work/compound-values-performance.json. No additional
upstream-method credit or dashboard benchmark refresh is claimed.


### List value-tabu comparison

The original public Java/native list benchmark now covers fixed-size value tabu
(size 3, aspiration), separately for relocations and ordered swaps. Both use
complete identical starts, original candidate ordering, best accepted selection,
first ties and a 20-step cap. Independent Python checks retain pins, entity ranges,
ownership, completeness, full scores and non-regression; small-case optima are
exhaustively enumerated. All 64 relocation and 64 swap cases have equal final
scores across engines. These original cases add no upstream-method coverage.

The 2026-10-09 comparison uses three alternating fresh-process forks, one warmup
and three measured solves per process. Median milliseconds (Rust / Java):

| Customers | Value tabu: relocation | Value tabu: swaps |
| --- | ---: | ---: |
| 16 | 3.998 / 90.608 | 2.965 / 49.539 |
| 32 | 23.474 / 121.569 | 17.354 / 92.632 |
| 96 | 507.758 / 498.238 | 776.981 / 429.377 |

All six timed value-tabu workloads have identical final scores. Swap score-call
counts also match; relocation counts differ. The larger swap workload exposes a
remaining Rust performance gap. Timings include per-solve setup and output
validation, with full scoring in both engines. Short JVM warmup and differing
candidate/tie/tabu histories limit generalization; this is not a steady-state,
incremental-scoring or universal speed claim. Full samples, runtime versions,
source fingerprint and method are in work/list-performance.json, consumed by the
existing dashboard list-solving section. Earlier modes were refreshed in the same
run: 320 valid small cases overall, 317 equal-score cases (three two-opt differences).


### Sparse value-scope discovery

List and mixed scoped score networks now retain the indices that have registered
value constraints. Candidate invalidation only discovers list membership for those
indices. Entity-only networks perform no value identity comparisons for discovery;
sparse networks avoid scanning unrelated registry values. Repeated scopes register
an index once. Disabled weighted constraints remain indexed so reactivation between
sessions retains normal updates. Callback ordering, conservative neighbor
invalidation, model validation and transaction rollback are unchanged.

Two original regressions cover list/mixed equality-operation bounds, repeated and
out-of-order scopes, zero weights, reactivation, trial rollback, committed scores
and independent full recomputation. Existing seeded scoring and weight cases also
pass; no adapted upstream methods are added.

A native comparison against f60ef2a uses complete four-route full-scoring solves,
32/96 customers, relocation hill climbing or value tabu with relocations/ordered
swaps, and 20-step limits. Three alternating fresh-process forks with one warmup
and three samples retain identical scores, assignments and score-call counts.
Median ms before/after: hill32 30.822/18.131, hill96 1111.185/256.585;
value-change32 47.938/23.606, value-change96 1200.028/313.309;
value-swap32 31.484/19.472, value-swap96 916.353/191.082.
This is a 38–79% reduction on these entity-scoped workloads, not a general solver
speedup or a claim about densely value-scoped graphs. Independent validation and
source/executable hashes are recorded in work/sparse-values-performance.json.

The Java/native list benchmark was also refreshed on this source (same three forks,
one warmup and three samples). All 320 small cases validate, with 317 equal final
scores; all 128 value-tabu cases agree. For 96 customers, median relocation time is
417.974 ms Rust / 1474.232 ms Java; swap time is 213.280 / 738.283 ms. Final scores
match, and swap score-call counts remain identical. Java timing varies substantially
from the preceding run, so the alternating native before/after comparison above is
the stronger evidence for the implementation gain. Dashboard samples expose the
observed timing ranges and retain the short-warmup/full-scoring methodology.


### Phase-scoped termination profiles

TerminationProfile now binds directly to construction and local-search configs.
Specified time, move and step controls replace corresponding phase settings;
unspecified settings, score-evaluation caps, cancellation, assertions and
interruption policies remain intact. Local search also accepts stagnation time and
diminished-returns tuning. Construction rejects those two improvement-only controls
with a typed, setting-specific error before a solve starts. Phase step counters
reset on invocation; existing solve-wide profile step semantics still count search
steps, while explicit SearchLimit::CombinedSteps counts construction and search.

Five TerminationTest methods preserve source construction score assertions and
inapplicability diagnostics. The adapted duplicate-pair test fixture skips missing
values; native structural incompleteness is tracked separately from its ordinary
zero score. Original all-six-path regressions verify repeated phase step caps under
a shared budget, zero candidate budgets and preservation/replacement of controls.
No Java environment-mode, thread-interruption or wall-clock trace claim is added.


### Unimproved-step profile limits

TerminationProfile and TerminationResolver now support an optional signed-input
unimproved-step budget. Negative values are rejected, zero remains active and None
clears the setting. Fallbacks fill missing values and revalidate conflicts with
stagnation time under the existing service-style count policy. Lower-level solver
configs continue to allow combinations outside that profile policy.

Direct profile binding preserves unspecified existing controls. Shared binding
sets the solve-wide unimproved-step counter; local binding sets the phase counter,
which resets on each phase invocation. Construction rejects this improvement-only
setting. Platform defaults and request counts inherit/suppress each other according
to the same rules as other counts; explicit request stagnation time clears platform
counts, and any count disables automatic diminished returns. Resolved shared
binding now authoritatively replaces or clears max_unimproved_steps too.

One additional TerminationTest method preserves construction inapplicability and
its setting-specific diagnostic. Original integration tests cover all six solver
paths, repeated phases versus shared stagnation, zero limits, missing/clear/fallback
behavior, count validation, policy conflicts and actual resolved solves. This adds
configuration support for existing native counters, not new timing/PRNG behavior.


### Multi-list k-opt relocation scratch state

Combined-cycle repartitioning now calculates current relocation positions using
integer identities for the selected movable suffixes. It no longer clones a second
complete assignment or searches payload identities to locate each relocation.
The complete proposed assignment is still validated once for receiving ranges,
pins and dependency cycles, with unchanged error precedence. Selected entity order,
empty destinations, pinned prefixes, assigned payloads and ordered moves are retained.

An original clone-count/payload regression supplements the existing exhaustive
28,070 tour/partition checks and list/mixed solver integration cases. Native paired
measurements at baseline 6fb4a90 show 11–18% lower median proposal-generation time
for 32/96/256 values across four lists, 128 attempts, k2..4 and seed731. Three alternating
fresh-process forks used one warmup and three samples; every generated move was
applied and validated outside timing, and ordered move/result hashes matched across
versions. These short generator measurements do not establish full-solve or Java
speedups. Raw samples and source/executable identities are in
work/repartition-performance.json. No new upstream method credits are claimed.


### Sampling dependency boundaries

The indexed planning-range sampler now lives separately from the general index
and slice samplers. Candidate-window ordering likewise lives separately from the
random generator. Their existing bodies and public exports are retained. The
internal list-shadow projection helper has a specific name, avoiding a collision
with unrelated stream projection callbacks in the development dependency scanner.

The arity join target now selects 59 source modules instead of 172 and no longer
selects local search, k-opt, or solution management. Checks retain the planning-range
adapter for range sampling and candidate-window/k-opt code for actual list solver
consumers. A future repartition edit selects 553 jobs, compared with 661 jobs in
the preceding repartition increment. No verification results are relabeled; changed input sets require a new
successful run. This reorganizes existing implementation boundaries without changing
sampling algorithms, solver behavior, the upstream pin or compatibility credits.


## Basic move doability and repeated candidate checks

`FiniteProblem::is_move_doable` checks whether a valid change, swap or nested
composite changes the final assignment. `move_checker` validates a source snapshot
once and borrows it for repeated queries. Subsequent checks inspect only written
variables, require Eq but not Clone, and do not mutate, score or invoke listeners.
Primitive checks allocate no scratch space. Composite checks use borrowed final
values and preserve execution-order index errors followed by sorted final
pin/domain errors, including range diagnostics. Restoring/empty composites return
false. Required variables may be unassigned in partial planning states, as with
native move application.

Three Community methods preserve every change-doability, change-execution and
entity-range swap-doability assertion. Native final-net-change semantics differ
from the upstream composite method's OR of child doability; that method remains
pending. Invalid moves return native model errors rather than a false result.
Original regressions compare nested checks to transactional application, cover
non-Clone values, restored invalid intermediate states, and count comparisons on
1,024-variable snapshots. A lazy custom provider filters unchanged/invalid moves
before scoring with both basic scoring backends. This is an opt-in API: existing
provider behavior and evaluation budgets are unchanged. No wall-clock speedup or
additional Java lifecycle equivalence is claimed.


## List candidate validity without planning-value clones

`ListProblem::is_move_doable` and `move_checker` cover assignment, relocation,
swap, removal, reversed sublist relocation/exchange and nested composites.
The reusable `ListMoveChecker` validates the immutable source once. Each query
copies references only for touched rows, checks operation-order geometry and
identity errors, then final pins and receiver ranges in entity order. Models with
dependencies also check the global predecessor graph. Source/model state, scores
and listeners are untouched. Empty/restoring composites return false; invalid
candidates return the native `apply` error, including payload-sensitive diagnostics.
Required values may be unassigned in partial states, consistent with native apply.

Five Community methods preserve all list-change, list-swap and sublist-change
boolean doability assertions, including entity-provided ranges. The adapter maps
native errors to false where the upstream predicate rejects an invalid candidate.
The disabled list-change range method is not claimed. Sublist-swap doability
methods remain pending: one accepts spans beyond the supplied source list, and
the other starts outside its declared range; native checked source/geometry
semantics do not match those methods. No Java listener-lifecycle claim is made.

Six original regressions compare 4,000 primitive/nested candidates to transactional
application, exercise final pin/range restoration and global dependency cycles,
check unequal reversed spans and deep composites, and prove zero planning-value
clones while preserving diagnostic payloads. Lazy providers integrate the checker
with all four list/mixed scoring paths, filtering before candidate scoring.
This opt-in API does not replace existing solver provider or budget behavior.
No wall-clock speedup is claimed; scratch references still require allocations.
