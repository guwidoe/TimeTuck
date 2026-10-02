# Mixed basic/list planning

`MixedProblem<B,L>` combines a validated `FiniteProblem<B>` and `ListProblem<L>`.
The two value types can differ. Application identities relate basic variables,
list owners and list values; there is no Java entity/descriptor representation.
`MixedAssignment` exposes both components read-only. Validation enforces each
component's existing domains, unique list ownership, ranges and pins. Completeness
requires both components to be complete. Cross-component hard constraints belong
in the score, so a complete assignment can still be infeasible.

## Atomic edits

`MixedMove` contains one basic move and one list move. Either can be an empty
composite for a single-component edit, or a nested composite. The native component
rules still apply: pin/range checks see the final component state; list operations
use the current indices and reject duplicate ownership during application.

`MixedProblem::apply` first validates both starting components. It prepares both
edits on a private clone and installs the combined candidate only after both
succeed. `AppliedMixedMove` exposes the combined candidate and original, read-only.
Dropping restores both components; commit retains both. Model errors, invalid
indices and ordinary panic unwinding cannot expose a partial combined edit.
Values must obey ordinary `Clone`/`Eq` semantics; interior mutation of shared
values is outside the guarantee. Abort, forgotten guards and panicking value
destructors have the usual Rust drop-guard limitations. Preparation and rollback
use full snapshots. The incremental contribution cache is described below.

## Exact joint search

`exhaustive_mixed_search` visits the full Cartesian product of complete component
assignments and evaluates one objective over both. It does not independently
optimize each component and combine the winners: that would miss optima for
coupled constraints. Existing private component enumerators are shared with the
standalone basic/list search APIs, preserving their order and budget behavior.

List assignments vary in native list insertion order. For each list assignment,
basic variables vary in domain order, rightmost movable variable fastest, with
optional `None` last. Movable initial values are reset; pins stay fixed. Equal
scores retain the first candidate. Negative structural scores reject a candidate;
hard-infeasible candidates remain eligible. The objective must be deterministic
and must not mutate shared facts or identity through interior mutability.

One explicit budget counts calls to the combined scorer. Zero calls no scorer.
Exactly reaching the last candidate reports `Exhausted`; otherwise a consumed
budget reports `EvaluationLimit`. Either impossible component makes the product
empty without traversing the other component's space, including at zero budget.
An empty component has one empty assignment. `is_optimal()` requires exhaustion
and a returned candidate; it proves optimality only over structurally accepted
candidates, and does not prove feasibility. Enumeration uses explicit component
stacks rather than recursion proportional to problem size. Runtime remains
exponential/factorial; combined candidate cloning and full scoring are baseline
implementations without a scalability claim.

## Evidence and limits

Eight original regression methods test atomic commit/undo, invalid starts, later
component failure, panic rollback, nested composites, pins and ranges. Independent
subset/permutation/split enumeration checks the exact complete product sets for
all 64 pairs of three-value/two-owner range subsets in required and optional list
models (128 configurations), combined with an optional basic variable and a
pinned variable. Further checks cover every small-case budget boundary, ties,
empty/impossible/overflowing spaces, structural rejection and hard infeasibility.
A coupled objective confirms that joint optimization can prefer a basic value
that loses when that component is optimized alone.

`cargo run --example mixed_routes` jointly selects two vehicle depots and ordered
routes for three customers, enforces capacity through the hard score, and proves
`0hard/-4soft` after 96 complete assignments. Its expected distance is also checked
against a hand-derived lower bound. This is a correctness example, not a benchmark.

These original checks earn no upstream method credit. The inspected upstream mixed
exhaustive tests also assert Java phase types, step bounds or exact phase traces;
those remain pending. No mixed Java/Rust solver comparison or mixed timing result
is claimed. Custom shadows and Java entity metadata remain pending. Existing Java comparisons
and timing cards continue to describe their original basic/list/stream workloads.


## Incremental mixed contributions

`MixedConstraintNetwork<B,L,S>` declares the basic-variable, list-entity and
list-value counts. Named callbacks receive only their declared `MixedScope` facts:
`Basic(i)` provides an optional basic value, `ListEntity(i)` an ordered list, and
`ListValue(i)` the registry value identity plus its optional owner/index/neighbor
view. Value indices are registry positions, not value IDs. Mixed and repeated
scopes preserve callback argument order; empty scopes represent constants.
Duplicate names, invalid scopes, model shape mismatches and invalid assignments
are rejected before scoring callbacks. Failed registrations add no dependencies.

Contributions must have structural zero. The network sums ordinary levels using
`AdditiveScore`, with wrapping arithmetic for built-in integer families. It sets
the structural component to minus the combined count of missing required basic
variables and list values. Optional unassignment contributes no incompleteness.
Counts that cannot fit the structural i64 representation return an explicit error.
Callbacks must handle partial states; captured facts and identity remain immutable.

