# Native ordered-list planning

`ListProblem<V>` models a finite global value registry distributed among entity
lists, with optional immutable per-entity range restrictions. Order is significant. `Eq` defines value identity; use stable
IDs. The range has no duplicates, and each assigned value appears exactly once
across all lists. Values outside the range are rejected. Partial states may leave
required values unassigned. A complete state assigns all values unless the model
opts into `allowing_unassigned()`; optionality applies to the whole list variable.
MixedProblem combines this with basic variables for atomic edits and exact joint
search; see `mixed-planning-contract.md`.

`ListEntity` supplies initial values and immutable pin metadata. A pinned prefix
preserves its values and positions but allows appending, even when the prefix
contains the entire initial list. A fully pinned entity freezes both contents and
length and requires a zero prefix setting. Metadata and the initial assignment
are validated by the constructor. These behaviors follow the pinned Community
public modeling documentation. Negative indexes cannot be represented by the
native `usize` API. Initial unassigned values are valid partial states.

`ListAssignment` exposes read-only lists. `location` and `element` derive owner,
index, previous and next from the current lists with a linear lookup. Unassigned
values have none of those relationships. These are native shadow views, including
inside move transactions; there is no cached listener state to become stale.
Custom/declarative/cascading shadows and Java event ordering remain pending.
Unchecked assignments can be constructed, but model operations validate them.

## Atomic moves

`ListMove` supports assignment of an unassigned value, single-value relocation,
position swap, contiguous-span relocation/reversal/exchange, half-open span unassignment,
and ordered composition. Within-list
relocation removes the source first, then inserts at the destination index in the
shortened list. Cross-list relocation uses the destination's current index.
Composites use current indices after each preceding operation. Assigning an
already assigned value is an error; relocation uses `Change`.

`ListProblem::apply` validates the starting assignment and creates a full-state
snapshot. A successful transaction exposes only a read-only candidate, its
original state and `commit()`. Dropping it restores every list. Invalid indices,
duplicate assignment, unknown values, invalid ranges and final pin violations
restore the complete starting state, including after earlier composite changes.
Pins are checked on the final composite state, matching native basic transactions;
an intermediate edit restored within the same composite is allowed. Required
values may be unassigned during partial planning. Empty relocation/unassignment
spans, relocations back to the same index and empty compositions are native no-ops.
Span swaps require two nonempty, disjoint ranges.

Ordinary panic unwinding restores the lists. Normal `Clone`/`Eq` identity semantics
are required; interior mutations of shared value objects are outside the contract.
Process abort, `mem::forget` and panicking destructors can bypass rollback, as with
ordinary Rust drop guards. Composition application and metadata traversal use an
explicit stack; dropping deeply nested move enums has ordinary recursive destructor
behavior. Full-state cloning remains the list transaction strategy. Incremental contribution
scoring is described below; move application itself still clones the lists.

Thirty-four adapted upstream methods preserve assertions in five assign,
six swap, nine change, two span-unassign, seven sublist-change and five sublist-swap methods. See `compat/provenance.json`.
The shared modulo-distribution fixture is attributed separately. Two sublist-change
methods and one sublist-swap method check touched entity identities; other Java move metadata, rebase,
score-director notification and custom listener assertions
remain pending and receive no credit from these tests.

## Exact solving

`exhaustive_list_search` resets movable initial values and preserves frozen lists
and pinned prefixes. It considers remaining values in range order, inserting each
at every available entity/index position; optional unassignment comes last.
Every distinct complete assignment is visited once: removing the last considered
assigned value uniquely identifies its parent state. A native explicit stack
implements this traversal without recursive search calls. Ties retain the first
visited state. Scores are calculated by a deterministic shared-reference callback.
It must not mutate facts or value identity through interior mutability.

Negative structural scores reject a candidate. Hard-infeasible scores remain
eligible. A zero evaluation budget makes no scoring calls. `EvaluationLimit`
means a complete candidate remains unscored; `Exhausted` means none remain,
including when the last evaluation exactly consumes the budget. `is_optimal()`
requires exhaustion and a returned structurally accepted candidate. Empty models
have one empty assignment; required remaining values with no mutable entities
have no complete assignments. Optional values can all remain unassigned.

Runtime grows factorially. Enumeration stores the candidate and best assignment
plus insertion choices for each active depth. Separate branch-and-bound is described
below. Bounded heuristic solving and its Java comparison
are described below.
Twelve original regressions check complete assignment sets against independent
owner-selection and permutation enumeration, optima, budgets, ties, structural
rejection, pins, derived shadows, move invariants, undo, atomic errors and panic
unwinding. They contribute no upstream coverage credit.

## Bounded exact search

`branch_and_bound_lists` and its observer variant use the same priority-queue engine
as basic bounded search. Each node decides the next non-pinned registry value by
inserting it at a permitted entity/index or omitting it when optional. Frozen
lists and pinned prefixes survive root preparation; movable initial values are
cleared. Each complete state has exactly one decision path. The callback receives
remaining registry indices, so a decided optional omission is distinguishable
from an unresolved value. A model-complete optional partial state is not a leaf.

Later insertions can change existing positions and neighbors. Caller-supplied
optimistic bounds must account for those changes and dominate every eligible
completion under full score order. Unknown bounds are safe; the engine retains
known ancestor bounds. Existing basic bound checks, exploration orders, counters,
eligibility, limits, strict-best observation and conditional optimality apply.
Each queued partial state owns its lists. The frontier limit bounds node count,
not bytes; per-parent insertion choices also consume memory. Full scoring remains
the callback's responsibility. Ordered Exact phases additionally support unknown
bounds or a declared non-increasing-score assumption; see solver-contract.md.
Declared per-level trends derive symbolic bounds in ordered phases. No automatic
constraint analysis is provided.

Original checks compare full traversal with independent subset/permutation/partition
enumeration for all 128 two-entity range/optionality configurations, then check
pins, prefixes, non-dense registry IDs, tight/loose/unknown completion-table bounds,
empty/impossible spaces, structural rejection and failure recovery. These add no
upstream-method credit or dedicated list branch-and-bound timing claim.

## Incremental list scoring

`ListConstraintNetwork<V, S>` registers named deterministic additive constraints.
`ListScope::Entity(i)` supplies an ordered entity list; `ListScope::Value(i)` uses
an index into the model's stable value range and supplies that value's identity
and optional owner/index/neighbor view. Value indices need not equal value IDs.
`ListFact` exposes only the requested facts. Mixed and repeated scopes preserve
callback argument order. Empty scopes are constant constraints. Invalid scopes,
duplicate names and network/model shape mismatches are errors.

Contributions must have structural zero. The network sums ordinary levels using
`AdditiveScore`, including modular arithmetic for built-in integer scores, and
sets structural state to minus the number of missing required values. Optional
unassigned values have no structural penalty. Full `calculate` validates the
assignment and evaluates every contribution. Callbacks must handle partial states;
captured facts and value identity must not change during a session.

