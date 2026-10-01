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
