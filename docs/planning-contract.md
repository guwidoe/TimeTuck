# Finite planning and exact search

The native planning API is an independent implementation. Its first scope is
finite basic variables. Scoped incremental scoring and bounded local search are
described in `incremental-search-contract.md`, along with greedy score-aware
construction. Ordered-list variables, built-in derived shadows and exact list search are described
in `list-planning-contract.md`. Mixed basic/list models and exact joint search are
described in `mixed-planning-contract.md`; streams in `constraint-stream-contract.md`.
Custom shadows remain pending.

## Model and moves

`FiniteProblem<V>` contains ordered domains of distinct values. Use small stable
IDs for values; equality defines identity. Initial values must belong to their
domains. Pins preserve initial values; a required variable cannot be pinned without
a value. An optional variable includes `None` as a legitimate complete choice.
An empty required domain means the complete search space is empty.

`Assignment<V>` stores one optional value per variable. Partial assignments are
valid planning states; `is_complete` additionally requires each required variable
to have a value. Completeness is independent of score feasibility.

`Move` supports change, swap and ordered composition. Applying a move returns a
read-only `AppliedMove` guard. Dropping the guard restores every write in reverse
order; consuming it with `commit()` retains changes. Invalid indices roll back
previous writes, including in nested compositions. Ordinary panic unwinding also
rolls back. As usual for Rust drop guards, abort, `mem::forget`, and panicking value
destructors can bypass cleanup. Values must obey normal `Clone`/`Eq` semantics;
interior mutations of shared values are outside the transaction's guarantees.

`Assignment::apply` checks indices only. `FiniteProblem::apply` also checks the
starting and final states against domains and pins. A composite is validated as
one atomic final change, not after every intermediate write. Clearing an unpinned
required variable is permitted for partial planning states. Observing the undo
journal includes writes with no net change, including both writes in an equal-value
swap. Nested composite application uses an explicit stack; deeply nested `Move`
values still have Rust's ordinary recursive destructor behavior.

Ten adapted upstream methods cover basic change/swap application and undo, including
the equal-value two-write regression. Their source identities and fixture changes
are recorded in `compat/provenance.json`. This does not claim compatibility with
all upstream move types, selectors or score-director callbacks.

## Exact search

`exhaustive_search` maximizes a deterministic score by enumerating complete
assignments in domain order, with `None` last for optional variables and the
rightmost movable variable changing fastest. Pins stay fixed. Movable initial
values do not restrict or prioritize enumeration. Ties retain the first candidate.

The caller supplies a deterministic full-recomputation scorer and an explicit
maximum number of evaluations. Negative structural scores reject a candidate.
Negative hard scores remain eligible, so the best solution may be infeasible.
Positive structural values are accepted according to the current score contract.
No scorer can mutate the assignment through the provided shared reference; callers
must also avoid changing shared facts or value identity through interior mutability.

`Exhausted` means the complete space was visited. `EvaluationLimit` means candidates
remain. A zero budget does not invoke the scorer. Exactly reaching the final
candidate reports exhaustion even when it also reaches the budget. An empty model
has one empty assignment; a model with an empty required domain has none.
`is_optimal()` requires exhaustion and a returned candidate. It proves the maximum
over structurally accepted candidates only, under the deterministic scoring
contract; it says nothing about hard feasibility.

Search is iterative with linear auxiliary storage, but its runtime is exponential
in the number of variables. `search_space_size()` returns `None` on `u128` overflow;
a smaller evaluation budget still permits bounded exploration. There is no
branch-and-bound pruning or heuristic performance claim.

Run the original example with:

```sh
cargo run --example tiny_schedule
```

Original regressions include four queens, independently enumerated binary
optimization problems, budget boundaries, nullable domains, pinning, zero/overflow
spaces, transactional error/panic rollback, and small exhaustive move sequences.
These original tests are not counted in upstream coverage. Upstream tests requiring
specific Java exhaustive-search traces remain pending. A public-API Java oracle checks
128 original finite graph-assignment cases against independent Python enumeration:
64 homogeneous required-variable cases and 64 with per-variable ranges, mixed
required/optional values, empty optional ranges, pinned nulls and signed unassigned
preferences. Both adapters reset movable initial values before solving and retain
pins. Each returned assignment is checked for range membership, completeness,
unchanged pins, independently recomputed score and global optimality. Equal-score
ties need not return the same assignment.

The extended Java model uses one entity class with two genuine planning fields:
a required field and an optional field. Only one corresponds to the native
variable. The inactive required field has a singleton range and fixed value zero;
the inactive optional field has an empty range and fixed null value. Thus each
native complete assignment has exactly one corresponding complete Java assignment,
without adding constraints or changing scores. This accommodates mixed optionality
within the public exhaustive-phase configuration. It is an oracle representation,
not an implemented native multi-variable entity feature or a trace-equivalence claim.