`ListScoreDirector` owns an assignment and contribution cache, borrowing the
immutable model and network. Applying a move reevaluates constraints on touched
entity lists and on any value present in those lists before or after the final
composite. This conservative invalidation covers relocated/unassigned values and
neighbors whose index or adjacency changes, including no-op edits. Each dependent
constraint is evaluated once even with repeated scopes or composite touches.
Unrelated contributions and constant constraints stay cached.

`ScoredListMove` exposes candidate assignment, score and constraint-evaluation
count. Cache updates are deferred. Drop rejects it; commit retains the lists and
installs saved contributions without callback replay. Model errors, structural
contribution errors and ordinary callback panics restore the original assignment
and every cache entry. Existing list transaction drop limitations still apply.
`assert_consistent` independently recalculates each contribution and the total,
including detection of cancelling per-constraint corruption.

Dependency discovery still scans value locations; shadow lookups are linear,
callback argument vectors allocate, and list transactions snapshot the full state.
Only constraint callbacks are incremental. No constant-time update or runtime
speedup is claimed. Nine original regressions include 1,200 seeded assign/change/
swap/span-unassign composites, each checked against an independent direct formula,
full network calculation and per-constraint cache assertions. They cover commit,
rejection, required/optional structure, neighbors, repeated/mixed scopes, non-dense
value IDs, pins, callback errors/panics, cancelling corruption and BendableScore
wrapping boundaries. A public API doctest exercises a partial-to-complete commit.
These original checks receive no upstream method credit or Java incremental-score
compatibility claim. Construction/neighborhood/search integration is described
below.

## Java move evidence

`python3 scripts/differential_lists.py` compares 256 original cases containing
seeded valid assign/change/swap/span-unassign composites against the pinned
Community public `Moves`, `MoveTester`, metamodel and shadow-enrichment APIs.
Java and Rust candidate, restored and committed states must each equal an
independent Python slice/concatenation reference, including all four built-in
shadow relationships. The seed is 20261003. Inputs, outputs, failures, artifact
hashes and source digest are saved under ignored `work/`.

The comparison uses one optional list variable and no Java pin metadata. An empty
move sequence compares initial shadow enrichment only: Java's public composition
API rejects an empty move collection. Java also rejects same-index relocation. Native no-op support is tested
separately; generated Java moves have distinct relocation/swap positions and
nonempty unassignment spans. The 256 cases contain 907 move operations. It does not compare list solver results, custom listeners, timing,
mixed models or exact Java traces. Passing generated checks grants no upstream
method credit. The dashboard exposes this evidence separately from basic-variable
solver and performance comparisons, with source freshness checks.

## Construction and bounded local search

`list_assign_moves`, `list_change_moves`, `list_unassign_moves` and
`list_swap_moves` validate an immutable assignment, then lazily yield moves.
Relocations use indices after removal and omit same-position no-ops. Swaps emit
both directions of each distinct assigned-position pair. Optional swaps also
include atomic replacements by currently unassigned values. Full pins exclude
both sources and destinations; prefixes exclude their positions but allow
insertion immediately after the prefix. Required partial assignments can use
assignment moves during construction. Required unassignment iterators are empty.
These native choices differ from Java provider constructor restrictions.

`list_moves` combines relocations, optional assignments, optional unassignments,
then swaps/replacements if `Neighborhood::ChangeAndSwap` is selected. Ordering is
stable by entity, position and global value-range order. Individual moves may
produce the same candidate state (for example opposite-direction swaps).
No full neighborhood is materialized. Thirteen adapted upstream methods preserve
all positive, negative and emptiness assertions for solution-wide ranges and
full pins. Six further methods cover entity-provided ranges, including allowed
assignment, shared-value relocation and optional replacements. Java metadata and
constructor assertions remain pending. Fixture records separately attribute
initial list states and range metadata.

`greedy_list_construction` retains every initial value and pin, then inserts
missing required values in range order. It scans legal entity/index positions
against the same base state and keeps the first highest score. The four shared
`ConstructionPickEarly` policies compare ordinary score levels, ignoring the
required-unassigned structural count. Optional missing values stay unassigned.
A model with a missing required value that no mutable list accepts returns
`NoCompleteAssignment` without scoring. A zero budget otherwise returns the
initial state without callbacks. Model/network validation still runs.

The initial score counts once and each trial counts once. A winning candidate
stores its assignment and sparse contribution updates; installation does not
replay callbacks. If a candidate remains after the budget is consumed, the best
scanned candidate is installed and `EvaluationLimit` is reported. This may yield
a complete assignment even though its insertion scan was truncated. Exhausting
the final scan exactly at the budget returns `Completed`. Construction neither
backtracks nor proves hard feasibility or optimality. Partial results expose
`complete`, optional score and assignment separately; `solution()` requires a
complete scored assignment.

`list_local_search_from` accepts a complete validated assignment, including a
construction result, and starts a new scoring/acceptance session. The handoff
rescore counts once. It supports the same five acceptance policies, first/best
accepted selection and optional best-rejected fallback as basic local search.
`MoveOrder::ShuffledVariables` shuffles entity-list order using its explicit seed;
value-range order and positions remain stable. `Acceptance::VariableTabu` keys
touched entity-list indices, including both lists for cross-list moves. Annealing
uses a separate seeded random stream. Equal seeds do not imply equal Java traces.

Each trial counts once; diagnostic full recomputations do not consume budgets or
reported callback work. Completed steps include rejected fallback steps, while
accepted moves count only accepted selections. At step boundaries the accepted
move cap takes precedence over the step cap. Best-accepted selection can install
the best scanned prefix before returning `EvaluationLimit`. A fully scanned
neighborhood with no selection returns `NoAcceptedMove`, including exact budget
exhaustion. The first global best survives worsening accepted/fallback steps.
None of these termination reasons proves optimality. Callback errors or ordinary
panics during candidate selection restore the base assignment and all cached
contributions, even after an earlier candidate was saved.

Fourteen original integration regressions and one unit regression check complete
small neighborhood state sets, pins, lazy value cloning, budgets, first ties,
early picks, optional replacement and capacity-constrained swap improvements,
fallback accounting, entity tabu, validation at zero budget, cache reuse and
selection failure rollback. Fifteen seeded policy/order combinations reproduce
identical native results and match the independently scored exhaustive optimum
on a six-value routing instance. These are native correctness/quality checks,
not a claim of Java list-solver parity. The `list_routes` example constructs and
searches 40 customers across four vehicles with explicit budgets, independently
checks identities, capacity and distance, and enforces a fixed quality threshold.
It is not a timing benchmark. Separate matched Java list-solving evidence is
described below.

## Java list-solving evidence

