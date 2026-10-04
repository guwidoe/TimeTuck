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
basic assignments. This default Enumerate strategy retains its original behavior;
bounded tree exploration is opt-in as described below.

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
solver threads, with a separate consumer pool for final/error callbacks. `new` uses
one consumer; `with_consumer_count` accepts explicit positive solver/consumer counts. `submit(problem_id, |token| ...)` queues work and returns a
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
synchronized; an observed result is final and includes completed final/error callback
delivery. Active includes jobs queued for or executing consumer delivery. Cloneable job handles share an immutable
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

Seven complete pinned SolverManagerTest methods establish parallel initialization,
manager/job status transitions, duplicate-ID rejection, started callbacks, solver
error notification, consumer failure handling and skip-ahead delivery, with fixture attribution.
Original tests additionally establish queue cancellation, ID reuse, exact-search
best retention, typed failures/panics, multi-waiter ownership, FIFO/concurrency caps,
shutdown/drop behavior, reentrancy and cancellation across all six real solver paths.
Native u64 IDs/closures replace Java reflection/configuration; problem changes,
automatic CPU sizing, first-initialized events and live statistics remain pending. No Java scheduling trace or manager throughput comparison is claimed.
The public Community lifecycle reference is
[Timefold's library integration guide](https://docs.timefold.ai/timefold-solver/latest/running-timefold-solver/library/library-integration).


### Job callbacks

`submit_with_callbacks(id, run, SolverJobCallbacks::default()...)` configures
`on_started`, `on_final` and `on_error` with owned Send closures. Started is invoked
once on the solver worker before the solve function. If it panics, solving still
runs; the failure is reported later on a consumer thread. A successful result is
borrowed by the final callback; typed solver errors and captured solver panics skip
that callback. Consumers receive started failures first, then final-callback or
solver failures through the error handler. Each cause is delivered once. An error
handler that panics is recorded and disabled, without recursive notification.

`SolverJobFailure` distinguishes borrowed solver errors from callback failures.
`job.callback_failures()` exposes owned stage/message diagnostics after completion.
Final/error callback failure never replaces the solve's successful result or its
original error. The job stays Active, and its ID stays reserved, until delivery and
callback-capture cleanup finish. Waiters then see both the result and diagnostics.
Callbacks and their destruction run outside manager locks, and callback captures
are released before result publication. A slow final callback occupies a consumer
thread; the solver thread can take another job. Cross-job consumer order follows
completion queue insertion, not submission order; parallel consumers may overlap.

Queued jobs cancelled before starting invoke none of these callbacks. Cancellation
or shutdown after a solve starts still drains final/error delivery; late cancellation
does not erase a computed result. Shutdown joins solver and consumer threads and
rejects self-joins from either pool. Do not wait for the current job inside any of
its callbacks, or wait on delivery that requires a saturated consumer pool. These
are cooperative execution contexts and cannot preempt blocking user callbacks.

Nine original callback regression methods cover order/thread identity, blocked
callbacks freeing solver capacity, failure handling without result replacement,
handler disablement, queued cancellation, shutdown draining/wakeup, two concurrent
consumers, reentrancy/self-shutdown, count validation and capture release. The
existing six-backend integration now checks final callbacks, and batch_schedules
verifies eight deliveries. No Java consumer thread scheduling or throughput parity
is claimed. Java's OutOfMemoryError recovery test remains pending: aborting native
allocation failure is not represented as a catchable exception.


### Live best-solution delivery

`submit_with_progress` passes `(CancellationToken, SolverProgress<R>)` to the solve
function. Publish owned snapshots with `progress.publish(snapshot)` and register
`on_best` on `SolverJobCallbacks`. The producer decides which states are best;
the manager is generic over results and does not compare scores or automatically
publish the final return value. Snapshots must not alias mutable working state.
The publisher is cloneable without requiring R: Clone and is scoped to one run.
Its weak references cannot keep a finished job alive or affect a reused problem ID.

Publication retains only the newest pending snapshot while a callback is running.
A true return means accepted; it does not promise consumption of that particular
snapshot. Older pending snapshots are disposed outside locks on the publisher's
thread, with unwinding destructor panics contained. A false return means no best
listener, a disabled listener, or a solve that has already returned. A racing
publication during failed-listener cleanup may still be accepted and skipped.
Cancellation alone does not close an active producer. Returning or panicking from
the solve closes it, even if publisher clones survive on other threads.

Each job has at most one queued/running delivery. Consumer callbacks for a job
never overlap, even with multiple consumers. After one delivery, any update that
arrived during it goes to the queue's back, allowing other jobs to proceed. The
latest accepted update is delivered before final/error completion if the listener
remains healthy. Waits include that delivery. Shutdown drains it along with final
callbacks. Intermediate delivery can run while solving is still active; started
errors are reported before its first best event, or at final completion if there
are no events. A panicking best listener is reported once and disabled; solving,
final callbacks and error delivery continue. Errors never replace the solve result.

`exhaustive_search_with_observer`, `exhaustive_list_search_with_observer` and
`exhaustive_mixed_search_with_observer` expose synchronous retained-best updates.
They notify for the first eligible complete candidate and strict improvements;
ties and structural rejection emit nothing. Observers borrow retained snapshots,
may clone them into the manager publisher, and do not add score evaluations.
Observer work counts toward cooperative elapsed time; cancellation is polled at
the next candidate. A synchronous observer panic propagates, becoming a managed
solve panic if executed in a job. The `live_schedules` example connects actual
exact optimization to live consumers and verifies eight optimal final results.
Automatic intermediate observers for construction/local-search/ordered plans,
first-initialized events and dynamic problem changes remain to implement.

The complete adapted skipAhead method preserves the four assignments, blocked
consumer, skipped obsolete update, initialized result, fewer than four best events,
one final event and absence of consumer failures. Extra channels make the source
race deterministic. Original tests exercise multi-consumer ordering/coalescing,
shutdown, error chains, stale publishers, non-Clone results, destructor reentrancy,
and exact-search observers across all three model types. No Java event-object ABI,
custom phase API, exact callback schedule or manager performance parity is claimed.

### Construction and local-search observers

All six model/backend combinations now expose `greedy_*construction_with_observer`
and `*local_search_from_with_observer`. `SolutionUpdate` borrows the committed
assignment and carries its cached score and completeness flag. Observers are
synchronous, may borrow non-Send local state, and can clone snapshots into a
SolverProgress publisher. Input facts must remain immutable during a solve.

Construction emits its scored initial state, including a partial one, and each
installed placement after requested consistency checks. It emits no trial candidate.
Optional unassigned variables may already be complete. Local search emits its
initial retained best, then only strict improvements after the accepted step and
requested diagnostics finish. Ties and accepted worsening moves emit nothing.
Each phase independently emits its initial score; composed publishers may filter
equal scores at phase boundaries, as demonstrated by `live_heuristic`.

Observed and ordinary paths share search loops, validation, selection and scoring
implementations. A private director adapter supplies notifications. Ordinary
unobserved directors use a no-op hook; completeness is computed lazily only for
observers, avoiding added validation/Eq calls on existing paths. Observations do
not trigger scoring or change counters. Zero scoring budgets, pre-cancellation,
impossible spaces and validation errors emit no scored state. Callback panics
propagate to the caller; inside a manager job they become solve panics.

Observer work counts toward elapsed deadlines and, during local search, the
unimproved-time limit. Cancellation is cooperative at the normal polling points.
The already committed state is retained if an observer cancels. Original tests
cover all six paths, both mixed construction orders, every small budget prefix,
score/counter equivalence, partial progress, plateaus, accepted worsening moves,
validation at zero budget, optional/empty models, managed delivery and panic
containment. Controlled clocks verify observer time against elapsed/stagnation
limits without sleeps. No additional upstream method credit is assigned.

`live_heuristic` runs four construction/local-search jobs, publishes strict
cross-phase improvements, and checks every job's final score and assigned values.
Direct construction/local-search observer entrypoints are now available; unified
ordered-plan observers, first-initialized event semantics and dynamic problem
changes still require implementation. Native notifications do not establish Java
callback-count or event-producer parity.

### Observing an ordered solve

`solve_with_observer` and its list, mixed and three stream counterparts observe
one complete SolverPlan/MixedSolverPlan. They share the ordinary plan runner,
validation, phase factories, controls and result retention. `SolverUpdate` carries
borrowed assignment, cached score, completeness, native phase index and
SolverPhaseKind (Construction, LocalSearch or Exact). Index zero is construction;
subsequent indices are positions in the requested phase sequence plus one.

The first scored state is emitted, including partial construction. Later updates
must improve the solve's retained state; repeated initial scoring at a phase
handoff is suppressed. Complete results supersede incomplete ones, as in ordinary
result retention. Exact phases emit only candidates beating their carried-in best,
with no additional score calculation. Empty/zero-budget paths can emit nothing.
Observers may clone state into a SolverProgress publisher without manually
filtering cross-phase duplicates. Native metadata does not establish equivalence
to Java event-producer IDs or first-initialized events.

Observed local search receives the same shared stagnation/step control as ordinary
local search. Callback time remains part of phase/solve deadlines. Cancellation
from a callback is cooperative and retains the committed prefix. Full enumeration
on the final exact candidate still proves optimality, even if that final callback
requests cancellation; cancellation before further candidates does not prove it.
Panics propagate and become solve panics when called in managed jobs. A graph
factory failure can follow already-delivered events, but still returns a phase
error rather than a partial SolverResult. Facts/objectives must remain stable.

Original tests compare observed and unobserved results/work for every score/move
budget prefix in all six backends, both mixed construction orders, exact phases
following partial construction, metadata, duplicate suppression, shared step
stagnation, cancellation/panics, graph failure prefixes, and managed delivery.
Controlled-clock tests check shared stagnation across local/exact phases and exact
elapsed-time stopping. `live_plan` runs four observed construction/local/exact jobs,
checking monotonic delivery, final phase metadata and independently bounded score 8
with exact exhaustion. No additional upstream method coverage is claimed.
First-initialized callback semantics, custom/repeated construction phases and
live problem changes remain pending; the rewrite remains incomplete.


## Resumable and repeated construction

All six construction paths expose `greedy_*construction_from` and observed
`greedy_*construction_from_with_observer` variants accepting an owned assignment.
The original APIs delegate with the model's initial assignment. Explicit input
is validated before zero-budget, cancellation and impossibility returns; scoped
network dimensions and stream-source ownership/emptiness are also checked.
Existing values, full/prefix pins and optional unassigned values are preserved.
Missing required values alone are considered. List insertions can shift unpinned
indices while preserving prior values' ownership and relative order. Each call
creates a new scoring cache, counts initial scoring once and reports only the
placements made during that call. Zero scoring budget returns the supplied state
without a score or observer notification.

`SolverPhase::Construction` and `MixedSolverPhase::Construction` permit subsequent
construction phases. A partial phase can continue into construction or exact
search; local search still requires a complete assignment. Each construction
phase receives the retained prior assignment, its own controls and, for mixed
models, its own component order. A complete assignment is only rescored, without
replacing choices. Shared count/time/cancellation/goal limits retain precedence;
phase-local exhaustion may allow continuation. All future token conflicts and
acceptance errors are validated before any scoring or graph factory work.

Native observer metadata identifies each construction phase by its actual index.
Equal initial rescoring is suppressed by the plan observer. Construction does not
add local-search steps or reset shared stagnation. Stagnation remains inactive
before the first improvement phase; once started, time spent in later construction
and its graph factory belongs to that existing timer. A global expired deadline
stops the plan, including when later phases could finish initialization.

Nine original integration regressions cover all six model/backend paths, resume
from every small budget prefix, pins, optional values, explicit-input rejection,
observed/unobserved work parity, per-phase mixed ordering, handoffs from search,
cancellation, future token validation and graph-factory failure prefixes. One
controlled-clock regression checks deadline and stagnation behavior, and a public
plan doctest demonstrates partial construction followed by completion.

This establishes native repeated construction, without adding upstream method
credit. Custom phases, plans without an initial construction phase and Java
first-initialized callback semantics remain pending. In the pinned manager tests,
the helper `InitialSolutionEvent.isInitialized()` actually records
`!event.isTerminatedEarly()`; it must not be interpreted as an independent check
of solution completeness. The CH/CH/LS callback identifies the second construction
phase; single-CH and single-LS plans omit that callback entirely.


## Initialization boundary events and managed delivery

Six `solve*_with_events` APIs emit `SolverEvent::Best` and
`SolverEvent::FirstInitialized` through the same runner as ordinary and observed
solves. Best updates keep their existing duplicate suppression. Initialization
fires once when the phase immediately preceding the first requested local-search
phase returns normally. It carries the retained assignment, optional cached score,
completeness and `SolverInitialization { phase, kind, terminated_early }`. A partial
or zero-work result can therefore emit with early termination and no score;
completeness is not inferred from the termination flag. No extra scoring occurs.

The boundary follows the planned first local-search phase, including if shared
limits stop the solve at that boundary, incomplete construction prevents search,
or a native exact phase exhausts its space there. A producing phase error or a
stop in an earlier phase emits no initialization event. Plans without local search
emit none. Repeated construction identifies the last preceding construction phase,
even when it only rescores a complete assignment and produces no new best. Later
local/construction phases do not generate another initialization event. Existing
best-only observers do not receive initialization notifications. The synchronous
callback runs after the phase report is recorded; its time belongs to solve limits,
and cancellation or panic has the existing solve/manager behavior.

`SolverJobCallbacks::on_first_initialized` consumes an explicitly published owned
snapshot with its metadata. `SolverProgress::publish_first_initialized` accepts
at most one per job execution, shared across publisher clones. It returns false
without that listener, for subsequent initialization snapshots, or once the solve
function has returned. The manager retains this snapshot separately from the one
waiting best snapshot. Consumer batches preserve publication order between the
initialization and the retained (possibly coalesced) best snapshot. All callbacks
remain serial per job, and final/error completion waits for initialized delivery.
No initialization is synthesized from the final return value.

Initialization callback panic records `SolverCallbackStage::FirstInitialized`,
reports it once and leaves best/final delivery available. Solving failures also
preserve an already-published initialization event. Error-handler panic retains
its existing nonrecursive disable behavior. Publication and captured snapshot
destruction do not execute user code under manager locks. Shutdown drains accepted
initialization, and an old publisher cannot affect a later job using the same ID.

Three complete pinned SolverManagerTest adaptations now cover default CH/LS,
single CH (no event) and CH/CH/LS. Their helper preserves the source's
`!isTerminatedEarly()` meaning and producer identity assertions; native ownership
and consumer delivery replace Java event objects. Original regressions cover all
six native solver paths, budget prefixes, partial/complete early termination,
exact-phase boundaries, cancellation, factory/validation failures, deadline time,
slow consumers, reliable/coalesced ordering, concurrent publications, ID reuse,
callback failures and solving failures. The original `live_initialization` example
runs four managed construction/construction/local/exact jobs and checks event
origin, initialization snapshot, monotonically improving bests and proven optimum.

Six source initialization-event tests remain pending. Construction-free plans,
custom phases, the source early-CH exception contract and optional-list construction
behavior are not established by these adaptations. Native exact boundary behavior
and consumer scheduling are original contracts, not Java trace parity.


## Explicit sequences without construction

All ordinary, best-observed and event-observed solve entrypoints now accept
`SolverSequence` (basic/list) or `MixedSolverSequence`, containing limits and the
full phase vector. Existing SolverConfig/SolverPlan and mixed counterparts convert
by prepending their configured construction phase, preserving prior behavior and
phase indices. Sequence defaults retain the existing construction/local-search
plan. Explicit sequences may instead start with local or exact search; no hidden
initialization phase or extra scoring is inserted. Report, observer, producer and
error indices directly address the explicit vector.

A direct local-search phase requires a complete model initial assignment, including
at zero budget or pre-cancellation; invalid initial state returns a phase error.
A direct exact phase enumerates from its normal assignment-space order without an
initial rescore. Its bounded result therefore compares scored candidates, not an
unscored initial incumbent. If no scoring occurs, the result retains the model
initial assignment and its independently determined completeness, with score None.
A complete but unscored result is not a scored solution or an optimality claim.

Empty explicit sequences return SolverError::EmptyPlan. Empty/invalid sequence
validation happens before model-fact cloning, scoring and graph creation. A valid
first exact phase checks scoped dimensions or stream-source ownership/emptiness
even at zero budget. Existing whole-solve controls still clip each actual phase;
initial scoring remains allowed at zero elapsed time for local/construction phases,
while exact search charges each visited candidate. Graph factory work belongs to
the shared clock. Local initial scoring precedes stagnation timing; exact search
starts stagnation before enumeration.

The first-local-search initialization boundary exists only if a preceding phase
exists. Starting in local search emits no FirstInitialized event, even if later
construction/local phases run. Starting in bounded exact search can hand off to
construction or local search, with the usual completeness requirements and direct
producer indices. The historical IncompleteConstruction termination is retained
when partial initialization cannot proceed to the next phase or remains incomplete
at the end; this also covers bounded exact initialization.

One complete pinned SolverManagerTest adaptation now covers single local search:
both entities start at the first value, ordinary score is zero, the shared target
is zero, initialization and empty final consumers are registered, and final wait
leaves the initialization marker false and producer absent. The attributed model,
constant scorer and event helper are reused. Eight original integration methods
exercise all six paths, explicit indexing, bounded exact proof, every shared count
prefix, zero work/cancellation, initial-state preservation, invalid configurations,
network/source validation, partial handoffs and event absence. A controlled-clock
unit method verifies first-factory time and initial-scoring/stagnation behavior;
a public sequence doctest demonstrates direct local search.

Custom phases, both source early-construction initialization tests and live
problem changes remain pending. This increment adds one upstream method, not a
claim of general phase or lifecycle parity.

Construction phases now also accept ConstructionConfig::max_steps. The per-phase
budget counts new required assignments (across both components for mixed models),
not candidate trials. A next required item after the cap produces a phase StepLimit;
completing the last item exactly at the cap produces Completed. A later construction
phase gets a fresh count and preserves the earlier assignments; exact search may
finish partial state. Initialization events retain the producer's early-stop flag.
The existing native IncompleteConstruction result for incomplete local-search
handoffs remains unchanged, and these native regressions do not earn credit for
Timefold's early-CH event methods. See incremental-search-contract.md for boundaries
and precedence. Local-search work/stagnation counters exclude construction steps.


ConstructionConfig::optional now selects Preserve (default) or NonWorsening. The
latter considers missing optional values and rejects insertions/assignments worse
than the cached current state. Equal-score placements are allowed. Construction
results distinguish completed scan steps from installed assignments; optional
all-rejected scans consume one step. SolverWork and solve-wide stagnation remain
local-search-only counters. Initialization-event early termination continues to be
independent of completeness: the adapted optional-list case is complete throughout,
ends construction at StepLimit, executes zero local-search steps and delivers the
event from construction phase zero. Its manager wait includes callback delivery.
Four source initialization cases remain pending. See incremental-search-contract.md
for the optional policy, budget and resume details.

## Native custom-command execution

The custom_phase module provides a shared command engine for basic, list and mixed
models, with scoped and stream scoring. Public entry points are custom_phase,
list_custom_phase, mixed_custom_phase and their stream equivalents, each with a
_with_observer variant. They take an explicit initial assignment, CustomPhaseConfig
and one closure receiving a typed CustomPhaseContext. These standalone functions
do not emit SolverInitialization boundary events. Ordered custom phases bind the
same engine into solver sequences and managed initialization as described below.

Context assignment()/score() expose the current working state read-only.
execute(move) validates, scores and commits a move through the existing transaction
cache; it may intentionally worsen the working score. best_assignment()/best_score()
retain the earliest strict committed best, with completion preferred over partial
state. The returned CustomPhaseResult contains that retained best, not necessarily
the last working state. Native partial assignments are permitted for initialization;
completeness, hard feasibility and command completion are separate concepts. No
custom command establishes an optimality proof.

evaluate(move, visitor) runs a visitor with a freshly scored candidate applied and
then rolls back both assignment and scoring cache. Its return value cannot borrow
the temporary state. Temporary improvements never replace the retained best or
publish progress. RAII rollback also occurs on visitor unwind. A caught visitor
panic can resume the command; the attempted evaluation and its known constraint
work remain charged. Observer variants publish the initial scored state and subsequent
strict committed bests through synchronous SolutionUpdate observers. Tied/worsening
commits and temporary trials publish no best update. As elsewhere, model facts and
captured scoring data must stay immutable during a session.

CustomPhaseConfig independently bounds score slots (including one initialization),
move-evaluation slots and elapsed time, and accepts a borrowed cancellation token.
Input shape, assignment validity, and stream ownership/empty-source checks happen
before zero-budget or cancellation returns. A zero score limit invokes no scoring,
command or observer; pre-cancellation also skips scoring. Zero move/time limits allow
initial scoring and its observer but prevent command execution. Time starts before
director initialization, includes callbacks and command code, and excludes graph
preparation before entry. A command must return or poll is_terminated(); no user
code or scoring callback is preempted. execute/evaluate also poll before acting.

Termination is checked before the command, before each operation and after return.
The first observed reason is latched. At a poll, cancellation wins over time,
then score exhaustion, then candidate exhaustion; the pre-initialization zero-score
check precedes cancellation. Exactly consuming the last allowed operation can thus
report a limit after a normally returning command. Operation APIs return Ok(None)
when a limit prevents work, retaining any earlier committed best. Work reports
score/evaluation slots, successful constraint callbacks and committed moves; it
includes successfully scored temporary trials and caught visitor panics. Diagnostic
assert_consistent() recomputes caches outside work counters/budgets.

Scoring and validation operation errors return CustomPhaseError::Scoring with the
underlying typed error and invalidate the invocation. If a command catches such an
error and returns success, the phase returns IgnoredOperationError. Caught scoring
or commit panics also invalidate the phase, because their callback work may be
unknown. Successful diagnostics cannot revive a failed context. Command(String)
reports explicit user-command failure. Uncaught command, observer and scorer panics
propagate normally. Progress already delivered before an error is not retracted.

Eight original integration methods cover all six paths: temporary rollback,
worsening/tied commits, earliest bests, partial initialization, every small budget
prefix, visitor/scorer panics, ignored errors, explicit errors, cancellation,
zero-time behavior and validation before zero work. Two controlled-clock unit
methods cover initialization, observers, command bodies, candidate scoring, latched
timeouts and count-limit precedence. A public doctest demonstrates temporary trials
and retained bests. These are independent native contracts and add no upstream
method credit. Deferred scoring and full Java structural-error/command API parity
remain unimplemented. Ordered integration and its source adaptations follow below.

## Ordered custom phases

SolverPhase::Custom and MixedSolverPhase::Custom now hold CustomPhaseConfig.
The six solve families each add _with_commands and _with_commands_and_events
entrypoints. A typed FnMut receives the actual phase index and command context;
use the index to dispatch different commands. Configurations remain model-independent
and cloneable. A callback is invoked once for each entered custom phase, unless
initial scoring already reaches a limit. Ordinary solve/observer/event entrypoints
reject any unbound custom phase before model cloning, scoring or graph creation,
including phases that a previous exact proof or limit would otherwise skip.

Commands execute through the same transactional engine as standalone custom phases.
Each phase starts from the retained best of its predecessor, charges one initial
score, and returns its retained committed best. Temporary improvements and worsening
working states never replace that handoff. A custom phase can accept or produce
partial native assignments; following custom/construction/exact phases can continue
initialization. An incomplete handoff to local search returns IncompleteConstruction.
Custom phases never prove optimality. Separate custom phases provide separate
retained-best handoffs; a single callback owns its own internal operation sequence.
The Java deferred-scoring command methods remain unimplemented.

Shared score/candidate limits clip phase-local limits. Cancellation token conflicts
are validated throughout the sequence before work. Shared elapsed time includes
stream factory preparation and clips custom time immediately afterward. Initial
scoring remains allowed at zero time; zero score/pre-cancel still skip it after
validation. The existing custom clock includes scoring, command and observer work.
Global score/feasibility goals inspect the retained committed best, including the
initial state, before allowing another operation; temporary trials do not meet goals.

Custom phases start the shared stagnation monitor after their initial score and
notification, as improvement/exact phases do. Initial scoring in later phases does
not reset it. Strict committed improvements reset it only before expiry; late bests
are retained without reviving an expired solve. Custom commits do not count as
local-search completed steps or accepted moves. Existing local-search step counts
remain shared across custom phases, and a strict custom improvement can reset their
stagnation baseline. Phase reports expose Custom(CustomPhaseTermination); the four
new goal/stagnation reasons are used only by ordered execution. Standalone controls
remain unchanged. Cooperative polling cannot preempt command code or callbacks.

The phase immediately before the first requested local search remains the sole
initialization producer. Custom produces SolverPhaseKind::Custom with its actual
index; only Completed is non-early. Events can carry partial or unscored retained
state and occur before boundary stopping, as with construction. Custom phases with
no later local search emit no initialization. Errors are located SolverError::Custom
with typed underlying scoring errors or explicit/ignored-operation command failures.
Factory errors retain SolverError::Phase. A failing producer emits no initialization.

Seven original integration methods cover all six paths, plain/event variants,
retained handoffs, partial initialization, phase indices, every small shared score/
candidate budget, goals, phase-local limits, cancellation/conflicts and failures.
Two controlled-clock methods use real stream scoring to check graph preparation,
initialization, candidate prefixes and stagnation across repeated custom phases.
A public doctest demonstrates the typed callback. Three SolverManagerTest methods
are fully adapted: custom/CH/LS, CH/custom/LS, and two custom phases without LS.
They preserve each command's pre-initialization assertion, final wait and producer
assertions. A deterministic one-step stagnation limit replaces the source's 1 ms
limit; no time/trace parity is claimed. Notices/provenance record that substitution.
The required-variable early-CH case uses the strict handoff policy below.

## Incomplete local-search handoffs

SolverLimits::incomplete_handoff selects IncompleteHandoffPolicy::ReturnPartial
(default) or Error. ReturnPartial retains the existing IncompleteConstruction result.
Error rejects a partial retained assignment when the next requested phase is local
search, returning SolverError::IncompleteLocalSearch with that destination's index.
The error is a typed invalid-state condition; its message says local search needs
to start from an initialized solution. This policy applies at a phase boundary;
standalone or first-phase local search keeps its existing input validation errors.

The preceding phase's best updates and initialization event are delivered before
this check. The invalid local phase does not build a graph or calculate a score,
even if its own work limits are zero. Solve-wide cancellation, time/stagnation and
reported budget stops take precedence. Under Error, a coincident exhausted shared
score/candidate budget also stops normally because the local phase cannot start;
this includes construction step limits that win their phase-local precedence check.
ReturnPartial keeps its existing boundary-reason precedence. Callbacks can cancel
or consume the remaining shared time before an error would otherwise be raised.

Error does not reject a final partial solution with no following local search,
or a partial handoff to construction, custom commands or exact search. Those phases
can continue initialization, and any later invalid local handoff uses its real index.
Complete assignments behave as before. Progress already published is not retracted
on failure. Managed waits include queued initialization delivery; a failed solve
has no successful final callback, and SolverJobError::Solve preserves the typed
SolverError as its source.

The remaining SolverManagerTest.firstInitializedSolutionConsumerEarlyTerminatedCH
method is now adapted in full, including its conditional exception assertions through
the native typed error path. It uses two required entities/two values, an assigned-
entity penalty, CH step limit one, and a following local search. Explicit Error
selects native rejection behavior; it is not a Java default-policy or exception-ABI
claim. The source called/early/producer assertions are preserved. Extra synchronization
proves a blocked initialization callback delays the failed result; the snapshot
contains exactly one assigned value and structural -1/ordinary -1. Four original
cross-backend methods cover both policies, partial/resumed/custom/exact handoffs,
zero work, shared stops and event prefixes. A controlled-clock unit check covers
initialization callback time, and a public doctest demonstrates the policy.

## Live problem changes

`LiveSolverManager<P, R, E>` owns mutable input per job and reuses the fixed solver
and consumer pools. `submit` takes a solve closure that rebuilds a native model and
scorer from `&P`, reconciles the previous `&R` for a warm start, and uses the supplied
per-run cancellation token. Changes execute serially on the solver worker as
`FnOnce(&mut P) -> Result<(), String>`. A change arriving during a run cancels that
run's token; after it returns, queued changes apply before a fresh solve. Each run
rebuilds ordinary phase/solve limits. Explicit job cancellation stays latched.

Changes receive monotonically increasing revisions. `ProblemChangeReceipt` succeeds
only after the configured user consumer returns successfully with that revision or
a later one. Applying a change, scoring or publishing a snapshot is insufficient.
Best delivery coalesces obsolete waiting snapshots; consuming a later revision
acknowledges every included change. Changed problems may have lower scores, so each
completed run is eligible for publication. `LiveSolution` retains the revision and
an `Arc<R>` without requiring `R: Clone`. Published results must be independent of
mutable input facts; shared interior mutation is the caller's responsibility.

Daemon jobs wait on a condition variable after ordinary termination. New changes
wake them; explicit cancellation/shutdown ends them. Non-daemon jobs atomically stop
accepting changes once their run ends with no queued changes. A queued job accepts
changes before its first run. Unknown or closed jobs fail immediately and identify
the problem ID. Final-only delivery postpones receipts until job completion, also
in daemon mode; consumers should use Best when acknowledgements are needed while
a daemon remains running. A receipt timeout does not cancel anything.

Cancellation does not preempt user code. An already executing change may finish and
be delivered; later unapplied changes fail with Cancelled. Queued cancellation drops
the job without solving. Change errors fail outstanding receipts with ChangeFailed;
solve errors/panics fail still-pending applied receipts with SolveFailed. Unapplied
receipts may already have been closed as Stopped or Cancelled. Consumer panic fails
pending receipts with ConsumerPanicked, records its normal callback diagnostic and
stops the live loop. A successful solver result can coexist with consumer failure.
No rollback of partially applied problem changes is promised. Previously settled
receipts never change. Job waits include callback completion.

This API publishes completed runs, not intermediate improvements during a run. It
requires explicit model/scorer rebuilding and warm reconciliation; it does not
provide Java reflection, ProblemChangeDirector, automatic object graph repair or
full real-time-planning compatibility. Ordinary SolverManager jobs are unchanged.

Three complete source adaptations cover addProblemChange, unknown-ID rejection and
changes to waiting solvers, using real native custom/construction/local phases and
the original four-entity/four-value fixture. Original checks cover all six native
solver paths, added/removed values and score decreases, blocked/coalesced consumers,
active scoring interruption, fresh limits, final-only daemon delivery, cancellation
during changes, queued cancellation, reused IDs, failures and concurrent shutdown.

## Owned job input and synchronous cancellation

`SolverManager::submit_owned` accepts an input of the same type as the result.
The solve closure receives that owned input, cancellation token and progress
publisher. If cancellation or shutdown removes the task while it is queued, the
manager returns the exact input as a successful result, without cloning, scoring,
solving or invoking any callbacks. Once a worker marks the job Active, the input
belongs to the solve closure and cancellation is cooperative. Existing opaque
submit APIs still return CancelledBeforeStart for queued cancellation. Rejected
submissions drop their input; this API does not add input recovery on rejection.

`SolverJob::is_termination_requested` exposes the run's latched request flag after
completion. Requesting cancellation on a completed/stale handle does not set the
flag or affect a reused ID. `cancel_and_wait` requests cancellation and waits through
callback delivery. `SolverManager::terminate_early_and_wait` captures the currently
registered job before cancelling/waiting; unknown or completed IDs return None.
The asynchronous cancel/terminate methods retain their existing behavior.

Synchronous waits on unfinished jobs from the same pool's solver or consumer thread
return io::ErrorKind::WouldBlock after requesting cancellation. Already completed
results (including immediately cancelled queued jobs) remain available from those
threads. A thread-local pool identity is scoped to each worker's lifetime; no unsafe
code or thread-name matching is used. Cross-pool waits are allowed, but callers must
avoid cyclic dependencies. External callers can still wait indefinitely on user
code that ignores cancellation. Existing direct wait APIs remain unchanged.

Three source cancellation methods now preserve synchronous status boundaries,
termination history and exact queued-input identity. Actual native custom commands
hold the active fixture until cancellation to remove scheduling races, with test
limits; no Java cross-phase timing parity is claimed. The CPU-scaled source batch
method runs twice the available CPU count through three consumer modes using two
real custom assignment phases. Its producer selects the two assignment improvements,
omitting the native initial all-unassigned snapshot. It preserves every source
initialization, final-delivery, coalescing-count and entity/value assertion with
stable native identities. Six original regressions cover ownership, shutdown,
active callback waits, pool-thread deadlock avoidance, reused IDs and non-Clone inputs.

## Problem size and managed statistics

`ProblemStatistics` derives `ProblemSizeStatistics` from finite, list and mixed
models without scoring or enumerating assignments. Counts include pinned slots;
a basic slot or list owner is one entity/variable. Values are deduplicated across
basic domains; list models use their registry; mixed components have distinct
namespaces. An empty finite model has no separate fact registry, so its value count
is zero. Java class/variable-name maps and unused external fact registries are not
represented by this native model.

`AssignmentSpace` counts structurally complete assignments before scoring constraints
using BigUint. Basic counts multiply each unpinned domain plus its optional null;
pinned slots contribute one. An empty required domain yields zero and the empty
model has one assignment. List counts retain fully pinned lists and pinned prefixes,
then distribute ordered free values across the remaining labelled suffixes. Optional
lists sum over every selected subset size. Per-owner restrictions that exclude a
free value mark the unrestricted count as an upper bound (`exact = false`). Mixed
models multiply their component spaces and retain the bound quality. Decimal output
is full digits, without Java locale grouping, floating logarithms or rounding.

`SolverJob::statistics` exposes revisioned immutable model size, optional SolverWork,
measured duration and whether timing is active. `submit_with_statistics` accepts size
computed before submission, so queued jobs can report it. `SolverProgress::record_work`
records cumulative counters; `add_work` accumulates completed-run counters. Work is
None until reported, not a fabricated zero. Decreasing cumulative updates and stale
problem revisions are rejected. Overflow saturates and sets work_overflowed. Speeds
are unsigned events per second, floored and saturated; zero duration gives zero,
and unreported work gives None. Reports do not require solution consumers.

Timing begins at the solve function, after started callbacks, and freezes when it
returns or unwinds. Queue time and later consumer delivery are excluded. Best
consumers may run concurrently with solving; their overlap does not pause the solver
clock. Queued cancellation has zero solving duration and no work report. Closing the
run rejects stale statistics publishers, including after reuse of the problem ID.
Previously reported counters survive solver failure; missing failed-run work is not
invented. This is explicit telemetry from native solver results, not automatic
instrumentation of arbitrary user closures.

`LiveSolverManager::submit_with_statistics` accepts a model inspector and a solve
closure returning `LiveRun { solution, work }`. Inspection occurs before queueing
and after each applied change batch; a published solution therefore includes its
revision's size metadata. Inspection before submission runs on the caller; later
inspection runs on the solver worker and has normal contained solve-panic semantics.
Completed-run work accumulates across revisions. Daemon condition-variable waits
pause timing; applying changes, inspecting/rebuilding and solving are measured.
Existing unmeasured live jobs still expose duration but keep unknown counters/size.
A failed inspector leaves the last known revision's metadata. Retained metadata is
immutable, and receipt acknowledgement still waits for successful consumer return.

Three complete source manager adaptations now cover five real score calculations
and four move evaluations, finished two-by-two size/scale 4, and queued four-by-four
size/scale 256 changing to four-by-five/625 after consumer acknowledgement. The
native statistics tests additionally enumerate small pinned/optional/restricted list
spaces, check large exact counts, mixed bounds, timing boundaries, idle restarts,
overflow, speeds, failure, stale publishers and unknown statistics. These tests do
not establish all ProblemSizeStatisticsTest formatting/reflection behavior.

## Configurable managed threads

`SolverManager::with_thread_factory` and `LiveSolverManager::with_thread_factory`
accept a native spawn function receiving `SolverThreadSpec` and an owned
`SolverThreadTask`. Specifications identify the solver or consumer role and its
zero-based pool index. The constructor calls the factory synchronously on the
caller thread, without manager locks, in solver-then-consumer order. The factory
may borrow caller-local state and can configure names, stack sizes and wrappers.
Existing constructors retain their counts and `timetuck-{role}-{index}` names.
Live revisions and daemon restarts reuse the same threads.

A successful factory call must spawn the supplied long-lived runnable exactly
once on a new thread, promptly returning its matching JoinHandle. Running it
inline would block construction. Wrappers must allow the task to run and finish;
the manager cannot make arbitrary wrapper code cooperate. Count validation and
handle reservation precede factory calls. A returned I/O error preserves its kind
and message; an unwinding panic during the factory call becomes an Other I/O error.
Both failure paths close the pools and join every previously returned handle before
returning. A factory is responsible for any thread it starts without returning its
handle. This containment does not cover arbitrary factory-capture destruction or
process-aborting failures. Pool-thread synchronous-wait protection uses native
thread-local identity, so custom or identical names do not disable it.

The complete pinned threadFactoryIsUsed adaptation runs a real custom command
asserting the worker's configured name, followed by construction for four entities
and four values. Six original regressions cover dispatch across both pools, invalid
counts, partial failure/panic cleanup, default names, wait guards and live restarts.
These native tests add no upstream method credit or Java reflection/API claim.


## Deferred job builders and best-event origins

`SolverManager::job_builder(id, solve)` configures one submission without enqueueing
it. `with_problem` supplies an owned input; `with_problem_finder` supplies a fallible
lookup taking the ID. The last source setter wins. `with_callbacks` and
`with_problem_size` configure delivery and optional caller-computed queued metadata.
`run` consumes the builder. Missing input/finder returns MissingProblem before any
queue reservation; duplicate IDs and closed managers preserve existing SubmitError
categories through SolverJobBuildError::Submission. Dropping an unused builder
releases its captures without executing them.

Lookup runs exactly once on the solver worker after the started callback, as part
of measured solving time. Its error/panic uses normal solve-failure delivery and
prevents the solve closure from running. Rejected or queued-cancelled jobs never
look up the problem. Input and result types may differ; inputs need only Send and
neither input nor output requires Clone. Builder inputs are captured task state:
queued cancellation drops them and returns CancelledBeforeStart. The separate
submit_owned API retains its original-input result contract. Source/callback setters
use ordinary Rust drop semantics; arbitrary destructor panics are not a builder
configuration guarantee.

`SolverEventProducer` copies the native phase kind/index from a SolverUpdate.
`SolverProgress::publish_from` carries this origin with its owned snapshot, and
`on_best_event` receives both through SolverBestEvent. They are coalesced as one
record, so origin cannot come from a discarded snapshot. Legacy publish has no
origin (None); legacy on_best consumes the same snapshot while ignoring metadata.
Both best callback setters replace the previous best callback, rather than installing
two consumers. Panic disables this callback only; final/error delivery retains the
existing behavior. Stale publishers cannot update a later job reusing the same ID.
Phase metadata is explicit native information, not automatic attribution of arbitrary
closures or Java event-producer identity; scores/revisions can live in the result.

Two complete source builder methods preserve deferred lookup, callback/error routing,
non-null final/best solutions and best producer presence through real solving. Native
regressions additionally verify ownership, no eager lookup, rejection/cancellation,
lookup failures, ID reuse, metadata coalescing, legacy publication and callback panic.
Java callback variance and mocked convenience-overload delegation remain uncredited.


## Reusable native solver factories

`SolverFactory<P, R, E, S>` owns immutable SolverSequence defaults and a reusable
Send + Sync solve function. Each invocation receives an owned input, the effective
sequence and a progress publisher. It must use that sequence when calling the native
solver and construct independent working/scoring state; arbitrary user closures
cannot be forced to obey a deadline. Inputs/results/errors need no Clone, and inputs
need only Send. Cloned factories share immutable defaults and the concurrent solve
function. This factory does not provide Java reflection or XML configuration.

`build_manager` creates the ordinary native pool; `with_manager` binds an existing
pool, preserving custom thread factories and consumer counts. The configured manager
provides a job builder without requiring another solve closure. Existing manager APIs
remain available through Deref. Only configured submissions use the factory defaults;
direct closure submissions retain their original contract. Explicit shutdown joins
the pool; drop retains ordinary cancellation/detachment semantics.

Each builder starts with an independent configuration copy. Whole-limit and phase
setters replace their sections, while time/unimproved-time setters alter one field
and use None to disable it. Setter order is significant. Submission validates and
captures the effective settings in an immutable configured job handle; they remain
inspectable during queueing, failure, cancellation and ID reuse. Worker execution
copies these exact settings and binds the new job cancellation token. No original
input is cloned. Queued cancellation follows the existing captured-input builder
contract and never runs deferred lookup.

Factories and configured submissions reject empty phase vectors and any template
cancellation reference, including each phase-local variant. Cancellation belongs to
the job and is injected into its solve-wide limits. Other phase/model validation is
performed by native solving. Callback delivery, statistics, deferred lookup failures
and ID reservation reuse the existing builder/manager paths. Caller-computed model
size can be supplied before queueing.

SolverLimits::time_gradient accepts explicit elapsed and unimproved Duration values.
It reports the maximum fraction of the configured time criteria (OR), capped at one;
zero limits yield one and no time criteria yields None. It omits counts, scores,
cancellation and phase-local limits, and does not infer stagnation from wall time or
job duration. It is inspection of supplied clock evidence, not a second timer.

Two complete source manager adaptations preserve default one-second and overridden
100-ms gradients at 50 ms through actual configured solving. Native tests separately
prove that time, unimproved-time and move limits affect solver results; they also
cover defaults, phase replacement, cancellation isolation, factory failure/reuse,
queued inspection/metadata, no eager lookup, validation and gradient boundary cases.
The source unimproved override method still requires a Java static-factory interaction
and inspection of its raw override object; the native effective-settings interface
has a different contract and does not earn that method yet.


## Bounded exact phases

`ExactPhaseConfig.strategy` now accepts `ExactSearchStrategy::BranchAndBound` with
`BoundedPhaseConfig`; Enumerate remains the default. The same independent tree
engine serves basic, list and mixed models in both scoped and stream scoring.
Four exploration orders and a maximum queued-node count are configurable. Frozen
values, movable initial resets, list insertion/optional decisions and mixed
list-then-basic branching retain the standalone bounded-search contracts.

`PartialScoreBound::Unknown` is the safe default and supplies no bound.
`NonIncreasing` is an explicit caller assertion: each partial ordinary score,
with structural state cleared to zero, must dominate EVERY eligible completion
under full score ordering. This includes optional omissions, later list insertions,
coupled constraints and actual arithmetic, including integer wrapping. Negative
weights alone do not establish this property. It is not an automatic inference or
per-level initializing-score-trend implementation. A visited eligible leaf exceeding
an ancestor bound returns `SolverError::InvalidOptimisticBound` with phase index
and diagnostic score/bound strings. Pruned branches cannot validate the assertion;
optimality remains conditional on its validity. Scoring errors retain their original
typed Phase error and index; they are never converted into synthetic scores or panics.

Every successfully scored node, including the root and partial assignments, counts
as one evaluation AND one move_evaluation. Constraint work counts actual callbacks;
no local-search steps or accepted moves are added. A prior complete scored incumbent
survives handoff and participates in pruning without rescoring. Initial model values
alone are not a scored incumbent. Ties preserve the earlier complete assignment.
Partial tree nodes are never published as complete solutions or retained in place
of an earlier phase's partial construction result.

The common solve clips phase score/move/time budgets and binds cancellation/goals.
Bounded phases poll score budget, move budget, cancellation, elapsed/shared
stagnation, score target and feasibility in that order. Strict best improvements
reset shared stagnation and notify observers; late improvements cannot revive an
expired shared limit. Preparation, scoring and observations consume elapsed time.
Finishing the final leaf or proving every remaining branch dominated wins over
controls reached by that callback. FrontierLimit is a local phase stop, never an
optimality proof; later construction/custom/exact phases can finish partial work.
A following local-search phase still obeys the existing incomplete-handoff policy.

Scoped phases fully calculate every node. Stream phases use one graph per phase
and existing transactional candidate scoring, restoring empty planning rows while
retaining immutable facts. No incremental tree-scoring performance claim is made.
Configured/managed jobs copy the strategy with their phase vector and preserve
per-job overrides and cancellation ownership. Existing Exact observation and
initialization-event semantics are unchanged.

Ten original six-backend integration/regression methods, five controlled-clock
methods and a managed-job override check cover budgets, bound use and violations,
incumbents, handoffs, goals, callback errors, cancellation, exact decimal scores,
factory timing and stagnation. Standalone and legacy enumeration regressions remain
required. These native extensions earn no new upstream-method credit or dedicated
phase timing comparison.
