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
or cancellation reached by the final callback. Exhaustion of the unrestricted space
ends the plan immediately and skips future phases/factories. SolverTermination::Optimal
and is_optimal() require exhausted unrestricted exact search plus a retained complete scored solution; hard
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
method credit. Deferred scoring and full Java command API parity
remain unimplemented. Ordered integration and its source adaptations follow below.

evaluate_structured(move, valid, flawed) dispatches exactly one handler using the
trial's scored structural level: negative selects flawed. It scores the applied
move once and always rolls back. Missing required values can produce such scores;
domain, pin and dependency validation errors remain typed operation errors.
evaluate_inspected instead previews dependency flaws before installation/scoring.

evaluate_structured_with_score additionally takes an independent full scorer
returning (score, actual constraint work). It rescores the same applied assignment
without replay and requires equality with the incremental score before dispatch.
Both phase-local score slots must fit before starting; one move slot is consumed.
An invalid initial move never invokes or charges the full scorer. A full-scorer
error, mismatch or caught panic invalidates the invocation. Caught handler panics
retain known work and permit later operations after rollback. Shared composed
conditions are checked at cooperative boundaries, rather than reserving arbitrary
composed quotas atomically. The caller must supply an actual independent scorer
and accurate work count to obtain independent validation.

All four DefaultPhaseCommandContextTest methods preserve the exact success/error
strings, selected/unused handler counts, one applied move and one/two calculations.
Real native required-value structure replaces mocked InnerScore wrappers; this
does not assert Java initialized-state metadata parity. Original regressions cover
36 fresh/retained solves across all nine backends, rollback, full-scorer failures,
handler/scorer panics, unused rescores and local/shared evaluation limits.

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

`add_problem_change_with_previous` on a job or manager also lends the edit an
`Option<&LiveSolution<R>>`. This is the previous successfully returned run result,
selected when the edit executes, including a cooperatively interrupted run. It is
`None` before any run returns. It is separate from interim progress and consumer
delivery, and may contain a partial or nonoptimal assignment. All changes drained
for one restart see the same previous revision and the earlier edits to `P` in
queue order. Callers can pin retained values or routes, then reconcile that old
assignment with the edited model in the next solve. This adds no result clone,
scoring callback or `R: Clone` bound. The ordinary input-only API shares the same
queue, cancellation and receipt behavior. Edits still do not promise rollback.

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

`submit_with_progress` also supplies a borrowed `LiveSolverProgress` for the
current run. Its `publish` method queues an owned intermediate best snapshot with
the run's revision, allowing an ordinary solver observer to deliver improvements
before the solve returns. Slow consumers coalesce waiting updates. Successful
consumption acknowledges receipts only through that snapshot's revision; queued
later changes remain pending. Acceptance into the queue alone is not acknowledgement.
An acknowledged receipt stays successful even if the run later fails.

The publisher cannot outlive its solve callback. It rejects new intermediate
snapshots after the run's token is cancelled and in final-only delivery mode.
Cancellation can race an already accepted publication, whose original revision
still governs acknowledgement. Callers publish actual best snapshots; the manager
does not compare their scores. Intermediate publication does not replace the
returned result used for warm reconciliation, nor suppress its ordinary delivery,
so the returned best may be delivered again. Consumer failure retains the existing
panic containment, cancellation and receipt-failure behavior.
`submit_with_statistics_and_progress` combines this with model inspection and
actual work from the returned `LiveRun`. Publication itself adds no work counters.

`LiveSolverProgress::finish_after_run` requests successful completion after this
callback returns, including in daemon mode. It neither cancels the current token
nor preempts solving. The first request returns true; subsequent requests return
false. Requests cannot be withdrawn. New changes may still queue until the callback
returns successfully; then submissions close and queued, unapplied receipts fail
with Cancelled without another solve. Applied receipts remain delivery-gated, and
the returned result is delivered normally. A later solve error or consumer panic
retains ordinary failure behavior; settled receipts never change.

The API requires explicit model/scorer rebuilding and warm reconciliation; it does not
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
weights alone do not establish this property. It is not automatic constraint analysis. A visited eligible leaf exceeding
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


## Declared initialization trends

`PartialScoreBound::Trend(InitializingScoreTrend)` supports per-level `Any`,
`OnlyUp` and `OnlyDown` declarations. `parse("ONLY_DOWN/ANY", 2)` specifies two
levels; a single token repeats across the given count. Tokens are exact and
case-sensitive. Invalid tokens/counts return ScoreTrendError; a configured trend
whose count differs from the score type returns typed SolverError::InvalidScoreTrend
with its phase before any score callback. A stream factory may already have run.
Zero-level score families accept an empty trend. Configuration now owns its trend
vector and is Clone, not Copy.

Optimistic bounds retain the partial value on OnlyDown levels and use positive
infinity otherwise. Pessimistic bounds retain OnlyUp levels and use negative
infinity otherwise. InitializingScoreBound stores the finite prefix and the first
infinite boundary: suffixes beyond it do not distinguish sets of reachable finite
scores. Bounds compare exact numeric levels, including decimals, without float
conversion or finite sentinels. Structural state is cleared to zero when deriving
bounds; comparison still considers structural state first. These derived bounds
require structural-first, numeric lexicographic Score ordering. All built-in
score families provide this ordering. A custom Score must honor that contract.

The ordered engine uses upper bounds for scheduling, inherited path minima, pruning
and visited-leaf validation. A known hard prefix can prune a worse hard branch
while unknown softer levels stay open. All-Any is safe and retains full exploration;
all-OnlyDown agrees with the existing NonIncreasing mode. Unknown suffixes do not
turn partial scores into retained solutions. Derived lower bounds are exposed for
reasoning, but never become incumbents without actual scored witness assignments.

Declarations must hold for every eligible completion under the actual arithmetic,
including omissions, changed list neighbors and wrapping integers. The API does
not infer declarations from weights and cannot validate pruned branches. Visited
leaf checks validate the inherited full optimistic bound, not every independent
trend assertion; OnlyUp/Any levels may make some assertions unobservable. Original
checks cover all six paths/four orders, every unknown-budget prefix, decimal
precision, invalid widths and independent two/three-level bound references.

## Phase-local diminished returns

LocalSearchConfig::diminished_returns works in every ordered local phase for all
six model/scoring paths. It starts after phase initialization and ends only that
phase; SolverLimits has no shared rate-history control. Phase reports retain the
DiminishedReturns reason, and later phases may run or prove optimality. Stream
factory preparation is excluded from this grace period while remaining part of
any shared elapsed-time budget. Existing shared stop reasons retain precedence.
See the incremental-search contract for the exact rational-rate and reset rules.

`StatefulSearchLimit::DiminishedReturns` also supports independent rate windows
inside AND/OR expressions bound to local-search phases. A validated
`PhaseDiminishedReturnsConfig` uses its score prototype only to check shape and
arithmetic. Grace starts at the first condition poll after a complete search-score
observation, using the retained best then available and excluding setup. Every window is
polled even when another expression branch decides the result. Strict retained
best improvements update history; worsening and structurally incomplete scores
do not. Expiry latches until a new phase/session reset. Progress is unknown before
expiry, so cooling needs another leaf with a useful progress estimate.

Solve-wide and non-local-phase bindings fail before runtime callbacks, including
under zero work budgets. Each phase gets fresh history, and an expression ends
that phase with the ordinary condition reason. The standalone additive-score
constructor remains available; typed phase configurations also support suitable
Score-only types. Native checks cover all eighteen fresh/retained solver routes;
this composition capability adds no upstream method credit.

## Pooled construction phases

Construction phases preserve ConstructionConfig::placement, including through
stream factories and per-job plan cloning. Pooled placement makes all eligible
missing items compete for a step; mixed phases combine both model components.
Shared score/candidate/time/cancellation controls still constrain these phases,
and subsequent phases receive the selected assignment and accumulated work.
A completed greedy phase is not an optimality proof. See the construction section
of the incremental-search contract for candidate order and optional-value rules.

## Significant-improvement phase limit

Local phases may independently require a minimum retained-best score improvement
within an elapsed-time window through LocalSearchConfig::significant_improvement.
Each phase arms after initialization, including stream preparation. Its typed
SignificantImprovementTimeLimit reason ends only that phase; later phases may run
and establish optimality. Existing shared controls remain authoritative. There is
no corresponding solve-wide score-difference history. See the incremental-search
contract for exact threshold arithmetic, prefix comparisons and deadline semantics.

## Value-first construction phases

Construction phases may select ConstructionPlacement::ValueQueue. Basic values cycle
in stable domain encounter order while each scan chooses the best missing receiver.
List phases retain sequential insertion; mixed phases honor component order. Every
phase starts a fresh queue, preserves existing assignments, and uses the same shared
budgets and observation pipeline as other construction modes. No optimality proof is
implied by completing the queue. See the incremental-search contract for optional
retries, stopping, runtime comparisons and evidence boundaries.