`benchmark_lists.py` uses the original public-API `ListSolverBenchmark.java` and
native `list_solver_benchmark` adapters for construction, relocation hill climbing
and two-opt.
The legacy seven-field protocol records case ID, customer x:y coordinates, depot
coordinates, route capacities, initial value-ID lists, full-pin bits and prefix
lengths. An optional eighth field records each entity range: `*` inherits the
global registry, `-` is empty, otherwise comma-separated value IDs; entities are
separated by semicolons. Empty customer ranges or lists use `-`. All values are required.
Coordinates are bounded to ±1,000,000, at most 1,000 values and 100 nonempty-model
entity slots; capacities are nonnegative Java-int-sized values. Rust remains
independent of Java; these bounds belong to the development protocol only.

Both adapters retain initial states. Construction must finish assigning required
values while retaining preexisting values in their original routes and relative
order. Local search starts complete, retains at least its initial quality, and
uses a matched completed-step budget. For relocation hill climbing, Rust's
evaluation limit bounds all candidate scans without an earlier cutoff. Two-opt
uses first-accepted hill climbing and a separate 10,000 score-evaluation budget;
Java uses its seeded public preview provider while Rust applies a seeded bounded
shuffle to finite candidates. Java uses only public annotations
and configuration, with NO_ASSERT and a fixed seed. Full pins, pinned prefixes,
empty ranges, capacity violations and Manhattan distances are checked by 192
original generated cases (96 unrestricted and 96 with per-entity ranges; 64 per phase) with independent Python enumeration of the global
pin-constrained optimum. Equality and optimum attainment are observations, not
criteria weakened into validity claims. See docs/dashboard.md for measurement
conditions and scope. Matching Java traces, custom shadows, optional-list solving
and mixed-model solver comparisons remain pending.


## Entity-specific value ranges

`ListEntity::with_value_range` restricts an entity to a duplicate-free subset of
the global registry. `value_range()` exposes the optional restriction: omitted
means all registered values; an explicit empty range accepts nothing. Unknown or
duplicate range values and initial assignments outside an entity range are model
errors. Range ordering is metadata only; the global registry continues to define
construction and unassigned-value enumeration order. Values can belong to several
entity ranges, but assignment ownership remains unique. Values belonging to no
entity are permitted in partial models and may remain unassigned in optional
models. These native construction choices do not assert Java metadata parity.

Model validation enforces membership for every assigned value. Transactions
validate entity ranges on the final composite state, as with pins; a temporary
range violation restored before the composite ends is allowed. Invalid final
assign/relocate/swap states roll back atomically before score callbacks. Lazy
assignment and relocation only choose accepting destinations. Swaps check both
receiving entities; optional replacement checks the receiving list. Exact search
filters destinations for each value without changing the enumeration's ordering
or uniqueness. Prefix/full pin semantics are unchanged.

With unlimited list lengths, each missing required value can be appended
independently if any unpinned entity accepts it. Construction and exact enumeration
use this fact to detect impossible completion before callbacks, including at a
zero evaluation budget. They report `NoCompleteAssignment` and empty `Exhausted`,
respectively. This checks model completeness, not hard-score feasibility; capacity
penalties are still ordinary constraints. Optional orphan values generate no
assign/replacement candidates, but do not prevent a complete solution.

Eight original regressions check metadata, rollback, range/pin interactions,
non-dense identities and budgets. Exact search is compared to independent
owner/permutation enumeration for all 64 pairs of three-value/two-entity range
subsets in both required and optional models. Optional neighborhoods are also
compared with independently range-filtered unrestricted moves at every valid
state. Two protocol and three Python evidence regressions verify range-aware
validation and independent optima. The Java comparison adds 64 original required
list-solving cases with per-entity ranges, separately from the six adapted
upstream methods. The measured larger routing workloads remain unrestricted.


## Incremental stream scoring

ListStreamScoreDirector now binds entity lists and all registered value views to
a transactional stream graph. ListStreamSources uses stable entity/range indices;
ListValueRow contains value identity and owned owner/index/previous/next views.
Explicit assignment filtering controls stream membership. Generic stream joins,
collectors and weighted terminals can score these rows through list moves,
construction and local search. The same private list algorithms serve both scoped
and stream scoring, with saved winners installed without callback replay.
See docs/constraint-stream-contract.md for propagation, rollback, diagnostics,
callback accounting and regression scope. Custom shadows and mixed stream scoring
remain pending. Existing list-move comparisons now read the cached native stream shadows;
list-solving timing workloads continue to use their full-scoring adapters.


## Contiguous sublist relocation and reversal

`ListMove::SubListChange` removes a half-open `[start,end)` span, optionally reverses
it and inserts it at a destination. Destination indices are measured after removal,
including within the same list. Entire-list moves, insertion at either boundary,
singletons and empty spans are valid; empty spans still validate source/destination
positions. Range errors are returned before draining. Final pin and per-entity
range checks share the existing transaction guard, so a failing later composite
step or ordinary caller unwinding restores the complete original state.

Touched entity metadata includes both owners, deduplicated. Existing scoped/list
stream directors refresh contributions and derived views for these owners and their
values; mixed transactions and scoring accept the same move. Seven complete source
methods preserve forward/backward/reversed/cross-owner states, unassignment, undo
and touched entity identities. Other methods in that source class remain pending.

`list_sublist_change_moves(problem, assignment, max_length)` lazily enumerates
legal lengths two through the bound, source owner/start/end, destination owner and
post-removal insertion index, then forward/reversed orientation. Singletons already
belong to ordinary changes. Forward same-position no-ops are omitted. Sources begin
after pinned prefixes; pinned owners are excluded and every moved value must fit
the destination range. Different moves may yield the same successor. Bounds below
two yield no span moves; `usize::MAX` is safely capped by available list length.

`Neighborhood::ChangeSwapAndSubList { max_length }` appends these moves, then
disjoint span swaps described below, after the existing change/optional assign/unassign/swap families. It enables list and mixed
local search with both scoring backends. Joint mixed moves also combine basics
with spans. Basic-only search uses changes/swaps for this option. Default
neighborhoods retain their prior move order and size; the larger neighborhood is
opt-in and can be expensive even though generation remains lazy.

Five original regressions check invalid boundaries/pins/ranges and panic rollback,
independent successor sets, 120 scored trial/commit/undo rounds with shadow-sensitive
scores, list/joint-mixed plateau escape in both backends, and basic swap retention.
The list oracle adds two reused upstream fixtures, two original boundary cases and
124 generated span cases (seed 20261124), for
384 cases / 1,348 operations total. All Java public MoveTester and native cached
stream candidate/undo/commit states and owner/index/previous/next views agree with
Python slicing. This move evidence adds no extra source-method credit, Java pin
or sublist-search trace claim, or new performance workload.


## Disjoint sublist swapping

