# Extra RealWorld application workloads

These workloads exercise the actual RealWorld application model independently
of the adapted upstream Timefold tests. They contribute **zero upstream methods**.
The approved public fixtures retain their original manifests and exact bytes in
`tests/application/real_world/`. The sailing fixture is the anonymized public
derivative; its restricted internal parent is not included.

The shared Rust model derives from the Claude `native-application` application spike identified
in that directory's `source-manifest.json`. It creates one group-valued variable
per attended person/session, singleton domains for fixed placements, and the
original fixed-first, greatest-spare-capacity initial assignment. Social Golfer
has 320 variables; Sailing Trip has 573, including the original attendance gaps.

The constraint graph retains the application's full equal-key self-join of
assignments followed by person-order filtering and grouping of encounters across
sessions. Unique contacts earn soft reward; repeat excess uses the specified
linear or squared penalty. Separate membership self-joins enforce hard and soft
pair constraints. Attribute balance uses the original collected occupied-group
semantics: an empty group emits no tally and therefore no balance penalty.
Capacity and hard pair violations occupy the hard score level. Soft input weights
are multiplied by 100 and rounded, as in the spike. These details are an application
model contract, not a claim about other RealWorld implementations.

The original within-session spare-seat changes and swaps retain their xorshift
move stream. Benchmark acceptance uses simulated annealing with soft temperature
500 (`SA=5` at scale100), cooling over 40000 steps, first-accepted selection and
the supplied seed. Diagnostic environment switches and `pprof` were removed.
No hot-path solver library implementation was changed.

Run the benchmark with:

```sh
cargo run --release --example real_world_benchmark -- \
  tests/application/real_world/social-golfer-32x8x10-v1/problem.json \
  10 31 /tmp/real_world-output.json
```

Stdout is one JSON summary containing `elapsed_ns`, `evaluations`,
`completed_steps`, `initial_score`, and `best_score` (integer `[hard, soft]`), plus
configuration and dimensions. The output JSON retains the problem and adds
`incumbent_schedule`, `benchmark_initial_schedule`, the original bundled
`solver_settings`, and
`benchmark_summary`. The schedule maps `session_N` to group IDs to person IDs,
allowing independent JSON-level rescoring. Reporting computes an initial score
before the timed solve; timed work includes the actual solve's score-director
initialization, as in the spike. Wall-clock budgets are cooperative.

Four deterministic regressions use both complete original public cases. A separate
JSON oracle directly enumerates people pairs and checks all capacities, pair
constraints and balances. It verifies full initial scores, trial scores and
rollback; repeated searches with seed31 and a 64-evaluation ceiling must retain
the same valid assignment and score, agree with a fresh graph and independent
rescoring, and preserve the initial best. This establishes bounded native
consistency, not stochastic Java trace parity, a global optimum, or a general
performance claim. Time-limited benchmark runs remain separate evidence.
