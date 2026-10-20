# Local progress dashboard

Generate results and serve the visual overview:

```sh
bash scripts/check.sh
python3 scripts/differential_scores.py
python3 scripts/benchmark_scores.py
python3 scripts/differential_solver.py
python3 scripts/benchmark_solver.py
python3 scripts/benchmark_heuristics.py
python3 scripts/benchmark_construction.py
python3 scripts/benchmark_repair.py
python3 scripts/differential_lists.py
python3 scripts/benchmark_lists.py
python3 scripts/benchmark_list_repair.py
python3 scripts/benchmark_mixed.py
python3 scripts/benchmark_kopt.py
python3 scripts/differential_streams.py
python3 scripts/benchmark_streams.py
python3 scripts/benchmark_aggregates.py
python3 scripts/benchmark_sequences.py
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
Current qualification uses portable OpenJDK 25.0.2 and Maven Central artifacts.
Use each retained report configuration when reproducing a comparison; standalone
driver defaults can select different native policies.

## Native rewrite readiness

The top panel reports the seven original native capability requirements,
original timed workloads at Java parity and current comparison jobs. It uses an
explicit checklist rather than inventing another effort percentage. All requirements
must have current source-bound execution evidence; exact accepted native policies,
reference-unavailable outcomes and JVM/private-array boundaries remain separate
from raw method and Java agreement counts. Missing or altered evidence blocks
native readiness. The strict comparison index below remains unchanged.

`compat/native-targets.json` defines the fixed native requirements and exact
reviewed cohorts. `compat_report.py` trusts named passes only after validating the
actual full-target Cargo producer, source fingerprints and log checksum. Mere
presence of a test path, a focused log or a native-reference count is insufficient.
The manifest also freezes the 109 original workload names, cases, configurations
and input limits. Missing, duplicate or altered rows/configurations block native
readiness even when remaining timings meet parity. Timings and outputs may improve.
The browser receipt binds actual app/HTML/CSS/font bytes and desktop/mobile checks;
the separate live `native_completion_digest` changes when those inputs or policy
records change, without rekeying Rust execution or measurements.

Runtime qualification plus the final k-opt adaptations demonstrate 7/7 native
requirements, 109/109 measured time/quality targets and 18/18 current jobs. Source inventory is
4412/4855 with 0 pending and 443 excluded; strict index 95.809%. Native readiness
is bounded by the original contracts and measured inputs. It is not universal
Timefold/JVM equivalence, arbitrary-scale performance or an estimate of remaining
engineering effort. See [completion evidence](completion.md).

## Test origins and additional applications

`test_origins` partitions checksum/source-validated recorded Rust test identities
into exact upstream adaptations, additional native regressions and declared
external application tests. These identity counts are distinct from original
Java method coverage and expanded runtime invocations. Additional native checks
can reuse attributed fixtures; the category does not assert independent authorship.
Missing declared application tests stay unverified, and fixture hash validation
is separate from observed test execution.

`compat/additional-tests.json` records the exact public RealWorld snapshots,
approved manifests, source commit and application spike origin. Its four native
regressions test both original workloads' full score, trial/rollback and bounded
seeded solving. They have zero upstream-method credit. `compat/provenance.json`
and the original Community inventory remain unchanged.

The independent `scripts/benchmark_real_world.py` runner saves
`work/real_world-performance.json`. It is outside the original 18 comparison
jobs/109 timed targets and strict index. The dashboard shows Rust streams,
actual pinned Community streams and optional hand-written Java incremental
throughput separately. Qualification means a complete default matrix and
independent scoring validation, not speed or solution-quality parity. Sailing
can remain infeasible; hard/soft observations and missing whole-group moves
remain explicit. The separate application input hash includes the actual model,
public fixtures and runner/converter/Java code; unrelated original measurements
are reused without changing their keys or dates.

Run `python3 scripts/benchmark_real_world.py --java-modes streams incremental`
with both runtimes on one machine. See [application benchmark details](../benchmarks/real_world/README.md)
and [native model contract](real_world-model.md). The native UI clearly labels
its original acceptance scope, with additional application performance below.

## Strict comparison index

The strict comparison panel is a configurable progress index, separate from raw
upstream-method coverage and native readiness. Its default weights in `compat/completion.json` are feature coverage 40%,
behavior checks 20%, performance 30% and current evidence 10%. These are explicit
project settings, not measured estimates of implementation effort or time.

- Features: average the passing/in-scope ratio of each feature family. Explicit
  exclusions stay outside this component's scope, while the original Community
  coverage section retains its unchanged full denominator and exclusion counts.
- Behavior: four fixed comparison families, each scored by its weakest top-level
  or first-level `passed/total` check. Native-reference passes never replace Java
  comparison passes; Java errors and unsupported cases remain in the denominator.
- Performance: fourteen fixed benchmark families, each scored by its weakest
  comparable workload. Credit is `min(1, Java median × target / Rust median)`
  only with valid timing samples and equal-or-better solution quality or checked
  update results. Faster cases cannot compensate for a slower case, and timings
  without comparable quality evidence earn no credit. The user-selected default
  target is Java parity (`java_time_multiplier:1`). Comparisons retain their
  original policies/work counts and do not imply equal execution work.
- Evidence: current comparison reports/18, gated by successful Rust verification
  and clear failed/unverified adaptation status. Missing or invalid evidence does
  not count as complete.

The weighted sum is labelled a measured progress index. Stale measurements retain
last-demonstrated contributions and remain explicitly labelled previous evidence;
the current-evidence component and visible freshness counts track that debt. 100%
and the completion status require every tracked component/target and current
successful verification. The browser also requires a matching live source/model
fingerprint before showing completion. The index is not a guarantee of universal
Timefold equivalence, a fraction of remaining engineering work or an ETA.

The expandable calculation shows each family, its limiting check/workload, the
weights and remaining blockers. Policy and model source have a separate
`completion_digest` exposed by `/api/freshness`; changing them warns until the
report is regenerated, without invalidating native execution or benchmark keys.
After editing the policy, regenerate with
`python3 scripts/compat_report.py --test-output work/rust-tests.log`.

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
  Rules can match a module `path_prefix` or one exact pinned `method_id`; method
  exclusions do not hide other tests in the same class. Eight JVM-only methods
  formerly pending now use exact exclusions. The final two k-opt methods have
  passing native active-descriptor adaptations, with Java sentinel/spare-storage
  translation documented in the provenance ledger. Exclusions earn no pass credit.

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

Verification uses separate dependency hashes for each Rust target. Public local
trait adapters are dependencies when the target references both the receiver type
and the trait or one of its members; blanket implementations match any receiver.
Factory/helper dependencies participate in the same scan. Inherent methods,
internal traits, and foreign traits such as operators and formatting remain tied
to their model types. Associated items are qualified by receiver so a common name
such as `Error` does not connect unrelated modules. This is a selective development
heuristic, not compiler name resolution; `--force` remains available for a deliberate
full run. Successful evidence is never rewritten to fit a changed hash.

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

The cyclic k-opt section compares complete single-route Manhattan-cycle problems
using public Java configuration and two native strategies. Both native strategies
change exactly k undirected edges: window search shuffles up to 4,096 enumerated
candidates; sampled search proposes random cuts and path reconnections on demand.
The latter allows at most 10,000 proposals per step by default, discards proposals
that retain a cut edge, and derives a seed for each completed step. Repeated moves
are possible. Java uses its public random selector. Work, sampling distributions,
PRNGs and score-call accounting differ between implementations.

All three searches configure first-accepted hill climbing, 30 steps, at most 10,000
score evaluations and seed 20261008. Each native strategy is paired with Java on
32 original eight-value routes and two k ranges (2–3 and 2–4), giving 64 comparisons
per strategy and 128 paired comparisons overall. These reuse the same routes and
Java results; they are not 128 independent inputs. Every small-case optimum is
independently enumerated. Eight timing cards cover both native strategies, both
k ranges and 12/16 values; larger-case optima are unknown. Quality and score counts
accompany times, with differing scores clearly labelled. No method-coverage credit
or general solver speed claim follows from these original benchmarks.

K-opt timings use three warmups and five measured solves in each of three fresh
processes, with runtime order alternating. Per-solve setup, full scoring, search
and result validation are timed; parsing, model/network/factory construction,
startup and output encoding are excluded. Four separate exact-3/exact-4 probes
retain the pinned Java observation of returning after its initial score call.
They have no speed ratios. Reproduce this section with
`python3 scripts/benchmark_kopt.py`; it checks all returned assignments and scores,
non-regression, candidate work and score budgets before saving the evidence.

The exact-search section measures ten workloads: brute force and four
branch-and-bound exploration orders, each at 6 and 8 variables with 3 values
(729 or 6,561 complete assignments). Brute force preserves the original mixed-sign
preference fixtures. Branch workloads use nonpositive preferences and identical
inputs for all four orders: depth first, breadth first, score first and optimistic
bound first. Java uses BRANCH_AND_BOUND with an ONLY_DOWN initializing-score trend;
native search uses the partial full score as an admissible bound. Python checks
this bound over complete small prefix trees. Every sample checksum and returned
assignment is checked against independently enumerated optima. This establishes
optimal scores on these fixtures, not identical bounds, traces or node counts.
Native branch search counts both partial and complete nodes; brute force enumerates
complete assignments. All runs recompute full scores; no incremental scoring.

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

The heuristic section compares eleven configurations on 32- and 96-variable graph
problems, with 50 completed steps at most: hill climbing, diversified late
acceptance, fixed/ratio entity tabu, move/value tabu, combined late acceptance/tabu,
strategic oscillation using last-step or best-score references, step counting and
Great Deluge. Both engines use original-order change moves, best-accepted selection
and first ties. Native rejected fallback chooses highest score; Java retains its
configured public forager behavior. Strategic oscillation adds entity tabu with
tenure two and global-best aspiration. Its documented native exact-reference
ordering difference remains visible in the separate correctness comparison;
these timings do not claim identical selected moves or fallback behavior.

Both start with exactly the supplied complete assignment and recalculate the full
score for every candidate. This does not measure the Rust incremental network
against Java incremental scoring. Setup and returned-result validation are timed;
process startup, parsing and one-time model/factory/network setup are excluded.
Default sampling uses three fresh processes, five warmup solves and seven measured
solves per process, alternating runtime order by workload/process.

The report records each sample's time, score, assignment and score-call count.
Python checks domain membership, pins, score recomputation and non-regression from
the initial score for every sample. The 22 workloads use a planted optimum of
`0hard/0soft`; reaching a step limit can leave them infeasible. The UI therefore
shows scores and feasibility beside runtime, without treating faster execution as
better quality. It labels differing scores explicitly. The recorded ratios apply
only to the chosen algorithm, budget, full-scoring implementation and inputs.
These shared-machine measurements do not establish statistical significance.

An additional 352 original checks (32 small inputs x eleven configurations) compare
valid results and scores, with independent exhaustive optima recorded per case.
These checks contribute no upstream compatibility credit. Validity, Java/Rust score
agreement and optima are separate counts, also grouped by algorithm in JSON.
The dashboard derives its configuration count from measured workloads, avoiding a
stale static label. Run `python3 scripts/benchmark_heuristics.py` to regenerate the
combined quality/timing artifact; optional flags set completed steps, warmup solves,
samples and process forks. Full conditions and raw data are included in the
downloadable JSON and the measurement-notes disclosure.


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

Consecutive-sequence correctness appears as a separate card in the generated
comparison section: 64 ordinary public Java/native/Python cases, 4,689 observed
states, and one explicitly divergent int-overflow boundary outside agreement
credit. Tie ordering may vary only within equal-index items; all endpoint identities
must agree with each runtime's observed items. Native stable ID ordering is checked
separately. These checks add no upstream-method credit and no timing workload.
The adapted source-method rows separately credit the twenty full native test
adaptations, whose immutable snapshot and identity adjustments are in provenance.

## Consecutive-sequence observations

`python3 scripts/benchmark_sequences.py` measures a standalone collector slot
replacement and its inverse update, including complete result observation after
both operations. Four workloads use dense consecutive or fragmented indices with
64/512 distinct items. Java uses public collector handles and live chains; Rust
uses SequenceCollector and owned immutable snapshots. Both adapters copy logical
item membership/index/rank and all sequence/break numeric metadata to arrays,
validate each complete array against independent Python, and consume checksums.
Equal-index identity ordering is canonicalized away only in this benchmark;
the separate differential checks endpoint identities within each runtime's order.

Setup, parsing, startup and output formatting are excluded. Finish/materialization,
array allocation, validation and inverse updates are included. This does not isolate
update-only performance and does not time graph checkpoints, scoring or search.
Native finish currently rebuilds chain boundaries; Java exposes maintained chains.
Validation/allocation costs differ. Each sample separately checks a committed update.
All 64 seeded small cases (duplicates, empty slots, no-ops and removals included)
and every timed state must pass; failures abort and are not discarded. Integer
indices are bounded; the known int-overflow discrepancy remains separately reported.

Defaults: 200 update/restore pairs per batch, 5 warmup batches, 7 measured batches,
3 fresh processes per runtime/workload, alternating runtime order. Report retains
all samples, environment and source fingerprint. Ranges are batch averages, not
latency percentiles; no CPU isolation, JMH or statistical significance claim.
This adds four timing cards and no upstream method credit.

Connected-range correctness is a separate generated-comparison entry. It shows
native reference passes alongside Java agreement, differing cases and errors;
shared-object removal, empty-range flags and integer overflow are explicit boundaries.
No failing case is removed from the report. The distinct-positive cohort is identified
separately. Full cases and per-state differences are retained in the downloadable
report, and these generated cases add no upstream-method coverage or timing card.

Load-balance comparison is included in the stream dataset: 101 native reference
passes / 101 cases, 8,290 states, 97 full Java agreements, four differing cases and
zero Java errors. All 96 bounded cases agree. The card retains large-number load
and unfairness differences, including overflow and cancellation. Six significant
digits and numeric trailing-zero normalization are explicit; no Java decimal-scale
or live-map parity. Paired inputs use consistent initial loads per key; native
inconsistent updates are tested separately as atomic rejection. No new timing card
or generated-case upstream-method credit. The existing 37 timing workloads remain.

Calendar-period comparison is part of the stream dataset: 227/227 cases and 8,771
observations agree with Java and independent Python, including 120 expected overflow
rejections. Its separate card labels rejection agreement explicitly and explains
first-error truncation, component arithmetic and subtraction-before-addition updates.
The full generated inputs and runtime output prefixes remain in the downloadable
report. No additional timing card or generated-case method credit is inferred.


The separate score-trend diagnostic compares 512 original exact-search cases
(64 pinned/required graph fixtures x four branch orders x two trend declarations).
It validates Community public configuration and native ordered bounded phases
against independent optima, including signed soft preferences with ONLY_DOWN/ANY.
This agreement count adds no timing workloads or adapted-method credit. Run
`python3 scripts/score_trend_checks.py` for that diagnostic alone; the standard
solver differential includes it and saves all inputs/results.

## Bounded repair comparisons

`python3 scripts/benchmark_repair.py` produces `work/repair-performance.json`,
shown in the separate repair section. The original fixtures compare basic required
variables with full scoring, using fixed half or whole ruined groups, seed 20261222,
and eight repairs. Every Java solve must expose eight public move evaluations;
every native solve must complete eight reconstruction attempts. No-op reconstructions
count as attempts, while native accepted-step history advances only on changed moves.

All initial variables are zero, with maximum graph conflicts and minimum per-variable
preferences. Thus every complete assignment scores at least as well as the initial
one. A step-counting threshold period of nine retains that initial threshold throughout the
bounded run, avoiding rejected-candidate retries. The parser rejects missing or
excess repair work rather than reporting its timing as a successful sample.
Java's completed-step limit and native attempt limit remain distinct APIs. Sampling,
reconstruction order and per-repair scoring work differ; equal seeds do not imply
identical random traces. Java cannot customize its embedded repair construction.

Every output is independently checked for domains, completeness, recomputed score,
initial non-regression and the independently known optimum bound. Thirty-two small
original conflict/preference cases per group mode use exhaustive Python optima;
32/96-variable timing cases have planted zero optima and nonpositive contributions.
Valid lower-quality results remain in the report. These checks add no adapted-source
method credit, and do not establish list, mixed, optional or stream repair parity.

Each timing includes per-solve setup, initial state copying, search and final solution
validation. Parsing, model/network/factory construction, process startup, registry
setup, post-solve metric observation and output are excluded. Java's default metric
collection remains enabled during solving. The adapter re-registers its public
move-count gauge after solving, keeps the solver reachable through the observation,
and clears the process-owned registry between solves. Five warmups precede seven
samples in each of three fresh processes, alternating runtime order. Raw assignments,
scores, score-call counts, repair counts and time ranges remain downloadable.
Ratios describe these bounded configurations, not equal scoring work or general
solver speed. Shared-host allocation, JIT and GC effects may affect the ranges.

## Required and optional list-repair comparisons

`python3 scripts/benchmark_list_repair.py` writes `work/list-repair-performance.json`.
The separate dashboard section compares required/optional homogeneous lists using
full scoring, no pins or receiver restrictions, eight repairs and seed20261225.
Requested ruined-count ranges are 1..ceil(n/2) or 1..n. Actual sampled counts and
eligibility differ; optional missing values are eligible in native repair. No PRNG,
trace, weighted union, mixed-model or incremental-stream equivalence is claimed.

Original fixtures penalize owner costs, squared capacity excess, ascending adjacent
IDs and optional missing values, all in the soft level with hard zero. Owner zero
has capacity zero and worst unary costs; all values start there in ascending order.
This realizes the worst score over valid assignments: total squared excess is at
most n², adjacency penalties at most 2(n-1), and each value's owner/missing cost is
at most its owner-zero cost. A step-counting threshold period longer than the run
therefore accepts every completed proposal. Java public move counts and native
started/complete reconstructions must equal the requested repairs for measured runs.
Counts, initial non-regression, unique in-range IDs, required completeness, optional
omissions and scores are independently checked. The adapters also validate returned
assigned owner/index metadata. All raw measurements and assignments remain available.

Thirty-two small fixtures run under both group limits. An independent owner/omission
and permutation enumeration establishes each optimum; validity, equality and optimum
attainment are separate counts. Larger24/48-value fixtures have planted zero optima
(all costs nonnegative), including real optional omissions. Quality differences
remain visible beside timing ratios. Native full constraint callbacks and Java easy
score calls are reported; work per repair is not assumed equal.

Fixed-minimum probes remain separate from the timings. In the three-value optional
probe, Java completes one of eight requested repairs after retaining two assigned
values when minimum=maximum=3; native repair completes eight by including eligible
missing values. With minimum1, both finish eight. Required controls also finish
eight. These are observed public-API results, not an inference about Java internals
or additional test-method credit. The dashboard displays requested and actual counts.

Timing includes per-solve solver/state setup, initialization, search and validation;
excludes process startup, parsing, model/network/factory creation, registry setup,
final meter reads and formatting. Java metric collection runs during measurement.
Three fresh processes per runtime, five warmups and seven samples per process,
with alternating runtime order and all raw samples. Shared-host ranges include
allocation and GC effects. Original benchmark fixtures add no adapted-method credit.

## Mixed construction and search comparisons

The mixed-planning section measures two warm-start and two cold-start workloads
with both Dense and Stream scoring against one shared Java sample group per case.
Cold runs clear mutable depot choices and route suffixes, preserve whole/prefix
pins, then construct and improve a solution under finite limits. Every repeated
solve must reach the independently enumerated small optimum or match/improve the
larger feasible certificate; the latter is not a proof of optimality. The report
retains the actual partial input, per-phase native evaluations and separate
construction placements. Partial input scores record hard/soft objective levels,
not an initialized complete solution. Java logical phase work is unobserved. Cold timing
includes input preparation, construction and search; warm timing retains its
existing scope. Search policies differ, so ratios do not imply equal work or
general scalability. These original workloads earn no upstream-method credit.

The native benchmark defaults to an explicit component-first search followed by
joint refinement under shared per-case limits; the original joint-only profile
remains selectable. Cold component-refinement scoring computes missing customers
as the fixed registered count minus the sum of route lengths, folding that term
into per-owner route constraints and charging the constant once to owner zero.
The objective is unchanged for partial and complete assignments. Complete value
source rows and shadow metadata remain retained; constraint grouping, explanations
and physical callback counts differ from the original per-value penalties. The
report records the selected profile and penalty formulation for both backends.

The mixed benchmark also accepts `--native-stream-graph-policy owner-links` or
`--native-stream-graph-policy owner-links-fused`. The default remains
`ordinary-join`. This optional application declaration uses
shared fixed owner relationships and a fixed tracked scoring terminal; complete
planning rows, shadows, match payloads and scores remain inspectable. Actual match
IDs, callback ownership and publication boundaries differ: a changed link publishes
its final pair once per director batch. Physical scoring calls can decrease.
This is an explicit graph declaration change, not an ordinary-join implementation
speedup or a claim of equal work with Java. Fused v2 directly projects and scores
each link in one tracked node, retaining the same full payloads and IDs as v1.
Projection/scoring interleave and ownership, cloning and error timing can differ.
Both prior declarations remain selectable.

When selected, all four original warm/cold inputs retain both native backends,
original quality floors and budgets. Each fork rotates Java, Dense, ordinary
Stream and owner-link Stream, adding fused Stream as a fifth group only when
selected; one Java group is shared between the two native
comparison rows. Every ordinary Stream sample and full observation is retained
in `ordinary_stream_controls`. Both policies independently validate assignments
and scores, and every output field must match except elapsed time, declared graph
metadata and actual physical scoring calls. Fused runs additionally retain every
v1 sample and full observation in `owner_link_controls`; v1/v2 physical scoring
counts must match, along with all other output fields except declaration metadata
and elapsed time. Unknown fields remain compared.
The dashboard identifies the selected declaration, shows its ordinary-join
control medians and physical call count, and keeps Java parity as the completion
target. These bounded workloads establish neither general scalability nor memory
savings.

## Selective verification during development

`bash scripts/check.sh` reuses successful jobs with matching inputs. Rust tests
are cached per target, Python checks per test file, and each of the 18 comparison
runners has its own key. Native dependencies are selected by a module-level source
scan of imports, referenced public symbols and out-of-line implementations; local
modules and literal include fixtures are tracked. A target hashes only the crate
facade declarations/reexports it references, plus remaining root code and crate
attributes. Adding an unrelated public export no longer invalidates every target;
unit tests, linting and doctests still include the complete facade. Library inputs
for integration tests, example tests and measurements omit exact `#[cfg(test)]`
items and their exclusively test-only modules. Runtime code, shared support files
and literal contents remain inputs. A test target's own test-only code is retained.
Doctests exclude library unit-test fixtures but retain documentation contents.
Compound/unknown conditional attributes remain conservative. This is a development heuristic,
not compiler-proven dependency analysis. Shared types can still select many modules.
Python imports and Java adapter references are followed separately. Dashboard,
provenance, unrelated tests and unrelated adapters do not invalidate Rust execution.