`ListRange { entity, start, end }` describes a half-open pre-move span.
`ListMove::SubListSwap { left, right, reverse }` exchanges two nonempty spans,
optionally reversing both blocks. Different lengths, different owners, adjacent
same-list spans and either argument order are supported. Empty spans, invalid
bounds and same-list overlap are rejected. Final pin/range checks, touched owner
metadata, composite failure and caller-unwind rollback share the list transaction.

`list_sublist_swap_moves(problem, assignment, max_length)` lazily enumerates
owner/start/end pairs, then forward/reversed exchanges. Each span is bounded and
at least one has length two or more; ordinary swaps already handle two singletons.
Pairs use canonical owner/start order, so symmetric duplicates are omitted, but
different moves may have the same successor. Fully pinned owners and pinned
prefixes are excluded. Every value must fit the receiving owner's range. Bounds
below two produce no moves; oversized bounds are clamped without overflow.
The existing opt-in ChangeSwapAndSubList neighborhood now includes these swaps
after span relocations, in list and joint mixed search through both score backends.
Default neighborhoods and performance workloads remain unchanged.

Five full Community SubListSwapMoveTest methods cover ordered results, undo and
owner membership. Five original regressions additionally cover invalid moves and
atomic rollback, all disjoint spans across every two-owner split of seven values,
both argument orders/reversal, independent successor sets, pins/ranges, 120 scored
trials/commits and list/joint-mixed plateau escape requiring a separated block swap.
Metadata for captured pre-move values, Java equality/string/rebase behavior and
mock listener-event assertions remain pending. A separately seeded original corpus
adds 128 Java/Rust/Python candidate/undo/commit/shadow comparisons (seed 20261125),
preserving all earlier 384 inputs. This adds no source-method coverage credit or
Java span-search/performance equivalence claim.


## Dedicated tail-swap and two-opt neighborhoods

`list_tail_swap_moves(problem, assignment, include_reversing)` exchanges nonempty
suffixes of different owners. Canonical owner order emits each pair once. Forward
moves always appear; reversed exchanges appear only when requested and at least
one suffix has two or more values. Empty tails, fully pinned owners, pinned
prefixes and values outside the receiving owner's range are excluded.

`list_two_opt_moves(problem, assignment, cross_entity)` first reverses every
within-list span of length two or more in place. With crossing enabled, it then
exchanges nonempty cross-owner tails without reversal. Native finite enumeration
has no randomized sampling or Java stream-order guarantee. These providers reuse
SubListChange/SubListSwap; transaction and score invalidation behavior is shared.

`Neighborhood::TwoOpt { cross_entity }` selects only those list moves, including
in mixed/joint search with either scoring backend. Basic variables still use
changes/swaps. Optional values remain at the same assignment status and empty
routes cannot receive customers through this neighborhood. This is an explicit
tradeoff of this move set; use the existing broader neighborhoods to permit those
changes. Defaults retain their previous move families and ordering.

The dedicated generators visit start positions and reversal endpoints directly,
without enumerating all pairs of arbitrary sublist ranges. The search dispatcher
boxes the selected iterator to bound stack usage in nested mixed products; it
allocates one iterator per list enumeration, and does not materialize successors.
An existing lazy-product regression exposed a debug stack overflow with the
larger combined iterator type and passes with this representation.

Twelve fully adapted Community methods cover tail membership, reversal toggles,
singleton-tail suppression, same-owner exclusions, in-place two-opt reversals,
cross-owner toggles, pins, owner ranges and execution indexing. Supporting
fixtures have separate provenance. Four original tests compare independent small
successor sets (including empty/frozen owners and value ranges), cached trial and
commit scores, list/joint-mixed plateau escape with both scoring backends, and
basic-swap/optional-list behavior. Existing 512 Java move comparisons exercise
the reused primitives; they do not prove dedicated provider sampling or Java
whole-search trace equivalence. The original timing workloads retain their prior neighborhoods. Two new
two-opt workloads use the same 16/32-customer inputs with first-accepted search;
the dashboard reports differing final quality and candidate-work counts. These
measurements do not establish equal traces or equal work.


The two-opt benchmark now explicitly selects ShuffledMoves with seed 20261006 and
a 1,024-candidate window in Rust. The library default remains Original. An independent
replay of the original 16-customer first-accepted run observed a two-state neutral
cycle after five steps; seeded ordering is an opt-in way to explore other moves.
The benchmark retains identical starts, limits and scoring but this changes its
native ordering configuration. Java uses the same numeric seed with different
PRNG/sampling behavior. Quality and callback counts remain explicit per workload.

`exhaustive_list_search_with_config` now also accepts `ExactSearchConfig::max_time`.
The cooperative timer spans setup and enumeration; it polls before scoring a
complete candidate and retains the evaluated best on TimeLimit. Exhausted remains
the only optimality-proving reason. See `planning-contract.md` for shared precedence,
empty-space semantics, callback overshoot and controlled-clock verification.

### Planning-value metadata comparison

The generated list comparison additionally exercises native
`ListProblem::planning_values` against an independent Python replay and pinned
Community public `Move.getPlanningValues()`. The same 512 original cases plus
two small composite reproductions give 514 native reference passes, 425 Java
identity-set agreements, 16 differing Java sets, 40 Java runtime errors and 33
unsupported empty Java compositions. These are original checks, not adapted
upstream methods. The original 512 candidate/undo/commit state comparisons still
all agree.

The reference records each child's moved or assigned identities before applying
that child, including removed items and intermediate composite operations.
Native capture is observed before temporary execution and again after undo,
before committing. Java getters are observed after each execution, as the public
API requires. Successful responses also check candidate, restored and committed
lists and all four derived shadows. Java errors stop that case's metadata
observations; they do not establish post-error state correctness.

Two original reproductions retain the distinction:
- Lists `[[0,1],[2]]`: swap positions `(0,0)` and `(1,0)`, then relocate
  `(0,0)` to `(1,1)`. Moved identities are `{0,2}`; Java reports `{0,1,2}`.
- List `[[0,1]]`: swap its two items, then unassign index range `[0,1)`.
  Moved identities are `{0,1}`; Java's getter throws an index error.

No production Java move implementation was used to explain these observations.
Rust keeps its independently specified capture semantics. Java order and
multiplicity are retained in `work/oracle/list-values-cases.json`; membership
comparison normalizes to sets, while native checks require exact first-touch
order without duplicates. The report exposes all differences and errors. These
checks establish neither identical value-tabu search traces nor new timing results.

### Gathering selected identities

`ListProblem::gather_move` builds a validated ordinary composite from a distinct
ordered sample and an optional original-state insertion boundary. It removes
selected assigned values, then inserts all selected identities consecutively in
sample order. Only selected values originally before the boundary reduce its
index. Other identities retain their order. A missing destination unassigns the
selected assigned values; already unassigned values have no effect in that case.
Assigned identities retain their existing payload, newly assigned identities use
the supplied payload. Empty selections are validated no-ops. Unknown identities,
duplicate members and invalid boundaries are rejected. Final pins/ranges are
validated; intermediate removal of a pin is permitted if the final state preserves
it. Required models may become partial, consistently with native Unassign.

