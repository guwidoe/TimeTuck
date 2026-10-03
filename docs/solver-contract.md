# Multi-phase solving

The native solver entry points run construction followed by ordered local-search or
exact phases, sharing score/candidate budgets, elapsed and unimproved time, cancellation and optional score/feasibility
goals. They provide an orchestration layer over the already-tested algorithms;
they do not establish whole-solver compatibility with Java.

## Entry points and plans

- `solve` and `solve_stream` handle basic-variable problems.
- `solve_list` and `solve_list_stream` handle ordered lists.
- `solve_mixed` and `solve_mixed_stream` handle coupled basic/list models.

SolverConfig contains SolverLimits, a ConstructionConfig and an ordered vector of
LocalSearchConfig values. Its default constructs and then runs one local-search
phase. An empty vector means construction only. MixedSolverConfig has the same
shape with mixed phase configurations, preserving the construction order and each
local phase's joint-move setting.

SolverPlan and MixedSolverPlan add an ordered phases vector. SolverPhase and
MixedSolverPhase select LocalSearch or Exact for each entry. All six entry points
accept either the old configuration or its corresponding plan. Defaults retain the
old construction-plus-one-local-phase behavior; conversion does not change it.

Construction preserves supplied initial values according to its existing contract.
Each local-search phase receives the retained best complete assignment. It creates
fresh scoring and acceptor state and restarts its configured random seed. A phase's
own limits, score goal, no-accepted-move reason or time limit normally end that phase
and allow the next one. Equal-score results retain the earlier assignment.

Scoped entry points reuse a borrowed network. Stream entry points take a fallible
factory returning a fresh graph and its empty planning sources for each started
phase. The factory must keep the same deterministic objective and immutable facts
across phases. Preparation and preloaded immutable-fact scoring are outside solve
evaluation counts; construction/local binding and initialization count normally.
Exact phases instead bind, score and roll back each complete candidate.
Skipped improvement phases do not call the factory. Construction still runs its
validation at zero budget, which invokes a stream factory but no phase score work.

## Shared limits and phase boundaries

The default shared score budget is 100,000. The candidate budget, cancellation token,
elapsed-time budget, unimproved-time budget, best-score goal and feasibility goal are optional/disabled by default. Each phase
gets the tighter of its own score/candidate cap and the remaining shared cap.
Construction/local initialization consumes one score evaluation and no candidate
evaluation. Exact phases have no initial rescore; every complete candidate consumes
one score and one candidate evaluation. Diagnostics consume neither count. Thus shared
candidate count is the sum of per-phase counts, not total score evaluations minus
one. Constraint-callback counts are independent: a candidate can invoke zero, one
or several callbacks.

A reported score/candidate limit ends the solve when the matching phase cap was
bounded by the shared limit (equal caps count as shared). Otherwise it remains a
phase-local stop. The phase's own reason is retained in its report. Before another
improvement phase begins, exhausted shared score or candidate budgets end the solve,
with score taking precedence if both are exhausted. This boundary check avoids
spending another initialization solely to discover that no candidate budget remains.
If the last planned phase ends naturally, consuming the last available evaluation
alone does not turn PlanCompleted into a limit reason.

A shared score goal is combined with a phase score goal by using the easier target
for that phase; only satisfying the shared target ends the whole solve for that
reason. Feasibility goals similarly combine for a phase while retaining their
scope. Shared goals are checked after construction and after each phase. Construction
is not interrupted mid-scan merely because an intermediate complete candidate meets
a goal. An observed shared evaluation limit takes precedence over a newly reached
goal. Completeness is required before either shared goal can finish the solve.

Cancellation is cooperative and propagates to all phases through the existing
CancellationToken. Phase tokens may be absent, or share identity with an enabled
solve-wide token (clones are accepted). A distinct simultaneous global/phase token
is rejected before work, rather than silently discarding one. Without a solve-wide
token, individual phase tokens remain usable; any cancelled phase ends the solve.
An already-reported shared limit retains precedence over a later cancellation poll.
Active callbacks, graph preparation, model validation and diagnostics are not
preempted. There is no hard cancellation-latency guarantee.

