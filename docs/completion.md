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

The latest integrated checkpoint has **8,446 passing Rust tests and 290 doctests**,
source digest `e3ec934a911c97717b51e6568b7a82d1b0963080a93f6080dd5bf51bb4eff9b8`. The required final `bash scripts/check.sh` passed
on runner-a in 290.08 seconds, running the affected checks and genuinely reusing
63 unchanged jobs. Source-bound output and checksum metadata were collected
unchanged; no duplicate local full check or rewritten evidence keys. The live
report remains the authority for later counts and freshness.

The applications establish concrete quality criteria: both roster backends solve
128 initially unassigned shifts to the planted zero-penalty certificate; mixed
cold construction reaches the independently enumerated small optimum and beats
the larger feasible certificate while preserving pins. These results establish
bounded application behavior, rather than universal optimality or performance.

The inventory has **4,410/4,855 passing adapted methods (90.834%), 10 pending,
435 excluded and zero failed/unverified**. The percentage counts statically
discovered annotated source methods, not all JUnit runtime invocations or overall
project completion. Even clearing all 10 pending methods would give only 91.04%
with the existing exclusions; exclusions remain visible in the denominator.
The 10 pending methods include eight JVM/loading/framework seams and two exact
private k-opt endpoint/partner-array assertions. They are not evidence of ten
missing native algorithms, and native implementation does not make those original
assertions pass. One complete Community version-identity method now preserves
its literal edition assertion using existing native Cargo package diagnostics.
This adds no runtime feature, JVM loading or Enterprise functionality.

Known differential limits remain in connected-range duplicate/empty-interval
semantics, sequence-distance arithmetic, large-number load balancing and other
recorded identity/ranking policies. Shared cyclic-supplier comparisons retain
21 actual Java runtime errors separately from successful-result agreement.
See the [stream contracts](constraint-stream-contract.md) and
[compatibility evidence](compatibility.md). Native-reference passes do not prove
Java agreement. Completion still needs evidence for every original-scope
requirement and resolution of the remaining relevant behavior differences.

Stream performance was deliberately refreshed on the final source using original
fixtures, validation, budgets and settings. All 128 controls and eight timing
workloads retain equal scores. Aggregate remains current from round265. The other
16 measurements retain their input keys/dates and remain visibly stale. No old
result was rekeyed. Last measured progress stays visible, while currentness
separately records remaining evidence debt.

Last measured larger mixed workloads (261; now stale): warm Stream **28.520 ms vs Java
22.606 ms**, all score [0,-338]. Cold Stream 27.043 ms vs Java
21.812 ms, all score [0,-60]. The larger reference [0,-72] is a
feasible certificate, not an optimum. The target remains Java median-time parity
or better at equal-or-better quality. These shared-host times vary with load;
controlled mixed-workload comparisons show no meaningful warm Stream gain.

The dashboard's configurable progress index is **84.1%**: feature families 40%,
behavior 20%, performance 30%, current evidence 10%. It is an explicit index of
tracked targets, not estimated engineering effort or remaining time. Family
balancing and weakest-workload scoring prevent numerous easy/fast cases from
hiding gaps. Stale measurements retain their last-demonstrated contribution with
visible freshness debt; 100% requires every target and current evidence. See
[the calculation](dashboard.md#project-completion-index).

The retained 261 optimization scans touched rows to discover sparse Eq-only
value updates, preserving full metadata and registry-ordered source callbacks.
It adds no Hash/Ord bounds or benchmark-profile changes. Controlled 512-value
temporary updates improve 35–45% locally and 35–48% on runner-a, depending on whether
value rows are observed, in all four forks. These timings include hot updates and
rollback, exclude setup/full diagnostics, and are not Java or whole-solve speed
claims. Smaller mixed workloads remain roughly flat with some slower controls.
Sparse, nested and callback-panic regressions include 32 registered Eq-only values.

The explicit contribution-only scoring APIs retain IDs and cached contributions
without terminal tuple payloads. Match inspection/explanations return a named
unavailable error; tracked defaults and full upstream source rows remain intact.
Twelve original tests cover replacement/retraction, transactional failure and
rollback, saved candidates, exact weights/replay, occurrence metrics and per-ID
consistency. Whole-solve private controls preserve assignments, quality and actual
callbacks. Larger mixed timings show no useful gain; canonical profiles remain
unchanged. No general speed or whole-graph memory claim is established.

The latest checked-sum implementation avoids internal snapshot copies and extrema
summaries while retaining capture frequencies and external persistent snapshots.
Controlled native sum timings fall 73.6% locally and 71.1–73.3% on runner-a, across
four balanced forks. A four-rank inline queue for single-source batches reduces
large change-solve timings by 5.57–7.68% locally; multi-change routing is unchanged,
and swap controls include slower observations. Original inputs, quality and physical
callbacks remain intact. These native comparisons establish no general speedup.
The current round266 stream comparison measures large hill/swap at 802.513ms
versus Java 475.796ms, and large tabu/swap at 1706.744ms versus 892.788ms,
with equal scores. Java parity remains the target, multiplier 1.

Sorted-set replacement now uses one staged snapshot, preserving multiplicities,
representatives, exact comparison/clone sequence and failure rollback. All 96
original aggregate controls and complete timed observations pass. On runner-a,
64/512-capture updates improve 11.01%/5.58%, all four forks; unchanged map controls
are flat. Local and sum controls vary; there is no general or whole-solve claim.
Fresh current Java comparisons still show gaps: set64 Rust 7.671µs/Java 4.752µs,
set512 25.733µs/15.925µs, sum512 1.976µs/1.520µs. Shared-host measurements vary;
do not attribute cross-run Java ratio changes to this optimization.

Fixed-link batches now resolve replacement inputs directly by endpoint slot,
retaining prior callback/ownership traces and panic recovery. One usize per
endpoint and small private vectors trade space for repeated lookup work. The
controlled large hill/swap confirmation improves 1.44% across all four forks;
other timings are variable. This is a modest native gain, not general Java parity.
No benchmark fixture, work, budget or validation is reduced.

Evidence: work/round266-qualification/collection.json, gate-receipt.json,
verdict.json, stream-remote-results.json,
stream-confirmation/stream-remote-results.json and stream-performance-receipt.json;
work/round265-set-qualification/collection.json, gate-receipt.json,
verdict.json, aggregate-paired/results.json, aggregate-remote-results.json and
aggregate-performance-receipt.json;
work/round264-parity/collection.json, gate-receipt.json,
aggregate-paired/results.json, aggregate-remote-results.json,
final-stream-paired/results.json and both performance receipts;
work/round263-score-only/collection.json, verdict.json and paired/results.json;
work/round261-touched-scan/verdict.json, collection.json,
performance-receipt.json and java-diagnostic-difference.json;
work/round262-community-version/collection.json and gate-receipt.json. Historical results
and retained tradeoffs remain in [next-session.md](next-session.md). Goal incomplete.