Construction works on a private snapshot and leaves input unchanged on errors or
ordinary panic unwinding. Returned positions are bound to that starting state and
must be regenerated after it changes. Applying the composite uses ordinary list
transactions and existing scoring rollback. Structural composite equality does
not claim the Java mass-move equality/hash/rebase contracts.

Five pinned execution/undo methods have complete adaptations. Original checks
cover 29,835 small layouts/ordered selections/destinations, independently assembled
at original boundaries, plus 260 commit/reject steps in both list scoring backends,
validation, payload identity and clone panic behavior. A separate seeded public
Java comparison checks nonempty single gathers and all candidate/undo/commit lists
and built-in shadows. It does not compare move metadata, pins, custom listeners,
search quality or timing and earns no upstream method credit.

### Bounded gather neighborhoods

`list_gather_moves` and `Neighborhood::Gather { max_size }` enumerate nonempty
ordered groups lazily. Destinations come first, in entity/insertion-boundary order;
for each destination, group length increases from one through the clipped bound,
then permutations follow registry order. Pinned values are excluded, destinations
respect pinned prefixes, and destination value ranges prune the eligible identities
before permutation enumeration. The identity immediately preceding a boundary is
also excluded before enumeration, avoiding entire duplicate permutation groups.
Consecutive original boundaries with the same
result for a sample use the earliest boundary; unchanged results are omitted.
Different samples may still reach the same successor.

Optional unassignments follow all insertion candidates, once per assigned identity
subset in increasing size and registry order. Required complete inputs remain
complete; partial inputs may receive missing values. Zero disables the list
neighborhood after input validation. Oversized bounds are clipped. Enumeration
allocates no factorial list, but consuming a large neighborhood still has factorial
work. Each yielded gather currently clones a private assignment during construction.

List local search and both mixed search backends select this neighborhood through
the shared configuration. Basic-variable search retains changes/swaps, including
when max_size is zero; mixed joint products combine those basic moves with gathers.
Entity shuffling changes destination order; registry member order stays fixed.
Rolling move shuffling, evaluation/step limits and typed move/value tabu use the
ordinary shared search paths. No Java neighborhood ordering or search-trace
compatibility is claimed, and these original checks add no upstream method credit.

Original evidence covers successor-set equality over 108 small required/optional
models with pins and ranges, deterministic order, zero/oversized bounds, frozen
models, and an 80-value clone guard proving lazy construction. Independent small
permutation counts, uniqueness, ordering and exhaustion are checked separately.
List and joint mixed scoring backends agree on results, evaluation counts, completed
steps and termination over three neighborhoods, three seeded/order modes, four
acceptance policies and five evaluation budgets. A target permutation demonstrates
an improvement unavailable to strict single changes/swaps. Basic backends retain
the ordinary neighborhood behavior. Gather-search performance and Java search
comparison remain unmeasured; the existing gather transaction comparison is separate.

## Assigned-only group moves

`list_mass_change_moves` produces immutable `ListMassChange` candidates with ordered
member identities, an original insertion boundary (or no destination for group
unassignment), and an ordinary snapshot-bound composite move. Missing values never
join a sample. All members must be assigned and movable; full pins and pinned
prefixes exclude values and receiving positions. Every receiver must admit every
member. Applying the move preserves each assigned payload and uses existing atomic
validation and scoring rollback. Regenerate candidates after changing the snapshot.

`ListSampleSize::exactly(n)` requires n eligible members; insufficient population
produces no candidates. `between(min,max)` rejects reversed bounds; the maximum is
clipped to population. Groups of zero or one always produce no moves. `all()` uses
the entire assigned movable population before per-receiver range filtering, not a
smaller receiver-specific population. The default config enumerates sizes two
through the whole population. This is finite deterministic enumeration, with
receiver/index order followed by size and registry-index permutation order.
Unchanged states and equivalent adjacent insertion boundaries are skipped; distinct
samples can still lead to the same state. Permutations are lazy, not materialized.

Whole-group unassignment defaults to the model's optionality. Explicit false
suppresses it. Explicit true on a required model returns UnassignmentNotAllowed,
even at zero search budget. Each unassignment subset appears once without ordering
permutations. Required partial inputs remain partial; this provider does not assign
missing values. General `list_gather_moves` keeps its broader previous behavior.

`Neighborhood::MassListChange(config)` integrates this provider into scoped and
stream list search and both mixed paths, including simultaneous basic/list moves.
Basic-only search retains ordinary changes/swaps. ShuffledVariables changes the
receiver order; ShuffledMoves uses the existing bounded shuffle window. Acceptance,
budgets, observation, retained best and transactional scoring use the existing local
search contract. List/mixed solver sequences validate every configured null policy
before any phase scores or invokes a stream factory; errors retain the phase index.

## List group assignment and unassignment

`list_mass_assign_moves` and `list_mass_unassign_moves` require optional list values,
including when the requested size or search budget is zero. Required models return
OptionalValuesRequired before scoring or graph factory work. The pinned public
neighborhood documentation's “Moving scattered values” section specifies missing
sources for assignment, assigned sources for unassignment, legal common insertion
destinations, and a minimum of two members for assignment/change.

ListSampleSize remains exact/ranged/all; the minimum is two for assignment/change,
one for unassignment. Zero is empty. Exact sizes exceeding source population are
empty, ranged maximums clip, and all uses the global eligible population before
receiver filtering. Assignment enumerates receiver/index, size and registry-index
permutations of missing values. Every receiver must admit every member. Pins and
prefixes constrain insertion positions. Unassignment enumerates assigned movable
subsets once in size/registry order, excluding missing values and all pinned values.
Both return ListMassChange metadata plus ordinary snapshot-bound composite moves;
existing gather validation, payload preservation and atomic scoring rollback apply.

`sample_list_mass_unassign_moves` uses an explicit seed and bounded draw count.
Groups sample without replacement internally and with replacement between draws.
Size is uniform over clipped positive bounds; subsets are uniform and presented in
registry order. A sufficient population supplies every draw, including repeated
singletons. Empty/insufficient populations return none. This is not Java PRNG or
sampling distribution parity. All draws use the same immutable assignment snapshot.

Neighborhood::MassListAssign and MassListUnassign integrate finite enumeration into
list/mixed scoped and stream local search, including joint basic/list moves. Basic
search retains changes/swaps. All list/mixed sequence adapters validate these modes
before any phase work, retaining phase-indexed errors. These new providers do not
yet have dedicated Java/Rust quality or performance measurements; earlier gather,
list-move and repair comparisons remain separate.
ShuffledVariables changes assignment receiver order; unassignment retains registry
subset order. ShuffledMoves can shuffle either finite candidate stream.