SolverLimits::max_time starts a monotonic clock on entry to the shared solve engine,
before plan validation. It spans construction and every improvement phase, including
stream graph factories, handoffs and diagnostics. Immediately after preparation,
each phase receives the smaller of its own elapsed limit and the remaining shared
time. This remaining time is never reset to the original solve duration. A phase
that reports TimeLimit with a shared-bound cap ends the solve; equal caps count as
shared. The engine also polls on phase return and before another improvement phase,
so a phase ending for a different reason may still finish the solve with TimeLimit.
Its original reason remains in its PhaseReport. A phase's shorter time limit can
allow later phases if shared time remains and construction is complete.

Expiry is cooperative: active callbacks, factories, validation, enumeration and
handoffs are not interrupted. Passing a remaining duration into a phase also leaves
a small handoff/validation interval before that phase's clock starts; this is not a
hard wall-clock deadline. Construction still validates and may score its initial
state when shared time is zero or preparation has consumed the budget. A factory
that fails still returns its phase error. Zero score budget, impossibility and
pre-cancellation retain their existing precedence. After a phase, reported shared
score/candidate caps and cancellation precede time; time precedes incomplete-state
and shared-goal checks. Later improvement phases do not start once expiry is observed.

Local-search stagnation timing remains available per phase. SolverLimits also
supports shared max_unimproved_time and max_unimproved_steps, described below.
Live problem changes, daemon operation, solver-manager services, concurrent phases
and cross-phase cache reuse remain unfinished. Standalone exact search keeps its
existing configuration and result types (see planning-contract.md). Exact orchestration
is specified below.

## Results and errors

SolverResult holds the retained assignment, optional score, completeness, aggregate
SolverWork, ordered PhaseReports and overall SolverTermination. Its solution()
returns a borrowed assignment and score only when complete and scored; this does
not imply hard feasibility. Partial construction results remain inspectable. An
incomplete construction never enters local search. A complete construction truncated
by a phase-local budget may proceed when shared resources remain.

Each report names its zero-based index (construction is zero), original phase reason,
retained phase score and work. Local accepted-move/step counts exclude construction
placements. Phase-local counters are not exposed as global stagnation estimates.
PlanCompleted means all planned phases ended, including phases stopped at their own
limits; it does not mean every construction scan finished, a score exists, or a global
optimum was proved. An impossible model returns NoCompleteAssignment before shared
zero-budget handling. If an incomplete initialization exhausts a shared cap, the
shared limit reason accompanies its partial state; otherwise it can enter an immediately following exact phase, or returns
IncompleteConstruction if the next phase requires complete input or no phase remains.

Every phase's acceptance and token settings validate before callbacks, even if that
phase will later be skipped. Model/network/source validation stays with the actual
phase entry points. Errors identify the phase (or invalid setting); a failed solve
returns an error rather than its earlier partial result. Caller models and networks
are borrowed, with search assignments owned separately. A graph factory may fail
when its phase starts; future graphs are not constructed eagerly for validation.

## Verification

Fourteen original integration methods cover all six entry points, two-phase handoffs,
exact shared-budget boundaries, paired backend accounting, competing phase/global
caps, goals, zero-work phases, incomplete construction, future-config validation,
factory failures, impossible models and controller-thread cancellation in a later
phase. A coupled four-state mixed problem proves that a later phase's joint setting
escapes a trap and reaches the independent exhaustive optimum. Its one joint move
causes two stream callbacks but one scoped callback; reports preserve the difference.
A public API doctest provides a minimal basic solve. Three of these integration
methods check real zero-time calls across all six entry points, phase-local versus
shared scope and precedence. Five additional internal tests inject a shared clock
into the engine and actual stream construction/search: one budget spans several
phases, graph preparation reduces it before initialization, shorter phase limits
and stagnation allow continuation, goals preserve time precedence, and disabled or
backwards clocks behave correctly. No sleeps or Java timed traces are used.

