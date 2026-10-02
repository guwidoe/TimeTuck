# Local progress dashboard

Generate results and serve the visual overview:

```sh
bash scripts/check.sh
python3 scripts/differential_scores.py
python3 scripts/benchmark_scores.py
python3 scripts/differential_solver.py
python3 scripts/benchmark_solver.py
python3 scripts/benchmark_heuristics.py
python3 scripts/compat_report.py --test-output work/rust-tests.log
python3 scripts/dashboard_server.py
```

Open http://localhost:8765. Override the port with `--port`. The server binds only
to loopback, serves the dashboard directory, and exposes a read-only source
fingerprint. No framework, node modules, account, or hosting is required.
The browser reloads saved results every 15 seconds. It never runs tests itself.
`bash scripts/check.sh` regenerates coverage when the pinned submodule is present.
The separate Java oracle and benchmarks require JDK 21 and Maven Central access.

## Reading the numbers

The percentage is **passing adapted upstream methods / statically discovered
annotated Java test methods**. The current implemented slice has 160 / 4,855
(3.296%). This includes 282 explicit JVM integration exclusions in the denominator;
removing work from scope cannot increase the displayed percentage.

The scanner recognizes `@Test`, `@ParameterizedTest`, `@RepeatedTest`, `@TestFactory`,
and `@TestTemplate` after removing comments and strings. Abstract template methods
are counted once; parameterized/inherited/repeated invocations are not expanded.
Custom composed annotations and non-Java tests are not included. It is a source
inventory denominator, **not the total number of runtime JUnit invocations**.
The 1,501 source files also include helpers and fixtures; they are not a test count.
The original Java JUnit suite has not been executed by this pipeline.

Statuses:

- Passed: mapped Rust test actually executed successfully in the captured run.
- Failed: mapped Rust test executed and failed.
- Unverified: implemented mapping not executed, skipped, or missing from results.
- Pending: upstream method not yet adapted.
- Excluded: explicitly out of scope with a recorded reason in `compat/exclusions.json`.

The full JSON contains individual source links, mapping identities, result logs,
exclusion reasons, timestamps, source hashes, and performance samples. Selecting
a subsystem shows one square per method and a searchable list of source methods.
Score APIs/types include score-analysis API tests as well as numeric score types.
Other groups follow deterministic package/module rules in `scripts/compat_report.py`.

## Evidence and freshness

`run_rust_tests.py` captures the actual Cargo output and records before/after source
fingerprints, exit status, and a log checksum. Reusing a stale or modified log fails
closed. A missing/ignored test never becomes a pass. The live server checks current
sources against the report fingerprint, and the page warns on changed sources.
The warning does not overwrite the previous evidence. When served by a generic
static host, only the saved generation timestamp is available.

Generated reports and downloaded binaries live in ignored `dashboard/data/` and
`work/`. Regenerate them in a fresh checkout. `--inventory-only` emits an inventory
with implemented cases unverified. Running `compat_report.py` without a captured
log executes mapped integration-test targets to obtain fresh results.

## Performance comparison

Three matched score-only workloads exercise addition, lexicographic comparison,
and conditional subtraction on deterministic LCG inputs. Java uses public APIs
of `timefold-solver-core:2.7.0`; Rust uses the release build of this library.
Every process warms up before measurement. Runtime order alternates between forks.
The default is 3 fresh processes, 5 warmups, 7 measured samples per process,
5,000,000 iterations per sample, seed 20261001. Final checksums must agree.
The report includes every timing, medians, observed ranges, JVM/Rust versions,
CPU, OS, configuration, and the Maven artifact's SHA-256.

Timings exclude startup/compilation, use a shared machine without CPU isolation,
and are not JMH results or statistical significance claims. Nanoseconds are per
loop iteration, not a single score operation. A 1.01× ratio may simply be noise.
These score operations do not measure whole-solver throughput, memory, or solution
quality.

A separate exact-search section measures two original graph-assignment problems:
6 or 8 variables, each with 3 values (729 or 6,561 complete assignments). Both
runtimes maximize identical hard-conflict and soft-preference scores by brute
force. Timefold uses BRUTE_FORCE/ORIGINAL_ORDER; Rust enumerates complete finite
assignments. Java also visits partial states, so traversal/evaluation counts differ.
Every returned score is recomputed; sample score checksums and final assignments
must match an independently enumerated optimum.

The default is 3 processes × 7 samples × 5 solves, after 5 warmup batches per
process. Milliseconds are per complete solve. Models are parsed before timing;
Java also builds its SolverFactory before timing. Per-run solver setup, search,
allocation and result validation are included. JVM startup and input parsing are
excluded. These are deliberately small correctness-baseline workloads, without
incremental scoring. Ratios may largely reflect framework overhead. They do not
establish performance for larger problems, local-search heuristics, constraint
streams or memory use. The displayed ranges include shared-machine and GC noise.