## Cyclic k-opt within one list

`ListProblem::k_opt_move` removes and adds explicit undirected edges in one
movable suffix. Endpoints are indices in the input assignment. The suffix wraps
from its last identity to its first; pinned prefixes stay outside the cycle.
At least three movable values and two distinct removed edges are required.
Reconnection must have degree two everywhere and form one connected cycle.
Self edges, repeated edges, invalid indices and disconnected subtours are errors.
The first unpinned identity stays fixed and an explicit `first_successor` chooses
which direction to traverse. Existing assigned payloads are preserved by ordinary
relocations in an atomic composite. These moves remain bound to the input snapshot.

`ListKOptConfig::new(minimum, maximum)` requires 2 <= minimum <= maximum.
`list_k_opt_moves` and `Neighborhood::KOpt` lazily enumerate every anchored order
whose undirected edge difference is within those bounds. Both directions matter
for directed objectives. Enumeration cuts a cycle into paths, permutes/reverses
paths and excludes retained cut edges; no unchanged edge sets or duplicate tours
are emitted. Storage is linear per active candidate, but the search space grows
factorially. An empty or short suffix, full pin, or oversized minimum yields none.
Missing values and other entities remain untouched; per-entity ranges are retained.
Basic variables keep changes/swaps; joint mixed search combines those with k-opt.

The APIs above implement single-entity cyclic reconnection; explicit multi-entity
repartitioning is described below. Java's implicit partition/orientation conventions,
callback spans and rebase remain pending. Eight upstream output fixtures are attributed without method
credit because their Java notification assertions are not adapted. The feasibility
method preserves both boolean outcomes using native success/error results. Original
permutation references, rollback and search tests add no upstream method credit.
The dedicated k-opt comparison reports single-route solution quality, score calls
and elapsed time against the pinned public Java API; its scope is described below.

### Bounded k-opt sampling

`ListKOptSampling` and `sample_list_k_opt_moves` generate proposals on demand.
Each proposal chooses an eligible entity uniformly, a possible k uniformly within
its clipped bounds, distinct cycle cuts, a path permutation and independent path
directions. Proposals retaining any selected cut edge are rejected. Every emitted
move therefore changes exactly k undirected edges; repeats across proposals are
allowed. The attempt count bounds proposals, not returned moves or coverage. No
uniform distribution over final tours or Java random-distribution parity is claimed.
Small impossible cycle sizes are rejected before consuming attempts. No eligible
entity yields no work even with a maximal attempt count. Invalid input still fails
when attempts are zero. Construction and each proposal use linear scratch storage;
edge checking is O(n*k) and conversion to ordinary relocations is O(n²).

`Neighborhood::SampledKOpt` integrates this sampler into list and mixed search.
The sampler config exposes `for_step`: step zero preserves the supplied seed;
subsequent completed steps derive new seeds using wrapping arithmetic. The list
and mixed engines use that derivation for both scoring backends. Phase counters
restart at zero. Standalone neighborhood enumeration uses the supplied seed as-is.
Move-order shuffling is separate; use Original to avoid a candidate window.
Basic-only search retains changes/swaps. Pins, ranges, anchors and missing values
have the same semantics as exhaustive k-opt. Each move still affects only one list.

The benchmark preserves both the exhaustive/window and sampled strategies on the
same route corpus. Compare their quality, work and time separately: sampling may
find better or worse outcomes even when it reduces generation cost substantially.

### Combined-cycle k-opt and explicit repartitioning

`k_opt_repartition_move` combines the movable suffixes of a supplied, distinct,
unpinned entity list in that order. Endpoints index this concatenation; there is
one wrap edge for the combined cycle, not a separate wrap per entity. The existing
edge validator requires at least three combined values and two removed edges,
equal removed/added counts, unique valid edges, degree two and a single cycle.
The caller chooses an ordered start/successor pair in the new cycle and each
destination's suffix length. Lengths must sum exactly to the original
combined count; zero lengths and previously empty destinations are supported.
Explicit edge input may remove and re-add an edge, as in the single-list API.

The resulting tour is divided according to those lengths in the supplied entity
order. Prefixes, all unselected lists, missing identities and assigned payloads
are preserved. All receiving ranges are checked against the complete target.
Construction emits ordinary current-index relocations; final-state validation and
atomic rollback use the existing composite-move path. The move is snapshot-bound.
This API permits changing ownership and list lengths without unassigning values.

`sample_list_k_opt_repartitions` and `Neighborhood::RepartitionKOpt` propose moves
across all unpinned lists, including empty receivers. Each bounded attempt chooses
a feasible k, distinct cuts, random path reconnections and a uniform cycle rotation,
then uniformly samples
a weak composition of the combined value count into destination lengths. Retained
cut edges and illegal receiving ranges reject a proposal; each yielded combined
cycle has exactly k changed edges. The number of proposals, not yielded moves, is
bounded. Repeats are allowed and final valid moves are not uniformly distributed.
Cycle rotations let any identity begin the first nonempty destination; there is
no fixed first identity in sampled multi-entity moves. Move-order shuffling may change the
entity concatenation order. Per-step seed derivation works as for SampledKOpt.
Basic-only search retains ordinary changes and swaps.

Scratch storage is linear in total assigned values and entity count; validation
and relocation construction may be quadratic. Empty or impossible cycle sizes
return immediately. Range rejection may consume the full configured attempt
budget. No complete candidate window or permutation space is allocated.

Two upstream ordered-output fixtures retain their initial lists, edge identities
and exact result lists with explicit direction and partition parameters. They are
attributed without method credit: Java's implicit multi-entity boundaries,
orientation and list-change notifications remain unadapted. In particular, local
wrap-edge fixtures cannot be interpreted as combined-cycle edges without defining
an additional translation. General Java multi-entity parity is not established.
Nine original tests independently enumerate small tours and all partitions,
validate sampled successor sets, payloads, pins, ranges, empty destinations,
large cuts, seed/budget prefixes and cross-owner list/mixed search. Existing
single-list regressions still cover the extracted shared graph and draw helpers.
The current Java/Rust timing section remains limited to single-route k-opt.

## Compound move drafts

ListMoveBuilder owns an isolated draft initialized from a validated assignment.
Each successful edit appends an ordinary ListMove and updates that draft. All
coordinates refer to the current draft; relocation destinations are post-removal.
assign and assign_many insert unassigned values, preserving input order. Duplicate,
already-assigned, unknown or disallowed values fail atomically. Empty assign_many
validates its destination but adds no step. set_unassigned displaces the current
slot (returning its value), or appends at the end (returning None).
replace moves an existing element into another element's position, unassigning
and returning the displaced element; it adjusts same-list source/destination
indices internally. Equal source/replacement positions fail. shift takes a signed,
nonzero offset, rejects overflow/out-of-bounds results, and returns the moved value.
relocate_span, swap_spans and unassign require nonempty valid spans.

