# Incremental scoring and local search

The native implementation now provides a static scoped constraint network,
transactional score caching, lazy basic-variable neighborhoods, and bounded local
search. List/entity and element-scoped contribution caching is described in
`list-planning-contract.md`; mixed basic/list scoped contributions are described in
`mixed-planning-contract.md`, including mixed local search and joint neighborhoods. Stream-backed construction and local search use the
same basic-variable algorithms; see `constraint-stream-contract.md`. Whole-solver
compatibility remains incomplete.

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

Search scans changes, optionally followed by swaps, and by default commits the
first accepted candidate before restarting its scan. `MoveSelection::BestAccepted`
selects the first highest-scoring accepted candidate over the evaluated prefix
instead, using saved assignment and sparse cache updates without rescoring it. Default strict improvement accepts only
better scores. `Acceptance::HillClimbing` also accepts ties, matching the adapted
upstream acceptance assertions; plateaus may cycle, so limits remain essential.
Late acceptance additionally permits a worse current score when it meets the
historical threshold. Variable tabu allows worsening candidates that do not touch
recently selected variables, with an optional global-best exception. Both are
described below. Seeded simulated annealing is described below as well. Parallel
search remains unimplemented.

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
separate score operations, small exhaustive problems and bounded full-scoring
heuristic problems; this incremental callback count is not a runtime benchmark.


## Late acceptance and seeded ordering

`Acceptance` is configuration. `start(initial_best)` creates a fresh `Acceptor`
session; `accepts(current, candidate, variables)` takes a mutable session because
annealing trials consume randomness. Deterministic acceptance checks do not
change their state, and
`step_ended(score, variables)` records only a committed move. Supply the touched
variable indices for tabu; score-only acceptors ignore that slice. Local search manages these calls automatically.
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
upstream negative-Java-integer case remains pending, while the Bendable score
case is now adapted as described in `score-contract.md`. Four full upstream methods are adapted, preserving 72 acceptance assertions,
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
quality. The dashboard now includes a matched full-scoring heuristic comparison; see
`dashboard.md`. It does not measure incremental-scoring performance.


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
selection loop. Construction itself is not included in the matched Java heuristic benchmark,
which starts from supplied complete assignments.


## Variable tabu and best-accepted moves

`Acceptance::VariableTabu { tenure, aspiration }` prohibits candidates touching
variables from the most recent `tenure` completed steps. Tenure must be positive;
zero is rejected before scoring, including with zero evaluation budget. Every
variable touched by one step has the same lifetime, and repeated touches refresh
it. A selected swap records both variables. Trial checks do not change memory,
and only selected moves update the session's best score. When aspiration is on,
a strictly better score than the global best bypasses tabu. A tie does not, and
improving only the current score is insufficient. Non-tabu candidates may worsen
the current score. Results continue to preserve the earliest global best.

The native model has individual basic variables, not separate multi-variable
entities. `TabuMemory<K>` supports stable equality-based keys; the integrated
acceptor uses variable indices. Three upstream entity-tabu methods map each test
entity to one variable ID, preserving all 96 assertions and every completed step.
No general multi-variable entity, move/value tabu, fading probability, ratio-size
strategy or Java negative-size compatibility is claimed. Memory uses countdowns
rather than an ever-growing step counter, with no up-front tenure-sized allocation.
Its current linear key lookup is a correctness baseline, not a throughput claim.

For a bounded tabu run with best-candidate selection:

```rust
use timetuck::{Acceptance, LocalSearchConfig, MoveSelection};
let config: LocalSearchConfig = LocalSearchConfig {
    acceptance: Acceptance::VariableTabu { tenure: 2, aspiration: true },
    selection: MoveSelection::BestAccepted,
    max_evaluations: 10_000,
    max_accepted_moves: 500,
    ..LocalSearchConfig::default()
};
```

Best-accepted selection evaluates candidates from one unchanged prior assignment
and filters them through the acceptor before ranking. Equal accepted scores retain
the first candidate. At an evaluation limit it may commit the best accepted prefix
candidate and reports `EvaluationLimit`; this selected step counts toward the
accepted-move statistic. If the scan completes, the ordinary step-boundary limits
apply. Empty/rejected neighborhoods stop without aging tabu memory. This can happen
when tenure covers all movable variables; search does not synthesize idle steps to
wait for expiry. The default first-accepted mode retains its previous behavior.

Original regressions check tabu against a separate 1,000-step history log for
three tenures, duplicate keys, empty steps, very large tenure, strict aspiration,
and rejected zero tenure. Search tests verify crossing a score valley, preserving
a best solution after worsening, both sides of a selected swap, 32 seeded runs
with pins/optional values/heterogeneous domains, best-vs-first selection, stable
ties, sparse winning-cache updates and exact budget/callback accounting. Existing
construction, late-acceptance and neighborhood tests still pass after sharing the
candidate-selection mechanism. A separate Java/Rust benchmark now exercises these native policies with full
score recalculation, matched completed-step budgets and independent result checks.


## Rejected fallback and completed steps

The public-API Java comparison exposed a forager difference: when all candidates
are rejected, Timefold still chooses the highest-scoring rejected move. The pinned
Community test `AcceptedLocalSearchForagerTest.pickMoveMaxScoreUnaccepted` confirms
this behavior. Native `RejectedMovePolicy::BestScore` explicitly enables that
fallback; the default `Stop` preserves existing stopping behavior. Accepted
candidates always outrank rejected candidates, even when they score worse. A
negative structural score is not eligible as a rejected fallback. This behavior
has original regression tests, not additional adapted-method coverage credit.

