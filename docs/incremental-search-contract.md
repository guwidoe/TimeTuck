# Incremental scoring and local search

The native implementation now provides a static scoped constraint network,
transactional score caching, lazy basic-variable neighborhoods, and bounded local
search. It is not a constraint-stream implementation or complete Timefold solver.

## Scoped constraints

`ConstraintNetwork<V, S>` declares the model's variable count. Each named
constraint has an ordered list of variable indices and a deterministic callback
receiving only those variables as `Option<&V>`. Repeated scope indices are passed
through to the callback. An empty scope is a constant contribution. Duplicate
names and out-of-range indices are rejected before registration.

The network sums ordinary score components using `AdditiveScore`. The built-in
integer scores use modular arithmetic, as documented in `score-contract.md`.
Constraint contributions must have structural zero. The network sets structural
state to minus the number of unassigned required variables; optional `None` values
are complete choices. This is the native model's incompleteness convention, not a
claim that every Timefold structural-validation behavior has been implemented.
Callbacks must handle partial assignments when their scoring session permits them.
Captured facts and value identity must remain immutable during a session.

`ScoreDirector` owns its assignment and contribution cache and borrows the model
and network. A candidate move uses the existing reversible assignment transaction.
Touched variables identify dependent constraints; each is reevaluated once even
when a composite writes a variable repeatedly or a scope repeats an index.
Untouched contributions remain cached. Domain/pin errors and scoring errors reject
the move. Ordinary unwinding also restores the assignment.

`ScoredMove` exposes a read-only candidate assignment and score. Its cache updates
are deferred: dropping it rejects the move without changing any cache entry;
`commit()` keeps both values and score updates. Existing drop-guard limitations
(abort, forgetting the guard, panicking value destructors) still apply. An external
fact mutated through interior mutability cannot be rolled back by this transaction.
`assert_consistent()` recalculates every contribution as well as the total, detecting
corruption even when two wrong contributions cancel. Diagnostic recomputation is
optional during search.

Only constraint callbacks are incremental in this first implementation. Model
validation and structural counting still scan variables; callbacks allocate small
argument vectors, and affected-constraint indices are sorted/deduplicated. No
constant-time move update or broad runtime-speedup claim is made.

## Neighborhoods and search

`change_moves` emits domain values in declared order, excluding the current value
and pinned variables, and emits `None` last for optional variables. `swap_moves`
emits ordered pairs, including both `(a,b)` and `(b,a)`, but skips pins, equal
values and incompatible target domains. Unassigned values may only move into
optional variables. Both iterators validate their starting assignment and generate
moves lazily without collecting a neighborhood-sized list.

`local_search` starts with existing initial values and pins. It fills each required
unassigned variable with its first domain value; optional `None` is retained. This
is first-fit initialization. For a score-aware start, use `greedy_construction`
and pass its complete assignment to `local_search_from` (described below).

Search scans changes, optionally followed by swaps, and commits the first accepted
candidate before restarting its scan. Default strict improvement accepts only
better scores. `Acceptance::HillClimbing` also accepts ties, matching the adapted
upstream acceptance assertions; plateaus may cycle, so limits remain essential.
Late acceptance additionally permits a worse current score when it meets the
historical threshold. It is described below. Tabu, annealing and parallel search
remain unimplemented.

Configuration requires finite evaluation and accepted-move limits (defaults are
10,000 and 1,000). The initial score counts as one evaluation; each considered move
counts once. A zero evaluation budget never invokes a scoring callback. Optional
assertion-mode recomputations are excluded from budgets and callback statistics.
At a step boundary the accepted-move limit takes precedence. Otherwise the
iterator checks for a next move before applying the evaluation cap: exhausting the
last rejected move exactly at the budget reports `NoAcceptedMove`.

Results retain the earliest best complete assignment. A hard-infeasible best is
still returned honestly. Termination distinguishes evaluation limit, accepted-move
limit, empty complete space, and a neighborhood with no accepted move. None is a
proof of global optimality. Tests include a local trap with a strictly better global
optimum to enforce that distinction.

## Evidence and limits

Nine additional fully adapted upstream methods cover provider behavior and the
hill-climbing acceptance rule. All source methods, fixture initial states and
modifications are recorded in `compat/provenance.json`. Original tests cover
2,000 seeded change/swap sequences against an independent full-score formula,
error/panic rollback, per-constraint cache checks, modular overflow, pins/optional
ranges, plateau limits, known separable optima, swap-only improvements and generated
graph solutions checked against independent optima and every local neighbor.

Run the original larger example with:

```sh
cargo run --release --example graph_coloring
```

It solves 100 variables with four colors and 400 scoped constraints. The current
deterministic configuration constructs `0hard/0soft` in 401 score evaluations,
then checks local improvements in 301 further evaluations with no accepted moves.
The combined 702 evaluations invoke 5,700 constraint callbacks; full rescoring of
those evaluations would invoke 280,800. This is a callback-count comparison,
not a Java comparison or measured runtime speedup. The existing dashboard benchmarks
still cover score operations and two small exhaustive problems only.