Each edit validates the resulting model, including pins and receiving ranges.
Failed calls and ordinary unwinding retain the earlier draft and recorded steps.
push accepts arbitrary native moves/composites; an explicit composite can make
intermediate changes to pinned/range-restricted lists provided the final state
satisfies the model. Missing required values remain valid intermediate state;
model completeness and structural score are separate checks. Cloning the builder
forks an independent draft. It uses full model snapshots, so no constant-time or
incremental draft-edit performance is claimed.

finish returns a plain move for one edit, or a Composite for multiple/zero edits.
Replay against the same starting state reproduces the draft and can use either
list score director, custom move providers, or mixed moves. Final scoring sees the
compound result, not intermediate states. The builder does not bind the finished
move to its original assignment or provide Java notification/lifecycle semantics.
Fifteen complete Community assignment methods preserve lists, element positions,
unassigned states, errors and undo. Notification-specific shift/replacement methods
remain pending. Original exhaustive small-index checks, failed-edit/panic recovery,
branching drafts, both score-director paths and provider solving are separate.

## Direct span sampling

SubListSelection indexes movable nonempty ranges using one count block per eligible
entity, without cloning values or enumerating every span. Whole pins are excluded;
prefixes shift the first eligible start. Size bounds are inclusive and clamp to
each movable suffix. Counts are u128. Ranks follow entity order, descending length,
then ascending start; out-of-range ranks return None. After ordinary model
validation, indexing uses O(entities) memory/time; lookup uses binary searches over
entities and length groups. Coordinates describe the original snapshot. Rebuild
after committing a change, while held indices retain their original coordinates.

random_iter samples ranks uniformly with replacement using native seeded randomness.
random_iter_with accepts externally supplied bounded ranks for replay/testing and
panics on an invalid draw. Empty indices never invoke the source; nonempty indices
repeat until the consumer stops. Unassigned values are not spans. These semantics
are independent of Java random sources and multi-entity selection probabilities.

sampled_sublist_change_moves samples spans and post-removal destination slots,
including optional unassignment and reversals according to SubListChangeConfig.
sampled_sublist_swap_moves samples independent left/right spans with asymmetric
bounds, canonicalizes interchangeable sides, rejects overlaps and optionally
reverses both sides. Receiving ranges are checked before yielding. Both APIs are
lazy after building their span indices and take an explicit attempt limit. Rejected
pairs, invalid receiving ranges and no-op relocations consume attempts. Duplicate
proposals remain. These are proposal distributions, not a promise of uniformity
over legal moves. A short sample can find no moves despite a nonempty neighborhood.

Use the returned iterators in a MoveProvider, rebuilding them from the immutable
assignment supplied at every committed step. Advance the seed across steps when
new draws are desired. The search engine still handles scores, acceptance, rollback
and evaluation budgets; the separate attempt limit bounds unscored rejection work.
Original integration checks solve an ordering problem through both list scoring
backends with score consistency checks enabled. No timing advantage is claimed
without a deliberate benchmark; existing performance reports remain stale.

## Deferred list cascades

ListCascadeCache supports deferred initialization and batching alongside its eager
API. new_deferred loads an assignment with None shadows and queues every value.
queue_update validates and installs the latest assignment, accumulating structural
owner/index/neighbor changes and explicit dirty roots without calling suppliers.
queue_move provides the same operation as a reversible native move trial; dropping
restores assignment, shadows and pending roots. Its changed-shadow list is empty
until later evaluation. Clones retain independent snapshots, including pending work,
without requiring shadow values to implement Clone.

flush_pending calculates queued roots in current list order, propagating only while
results change. Each affected value runs at most once; intermediate assignments
are not replayed. Eager update/apply also consume existing pending work. Successful
evaluation clears the queue; invalid metadata or supplier unwinding preserves all
stored values and pending work for retry. Pending root count is not a prediction
of the number of downstream callbacks. Supplier external side effects cannot be
rolled back.

clear_pending_updates discards queued roots and returns their count. It preserves
the latest assignment and stored shadows, which may deliberately remain stale or
uninitialized. Flushing an empty queue does not silently repair cancelled work.
Queue all problem values explicitly to repair a cancelled or partially imported
snapshot. This facility batches custom cascades; standard list shadows and stream
events retain their existing eager behavior. Callers feed changed custom values
into scoring sources after flushing; cache and separate graph transactions do not
form one automatically coordinated transaction.


`ListProblem::from_entity_ranges` accepts entities with explicit ranges and derives
the global registry by a stable first-encounter union. Every entity must supply a
range; an empty range is valid, and shared values occur once in the registry.
Ordinary constructor validation still rejects duplicate entries within a range,
illegal initial assignments and pins. Equality denotes planning identity; no Hash
or Ord bound is added. Entity/domain iteration order defines the registry order.
Use the ordinary constructor when an explicit global registry is available or some
entities have unrestricted ranges. Optionality remains a separate model setting.

`RuinRecreateConfig::additional_neighborhoods` adds weighted existing neighborhood
families to the repair/change/swap mixture. All use the same current state, retained
best, acceptance history and score budgets. Zero-weight entries skip validation
and enumeration; enabled empty families are removed for the current attempt. A
family is selected by weight, then one candidate is sampled uniformly using bounded
storage. Sampled k-opt seeds advance by total attempt index. Candidate enumeration
remains cooperative with cancellation/deadlines. Model-specific neighborhood rules
are unchanged; mixed search emits independent basic/list moves, not joint products.
An empty slice preserves the legacy seeded trace. Enabled configurations validate
before scoring, including when placed later in an explicit solver sequence.

`MixedNeighborhood` configures separate optional basic/list families, a basic-slot
encounter order/filter, and optional joint products. `mixed_moves_configured` emits
basic singles, list singles, then joint candidates. It validates the assignment and
enabled settings first. Disabled basic selection ignores its slot list; enabled
basic selection rejects unknown/duplicate slot indices. An empty slot list disables basic
candidates. Pins/domains and whole-row pillar rules remain in force; filtering a
column never splits a multi-column pillar move. `mixed_moves_configured_union`
combines these families with sequential or seeded weighted interleaving, retaining
duplicate occurrences and skipping zero-weight configurations. Step indices advance
union and sampled-neighborhood seeds. Both functions return iterators that can feed
existing MoveProvider bindings in scoped or stream-backed mixed solver plans.

Repair search now accepts `ruin_candidates` as an allowed identity set: basic-slot
indices for basic models, registry indices for lists, and basic slots followed by
offset list-registry indices for mixed models. Pins and model eligibility intersect
this set. Candidate order stays in model order; duplicate/out-of-range indices are
errors. None preserves unrestricted behavior and Some(empty) disables repair.
Fine moves remain governed by their own neighborhood configurations.