## Ruin-and-recreate phases

`SolverPhase::RuinRecreate` runs basic or list repair; the corresponding
`MixedSolverPhase` retains mixed construction order. Both scoped and stream plans
support them, including implicit initial construction through SolverPlan and
explicit ordering through SolverSequence. Repair phases start from retained complete
best, with a fresh acceptor and configured seed. Required incomplete handoffs obey
IncompleteHandoffPolicy; the existing IncompleteLocalSearch error identifies the
following improvement phase, including repair. No partial reconstruction replaces a
complete incumbent. Completing a repair phase never proves optimality.

Repair total-score and candidate budgets are clipped to solve-wide remaining work.
Initial scores for the phase and each repair count as scores but not candidates.
Each phase reports RuinRecreate termination and SolverWork; completed_steps and
accepted_moves both count accepted *changed* complete repairs or enabled fine moves. Rejected and unchanged
attempts do not count as completed steps, while each started reconstruction consumes
its own initial score budget. Standalone repair results additionally expose attempt
and complete-reconstruction counts. A local attempt limit can hand off to another
phase; solve-wide score/candidate/cancellation/time/goal controls remain authoritative.

Shared stagnation starts after initial repair scoring, before the initial callback.
Strict retained-best improvements reset it only before expiry. Accepted changed
repairs and fine moves advance its step count, including equal or worsening acceptance; no-op or
rejected attempts do not. History survives local/repair phase transitions. Shared
stagnation is checked at reconstruction boundaries; active reconstruction cannot be
preempted by the monitor. Existing reconstruction time/cancellation/evaluation
checks still apply internally. Shared stagnation follows cancellation and elapsed
phase time but precedes remaining budget/attempt/score checks at repair boundaries.

Repair validation checks every configured ruin range, move weights and acceptance policy before
any phase scoring or factory. Cancellation-token conflicts are rejected up front.
Runtime model/scoring/factory errors use SolverError::Repair with a phase index and
typed RuinRecreateError; this avoids recursively nesting native local-search errors.
Stream repair phases build graphs inside the repair API: one for initialization and
one per started repair or fine score, including factory time in the cooperative budget. Zero score
budget or pre-cancellation invokes no repair graph factory.

Retained-best notifications carry SolverPhaseKind::RuinRecreate and are filtered
across phase boundaries using the existing score/completeness rules. FirstInitialized
is emitted after the phase before the first local *or repair* phase; it does not fire
for a repair-only sequence lacking a preceding phase. Callback time and cancellation
remain part of solve controls. Managed factories preserve repair configurations,
reject template cancellation tokens, and apply independent per-job limits. Published
best snapshots carry the actual repair origin through existing consumer callbacks.

Sixteen original regressions cover all six model/backend paths, 2,016 independent
shared-budget combinations with quality references, incomplete/exact/construction
handoffs, mixed order, goals, cancellation, zero limits, stagnation across phases,
factory errors, initialization/best events, owned decimal targets and managed jobs.
The existing six-path solver test fixture is shared between legacy and repair tests.
No upstream method credit is added. Java repair quality/timing comparisons remain
separate pending evidence; the standalone reconstruction contracts still apply.

## Repair work reporting

`SolverWork.repair_attempts` counts started reconstructions, including incomplete,
rejected and unchanged attempts. `complete_reconstructions` counts those producing
a complete scored assignment, regardless of acceptance or best retention. Neither
includes phase initialization. Completeness does not require exhausting every
candidate: a final-slot candidate or optional partial reconstruction may already
be a complete solution when a budget expires. Failed phases return typed errors,
not a fabricated successful work report.

These counters survive all six phase adapters, phase aggregation and managed-job
publication. Non-repair phases contribute zero. They do not change existing
`move_evaluations` (individual reconstruction candidates and enabled fine scores), accepted moves, completed
steps or shared termination limits. Managed statistics reject decreasing absolute
counts and saturate additive overflow with `work_overflowed`, as for other work.
Original six-path budget-prefix, complete/no-op/rejection and managed monotonicity/
overflow tests verify the distinction. Public Community basic-repair quality and
performance evidence is now available separately in docs/dashboard.md.

## Weighted fine moves inside repair phases

`RuinRecreateConfig.move_weights` optionally interleaves ordinary changes, swaps
and repairs. Positive integer weights select a family (2:2:1 gives 40/40/20 when
all are available); zero disables it. All-zero weights fail validation before any
phase scoring. Unavailable families are removed until the next attempt and weights
renormalized. A fine family is sampled uniformly from its iterator, retaining
multiplicity of equivalent successor states. Reservoir sampling stores one move
but scans the entire chosen family. Cancellation, elapsed time and shared stagnation
are checked between iterator calls; user callbacks and iterator calls are cooperative.
Default 0:0:1 preserves repair-only behavior and its previous random sequence.

Basic fine moves use existing change/swap providers. Lists use relocation, optional
assignment/unassignment, swaps and optional replacements. Mixed fine moves change
one component, enumerating basic then list providers. Existing pins, prefixes and
ranges remain authoritative. All attempts share the same current solution, best,
acceptor metadata/history and phase seed. Accepted changed fine moves also advance
completed steps and shared stagnation. Improvements retain the same notifications.

`max_attempts` caps started repairs plus selected fine moves. Standalone `attempts`
and plan `repair_attempts` continue to count only reconstructions;
`fine_move_attempts` records selected fine attempts including interrupted scoring.
`fine_move_evaluations` records actual fine scores, charged against both total and
candidate budgets. Each fine score initializes a fresh scoring cache/graph, including
factory time and enabled diagnostics. This is a correctness baseline with allocation
and initialization cost, not an incremental fine-search performance claim.
`NoAvailableMoves` ends a phase whose enabled families are all empty.

Phase aggregation and managed statistics preserve fine counters, reject decreasing
absolute updates and report saturating-add overflow. Native reference/budget/history/
notification/decimal/cancellation/managed checks cover all six paths; adapted list
selector methods add only the documented source-method assertions. Existing Java
repair timings remain repair-only and do not measure this weighted mode.

## Composed solve-wide termination

`SolverLimits::termination` accepts a borrowed `SearchTermination` tree: `All`,
`Any`, or `Condition(SearchLimit)`. Conditions currently cover completed search
steps, construction steps, combined construction/search steps, unimproved search
steps, elapsed solve time and unimproved search time. Phase changes do not reset
counters. `Steps` retains its local/repair semantics; `ConstructionSteps` counts
construction placements and `CombinedSteps` sums both with saturation. Custom
commands and exact candidates are not steps. Construction is excluded from
stagnation counts. Strict improvements reset stagnation until
the whole expression becomes true; individual children of AND are not latched.
The reached expression stays latched, preserving late improvements in the result.

Expressions are polled at phase boundaries, within local search, between repair
attempts, between exact-search candidates/nodes, and by custom-command checks.
Construction polls between candidates and retains the best evaluated prefix,
reporting `ConstructionTermination::Condition`. It does not advance search steps
or start/reset stagnation tracking. Each completed placement advances the separate
construction counter, including a scored no-change placement or retained interrupted
prefix. A discarded incomplete scan or empty candidate pool does not count. One
provider composite is one placement even when it initializes several items. Phase
and solve `SolverWork::construction_steps` expose this work independently of
`completed_steps`; managed job statistics accumulate it and detect overflow or
regressing absolute updates. Elapsed time includes setup/scoring;
active callbacks are cooperative and may overshoot. Existing hard budgets and
cancellation remain independent. `SolverTermination::Condition` identifies this
stop; phase reports retain the corresponding phase reason. Full exact exhaustion
still proves optimality when it coincides with a condition becoming true.

For example, limit the complete construction/search sequence to 100 placements
and search steps combined:

```rust
let condition = SearchTermination::Condition(SearchLimit::CombinedSteps(100));
let config = SolverConfig::<SimpleScore> {
    limits: SolverLimits { termination: Some(&condition), ..Default::default() },
    ..Default::default()
};
let result = solve(&problem, &network, config)?;
let construction_steps = result.work.construction_steps;
let search_steps = result.work.completed_steps;
```

Use `ConstructionSteps(100)` to count only initialization work. This is still a
solve-wide stop: reaching the construction threshold ends the entire solve,
potentially with a partial result.
Use `ConstructionConfig::max_steps` to stop only one construction phase and then
continue with the next phase.

`TerminationExpression<C>::evaluate` also supports caller-defined conditions and
scope applicability. Inapplicable leaves return None; unsupported progress estimates
are independent of exact boolean decisions. AND reports minimum supported progress
(default 1), OR maximum (default 0). Empty composites use logical identities. Native
`SearchLimit` snapshots before search starts mark stagnation as unreached with no
estimate. Rounded progress never decides whether an integer threshold is reached.

## Retained stream score explanations