Development protocol lines keep the six original tab-separated fields for the
existing homogeneous adapters. Extended lines append three fields: semicolon-joined
per-variable ranges (colon-joined values, empty for no non-null choices), optional
bits, and comma-joined unassigned soft weights. `-` denotes an unassigned value in
inputs and outputs. Null/null edges do not conflict; each unassigned variable still
contributes its specified soft weight. Extended input domains and supplied initial values
are validated before movable values are reset. Cases use small integers without
score overflow and always have at least one complete assignment. Empty required
ranges remain covered by native regressions, not these cross-runtime cases.

The dashboard has a separate matched exact-search benchmark for two small
homogeneous required-variable problems. The extended comparisons do not establish
performance for optional/range-specific problems, heuristics or large instances.
See `docs/dashboard.md` for measurement limits.


## Exact-search cancellation

`exhaustive_search_with_config`, `exhaustive_list_search_with_config` and
`exhaustive_mixed_search_with_config` accept `ExactSearchConfig<'a>` with a shared
optional borrowed CancellationToken and a max_evaluations budget (default 10,000).
The existing budget-only functions delegate with cancellation disabled; their
signatures and enumeration behavior are unchanged.

Before scoring each complete assignment, exact search checks the evaluation cap,
then cancellation. A request observed there returns Termination::Cancelled,
retaining the first highest-scoring eligible assignment already evaluated. The
active callback can finish; negative structural scores remain ineligible. Pins,
ranges, optional values, ordering and joint product enumeration remain intact.
There is no hard latency bound: model setup, enumeration, user value operations
and the scoring callback are not preempted.

Reaching the end of the actual space reports Exhausted, even if cancellation was
requested by the final callback or the final score exactly spent the budget. That
is the only reason which can prove optimality, and a best solution must also exist.
An impossible model has an empty space and is exhausted without callbacks even if
pre-cancelled; it has no optimal solution. A zero-variable model instead contains
one empty assignment, which must be evaluated before exhaustion is established.
If both the budget and request prevent the next assignment, EvaluationLimit wins.

Three original regression methods cover every budget/request prefix of basic,
pinned-prefix optional-list and mixed product spaces, retained eligible scores,
empty versus singleton-empty spaces, exact-final-callback exhaustion and real
controller-thread cancellation. A public API doctest demonstrates the new entry
point. These tests add no upstream adapted-method credit.

## Exact-search elapsed time

All three configurable exact entry points additionally accept
`ExactSearchConfig::max_time: Option<Duration>`, disabled by default. Their public
wrappers start a monotonic clock before setup and enumeration. One timer spans the
whole mixed product; it does not reset for each list or basic component. Elapsed
time includes model preparation, enumeration, candidate copies, scorer calls and
retaining the best assignment. The mixed scorer is polled for time after its owned
joint assignment has been prepared. A disabled limit does not sample a clock.

Before scoring an unvisited complete assignment, the evaluation cap has precedence,
then cancellation, then elapsed time. An observed deadline returns
`Termination::TimeLimit`, retaining the first best structurally eligible candidate
already scored. A callback that overruns can finish and its result is still retained.
Ties and negative structural scores keep their existing behavior. There is no
preemption inside enumeration, a callback or a user value operation, and therefore
no hard latency or deadline guarantee.

Only a remaining candidate can report a limit. If enumeration actually finishes,
`Exhausted` wins, even when the final callback overruns or requests cancellation.
An impossible space is exhausted without scoring or clock polling; it has no best
candidate and proves no optimum. A zero-variable/zero-value model instead contains
one empty assignment; a zero deadline stops before scoring it. The old budget-only
APIs leave the time control disabled and retain their signatures/order/results.
The new config field and termination variant require callers with complete struct
literals or exhaustive enum matches to account for them.

Six original controlled-clock unit methods cover every evaluation/count/cancellation/
time prefix for pinned/optional basic, list and mixed spaces; first-tie retention,
structural rejection, late callbacks, disabled/very-large/backwards clock samples,
empty spaces and setup/owned-joint-copy costs. Two original public integration
methods check real zero-duration precedence and unchanged disabled/large-duration
results against the budget-only entry points. One doctest shows the public setting.
These checks use no sleeps and grant no additional upstream method credit or Java
wall-clock/phase-trace equivalence. Existing exact benchmarks leave time disabled.

## Pillar moves

`Pillar` describes a nonempty group of members with aligned selected basic-variable
slots. `new` creates one column; `from_rows` accepts multiple columns per member.
Rows are canonicalized by slot order, but column order remains significant. Empty
members/columns, ragged rows and duplicate slots are rejected. Move construction
checks every selected slot and requires a uniform current value in each column.
Swaps require disjoint groups with matching column counts; member counts may differ.
All old values are read before constructing any write. Unselected slots stay intact.

`change_move` and `swap_move` return ordinary `Move::Composite` snapshots. Rebuild
these after assignment changes. Raw construction permits no-ops and out-of-range
values, just like raw assignment transactions; `FiniteProblem::apply` and score
directors enforce domains/pins atomically. Existing touched-variable discovery,
score invalidation, commit and drop rollback cover every member. The API models
selected slots explicitly and supplies no Java object metamodel or rebase operation.

## Subpillar membership selection