The separate solver differential section compares 128 original cases: 64
homogeneous required-variable cases and 64 with per-variable ranges, optional
values and signed unassigned preferences, including empty optional ranges and
pinned nulls. The latter include eight explicit boundary cases and 56 seeded
random cases. Seeds and group outcomes are retained in the report. Both adapters reset movable initial values before
solving, retaining pins: Timefold's exhaustive phase otherwise preserves assigned
values and explores a smaller space. Both returned assignments are checked against
an independent Python Cartesian enumeration. The extended Java adapter uses one
entity class with required and optional fields, giving its inactive field one
fixed choice; this preserves the native assignment space without extra score
constraints. These cases add no upstream coverage and are not additional timing
benchmarks. See `docs/planning-contract.md` for the protocol and mapping.

`scripts/prepare_solver_oracle.py` resolves only the pinned Community dependency
and its runtime dependencies using Maven 3.9.16 in ignored `work/oracle/solver/`.
It verifies the downloaded Maven distribution checksum and records all resolved
JAR SHA-256 hashes. No upstream files are changed; no Maven installation is needed.
Restart the local dashboard server after editing Python reporting/fingerprint code
so the live freshness endpoint uses that version.


## Heuristic comparison

The heuristic section compares deterministic hill climbing and tabu search on
32- and 96-variable graph problems, with 50 completed steps at most. Both engines
use original-order change moves, best-accepted selection, first ties and the best
rejected fallback when no accepted candidate exists. Tabu tenure is two steps.
Both start with exactly the supplied complete assignment and recalculate the full
score for every candidate. This does not measure the Rust incremental network
against Java incremental scoring. Setup and returned-result validation are timed;
process startup, parsing and one-time model/factory/network setup are excluded.

The report records each sample's time, score, assignment and score-call count.
Python checks domain membership, pins, score recomputation and non-regression from
the initial score for every sample. Four workloads use a planted optimum of
`0hard/0soft`; reaching a step limit can leave them infeasible. The UI therefore
shows scores and feasibility beside runtime, without treating faster execution as
better quality. It labels differing scores explicitly. The recorded ratios apply
only to the chosen algorithm, budget, full-scoring implementation and inputs.

An additional 64 original checks (32 small inputs × two algorithms) compare valid
results and scores, with independent exhaustive optima recorded per case. These
checks contribute no upstream compatibility credit. Validity and Java/Rust score
agreement are separate counts. Run `python3 scripts/benchmark_heuristics.py` to
regenerate the combined quality/timing artifact; optional flags set completed
steps, warmup solves, samples and process forks. Full conditions and raw data are
included in the downloadable JSON and the measurement-notes disclosure.


## List-move comparison

Run `python3 scripts/differential_lists.py` for 256 original valid cases comparing
Java public `MoveTester` candidate/undo/commit states to native transactions and
an independent Python reference. Owner, index and previous/next shadows are
checked at each observed state. Seed 20261003; source digests, artifact hashes,
inputs, outputs and failures are retained. The dashboard renders this as a
separate evidence row and marks it stale when measured sources change.
It uses one optional list variable; pins and exact list solver results are covered
by native regressions, not this Java comparison. No list performance claim or
additional upstream coverage credit. See `list-planning-contract.md`.

## List-solving comparison

`python3 scripts/benchmark_lists.py` adds required-list construction and hill
climbing with identical routing inputs, Manhattan round-trip distance and capacity
penalties. The 128 original small validation cases (64 unrestricted and 64 with
entity-specific ranges) cover empty ranges, complete or
partial initial lists, full pins and pinned prefixes. Python independently
checks every output and enumerates complete owner/permutation assignments to
record exact optima. Construction retains existing values in their original
routes and relative order; hill climbing must not regress from the initial score.
Validity, Java/Rust score agreement and optimum attainment are separate counts.
These generated cases grant no additional upstream test-method coverage.

Four measured workloads use 16 or 32 customers, four depots, capacity n/4 and
seed 20261006; the additional range cases use seed 20261007. Larger timing
workloads remain unrestricted. Construction evaluates insertions without early picking and runs
to completion. Hill climbing starts from complete identical assignments, evaluates
relocations, picks the best accepted candidate, accepts ties and permits a best
rejected fallback. Its default limit is 20 completed steps. Engine-specific
original move ordering and construction tie handling remain intact; equal scores
or traces are not assumed. The UI marks differing final quality beside the ratio.
Larger workload optima are unknown and are explicitly labeled as such.

The default is three fresh processes per runtime, three warmup solves and five
measured solves per process, with alternating runtime order. Both use full scoring:
Java `EasyScoreCalculator` and a global Rust list constraint. Per-solve setup,
search and assignment/score/shadow validation are timed; process startup, input
parsing, one-time model/network/factory setup and output encoding are excluded.
Java maintains shadow state; Rust derives shadow views and snapshots lists during
transactions. Timings therefore include their different mechanisms and are not
an incremental-scoring comparison. Every sample's score, assignment, callback
count and elapsed time is saved, together with environment, source digest and
pinned dependency hashes. Pin/ownership/score validation is independent in Python;
each adapter also checks its returned owner/index/neighbor views.

This is a required-list, full-scoring comparison on these inputs. It does not
establish optional-list, mixed-variable, random-search, large-production-instance,
memory-use or overall Timefold replacement performance.