`StreamGraph::explain_constraint_default` returns an owned snapshot with ordered
fact justifications, exact match scores and deduplicated per-fact indictments.
`explain_constraint` accepts a custom justification and independently selected
blame facts. Both read retained terminal matches without evaluating score callbacks;
explanation callbacks run only on request. Basic/list/mixed stream directors expose
the graph, including transactional candidate views. Scoped directors expose aggregate
contribution justifications through the APIs described below.

Equal payloads at distinct tuple IDs remain distinct matches. Repeated blame facts
within one match count once; matches blaming multiple facts contribute their full
score to each, so indictment totals need not sum to the graph score. Zero-score and
empty-blame matches remain visible. Snapshots survive later commits and rollbacks.
`ScoreExplanation::add_constraint` aggregates selected reports, qualifies match IDs
by constraint name, and rejects duplicate names. `into_indexed` allows aggregation
of different tuple-ID types while returning every original ID in index order.
Take component snapshots from one graph state; the aggregate covers only supplied
constraints and excludes structural penalties maintained outside the graph.

Twenty-three upstream uni-stream scoring/default/custom-justification methods
preserve total scores, constraint keys, match counts and ordered fact assertions.
Native signed terminal closures replace Java scoring-builder syntax, and on-demand
typed snapshots replace Java justification-policy/director retrieval. No Java
reflection, API, or disabled-justification lifecycle parity is claimed.

## Configurable stream constraint weights

`StreamGraphBuilder::score_weighted` multiplies an ordinary score weight by an
integer match factor; positive factors reward and negative factors penalize.
Scaling uses score arithmetic without floating-point conversion; built-in integer
impacts are checked by default and decimal coefficients retain arbitrary precision. Factors include
the full i64 range. Signed ordinary weights are permitted by the native API;
structural weight components are rejected. Fractional decimal weights work with
integer factors; use `score_weighted_decimal` for fractional match factors.

`ConstraintWeightOverrides` distinguishes missing names from explicit zero and
iterates in sorted name order. `build_with_weights` resolves overrides once before
source initialization, rejecting unknown names, overrides of ordinary unweighted
terminals and structural weights. Unknown-name diagnostics include all registered
names. Rebuild the graph for another weight configuration or live solve revision;
existing graphs retain their copied weights.

Zero-weight terminals are disconnected, along with branches feeding only disabled
constraints. Shared ancestors of active constraints continue to execute. Source
rows remain available, and disabled handles report zero score and empty matches.
Ordinary `score` closures returning zero are still evaluated. Weighted matches
participate in existing transactions, score consistency checks, candidate caching
and the explanation API. Basic/list/mixed stream solves use the same graph logic.
Scoped network weight configuration is described below.

Three weight-override source methods plus uni-stream zero-weight and duplicate-name
methods are adapted. Java solution reflection and resetting an existing director
are replaced by explicit native maps and graph reconstruction; inherited-solution
annotation rejection remains pending. Independent regressions cover pruning,
shared branches, dead joins, frozen overrides, rejected configuration, arithmetic,
rollback, explanations and optimum changes across all three stream solve paths.


## Exact fractional match impacts

`score_weighted_decimal` supports all four native decimal score families through
`DecimalWeightedScore`, including bendable scores of arbitrary dimensions. Each
level multiplies the weight and match-factor coefficients and adds their scales.
It does not floor at the old weight scale, unlike the existing decimal score
`multiply` operation. `Decimal::multiply_exact` also exposes the scalar operation.
Structural components are cleared by score arithmetic; graph weights still reject
structural state. Native zero normalization follows each score family's existing
rules. Scale sums outside i32 return ScaleOverflow, including zero products.

Integer and decimal weighted terminals share override resolution, disabled-branch
pruning and retained match accounting. An arithmetic failure identifies the
constraint and carries its underlying error. The current graph transaction becomes
unusable; dropping it restores every affected source, operator and terminal, including
updates routed before the error. Explanations retain the exact weighted contribution.

Sixteen complete decimal ScoreInliner methods preserve empty scores and all impact/
undo states. Native source occurrences and retractions replace Java impact handles;
no Java object/match-policy lifecycle equivalence is claimed. Separate regressions
cover genuinely fractional products, long coefficients, scale boundaries, rollback,
overrides, pruning and exact optimum selection in all three stream solve paths.


## Configurable weights in scoped networks

Basic, list and mixed scoped networks expose `add_weighted_constraint` for signed
integer factors and `add_decimal_weighted_constraint` for exact decimal factors.
They share arithmetic with stream weighting. `apply_weight_overrides` validates
all supplied names and structural components before applying any changes. Missing
names retain their current weights; ordinary direct-score constraints cannot be
reweighted. Invalid registration does not alter the network. `constraint_weights`
returns configured weights, including zero, in registration order.

Weights may change between scoring sessions. A director borrows its network,
preventing weight changes while its contribution cache exists. Explicit zero skips
fact assembly and factor callbacks during initialization, full calculation,
diagnostics and moves. Disabled constraints stay visible as zero contributions but
are excluded from candidate callback counts and exact-solver callback work totals.
A later nonzero override reactivates the declared dependencies. Structural missing
assignment penalties remain active even when all weighted constraints are disabled.

Decimal scale errors identify the constraint index and expose their underlying
cause. Errors and ordinary panic unwinding restore candidate model values and
leave every contribution cache unchanged. Constants, repeated scopes, deterministic
callbacks and model validation retain their existing scoped semantics.

Sixteen integer ScoreInliner methods are adapted, preserving empty scores and all
impact/undo score assertions. Optional native variable slots represent independent
impacts. The eight upstream integer overflow methods now also have complete adaptations
through the default checked scoring policy described below.
Original regressions cover all three networks, atomic invalid overrides, disabling
and reactivation, callback counts, decimal failure/panic rollback and fractional
optima under both exact solving and construction/local search. These original
regressions add no upstream method credit.


## Scoped explanations and snapshot differences

`ScoreDirector`, `ListScoreDirector` and `MixedScoreDirector` expose `explain` and
`explain_default`. Callbacks receive the constraint name, declared scope in order,
current scoped facts and the retained weighted score. They do not invoke scoring,
weight-factor or graph callbacks. Each active scoped constraint produces one
aggregate match (ID zero); the API cannot infer inner matches from an opaque
aggregate callback. Zero-weight disabled constraints remain visible with no matches
and never invoke the explanation callback. Ordinary zero contributions retain their
match. Custom justification data and blamed fact identities are independent.
Default justifications preserve fact order/repetitions; indictment scores count each
fact only once per match and aggregate across constraints by qualified identity.

`ScopedScoreExplanation.score` preserves the director's structural component.
Its `matches` member contains the ordinary contribution report, whose score excludes
structural incompleteness. Thus a model initialization change can change the full
score while leaving the contribution report unchanged. Owned reports remain valid
across later moves, rejected trials, reweighting and destruction of the director.
An explanation callback panic leaves director values and caches unchanged.

`current.diff(&baseline)` on `ScoreExplanation` returns current-minus-baseline
ordinary score deltas, changed constraints and changed indictments. Each entry
retains its before/after snapshot; `None` distinguishes addition/removal from an
existing zero-score entry. Changes to justifications, blame or match occurrence
identity are retained even when the score delta is zero. Unchanged entries are
omitted. Match IDs must mean the same thing in both reports; separately indexed
heterogeneous stream reports must align original identities before comparison.
The operation clones changed snapshots and performs no rescoring.

One complete upstream ConstraintMatchTotal method preserves the initial zero and
all six impact/removal totals. Native string fact identities and graph occurrences
replace Java entities and match handles; Java equality/hash/order methods remain
pending. Seven original scoped/difference regressions cover all three model types,
callback counts/panics, disabled and empty constraints, structural state, exact
fractions, zero-delta changes, rollback and owned snapshot independence. They add
no upstream method credit.


## Checked solver scoring

Stream graphs and all three scoped networks now default to `ScoringArithmetic::Checked`.
Built-in integer score families reject unrepresentable weighted impacts, constraint
match totals and complete graph/network totals with a typed ScoreOverflow. This
covers direct-score callbacks too when their returned contributions overflow a
managed total. Arithmetic performed inside a caller's callback or a collector keeps
that callback/collector's own contract. Score value operators themselves still wrap.
`with_arithmetic(ScoringArithmetic::Wrapping)` explicitly retains modular scoring.
Configure the policy before building a graph or borrowing a network in a director.

Integer sum and adjustment hooks use i128 intermediates and validate final i64 levels.
Full scoped scoring accepts cancellation across any constraint order. Scoped moves
check the complete batch of changed contributions, and graph totals check a complete
routed source update. Individual stream terminal match changes are checked as they
occur. A native replacement does not fail solely because subtracting the old value
before adding its replacement would temporarily exceed i64. Structural components
remain separately managed; invalid weights/contributions retain their prior errors.