`max_steps` bounds every committed step, including rejected fallbacks. The result
reports `completed_steps` separately from `accepted_moves`; fallback steps update
acceptor history and the global best, but do not count as accepted moves. Both
limits default to 1,000. At a step boundary, accepted-move limit takes precedence
over completed-step limit; zero completed steps still permits initial scoring if
the evaluation budget allows it. During a partial scan, `EvaluationLimit` takes
precedence after retaining the selected prefix candidate. Exhausting an empty
neighborhood still stops without synthesizing a step.

The benchmark enables fallback, disables an independent accepted-move cap, and
uses the same completed-step cap as Java. This corrected the initial pilot's
mismatched work counts; tested deterministic inputs then matched scores,
assignments and callback counts. No general Java trace-equivalence claim follows.


## Simulated annealing

`Acceptance::SimulatedAnnealing { starting_temperature, cooling_steps, seed }`
adds a separate seeded random stream to local search. `Acceptance<S>` and
`LocalSearchConfig<S>` are now generic over the score so temperature has exactly
the problem's score type. Type inference usually supplies `S` from the network;
standalone configuration annotations default to `SimpleScore`. `Score` exposes
ordered integer `ordinary_levels()` as an iterator for the current integer score
families, including BendableScore.

Starting temperature requires structural zero and nonnegative ordinary levels.
The cooling duration must be positive; validation runs before scoring, including
when evaluation budget is zero. All-zero temperature is valid hill climbing.
The gradient is completed steps divided by cooling duration, capped at one.
Temperature scales by `1 - gradient`. This is a deterministic step schedule, not
wall-clock termination or a Java time-gradient claim. Search limits and cooling
duration are independent; search can end before cooling finishes or continue cold.
A discarded candidate does not cool. A selected rejected fallback does cool.

For structurally sound scores, ties and lexicographic improvements always pass.
For a worse candidate, native energy is the sum of each positive ordinary-level
loss divided by that level's current temperature; probability is `exp(-energy)`.
Softer improvements do not cancel losses. A declining level with zero temperature
makes probability zero. Structural nonzero inputs to the standalone policy are
rejected. Multi-level energy is an explicitly native policy: the adapted source
assertions establish scalar acceptance and negative-temperature rejection only.

Loss subtraction uses i128 before conversion to f64, avoiding wrapping and
preserving adjacent integer deltas above 2^53. Exponential evaluation and cooling
use floating point; no bit-identical probability/trace across platforms is promised.
The PRNG draws from a 53-bit uniform grid in [0, 1), independent of variable-order
shuffling. Draws occur only for probabilities strictly between zero and one.
Cloning a session clones its random position and cooling state. Reproducibility
requires the same problem, configuration, platform and implementation revision.

The standalone `SimulatedAnnealing` exposes `probability_at` and `accepts_at` to
inspect probabilities or replay supplied gradients/draws without mutating state.
It rejects non-finite/out-of-range gradients and draws. `accepts` uses the internal
clock/random stream; `step_ended` advances the capped clock. Local search manages
these calls, including returned-best retention and evaluation/step accounting.
The default first-accepted selection is appropriate for classic annealing;
best-accepted selection and rejected fallback remain explicit alternatives.

Two source methods preserve all 12 upstream acceptance decisions and negative
HardMediumSoft temperature rejection. Ten original regression methods check
probability boundaries, zero temperatures, native multi-level energy, large
integer deltas, validation before callbacks, cooling and random consumption,
seeded acceptance frequency, best retention, fallback and exact limits. Across
32 fixed seeds, a two-variable valley test reaches an independently enumerated
optimum that strict improvement cannot reach. This is a small correctness and
quality case, not a general quality guarantee or measured annealing benchmark.

```rust
use timetuck::{Acceptance, HardSoftScore, LocalSearchConfig};
let config: LocalSearchConfig<HardSoftScore> = LocalSearchConfig {
    acceptance: Acceptance::SimulatedAnnealing {
        starting_temperature: HardSoftScore::new(2, 100),
        cooling_steps: 1_000,
        seed: 20261001,
    },
    max_steps: 1_000,
    max_evaluations: 50_000,
    ..LocalSearchConfig::default()
};
```


## Stream scoring backend

The scoped entry points retain their signatures. `greedy_stream_construction`,
`stream_local_search` and `stream_local_search_from` accept a typed stream graph
and its initially empty planning source. A private director interface shares the
construction and local-search control flow, so acceptance, neighborhood order,
best retention, fallback and limit precedence stay identical. The stream director
saves candidate assignment/node state and installs a winner without callback
replay. Its saved graph state now shares persistent map roots; updates copy only
affected tree paths. The scoped director retains its sparse contribution cache.
Assignment snapshots and model checks retain their existing linear work. Runtime
performance comparisons of these stream operations remain pending. Both result
types count score evaluations; for streams,
`constraint_evaluations` counts terminal scoring invocations, including temporary
propagation results, and excludes graph preparation and diagnostic rebuilding.

Stream diagnostics reset operator caches, replay owned source rows and compare
terminal match IDs, contributions and totals before restoring the original graph.
They detect opposing errors that leave the total unchanged. They do not establish
independent semantic correctness of a constraint callback; original cross-checks
also compare whole searches with separately written full-scoring loops. See
`constraint-stream-contract.md` for the current validation scope and limitations.