Direct benchmark/differential commands also reuse successful matching reports.
Command arguments, toolchain/environment identity and output checksums participate
in reuse. Original measurement dates remain unchanged. Missing/modified outputs,
failed runs and changes during execution are never reused. Cache files live in
ignored `work/verification/`; deleting them requests fresh execution.

Use `bash scripts/check.sh --force` or add `--force` to an individual comparison
command when fresh execution is needed, including final-phase validation. Reports
use each measurement's own source key. Evidence from the former global-key scheme
stays stale until that job is next run; migrating the keys does not refresh timings.

## Checked list registry comparison

The mixed runner also accepts `--native-list-registry-policy ordinal` (default
`eq-only`). It selects checked list ordinals for both native backends, including
fresh cold-model reconstruction inside solve timing; Basic registries remain
Eq-only. Two additional runtime groups retain complete same-graph Eq-only Dense
and Stream controls. Registry comparisons require identical full observations
and actual physical scoring calls except declared registry metadata and elapsed
time. All original graph controls and the single Java group per case/fork remain.
The dashboard shows the selected registry and its Eq-only control median; slower
controls or regressions stay visible. Stable equality-consistent ordinal keys are
an explicit model-construction obligation, not an inferred property of all values.

## Checked-sum deferred replacement evidence

The aggregate panel retains the original six inputs, 96 full validation cases,
200 iterations, 3 warmups, 5 samples and 3 Java/native forks. Qualification 299 changes
native storage work by composing one pending checked replacement or cancelling
its inverse. Every public arithmetic/membership check and full trial/restore/commit
observation remains required. All six fresh workloads meet measured Java parity.
The completion target remains Java multiplier 1; no UI or model weighting changed.