`MixedScoreDirector` owns the combined assignment and per-constraint cache, borrowing
the immutable model/network. A move reevaluates the union of constraints depending
on touched basic variables, touched entity lists, or values present in those lists
before or after the composite. This conservatively covers index and neighbor
changes. Overlapping and repeated dependencies execute once; no-op writes still
touch dependencies, while an empty composite does not. Constant contributions
stay cached. Values temporarily inserted and removed within a composite have no
changed final facts and need no separate invalidation when absent at both ends.

`ScoredMixedMove` exposes candidate assignment, score and callback count. Cache
updates are deferred until commit; rejecting, errors and ordinary panic unwinding
restore both components without changing contributions. Commit installs both state
and cache without replaying callbacks. Existing mixed transaction limitations
apply. Full `calculate` independently recomputes the score; `assert_consistent`
checks every contribution as well as the total, detecting cancelling corruption.
Diagnostic work is separate from move callback counts.

Only callback evaluation is incremental. Model validation, snapshots, structural
counting and dependency discovery still copy/scan model state; no constant-time
update or measured performance improvement is claimed. Six original regressions
include 1,200 seeded combined moves in required/optional models (400 list-only
rounds), each checked
against an independent direct formula and full network recomputation. They cover
non-dense value IDs, pinned variables/lists/prefixes, partial states, repeated
scopes, unchanged callback reuse, failed registration, invalid models/moves,
callback errors/panics, caller unwinding, cancelling corruption and BendableScore
wrapping. A public API doctest exercises coupled scoring and commit. These checks
add no upstream method credit or Java incremental mixed-scoring comparison.


## Mixed neighborhoods and local search

`mixed_moves` lazily emits basic changes (and optional swaps), followed by native
list relocations/optional assignment/unassignment (and optional swaps/replacements).
With `joint_moves` enabled, it then emits every pair of one basic and one list
move from the same starting assignment. Component domains are independent, so
valid component moves remain valid in combination. Complete states stay complete;
pins and per-entity ranges are preserved. Ordered component swaps may produce
duplicate successor states. Product enumeration does not allocate the full
neighborhood, but evaluating it may be expensive.

`mixed_local_search_from` accepts a complete combined assignment and one scoped
network. `MixedLocalSearchConfig` contains the existing `LocalSearchConfig` plus
`joint_moves` (false by default). Simultaneous moves can escape coupled traps where
either component alone worsens the score. They do not guarantee global optimality.

One evaluation budget counts the initial rescore and each trial. Accepted-move
and completed-step limits are separate; rejected fallback steps count as completed
steps. The five native acceptance policies, first/best accepted selection and
best-rejected fallback retain their existing semantics. The first global best is
retained even after worsening steps. At step boundaries the accepted-move cap
precedes the step cap. A budget-truncated scan can install its best evaluated
candidate before reporting EvaluationLimit. Exhausting the last rejected candidate
at exactly the budget reports NoAcceptedMove. Zero evaluations calls no scorer,
but still validates model shape, domains, pins, completeness and acceptance.

Candidates share one unchanged base within a scan. Winners save the combined
assignment and sparse contribution updates; installation does not replay callbacks.
Errors and ordinary unwinding after an earlier saved candidate leave the base and
caches intact. Diagnostics fully recompute without consuming search budgets or
callback statistics. Existing transaction and deterministic-callback rules apply.

ShuffledVariables shuffles basic-variable order and then list-owner order with one
explicitly seeded stream. Family order, values and insertion indices remain fixed.
VariableTabu assigns basic variable i the key i, and list entity j the key
basic_variable_count+j. Joint moves touch both sets; equal indices in different
components never collide. Equal seeds do not imply Java trace equivalence.

Seven original integration regressions and one internal regression check independent
complete neighbor sets, a coupled local trap against exact solving, lazy product
enumeration, pins/ranges, budget boundaries, cache reuse, zero-budget validation,
separate tabu identities, fallback/global-best retention and selection failure after
saving a candidate. Fifteen seeded policy/order combinations repeat exactly and
reach the known optimum of the small coupled instance. The mixed_routes example
now also reaches its exact 0hard/-4soft optimum through local search in 127
evaluations, 234 callbacks and one completed step. These are native correctness
and quality checks, not Java mixed-search or runtime performance comparisons.
Java mixed solver comparisons remain pending; no extra upstream test-method credit
is claimed.


## Mixed greedy construction

`greedy_mixed_construction` fills missing required values using one combined
contribution cache. `MixedConstructionConfig` wraps the existing construction
configuration and an explicit `MixedConstructionOrder`: `BasicThenLists` (default)
or `ListsThenBasic`. Basics follow model/domain order; lists follow value registry,
owner and insertion-index order. First equal scores win. Coupled callbacks must
handle partial states in either phase; construction order can affect final quality.

