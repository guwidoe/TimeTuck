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
still returned honestly. Termination distinguishes evaluation, accepted-move and
completed-step limits, the optional criteria below, empty complete space, and a
neighborhood with no accepted move. None is a proof of global optimality. Tests
include a local trap with a strictly better global optimum to enforce that distinction.

## Optional deterministic termination

All three local-search models, with scoped or stream scoring, accept optional
`max_unimproved_steps`, `best_score_limit` and `stop_when_feasible` fields. They are
disabled by default. The retained best is updated only by a selected step with a
strictly greater score; ties preserve the earliest best assignment. A stagnation
limit measures completed steps since that improvement, or since initial scoring
if no improvement has occurred. Accepted worsening, recovery to an earlier best,
ties and selected rejected fallback steps all count without resetting it.
An exhausted neighborhood completes no step. Results expose `unimproved_steps`.

The score target compares the complete score using its ordinary ordering; reaching
it need not establish hard feasibility. Feasibility uses `Score::is_feasible`,
including every hard level and structural state. Negative soft scores do not
prevent feasibility. A complete SimpleScore assignment has no hard levels and
already satisfies feasibility. These are stopping criteria, not optimality proofs.

Criteria are checked after initial scoring and at completed-step boundaries.
Precedence there is accepted moves, completed steps, score target, feasibility,
then unimproved steps. A zero evaluation budget ends before initial scoring.
Candidate selection that detects exhausted evaluation budget installs any saved
winner and returns `EvaluationLimit` before another boundary check. Detection
requires a next candidate: consuming the final evaluation on the last candidate
alone does not mark selection exhausted. A first-accepted choice can therefore
reach a score target at its next boundary where a best-accepted scan of a truncated
neighborhood reports the evaluation limit instead. Existing behavior is preserved.

`StepProgress` records completed counts and the last strict best step;
`StepCountLimit` supplies the threshold and capped fraction. Zero is reached
immediately with fraction 1. Large counts use f64 precision for the fraction,
while threshold decisions remain exact integer comparisons. Two adapted upstream
methods preserve all twelve threshold and twelve gradient assertions for step and
unimproved-step limits. Eleven original integration regressions cover native
boundaries, resets, fallback steps and both backends across all model types.
Nine additional integer score-target/feasibility methods now have complete
adaptations; the two decimal score-target gradient methods and upstream unimproved-time
termination remain pending.

`ScoreTargetProgress` fixes a starting score, target score and N-1 split weights
for N ordinary levels. Each weight receives that fraction of the remaining share,
and the final level receives the rest. Per-level fractions are capped to [0,1];
flat or reversed intervals earn their share if the current level meets target.
An exact target check returns 1 first; a lexicographic regression below the start
returns 0. `FeasibilityProgress` uses only the hard levels and zero as their target,
without letting positive hard levels compensate for negative ones. Both reject
structurally flawed reference scores and invalid/missing/nonfinite weights. Flawed
current scores produce zero progress and do not meet the stopping criterion.
No-hard-level types are feasible once structurally sound.

These reusable estimates do not change local search's existing stopping rules or
automatic completed-step annealing clock. Callers can supply their fraction to
`SimulatedAnnealing::probability_at` or `accepts_at`. Use the separate `is_reached`
method for exact termination: zero weights and floating rounding can produce 1
before the target is reached. Integer differences are widened to i128 before
conversion to f64 so adjacent large scores are not lost and full-width differences
do not overflow. This is a native numeric policy, not a Java overflow-parity claim.
Seven original regressions cover these bounds, structural flaws, all-hard-level
feasibility, invalid inputs and explicit annealing use. Nine source adaptations
preserve every termination/gradient assertion for scalar, HardSoft and Bendable
integer scores; they replace mock phase/solver scopes with fixed score references.
No Java lifecycle or direct Java gradient-oracle comparison is claimed.

Original direct Community comparisons now cover 219 deterministic required-basic
cases (seed 20261119). They use public OR-composed phase limits and original-order
best-accepted hill climbing, first ties and best-rejected fallback. Cases include
zero limits, initial targets, pins, no moves, signed preferences, unreachable
criteria and a delayed improvement after worsening and recovery. That last fixture
(found with seed 20261120) returns different scores under different stagnation,
step, score-target and feasibility limits. All final scores and assignments agree
with independent Python neighbor enumeration. Rust scoped and stream backends also
match 398 reference completed steps in aggregate, per-case stagnation/acceptance
counts and native reasons. Java internal counts, reasons and traces are not
exposed or inferred. No list/mixed-search, stochastic or timing claim follows.
These original cases earn no upstream method credit. Five original validator
regressions check literal milestones and reject incorrect solutions and metadata.
Run `python3 scripts/differential_solver.py` to refresh this evidence with the
exact-search checks; raw cases, Python traces and both runtime outputs are saved
in `work/oracle/termination-cases.json`.

## Cooperative elapsed-time limits

`LocalSearchConfig::max_time` optionally bounds the duration of each local-search
phase across basic, list and mixed models with either scoring backend. Normal
runs use `Instant`; disabled timing does not start or poll a clock. The limit
includes initial scoring, first-fit initialization, phase validation, candidate
selection and enabled diagnostic recomputations. Model/network/graph preparation
before the search call is outside this phase. Construction has a separate
max_time control; multi-phase solving also accepts a shared limit as described in
`solver-contract.md`.

The clock is checked after existing step-boundary criteria, before scoring each
emitted candidate, and after selection and diagnostics. Expiry stops the lazy
candidate iterator, installs any selected winner from the evaluated prefix, and
returns the retained global best with `TimeLimit`. A prefix with no selected move
also reports `TimeLimit`, not neighborhood exhaustion. A currently running
callback, validation, move enumeration or diagnostic operation cannot be interrupted;
this is a cooperative limit and can overshoot. Zero time still permits initial
scoring and returns that initial solution unless an earlier criterion applies.
Invalid input, empty complete space and zero evaluation budget retain their earlier
handling.

An evaluation limit already observed during selection takes precedence over the
post-selection time poll. If the time poll prevents a next candidate from being
emitted, selection never observes that candidate's evaluation cap and time wins.
Post-selection expiry precedes a new step-boundary count/score criterion. Tests
cover both orders so timing is not mistakenly reported as local optimality.

`ElapsedTimeLimit` exposes exact Duration threshold decisions and capped floating
progress; zero is reached with fraction 1. Negative durations are unrepresentable.
The two adapted TimeMillisSpentTerminationTest methods preserve all twelve
threshold and twelve gradient assertions. Eight original unit regressions inject
controlled clocks into the actual shared search algorithms, covering all six
model/backend combinations, winner installation, rejected prefixes, initial
scoring, disabled-clock behavior, zero limits, overlapping criteria, monotonic
clamping and irreversible expiry. A public real-clock zero-limit call is checked
without sleeps.

`max_unimproved_time` optionally stops the same six search paths after a duration
without a strict retained-global-best improvement. Its clock starts after initial
scoring and initial diagnostics. Only an installed step with a new strict best
resets it; ties, worsening fallback, recovery to an old best and unselected trials
do not. Initialization is excluded even if it takes longer than the configured
interval. Zero stops before the first move while preserving the initial solution.

Both time limits use one monotonic clock. Before resetting stagnation, the search
polls it; an improvement installed at or after the deadline is retained but cannot
revive an expired budget. Expiration latched during candidate enumeration similarly
survives installation of its best prefix. `UnimprovedTimeLimit` distinguishes this
stop from total `TimeLimit`; total time wins if both are first observed expired in
the same poll. Observed evaluation exhaustion retains its existing precedence.
Extra improvement/start samples occur only when stagnation timing is enabled.
Six additional original controlled-clock tests cover initialization, strict/tied
improvements, fallback/recovery, prefix retention, callback overrun, simultaneous
expiry and backwards/maximum-duration samples. They earn no upstream method credit.

Direct Java timed-search comparisons, complete upstream unimproved-time lifecycle
adaptations and hard preemption remain pending. Deterministic benchmarks disable
both time limits.

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
the cursor. A new best that improves the score family's reset levels resets all
history entries to that best. `Score::late_acceptance_cmp` uses hard levels by
default, and hard plus medium for HardMediumSoft integer/decimal scores. Bendable
families retain only their declared hard levels; soft changes do not reset them.
`Score::hard_cmp` and feasibility remain hard-only. Structural state is excluded
from reset-level comparison; local-search phases use complete assignments.

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
No general multi-variable entity, move/value tabu, ratio-size
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


Opt-in `Neighborhood::ChangeSwapAndSubList { max_length }` adds lazy contiguous
list-span relocation/reversal and disjoint span swaps after the existing families. Both list/mixed scoring
backends share it; mixed joint moves combine basic moves with spans. Basic-only
search retains changes/swaps under this setting. Bounds below two add no spans.
Relocations use post-removal indices; swaps use pre-move nonempty disjoint spans.
See the list-planning contract for bounds, pins/ranges and evidence.
Default benchmarks continue to use their existing single-value neighborhoods.


`Neighborhood::TwoOpt { cross_entity }` uses only in-place reversals and optional
non-reversing nonempty cross-owner tail swaps for lists. Basic search keeps its
change/swap set; mixed joint search combines those basics with this list set.
Optional-list assignment status and empty owners are unchanged by these moves.
The dispatcher stores one boxed selected list iterator so nested mixed products
remain bounded in stack size without materializing moves. All six search paths
retain their existing acceptance and termination mechanisms. Source-adapted
provider checks and original end-to-end search checks are documented in the list
contract; the default benchmark workloads do not select two-opt.


## Seeded candidate-window ordering

`MoveOrder::ShuffledMoves { seed, window: NonZeroUsize }` reorders the emitted
candidate stream for basic, list and mixed search in both scoring backends. Mixed
ordering covers the combined basic/list/joint stream. It leaves the underlying
variable order unchanged and uses a separate SplitMix64 state from the acceptor.
The state continues across steps. Original and ShuffledVariables retain their
previous ordering and random sequences; one-slot candidate windows preserve
Original behavior without consuming random draws.

A scan starts with an empty buffer. Each request fills up to the effective window
from the lazy source, then removes a uniformly selected buffered move. Subsequent
requests replenish the free slot. Fully consuming a scan preserves every emitted
candidate exactly once, including existing duplicate successor representations.
If the window covers the entire finite neighborhood, this is a uniform permutation.
A smaller window shuffles locally and is not uniform across the whole neighborhood.

The effective window is min(requested window, remaining evaluation budget), with a
minimum of one to preserve existing exhausted-budget detection. Buffer allocation
is incremental and never pre-reserves the requested size. Construction does not
pull candidates. The first output needs at most window source moves, later outputs
at most one new move each. Generation is unscored; early acceptance discards unused
lookahead. Time limits are checked around emitted candidates, so bounded prefetch
can overshoot cooperative timing just as generation/scoring callbacks can.
Reproducibility requires identical input, configuration/budgets and implementation.

Three original unit tests check candidate conservation and repeatability across
64 seeds/window sizes, bounded lazy pulls even with usize::MAX and tiny budgets,
and empty/singleton/non-fused inputs. Three original integration methods cover
neutral-cycle escape through all six model/backend paths, identical backend
scores/counters/termination, repeatability, one-slot identity and budget limits.
They establish specific cases, not a general quality or Java PRNG equivalence.


## Cooperative external cancellation

`CancellationToken` is a cloneable, thread-safe, one-way request shared by any
local-search phases configured with `cancellation: Some(&token)`. `cancel()` returns
true only for the first request across all clones. There is no reset; use a fresh
token for an independently cancellable run. Equality compares shared token identity,
not its changing flag. All three model types and both scoring backends return
`LocalSearchTermination::Cancelled` when they observe the request.

`LocalSearchConfig<'a, S>` borrows the token to remain Copy; mixed configuration
carries the same lifetime. Named type aliases and helpers returning configurations
may need an explicit lifetime. A helper returning a configuration without a token
can use `'static`; one returning a borrowed token must use that token's lifetime.
The default remains `None`.

Model, initial-assignment and acceptance validation precede cancellation. An empty
basic search space still returns NoCompleteAssignment, and a zero evaluation
budget still returns EvaluationLimit. Otherwise a request observed before scoring
returns no best solution and zero evaluations/steps. Cancellation during initial
scoring retains the scored initial solution. Cancellation during candidate scoring
allows the callback to finish, then installs the selected eligible candidate from
the evaluated prefix, preserving the retained global best even when rejected-move
fallback installs a worse working solution.

Search polls before initialization, at each step boundary, between candidates and
after selection. At a step boundary cancellation precedes count, score and time
criteria. After selection, already-observed evaluation exhaustion wins; otherwise
cancellation precedes time expiry and no-accepted-move termination. A request that
closes enumeration before another candidate is emitted can prevent evaluation
exhaustion from being observed, even when the last evaluation spent the budget.