Overflow poisons graph trials until drop; dropping or failed commit restores all
sources, operators, matches and totals. Scoped failures restore candidate model values
and leave caches unchanged. Constructor/full-calculation failures propagate normally;
all six solving paths propagate scoring overflow instead of accepting wrapped quality.
Graph diagnostics may replay rows in another order: accumulation is modular during
replay while weighted impacts remain checked, and every retained match score and total
must still equal the original snapshot. This avoids false replay-prefix failures
without masking an overflowing impact whose modular value happens to match.

`AdditiveScore` provides checked_scale/checked_sum/checked_adjust hooks. Built-in
integers implement bounds checks; arbitrary-precision decimal scores retain their
existing arithmetic. Defaults retain a custom score's ordinary operators, so bounded
custom scores must override these hooks for overflow detection.

All eight integer ScoreInliner overflow methods preserve weights, factors, the first
successful impact when present, and rejection. Seven original regressions cover
wide arithmetic boundaries, terminal removal and global overflow rollback, replay
order, modular collisions during diagnostics, all scoped models, and exact/construction
error propagation through all six solving paths. Existing modular regressions select
Wrapping explicitly. No release-mode or timing rerun is needed for these explicit
checked/wrapping operations; benchmark datasets remain stale.

## Models without movable entities

After the existing entry validation, ordinary solve plans skip their configured
phases when all basic variables and list entities are absent or fully pinned.
Plans containing an explicit exact-search phase still run normally, preserving
their one-candidate empty/fixed search space and exhaustion/optimality contract.
A mixed model skips only when both components meet the frozen condition.
A prefix-pinned list is still movable: it can accept appended values, so a full
current prefix must not trigger the shortcut. Other causes of an empty move
neighborhood (ranges or scores) do not trigger this structural check.

A complete initial assignment is scored once using the ordinary score director,
without invoking user commands or move providers. A completed initial score records
one evaluation and zero moves/steps; phases is empty. Stream factories run once to
prepare or validate the binding. For complete models, zero evaluation budgets and
early cancellation skip the factory too; impossible models still validate bindings.
The best update has native kind Initial and index zero. No FirstInitialized event
is emitted because no construction/custom/exact phase ran. Initial scoring and
graph/source-validation failures use SolverError::InitialScoring. This changes the
origin and work counts for empty and fully pinned solves, while movable models
retain their prior phase behavior.

Global evaluation/cancellation/expression gates can prevent scoring entirely.
Other global limits and score targets retain the director kernel's precedence,
including one initialization score with a zero candidate budget. Phase-local
limits are not entered. Complete unrestricted results end with PlanCompleted;
incomplete frozen lists validate their score binding and end with
NoCompleteAssignment without scoring. This impossibility check precedes evaluation
budget/cancellation gates, retaining native precedence and zero work. No new
optimality claim is made by the shortcut. Empty plans, missing command
bindings and invalid provider bindings still fail before the shortcut. Existing
entry checks for cancellation consistency and local-search policies also run;
phase-specific checks inside skipped phases are not entered.

Three complete DefaultSolverTest methods retain empty/pinned fixtures, failing
command non-invocation, score assertions and initial-best event behavior. Native
Initial replaces the Java solvingStarted event origin. Native variable-owned
domains have no separate unused global registry for the empty-entity fixture.
Nine original checks cover all six backends, empty and pinned models, mixed
component mobility, budgets/cancellation/targets, missing required list values,
prefix appending, provider non-invocation, plan errors and initialization failures.

## Indexed value ranges

ValueRange is an immutable, object-safe sequence of value occurrences with u128
counts, indexed access and membership. Implementations must keep their order/count
stable and provide every in-range value. Iterators traverse lazily in either
direction; nth/nth_back skip directly by rank. Counts exceeding usize remain valid,
so iterators do not implement ExactSizeIterator. Random iterators select uniform
occurrence ranks with replacement; empty ranges terminate without drawing. Injected
rank sources permit deterministic replay and panic on an out-of-bound rank. Native
seeds are reproducible, without Java PRNG parity.

IntegerValueRange uses a signed i64 half-open interval and a positive u64 step,
with widened arithmetic for both endpoints. A nondividing step includes the final
progression member below the upper endpoint; no Java validation parity is claimed
for such bounds. EnumeratedValueRange retains ordered duplicate occurrences in
shared immutable storage. BooleanValueRange orders false then true. CompositeValueRange
concatenates heterogeneous child ranges using checked prefix counts, preserving
empty children and duplicates. OptionalValueRange prefixes one None, with checked
count growth; nested wrappers preserve distinct Option layers.

materialize and sorted_by require explicit element limits before reading values.
They are not byte limits or allocation guarantees. Sorting creates a stable copy;
sorted_present_by on optional ranges preserves None first even when descending.
Ordinary indexed arithmetic/boolean ranges also support a bounded sorted copy;
Java unsupported-sort exception behavior is not claimed. Out-of-range indices yield
None. Negative indices do not exist in this native API. contains_assignment(None)
rejects an absent assignment; an optional range member is explicitly Some(&None).

variable_from_range makes bounded conversion into the existing finite solver
explicit. It does not turn the solver into a lazy-domain engine. Duplicate values
remain for FiniteProblem's ordinary validation to reject. An original exact-solve
case verifies a combined arithmetic/explicit domain and an independently calculated
optimum. Large-range support does not imply a new end-to-end performance result.

## Common destinations for variable groups

CommonValueRanges retains immutable shared range handles, deduplicating Arc
identities without scanning values. Equality ignores handle order but preserves
identity membership; equal contents in separate allocations do not imply equality.
The first smallest range supplies candidate occurrences. for_variables borrows
selected domains, deduplicating content-equal ordered slices and rejecting empty,
repeated or invalid member IDs. from_shared_with performs one explicit fetch for a
caller-established solution-wide range. Optional variables contribute assigned
values only; absent destinations are rejected.

pick_exact uses reservoir sampling over admissible occurrences of the smallest
range, applying every domain and the optional excluded value. Distinct destinations
are uniform for a unique source; duplicate source occurrences preserve their weight.
A caller scan limit is checked before reading source values. find_target first
performs bounded rejection draws, then the same exact scan. Its probe budget is
capped by both the caller limit and a saturating ten-times-source-count bound.
A ScanLimit error means search was inconclusive; Ok(None) means the admissible
intersection was proven empty. A zero-size source needs neither probes nor reads.
Native default limits are 64 random draws and 100000 exact source entries.
Callbacks remain cooperative; these are operation bounds, not wall-clock deadlines.

sample_common_change builds one ordinary Change or Composite for a selected group,
including groups with different or unassigned starting values. It excludes a shared
current value, rejects pinned members, checks model/domain/member validity, and
preserves caller member order. It proposes assigned destinations only; existing
unassignment neighborhoods handle null destinations separately. Generate it anew
for each provider snapshot and let the existing model/score-director transaction
handle evaluation, acceptance and rollback. An integration case escapes a local
optimum requiring simultaneous changes through scoped and stream scoring.

## Exact numeric ranges

BigIntegerValueRange stores arbitrary-precision start/end/step values and a checked
u128 occurrence count. The default step is one; explicit steps must be positive.
Bounds are half-open and ascending (equal endpoints form an empty range). A step
need not divide the interval: the final included member is strictly below the end.
Counts larger than u128 are rejected before publishing a range. Lookup, membership,
iteration and random sampling use exact integer arithmetic without materialization.

DecimalValueRange indexes BigInt coefficients at one fixed Decimal scale. The
default step is one coefficient unit at that scale, including negative scales.
Both endpoints and any explicit step must share their representation scale; a
mismatch is rejected before alignment/padding. Membership includes scale equality,
consistent with native Decimal Eq. Thus 1.0 and 1.00 remain distinct domain members.
Neither range uses floating-point arithmetic. Existing ValueRange combinators,
common-value selection, bounded materialization and variable_from_range work with
both. The solver itself still uses explicitly materialized finite domains.

Original checks include thousand-digit offsets, huge strides, maximum counts,
negative and extreme i32 scales, independent small progressions and a two-variable
exact decimal optimization with optimum0.06. No new performance measurement or
Java mismatched-scale/unsupported-sort behavior is claimed by this increment.


## Construction providers in complete plans

`SolverExtensions.move_providers.bind(index, provider)` now accepts construction
as well as local-search phases in all six solver entry points. Bindings to exact,
custom, repair or missing phases are rejected before factories, scores or events.
Construction providers replace built-in placement and mixed component ordering
with their candidate pools; the existing construction config still controls
optional assignments, early picking, score/move/step/time limits and assertions.
`max_construction_scans` bounds raw proposals independently in every bound
construction phase (default 100,000); it does not change local-search providers.
Provider state persists across phases/solves, while step indices restart at zero.

These phases use the same assignment-preservation and validation rules as the
standalone provider constructor. They emit the initial scored state and committed
placements through the plan's existing best/initialization event machinery, never
trial candidates. Shared score/move budgets include provider work. Cancellation and
composed conditions keep the evaluated-prefix winner. Construction does not advance
search step counters or start/reset stagnation. `NoCandidates` and `ScanLimit` remain
honest phase-local reasons; neither proves infeasibility. A following construction
or exact phase can complete partial work. Local/repair handoffs retain the configured
incomplete-handoff policy. Graph factories remain subject to the existing validation
contract: an incomplete initial model can prepare a graph at zero score budget,
but no scoring, provider or event callback runs.

