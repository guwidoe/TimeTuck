# Native XML termination composition

`XmlTerminationPlan::with_sequence_extensions` and
`with_mixed_sequence_extensions` lend executable solver/phase expressions to a
caller closure. The closure installs the supplied phase bindings in
`SolverExtensions::phase_terminations` and executes its normal solver route before
returning. The plan does not own a solver, score calculator, callbacks or providers.
These original native regression cases earn no additional upstream-method credit.

`terminationCompositionStyle` accepts `AND` and `OR`. Absence means OR. Conditions
are elapsed time, stagnation time, completed steps, stagnation steps and typed best
score; local phase composition also supports diminished returns. An empty
configuration contributes no condition, including an empty explicit AND. Invalid
style, duration, score, phase order/type and conflicting phase bindings fail before
the execution closure. Best scores are parsed against the actual native score type.

Root count/time/score expressions use the existing solve-wide counters and retained
complete best scores. Phase expressions use existing phase-local counters and
clocks, reset at each phase start. Construction step limits count committed
construction steps. Partial construction does not satisfy a best-score target; a
complete retained placement can satisfy it before an optional improvement scan.
Exhaustive, custom and native `ruinRecreate` phase score targets use the existing
phase hooks. `ruinRecreate` is an explicit native declaration, with no claim that
Timefold XML defines that phase. `partitionedSearch` does not denote ruin/recreate.

Caller hard caps, cancellation and existing root expressions remain independent
safeguards; they are not AND children. The XML root expression combines with an
existing scored root expression through OR. Existing nonoverlapping phase bindings
are preserved. Overlapping bindings reject rather than silently override either
expression. XML conditions on the extension route do not replace native count,
time or best-score fields.

Solver-level diminished returns retains the existing inheritance into local-search
phases. Explicit phase tuning takes precedence over inherited root tuning and
caller native policies remain independent. Solver-level AND with diminished
returns rejects because a phase-local rate policy cannot be combined with
solve-wide counters as one expression. A nonlocal phase diminished policy and
construction stagnation declarations reject. Significant-improvement thresholds and native score-goal gradients remain outside
this projection. Arbitrary nested XML termination configurations execute on the
extension route; each node preserves its AND/OR grouping.

The sequence-only `with_sequence`/`with_mixed_sequence` methods keep their existing
scalar projection and `FromStr` type bounds. They reject explicit AND and phase
conditions requiring the extension route. Their existing configured-factory
bindings remain compatible. The extension closure prevents temporary expression
references from escaping without leaking storage or changing the shared solver
kernels.

## Nested configurations

Repeated `termination` children are independent owned configurations, as in the
pinned Community test resource
`core/src/test/resources/ai/timefold/solver/core/config/solver/testSolverConfigWithoutNamespace.xml`
(local-search AND containing an OR child). Public composition semantics are described
in [Timefold optimization overview](https://docs.timefold.ai/timefold-solver/latest/optimization-algorithms/overview#termination).
The resource establishes syntax; no production annotations or implementation were
consulted. These native controls earn zero additional upstream-method credit.

Each node combines its own scalar leaves and nonempty nested children. Empty
configurations contribute no condition, even when explicitly AND. Owned parsed
values compile to borrowed expressions recursively inside the execution closure.
The tree retains grouping without self-references, unsafe code or leaked storage.
Phase nodes may contain nested diminished-return conditions. Solver diminished
returns remains inherited local-search tuning; an AND ancestor of such a leaf
rejects because its rate scope differs from solver counters. Multiple inherited
solver policies also reject rather than silently selecting one. Scalar-only
sequence projection rejects nested configurations and directs callers to the
extension route. Factory construction recursively preflights every declaration
before publishing its executor.