This is cooperative phase cancellation. Active callbacks, validation, optional
score diagnostics and bounded candidate prefetch are not interrupted; diagnostics
may execute after a request. There is no bounded cancellation latency or optimality
claim. Construction and exact search also accept this shared token as described
below. Whole-solve orchestration, daemon operation and queued problem changes
remain pending.

Six original integration methods exercise all six paths, repeated requests and
shared identity, a real thread cancelling a blocked score callback, initial and
candidate prefix retention, both selection policies, validation and rejected-move
fallback. These checks earn no upstream method credit; the complete Java solver
lifecycle methods remain pending.


### Construction cancellation

`ConstructionConfig<'a>` now optionally borrows the same token through
`cancellation`; `MixedConstructionConfig<'a>` carries that lifetime too. Existing
full struct literals must supply `cancellation: None` or use the default update
syntax. Helpers returning token-free configurations can name a `'static` lifetime.
All six basic/list/mixed and scoped/stream construction paths share these controls.

Model/network and stream-source validation run first. Impossibility returns
NoCompleteAssignment before cancellation and before zero-budget handling. A zero
budget returns EvaluationLimit; otherwise a request observed before initialization
returns the initial assignment, no score and zero work. Completeness describes the
assignment independently of its score or termination reason.

Requests during scoring let that calculation finish. Before each required item and
between emitted candidates the constructor polls the token. It installs the first
highest-scoring candidate from the evaluated prefix using its saved cache, then
returns that partial or complete assignment with its calculated score and normal
work counters. Initial and post-scan diagnostics are not preempted and remain
excluded from the work budget. An already-observed evaluation limit takes
precedence, including when diagnostics request cancellation afterward. Otherwise a
final poll reports Cancelled even when the last candidate completed construction,
or when there were no missing required values. This differs from exact-search
exhaustion, which proves the complete assignment space was visited.

Five original methods verify cancelled prefixes against the corresponding bounded
construction results for both early-pick modes, both mixed construction orders,
all six backends and every budget/cancellation boundary of the small models.
Returned assignments and scores are independently checked, with explicit partial
progress and cross-thread cancellation checks. Additional cases cover diagnostic
requests, reused tokens, validation, impossible models and already-complete inputs.
These are native lifecycle contracts; no extra upstream method credit is claimed.


## Separate candidate-evaluation budgets

ConstructionConfig and LocalSearchConfig now accept optional
`max_move_evaluations`; mixed configurations inherit it. It is disabled by default.
It counts evaluated candidates, including rejected trials, independent of accepted
moves and completed steps. Initial scoring and diagnostic recomputations do not
consume this budget. The existing `max_evaluations` still includes initial scoring.
All six result types expose `move_evaluations()` separately from total evaluations
and terminal/scoped constraint callback counts. One candidate may invoke multiple
callbacks or none, so those counters are not interchangeable.

The six construction and six local-search paths bound each selection by the tighter
remaining score or move budget. Optional candidate shuffling uses that same bound
for its lookahead window. When a further candidate reveals exhaustion, selection
installs its cached best evaluated prefix and reports MoveEvaluationLimit for a
strictly tighter move cap, or EvaluationLimit for a score cap or equal caps. The
reason is captured before selection so post-selection counters cannot erase which
budget constrained it. Arithmetic stays within u64 without adding one to a maximum
move limit.

A zero move budget permits the initial score, then no candidate scoring. A zero
score budget still skips initialization. No candidate means no budget-exhaustion
observation: already-complete construction can report Completed, and an empty local
neighborhood can report NoAcceptedMove. Consuming the last permitted evaluation
alone does not report exhaustion until a further candidate is attempted. Existing
step/score/feasibility boundary rules can stop first. Cancellation and elapsed-time
polls may close enumeration before the cap is observed; an already-observed cap
wins over later cancellation or time polls, including requests during diagnostics.
The configured budgets apply to each phase independently, not to an orchestrated
multi-phase solve.

CountLimit exposes exact threshold decisions, saturating remaining counts and
capped floating progress. It supports the entire u64 range; TryFrom<i64> rejects
negative configuration input. Progress may round to 1 near large limits, so it
must not replace the exact stopping predicate. All five MoveCountTerminationTest
and ScoreCalculationCountTerminationTest methods have complete adaptations of their
count, gradient and negative-input assertions. Explicit native counts replace the
mocked Java scopes/directors. These tests do not establish Java counter-accounting
or whole-solver lifecycle equivalence.

Five original integration methods and one controlled-clock unit method additionally
cover every small score/move budget pairing across all backends, first/best selection,
strict/hill acceptance, rejected fallback, shuffled lookahead, both mixed construction
orders, early picking, diagnostics, cancellation, time precedence and u64 boundaries.
Returned construction assignments and scores are independently checked; bounded
runs retain the same evaluated prefixes as the corresponding score-only caps.


## Construction elapsed time

ConstructionConfig::max_time optionally bounds all six construction paths. The
clock starts on entering the shared algorithm, before initialization and diagnostic
scoring. Model/network/source preparation in the outer entry point is outside this
phase-local clock. Native multi-phase solves additionally charge preparation to
their shared timer; see `solver-contract.md` for the cooperative handoff boundary.
Disabled construction timing neither starts nor samples a clock.

Polls occur before each item, between candidate emissions, and after the final scan.
Expiry retains the best evaluated prefix of the current scan and reports TimeLimit,
even if that prefix completed the model. Zero time allows initial scoring and no
candidates. Optional-only and empty models still perform the final poll. Initial
scoring and diagnostic callbacks count as time, while diagnostics remain excluded
from evaluation counters. Active work cannot be preempted. Validation, impossible
models, zero score budgets and pre-cancellation retain their existing order.
An already observed score/candidate cap wins over a later poll; if time stops the
iterator before selection sees that cap, time wins. Cancellation precedes time when
both are observed at the same boundary.

Six original unit methods check actual scoped and stream algorithms with controlled
clocks, both mixed construction orders, early picks, independent assignment/score
validation, partial and complete prefixes, diagnostics, competing limits and empty
or optional-only models. These regressions add no adapted upstream method credit.


## Step-counting hill climbing

Acceptance::StepCountingHillClimbing { period, mode } starts a fresh threshold
session for each local-search phase. StepCountingHillClimbing exposes the same
policy independently. The initial threshold is the starting score. A trial is
accepted when its score is at least the current score OR at least the threshold.
This permits deterioration within the threshold while still allowing recovery after
a selected rejected fallback. A threshold is independent of the retained global best.

Five StepCountingMode values define the reset counter:

- Step counts every completed step, including a selected rejected fallback.
- EqualOrImprovingStep counts steps scoring at least the previous completed step.
- ImprovingStep counts strict improvements over the previous step, including
  recovery that remains below an earlier global best.
- SelectedMove counts every scored candidate passed to the acceptor, even rejected
  or unselected trials. It does not count unscored neighborhood prefetch.
- AcceptedMove counts every trial the policy accepts, including unselected trials.

The threshold stays fixed throughout a scan. At a completed step reaching the
period, it becomes that step's score and the counter resets to zero. Excess trial
counts do not carry to the next interval. Native count() is capped at the period,
which avoids overflow even at u64::MAX. No history buffer is allocated. Trial
modes count repeated accepts calls; other modes do not mutate their counter during
trial checks. Diagnostic rescoring bypasses acceptance and cannot advance a counter.

All six local-search model/backend paths and ordered solver plans share this policy.
FirstAccepted may count one accepted trial per step; BestAccepted can count several.
Zero periods fail validation before scoring or factory calls, including skipped
future phases. Native periods are u64, so negative signed configuration is not an
exposed input. This does not earn credit for the upstream negative-size test.
Owned decimal, fixed/bendable and multi-level scores use their native ordering.