The native comparison and separate public-API controls are recorded in
compat/observations/sum-replacement-coalescing.json. Alternating edits of independent
captures are slower in the bounded controls; deferred work can move onto unrelated
edits/equality and storage grows. No universal collector or solver speed claim.
Other 17 raw reports are unchanged, with relevant stale inputs visibly retained.
Backend-only same-contract report refresh reuses the 296 desktop/mobile rendering evidence
plus actual current HTTP/source/model/freshness checks. The older browser's report
hash remains historical; it is not rebound to the new data.

## Remote refresh of the remaining measured gaps

Qualification 300 deliberately refreshes current stream and repair evidence on
runner-a, using both Rust and Java on the same CPU and all original settings,
workloads, quality fields and native controls. Stream meets parity in 8/8 timed
rows; repair in 3/4. Repair validation retains 64 valid cases but only 36 equal scores
under the documented different sampling policies. All four timed repair scores
are equal. The remaining whole 96 repair timing gap stays visible.

Both rejected micro-optimization trials remain separate from current evidence.
Restored native source and qualification 299 are identical, so its successful Rust
verification is reused. Other 16 raw reports remain unchanged. The completion
index changes with the fresh observations; weights, Java multiplier 1, source-method
denominator, pending/excluded cases and UI rendering contract stay unchanged.
