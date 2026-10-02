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
use full snapshots, not incremental mixed scoring.

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
is claimed. Mixed construction/local search, incremental/stream score directors,
custom shadows and Java entity metadata remain pending. Existing Java comparisons
and timing cards continue to describe their original basic/list/stream workloads.
