# Local progress dashboard

Generate results and serve the visual overview:

```sh
bash scripts/check.sh
python3 scripts/differential_scores.py
python3 scripts/benchmark_scores.py
python3 scripts/differential_solver.py
python3 scripts/benchmark_solver.py
python3 scripts/benchmark_heuristics.py
python3 scripts/differential_lists.py
python3 scripts/benchmark_lists.py
python3 scripts/differential_streams.py
python3 scripts/benchmark_streams.py
python3 scripts/benchmark_pillars.py
python3 scripts/benchmark_concurrent.py
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
annotated Java test methods**. Read the current verified count and percentage from
the generated report. Its denominator includes explicit JVM integration exclusions;
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

The same solver comparison command additionally records `termination_checks`:
219 original deterministic complete required-variable graph cases, seed 20261119.
Java public configuration composes step, unimproved-step, score-target and
feasibility phase limits with OR. Both runtimes use original-order best-accepted
hill climbing, first ties and best-rejected fallback. Independent Python neighbor
enumeration checks final scores and exact assignments, and separately checks both
Rust backends' completed-step, stagnation and acceptance counts and stop reasons
(398 reference steps across the cases). Java internal counters, traces and stop
reasons are not inferred. Fifteen variations of a delayed-improvement fixture
(found using seed 20261120) distinguish limits by returned quality after worsening
and recovery. These cases preserve initial assignments, unlike the exact-search
cases above. Raw inputs, reference traces and outputs are saved separately in
`work/oracle/termination-cases.json`. The dashboard shows the count separately,
with inherited measurement freshness and explicit mismatches; it adds no upstream
coverage or timing claim. List/mixed search and randomized acceptance are outside
this comparison.

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

Run `python3 scripts/differential_lists.py` for 384 valid cases comparing
Java public `MoveTester` candidate/undo/commit states to native transactions and
an independent Python reference. Owner, index and previous/next shadows are
checked at each observed state. Seeds 20261003 and 20261124; source digests, artifact hashes,
inputs, outputs and failures are retained. The dashboard renders this as a
separate evidence row and marks it stale when measured sources change.
It uses one optional list variable; pins and exact list solver results are covered
by native regressions, not this Java comparison. No list performance claim or
additional upstream coverage credit. See `list-planning-contract.md`.

## List-solving comparison

`python3 scripts/benchmark_lists.py` adds required-list construction, relocation hill
climbing and two-opt with identical routing inputs, Manhattan round-trip distance and capacity
penalties. The 192 original small validation cases (96 unrestricted and 96 with
entity-specific ranges; 64 per phase) cover empty ranges, complete or
partial initial lists, full pins and pinned prefixes. Python independently
checks every output and enumerates complete owner/permutation assignments to
record exact optima. Construction retains existing values in their original
routes and relative order; hill climbing must not regress from the initial score.
Validity, Java/Rust score agreement and optimum attainment are separate counts.
These generated cases grant no additional upstream test-method coverage.

Six measured workloads use 16 or 32 customers, four depots, capacity n/4 and
seed 20261006; the additional range cases use seed 20261007. Larger timing
workloads remain unrestricted. Construction evaluates insertions without early picking and runs
to completion. Hill climbing starts from complete identical assignments, evaluates
relocations, picks the best accepted candidate, accepts ties and permits a best
rejected fallback. Its default limit is 20 completed steps. Engine-specific
original move ordering and construction tie handling remain intact; equal scores
or traces are not assumed. The UI marks differing final quality beside the ratio.
Two-opt instead uses first-accepted hill climbing with cross-owner tail swaps
and within-route reversals, the same 20-step limit, and a 10,000 score-evaluation
budget. Java uses the public preview TwoOptListMoveProvider with NEIGHBORHOODS;
Rust uses seeded SplitMix64 candidate-window ordering (seed 20261006, window
1,024, capped by remaining evaluations). Its finite candidate generator is
unchanged; neither PRNG nor sampling matches Java. Sampling order and score accounting
differ; these are bounded quality/time comparisons, not equal-work measurements.
The small two-opt corpus reuses the 64 complete hill-climbing inputs, including
pins/ranges, without changing either earlier phase corpus. Per-phase equality and
optimum counts are displayed separately. Enumerated optima cover the full legal
routing space; the restricted two-opt neighborhood need not reach that space.
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

## Incremental-stream comparison

`python3 scripts/differential_streams.py` checks 128 original cases, comprising
2,899 scoring states with required variables, partial assignments, pins, duplicate
and self edges, signed weights, cancellation, committed edits and trial/undo edits.
The seed is 20261017. Indexed edge/job joins penalize equal endpoint values;
jobs grouped by assigned value contribute `sum * abs(sum)` to the soft score.
The nonlinear contribution makes grouping observable in the final score.
Every ordinary hard/soft score is independently computed in Python. Native exact
conflict IDs and nonzero group sums are also checked, including after rollback.

The Java adapter uses Community `SolutionManager.update()` on each state, while
Rust uses persistent graph transactions. `SolutionManager.analyze()` is
Enterprise-only in the pinned release and is not used. Java outputs therefore
contain scores only; there is no Java match-set or structural-score comparison.
These are correctness observations, not Java incremental-edit timings.

`python3 scripts/benchmark_streams.py` separately measures incremental scoring
inside both engines' local search. It uses the same join/group constraints above,
32/96-variable inputs, hill climbing and tabu tenure two, original-order change
moves, best accepted selection, first ties, best rejected fallback and at most
50 completed steps. Defaults: three fresh processes, five warmup solves and seven
measured solves per process, with alternating runtime order. Both engines receive
the same complete initial assignment. Per-solve setup, search and independent
result validation are timed. Rust builds graph topology and loads fixed edges
each solve; Java reuses a prebuilt `SolverFactory` and builds a solver each solve.
Process startup, parsing, factory creation and output are excluded. This measures
these end-to-end entry points, not an isolated tuple-update rate.

The report retains every assignment, timing, final score and terminal weighting
callback count. Callback counts can differ because the engines propagate and
coalesce updates differently; they are not counts of candidate moves. Python
checks domain/pin validity, recomputes scores and rejects regression from the
initial score. All four timing workloads have a planted `0hard/0soft` optimum;
budget exhaustion can still leave infeasible results. Timings and quality appear
together. The workload differs from the earlier full-scoring heuristic section,
so comparing their times does not isolate the benefit of incremental scoring.

An additional 64 small search checks record independent exact optima, validity and
Java/Rust score agreement separately. Neither these checks nor the stream edit
cases add upstream method coverage. Both saved artifacts use the same freshness,
environment and pinned-dependency reporting as the earlier comparisons.


The list-move oracle now reads Rust's cached stream value views, so its existing
256 comparisons also validate ListStreamScoreDirector owner/index/neighbor
propagation and undo. Cached entity rows and a native position score are checked
inside the adapter; this is not a Java/Rust score comparison. The list-solving
timing cards still measure scoped full-scoring adapters. The new native
stream_list_routes example is a correctness/quality example, not a timing card.


## Native stream hotspot profiling

Run `python3 scripts/profile_streams.py --variables 96 --algorithm hill --steps 50`
to instrument an isolated copy of the native crate. It saves
`work/native-stream-profile.json` with the source fingerprint, original workload,
independently checked result, node-application/checkpoint/capture/undo timings and
AVL node allocation count. The default is one solve with no warmup. Compilation
uses a separate target directory, and the temporary source copy is removed.

Instrumentation adds substantial overhead. These numbers locate expensive native
operations; they are neither Java comparisons nor benchmark timing samples, and
do not appear as performance results in the dashboard. Source-marker changes
fail closed so instrumentation must be reviewed when relevant internals change.
Run this command separately from timed benchmarks. It grants no upstream coverage.


The stream comparison also runs original standalone average-collector checks via
public Community value handles and native try methods. The dashboard lists these
137 cases and their observations separately from the 128 stream-score cases. Python
checks exact double bits plus expected insertion/removal sum overflows and
replacement-difference overflows, including wrapping replacement totals. Cases
stop at the first rejection; no Java rollback behavior is inferred. Full inputs
and outputs are retained under average_collector_checks in stream-differential.json.
This evidence grants no additional upstream-method credit or average timing claim.


The stream evidence also contains duration_collector_checks: 288 original
signed-duration sum/average sequences, compared exactly through Community public
handles and native collectors against Python. Update and finisher overflow are
distinguished; 7,650 observations include 142 expected errors. The dashboard shows
these counts and their seed separately from average and stream-score cases.
No post-error Java state, additional upstream credit, or duration-specific timing
is claimed. Full inputs and outputs are retained in the downloadable report.


big_number_collector_checks adds 304 original arbitrary-precision sum/average
sequences and 16,298 observations to the stream evidence. Each integer output,
decimal coefficient, and decimal scale is checked exactly against Java and Python,
including half-even ties and precision retained after removal. Its seed and counts
are separate from upstream coverage. Inputs/results are retained in the report;
no arbitrary-precision timing or general decimal score compatibility is claimed.


Mixed score/move evidence is a separate `mixed_stream_checks` supplement under
stream-differential.json. It contains 128 original cases, 2,378 states and 3,611
primitive basic/list operations (seed 20261108). Java uses public composed moves
and full score updates; Rust uses mixed transactional streams. Independent Python
checks ordinary scores, completeness, ordered assignments and owner/index/previous/
next views through candidate, undo and commit states. Basic variables are required;
list values may remain unassigned. Basic pins, partial basics, non-dense identities,
empty registries and repeated joint writes are included. Source freshness follows
the parent stream report. The existing comparison section displays this supplement
without adding to upstream coverage or mixing it into benchmark counts. Full case
inputs, both runtime outputs and Python-derived assignments remain downloadable.
No mixed solver-search, Java match-analysis or mixed timing claim follows.

Subsystem grouping includes the core `impl/bavet/` tests in Constraint streams,
alongside `impl/score/stream/`. Earlier reports put the Bavet engine tests under
Core utilities & configuration; the correction moves methods between subsystem
denominators without changing overall coverage.

Ordered/overlap join comparison evidence is included under `ordered_join_checks` in the
stream differential report and displayed alongside the other original checks.
It retains all inputs and runtime outputs for 153 cases / 10,155 states, seed
20261116. This includes strict overlap joins with and without equality partitions,
and touching, zero-length, reversed and extreme endpoints. Independent Python pairs check Java/Rust match counts and weighted
scores plus native exact IDs/payloads. Existing benchmark cards do not measure
these operators. This evidence does not add upstream test-method coverage.


Equality-only existence evidence is retained under `equality_existence_checks` in
the stream comparison report, with all original inputs and both runtime outputs.
The dashboard shows 54 cases / 4,834 states, seed 20261118, covering Exists/NotExists
with zero, one and two equality keys. Independent Python enumeration validates
retained counts, signed/zero-weight scores and exact native left IDs/payloads.
Community recalculates scores through public APIs; no Java match-analysis or
incremental-notification claim. Existing timing workloads do not measure this
specialization or complement. Generated cases earn no upstream method credit.


Mixed-search quality evidence is nested under solver_differential.mixed_solver_checks
and refreshed by differential_solver.py. The dashboard shows valid case counts
separately from per-engine optimum counts: current 64 valid cases, 64 Rust joint
exact optima, 39 Community BRUTE_FORCE optima and 63 optima for each native local
backend. Python independently enumerates 32,116 assignments. This is a quality
comparison of different starts/strategies, without mixed timing or Java trace
claims. It earns no upstream method credit. Raw evidence is saved in
work/oracle/mixed-solver-cases.json.


List-move evidence now totals 384 cases / 1,348 operations. The original 256 cases
(seed 20261003) are preserved; 128 span-relocation/reversal cases use seed 20261124.
Java public Moves.change(Range, ElementPosition, reverse), native cached list-stream
views and Python slicing agree on candidate/undo/commit states and all four derived
shadows. The report records sublist_seed separately. This is move evidence; existing
list/mixed search timings retain their original neighborhoods.


## Step-counting search evidence

The solver comparison also records solver_differential.step_counting_checks:
600 original cases with seed 20261218, five counting modes, three periods and
first/best accepted selection. Java and Rust final assignments/scores are compared
to independent Python enumeration; Python also verifies Rust scoped/stream counters
and termination reasons. This record inherits the enclosing source fingerprint and
adds no adapted-method credit. Its failures make differential_solver.py fail.

The full-scoring heuristic benchmark now includes STEP mode, period 7, at 32 and
96 variables. It uses the same 50-step cap, first-tie best-accepted selection and
best-rejected fallback as its existing workloads. Every returned assignment is
independently rescored. Six workloads now occupy this section (21 across the full
dashboard), with quality beside each timing. Small validation expands to 96 runs,
including 32 step-counting cases; validity, equal scores and optimality are separate.
Other counting modes and incremental step-counting timings are not benchmarked.


## Concurrent shadow transactions

`benchmark_concurrent.py` measures temporary change/swap moves, all nine derived
fields, and undo for fully assigned schedules with disjoint shared-start groups.
Rust uses ListProblem plus CachedSchedule; Java uses public MoveTester with the
attributed concurrent domain. This measures the evaluator used by cached scoring,
not the stream graph or either score engine. Model setup and Python reference
computation are outside timing; move decoding/construction, cache snapshots,
full output copying/checking, undo and checksums are inside. Framework costs differ.

All 64 seeded validation cases and every timing sample must match independent
Python fields after trial, undo and a separately checked committed move. Invalid
no-op positions are rejected before dispatch, matching Java's distinct-position
requirement. No runtime-error case is silently removed. Fully assigned timing
inputs leave the separate partial-group comparison and its 21 known Java errors
unchanged. Default measurements use 200 transactions per batch, five warmup batches,
seven measured batches and three fresh processes in alternating runtime order.
The four workloads cover swap and relocate with 16 or 64 visits. Observed ranges
are batch averages, not latency percentiles; there is no whole-solver speed claim.
