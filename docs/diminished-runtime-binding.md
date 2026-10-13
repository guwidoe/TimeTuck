# Runtime diminished-return properties

`RuntimeSolverProperties` owns optional termination and diminished-return
properties. Binding to an existing `SolverSequence` or `MixedSolverSequence`
stages validation, then fills missing local-search policies through the existing
native default application. Construction and other phases, solve-wide limits,
and explicit local policies are retained. Phases added later need a fresh bind.
No shared timer, reflection, Quarkus injection or Java lifecycle is claimed.

An explicit true, supplied window, or supplied ratio activates defaults. Explicit
false suppresses this newly supplied runtime policy, including otherwise supplied
fields; it does not delete existing phase policies. Empty or absent nested
properties introduce no policy. The receipt always owns a solver termination
observation; its optional raw declaration preserves unset window/ratio fields.
The same receipt exposes the resolved configuration actually applied and the
number of local phases changed. Defaults use the existing thirty-second window
and 1/10000 rate ratio. A supplied 123.0 resolves exactly to 123/1.

Positive finite floating values are converted to their exact IEEE 754 rational
value. Fractions requiring a numerator or denominator outside u64 are rejected,
without rounding. For example 0.125 becomes 1/8, while 0.1 retains its exact binary
fraction. Zero/negative/nonfinite ratios and zero windows reject before phase
mutation. Suppressed fields are not consumed or validated.

Seven pinned Community recorder TEST methods preserve the complete nested
present/absent observations, 30-minute window, 123.0 ratio and disabled phase-helper
assertions. Native typed configuration replaces mocked runtime interfaces;
absence of newly injected phase termination and full unchanged phase equality
replace Java null phase termination containers. Runtime execution resolves defaults
separately from these raw observations. Four originals cover validation,
idempotency/preserved explicit controls, and controlled-clock actual standard and
mixed construction/local solves for dense and stream scoring, seed 37 with 40 steps
and 80 evaluations. This is native behavior, not framework/ABI compatibility.

The standard controlled-clock control charges 11 seconds per real score callback: the
30-second default expires while the 30-minute window reaches its step ceiling. A
second 30-minute run charges 601 seconds per callback and reaches genuine window
expiry. The same real configuration pipeline executes both runs.
