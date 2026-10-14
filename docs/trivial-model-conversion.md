# Owned identity model conversion

`TrivialModelConvertor<M, C>` is an optional conversion boundary for a real owned
model type that serves as input, solver model and output. Ordinary typed requests
use `ModelConversionRequest::new`; raw optional fields allow transport adapters to
represent missing arguments. A missing previous-output declaration differs from
an explicit declaration of no previous output. Preparation validates all three
required arguments before any solve callback runs and retains owned `Arc` handles.

Input-to-solver conversion retains input identity. Solver-to-output conversion
retains solver-model identity. Applying output to input returns the actual output
handle as the replacement; it does not mutate the old input. Rust type parameters
require matching model types without runtime class names or erased-type flags.
Configuration is validated for presence and carried unchanged to the native
callback. Identity conversion itself does not interpret it.

`solve` composes validated preparation with a caller-supplied actual solver. Its
callback receives the configuration and any previous output; using that output
as a warm start is an explicit native callback policy, not a claim about untested
Timefold previous-output semantics. On error the wrapper publishes no output or
replacement handle. Arbitrary callback side effects are not rolled back.

Five complete pinned Community AbstractTrivialModelConvertorTest methods retain
all missing-argument names, null boundaries, empty-previous-input identity and
output-to-input identity assertions. Native owned handles/optional request fields
replace Java null references, Optional and interface markers. Original controls
execute existing solver managers with both scoring backends, consume per-request
step settings, carry actual solved values into a second job, independently rescore
outputs and reject mismatched previous models before scorer execution. No Quarkus,
HTTP serialization, JVM interface or deployment integration is claimed.
