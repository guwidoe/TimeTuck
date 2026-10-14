# Native XML termination composition

`XmlTerminationPlan::with_sequence_extensions` and
`with_mixed_sequence_extensions` lend executable solver/phase expressions to a
caller closure. The closure installs the supplied phase bindings in
`SolverExtensions::phase_terminations` and executes its normal solver route before
returning. The plan does not own a solver, score calculator, callbacks or providers.
These original native regression cases earn no additional upstream-method credit.

`terminationCompositionStyle` accepts `AND` and `OR`. Absence means OR. Conditions
are elapsed time, stagnation time, completed steps, stagnation steps, score and
move evaluation counts, typed best score and feasibility; local phase composition also supports diminished returns. An empty
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
are preserved. Explicit phase XML overlaps reject rather than silently override either
expression. Inherited root rate trees combine independently through OR with an
existing binding on the same phase. XML conditions on the extension route do not replace native count,
time or best-score fields.

On the extension route, solver-level diminished returns lends the complete root
expression to each local-search phase. Universal leaves use each phase's actual
counters, elapsed time and retained score; rate occurrences have independent
histories. Explicit phase expressions and caller native policies remain independent
OR safeguards, so a slower explicit phase rate does not suppress an earlier root
rate stop. The scalar-only projection retains its earlier tuning precedence. A nonlocal phase diminished policy and
construction stagnation declarations reject. Significant-improvement thresholds and native score-goal gradients remain outside
this projection. Arbitrary nested XML termination configurations execute on the
extension route; each node preserves its AND/OR grouping.

The sequence-only `with_sequence`/`with_mixed_sequence` methods keep their existing
scalar projection and `FromStr` type bounds. They reject explicit AND and phase
conditions requiring the extension route. Their existing configured-factory
bindings remain compatible. The extension closure prevents temporary expression
references from escaping without leaking storage. The existing first-step hook
arms composed phase rates after preparation.

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
Phase nodes may contain nested diminished-return conditions. Solver expressions project separately to solve scope and each local-search phase.
Phase-only rate leaves are omitted at solve scope, while the phase projection
retains universal scalar leaves and all rate occurrences. Nested grouping remains
intact; distinct rate occurrences never share a sliding-window policy. Scalar-only
sequence projection rejects nested configurations and directs callers to the
extension route. Factory construction recursively preflights every declaration
before publishing its executor.

## Scoped rate projection evidence

The pinned Community `TimeMillisSpentTerminationTest.phaseTermination` specifies
phase-local elapsed time for universal time leaves. `AndCompositeTerminationTest`
`phaseTerminationInapplicable` omits nonapplicable leaves, and
`DiminishedReturnsTerminationTest.testGracePeriodAndLongInitializationPeriod`
starts grace at the actual first step after preparation. The native composed
runtime now arms its rate histories from that existing first-step hook; a rate
leaf remains applicable and false before arming. New phases and new jobs create
fresh histories. No global improvement rate is synthesized.

Original opaque public-configuration probes against the pinned Community2.7.0
artifact additionally establish that an initially satisfied scalar goal can stop
root AND even when its rate leaf is unarmed. Root AND with two rate-only children
has a solve-scope empty AND identity and stops immediately; root OR with those
children has an empty OR identity and continues until a phase rate stops. These
are projections of real nonempty configurations, distinct from genuinely empty
XML declarations, which still contribute no condition. One-child nodes introduce
no additional composite identity. This behavior may surprise users, but replacing
it with a global rate would invent semantics. Native early solve checks may stop before initial
scoring, returning no score; the opaque Java probe initialized once first. This
is a documented initialization/trace difference, not additional compatibility
credit.

The probes use explicit ORIGINAL change moves and HILL_CLIMBING acceptance to
avoid default selector/acceptor exhaustion. Root AND(time1500ms, rate200ms) ran to
the time limit, OR(rate200ms, rate600ms) stopped around200ms, a phase600ms rate
remained independent of root200ms, and two phases restarted root200ms grace.
These elapsed observations establish scope and precedence, not timing performance
or exact Java/Rust trace parity. The raw original probe and its logs are retained
in the round178 scope evidence bundle. All new controls earn zero source-method
credit; existing complete adapted methods and provenance remain unchanged.