Original tests exercise partial and complete handoffs, two provider phases with
reset step indices, shared budgets, cancellation from events, scan limits, exact
completion after empty pools, initialization boundaries, and validation across all
six paths. A controlled shared-condition test checks prefix retention and excludes
search-only control calls. These tests add no upstream method credit; provider
SelectionSource lifecycle events still need explicit wiring by callers.


## Pinned-variable startup diagnostics

Explicit basic and mixed local-search starts now report
`PinnedUnassignedVariable { variable }` when a required pinned basic slot is unset.
The first such slot in model order is reported, even if an earlier movable slot is
also missing. Mixed indexes identify basic variables, independently of list-owner
indexes. Optional pinned None values remain valid. Model validation keeps precedence,
so malformed lengths, values and changed pins retain their existing model error.
The check precedes scoring/provider execution and applies at zero evaluation budget.

Both scoped and stream backends use the same startup check. A local-search phase in
a solver sequence wraps the error with its phase index. Other incomplete explicit
starts retain IncompleteInitialAssignment; construction handoff policy, frozen-model
shortcuts, automatic initial assignment selection and partial exact/custom results
retain their prior contracts. This diagnostic identifies native slots; callers map
slots to application entity/variable names. It does not reproduce Java reflection or
exception text. The remaining DefaultLocalSearchPhaseTest pinned-null method now
checks that slot2 identifies e3.value on both basic scoring paths; original cases
cover both mixed paths, multiple missing pins, optional pins, model-error priority,
zero budgets and no scoring/provider calls.

## Completing an existing assignment

`ExactSearchStrategy::Complete` explores extensions of the assignment entering
that phase. It retains assigned basic values and existing list owners and relative
order. Missing list members may be inserted before, between or after existing
members when pins, ranges and dependencies permit; optional members may stay
absent. Optional basic variables already holding values also stay fixed.
The existing model preparation for dependency inconsistencies occurs before phases.
Standalone `visit_basic_completions`, `visit_list_completions` and
`visit_mixed_completions` instead validate the supplied state without repairing it.
Visitors return true on exhaustion and false on callback stop, never changing input.

Completion uses the same scoring, budget, cancellation and event machinery as
ordinary exact enumeration across basic, list and mixed models with scoped,
stream and calculator scoring. `CompletionExhausted` describes only the restricted
extension space. It neither claims global optimality nor prevents later phases
from improving existing values. Even an empty extension space cannot prove the
whole model impossible. Ordinary `Enumerate` and branch-and-bound retain their
unrestricted semantics. Exhaustion on the last permitted evaluation wins over an
evaluation limit; a zero budget scores nothing in a nonempty extension space.

## Ordered set value ranges

`SetValueRange` retains the first representative of each equal value in encounter
order. A hash index supports expected constant-time membership and reverse rank
lookup; indexed reads also take constant time. Traversal and seeded sampling use
the ordered storage, never hash iteration. Sampling gives each distinct member one
rank, while `EnumeratedValueRange` continues to weight duplicate occurrences.

Construction takes ownership without cloning payloads. Cloned snapshots, union,
intersection, difference and stable `reordered_by` results share individual values.
Borrowed `values` and `get_ref` also avoid payload clones, including for non-Clone
values. Owned `ValueRange` access requires Clone as usual. Union keeps left members
followed by unseen right members; intersection and difference retain left order and
representatives. Reordering rebuilds the reverse index and leaves old snapshots
unchanged, including on comparator panic. Equality/hashing must stay stable.

The existing bounded `variable_from_range` feeds these distinct domains into finite
models. Set operations do not make the solver a lazy-domain engine. The inherited
bounded `sorted_by` creates an enumerated copy; `reordered_by` instead retains the
set index and shares payloads, operating only on members already in storage.
No end-to-end runtime improvement is claimed without a benchmark.

## Typed score definitions

`ScoreDefinition` adds typed construction from a vector of ordinary levels,
level labels, total/feasibility level counts, zero and the softest unit score to
all eight integer and decimal families. Bendable dimensions remain part of the
Rust type. A wrong number of levels returns `ScoreDefinitionError::LevelCount`.
The unit occupies the last ordinary level, including the last hard level when
there are no soft levels; a zero-level shape returns `NoLevels`.

`divide_by_sanitized_divisor` divides corresponding levels, treating each numeric
zero divisor as one. Integer quotients use exact widened arithmetic and round
down, including negative operands; MIN/-1 returns an indexed overflow error.
Decimal quotients use exact arithmetic and floor at the dividend's scale clamped
to zero. Scale overflow remains an error. These constructors and arithmetic clear
structural state and leave their inputs unchanged. Generic callers can construct
scores for ordinary solver runs without score-specific constructor branching.

`finite_optimistic_bound` and `finite_pessimistic_bound` are available only for
integer score levels. They validate the trend shape and independently retain
ONLY_DOWN or ONLY_UP components, respectively, filling other components with
integer MAX or MIN. These finite extrema do not replace the search engine's
existing infinity-based `InitializingScoreBound`. Callers remain responsible for
valid trend declarations across every completion, including overflow behavior;
a pessimistic bound alone is not a feasible incumbent. Decimal score definitions
continue to use the existing infinity-based trend API when bounds are needed.

## Automatic phase score checks

Set `SolverLimits::assert_phase_scores` to recalculate every scored phase output
using a fresh scoring backend before handoff and initialization notification. The
default is false. Basic, list and mixed plans support this for scoped networks,
stream factories and incremental calculator factories, across construction, local,
repair, custom and exact phases. Immovable initial-only solves are also checked.
Unscored outputs are skipped, including zero-score-budget phases.

```rust
let config = SolverConfig::<SimpleScore> {
    limits: SolverLimits { assert_phase_scores: true, ..Default::default() },
    ..Default::default()
};
let result = solve(&problem, &network, config)?;
```

Fresh scoring uses an owned snapshot and the backend's initial scoring kernel;
it does not run custom commands, reopen providers, generate moves, or publish new
progress/initialization events. Diagnostic work is excluded from phase/solve work
counters and score/move/step budgets. A completed scored phase is checked even if
its search budget or cancellation has stopped further search. Diagnostic elapsed
time belongs to the shared deadline; checks cannot preempt slow user callbacks.
Existing best events emitted inside a phase cannot be retracted after a failure.

`PhaseScoreMismatch` reports the original phase index/kind and actual/expected
scores. Fresh backend scoring/factory errors preserve their underlying errors as
Phase or InitialScoring failures; panics propagate. A successful fresh check then
runs any separately configured `SolverExtensions::score_check`, allowing a second,
independent objective implementation. Rebuilding the same scorer detects incremental
state corruption and nonrepeatable objectives, but cannot detect a deterministic
mistake shared by both calculations. Stream/calculator factories must build equivalent
objectives each time and keep independent mutable state.

This is a native phase-boundary diagnostic, not an implementation of Timefold's
complete environment-mode policy set, shadow-event tracking or phase factory caches.
It does not replace per-candidate `assert_scores` or independent expected-score checks.

## Rejecting required unassigned pins at startup

Set `SolverLimits::reject_unassigned_pins` to fail before phase execution when a
required basic variable is both pinned and missing. The error is
`SolverError::PinnedUnassignedVariable { variable }`, using the basic slot index
also for mixed models. This validation follows plan/configuration validation but
precedes score factories, provider solve events, commands, progress events and
budget/cancellation exits. It applies even to immovable initial-only solves.

Optional pinned None values and initialized pins remain valid. List-only models
are unaffected. The default is false: native partial construction and low-level
completion enumeration still support permanently incomplete pinned states. With
the policy disabled, local search retains its existing phase-specific incomplete
assignment diagnostics. The startup policy is explicit in the corresponding
Timefold construction and exhaustive-search adaptations; Java defaults/API parity
are not claimed.

### Cooperative termination inside candidate filters

`FilteredSelectionSource::with_termination` and
`FilteredMoveProvider::with_termination` accept a positive polling interval and a
`FnMut() -> bool` callback. They poll inside rejected runs, after each interval of
inspected candidates, before requesting the next candidate. Accepted candidates
also advance that interval across iterator calls. There is no initial poll; an
interval of one gives the shortest cooperative delay. A true response ends and
fuses the current traversal with `CandidateFilterStop::Terminated`. Counters and
poll cadence reset when reopened; the callback retains its own captured state.
Size hints and early cursor drops do not poll or fabricate exhaustion. Source and
provider factories are lazy; provider scan/selection limits take precedence over
a coincident poll. Existing callers without termination callbacks retain their
work limits and filtering behavior.

