# Executable native XML termination

`SolverXmlConfiguration::native_termination_plan()` projects termination settings
into owned typed declarations. `XmlTerminationPlan::with_sequence` and
`with_mixed_sequence` validate the whole binding before invoking the caller's run
closure, which receives the actual existing native solver sequence. They support
basic, list and mixed problems through both scoring backends. XML factory
callbacks may project the plan before constructing scoring resources and execute
it with the usual native solve APIs. This is an explicit optional binding; raw XML
parsing alone never promises that declarations will execute.

```rust,ignore
let xml = SolverXmlConfiguration::parse(text)?;
let termination = xml.native_termination_plan()?;
let result = termination.with_sequence(sequence, |sequence| {
    solve_with_extensions(&problem, &network, sequence, extensions)
})??;
```

The projection consumes `spentLimit` and `unimprovedSpentLimit` ISO durations,
additive milliseconds/seconds/minutes/hours/days aliases, step-count and
unimproved-step-count limits, `bestScoreLimit`, `bestScoreFeasible`,
`scoreCalculationCountLimit`, `moveCountLimit`, `unimprovedScoreDifferenceThreshold`,
and nested `diminishedReturns`. That container
supports `slidingWindowDuration`, additive sliding-window units, and
`minimumImprovementRatio`. The names and container shape follow the pinned
Community TEST configuration API and the [public termination examples](https://docs.timefold.ai/timefold-solver/latest/optimization-algorithms/overview).
The public page may advance beyond the pin; numeric fixture attribution and
runtime semantics remain grounded in the pinned TESTs and existing native engine.

Absent raw fields stay absent. `<diminishedReturns/>` is an explicit declaration
that resolves to 30 seconds and exact integer ratio 1/10000. Explicit decimal
ratios parse as `f64` and preserve their exact positive finite binary value,
including subnormals and `f64::MAX`; they are not silently replaced by the
integer default. Empty tuning is not synthesized when the container is absent.
In the scalar projection, existing native diminished-return policies win, followed
by explicit XML phase
settings, then XML solver tuning. Other specified XML controls replace their
corresponding native setting; unspecified settings, safety caps, seeds,
cancellation and score assertions survive. The extension route instead preserves
independent root and phase rate trees; see the composition contract below.

Solver spent time uses the existing shared deadline, including resource
preparation and phase handoffs. Solver stagnation excludes construction and
survives later phases. Solver step limits compose with the existing expression
using native OR semantics. Phase spent/stagnation/step limits use the existing
phase boundaries and reset at each entered phase. Diminished returns tune local
search only, using its existing phase-local history. No Java lifecycle, wall-clock
trace, preemption or solver ABI equivalence is claimed.

When XML declares phases, their count/order/types must match the supplied native
sequence. Explicit construction stagnation/diminished policies are rejected.
Score targets apply at solver and local-search scopes. Their raw text is retained
until binding, where the native score type implements `FromStr`; all declared
targets are parsed before any run callback. Absent targets preserve caller limits.
The sequence-only profile rejects construction score targets and supports only
time and work-count budgets on other nonlocal phases. Its AND declarations also reject. The
additive scoped extension APIs support AND/OR expressions and construction,
exhaustive, custom and native ruin/recreate score targets; callers install the
supplied phase bindings during execution. See
[XML termination composition](xml-termination-composition.md) for scope,
precedence and remaining unsupported combinations. Custom termination classes
and other unsupported executable fields reject rather than being ignored. The symbolic parser/read-write API can
still retain independently supported declarations. The legacy
`native_spent_limit()` getter now uses the same time-group parser for aliases
while retaining its limited scope: unrelated termination fields are ignored by
that getter and are validated by the complete executable projection.

Duplicates, malformed or negative numbers, nonfinite/nonpositive ratios, zero
sliding windows, time overflow, and combined duration/unit groups reject before
run/scoring/provider callbacks. Zero spent/count limits remain active native
limits; native initial scoring may occur at a zero valid budget. Public raw
objects are revalidated at binding, so subsequent invalid edits cannot reach the
run closure. Time enforcement remains cooperative.

Eight original methods exercise 46 real controlled-clock solves: each of the
basic/list/mixed models through Dense and Stream consumes solver diminished,
solver spent, solver unimproved, phase spent, phase unimproved, solver score-target and local score-target settings; four
additional two-phase solves distinguish shared deadlines from restarted phase
deadlines. They check actual edits, returned scores/terminations, validation and
unchanged mixed list ownership. Fixture-only numeric/default assertions retain
Apache attribution. These additions earn no new upstream-method credit.

XML score-calculation limits count native scored states, including initialization
and reconstruction; move limits count evaluated candidates and exclude initial
scoring and diagnostics. Composite candidates count once. The extension route
composes these budgets and feasibility as actual AND/OR leaves without replacing
caller caps. Feasibility requires a structurally complete retained best score;
`false` (or `0`) contributes no stopping condition. Boolean lexical forms are
`true`, `false`, `1`, and `0`. The scalar route binds native budget fields directly;
nonlocal feasibility requires the extension route. Original controls cover real
basic/list/mixed Dense/Stream work counts, grouping, independent caps, malformed
values, and every basic/mixed scalar phase profile. They earn no method credit.

Accumulated score-difference thresholds execute through scoped controls. Root
thresholds require the new three-output controls API (or the owned factory's
new_with_controls constructor); phase-only thresholds also work through the
two-output extension API. Scalar projection rejects this field. The composition
contract above describes exact comparison, grouping and solve/phase histories.