`repair_pools` optionally replaces the one reconstruction pool with separately
weighted `RepairPool` entries. A chosen attempt samples within only one pool;
pool weights multiply the repair-family weight, and empty/ineligible/zero-weight
pools are unavailable. `ruin_candidates` is also applied to every pool. Unselected
basic slots are frozen during reconstruction; unselected list values retain owners
and relative order (insertions may shift indices). Zero-weight pools skip validation.
Enabled scope validation runs before scoring, even for later solver phases.

Additional fine families now use `RepairNeighborhood::Standard(n)` (or `n.into()`)
and `RepairNeighborhood::Mixed(config)`. The mixed form uses independent component
families and basic-slot filters from MixedNeighborhood; basic/list-only models
reject it explicitly. All families and pools share current state, acceptor, retained
best and resource limits. Defaults preserve the previous seeded trace. These are
native search controls and do not imply Java selection-distribution parity.


## Bounded random gather neighborhoods

`sampled_list_gather_moves` accepts `GatherSamplingConfig` with validated inclusive
minimum/maximum member counts, an attempt budget, a seed and opt-in unassignment.
It clips maximum size to movable registered identities; too few identities or zero
attempts yield an empty fused iterator. Setup validates the snapshot, indexes eligible
receivers and collects movable identities without cloning value payloads.

Each attempt draws size uniformly, then an ordered group without replacement using
a partial Fisher-Yates permutation, then an eligible pre-removal destination slot.
The group intersects receiver ranges through ListDestinationIndex. Full/prefix pins
exclude source values and receiving positions; pending unassigned values remain
eligible. Optional unassignment contributes one destination and is rejected during
setup for a required-list model. Required partial assignments may gain missing values.
The gather builder adjusts removal indices and validates dependencies and final state.
No-op, impossible-receiver and dependency-invalid proposals consume attempts without
yielding. Duplicate successors remain; sampling is not uniform over final assignments.

The iterator retains one index permutation and receiver metadata, never the full
factorial gather catalog. Each proposal still has group/range work and validated
move construction may clone a temporary assignment; no constant-time or measured
performance claim is made. Exhaustion can miss feasible moves. Callers choose a
bounded attempt count appropriate for cooperative cancellation.

GatherSamplingProvider regenerates the snapshot at each search step using seed plus
completed_steps with wrapping arithmetic. List and mixed scoped/stream scoring use
the standard acceptance, evaluation limits and rollback paths. Mixed moves preserve
basic variables. Empty sampled scans can end ordinary local search; compose providers
when a fallback neighborhood is needed. Invalid model/config combinations panic at
the infallible provider boundary; direct sampling offers fallible setup.

Six original cases cover small reachable-state equivalence to an independent remove/
insert oracle, required/optional lists, pins/ranges/dependencies, partial assignments,
empty/no-op neighborhoods, seed replay, lazy value cloning, evaluation limits and
nonadjacent gathering improvements on all four list/mixed scoring paths. These are
native tests and earn no additional upstream method credit.

## Borrowed segments and repartitioning

`SegmentedSlice` exposes multiple borrowed slices as one logical flat sequence.
It stores boundaries and references, supports empty/repeated segments, converts
between flat and segment indices, and offers ordered/reverse iteration and subviews.
Flat lookup is logarithmic in the number of segments. `SegmentedSliceMut` supports
replacement, swap, reversal and borrowed mutable subranges without cloning values;
Rust borrowing prevents resizing or aliasing during the view's lifetime.
`repartition_lists` redistributes owned payloads in unchanged combined order into
explicit lengths, validating shape, sums and allocations before moving anything.
These are general collection APIs, with no implicit solver pin/range checks.

Explicit and sampled K-opt repartitioning use the borrowed view for selected
movable suffixes. Existing snapshot-bound move construction and final model
validation still apply. Sampled proposals violating dependency cycles are rejected
just like receiving-range violations, instead of panicking. Setup no longer clones
the combined value payloads; candidate construction still clones working states.
This trades a temporary payload vector for segment metadata and indexed boundary
lookups; runtime improvements have not been benchmarked.

## Ordered destination selection and sampling

`ListDestinationSelector` borrows a validated assignment and accepts independent
entity and anchor orders. Entity entries contribute their first unpinned insertion
position; movable assigned anchors contribute the following position. Full entity
pins, pinned anchors and missing anchors are excluded. Duplicate entries retain
occurrence order and sampling weight. Entity selection controls the starting
positions only; an anchor may name a value on another entity.

`all` returns an owned `ListDestinationOrder`; `for_values` also intersects receiver
ranges and removes anchors belonging to the selected group. Each query derives a
fresh order, so changing the selected group cannot retain an old exclusion. Optional
unassignment, when requested, is first, including when no receiver is eligible.
The owned order supports indexed/double-ended traversal and repeated uniform rank
sampling. Empty and singleton `sample` calls consume no randomness. Positions are
snapshots before source removal; reopen after assignment changes and validate actual
moves, including source pins, adjusted indices and dependencies.

`SampledAnchoredListChangeProvider` supports list and mixed construction/local-search
bindings. It filters source pins, then draws a source occurrence and a destination
from that source's eligible order. It can assign missing values, relocate assigned
ones and optionally unassign them. Candidates use existing full move validation;
no-ops, empty destinations and dependency rejections consume attempts. Memory does
not retain a Cartesian move catalog: destination choices are built only for the
current proposal. Reopening uses seed+completed_steps with wrapping addition, so
partial consumption cannot disturb replay. A bounded empty sample does not prove
optimality. Mixed moves preserve basic variables. No Java selector fallback or
random-trace equivalence and no measured runtime improvement are claimed.

## Pending edits and rebased recordings

`PendingListChanges` holds the first open entity inline, activating a BTreeMap for
additional simultaneous entities. Sequential open/close operations avoid map
allocation. The first slot stays inline after promotion; closing it lets another
entity reuse that slot. `clear` resets the mode. Duplicate inserts fail without
replacing an action. Insert/remove and promotion never clone keys or actions.
`ListChangeRecorder` now uses this tracker for its pending edit indices, retaining
its existing candidate-state publication and failure behavior.

`ListChangeRecorder::rebase` maps a snapshot into an independent recorder with
explicit target field metadata and fallible entity/value lookup callbacks. It
preserves before/after snapshots, range boundaries, edit order, open brackets and
notification delegation. Each distinct source key/value is mapped once; repeated
occurrences reuse the result. Distinct source identities may not collapse to the
same target identity. Target value types may differ, but metadata must match the
target type and the source list's unassignment policy.

Errors and lookup panics leave the source intact. Reentrant mutations during lookup
return Busy. Completed target undo does not consume source history; unfinished
brackets must first be closed in the target recording. Later source/target edits
and their live undo handles are independent. Lookup side effects are outside the
transaction. This API requires stable Clone/Eq identity and maps whole saved list
snapshots, including unchanged values; it does not infer missing target objects.