For cancellation during long rejection runs, capture the same `CancellationToken`
in the callback (`|| token.is_cancelled()`) and pass it in solver configuration.
The callback can also check a deadline. A callback-only stop means this provider
has no more proposals; it does not by itself set a solver-wide termination reason.
Child iterator, predicate and termination callbacks remain cooperative and cannot
be interrupted while running. External effects and panics are not rolled back.
A terminating repeating source retains its child's never-ending metadata; a stop
is not evidence that a complete candidate pool has been materialized.

The pinned filtering bailout case preserves two termination polls and exactly
2,000 rejected child occurrences with interval 1,000. Original regressions cover
cadence across accepted/rejected candidates, reopen/drop/panic behavior, cap
precedence and cancellation during actual scoped and stream basic search. This
does not claim a global Java polling cadence or add upstream credit for native
regressions.

### Pairing independent selection streams

`ZippedSelectionSource` lazily pairs corresponding occurrences from two sources,
including sources with different payload types. It does not collect inputs or
require cloneable payloads. Both sources receive lifecycle events in left/right
order. Child factories open on the first requested pair. Reads happen left first;
if the right child ends, one unmatched left occurrence may have been consumed.
Either child ending fuses the cursor; reopening creates fresh child traversals.

Apply pin/value filters to individual children when each input must advance
independently, or filter the paired source when rejecting whole combinations.
Never-ending metadata requires both children to advertise repetition. Exact size
is reported only for known empty inputs or two finite inputs with known sizes;
repetition alone cannot guarantee nonempty output or rule out cancellation.
Bound repeating pairs with `LimitedSelectionSource` before using a scoped
`CachedMoveProvider`, whose mapper resolves candidates against the current model
and assignment. Source iterators remain cooperative.

Four adapted list cases preserve scripted change/swap orders, selected positions,
no-ops, optional assignment, continuation metadata, and the destination draw bound.
They use existing native move validation and PRE-to-POST-removal coordinate
conversion. They do not assert Java seeded random-stream parity. Native regressions
also drive a scored list search through bounded repeating value/destination pairs.

### Score checks at step boundaries

`ConstructionConfig`, `ProviderConstructionConfig`, `LocalSearchConfig` and
`RuinRecreateConfig` now have `assert_step_scores`, disabled by default. It enables
existing initialization and scan/placement-boundary consistency checks without
rescoring each candidate or each rollback. The setting reaches basic, list and
mixed engines with scoped, stream and calculator scoring. Solver-bound construction
providers inherit it, and ruin/recreate passes it to reconstruction and initial
scoring. A phase that never starts does not gain checks; use the separate solver
phase-score option for initial-only solves.

`assert_scores` still enables the full candidate/rollback/commit checks. Enabling
both flags does not duplicate checks. Step checks can catch a corrupted committed
state, but can miss a temporary corruption repaired before the next boundary.
They do not implement Timefold's complete STEP_ASSERT environment-mode contract,
which also specifies undo checks. The environment-policy draft remains unintegrated;
no new upstream method credit is claimed for this native option.

Diagnostics do not consume reported evaluation budgets, but callback time still
belongs to cooperative deadlines. Original fault-injection regressions exercise
all nine backend routes at initialization, provider construction and reconstruction,
and committed-state failures in basic/list/mixed local search and construction.
For a one-candidate calculator scan, step checks create three scorer instances
(initial plus two diagnostics); full checks create more. Search work and retained
results agree when scoring is correct. This is a callback-count observation, not
a wall-clock performance claim.

### Entity-driven sublist proposals

`SubListSelection` now exposes `entity_len` and `at_entity` alongside global
ranks. Entity-local ranks have the same descending-length/ascending-start order,
excluding whole pins and pinned prefixes. Missing and ineligible entities have
zero spans. Lookups use the existing per-entity index; no span catalog is built.

`random_iter_for_entities_with` samples within externally selected entities.
Callers can supply value-owner selections, explicit entity weights or replayed
entity orders. This differs from the existing sampler's uniform distribution over
all spans. `change_samples_with` combines these draws with destination inputs;
`swap_samples_with` draws each side independently. Rank/reversal callbacks permit
native randomness or explicit replay. The existing seeded samplers are unchanged.

Each iterator bounds entity reads, including skipped entities; swaps apply that
bound independently to both inputs. Skipped entities consume neither randomness
nor destinations. Empty span indexes consume no inputs. Exhaustion fuses the
cursor, including when an input itself could resume after returning None. A missing
right side can consume one unmatched left draw. Callback effects are not rolled
back if iteration panics.

These APIs emit proposals, retaining duplicates, no-ops, overlaps and invalid
receivers. Destinations use native **post-removal** indices without adjustment.
Callers must validate proposals before scoring them. Same-entity swap proposals
normalize by start index, preserving ties; cross-entity side order remains intact.
Unassignment consumes no reversal draw. Rebuild indexes after assignment changes.
Original regressions exercise read budgets, pins, independent draw order, rollback
and actual list/mixed solves. Ten pinned upstream proposal sequences are retained
as attributed partial fixtures; selector cardinality, unbounded metadata and
lifecycle remain outside these iterators, so those fixtures earn no method credit.

`EntitySubListSource::repeating` and `repeating_with_draw` add opt-in productive
repetition to the composable source API. Their nonzero read limit applies to each
requested span, including its successful entity read. A success resets the next
request's budget; child exhaustion or a full run of rejected entities fuses the
opening. Reopening rebuilds span coordinates and resets the read budget while
continuing random state. The ordinary constructors retain a total read budget.
Never-ending metadata is inherited from the repeating child and describes a
capability; rejected or exhausted children can still end an opening.

Ten separate complete adaptations exercise the composable change/swap sources:
ordered proposal strings, catalog cardinalities where asserted, continued
iteration, metadata and requested random bounds. Their fixture/provenance records
include optional values, reversal, span bounds and short-entity rejection.
Scripted owner indices replace Java value anchors, and a valid extra proposal
replaces the source helper's `hasNext` check. Raw change proposal strings preserve
the source's pre-removal coordinates; executable native moves still require
coordinate conversion and validation. Java lifecycle methods and PRNG traces
remain outside this coverage.

## Retained stream phases

`StreamScoreDirector`, `ListStreamScoreDirector` and `MixedStreamScoreDirector`
now expose `run_construction` and `run_local_search`. These methods start from the
bound assignment and reuse the same compiled graph, source identities, immutable
facts and cached scores. Construction leaves its completed prefix installed; local
search saves graph snapshots at initialization and strict best improvements, then
installs the retained best assignment/graph on successful return. Restoring the best
uses cached state, including list shadows and joined/grouped constraints, without
scoring callbacks. Snapshots add storage proportional to graph state (with sharing
where supported by its nodes).

Each call starts fresh phase acceptance, random and termination state. The usual
initial score evaluation still consumes one evaluation; already-completed graph
initialization contributes zero new constraint callbacks. Search results report
actual new constraint work, excluding diagnostics. Zero-budget and pre-cancelled
calls preserve the director; validation still precedes those checks. A scoring error
or callback panic leaves the last committed consistent step, which may differ from
the phase-entry or best assignment. External callback side effects are not undone.

These explicit phase calls have per-call limits; callers can replace assignments
between calls. For shared controls and events, use the retained whole-solve methods
below. Existing `solve_*_stream` functions retain their fresh factory-per-phase
contract. Retained sequences support all built-in phase kinds as described below;
provider extensions are supported as described below. No upstream whole-solver
list-state-identity test credit is claimed.

## Whole solves on a retained director

All three stream directors expose `run_solver(config)` and
`run_solver_with_events(config, observer)`. Basic/list methods accept `SolverSequence`
or `SolverConfig`; mixed methods accept `MixedSolverSequence` or `MixedSolverConfig`.
Ordered construction, local, custom, exact and ruin/recreate phases use the existing
shared solver controller, including score
and candidate budgets, cancellation, deadlines, stagnation, composed termination,
score/feasibility goals, incomplete-handoff policy and frozen-model initialization.
Mixed construction order and each local phase's joint-move setting are preserved.
Phase outputs, initialization notifications and best-event suppression use the same
controller as fresh-graph solves.

The solve starts from the director's current valid assignment. Graph construction,
fact loading and original binding have already happened and are outside this call's
evaluation, callback and time counters. Constraint counts include only new work;
score-evaluation and candidate budgets keep their normal semantics. On successful
scored return, the director retains the result assignment and its best cached graph.
Errors or callback panics keep the last committed consistent state. Callers must
prepare invalid dependency layouts before constructing a director; an already
prepared initial state emits an ordinary first-best event, unlike a fresh solve's
suppressed repair-baseline event.

`assert_phase_scores` rebuilds/checks the retained graph through `assert_consistent`
at phase boundaries; it cannot test reproducibility of an external graph factory.
Diagnostic work is excluded from work counters, while elapsed time remains part of
the solve deadline. Events and the controller's initial-only scoring path do not
rebuild graph topology or rerun immutable-fact setup.

