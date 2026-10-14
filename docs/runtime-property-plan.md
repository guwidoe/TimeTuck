# Native runtime property plan ingestion

`RuntimePropertyPlan::resolve(source, rules, namespace, sink)` reads an owned source
through the existing canonical/legacy resolver. `bind` and `bind_mixed` stage a
clone, validate existing RuntimeSolverProperties diminished policies, and commit
only on success. Supplied spent/unimproved time limits replace shared limits;
missing fields and other controls remain unchanged. Explicit local diminished
policies take precedence over runtime defaults. Phases added later require rebinding.

Public documentation references (read 2026-10-14; latest is not a pinned TEST):
https://docs.timefold.ai/timefold-solver/latest/running-timefold-solver/library/quarkus
 documents quarkus.timefold.solver.termination.spent-limit,
unimproved-spent-limit and diminished-returns.enabled/sliding-window-duration/
minimum-improvement-ratio. The service guide
https://docs.timefold.ai/timefold-solver/latest/quickstart/service/getting-started
 documents timefold.model.termination.spent-limit=PT30S.
The additional suffixes under timefold.model.termination are an explicit native
namespace policy; they do not claim an unseen Timefold service property contract.
Caller may select the documented Quarkus namespace, without framework loading.

ISO durations delegate to the existing JsonDuration parser/to_std and its supported
profile. Native short syntax is unsigned integer followed by ms,s,m,h,d with no
whitespace or fractions; arithmetic and standard Duration conversion are checked.
Enabled accepts only true/false. Ratio is parsed to f64 and must be positive finite;
existing binding additionally requires its exact binary rational to fit native u64
fractions. Zero time limits are allowed; diminished window must be positive. Even
explicitly disabled supplied declarations are parsed/validated. Missing properties
remain absent; unknown keys do not mutate the plan. Diagnostics are live resolver
callbacks, not JUL/SmallRye/Quarkus log integration.

This batch earns no upstream source-method credit. Original tests cover actual
solver consumption, deterministic fake clocks and seeded move ordering, dense and
stream scoring for basic and mixed models, untouched list ownership, policy/limit
preservation and malformed/overflow no-write failures. No production implementation,
Enterprise source, private runtime definition or bytecode was read.

Known ratio limit: an explicit decimal `0.0001` parses to an IEEE binary fraction
whose denominator is2^66, beyond the existing native u64 fraction envelope, and
binding rejects it without mutation. An absent ratio uses the native exact1/10000
default and succeeds. These paths are intentionally distinguished; this adapter
does not claim complete Timefold decimal-ratio compatibility.