## Late acceptance and seeded ordering

`Acceptance` is configuration. `start(initial_best)` creates a fresh `Acceptor`
session; `accepts(current, candidate)` never mutates its state, and `step_ended`
records only a committed move. Local search manages these calls automatically.
This replaces the earlier stateless configuration-level `accepts` method.

`LateAcceptance { history_size, hill_climbing }` uses a circular history initially
filled with the phase's best score. A candidate is accepted if it is at least the
current history entry, or (when enabled) at least the current score. Completing a
step stores that step's score, even when worse than the global best, then advances
the cursor. A new best that improves hard levels resets all history entries to
that best; soft or medium improvements do not reset them. `Score::hard_cmp` compares
hard levels only. Structural components are not hard levels, and local-search
phases use complete assignments with structural zero.

Zero history is rejected before scoring, even with zero evaluation budget. History
allocation failure is reported as an error. The native size is unsigned; the
upstream negative-Java-integer case remains pending, as does its Bendable score
case. Four full upstream methods are adapted, preserving 72 acceptance assertions,
all five hard-reset history assertions, and the zero-size rejection.

`MoveOrder::Original` preserves the existing variable order. The optional
`ShuffledVariables { seed }` uses SplitMix64 and a Fisher–Yates shuffle with
rejection-sampled indices. Each neighborhood scan shuffles variable indices once;
value order remains the declared domain order, and changes still precede swaps.
Swaps use that same variable permutation on both axes. This requires linear index
storage, not a materialized list of all moves. It is not uniform random sampling
of moves and does not match Java's PRNG or trace. Reproducibility requires the same
inputs, configuration, and implementation revision.

Original tests demonstrate a seeded late-acceptance run crossing a score valley
that stops strict hill climbing with the same seed. They also verify best retention
after a worsening final step, history immutability during trial moves, size-one
history, hard/medium distinctions, invalid configuration, and bounded/reproducible
runs across 64 seeds. These tests establish specific behavior, not general solution
quality. The dashboard still has no matched Java local-search benchmark.


## Greedy construction and explicit phase handoff

`greedy_construction` visits unassigned required basic variables in model order.
It preserves all supplied values and pins, and leaves optional `None` values as
complete choices. Every candidate for a variable is scored against the same prior
assignment. The first highest-scoring domain value wins; with a budget limit, only
the evaluated prefix participates. This is a greedy algorithm without backtracking.
Callbacks must support partial assignments.

`ConstructionPickEarly` can scan the full domain (`Never`), or stop at the first
non-deteriorating ordinary score, first hard-feasible score, or first hard-feasible
score / non-deteriorating hard score. These predicates ignore the network's
negative structural count of required unassigned variables. A hard-feasible
partial candidate is still incomplete. The four adapted upstream predicate tests
preserve all score values, unassigned counts and assertions. The separate upstream
structurally flawed rejection test remains pending: native incompleteness does not
represent that additional validation state.

Initial scoring counts once. Every trial counts once, including when no constraints
are affected. Selecting the winner installs its retained assignment and sparse
cache updates without replaying callbacks. The director owns selection for the
whole scan, so a retained candidate cannot be installed into another session or
against a changed base. Saving an improved candidate clones the assignment and
its affected contributions; this is not a constant-time selection operation.
Errors and ordinary panic unwinding before installation preserve the previous
assignment and cache. Usual value-cloning and destructor limits still apply.

Results expose the current assignment (possibly partial), optional score,
completeness, assigned-variable count, score evaluations, constraint callbacks and
termination. `solution()` returns a scored assignment only when complete. A zero
budget performs no scoring, even for an already complete model. An impossible
required domain reports `NoCompleteAssignment` before scoring. `EvaluationLimit`
means a candidate remained unevaluated; the selected prefix can nevertheless have
completed the final variable. An exactly exhausted full domain, or an early pick,
can finish at the limit and report `Completed`. None implies hard feasibility or
optimality. Diagnostic full recomputations are explicitly excluded from statistics.

`local_search_from` validates a supplied complete assignment against the original
model, including its pins. Incomplete or invalid assignments are errors even with
zero budget. It initializes a fresh score director; this initial score counts
against the local-search budget. Construction and search have separate budgets;
sum their reported evaluations/callbacks for total work. The example performs this
handoff and reports combined work. No untrusted external score cache is accepted.

Original regressions exercise all budget boundaries, partial/complete separation,
zero work, impossible/empty models, pins and optional values, tie order, infeasible
complete results, early picking, callback counts without replay, phase handoff,
and failure rollback after retaining an earlier candidate. Across 24 interacting
constraint networks, incremental greedy choices match an independent full-score
selection loop. The new work has no matched Java heuristic benchmark yet.