`run_solver_with_commands` and `run_solver_with_commands_and_events` bind typed
custom commands by phase index. Unbound custom phases reject before scoring;
initial-only checks never execute user commands. Successful custom phases restore
their best cached graph, even if the last accepted command worsened the score.
A caught best-observer panic invalidates the custom phase just like a caught scoring
panic, so it cannot silently return a best assignment with an older graph snapshot.

Exact enumeration, completion and branch-and-bound replace planning rows within the
same graph. Successful scoring commits each candidate; best notifications capture
its graph. The returned best (or unchanged prior best) is installed without scoring.
Ruin/recreate uses temporary model pins/ranges for reconstruction selection while the
bound model retains scoring identities, value registry and dependency rules. Its
initial scoring, fine moves and repeated reconstructions share the graph. Rejected
or incomplete candidates are replaced by the retained best on successful return.
After an error or panic the last committed consistent candidate can be partial;
phase execution does not promise whole-phase rollback. Input and scoring errors remain errors. Dependency-cyclic fine candidates are
handled as described below.

Existing free `solve_*_stream` functions preserve their factory-per-phase contract.


## Provider extensions on retained directors

All three directors expose `run_solver_with_extensions(config, extensions)`, using
the same `SolverExtensions`/`MixedSolverExtensions` as fresh solves. Providers bind
to construction/local phase indices and may share one instance across several
phases. Bound providers replace the built-in neighborhood for that phase. Custom
commands, progress events and independent `score_check` callbacks compose with
those bindings and ordinary shared budgets, scan limits and phase assertions.
Checks operate on phase outputs before handoff; they never rerun commands/providers.

Invalid bindings reject before new scoring or lifecycle notifications. A valid run
starts one solve lifecycle per distinct provider at its first scoring kernel and
closes it on success or returned error; panics do not invoke cleanup callbacks.
Phase/step events retain the existing provider engine ordering, including iterator
destruction before StepEnded. Callers must reset provider state after a panic.
Independent score-check failures leave the director's last committed graph intact.

`run_construction_with_provider` and `run_local_search_with_provider` offer standalone
retained phases with per-call limits and ordinary phase/step events. Construction
uses `ProviderConstructionConfig`, including scan caps and incomplete-scan policy;
local search restores its best graph without rescoring. Earlier binding/fact setup
is excluded from new work counters. These APIs reuse the existing provider engines;
no separate candidate-selection or lifecycle implementation is introduced.


## Dependency-safe fine sampling in repair search

For list and mixed repair searches, fine-move families exclude proposals whose
final list predecessor graph introduces a dependency cycle. A borrowed move checker
validates the snapshot once and checks each proposal without applying it to live
state or scoring it. Configured mixed families check list singles and joint moves;
basic-only proposals remain eligible. Other model/scoring errors retain their usual
failure behavior. Explicit moves and custom commands still report dependency cycles.

Uniform reservoir sampling counts only the remaining candidates. A family with no
eligible candidates is removed and its weight is redistributed normally; if every
family is empty, search stops with `NoAvailableMoves`. Rejected cyclic proposals
consume no random draw, attempt or score/candidate evaluation. The sampler receives
one slot per raw candidate, including rejections, and polls cooperative controls
between slots. Models without dependencies retain their prior candidates and random
draw order, using the existing iterator allocation without a checker.

## Live step-score metrics

`SolveMetricSnapshot.last_step` carries the phase index/kind, attempted step index,
score and completeness of the last successfully completed step. It includes
construction, local-search and accepted changed repair steps, plus every committed
custom `execute` move. It retains worsening scores even if a phase later restores
its best state. `completed_step_count` counts these publications across phases;
custom commits do not change existing search-step termination counters.

Snapshots publish before `StepEnded`; `StepStarted` sees the preceding completed
step. Custom execute attempts now emit these lifecycle boundaries, while previews,
initial scoring, exact enumeration and diagnostics do not. Failed attempts preserve
the last published step. Publication remains visible if a subsequent StepEnded
callback fails. Failed/panicked runs retain that observation, and a new run starts
empty. Caught custom lifecycle panics invalidate the command phase.
Score types remain generic, without a new static-lifetime or serialization bound.
Disabled monitoring does not clone scores or scan completeness for publication.


Shared invocation clock and live reconstruction quotas
------------------------------------------------------
`SolverExtensions.clock` accepts a borrowed, clonable `SolverClock` whose callback
returns arbitrary duration ticks. Each solve rebases that source and clamps
backwards readings; its phase and reconstruction timers have separate origins.
All 18 fresh/retained model/backend routes share the clock for deadlines and
stagnation windows. Existing setup/diagnostic charging boundaries remain. Metrics
keep their own observation clock; standalone kernels retain real clocks. A frozen
source cannot terminate an otherwise unbounded search. Callback panics propagate.
No custom-clock allocation, callback or lock occurs when the option is absent.

Composed move-count conditions now observe successful reconstruction candidates
between scores, including nested AND/OR and scored conditions. Pending counts
are classified as Reconstruction without incrementing outer construction steps.
Metrics still publish counts only after a successful rebuild; failures publish
no partial rebuild counts. The temporary counter exists only when a move-count
expression needs it. Hard budgets and cancellation remain independent.


## Composed conditions for individual phases

`SolverExtensions.phase_terminations` borrows a slice of
`PhaseTerminationBinding::search(index, expression)` or `::scored(index, expression)`.
Indices refer to the complete plan. Duplicate or out-of-range bindings fail before
clock, lifecycle and scoring callbacks. The default empty slice activates no phase
control or extra counters.

Each bound phase starts independent elapsed-time, score-count and move-count origins,
best-score history, stagnation tracking and expiry state. Time includes its start
callback and preparation. Initial scoring counts as a score evaluation, but not a
move. Complete retained input can satisfy a scored goal immediately; partial input
cannot. Construction observes complete scores at retained placement boundaries.
Local, repair and custom search arm stagnation after initialization; exact search
arms it before enumeration, and construction never arms it.

A phase condition stops that phase and allows the next phase to run. Its expiry
state is removed before handoff checks; independent global limits keep their normal
precedence and may stop the whole solve. An immovable initial-only solve enters no
phase and therefore uses no phase binding. Reconstruction scans poll enabled
expressions even without optional counters or an injected clock, and retain the
existing rules for provisional candidate counts and successful bulk publication.
Standalone kernel APIs are unchanged. These native semantics do not promise Java
phase traces or reflective factory behavior.


## Composed score-difference stagnation

`StatefulSearchTermination` combines static scored/count/time expressions with
`SignificantImprovement` leaves through AND/OR. Bind it solve-wide through
`SolverExtensions.stateful_termination`, or per phase through
`PhaseTerminationBinding::stateful`. Each history occurrence owns an independent
threshold state, including repeated leaves. Every leaf receives observations and
time samples even when another branch already determines the Boolean outcome.

`StatefulTerminationSession` separates search start, complete retained-score
observations, and status queries. Construction cannot arm or reset a score anchor;
once armed, solve time continues through later construction and setup. A later
phase's worse score cannot reset the existing solve anchor. Phase bindings start
fresh histories. Threshold comparison and strictly-after-window expiry reuse
`SignificantImprovement`; sampling progress alone is not a score observation.
Stateless `try_status` returns `HistoryRequired` for history-bearing expressions.
Existing stateless termination APIs and independent hard limits retain their scope.

## Updating retained explanations

`ScoreExplanation::replace_constraint` inserts or replaces one report;
`remove_constraint` retracts one by name. Both return the previous report and use
checked arithmetic by default. Their `*_with_arithmetic` variants also support
wrapping. Checked arithmetic validates final totals after cancellation. Qualified
match IDs preserve equal tuple IDs from different constraints, and live zero-score
matches retain indictment membership. Justifications need not implement Clone.

Existing-fact replacements update affected indictments without rebuilding unrelated
reports. Introducing or retiring fact keys also scans the fact-key index. Errors
and calculation/clone/comparison panics preserve the explanation; panicking
destructors are excluded. Public snapshot fields must remain internally consistent,
as produced by constructors. These methods do not repair manually corrupted reports.

## Recoverable shadow callback errors

`ShadowSessionBuilder::try_input`, `try_list_input` and `try_derive` accept callbacks
returning Result. The first error stops further evaluation before cached values,
dependencies, change reports or scores are published. Pending notifications remain
available for retry. Successful None still means a consistent absent value, and
cycles retain their normal supplier-suppression behavior. Existing infallible APIs
are unchanged; no extra full-cache snapshot is needed.

`ShadowSessionError::Callback` retains the original downcastable error, callback
stage, field/entity type and session-local registration index. Error values remain
Send and Sync; only the new callback error types require these bounds. Session and
manager errors retain the typed cause, while graph adapters retain their existing
string diagnostics. External callback side effects cannot be rolled back.


## Recoverable move-provider errors

