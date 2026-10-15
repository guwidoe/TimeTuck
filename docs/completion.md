# Completion evidence and remaining gaps

The original target is native Rust planning and optimization behavior, using
pinned Timefold Community tests and public APIs as specifications. Java API/ABI,
reflection, framework integration and Enterprise were excluded in the initial
[scope](compatibility.md#target). These boundaries have not changed.

The implementation now has executable evidence for the following requirements.
Each row describes verified behavior within its contract, rather than a promise
about every problem size, search trace or finite-budget optimum.

| Requirement | Executable evidence | Limits |
| --- | --- | --- |
| Independent native solver, Community reference and attribution | Rust-only [crate](../crates/solver/Cargo.toml), pinned [reference](../compat/upstream.json), [provenance](../compat/provenance.json), notices and report validation | No Java runtime in the production library; no formal clean-room or whole-Timefold equivalence claim. Crates retain `publish = false`. |
| Scores and independently checked arithmetic | Integer/decimal fixed and bendable score tests; [score comparisons](../scripts/differential_scores.py) and native checked-scoring regressions | Overflow, exact decimal and initialization policies are specified in the [score contract](score-contract.md). Cross-runtime transcendental rounding is not established. |
| Basic/list/mixed models, moves, pins, ranges and rollback | [Temporary move tests](../crates/solver/tests/temporary_moves.rs), independent sublist/pillar/group successor tests, physical k-opt execution and exact search | Equality/explicit IDs define native identity. Private Java descriptor arrays and reflection are not native APIs. |
| Construction, heuristic improvement, repair and exact optimization | [Search contract](incremental-search-contract.md), independently enumerated optima, retained-best/budget-prefix tests, [branch-and-bound](../crates/solver/tests/branch_and_bound.rs) | Heuristics retain bounded best results; only exhausted exact search with valid bounds proves optimality. Seeds do not imply identical Java traces. |
| Incremental scoring, constraint streams, collectors and derived values | Transactional graph/collector tests, full recomputation checks, [stream comparisons](../scripts/differential_streams.py), dependency/cycle recovery tests | Explicit native policies and observed Java discrepancies below remain visible. User callback side effects and allocation aborts are outside rollback guarantees. |
| Configured applications and solver services | Owned XML/model factories, live daemon changes, cancellation and manager tests; [roster file workflow](owned-roster-workflow.md), [mixed cold-start qualification](../crates/solver/tests/mixed_benchmark_regression.rs), canonical solution persistence | Native typed bindings replace Java frameworks. Scheduling/time limits are cooperative. Larger-case optimum and arbitrary-scale quality are not proved. |
| Visual test and performance dashboard | [Report pipeline](dashboard.md), desktop/mobile rendering evidence, actual native/Community timings with quality, work, timestamps and input hashes | Method coverage uses statically discovered annotated source methods, not all JUnit runtime invocations. Stale measurements remain stale until rerun. |

The latest integrated checkpoint has **8,415 passing Rust tests and 290 doctests**,
source digest `effcc3e56aa6341264984a176ee9cf039f13a11a136a6bf0a03c72735dac6f4b`. The final `bash scripts/check.sh` passed
on runner-a in 271.34 seconds. The first worker check also passed in
841.00 seconds, but its metadata import correctly refused
the worker-local vendoring fingerprint. Equivalent dependency configuration was
moved to an isolated Cargo home, and checks actually rerun with compiled artifacts
reused. The first result was archived, not relabeled. Actual Rust output and checksum metadata were
collected unchanged, verified against the matching local source, and used for the
report. No duplicate local full gate or transplanted verification-cache keys.
The report remains the authority for later counts and freshness.

The applications establish concrete quality criteria: both roster backends solve
128 initially unassigned shifts to the planted zero-penalty certificate; mixed
cold construction reaches the independently enumerated small optimum and beats
the larger feasible certificate while preserving pins. These results establish
bounded application behavior, rather than universal optimality or performance.

The inventory has **4,409/4,855 passing adapted methods (90.814%), 11 pending,
435 excluded and zero failed/unverified**. The percentage counts statically
discovered annotated source methods, not all JUnit runtime invocations or overall
project completion. Even clearing all 11 pending methods would give only 91.04%
with the existing exclusions; exclusions remain visible in the denominator.
The 11 pending methods include nine JVM/loading/framework seams and two exact
private k-opt endpoint/partner-array assertions. They are not evidence of eleven
missing native algorithms, and native implementation does not make those original
assertions pass. No reclassification or extra source-method credit in this batch.

Known differential limits remain in connected-range duplicate/empty-interval
semantics, sequence-distance arithmetic, large-number load balancing and other
recorded identity/ranking policies. Shared cyclic-supplier comparisons retain
21 actual Java runtime errors separately from successful-result agreement.
See the [stream contracts](constraint-stream-contract.md) and
[compatibility evidence](compatibility.md). Native-reference passes do not prove
Java agreement. Completion still needs evidence for every original-scope
requirement and resolution of the remaining relevant behavior differences.

Five of the 18 comparison jobs have current input hashes: solver/list/stream
correctness and mixed/stream performance. Thirteen older jobs keep their actual
timestamps and input keys and remain visibly stale. Three affected correctness
comparisons were freshly rerun; known discrepancies/errors are unchanged. No old
measurement was rekeyed and no unrelated benchmark was refreshed.

Fresh larger mixed measurements: warm stream **119.380 ms vs Java
15.749 ms**, dense 20.480 ms; all score [0,-338].
Cold stream 37.317 ms vs Java 18.677 ms,
dense 9.925 ms; all score [0,-60]. The larger reference [0,-72]
is a feasible certificate, not an optimum. Work counts and policies differ; these
are bounded shared-host measurements, not equal-work comparisons or universal speed
claims. The stream gap remains substantial. Large basic hill/tabu swaps measure
570.692/568.845 ms vs Java
323.507/329.332 ms. All 128 stream validation
cases have valid, equal-score outcomes and unchanged non-timing signatures.

The latest change stores the first four borrowed map-traversal nodes inline,
with Vec overflow for deeper paths. Range queries and map updates are unchanged.
Two captures of each actual artifact on six workloads show whole-process allocation
requests falling **3.97% for warm mixed streams and 3.60% for cold mixed streams**;
basic stream and dense counts are unchanged. These count malloc/calloc/realloc
requests, not bytes or live/peak memory. A noisy local timing comparison was
checked once on runner-a with the same actual binaries: warm stream is flat, cold
stream 0.62% lower, basic change 1.07% lower, and basic swap inconclusive. No general
throughput speedup is claimed. Three original traversal/ownership controls earn no
upstream credit.

Evidence: work/round253-map-iteration/verdict.json and source-proof.json,
work/round253-qualification/collection.json, work/round253-performance/receipt.json.
Historical optimization results and retained tradeoffs remain in
[next-session.md](next-session.md). Goal remains incomplete.
