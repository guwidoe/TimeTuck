# Finite planning and exact search

The native planning API is an independent implementation. Its first scope is
finite basic variables. Scoped incremental scoring and bounded local search are
described in `incremental-search-contract.md`, along with greedy score-aware
construction. Constraint streams and list/shadow variables remain unimplemented.

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
specific Java exhaustive-search traces remain pending. A public-API Java oracle also checks 64 original finite graph-assignment cases
against independent Python enumeration. Both adapters normalize movable initial
values to unassigned, retaining pins. The dashboard has a separate matched
exact-search benchmark for two small problems. This evidence does not cover
nullable variables, entity-specific domains, heuristics or large-instance speed.
See `docs/dashboard.md` for measurement limits.