`SubPillarSelection::combinations(min, max)` lazily enumerates all member subsets
within an inclusive size interval. `consecutive(min, max)` restricts selection to
contiguous member windows. Both reject zero/reversed bounds; bounds above a group's
size are clipped, and groups below the minimum yield nothing. Results are ordered
by increasing size, then lexicographic member position. Each cursor retains O(k)
indices for its current size rather than precomputing a power set; usize::MAX is
supported as a maximum bound without computing binomial counts or bit masks.

`Pillar::subpillars` selects complete rows in the pillar's canonical slot order,
retaining selected-column order. It does not read assignment values; the resulting
pillar validates uniformity when building a change/swap. This permits explicit
multicolumn subsets without inferring an entity schema or storing user comparators.

## Explicit entity schemas for pillar neighborhoods

`PillarSchema::new(rows)` validates a nonempty rectangular set of disjoint selected
slot rows, retaining column order and canonicalizing row order. Empty/ragged rows
and duplicate slots are rejected. `FiniteProblem::with_pillar_schema` also checks
slot bounds before attaching immutable metadata. Clones retain the schema;
`without_pillar_schema` removes it. Newly rebuilt models need their desired schema
attached explicitly. Construction, exact search, ordinary changes/swaps and raw
model validation continue to treat all basic slots independently.

Pillar providers use selected rows as members. Keys compare all selected column
values, including None. Unselected slots participate in neither key nor writes.
Any pin among selected slots excludes the entire row; a pin in an unselected slot
does not. This conservative row-level rule is explicit native metadata, not Java
annotation or descriptor inference. Different column value families may use stable
native IDs or a user enum; target domains enforce allowed cross-values.

`pillar_swap_moves` enumerates pure whole-pillar swaps in both directed orientations.
With no schema, basic slots are single-column rows. Attached schemas also drive the
existing pillar_moves and subpillar_moves providers: a change modifies one selected
column across the chosen members, while a swap exchanges the complete selected tuple.
Every target must be legal for every selected member in every column; a single
invalid cross-value rejects the entire pair. No-ops in unchanged columns can remain
inside a valid Composite swap. Domain/pin validation and scored rollback retain
existing atomic semantics. Neither moves nor provider grouping mutates the schema.

## Branch-and-bound basic-variable search

`branch_and_bound` and `branch_and_bound_with_observer` search the finite basic
model using owned partial assignments and a priority queue. They preserve pins,
clear movable initial assignments, branch in model/domain order and include optional
None last. The callback receives remaining movable indices: this separates an
unresolved variable from a decided optional None. An empty remaining slice marks a
leaf, even when the model considers optional partial assignments complete earlier.

`BranchEvaluation` contains a partial scheduling score (actual score at leaves)
and an optional optimistic bound. Bounds must dominate every eligible completion
under full Score ordering, including structural state. None adds no bound; known
ancestor bounds still apply. The tightest supplied bound on a path permits pruning
when it cannot improve the retained best. A visited eligible leaf exceeding that
bound returns InvalidOptimisticBound. This cannot validate branches pruned before
visiting any completion. As with deterministic scoring, admissibility is a caller
contract: is_optimal is conditional on it. No automatic score-trend inference or
Java bound implementation is claimed. Unknown bounds permit unpruned exploration.

NodeExploration supports depth first, breadth first, score first and optimistic
bound first. Public SearchNodeKey comparison uses exact ordinary numeric levels,
ignoring partial structural incompleteness for scheduling. It then uses depth,
parent breadth and breadth ties as documented/tested; final native insertion order
makes queue ties deterministic. Child scores are calculated in each parent's domain
order; queue policy chooses the next partial parent. Negative structural leaves are
rejected, hard-infeasible eligible leaves retained, and equal scores keep the first
evaluated eligible leaf. Positive structural leaf values use full score ordering.

Every callback counts as one evaluation, including root and partial nodes.
Complete evaluations, opened parents, cut branches and peak queued-node count are
reported separately. A cut branch count is not its number of descendant solutions.
Budgets poll before callbacks and parent expansion, with evaluation/cancellation/time
precedence. Exhaustion after the final callback or pruning all remaining branches
wins over expired controls. Callback/observer execution cannot be preempted. A full
frontier stops with FrontierLimit; discarded unexplored work never earns optimality.
Queue size is bounded by configuration, but each queued assignment owns all variable
slots; this is not linear-memory DFS or an incremental scoring implementation.

Original checks compare all four policies with exhaustive optima, full traversal
sets, independently enumerated admissible prefix bounds, every evaluation prefix,
scoped-constraint scores, full-width decimal scores, optional ranges/pins, structural
rejection, clone failures, observer cancellation and controlled elapsed clocks.
Four complete pinned comparator methods retain every fixture and bidirectional
pair assertion. Other exhaustive-search lifecycle/branching methods remain pending;
list/mixed bounded search is described in the component contracts. Ordered solver
phase integration and automatic score-trend bounds are not yet implemented.

The comparator priorities and pruning contract use the pinned public documentation
`docs/src/modules/ROOT/pages/optimization-algorithms/exhaustive-search.adoc` as a
specification, together with the attributed test fixtures. No Timefold production
implementation was inspected or translated for this implementation.