`cargo run --example mixed_routes` additionally checks scoped and stream orchestration
with both construction orders against the existing independent optimum 0hard/-4soft.
Basic-first uses 141 score evaluations in either backend; list-first uses 173. Callback
counts differ (249/353 and 313/469, scoped/stream), and the automatic stream totals
match explicit manual phase composition. These are correctness/example observations,
not runtime benchmarks or Java lifecycle evidence. All new tests are original and
add no upstream adapted-method credit.


## Solve-wide unimproved time

SolverLimits::max_unimproved_time is disabled by default. When enabled, one
monotonic timer spans local-search phases and returns SolverTermination::UnimprovedTimeLimit
on expiry. Construction, first graph preparation and first search initialization
(including its initial diagnostic scoring) are excluded. The timer starts when the
first improvement phase is ready (after local initialization or before exact
enumeration). Construction-only plans, skipped search and a shared goal met immediately
after construction leave it inactive. A zero limit still allows construction and
that first initial score, then stops before candidate scoring.

Only a strict improvement of the retained best resets the timer. Equal-score moves,
accepted worsening moves, recovery to an earlier best, phase completion and later
phase initialization do not reset it. Once active, later graph factories, handoffs,
initial scoring, diagnostics and candidate work consume the same budget. Phase-local
max_unimproved_time keeps its own independent start/reset state and may end one
phase while shared time remains. Neither timer is approximated by repeatedly
passing a smaller phase-local duration, which would mis-handle resets.

Expiry is cooperative, checked through the existing search polling points and on
phase return/boundaries. An improvement found by an active callback after the
deadline remains in the result, but cannot revive the expired timer. Backwards
clock readings are clamped; expiry is latched. Validation and callbacks remain
non-preemptive, including a later factory or initialization already in progress.
No hard latency guarantee or Java timed-trace equivalence is claimed.

Existing reported score/candidate limits and cancellation take precedence; shared
elapsed-time expiry then takes precedence over shared stagnation, which precedes
shared goals. Original phase reasons are preserved. For example, a zero-step phase
can retain StepLimit in its PhaseReport while a shared zero stagnation limit ends
the whole solve with UnimprovedTimeLimit after initialization.

Three original integration methods cover all six public entry points, both initial
states and mixed construction orders, construction exclusion, initial-score retention,
skipped future factories, construction-only/incomplete plans, early goals, whole-plan
validation and zero score budgets. Six additional controlled-clock unit methods run
real stream construction/search through the shared engine or inspect its clock:
strict/equal improvements, cross-phase initialization, first/later preparation,
competing local/global/time/goal limits, phase-return expiry and backwards/latched
timestamps. Tests use no sleeps. The pinned UnimprovedTimeMillisSpentTerminationTest
was inspected as behavioral context but has not been adapted; all six of its methods
remain pending and these native regressions earn no upstream method credit.


## Solve-wide unimproved steps

SolverLimits::max_unimproved_steps is an optional u64 limit, disabled by default.
The shared counter becomes active when the first improvement phase is ready and persists
across subsequent phases. Each selected move completes one step, including a
rejected fallback. Only a strict retained-best improvement resets its unimproved
count to zero. Accepted ties, worsening moves and recovery to a previous best
consume steps without resetting it. Construction, initialization, rejected
candidates without a selected move, and phases ending with NoAcceptedMove add no
steps. Later phases start from the retained best without resetting shared progress.

At the limit, the solve returns SolverTermination::UnimprovedStepLimit. Zero allows
construction and first search initialization, then no candidate work. Construction-only
plans and shared goals reached before local search leave this control inactive.
Per-phase max_steps and max_unimproved_steps remain independent; a shorter phase
limit may permit another phase while the shared counter retains its progress.
SolverWork.completed_steps remains the sum of completed local steps; it is not the
shared unimproved count. Individual PhaseReports keep their original stop reason.

