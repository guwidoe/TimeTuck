# Multi-phase solving

The native solver entry points run construction followed by configured local-search
phases, sharing score/candidate budgets, elapsed time, cancellation and optional score/feasibility
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

Construction preserves supplied initial values according to its existing contract.
Every improvement phase receives the retained best complete assignment. It creates
fresh scoring and acceptor state and restarts its configured random seed. A phase's
own limits, score goal, no-accepted-move reason or time limit normally end that phase
and allow the next one. Equal-score results retain the earlier assignment.

Scoped entry points reuse a borrowed network. Stream entry points take a fallible
factory returning a fresh graph and its empty planning sources for each started
phase. The factory must keep the same deterministic objective and immutable facts
across phases. Preparation and preloaded immutable-fact scoring are outside solve
evaluation counts; binding and initialization inside every phase count normally.
Skipped improvement phases do not call the factory. Construction still runs its
validation at zero budget, which invokes a stream factory but no phase score work.

## Shared limits and phase boundaries

The default shared score budget is 100,000. The candidate budget, cancellation token,
elapsed-time budget, best-score goal and feasibility goal are optional/disabled by default. Each phase
gets the tighter of its own score/candidate cap and the remaining shared cap.
Initial scoring consumes a score evaluation in every phase that scores; it never
consumes a candidate evaluation. Diagnostics consume neither count. Thus shared
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

Local-search stagnation timing remains phase-local and resets each phase. Global
stagnation, exact-search timing, live problem changes, daemon operation, solver-manager
services, concurrent phases and cross-phase cache reuse remain unfinished.

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
shared limit reason accompanies its partial state; otherwise it returns
IncompleteConstruction.

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