All supplied basic values and initial list relative order survive construction,
including unpinned values. Pins and owner-specific ranges remain enforced. Optional
basic `None` and optional missing list values remain unchanged for local search.
There is no backtracking or guarantee of hard feasibility or optimality.

One evaluation budget includes the initial score and every trial in both phases.
Selection saves the winning assignment and sparse cache updates without rescoring.
A truncated scan commits its best evaluated prefix candidate and reports
`EvaluationLimit`, even if the combined state is now complete. Finishing exactly
at the budget reports `Completed` when no further candidate remains. All four
native early-pick policies use ordinary score levels, ignoring structural missing
counts. Diagnostic full recomputation is outside budgets and callback statistics.

`MixedConstructionResult` exposes the partial assignment, optional score,
completeness, separate counts of newly assigned basics/list values, and work counts.
`solution()` requires completeness and a calculated score, but not hard feasibility.
Even an already complete model has no scored solution at zero budget. Shape/domain
validation precedes callbacks; empty required basic domains or unreachable required
list values return `NoCompleteAssignment` before scoring, including at zero budget.
Errors and ordinary unwinding expose no mutable model state.

Seven original regressions cover both orders, every small budget boundary, ties,
cache reuse, early selection, impossible/empty/complete models, shape errors,
initial values, optional values, pins/ranges, callback errors/panics after progress,
and a coupled objective checked against exact search. The routing example now
constructs from an empty model with both orders, checks independent complete scores,
and improves both to the proven `0hard/-4soft` optimum. Basic-first construction
starts at `0hard/-38soft` and uses 141 combined evaluations / 249 callbacks;
list-first starts at `0hard/-18soft` and uses 173 / 313. Local search's initial
rescore is included. These work counts are correctness evidence, not runtime
benchmarks or Java equivalence claims. No upstream method credit is added.


## Transactional mixed streams

`MixedStreamSources<B,L>` groups the basic planning source and the existing
`ListStreamSources<L>` entity/value sources. `MixedStreamScoreDirector` binds all
three to one graph and one combined assignment. Basic rows use stable variable
indices and `(index, Option<B>)` payloads. Lists use owner indices and ordered
vectors; value rows use registry indices plus identity/owner/index/neighbor views.
Every row is loaded, including optional/required unassigned values and empty lists.
Constraints explicitly filter unassignment and join component identities as needed.
Other graph sources are immutable facts and may be populated before binding.
All three planning sources must belong to the graph and start empty.

Combined moves validate both final components before any graph callback. They then
deliver final touched basic rows, touched entity lists and affected value/shadow
rows in that order, each once. Values in a touched list before or after are
conservatively refreshed. Intermediate composite assignments are never delivered,
but joins can observe intermediate source updates. Callbacks must handle those
states; only the fully drained candidate is final. Structural score is minus the
combined required missing count, with the existing explicit overflow check.

Dropping, graph/model errors and ordinary unwinding restore both assignments and
all changed graph roots. Commit replays no callbacks. Saved candidates retain
persistent graph roots and their assignment; an error after saving an earlier
winner leaves the base unchanged. Full rebuild diagnostics compare terminal match
identities/scores and restore the prior graph on failure. Existing graph callback,
immutable-fact, allocation-abort and destructor limitations apply. Assignment
snapshots, validation and structural counting still scan/copy model data.

`greedy_mixed_stream_construction` and `mixed_stream_local_search_from` share the
same internal algorithms as scoped mixed scoring. Orders, policies, optional joint
moves, budget boundaries, ties and global-best retention therefore follow the
contracts above. Source ownership/emptiness and model validation precede zero-budget
returns. Each phase initializes a fresh director. `constraint_evaluations` counts
terminal weighting calls, including intermediate join updates, and excludes
preloaded-fact processing and diagnostic rebuilds. It is not a predicate/key count
and is not directly interchangeable with scoped contribution counts.

Seven original integration regressions include 800 seeded combined moves in
required and optional models (seed 20261106), independent full-score and source-row
checks, pins/ranges, preserved snapshots, partial composites and exact match tuples.
They also check no-op work, zero-budget validation, impossible models, later graph
errors/panics, caller unwinding and failed rebuild recovery. Construction agrees
with scoped scoring for both component orders, all four early-pick policies and
budgets 0 through 7. Local search agrees for all five acceptance policies, both
selection policies, joint moves on/off and eight budget limits (seed 20261107).
An internal regression verifies rollback after saving an earlier joint candidate.

The routing example reaches its proven `0hard/-4soft` optimum with both stream
construction orders: basic-first uses 141 combined evaluations / 353 terminal
calls; list-first uses 173 / 469. Independent direct scores validate construction
and final solutions. These are native correctness/work counts, not Java mixed
comparisons or runtime benchmarks. Upstream coverage is unchanged.