The observer carries both step and time events. With only a step limit enabled,
neither the solve nor phase timer reads a clock. Phase elapsed/unimproved time is
checked before the observer's stop reason. After phase return, previously reported
shared score/candidate limits and cancellation precede shared elapsed time, then
shared unimproved time, then shared unimproved steps, then goals. This can preserve
a phase-local time/step reason while the shared step cap ends the whole solve.
Counters use the existing checked StepProgress arithmetic; the shared score budget
bounds the number of completed steps, and u64::MAX is a valid configured limit.

Six original integration methods exercise all six model/backend paths with multiple
phase sizes, caps 0..5, plateau improvements, rejected fallback/recovery, no-selected-move
phases, independent local caps, construction-only/early-goal paths, cancellation,
validation and simultaneous time limits. The plateau matrix independently checks
that exactly cap+1 completed steps occur (one improving step and cap unimproved
steps), except zero, which stops before the first candidate. Two unit methods check
clock-free step-only operation, reset/latching across fresh phase budgets and
simultaneous time/step limits with retained phase reasons. These are native
orchestration regressions. The existing adapted UnimprovedStepCountTerminationTest
method still covers its original phase threshold/progress assertions; this new
solve-wide feature adds no upstream method credit or Java whole-solve trace claim.


## Exact phases in ordered plans

ExactPhaseConfig defaults to 10,000 complete candidates. Optional candidate/time
caps, cancellation, best-score target and feasibility stop use the same shared
controls as local search. The phase retains the earlier complete best as incumbent;
it enumerates the full model space independently of movable initial assignments.
Basic domains, optional unassignment, list order, ranges and pins keep the standalone
exact-search semantics. Mixed enumeration visits each list assignment and then all
basic assignments. This is exhaustive enumeration, not branch-and-bound.

An exact phase can follow incomplete construction, including construction with a
zero phase-local budget. A zero-work exact phase may pass that partial state to
another exact phase. Local search still requires complete input. A newly found
complete assignment supersedes partial progress; equal complete scores preserve
the earlier assignment. A truncated exact prefix retains any better prior incumbent.

Scoped candidates use the network's full calculate operation. Stream phases create
one graph, validate empty planning sources, then bind each candidate in a transaction
that is dropped after scoring. All basic variables, entity lists and value shadow
rows are supplied; immutable facts survive rollback. No graph is constructed for a
skipped phase. Candidate errors propagate with the phase index. This scoring path
recomputes the candidate objective and makes no incremental exact-scoring performance
claim. Score callback counts exclude immutable graph preparation and include actual
candidate callbacks, even when several callbacks belong to one candidate.

Every exact candidate consumes one evaluation and one move_evaluation. Exact phases
add zero completed_steps and accepted_moves. They have no initial rescore. Shared
unimproved time starts before the first exact enumeration if no earlier local phase
started it. Later factories and handoffs consume the same timer. Strict exact
improvements reset shared stagnation, including the step baseline, without adding
local steps. An already-latched stagnation stop cannot be revived. A zero shared
stagnation limit permits construction and exact setup but no exact candidate scoring.

Before scoring each complete assignment, the exact phase checks score budget,
candidate budget, cancellation, elapsed/shared-stagnation limits, score goal and
feasibility goal, in that order. Enumeration and active callbacks are non-preemptive.
Only a visit to the next candidate can establish that an unvisited suffix remains.
Consequently, actually exhausting enumeration takes precedence over limits, goals
or cancellation reached by the final callback. Exhaustion ends the plan immediately
and skips future phases/factories. SolverTermination::Optimal and is_optimal()
require an exhausted exact phase plus a retained complete scored solution; hard
feasibility is not required. Without a complete solution, exhaustion cannot claim
optimality. PlanCompleted and any bounded prefix never prove it.

