# Native ordered-list planning

`ListProblem<V>` models a finite global value registry distributed among entity
lists, with optional immutable per-entity range restrictions. Order is significant. `Eq` defines value identity; use stable
IDs. The range has no duplicates, and each assigned value appears exactly once
across all lists. Values outside the range are rejected. Partial states may leave
required values unassigned. A complete state assigns all values unless the model
opts into `allowing_unassigned()`; optionality applies to the whole list variable.
The basic-variable model remains available separately; mixed basic/list solving
has not been implemented.

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
position swap, half-open span unassignment, and ordered composition. Within-list
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
values may be unassigned during partial planning. Empty spans, relocations back to the same index and empty
compositions are native no-ops.

Ordinary panic unwinding restores the lists. Normal `Clone`/`Eq` identity semantics
are required; interior mutations of shared value objects are outside the contract.
Process abort, `mem::forget` and panicking destructors can bypass rollback, as with
ordinary Rust drop guards. Composition application and metadata traversal use an
explicit stack; dropping deeply nested move enums has ordinary recursive destructor
behavior. Full-state cloning remains the list transaction strategy. Incremental contribution
scoring is described below; move application itself still clones the lists.

Twenty-two adapted upstream methods preserve all state assertions in five assign,
six swap, nine change and two span-unassign methods. See `compat/provenance.json`.
The shared modulo-distribution fixture is attributed separately. Java move
metadata, rebase, score-director notification and custom listener assertions
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
plus insertion choices for each active depth. There is no branch-and-bound. Bounded heuristic solving and its Java comparison
are described below.
Twelve original regressions check complete assignment sets against independent
owner-selection and permutation enumeration, optima, budgets, ties, structural
rejection, pins, derived shadows, move invariants, undo, atomic errors and panic
unwinding. They contribute no upstream coverage credit.

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
native `list_solver_benchmark` adapters for construction and hill climbing.
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
uses a matched completed-step budget. Rust's evaluation limit is an upper bound
on all candidate scans, not an earlier cutoff. Java uses only public annotations
and configuration, with NO_ASSERT and a fixed seed. Full pins, pinned prefixes,
empty ranges, capacity violations and Manhattan distances are checked by 128
original generated cases (64 unrestricted and 64 with per-entity ranges) with independent Python enumeration of the global
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
callback accounting and regression scope. Custom shadows and mixed models remain
pending. Existing list-move comparisons now read the cached native stream shadows;
list-solving timing workloads continue to use their full-scoring adapters.