`FallibleMoveProvider::try_moves` can fail during setup or while drawing a lazy
candidate. Bind it with `bind_fallible` or `bind_shared_fallible`, or use a
`*_with_fallible_provider` entry point. An observed error discards the current
scan, including a saved winner, while retained directors keep their last completed
step. Construction applies the same rule even with KeepBestPrefix enabled.
`MoveProviderError` retains a downcastable cause; cloning preserves error identity.

Candidate buffering and the existing budget-exhaustion lookahead can observe an
error before another move is scored. Errors beyond the part actually polled remain
unobserved. Returned errors close ordinary lifecycle scopes; arbitrary panics keep
the existing unwind behavior. Lifecycle hooks remain infallible. Existing
MoveProvider implementations and their draw order remain unchanged.

`take_binding` removes either provider kind and returns a shared provider only when
its last binding is removed. Legacy `remove` leaves fallible bindings untouched.
`bind_provider` returns either replaced kind; legacy `bind` can replace a fallible
binding but cannot return it through its legacy return type.

## Cooling from termination progress

`Acceptance::SimulatedAnnealingTermination` uses the configured Solve or Phase
termination expressions. It samples progress once at step start and freezes that
temperature throughout candidate selection. Solve expressions combine by OR;
phase cooling follows that phase's binding. Hard safety budgets and cancellation
do not supply a cooling fraction.

Unknown progress holds the previous fraction, initially zero. A lower known
fraction, for example after a significant-improvement reset, reheats the acceptor.
Every phase starts fresh acceptance state; solve progress retains its existing
cross-phase history. Missing expression scope fails before plan callbacks, and
standalone kernels return TerminationContextRequired. Low-level acceptors may
receive explicit fractions through `termination_step_started`. Existing step-
and elapsed-time cooling policies retain their behavior.


## Initialized score-goal progress

`StatefulSearchLimit::ScoreGoal` accepts a validated
`ScoreGoalProgressConfig::target` or `::feasibility`. Each occurrence captures its
own initial complete score. Solve scope includes construction initialization or a
complete committed placement; phase scope uses complete retained input, otherwise
its first complete observation. Negative structural scores cannot set a baseline.
Capturing a construction baseline does not update retained best-score history or
arm stagnation. Optional worsening construction keeps its existing semantics.

Weighted progress reuses the existing score-progress arithmetic and feeds
termination-driven cooling. Exact target and feasibility predicates decide when
to stop: rounded progress or zero-weight levels can produce 1.0 before the goal
is reached. Configuration validates weights and score shape before callbacks.
Polling borrows scores without cloning them. Standalone sessions require explicit
`observe_initialized_score`; status queries do not initialize a baseline.


## Live list-swap selection

`ListValueSwapSource` lazily pairs two child value sources using the current
assignment. Pins and required missing values are filtered independently before
pairing; receiver ranges and dependency validity are checked afterward. Duplicate
occurrences keep their weights and order. Optional assigned/missing pairs produce
an atomic replacement. `include_noops` retains equal or two-missing pairs as empty
composites without bypassing validation.

`ListSwapSourceLimits` bounds reads per side for each attempted pair and pairs
per yielded item. Exhaustion or either limit fuses an opening; success resets the
next item's budgets. Never-ending metadata requires both child capabilities and
does not promise infinite productive output. First iteration builds the position
index and opens children, left first. Reopen after assignment changes; unknown
streamed values panic at the infallible source boundary. Bound outputs with
`LimitedSelectionSource` or `take` before provider collection. Four complete
Community methods cover scripted random prefixes, pinning, optional replacement
and restricted entity ranges; optimized Java random traces remain pending.


## Conditional custom-command commits

`CustomPhaseContext::execute_if` applies and scores one candidate, then gives its
assignment and score by shared reference to a commit decision. It returns a
`CustomTrialOutcome` with that score and whether the candidate was committed, or
None when a cooperative limit prevents evaluation. Acceptance does not replay the
move or rescore it. Rejection rolls back while retaining evaluation/work charges;
only accepted trials enter existing best retention and custom committed-move
accounting. SolverWork keeps its existing custom-phase summary convention.

Accepted StepStarted notification occurs after the decision and before commit;
rejected trials emit no step events. A caught decision panic rolls back and leaves
the command usable. Scoring, commit or lifecycle failures invalidate the phase.
Failures after commit do not undo the committed move. Cancellation raised during
the callback is observed at the next cooperative boundary. All eighteen solver
routes retain their existing best restoration behavior, including after accepted
worsening moves. A stateful four-decision regression reduces score calls from
seven to five and before/after change calls from ten to six each, with the same
retained best and two commits. This is callback-count evidence, not a timing claim.

## Sequenced custom commands

`CustomPhaseContext::run_commands` executes an iterator of commands within one
phase budget, score history and calculator instance. Each invoked command starts
from the retained best assignment. Between commands, a worse or tied working
assignment is restored, including actual payloads when equality compares only
identity. A command already at the retained best needs no reset. The last command
uses the ordinary phase-exit best restoration.

`CustomPhaseWork::restorations` counts these handoffs. Each necessary restoration
charges one score evaluation and actual constraint work, without a move, step or
improvement. Limits are checked before pulling the next command and before its
execution; a restoration can exhaust the remaining budget. Errors and caught
command panics invalidate the context. Earlier commits are not collectively
rolled back. All eighteen solver routes have focused sequence coverage.

## Weighted interleave refill

`WeightedInterleave` primes enabled children on its first draw, then refills only
the previously selected child. It rebuilds the weighted distribution when a child
retires; ordinary draws use the existing binary search. Initial priming and each
retirement rebuild still scan the families. Caught child panics preserve pending
refill and previous retirements. Construction remains lazy.

The native construction-and-consumption comparison includes allocations and
priming. Seven workloads, each with eleven alternating pairs, retain identical
output counts and checksums. Three active families improve about 2.14x and 1,024
active families about 23.67x; short and retirement-heavy cases are included.
These are iterator measurements, not full-solver or Java comparisons. Evidence:
`work/parallel-round21-evidence/interleave/work/interleave-refill-evidence.json`.

## Transparent provider composition

MoveProvider::neighborhood_count queries explicit generator structure without
opening candidate cursors, invoking predicates/reset callbacks or emitting events.
Opaque providers count as one, including Cartesian generators and per-move
probability selection. ProviderUnion::unweighted recursively sums child counts;
explicitly weighted ProviderUnion::new returns NeighborhoodCountError::WeightedUnion,
including through nested transparent decorators. Weighting remains available for
actual candidate selection. Counts use checked usize addition and are structural
metadata rather than candidate cardinality, productive output or an exhaustion proof.
Unweighted unions still choose immediate children equally. MoveProviderGroup keeps
its existing iterative flattening and leaf-proportional random distribution.

FilteredMoveProvider, PhaseResetMoveProvider, LimitedMoveProvider and
CachedProviderMoves forward counting and errors to their child. LimitedMoveProvider
caps yielded proposals per opening and skips the child factory at zero. Lifecycle
and construction-placement notifications remain delegated. A downstream filter
can reject these proposals; put the limit before filtering to bound inspected work.

CachedProviderMoves retains a complete finite stream of owned moves in original
order, preserving duplicates. Step, Phase and Solve scope invalidation follows real
provider events, with higher boundaries clearing shorter-lived snapshots. Direct
calls synthesize no events and require explicit reset. Editing the child clears the
snapshot first. A maximum collection limit bounds reads; an overflow reads at most
one extra candidate, publishes no partial snapshot, and returns ProviderSnapshotError
from try_moves. The infallible MoveProvider boundary panics with that error.
Zero skips collection. Output cloning is lazy. Callbacks and reads cannot be rolled
back or preempted. Use this wrapper only for moves valid throughout the selected
scope; cached list positions/dependencies/pins can stale. CachedMoveProvider instead
retains selection keys and maps them against each current assignment.

## Reversible ordered filtering

FilteredSelectionSource::cursor opens its child once, lazily, under an explicit
inspection ceiling. BidirectionalSelectionCursor retains accepted occurrences
without Clone. has_next can prefetch one accepted item without advancing position;
next_item/previous_item borrow retained payloads. Moving backward and revisiting
history do not call the source or predicate again. Position ranges from zero to
the retained length. Exhaustion/inspection termination is sticky for new reads;
retained history can still be revisited. The inspection limit bounds rejected work
and retained storage; reaching it does not prove source exhaustion. Zero skips the
child factory. Existing cooperative cancellation polling also applies. Source and
predicate callbacks must return promptly. Dropping the cursor releases history.

CachedSelectionIter now distinguishes Original slice traversal from Random weighted
replacement sampling as actual enum variants. Empty random snapshots keep the
Random strategy while yielding nothing. Existing ordering, duplicate, progress,
no-draw-on-empty and fused exhaustion behavior remains. Original iteration no longer
stores an unused random callback; it drops that callback when the iterator is built.
This does not establish a timing improvement or Java iterator class identity.
