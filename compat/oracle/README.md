# Development-only Java reference adapters

These are original adapters around public score and solver APIs in the released Community
artifact `ai.timefold.solver:timefold-solver-core:2.7.0`. They do not read or port
production source. Java and the downloaded artifact are not Rust library dependencies.
The artifact is downloaded to ignored `work/oracle/`, checked against Maven Central's
SHA-1, and recorded by SHA-256 in measurement reports.

## Score comparison protocol, version 1

Each UTF-8 input line has four tab-separated fields:

```
score_type<TAB>operation<TAB>a<TAB>b
```

Types: `simple`, `hard_soft`, `hard_medium_soft`. `a` is a full score string.
Operations `add`, `sub`, and `cmp` take a second score string in `b`.
`mul`, `div`, and `pow` take a floating scalar in `b`; `parse`, `neg`, and `abs`
ignore it but the fourth field must exist. Valid score outputs have full string,
short string, lowercase feasibility boolean, and structural integer separated by
tabs. Comparison outputs are -1, 0, or 1. Malformed score input yields `ERROR`.
Adapter programming/protocol errors are not covered by the score compatibility claim.

The Python generator saves original inputs as versioned JSON in `work/oracle/cases.json`:

```json
{
  "schema_version": 1,
  "cases": [
    {"type": "hard_soft", "operation": "mul", "a": "1hard/-1soft", "b": "1.2"}
  ]
}
```

The driver requires one output per input and exact Java/Rust output agreement.
These generated boundary/random cases are original checks, not imported upstream
tests, and do not increase the adapted-method count. Their fixed seed, failures,
reference pin, artifact identity and source fingerprint are recorded in
`work/differential.json`. See `docs/score-contract.md` for exclusions.

## Benchmark protocol

Both score benchmark executables take `kind iterations seed warmup samples`.
Each measured output line is `elapsed_nanoseconds<TAB>checksum`.
The driver requires all Rust/Java checksums to agree before writing a comparison.
Benchmark source is original and equivalent in both languages; it is not copied
from Timefold's implementation or benchmark suite. All samples and conditions
are preserved in `work/performance.json`; see `docs/dashboard.md`.


## Complete-planning protocol, version 1

`SolverOracle.java` and `examples/solver_oracle.rs` accept six tab-separated fields:
case ID, number of values, comma-separated initial values (`-` for unassigned),
pin bit string, comma-separated `a:b` hard-conflict edges, and row-major
comma-separated signed soft weights. Domains are homogeneous integer IDs `0..k`.
Both adapters clear movable initial values before solving, preserving pins. This
normalizes the search space because Timefold's exhaustive phase otherwise retains
preassigned values. All variables in this protocol require assignment.

Each output is case ID, hard score, soft score and comma-separated assignment.
The driver compares score and pin/domain validity with an independent Python
enumerator. Equal optima may have different assignments. Inputs, outputs, seed,
source hashes, runtime dependencies and errors are saved under ignored `work/`.
Original generated cases do not increase upstream-method coverage.

`SolverBenchmark.java` and `examples/solver_benchmark.rs` take iterations, warmup
batches and sample count, and consume one protocol line on stdin. Models (and the
Java factory) are prepared before timing. Each output gives elapsed nanoseconds,
hard/soft checksum sums across solves, and the last assignment. Both validate
returned assignments and recomputed scores inside the measured work. The Python
driver also checks sample checksums and last assignments against the independently
computed optimum, converts to milliseconds per solve, and preserves every sample.
See `docs/dashboard.md` for the explicit algorithm/workload limits.

## Stream comparison protocol, version 1

`StreamOracle.java` and `examples/stream_oracle.rs` extend the six-field planning
protocol with a seventh field containing semicolon-separated operations:
`c:variable:value` commits, and `t:variable:value` tries then undoes; `-` is null.
They retain all initial values. Duplicate/self edges remain distinct facts.
Hard score is minus the number of equal assigned edge endpoints. Soft score
groups selected job weights by assigned value and sums `sum * abs(sum)`.
Adapters bound variables to 10,000, values to 128 and weights to [-1,000, 1,000],
keeping all intermediate and final scores within signed 64-bit bounds.

Each response has seven tab-separated fields: ID, zero-based state index,
completeness bit, hard score, soft score, conflict IDs and group sums. The initial
state and every candidate produce an output; trial rollback produces another.
Java's last two fields are `-` because Community score updates expose no match
analysis. Native conflict IDs are sorted comma-separated edge indices, and groups
are sorted comma-separated `value:sum` pairs, omitting zero sums. Empty native
match fields are empty strings. Python checks both ordinary scores, then checks
native match fields independently. Completeness is derived from null presence,
not Java structural scoring. No Enterprise dependency or production source is used.

`StreamBenchmark.java` and `examples/stream_benchmark.rs` consume the original
six fields with complete initial assignments and take `hill|tabu steps warmup
samples`. Output fields are ID, elapsed nanoseconds, hard score, soft score,
assignment CSV and terminal weighting callback count (which may be zero). Both
use incremental streams within local search. Java reuses its `SolverFactory`;
Rust constructs graph topology per solve. Per-solve setup, search and result
validation are timed. See `docs/dashboard.md` for full measurement conditions.


`AggregationBoundaryOracle.java` is an original three-state presence probe run by
`differential_streams.py`: empty input, one assigned zero-weight fact, and removal
of that last fact. Global count and sum groups each receive a constant penalty,
so a zero result cannot hide a spurious or missing match. The probe checks ordinary
scores from Community full updates. Native empty-group behavior is separately
covered by `collecting_stream_regression`; the report records this boundary check
separately from the 128 differential cases and grants no upstream-method credit.


`MixedStreamOracle.java` is an original Community public-API adapter for required
basic variables and optional ordered lists. Public preview MoveTester executes
composed basic/list moves; SolutionManager UPDATE_SCORE_ONLY calculates ordinary
scores at each initial, trial, undone and committed state. The native
mixed_stream_oracle uses persistent mixed stream scoring. Run the comparisons via
`python3 scripts/differential_streams.py`; mixed_stream_checks.py generates and
independently validates 128 cases / 2,378 states / 3,611 primitive operations.
No upstream tests or production sources were copied for this adapter. No mixed
solver-search, custom-shadow or runtime-performance compatibility is claimed.


`MixedSolverOracle.java` and `examples/mixed_solver_oracle.rs` consume four
fields: ID, depot-coordinate CSV, customer-coordinate CSV and capacity CSV
(one capacity per owner). Original small cases are generated by mixed_solver_checks.py
and run by differential_solver.py. Java outputs ID, hard, soft, chosen depots and
semicolon-separated route-ID lists (a dash for an empty route). Native output adds
a mode after ID (exact/scoped/stream), then evaluation count, completed steps and
termination reason after the routes. No field is a timing measurement.

Community runs public BRUTE_FORCE mixed configuration from unassigned state;
Rust exact enumerates the joint product, while native local search starts complete
with joint moves and 20 steps. Python independently recomputes all scores and
joint optima; differences in returned quality are retained. The current corpus
has 64 valid cases, with 39 Java, 64 native exact and 63 native local optima.
No production source or upstream tests were copied. No trace equality, Java
local-search comparison, mixed timing or extra upstream method credit is claimed.


The list move protocol additionally accepts `r,entity,start,end,destination,index,reverse`
for a half-open span, post-removal insertion and 0/1 reversal. The original 256
cases are retained and 128 span cases are added (seed 20261124). All 384 cases /
1,348 operations compare candidate/undo/commit states and four derived shadows.
This does not add Java sublist-search timing or pin-validation evidence.