The pinned Community StepCountingHillClimbingAcceptorTest supplies four full adapted
methods: all 109 acceptance assertions in its three step-mode sequences, their six
completed scores per sequence, and zero-size rejection. Provenance and notices record
the adaptations. The two trial-counting modes additionally have original regression
coverage and are checked through public Java configuration APIs. Algorithm background:
[Timefold step-counting hill climbing](https://docs.timefold.ai/timefold-solver/latest/optimization-algorithms/local-search#step-counting-hill-climbing).

scripts/step_counting_checks.py compares 600 original cases (seed 20261218): all five
modes, periods 1/2/7 and first/best accepted selection on required-variable graph
problems, including plateaus, fallback/recovery, pins and zero steps. Java and both
Rust backends agree on final assignments and scores. Independent Python enumeration
also checks native completed/accepted/unimproved counts and stop reasons, over 6,180
reference steps. Java internal traces and list/mixed differential equivalence are
not asserted. These generated checks add no source-method credit.

## Great Deluge

`GreatDeluge<S>` and `Acceptance::GreatDeluge` accept a candidate when it meets
or exceeds the water level, or strictly improves the previous completed score.
An equal score below water is rejected. Water starts at the phase initial score
unless `initial_water_level` supplies an explicit threshold. Trial evaluation,
best-score changes and rejected candidates leave water unchanged. Every completed
step raises water, including a selected best-rejected fallback. Each phase starts
a fresh session; all six basic/list/mixed scoped/stream engines use the shared
acceptor. Configuration is validated before solver work, even for skipped phases.

`GreatDelugeRise::Score` adds nonnegative ordinary levels after each step; zero
rain is permitted. Initial water, score increments and phase initial scores must
have structural zero. `GreatDelugeRise::Ratio` requires positive u64 numerator
and denominator. It adds that fraction of the absolute initial water components
per step. Fractional rain accumulates exactly, then the total is rounded down at
the initial component's decimal scale (at least zero). The initial water is not
replaced by improved scores. Zero initial components stay zero with ratio rain.
Integer thresholds use arbitrary precision and cannot wrap into acceptance.
Native Decimal thresholds retain their precision, with the same potentially
unbounded storage needs as existing exact decimal arithmetic.

The optional `ScoreLevel::exact_decimal` capability supplies numeric levels and
precision. It must return Some for every value or None for the entire level type;
i64 and Decimal implement it. Other custom levels retain their existing search
policies but reject Great Deluge with UnsupportedScoreArithmetic. Score's existing
constant dimension/order requirement applies. Standalone sessions bind their
score type and expose numeric thresholds through `water_levels()`.

All three pinned GreatDelugeAcceptorTest methods are adapted, preserving 36
acceptance assertions. Original boundary/integration regressions and the public
Community API comparison add no method credit. The original 276-case matrix
covers fixed rain (0,1)/(1,0)/(1,3), ratios 1/10, 1/2 and 1/100, first/best accepted
selection and rejected fallback. Rust scoped/stream results and counters match
independent exact Python enumeration in all 276 cases. Java final scores and
assignments agree in 275. The remaining ratio case is a recorded divergence:
Rust reaches (0,9) at step 11; Java reaches it at step 13. A frozen 25-prefix probe
checks this boundary (23 agreements, two divergences). A separate Python model
using cumulative binary-floating ratios reproduces those final outputs; this is
an explanatory hypothesis, not an inspection of Java's thresholds or traces.
Known divergences never count as agreement passes; changed or unexplained outputs
fail the comparison command. No general Java floating/overflow or decimal-search
parity is claimed. See `scripts/great_deluge_checks.py` and report details.

The public configuration and default initial-water concept are described in
[Timefold's local-search documentation](https://docs.timefold.ai/timefold-solver/latest/optimization-algorithms/local-search).
The original fixed-rain performance workload uses (0hard/1soft) and 50 steps;
it does not measure ratio rain, incremental Great Deluge scoring or decimal scores.

## Construction assignment-step limits

`ConstructionConfig::max_steps: Option<u64>` bounds evaluated scans of missing basic
variables or list values. None (the default) leaves existing behavior unchanged.
One step finishes a variable's domain scan or a list value's insertion scan,
including an early pick; scanning several candidates still consumes one step.
Construction result completed_steps counts scans with at least one candidate; the
assignment counters separately count installed placements. Mixed construction
uses a single limit across both components, in its configured component order.
Each call or phase starts a fresh count, including resumptions.

The limit is tested before scanning the next eligible missing item. Initial scoring,
diagnostics, preserved assignments, pins and scans with no candidates consume no
steps. Optional unassignment is skipped by the default Preserve policy. A zero limit permits initialization, then no assignment. If no eligible missing
item remains, completion wins, including exactly reaching the limit on the final
scan. Optional-only and empty models still complete with one
initial score. The optional NonWorsening policy also scans unassigned optional values as described
below; their rejected scans count toward the same step budget.

`ConstructionTermination::StepLimit` means a further eligible item was stopped.
It does not truncate the preceding scan. An evaluation/candidate cap can truncate
that scan, retaining its best prefix and its own stop reason; cancellation and time
also retain their existing cooperative polling behavior. Validation, impossibility,
zero score budget and pre-cancellation keep their earlier precedence. At an item
boundary cancellation and time are checked before the step limit, while the step
limit is checked before candidate/score budget exhaustion. A completed-step cap of
one therefore evaluates the whole first scan if no other limit stops it.

Ordered solves preserve the phase stop reason and can resume construction or enter
exact search. An incomplete handoff to local search still returns the native
IncompleteConstruction termination; its initialization event precedes this check
and records early termination. SolverWork.completed_steps/accepted_moves and shared
stagnation remain local-search statistics; construction assignment steps are excluded.

Seven original integration methods cover all six model/backend paths, both mixed
orders, early picks, diagnostic scoring, every small score/candidate/step budget
combination, cancellation, prior assignments/pins/optional values, validation,
impossibility, resumptions and phase/event handoffs. A controlled-clock unit method
checks elapsed-time precedence. A public doctest demonstrates a two-call resume.
Those original step-limit cases add no upstream-method credit. Optional-construction
adaptations are recorded separately below; the early required-CH exception path
remains pending.


## Score-aware optional construction

ConstructionConfig::optional defaults to OptionalConstruction::Preserve.
OptionalConstruction::NonWorsening considers missing optional variables and list
values using the same domain/insertion order as required construction, across all
six model/backend paths. Supplied assignments, full/prefix list pins and optional
basic variables pinned to None are preserved. Each optional candidate must score
at least as well as the cached pre-scan state. The highest accepted score wins;
ties keep the first candidate, including a tie with the initial unassignment.
If all candidates are worse or no placement is legal, the item stays unassigned.
No extra synthetic null candidate or initial rescoring is charged.

The initial policy decision uses full score ordering; structural state is unchanged
by optional placement. Required variables retain their existing fill behavior even
if their ordinary score worsens. Early-pick policies apply only to candidates that
meet the optional non-worsening condition. Normal evaluation/candidate limits can
truncate the scan, committing only an eligible winner from the evaluated prefix.
Time, cancellation, diagnostics and transactional rollback retain existing rules.

All three construction result types expose completed_steps. It counts scans with
at least one evaluated candidate, including all-rejected optional scans and an
interrupted scan. Empty candidate scans count zero. A step cap is checked before
candidate enumeration, so a zero cap stops before discovering whether the next
missing item has any candidates. Assigned-variable/value counters increase only
on committed placements. Mixed models share one completed-step count across both
components. A resumed call starts scanning the remaining missing items again,
including previously rejected optional items; it is not a persisted scan cursor.
Standalone observers emit initial state and committed placements, with no duplicate
notification for a rejected optional scan. Ordered solver best-event filtering and
local-search-only work/stagnation counters remain unchanged.

Optional assignments are already complete in the native model. Completed, StepLimit
and complete=true can therefore coexist with unassigned values, and an early
initialization event does not imply an incomplete assignment. Cross-phase equal
scores still preserve the earlier retained assignment under the solver contract.

Three complete pinned methods are now adapted: DefaultConstructionHeuristicPhaseTest
basic and list optional construction cases, and SolverManagerTest's early-terminated
optional-list construction initialization event. They explicitly select NonWorsening;
no Java default-policy, evaluation-count or internal-trace parity is claimed.
Fixture provenance covers both optional model families and their distinct scoring
rules. Four original integration methods independently cover all six paths, mixed
orders, positive/zero/negative optional objectives, budgets, cancellation, pins,
resumption, input validation and unavailable insertions. The required early-CH
exception and custom-phase initialization cases remain pending.

## Whole-pillar neighborhoods

`Neighborhood::Pillar { swaps }` selects whole equal-value groups of unpinned basic
slots, including an unassigned group. A group may change only to a value accepted by
every member's domain; None requires every member to allow unassignment. Unordered
swaps require both cross-values to satisfy all target domains. No-op changes are
omitted. Pins never join a movable group. Singletons are valid groups.

Groups follow the first member in the configured variable order. Changes follow
that member's domain order, with optional None last, then swaps follow group-pair
order. ShuffledVariables changes that encounter order; ShuffledMoves uses the same
existing bounded shuffle and seeded generator. Group membership costs O(n) storage
and worst-case O(n²) equality comparisons. Candidate values are cloned lazily for
only the current move; this does not imply constant-time group preparation or
interruptible user equality/cloning operations. Shared boxed iterator dispatch also
serves the existing basic neighborhoods.

Both scoped and stream basic search use this provider. Mixed search uses it for the
basic component and the ordinary list changes/optional swaps for the list component;
joint mode combines both components. List-only searches likewise retain ordinary
changes and the selected swap setting. Existing acceptance, first/best selection,
limits and rollback apply to the resulting composite moves. Default neighborhoods
are unchanged. This provider does not enumerate subpillars or infer entity schemas;
use explicit multicolumn Pillar rows to construct such selected-variable group moves.

Six original integration methods cover a group local optimum and a mixed joint
trap in both scoring backends, seeded orders and evaluation caps, a small independent
successor oracle over optional/ranged/pinned states, malformed/nonuniform groups,
atomic score rollback/rejection and lazy candidate value cloning. Six complete
pinned source methods add swap/undo/range/raw-execution evidence. Source unchecked
execution intentionally includes a no-op and an out-of-range fixture; it grants no
unchecked-operation safety or Java metamodel parity claim.

## Subpillar neighborhoods

`Neighborhood::SubPillar(config)` uses bounded subsets of the existing unpinned
single-column equal-value groups. `SubPillarConfig::new(selection)` enables all
change targets and swaps with the same selection on each side. `changes` accepts
Disabled, AssignedOnly, All or UnassignOnly; optional None targets remain subject
to every selected member's domain policy. `swap_selection: None` disables swaps,
otherwise its independent bounds/order apply to the second group. A required
member does not globally reject optional mode; it simply cannot receive None.

Group order follows first encounter in the configured variable order. Within each
group, subsets follow increasing size then member positions (combinations or
consecutive windows). Targets follow the first selected member's domain, with None
last where enabled. Range checks apply only to selected members, so a value absent
from an unselected full-group member's range remains eligible. Swap pairs follow
changes. Equal selection policies visit unordered full-group pairs; different
policies visit both orientations so asymmetric bounds cover both assignments of
roles. Selected groups never overlap. Equivalent successors can still occur when
bounds overlap across asymmetric roles; no global deduplication is promised.

Both basic scoring backends use this provider. Mixed search uses it for basic moves
and joint products; list components retain ordinary changes and enable swaps when
swap_selection is Some. The basic change mode does not restrict list operations.
ShuffledVariables changes group/member encounter order, including consecutive
windows; ShuffledMoves retains the existing bounded seeded shuffle. Direct public
subpillar_moves uses increasing variable indices. Explicit Pillar::subpillars uses
canonical row order instead. No Java random-sampler trace/distribution or comparator
identity compatibility is claimed. No variable schema is inferred.

Subsets and pair products are generated lazily; only emitted move values are cloned.
Grouping still takes O(n) storage and worst-case O(n²) comparisons. A current subset
cursor stores O(k) indices, and a swap keeps current left/right selections without
materializing their Cartesian product. An unrestricted combination policy can expose
an exponential number of candidates; search limits cap scored candidates but do not
preempt grouping, filtering rejected subsets or user value operations. Use bounds
appropriate to the model; no hard latency guarantee follows from a candidate cap.

Six original methods verify subset order against independent bitmasks, boundaries,
multicolumn rows, huge lazy combination spaces, complete successor sets under domains/
pins/optional values/asymmetric swaps, both basic backends escaping a partial-group
trap under seeds/budgets, subset-local domains and early rejection of undersized
swap partners before expanding large combination spaces. The existing mixed joint trap now
also exercises subpillar neighborhoods. Thirteen complete source methods preserve
provider membership, diversity, pinning, null-target and independent-size assertions
through finite enumeration; source random draw sequences remain outside that evidence.

## Schema-aware pillar search

An optional PillarSchema on FiniteProblem makes both Pillar and SubPillar neighborhoods
operate on aligned selected entity rows. No new search entry point is required;
scoped/stream basic and mixed search obtain the same metadata from their basic model.
Grouping and selected row values are rebuilt from each search snapshot, so earlier
moves can split or merge groups. Without a schema the earlier single-slot provider
ordering and behavior are unchanged.

Rows follow the first selected slot encountered in the configured variable order.
Within groups, subset selection follows that row order; consecutive selection is a
window over member rows. Changes iterate groups, subsets, columns, then the first
selected member's domain, optionally followed by None. They change one column at a
time, rather than enumerating a Cartesian product of column targets. Swaps iterate
both directed full-group pairs, then each side's selected subsets, and write all
selected columns using original values. This schema path preserves both orientations
even for equal side policies; budget/tie outcomes can therefore differ from the
single-slot unordered provider. Selected pins exclude entire rows. Unselected slots
remain available to other neighborhood types and construction/exact phases.

Grouping retains O(variable count + row count) indices and may compare O(rows² ×
columns) values. Subsets, pair products and move value cloning remain lazy. Both
partner sizes are checked before expanding combinations. The schema itself is shared
through Arc on model clones. No bound guarantees prompt interruption of grouping or
long rejected-subset scans. Existing evaluation/seed/acceptance/cancellation rules
apply when candidates reach scoring.

Six original integration methods cover schema shape/bounds/clones/removal/selected
pin rules, exhaustive binary assignment/mask successor sets with excluded columns,
restricted domains and pins, per-column changes/optional targets, both basic scoring
backends escaping a tuple-group trap under seeded orders with score rollback, both
mixed backends requiring a joint tuple/list move, lazy value cloning and early
undersized-pair rejection. Eleven source methods preserve whole/subpillar swap
membership, directed orientations, range and selected-column key behavior.


## Fading variable tabu

`Acceptance::FadingVariableTabu { hard_tenure, fading_tenure, aspiration, seed }`
extends recent-variable rejection with probabilistic expiry. A selected key starts
at age 1. Through hard_tenure completed steps its acceptance probability is zero;
subsequent age hard_tenure + k gives (k + 1)/(fading_tenure + 1), reaching one at
the combined horizon. Zero fading reproduces hard tabu; zero hard tenure permits
immediate fading. The combined tenure must be positive and fit usize.

The youngest of all touched retained keys determines a candidate's probability.
Duplicates refresh one key, empty completed steps age keys, and trials do not age
memory. Aspiration requires a strict global-best improvement. Certain decisions
consume no randomness; intermediate probabilities consume one native seeded draw.
`FadingTabuMemory<K>` additionally exposes pure probability inspection and lazy
supplied-draw replay with finite [0,1) validation. Selection stops without aging
if no move is selected, unless the configured rejected-move fallback selects one.

All six local-search paths use this policy. Basic moves track written variables;
list moves track touched entities; mixed moves use separate index namespaces.
Independent reference histories check multiplicity, refresh, expiry and youngest-key
probabilities. Search regressions cover actual tabu stops and mixed key separation,
plus existing backend-comparison matrices for pins, budgets, selection, fallback,
shuffled ordering and joint moves. Owned and decimal scores also pass those phases.

The complete Community EntityTabuAcceptorTest.fadingTabuSize adaptation preserves
sizes 2/4, all step observations, exact supplied draws and no-draw assertions.
This source method covers one queried key at a time. Multi-key fading, zero-hard
configuration and the native PRNG are independent policies, not verified Java
trace equivalence. No value/move-tabu search, ratio sizing or new timing claim.


## Medium-level late-history correction

The earlier hard-only policy incorrectly kept old thresholds when a HardMediumSoft
best improved only its medium level. The pinned DefaultLevelScoreStateTest requires
that improvement to reset history. Score::late_acceptance_cmp now separates reset
levels from hard-level feasibility, with a backward-compatible default for custom
scores. Both fixed HardMediumSoft families override it using exact lexicographic
hard/medium comparison. This changes accepted paths under late acceptance; other
acceptors and score feasibility are unchanged.

Six source methods preserve their complete fixtures and reset/no-reset assertions
for HardSoft, HardMediumSoft and Bendable(1,2). Java mock indices map to explicit
initial/new-best comparisons; three lifecycle/cache-index methods remain pending.
Original regressions first failed on stale slots and a 3-step native search where
reset thresholds stop after one step. They now check full history, wraparound,
global-best improvements, worsening steps, ties, exact tiny decimal differences,
bendable soft levels, structural exclusion and all six search paths. No new Java
search-trace or timing claim is made.

## Combining acceptance policies

`LocalSearchConfig::additional_acceptance` borrows an arbitrary-length slice of
policies. Each candidate must satisfy the primary `acceptance`, then every extra
policy in order. Trials stop at the first rejection; later children consume no
random draws or trial counters. A completed step updates every child, including
those skipped or rejecting that candidate. This also applies when the configured
best-rejected fallback selects a move. Aspiration only overrides its own child's
rejection. All policies validate before scoring, graph factories or events, even
in later phases that would otherwise be skipped. Each phase starts fresh state.
An empty extra slice preserves the existing single-policy execution path.

Standalone `Acceptor::from_policies` creates an ordered conjunction; an empty
conjunction accepts everything. `Acceptor::All` permits nested state composition.
These ordering, empty-identity and state-update contracts are native decisions.
The adapted CompositeAcceptorTest.isAccepted preserves all five source truth cases;
the mocked phaseLifecycle method remains pending. No Java callback ordering,
random sequence or lifecycle equivalence is claimed.

All six basic/list/mixed scoring paths track touched keys when any child uses tabu.
Original checks exercise short-circuit counters/randomness, rejected fallback state,
validation precedence, fresh sessions, list entities and disjoint mixed namespaces.
Backend matrices cover combined policies alongside pins, selection, budgets, fallback,
shuffled order and joint moves. Owned non-Copy scores retain the same outcomes.

Borrowed extra policies must outlive a scoped solve, reflected by `S: 'c` where
configuration uses lifetime `'c`. Existing factories store static configurations;
their policy slices and score types therefore require static lifetimes. Local
runtime slices work with scoped entry points, not the static factory. Configuration
remains Copy when its score type is Copy; no Java API/ABI parity is intended.

The combined late-acceptance/tabu benchmark uses history size 3 (native hill-climbing
shortcut enabled), tabu tenure 2 and global-best aspiration. Both engines use full
score calculation, deterministic change moves and the same fixed step budget.
All assignments are independently rescored, scores/score-call counts accompany
samples, and quality equality is reported separately. This establishes bounded
workload validity and measurements, not identical internal traces or general speed.

## Move-based tabu

`Acceptance::MoveTabu` and `FadingMoveTabu` retain complete selected descriptors
instead of touched-variable keys. Public Community documentation defines move tabu
as rejecting a move equal to a recent selected step. Native equality is the existing
structural Eq of Move, ListMove and MixedMove: values, positions, swap orientation
and ordered composite children remain significant. Equality must stay stable while
retained. There is no hash, pointer identity, debug-string encoding, automatic
inverse retention, or equivalence by successor state. Java descriptor equality is
not claimed, particularly for reversed swaps or entity-based list representations.

`Acceptance::start_with_moves` produces typed `MoveAcceptor<S, M>`; its accepts and
step_ended calls require the actual move alongside variable/entity keys. Scalar
`Acceptance::start` returns MoveContextRequired for move policies. This avoids
silently interpreting variable keys as move identities. Typed from_policies retains
ordered conjunction/short-circuit behavior, independent variable/move memories and
completed-step updates to all children, including rejected fallback. Ordinary
policies use existing Acceptor state, without cloning ignored descriptors. Trials
never retain a move; only selected steps clone descriptors into move memory.

Both policies work in all six solver paths, as primary or additional policies.
Tenures, fading probabilities, deterministic native draws and strict global-best
aspiration follow the already documented native tabu contracts. Zero fading gives
hard tabu; hard+fading must be positive and fit usize. Each phase creates fresh
memory. Configuration validation precedes scoring and state construction.

All three MoveTabuAcceptorTest methods are adapted with every acceptance and supplied
random-consumption assertion. Integer IDs replace source mocks; source fading draws
are supplied explicitly, not reproduced by the native PRNG. Original regressions
cover structural distinctions, chronological expiry/refresh/best-score references,
trial clone avoidance, validation, skipped random draws and all six search paths
against independently retained selected histories. Existing paired-backend and
owned-score matrices include hard and fading move policies, combined with other
acceptors, budgets, pins, fallbacks, shuffled order and phase transitions.

The public Java/native full-scoring benchmark adds move tabu size 2 with global-best
aspiration, original-order change descriptors, fixed steps and best-rejected
fallback. Every sample is independently rescored and final quality is shown with
timings. This bounded change-only comparison does not establish general Java move
equality, fading random-trace parity, memory scaling or production performance.

## Value tabu and planning-value capture

`Acceptance::ValueTabu` and `FadingValueTabu` retain the planning values of selected
steps. Native `ValueAcceptor<S,M,K>` receives explicit variable/entity, move and value
identities; children retain independent memories. `start_with_values` supports every
policy. Scalar and move-only constructors report ValueContextRequired instead of
silently ignoring missing value context. Ordered conjunction, skipped trial draws,
completed fallback updates, fresh phases and strict global-best aspiration follow
the existing acceptance contracts. Zero fading reproduces hard value tabu.

The public Community Move API describes values assigned by a move and permits null.
Native `Assignment::planning_values` captures each basic write target, including
None, swaps at their execution point and intermediate composite writes. Values are
deduplicated in first-write order. Raw assignment helpers validate indices; model
pins/ranges remain the FiniteProblem responsibility. The input is never modified.

`ListProblem::planning_values` captures assigned, relocated, exchanged, reversed
span and removed items in first-touch order. Neighbors shifted by insertion/removal
do not become keys merely because their indices change. Each composite child sees
the preceding children's state; full domain/pin validation happens at the same final
boundary as ordinary apply. Capturing reuses native move execution with an optional
recording callback, keeping one implementation of composite indexing and validation.
None is only a basic unassigned key; list removal tracks the actual removed values.

Mixed capture validates the full move, then wraps basic Option<B> and list L keys
in distinct MixedPlanningValue variants. Equal primitive representations in the two
components never collide. Equality must remain stable while values are retained;
no hashing or mutable Java identity equivalence is implied. Duplicate values refresh
one key and an empty selected-value list still ages existing tabu memory.

All six solver paths derive trial and selected-step metadata from the same pre-step
snapshot, only when a configured child needs value keys. Basic capture uses direct
change/swap paths or a sparse borrowed-reference overlay for composites. It clones
only distinct returned values, after index validation. List capture still replays a
private snapshot; mixed capture still validates its full transaction before extracting
component keys. All capture costs are included in native value-tabu timing. Capture
does not invoke scoring or observers. Ordinary policies retain no value keys.

All five ValueTabuAcceptorTest methods preserve every source acceptance assertion,
repeated query, multi-key selection, null expiry and supplied fading draw. Source
mock step scores become explicit observations of the source global-best baseline.
Original tests cover native extraction, chronological multi-key/null references,
context/config errors, skipped randomness, combined independent memories and all
six search paths against independently extracted primitive values and histories.
Paired-backend and owned-score matrices include hard and fading value policies.

Public Java/native benchmarks add value tabu tenure 2 with global-best aspiration,
complete change-only assignments, fixed steps and best-rejected fallback. Every
sample has independent Python validity/rescoring and retained quality/score-call
counts. Native capture cost is timed. This does not establish Java metadata
parity for arbitrary composites/lists, fading PRNG traces or general performance.

### Diversified late acceptance

`Acceptance::DiversifiedLateAcceptance { history_size }` is available through
all basic, list and mixed scoring/search paths and ordered policy conjunctions.
A fresh session fills history with the phase's initial score. A trial is accepted
when equal to the current score or strictly above the worst history score.
Accepted trials replace the current slot when below that slot, or when above
both that slot and the current score. Rejected trials do not change history.
Completed steps rotate the slot, including selected rejected fallbacks; the
completed score is not an unconditional history replacement. There is no
hard/medium-level reset. Worst score and multiplicity are recomputed after each
replacement, taking linear time in history size. Scores retain full native ordering.

This candidate-evaluation history policy follows the observable pinned tests;
it is distinct from interpreting each entire multi-candidate scan as one proposal
in the [published DLAS algorithm](https://arxiv.org/pdf/1806.09328). A queried child
retains its history updates if a later policy rejects. A child skipped by an
earlier rejection does not update; all children rotate on completed steps.
The independent rotating-history regression checks these native rules. Basic
network/stream searches additionally match independently enumerated selection
for first/best acceptance, rejected fallback and three history sizes. Existing
owned-score matrices exercise every backend with decimal and extreme scores;
list network/stream comparisons include the new policy.

Both pinned DiversifiedLateAcceptanceAcceptorTest methods have full adaptations
as private-state unit tests, retaining all injected history, cached worst/count
fixtures and assertions. The provenance identifies the Cargo library target and
qualified names. Coverage parsing credits only the timetuck library executable,
keeps ignored cases unverified and excludes unrelated libraries/examples.

The full-scoring heuristic benchmark adds public Community preview configuration
with history size three on the same complete change-move workloads. Final validity,
quality, raw scores and score-call counts are measured separately from timings.
These comparisons do not establish identical Java candidate acceptance or traces.

### Population-relative tabu sizing

`TabuSize::fixed(i64)` validates a positive fixed tenure; `TabuSize::ratio(f64)`
validates a finite ratio in `[0,1]`. Ratio resolution rounds the binary64 product
to the nearest integer (positive halves upward), clamps to at least one and at
most population minus one, and returns one for empty/singleton populations.
Signed zero is canonicalized. The final upper cap is exact even for populations
outside binary64's exact integer range. Fixed sizes do not depend on population.
The zero/one-population and ratio endpoint rules are native extensions verified
by original regressions; the pinned ratio method specifies six cases at count 100.

`Acceptance::SizedVariableTabu { size, aspiration }` resolves at phase start.
Basic models count variables, list models count list-owning entities, and mixed
models add both namespaces. All counts include pinned entries. Primary and
additional policies both receive that population. Each new search resolves
again from its own model. Resulting memory retains the existing completed-step
expiry and strict global-best aspiration semantics; the size is not a maximum
number of retained keys. There is no automatic population-relative value/move
or fading policy in this increment. `TabuSize::resolve` can explicitly size a
fixed native policy when the caller supplies the appropriate population.

Standalone acceptors require `policy.for_population(count)` before starting a
sized policy, so a missing population cannot silently use an arbitrary tenure.
Validated private sizing representation preserves Copy/Eq configuration without
permitting NaN or invalid raw ratios. Oversized fixed integers reject conversion
instead of wrapping on narrower platforms.

All three methods in the pinned EntityRatioTabuSizeStrategyTest and
FixedTabuSizeStrategyTest are fully adapted with provenance. Original checks
compare exact binary fractions with integer rounding; verify validation, huge
counts, new models and independent memory; and compare all six backend paths
with explicitly sized controls, including pinned entries and additional policies.
Existing owned-score and list-stream matrices also include the new policy.

The matched basic-change benchmark adds public Java `withEntityTabuRatio(0.2)`.
Native basic counts correspond to one Java entity per variable, including pins.
The 32/96-variable workloads resolve to 6/19 completed-step tenures. Timing,
returned quality, validity and small-case optima remain separate evidence;
no general Java list/mixed population or dynamic problem-change claim is made.


### Move-value metadata assertions and sparse basic capture

Six additional pinned methods assert planning values for singleton mass changes,
pillar swaps, list changes, sublist relocation, sublist swaps and sublist unassignment.
The mass-change adaptation queries metadata before execution, as does its source.
The other adaptations capture immutable metadata from the input and retain it across
actual execution, replacing the public Java getter's post-execution observation.
Only the pillar-swap source assertion permits unordered membership. All six fixtures
and assertions have provenance; the generated Java list comparison remains separate
and retains its known differences, errors and unsupported empty compositions.

Basic capture now borrows values from the original assignment or the move tree.
Composite overrides contain only written indices and are read in child execution
order, so swaps see prior writes without copying the full input. Unique output
identities retain the payload of their first write even when later equal identities
carry different payloads. Change and swap fast paths need no overlay. Invalid
queries clone no output values. Clone/equality panic regressions confirm that the
input remains intact and reusable.

Four original regressions include 1,500 generated nested cases checked against a
separate recursive full-array reference, with explicit valid/invalid and change/swap
coverage guards. A 10,000-variable clone counter proves one clone for a change,
two for a distinct swap and only one per distinct non-null composite output.
These are clone-count bounds, not a general speed claim. Existing value-tabu
search, list/mixed capture and transaction tests remain part of the full gate.

## Per-step foraging and random ties

MoveForager retains typed candidate payloads under accepted-first, then highest-score
priority. Rejected fallback candidates enter only when enabled and structurally
sound. First-tie ordering retains one candidate. Random ties retain every equal
finalist in encounter order and choose one with a bounded native draw; better
candidates clear the previous finalist set. Saved candidates need not implement
Clone. Full and stream scoring backends retain candidate assignments and cache or
graph snapshots, then install the selected state without callback replay.

LocalSearchConfig adds accepted_candidate_limit: Option<NonZeroU64> and
TieBreaking::First / Random { seed }. The default remains First, with no per-step
accepted-candidate limit. max_accepted_moves remains the separate phase budget.
Every accepted candidate counts, including inferior scores and ties; rejected
candidates never consume this limit. Each neighborhood scan starts at zero.
FirstAccepted stops immediately; BestAccepted scans the available prefix;
FirstBestScoreImproving and FirstLastStepScoreImproving stop at the first accepted
score strictly above the retained phase best or the current step score, respectively.
Any configured accepted-candidate limit may stop these scans sooner.

Random tie selection uses SplitMix64 with seed + completed_steps (wrapping),
independent of move-order and acceptance randomness. Each local-search phase starts
its own step count. Standalone foragers use their supplied seed directly. The
pick_with method instead accepts a zero-based finalist index, checked before any
candidate is returned. First, empty and singleton pools do not request a draw.
No Java PRNG trace, mutable scope identity or lifecycle equivalence is claimed.
Random ties can require storage proportional to the best tied candidates in the
evaluated prefix; default first-tie storage remains constant in candidate count.

Construction retains its existing custom early-pick callbacks and first-tie policy.
All six native local-search paths use the shared forager, including solution
observation wrappers. An early selection stop does not probe another candidate to
discover evaluation exhaustion; existing phase/step budget precedence is retained.
Failed candidate capture or a later scoring/acceptance error cannot install an
unfinished selection. Ordinary unwinding restores the base assignment and caches.

All seven AcceptedLocalSearchForagerTest methods preserve their complete candidate
fixtures, intermediate quit assertions and selected identities. Native typed payloads,
score references, canonical structural flaws and supplied index 1 replace Java
mocks/scopes and TestRandom; provenance and notices record these adaptations.
Original checks cover 256 generated histories across 32 policy/limit/fallback
configurations, every finalist choice, native seeded reproducibility, invalid
indices, lazy capture and save failure, exact early/budget boundaries, and 240
configurations across all six backends. Additional owned-score checks compare all
six backends against Copy scores. Six existing failure regressions now retain two
tied snapshots before the later failure. These add no source-method credit.
Existing Java/Rust performance workloads are refreshed; they retain their original
selection configuration and do not claim timing or random-trace parity for these
new policy options.

## Strategic finalist ranking

`FinalistRanking` defaults to `HighestScore`. Set `selection` to `BestAccepted`
to compare the evaluated neighborhood; the default `FirstAccepted` still picks
the first accepted move immediately. Both strategic modes rank accepted
candidates by structural state, overall improvement over their selected reference,
then earliest ordinary level improving that reference, then ordinary full-score
order. `StrategicOscillationLastStep` uses the current score;
`StrategicOscillationBest` uses retained best. References are immutable per scan
and refresh at each completed step. Level comparisons use exact `ScoreLevel`
numeric comparison, including full-width integers and arbitrary-precision decimals.
No floating projection is used for ranking. Rejected `BestScore` fallback retains
its existing lexicographic behavior.

An explicit early-pick condition selects that candidate immediately; the accepted
count limit instead selects the best finalist in its evaluated prefix. Seeded ties,
lazy saved state, failed-capture atomicity and callback-free installation keep
their existing contracts. Native callbacks across all six backends verify actual
selected-step neighborhood exclusions, including worsening moves and both reference
modes. Original generated ranking/permutation, exact decimal/full-width,
failed/panicking capture and 1,728 paired owned/Copy configuration checks add no
upstream coverage. See compatibility documentation for the public Java comparison
and its recorded exact-reference encounter-order differences.

### Lazy late-acceptance score history

`ScoreHistory<S>` is a nonempty native circular history used by `LateAcceptance`.
Updates write the current slot and advance with wraparound. Reset changes all
logical entries and preserves the cursor. Repeating an equal reset without writes
is a no-op; any intervening write makes the reset effective again. Bounds-checked
`get`, `current`, and slot-order `iter` borrow scores without cloning them.

Construction allocates slot storage but does not clone the initial score. Ordinary
updates and resets move scores and perform constant slot work. Each slot has a
generation stamp; a rare generation-counter rollover clears all slots. Stale owned
values remain allocated until overwritten, rollover or history drop. This trades
extra slot metadata and retained values for avoiding per-slot clones on resets.

`as_slice` materializes a contiguous snapshot only when requested, cloning every
logical slot once and caching it until mutation. Invalidating a materialized cache
also drops its entries, so the first subsequent mutation can do linear work.
`LateAcceptance::history()` preserves its borrowed-slice API using this cache;
normal search acceptance checks do not materialize it. Shared observations use a
thread-safe once-initialized cache for Send + Sync score types. Cloning a history
produces independent storage and can clone stale entries and a populated cache.

LateAcceptance initialization clones one score for its retained best. A best-score
improvement clones at most two scores, independent of capacity, preparing both
before changing history or best. Score-clone panics therefore leave completed-step
state unchanged; failed snapshot cloning leaves logical history intact and can be
retried. Equality during reset also precedes mutation. Existing hard/medium reset
policy and all six search-backend integration checks remain in effect.

Seven adapted buffer methods preserve every pinned slot/current fixture. Original
checks additionally cover generated eager-reference histories, generation rollover,
100,000-slot clone counts, concurrent cache initialization, independent copies and
injected clone/equality failures. These deterministic cost checks do not establish
a wall-clock speedup or Java internal allocation equivalence.

## Diminished returns

LocalSearchConfig::diminished_returns optionally stops a phase when the recent
retained-best improvement rate falls strictly below a configured fraction of its
initial rate. DiminishedReturnsConfig::new takes a positive Duration and positive
u64 numerator/denominator; equality continues and ratios above one are supported.
There is no default guessed from Java or floating-point conversion. The option is
disabled by default. All six basic/list/mixed and scoped/stream search paths share
the policy and report LocalSearchTermination::DiminishedReturns.

The first candidate-boundary poll arms the grace period after initial scoring and
its notification. No improvement by grace completion stops the phase. Otherwise,
the initial softest-level gain divided by the actual elapsed grace duration is
the reference rate; a late first poll therefore lengthens that reference period.
Later polls compare the softest-level gain over the configured trailing window.
The predecessor is the latest observed score at or before the window cutoff;
scores are piecewise constant, with no interpolation. Step completion and polls
observe retained best scores, including repeated timestamps. History coalesces
same-time updates and retains changes inside the window plus one predecessor.
Space depends on improvements within the window, not total elapsed steps.

A structural improvement or improvement in the lexicographic prefix containing
all hard levels and every level above the softest resets grace and history. This
includes the medium level and earlier soft levels for multi-level native scores;
all-hard scores reset on every best improvement. These are explicit native
generalizations, not claims of Java multi-level equivalence. Built-in i64/Decimal
scores use exact widened differences and BigInt cross-products. Custom scores
require numeric lexicographic ordering and exact_decimal arithmetic; unsupported
level types and zero-width scores return typed errors before scoring. Large
Decimal scale gaps retain the normal Decimal arithmetic allocation cost.

Expiry latches until an explicit restart. A candidate selected before an expiry
poll is still committed and retained; it cannot revive an already expired phase.
A higher-priority improvement observed before the first expiry poll may reset
grace. Timers are cooperative and do not preempt callbacks. Existing cancellation,
count/step controls and total/unimproved/shared time checks retain their order.
A zero evaluation budget never samples a clock. Disabling diminished returns adds
no clock reads. This option is phase-local; later solver phases may continue.

The public deterministic DiminishedReturns API separates phase_started (unarmed)
from step_started, supports explicit start and clamps backward time observations.
All eight pinned DiminishedReturnsTerminationTest methods are fully adapted and
attributed. Five original regressions cover full-width arithmetic, exact decimal
boundaries, large durations, history retention, level resets and 512 sparse traces
(51,200 events) against an untrimmed reference. Four injected-clock tests exercise
all six actual search paths; another checks phase handoff and a 50-second factory
initialization delay. These original checks earn no extra source-method credit.
Java wall-clock traces, internal ring-buffer storage and dedicated performance
comparisons for diminished returns are not claimed.

## Pooled construction (cheapest insertion)

ConstructionConfig::placement defaults to Sequential. Pooled compares assignments
for every eligible missing basic variable or list value in each step. The highest
score wins; equal scores preserve model/domain or registry/entity/index order.
With pick_early=Never, this follows the pinned public cheapest-insertion algorithm.
Other early-pick policies intentionally truncate the pool. Each winning placement
is installed from its saved score/cache without callback replay. Already assigned
values, pins and initial list order remain fixed. All six scoring/model paths and
ordered construction phases use the same selection machinery.

For mixed models, basic and list candidates compete in one pool. The configured
component order determines encounter order for ties and early picks, rather than
requiring one whole component to finish first. Each chosen move assigns exactly
one item. Required assignment reduces the native structural deficit, which ranks
ahead of optional placements. OptionalConstruction::Preserve skips optional values;
NonWorsening accepts only placements at least as good as the current score. A
rejected optional item can become useful after another placement, so it is
reconsidered on the next pool scan. An entirely rejected pool ends construction.
This never searches worsening combinations of optional assignments.

A step is a whole pool scan with at least one evaluated candidate, including an
interrupted scan or the final rejected optional pool. Candidate/evaluation caps,
cancellation and cooperative time retain the best evaluated prefix. A step cap
ends between scans. Empty pools consume no steps. No global optimality or hard
feasibility follows from Completed. Repeated scans generally cost more than
sequential construction: n required variables with d values need d*n*(n+1)/2
trials when all start unassigned and early picking is disabled. The implementation
enumerates moves lazily, retaining indices/eligible values plus the best candidate;
it does not materialize a vector of all candidate assignments.

Original references cover global order, every basic evaluation-budget prefix,
non-worsening optional reconsideration, pinned list prefixes and owner ranges,
coupled mixed scores, deterministic ties and owned decimal scores. Existing
scoped/stream matrices now run both placements across policies and budgets.
Injected-clock, cancellation and candidate-budget checks cover all six paths.
No pinned source method is credited: PooledEntityPlacerTest also checks Java
mock lifecycle/copy interactions that these native tests do not reproduce.

The public Java comparison has 66 original required homogeneous-domain graph
cases with pins and signed weights. CHEAPEST_INSERTION disables entity/value
sorting and uses NEVER early picking. Native assignments must match independent
Python reconstruction of every greedy candidate; both runtimes must return valid
assignments with independently recomputed scores. Final score agreement, selected
assignment agreement and exhaustive optimum counts are recorded separately.
This establishes neither intermediate Java traces nor list/mixed Java pooled
behavior, performance parity or adapted upstream-method coverage.

## Minimum significant improvement within a time window

LocalSearchConfig::significant_improvement optionally enables an independent
phase-local stagnation limit across all six search paths. Construct its validated
SignificantImprovementConfig from a Duration and a zero-structural score difference.
The config field borrows these immutable settings; each running phase clones them
and owns independent history. Keep the settings alive for the configured plan.
It observes retained best scores after initialization and ignores small gains until
the accumulated difference from the last qualifying anchor meets the threshold.
A qualifying improvement resets both anchor score and anchor time. Ties never
reset, including with zero or negative minima. Structural improvement qualifies
independently; otherwise comparison is lexicographic numeric score difference.

Unlike ordinary max_unimproved_time, this policy expires strictly after its
deadline: an improvement exactly at the deadline can still qualify. A late
improvement remains in the result but cannot revive the policy. Expiry latches
until explicit restart. Progress is a capped floating estimate and can equal one
at the deadline without expiry. Zero windows permit only observations at the
start timestamp; Duration::MAX cannot be exceeded by a Duration elapsed sample.
Backward time clamps. Public SignificantImprovement exposes the same deterministic
start, query and progress operations; queries before start are inactive.

Integer and decimal differences use exact Decimal arithmetic, avoiding wrapping
score subtraction and loss of tiny improvements at large magnitudes. A configurable
ignored suffix compares only the remaining prefix, so a one-hard-point threshold
can permit arbitrary soft deterioration. This is a native prefix operation, not
a finite MIN sentinel or a claim of Java wildcard behavior at numeric boundaries.
At least one ordinary level must remain. Custom scores require exact_decimal
levels and structural-first numeric lexicographic ordering. Empty scores, flawed
thresholds, unsupported arithmetic and invalid suffix widths fail during config
construction. Large scale gaps retain Decimal's ordinary allocation costs.

The optional policy composes with existing limits: cancellation/count boundaries
retain their order, then total/unimproved/shared time, significant-improvement
time, and diminished returns. Enabling both stagnation options means either may
stop the phase. Disabling the new option adds no clock reads. Initialization and
initial notification are excluded, including stream factory preparation. Candidate
prefix winners are still installed on expiry; later ordered phases may continue.
There is no new solve-wide score-difference limit in SolverLimits.

Two pinned scoreImproves methods from
UnimprovedTimeMillisSpentScoreDifferenceThresholdTerminationTest retain every
score/time fixture and phase/solver query/gradient assertion through the native
policy, with provenance. Mock scope identities and unasserted lifecycle cleanup
become native calls/lifetimes. The other three methods remain pending. Seven
original regressions include 640 histories / 64,000 events against widened integer
deadline references, numeric extremes, ignored suffixes, exact decimal boundaries,
invalid configuration and zero/max durations. Four injected-clock methods exercise
all six search paths, plus one ordered-phase factory/handoff test. These earn no
additional method credit. No Java timed-trace, shared lifecycle or dedicated
performance equivalence is claimed.

## Allocate receivers from a cycling value queue

ConstructionPlacement::ValueQueue independently implements the round-robin
value-first construction described by pinned Community public documentation
(optimization-algorithms/construction-heuristics.adoc, Allocate to value from queue).
For basic variables, the queue contains distinct domain values in first model/domain
encounter order, using Eq equality. A step selects one value and scores its eligible
unassigned receivers in model order. The best accepted receiver wins, with first
ties and the configured early-pick policy. The next step advances the value cursor;
a successful assignment permits another full rotation. Existing assignments and
pins stay intact; heterogeneous domains filter out illegal receivers.

Optional Preserve excludes missing optional variables. NonWorsening considers them
but rejects assignments below the current score. Rejected receivers may be retried
after another successful assignment. One full rotation with no installed assignment
ends construction, preventing an infinite loop when all optional choices are worse.
Values without any eligible receiver are skipped without scoring or consuming a
step. Optional rejections with scored trials consume a step. No queued state survives
a call: resumed construction preserves supplied values and restarts the value queue.

All score/candidate/step/time/cancellation limits retain their existing scope and
precedence. Interrupted receiver scans keep the best scored prefix. Selected caches
are installed without rescoring. Empty/unreachable required models retain existing
NoCompleteAssignment behavior; finite optional-only queues may finish unassigned.
Building the distinct queue uses Eq comparisons with worst-case quadratic comparison
cost in total domain entries; no new hash/order constraint is imposed on value types.
Like other candidate generation, queue preparation is cooperative, not preemptive.

List construction is already value-first and uses its existing sequential registry
order for this placement mode. Mixed models finish components in the requested order:
basics use the cycling queue and lists use sequential insertion. Unlike pooled mixed
placement, the two components never compete in a common queue. Optional basics are
not revisited after subsequent list construction; another phase may reconsider them.
Scoped and stream backends share these algorithms. Ordinary integer and owned decimal
score ordering determine receiver selection, with no floating ranking conversion.

Seven original regression methods check cycling/ties/resume, rejected rotations,
3,200 heterogeneous-domain reference comparisons (32 models, 25 score budgets, four
basic/mixed scoring paths), empty queues, list order, coupled mixed objectives and
sub-float decimal differences. Existing six-path injected-clock, cancellation and
candidate-budget matrices now include this mode. These checks confer no upstream
method credit; QueuedValuePlacerTest lifecycle/copy assertions remain pending.

A separate public-Java comparison uses ALLOCATE_TO_VALUE_FROM_QUEUE with original
entity/value order and NEVER early picking. All 67 original homogeneous required
graph cases agree on final assignments and independently recomputed scores; both
runtimes reach 24 enumerated optima. Native assignments also match independent Python
receiver reconstruction. The generated comparison does not establish Java internal
traces, heterogeneous/optional/list/mixed parity or dedicated performance results.

## Measured basic construction strategies

scripts/benchmark_construction.py provides dedicated end-to-end construction timing
for Sequential, Pooled and ValueQueue on homogeneous required basic-variable graph
models. Java uses only pinned Community public construction configuration, original
entity/value order and NEVER early picks. Both engines perform full score calculation:
one global native constraint and a Java EasyScoreCalculator. Neither incremental
stream behavior nor list/mixed/optional models are represented by these timings.

Each sample measures solver/director setup, initial scoring, complete construction
and returned assignment/score validation. Input parsing, model/network/factory setup,
process startup and output encoding are excluded. Factories/networks are reused;
per-solve state is fresh. Score-call counters exclude the final independent validation
recomputation while its time remains included. Counters need not agree between engines.
Warmups precede measured samples and fresh processes alternate runtime order. Timing
ranges include shared-host scheduling, allocation and JVM garbage collection effects.

There are 67 original small cases per strategy (201 total) with independent enumerated
optima, plus two timing sizes (32/96 variables) per strategy. Every returned assignment
is independently checked for completeness, domains, pins and score; each native
assignment also matches Python greedy reconstruction. The larger cases have known
zero optima: every contribution is nonpositive and a planted coloring attains zero.
The dashboard reports runtime, score/feasibility, known optimum and score-call range.
Agreement and optimum attainment are separate: a completed construction phase need
not return an optimal or even hard-feasible result. No upstream-method credit comes
from these original comparisons. Run the script with default warmup/sample/fork
settings for the full comparison; pilot runs with smaller counts are not equivalent
measurement evidence.

## Nested step-boundary stopping conditions

`LocalSearchConfig::step_termination` borrows a `StepTermination` expression for
basic, list and mixed local search with either scoring backend. `All` requires
every child and `Any` requires at least one; they can nest. Leaves test completed
steps, steps since a strict retained-best improvement, accepted installed moves,
an exact best-score target, or feasibility. For example, `All([Steps(20),
UnimprovedSteps(5)])` continues until both the minimum work and stagnation threshold
are satisfied. Existing mandatory budgets can still stop it earlier.

The expression is checked after initial scoring and at each completed-step boundary,
after the existing accepted-move, step, score, feasibility and unimproved-step
limits. It reports `LocalSearchTermination::StepCondition`. Evaluation/candidate/time
limits, cancellation and exhausted neighborhoods keep their existing checks and
precedence. Selected rejected fallback moves count as steps, but not accepted moves.
Score targets retain exact ordinary ordering, including owned decimal scores and
structural state. Feasibility still checks every hard level.

Children do not latch independently. A stagnation threshold reached while another
`All` child is false can become false again after a strict improvement. Empty `All`
is true and empty `Any` is false. Evaluation is read-only, short-circuited and uses
borrowed expression slices without per-query allocation. Deep expressions use the
call stack. Each phase uses its own counts and best score; reusing an expression
across phases does not carry state or prevent subsequent phases from executing.
Construction, exact search, elapsed-time expressions, progress gradients and shared
solve-wide composition are outside this option's scope.

Original regressions exercise 98 generated rule/acceptance configurations in all
six paths, initial/empty conditions, retained scores, rejected fallback counts,
resource precedence, neighborhood exhaustion, exact decimal/multilevel comparisons,
non-latching stagnation and repeated phase handoffs. The pinned AND/OR composite
termination tests were inspected, but their mocked solver/phase applicability and
gradient contracts are not fully represented by this native boundary API. They
remain pending; these original checks earn no adapted-method credit.

## Seeded basic ruin-and-recreate search

`ruin_recreate_search_from` and `ruin_recreate_stream_search_from` accept a complete
basic assignment and repeatedly clear a sampled group before greedy reconstruction.
The implementation follows the pinned public ruin/recreate behavior independently.
The caller supplies explicit seed, attempt/evaluation budgets, positive minimum and
maximum ruined counts, optional time/cancellation, native acceptance, construction
placement and early-pick policy. Counts clamp to the movable population; each group
uses uniform count selection and sampling without replacement through the native
PRNG. No Java random trace or default-strategy parity is implied.

Each repair freezes all unsampled values, including optional None, and preserves
original pins. Reconstruction visits selected variables in model/domain order,
using Sequential, Pooled or ValueQueue placement; sample order does not reorder
construction. Optional assignments use NonWorsening and may remain unassigned.
Scoped constraints must support partial assignments. Each stream factory supplies
fresh empty planning sources and the same deterministic objective; factories run
for initialization and each attempted repair, not when initial scoring is skipped.
Per-repair model/cache/graph rebuilding is intentional; this is not an in-place
incremental repair optimization. Solver-plan integration is described in the solver
contract.
List and mixed repair are described below.

The initial score, each repair's initial score and all candidate scores share the
exact evaluation cap. `repair_move_evaluations` counts only construction candidate
scores; `attempts` counts started repair calls, including unscored, partial, unchanged
or rejected ones. `complete_reconstructions` counts complete scored repair results.
`accepted_moves` counts changed complete successors accepted by the configured
policy; only these advance acceptance history. Rejected/no-op attempts resample
until a budget ends the search. Move identities are final model-index-ordered
Composite(Change) edits; value tabu receives their destination planning values.
These are native identities, not Java ruin/recreate descriptor equality.

Every complete scored repair may improve the retained best independently of whether
acceptance changes the current state. Partial interrupted repairs are discarded;
complete interrupted repairs may be retained. Returned best never loses quality,
need not be feasible and never proves optimality. Zero evaluations skips scoring;
zero attempts or zero time permits only the initial score. Validation precedes
skipped work. At attempt boundaries precedence is cancellation, elapsed time,
evaluation cap, candidate cap, attempt cap, score target, hard feasibility, then
no movable variables. Time includes validation, model/cache preparation, callbacks,
diagnostics and stream factories. Checks are
cooperative; active work cannot be preempted, and factory time is deducted from the
repair's remaining time before construction begins. Caller data is immutable;
errors propagate without returning a partial search result.

One full pinned test adaptation preserves all three final unassignment assertions
under the assigned-entity penalty, with both backends. Original regressions include
1,600 backend solves against independent full-score reconstruction over 32 varied
models and 25 budgets, local-optimum escape, exact work counts, pins/null preservation,
repeatable sampling, multiple acceptance families/placements, cancellation at every
score boundary, delayed/cancelling factories and sub-float decimal differences.
The crafted local-optimum case is escaped by Sequential/Pooled but not ValueQueue;
quality is not assumed equal across strategies. Dedicated Java trace/timing evidence
for this new search is not claimed, and the other selector/lifecycle tests remain
pending.

## List ruin-and-recreate search

`ruin_recreate_list_search_from` and `ruin_recreate_list_stream_search_from` share
the bounded search engine with basic repair. `RuinRecreateConfig`, termination
precedence, all work counters, native acceptance and strict complete-best retention
keep the same meanings. `RuinRecreateResult` and `ListRuinRecreateResult` alias the
shared `RuinRecreateOutcome` container with their existing scored-assignment types.
The basic sampling permutation is retained when the eligible population is unchanged,
preserving its established seeded trace.

List samples use registry identities. Values inside full pins or pinned prefixes
are excluded. Missing optional values with a legal unpinned receiver are eligible,
allowing a later attempt to reconsider earlier unassignment. Missing values without
legal receivers are excluded. Counts clamp to the eligible population. The sampled
values are removed; all other assigned values keep their owners and relative order.
Insertion may change their indices/neighbors but cannot change a pinned prefix.
Entity-specific ranges still constrain every insertion. Existing missing values
outside the sample stay missing: temporary ranges exclude them during repair.
The registry and its order never change, preserving scoped and stream value IDs.

Sequential and ValueQueue placements use registry-order greedy insertion; Pooled
compares every missing sampled value and legal insertion. Optional placement uses
NonWorsening. Full/prefix pinning, required completeness and caller data remain
protected across incomplete/cancelled repairs. List acceptors receive changed owner
indices, final lists for those owners in entity order as move descriptors, and
registry-ordered values whose owner/index changes (including unassignment and
shifted unsampled values). Sized tabu uses entity count. These conservative native
identities do not claim Java move equality or traces.

One full pinned method preserves the one-entity/three-value optional fixture,
list-size penalty and final size-one assertion through both backends. Eight original
regressions include 1,584 scoped/stream solves against independent full-score insertion
(24 models x 33 budgets x two backends), sampled subset preservation, pin/range/null
boundaries, stable value scopes/shadows, multiple placements and acceptance policies,
factory timing/cancellation/errors, complete-vs-partial cancellation retention and
exact decimal ranking. All earlier basic repair checks continue to pass.
Dedicated Java list-repair performance remains pending.

## Mixed ruin-and-recreate search

`ruin_recreate_mixed_search_from` and `ruin_recreate_mixed_stream_search_from`
reuse the same bounded engine for a combined basic/list problem. A single group
samples basic slots and list registry identities without replacement. Counts bound
the combined eligible pool, so an attempt may repair either or both components.
Basic pins, list full/prefix pins, ranges, unsampled basic values, unsampled list
owners/relative order, and unsampled missing optional values remain protected.
List registry indices remain stable for value scopes and stream shadows.

`MixedRuinRecreateConfig` combines shared search settings with explicit construction
order. Reconstruction evaluates the full coupled objective: Sequential and
ValueQueue finish components in the selected order; Pooled compares candidates
across both components. Constraints must handle partial mixed assignments. A
required incomplete repair cannot replace the complete incumbent. Complete optional
repairs may improve the best even when cancellation interrupts reconstruction.

Acceptance entity identities use basic slot indices followed by list entity indices;
the combined population sizes relative tabu. Sampling uses list *value* indices
at that offset instead; the two identity sets serve different purposes. Move
identities pair final basic edits with changed lists. Planning values explicitly
wrap Basic or List, preventing collisions even when their underlying Rust types
and values are equal. These identities do not claim Java move equality.

Ten original regressions include 2,016 solves against independent coupled tables
(24 tables x 21 evaluation budgets x two orders x two backends), order/placement
quality differences, single-member samples with pins and optional values, empty and
fully pinned models, deterministic tabu policies, metadata namespaces, cancellation
inside reconstruction, skipped factory work, factory elapsed time and typed errors.
These tests earn no upstream method credit. Repair still rebuilds models and caches;
Dedicated Java repair quality/timing remains pending.

## Shared repair candidate limits and score goals

All six basic/list/mixed scoped/stream repair entry points accept optional
`max_move_evaluations`, `best_score_limit` and `stop_when_feasible` through their
shared config. Defaults preserve existing behavior and seeded traces. Candidate
limits count only construction trials, excluding the original initial score and
each repair's initial score. Every repair receives the remaining candidate budget;
that budget cannot reset between attempts. Zero candidates still permits scoring
the supplied complete assignment, but starts no repair. The existing total score
budget includes all initial scores and candidates and remains independent.

Targets use exact score ordering; feasibility uses the score's hard-feasibility
contract. Both inspect retained complete best after initial scoring and after each
repair. They do not interrupt reconstruction midway or consider incomplete trial
scores. A complete truncated repair is retained before stop checks; partial required
repairs remain discarded. Optional repairs can be complete with missing values.

Boundary precedence is cancellation, elapsed time, total score limit, candidate
limit, attempt limit, target, feasibility, then no movable variables. A coincident
resource limit wins the reported reason even when the retained best reaches a goal.
Before initialization, zero total scores still precedes pre-cancellation. Error and
validation behavior is unchanged. Native counters/precedence do not claim Java trace
parity or add adapted-method credit.

Seven original regressions cover 2,550 independent score/candidate budget prefixes
(25 score limits x 17 candidate limits x six entry points), exact work/quality at
cutoffs, optional/required retention, coincident limits, initial and post-repair
goals, owned decimal thresholds, and 702 placement/early-pick budget combinations.
Solver-wide stagnation and phase-plan integration are described in the solver
contract. Standalone observation is described below.


## Repair solution notifications

The six repair entry points now have `_with_observer` counterparts accepting
`SolutionUpdate`. They emit the initial complete scored assignment and then each
strict retained-best improvement. Ties, incomplete required repairs, accepted
worsening states and unchanged attempts emit nothing. Complete optional repairs may
contain unassigned values. Notifications describe retained best independently of
whether the acceptance policy changes the current search state.

Callbacks are synchronous, borrow the assignment and receive an owned score.
They do not trigger additional scoring. The existing unobserved APIs pass no
callback and perform no notification-related score clones. Observed searches retain
the same work counters and seeded trace unless callback side effects stop work.
Observers must not mutate shared problem facts. Clone snapshots for publication.

Retention and acceptance bookkeeping precede improvement notification. Cancellation
from a callback is honored at the next attempt boundary, preserving the notified
solution. Callback time consumes the cooperative search deadline. Panics propagate;
input models and caller-owned assignment clones stay unchanged. Validation failures,
zero score budgets and pre-cancellation emit no initial event. Errors in a later
repair can follow earlier valid notifications; an error is not a successful result.

Five original regressions add 1,638 observed/unobserved budget pairs across all six
paths (3,276 solves), exact event/quality/counter equivalence, cancellation from
initial and improvement callbacks, callback timing, owned decimal scores, skipped
scoring, panic propagation/reuse, optional unassignment, and successive sampled
improvements. These checks earn no upstream method credit. Shared stagnation,
solver-plan and managed lifecycle integration are described in the solver contract.

## Arbitrary basic group changes

`MassChangeConfig::exactly`, `between` and `all` configure a lazy common-target
neighborhood over arbitrary basic slots. These groups need not share a current
value. Sizes zero and one produce no moves; reversed bounds return a typed model
error. All selects the full eligible source population before destination filtering.
Enumeration follows increasing group size, combinations in variable order, then
the first member's domain order and optional null. Every target must be legal for
every member. Pins are excluded and unchanged states skipped. No subsets are
precomputed. PillarSchema does not change selection of these independent slots.

Default null crossing follows each variable's optionality. Missing required slots
are excluded; missing optional slots can join. A null destination requires every
member to be optional. Explicit false excludes all missing members and null targets.
Explicit true is rejected if any model variable is required, including pinned ones.
Empty models are vacuously valid. `MassChange` exposes immutable member IDs, target
and an ordinary composite which writes every selected slot. Existing model and
score-director transactions validate and roll back these moves atomically.

`Neighborhood::MassChange` uses this family for basic and mixed search; list-only
search retains changes/swaps. ShuffledVariables alters member encounter/target
order; ShuffledMoves uses the bounded shuffle window. Existing acceptance, budgets,
tabu identity, score checking and observations apply. Basic/mixed sequences validate
all local-phase null policies before any scoring or graph factory, with phase-indexed
errors even for a zero global budget. Mass-change search performance and public
Java search quality remain unmeasured; existing pillar benchmarks are separate.

## Group assignment and unassignment

`MassAssignmentSize` supplies exact, ranged or all source cardinalities. Zero is
empty; exact sizes larger than population are empty; range maximums clip and
reversed bounds are errors. `mass_assign_moves` selects only unpinned missing
basic slots, including required slots in partial inputs. Groups need at least two
members and one non-null destination legal for every member. `all` uses the full
missing source population, not a smaller destination-specific set. This primitive
can complete partial assignments; local search still requires a complete input.

`mass_unassign_moves` selects assigned unpinned slots and permits singleton groups.
Every model slot must be optional, including pins; invalid models are rejected
even with empty sizes or budgets. Finite enumeration emits each subset once by
size and member order. Missing values never join unassignment groups. Both providers
return immutable MassChange metadata and ordinary atomic composite moves.

`sample_mass_unassign_moves` draws with replacement between groups and without
replacement within a group, bounded by an explicit draw count and seed. It chooses
a size uniformly from clipped allowed bounds, then a uniform subset in sorted ID
order. Empty/insufficient populations produce no draws; otherwise every requested
draw is produced, including repeated singletons. No Java PRNG/distribution parity.
It holds a linear-size source index pool and only constructs requested candidates.
All candidates refer to one immutable snapshot; regenerate after changing state.

`Neighborhood::MassAssign` and `MassUnassign` use finite enumeration in basic/mixed
local search and joint moves. List-only search retains changes/swaps. Existing
shared controls, scoring rollback and observation apply. Invalid unassignment
models are rejected before any basic/mixed phase scores or graph factory runs.
These providers have no dedicated Java/Rust performance comparison yet.

## Construction from custom providers

`construction_from_with_provider`, `list_construction_from_with_provider` and
`mixed_construction_from_with_provider`, plus their `*_stream_*` counterparts,
accept a partial assignment and the existing `MoveProvider` interface. Each step
opens a fresh pool against an immutable snapshot of the committed assignment.
Candidates can initialize multiple basic variables and list values atomically.
Stable ties keep the first best candidate; the saved winner installs without
replaying scoring callbacks. The result can be resumed or, once complete, supplied
to local search. Full solver plans also accept providers bound to construction
phases through `SolverExtensions`; see `solver-contract.md`.

Provider construction only extends assignments. Existing basic values remain
unchanged; each existing list stays a subsequence of the same owner's new list.
Insertions may shift positions but cannot relocate or reorder existing values.
Model validation enforces domains, ownership, and prefix/whole pins. Invalid,
non-progressing, or preservation-violating proposals are skipped before scoring
and counted in `rejected_moves`. This differs from provider local search, where
invalid moves are errors. Scoring errors still propagate. Validation currently
previews a cloned assignment; this is not an optimized incremental legality filter.

`ProviderConstructionConfig` has pooled selection, optional early picking, and
per-call score, move, step, raw-scan, elapsed-time and cancellation controls.
Required initialization improves structural score; optional-only initialization
must not worsen the score. Optional values remain untouched by default; use
`OptionalConstruction::NonWorsening` to consider filling them. A joint move is
accepted by its whole score, including structural progress. Score counts include
initialization, move counts exclude it, and diagnostic checks count toward neither.
`scanned_moves` includes every proposal, including rejected ones; its default
100,000 bound also stops an infinite stream of unusable candidates. Every callback
must return promptly. No additional provider pull proves exhaustion at an exact
limit, so a limit result may contain a complete assignment. Limits and cancellation
retain the best candidate already evaluated in the current scan.

`NoCandidates` reports a pool without an accepted initialization; it never proves
infeasibility. `complete` is independent of the stopping reason, and does not assert
hard feasibility or optimality. An empty optional-only pool may leave a complete
assignment. Zero score budget, pre-cancellation and zero time avoid score/provider
callbacks, after validating model and source ownership. Zero step/move/scan limits
permit initial scoring but do not open the provider. Original integration tests
cover all six entry points; they add no upstream compatibility-method credit.


## Queued and pooled construction placements

`PlacementQueue` captures a bounded finite key source at phase start, then opens
one keyed `PlacementSource` at a time against the current context. `Once` visits
all child sources for one key before advancing to the next key; `Cycle` repeats
that order. Empty key/child collections exhaust immediately. An empty candidate
pool is still a placement, so passive `has_next` does not promise usable moves.
A cycle with only empty pools requires a caller limit when searching for a usable
placement. `pooled_placements` repeatedly reopens a normal SelectionSource using a
single unit key. Composite moves can be generated by a child using SelectionProduct
or application-specific logic; the queue itself performs no scoring or filtering.

`PlacementSnapshot` owns shared immutable keys and candidates. Keys and moves need
not implement Clone; snapshots can be shared and traversed repeatedly after later
phase changes. Providers can clone native moves from a snapshot into their pools.
Key-source changes wait until the next phase, while candidate-source changes are
visible at the next opening. Per-key and per-pool collection bounds are separate;
unknown finite sizes are accepted, exact size declarations are checked, and an
unknown productive infinite source stops after at most limit+1 pulls. Individual
callbacks must return promptly. PlacementFn is a convenience for callbacks returning
vectors; the queue cannot limit allocations performed inside such callbacks.

Lifecycle delivery remains explicit. Every successful event forwards once to the
key source and each child. Ordinary key-build errors still deliver the entered
phase event to children, allowing balanced cleanup. Invalid transitions change
nothing. Failed/panicking builds or callbacks invalidate the current phase; finish active scopes and enter a
new phase to recover. Held snapshots remain valid. Partial callback delivery and
external side effects cannot be rolled back. There is no automatic solver listener
wiring or Java factory-copy implementation. Five complete upstream placement methods
preserve sequence, exhaustion, phase restart and all lifecycle counts; original
regressions cover limits, failures, ownership and two-variable joint placements
inside full solver plans with both basic scoring backends.


## Lazy sampling without replacement

`unique_indices`, `unique_slice_samples` and `unique_range_samples` provide seeded
uniform occurrence sampling without replacement. Injected-draw constructors on
UniqueIndices/UniqueSliceSamples/UniqueRangeSamples make selection reproducible
under caller-supplied draws. A sparse remapping table represents a shrinking live
prefix; each draw swaps in its final occurrence. Sampling k entries therefore
stores at most k remappings, without expanding the declared range. Counts/ranks
use u128, including ranges larger than addressable memory. This is separate from
full-cache shuffling and from repeated weighted sampling.

Slice sampling borrows payloads and needs neither Clone nor Eq. Range sampling
uses the immutable ValueRange contract and fetches only selected entries, including
through trait objects. Equal payloads at distinct indices remain distinct; this
does not deduplicate values. Passive availability/remaining queries draw nothing,
and empty/singleton pools do not call the random source. Native seeds are stable
without matching Java traces. The table's hashing order is never used for drawing.

An invalid injected draw or missing indexed value yields one typed error and ends
the cursor. A draw callback panic precedes mapping mutation; external callback
effects cannot be undone. Fallible size hints do not promise the entire remaining
count as a lower bound, and no ExactSizeIterator is claimed. Exhaustion removes
mapping entries but may retain allocated table capacity. Callers must bound
consumption when sampling only a small part of a huge range.

Three unique and two repeating upstream methods preserve all membership, repeated
drain/exhaustion, 300 repeating draws and singleton assertions. Unsupported Java
remove/bulk-drain exception methods remain pending. Original checks exhaust every
small draw path to prove a one-to-one mapping to permutations, cover u128 counts,
non-Clone/duplicate/optional payloads, callback failures and seeded reproducibility,
and initialize with unique lazy candidates through both scoring backends. There
is no new timing benchmark or full-source materialization in the sampler.


## Independently configured basic move families

`BasicNeighborhood` selects a neighborhood, optional variable indices and an
optional pillar schema. The indices supply encounter order and restrict every
move to selected slots. Schema overrides are valid only for Pillar/SubPillar;
None keeps the model schema. Row pinning, aligned swaps, domains and optionality
retain their existing behavior. Configuration validation rejects duplicate and
out-of-range slots before enumeration. An empty selection produces no moves.

`basic_moves_configured` returns a lazy iterator suitable for move providers.
`basic_moves_configured_union` combines independently scoped/grouped children,
retaining duplicate occurrences and configured union order. Zero-weight children
skip configuration validation and enumeration; assignment validation always runs.
The step parameter advances seeded union and sampled-family ordering. Candidate
bounds do not preempt grouping, rejected-subset scans or user callbacks.

`RepairNeighborhood::Basic` uses the same configuration in basic and mixed repair
search. On a mixed model it emits basic-only moves; list state remains intact.
List-only models reject it. Model-aware solver validation happens before scoring
or graph construction. MixedNeighborhood now delegates its basic component to
this shared implementation, retaining its public error shapes and defaults.

Four original regressions compare 432 scoped change/swap enumerations with direct
expected moves, preserve union multiplicity and repeatability, cross a grouping
barrier in complete scoped/stream plans under budget prefixes, reject invalid
configuration before callbacks, and compare Basic versus Mixed repair families
in both mixed scoring backends. These add no separate upstream method credit.


## Per-family target ordering

`BasicValueOrder` supplies a full permutation of one variable's domain indices,
plus exactly one None for optional variables. `sorted_by` builds a stable
permutation using actual optional values and a caller comparator, without cloning
values. Equal keys retain encounter order. Null placement is explicit in that
comparator. Rebuild these static permutations when domain order or sort keys
change; indices refer to the configured model, not a globally stable value ID.

`BasicNeighborhood.value_orders` and `MixedNeighborhood.basic_value_orders` apply
these orders independently per family. Unspecified variables retain domain order.
Supported families are Change, ChangeAndSwap, Assign, Unassign, Pillar, SubPillar,
PillarChange and PillarUnassign. Swap-only and product/mass/list families reject
nonempty target-order configuration; there is no silent fallback. Pins, scope,
current-value exclusion and family null policies still filter ordered targets.
Pillar changes use the first selected member's order for each selected column,
then intersect target membership across all selected members. Swap ordering is
unchanged. No complete candidate set or model clone is built to reorder values.

Duplicate/missing/invalid targets, required-null entries and repeated/unknown
variables fail before scoring or graph construction in complete solver plans.
Disabled components and zero-weight families retain their usual validation skip.
Four original regressions cover all six optional-target permutations across eight
single-target families and flat/schema grouping (288 configurations), stable ties
with non-Clone values, pins/ranges, invalid orders, distinct union children and
one-move-budget order sensitivity in both complete basic scoring backends.

## Multi-variable entity construction

`EntityConstruction` binds disjoint variable rows to one basic problem. Rows define
entity and column order without changing model IDs; different widths and omitted
slots are allowed. `SeparateVariables` finishes one column at a time, while
`JointVariables` lazily scores Cartesian combinations as composite moves. This lets
constraints spanning several columns guide initialization together. Existing values
and pinned slots are preserved; row/slot validity is checked when building the
provider. Pins retain their native per-slot meaning.

`OptionalConstruction::Preserve` skips optional columns. Both other policies allow
an optional column to stay empty within a joint candidate, including an empty-domain
column, while another column initializes. All-empty combinations are omitted; the
construction engine owns the explicit no-change choice for EvaluateUnassigned.
Its existing final no-change probe after provider exhaustion can add one placement
when optional slots remain. Pass the same optional policy to provider and engine.
The whole composite score determines acceptance; required structural progress
retains priority over ordinary score levels.

Storage and cursor state are linear in selected slots; neither domains nor the
Cartesian product are materialized or cloned at setup. Candidate values are cloned
only as the lazy product advances. `QueuedMoveProvider` supplies the existing retry,
advancement, rewind and reset semantics. Budget interruption with DiscardIncompleteStep
retains the same row for retry. Basic/mixed solver extensions accept the provider;
direct mixed use preserves lists, and MixedConstructionProvider schedules it with
list construction in either component order. Shared construction limits count one
joint placement, independently of how many columns it initializes.

Original regressions compare sequential and joint choices on all six basic/mixed
scoring paths, combine with list construction and shared limits on all three mixed
backends, and exercise optional subsets, schema/pins, retries and bounded lazy
consumption. No new upstream method credit: existing queued entity fixture mappings
remain unchanged, and this adapter does not add Java selector lifecycle wiring.

### Cached selection sources in actual search

`CachedMoveProvider` connects an existing `SelectionSource` and `SelectionCache`
to construction/local-search engines. A context callback captures owned selection
facts at the first opening of each scan. A separate mapper receives each borrowed
key together with the current problem, assignment and completed-step counter, and
returns a move or rejects that occurrence. Cached keys need not implement `Clone`.
The mapper can filter changing eligibility while a phase cache retains stable keys.
Use step scope when key membership or weights themselves must follow assignments.

`CachedProviderConfig` chooses solve/phase/step scope, original/weighted-random ordering,
maximum retained candidates, maximum draws per scan and a seed. Draw limits include
mapper rejections, so an all-rejected random stream terminates. Reopening a scan
uses the same cached keys and seed plus completed steps (wrapping); no hidden random
state is advanced across openings. Zero draws and unopened scans do not capture
context, build a cache or emit source events. Reaching a draw cap is not proof that
no further productive move exists.

Source lifecycle begins lazily at the first opening. Solver sequences send one
SolveStarted/SolveEnded pair per distinct registered provider, and retain an opened
source session across phases. Bind the same provider to several phases with
`PhaseMoveProviders::bind_shared`; solve caches then build once and survive unbound
intervening phases. Standalone engine calls keep a one-phase private session. Every
opened source scan has balanced StepStarted/StepEnded, using its captured pre-scan
context. Phase/solve end uses the most recently captured context. Keys release at
the configured boundary; explicitly retained snapshots remain valid. Automatic fresh
phase-score diagnostics do not restart provider lifecycles or reopen sources.

`try_moves` returns source size/weight/collection errors to direct callers. The
existing infallible `MoveProvider` interface panics on those errors instead of
returning an empty neighborhood. Engine panic unwinding skips user lifecycle
callbacks; `reset` closes the partially opened selection session before reuse.
It can continue cleanup if an end callback previously panicked, because the cache
advances lifecycle state before invoking the callback. External callback effects
cannot be rolled back. Reset must not run during an active engine invocation.

Six native tests exercise all three scopes on all nine scoring/model backends, shared
providers across two phases, repeated
solves and diagnostic scoring, construction with current-assignment filtering,
weighted sampling/replay and bounded rejection, lazy unopened/zero-draw paths,
source errors and panic recovery, and non-Clone key ownership with held snapshots.
They add no upstream test-method credit.


### Shared providers and solve lifecycles

`PhaseMoveProviders::bind_shared` borrows a provider once for multiple phase indices.
Duplicate indices are deduplicated; empty lists and collisions are rejected without
changing bindings. Existing binding validation still checks every phase before user
callbacks. `bind` replaces one alias while preserving others; `bind`/`remove` return
the previous provider only when its final binding is removed.

All nine solver backends own one provider solve scope. It begins at the first phase
kernel after shared plan validation and closes on success or a returned error,
including later graph-factory failure. All distinct registered providers participate,
including those whose phase is never reached. Entering a zero-budget kernel emits
only solve events; phases/scans preserve their zero-budget suppression. A run that
never enters a kernel emits no solve events. Start/phase/scan callbacks follow the
ordinary solve clock; solve-end cleanup runs after result timing is finalized.
Callbacks add no score/move work counts. Panic unwinding skips
further user callbacks; affected state must be reset before reuse. Source end events
can be resumed with CachedMoveProvider::reset after a caught panic. Do not wrap a
cached provider in a callback that resets it during an active solve; it already owns
its cache lifecycle. PhaseResetMoveProvider remains available for resumable queues.

Five additional native regressions cover atomic registration, replacement/removal
of aliases, validation and early-stop boundaries, unbound intervening phases, later
factory errors and caught-panic recovery. Upstream lifecycle-method credit still
requires complete case adaptations; native ownership alone does not confer it.

### Pair selection sources

`PairSelectionSource` composes one shared source into unordered positional pairs
(i < j), or two distinct sources into a Cartesian product with the right side
varying fastest. Equal payloads retain occurrence identity; two equal sources are
not implicitly shared. Each child receives every forwarded lifecycle event exactly
once, including empty children. Source items are collected by SelectionProduct under
its existing finite-source, exact-size and per-child-limit checks. Pair iteration
retains only those immutable pools and clones Arc handles for each selected pair;
no payload Clone bound or expanded pair list is required. ProductSnapshot::child_pool
exposes retained occurrences by original child index, with absent pools for omitted
children or an annihilated empty product.

The source can feed CachedMoveProvider; the outer cache then retains emitted pair
keys up to its separate candidate limit. A mapper interprets those keys as actual
moves and applies domain-specific filtering. Pairing alone does not filter equal
entity IDs, pins, invalid moves or no-ops. try_pairs returns typed collection errors;
SelectionSource::open panics on failure rather than reporting an empty selection.

Four full SwapMoveSelectorTest adaptations preserve every move code, exact finite
size, repeated exhaustion and all child lifecycle counts over two phases with two
then three scans. Shared and distinct sources include empty-source cases. The native
fixture sources supply only row IDs and log callbacks; production pairing generates
the candidate order. Mock Java scopes/identity are replaced by explicit native
lifecycle events and shared/separate source construction. Three original regressions
cover occurrence identity, limits/misdeclared sizes/unbounded input, a 100-million-pair
product retaining 20,000 non-Clone inputs, and real solver phases consuming all 30
swaps with one solve/two phase/five scan lifecycle per child.


Schema-bound move selection now supports `SchemaMoveKind::Assign` and `Unassign`
for selected optional basic and list fields, including combined mixed-model
providers. These variants require every selected field to allow unassigned values;
required fields are rejected during resolution, even when their slots are pinned.
Selection remains lazy and preserves field/slot ordering, ranges, pins and list
prefixes. Scoped assign/unassign providers do not impose optionality on unrelated
model fields outside the binding.

`SchemaMoveConfig::with_crossing_unassigned(false)` applies to Change/ListChange:
basic changes skip currently unassigned slots and omit unassignment targets; list
changes only relocate currently assigned values. True requires optional metadata
for every selected field. None preserves existing default change behavior,
including initialization of missing required values. Other move families reject
an explicit crossing policy instead of silently ignoring it. Dedicated transitions
can be composed through existing provider unions and phase plans. Construction,
local search, scoring, saved candidates and rollback use the same provider path.
Six Community tests preserve accepted/rejected metadata configurations and optional
change membership; three original tests cover scopes/pins and assign/unassign
search through all six basic/list/mixed full/stream paths. No Java trace claim.

## Validated termination profiles

`TerminationProfile` provides optional spent/unimproved time, signed external
step/move counts and exact rational diminished-return tuning. It rejects negative
counts, nonpositive ratios/windows, and the service-policy combination of
unimproved time with step/move counts. Lower-level native solver controls retain
their existing unrestricted combinations. Fallback merging fills each missing
field independently, preserves explicit zero and revalidates the complete result;
failed merges mutate neither input. No fallback returns a borrowed profile.

`with_limits`, `with_sequence` and `with_mixed_sequence` scope the bound settings
around a callback that can return owned solve results. Specified time/move budgets
replace those fields; unspecified controls survive. The profile's search-step cap
is OR-composed with an existing solve-wide expression and spans phases, excluding
construction. The temporary expression cannot escape the callback. No score clone
is needed. Supplying either diminished tuning field replaces local-phase policies,
using 30 seconds or ratio 1/10000 for the missing tuning field; supplying neither
preserves existing policies. Other phase kinds and mixed component order survive.

All 21 service SolverTerminationConfigTest methods have native adaptations, with
rational values and explicit None constructor defaults recorded in provenance.
These configuration cases do not establish Java runtime counters or timer scopes.
Original tests exercise cross-phase steps, candidate budgets, inherited expressions
and zero-work stopping through all six native solver paths, plus mixed-policy
binding with a non-Clone score, exact defaults and atomic conflict rejection.
