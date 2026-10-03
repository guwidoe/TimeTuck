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
The two decimal gradient methods remain pending. Native progress objects replace
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
Decimal score families remain pending.

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
compatibility, daemon/problem-change behavior and shared elapsed-time/stagnation
limits remain pending; see `docs/solver-contract.md`.


Native construction and multi-phase solving now have cooperative elapsed-time
controls, independently checked with controlled clocks and real zero-time calls.
The shared timer includes graph preparation and spans construction and improvement
phases; it does not establish Java timer/lifecycle equivalence or hard preemption.
All fourteen new regressions are original and add no upstream method coverage.
See `solver-contract.md` and `incremental-search-contract.md` for scope and precedence.