Fifteen original integration methods exercise all six public entry points, mixed
phase orders, candidate/shared-cap boundaries, incomplete handoff, cancellation,
phase/global goals, skipped-plan validation, source errors, empty/impossible models,
owned decimal scores, pins/optionality and a local optimum escaped by exact search.
Five internal methods verify shared timing, graph preparation, final exhaustion,
step reset without added steps, list shadows/immutable facts and error rollback.
These add no upstream method credit and do not assert Java exact-phase trace parity.

## Asynchronous solver manager

`SolverManager<R,E>` runs owned solve functions on an explicit fixed number of
native worker threads. `submit(problem_id, |token| ...)` queues work and returns a
`SolverJob` immediately. Jobs use u64 problem IDs; duplicate queued or active IDs
are rejected, and completion releases an ID for reuse. The queue is FIFO and is
not size-limited. Parallelism limits active functions, not threads that a caller
might independently create. Build the chosen basic/list/mixed solver and its score
network inside the function, passing the supplied CancellationToken into its
configuration. The captured input must be Send; returned results/errors must be
Send + Sync. Worker-local network closures can borrow non-Send state. See the
original `batch_schedules` example and six-backend orchestration regressions.

Manager and job handles expose Scheduled, Active and NotSolving. Unknown IDs are
NotSolving. Completion, registry removal, final status and result publication are
synchronized; an observed result is final. Cloneable job handles share an immutable
Arc<Result<R,SolverJobError<E>>> without requiring R/E to implement Clone. `wait`
blocks for completion, `wait_timeout` bounds the caller's wait without cancelling,
and `try_result` polls. Completed handles keep their original result when the same
ID is reused. The manager does not retain finished results after handles are gone.

`terminate_early` and `job.cancel()` are nonblocking cooperative requests. Scheduled
jobs are removed and finish with CancelledBeforeStart; their solve function never
runs. Active jobs remain Active until returning; passing the token to native solver
limits preserves the best valid prefix according to that solver's contract. A
function that ignores cancellation can still return success. Stale handles cannot
cancel a newer job with the same ID. Dropping a job handle does not cancel work.
This differs from Java's blocking terminateEarly and input-return behavior for
scheduled cancellation; those source methods remain pending.

Solver errors and unwinding panics become job results and leave the worker available
for later jobs. Standard panic hooks still run. Abort/OOM and double panics during
unwinding cannot be recovered. If destroying a caught custom panic payload itself
panics, the secondary payload is deliberately forgotten to prevent recursively
panicking destructors from killing the worker. Captured queued inputs are destroyed
outside manager locks, allowing their destructors to reenter the manager. Workers
also contain unwinding panics when destroying an abandoned final result, so the
next queued job can still run; destructors executed by a result owner remain that
owner's responsibility.

`request_shutdown` rejects further submissions, cancels queued jobs and requests
active cancellation without waiting. `shutdown` additionally joins all workers;
uncooperative functions can delay it indefinitely. Calling shutdown from the same
pool's worker requests shutdown but returns WouldBlock instead of self-joining.
Drop uses the nonblocking request and detaches workers; surviving job handles still
receive results. Avoid waiting inside a worker for work queued to that same saturated
pool. On partial thread-start failure, construction closes and joins workers already
started before returning the I/O error.

Three complete pinned SolverManagerTest methods establish parallel initialization,
manager/job status transitions and duplicate-ID rejection, with fixture attribution.
Original tests additionally establish queue cancellation, ID reuse, exact-search
best retention, typed failures/panics, multi-waiter ownership, FIFO/concurrency caps,
shutdown/drop behavior, reentrancy and cancellation across all six real solver paths.
Native u64 IDs/closures replace Java reflection/configuration; consumer callbacks,
intermediate best events, problem changes, automatic CPU sizing and live statistics
remain pending. No Java scheduling trace or manager throughput comparison is claimed.
The public Community lifecycle reference is
[Timefold's library integration guide](https://docs.timefold.ai/timefold-solver/latest/running-timefold-solver/library/library-integration).
