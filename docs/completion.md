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

The retained Rust checkpoint has **8,409 passing tests and 290 doctests**, source
digest `8eedef86e478e230a5a0afe13c9fd82f4a14facbd286866cb8ac47b521adc962`.
Its final `scripts/check.sh` passed in 386.73 seconds. The current layout uses
22 integration executables and 95 verification jobs, preserving original test
files and source attribution. Successful checks are reused when their relevant
inputs are unchanged; documentation-only updates do not rerun the solver suite.
The generated report remains the authority for later counts and freshness.

The applications establish concrete quality criteria: both roster backends solve
128 initially unassigned shifts to the planted zero-penalty certificate; mixed
cold construction reaches the independently enumerated small optimum and beats
the larger feasible certificate while preserving pins. These results establish
bounded application behavior, rather than universal optimality or performance.

Completion is still unproven. The inventory retains **11 pending methods and
435 exclusions**, with 4,409/4,855 methods passing (90.814%). Pending identities
include Java loading/edition seams and exact private k-opt buffer assertions.
Implemented native algorithms do not turn those source assertions into passes.
Excluded frameworks remain visible in the denominator.

Known differential limits also remain: connected-range duplicate/empty-interval
semantics, widened sequence-distance arithmetic and large-number load-balancing
arithmetic differ from Java in recorded cases. Shared cyclic-supplier comparisons
retain actual Java runtime errors separately from successful-result agreement.
See the [collector and dependency contracts](constraint-stream-contract.md).
Passing independent Python references does not establish Java agreement.

Four of the **18 comparison jobs** have current input hashes: solver, list and
stream correctness comparisons, plus stream performance. Fourteen older jobs,
including mixed performance, retain their actual timestamps and keys and are
visibly stale. They have not been relabeled as current.

The current large stream workload measures hill-climbing swaps at 475.434 ms
against Java's 285.759 ms, and tabu swaps at 498.166 against 304.284 ms. All 128
validation cases have valid, equal-score outcomes. These bounded shared-host
measurements include setup and result validation; they do not prove general Java
parity, identical execution traces or universal speed requirements.

The latest retained change caches primitive endpoint IDs during fixed-link batch
coalescing and directly indexes validated contiguous endpoint slots. Sparse and
custom IDs retain their fallbacks. Four balanced paired runs against the preceding
actual binary show large swap time 3.99% lower overall, with all four forks improving.
Change time is 2.31% lower and mixed stream 3.24% lower overall, but slower outliers
leave those gains inconclusive. The dense control also varies. This does not prove
a universal speedup. Exact artifacts, original tests and observations are in
`work/round245-link-index/`.

The preceding terminal-scoring change replaces per-update tree searches with
validated direct indexing for contiguous primitive planning and relationship IDs. It still checks
the complete relationship ID, including endpoints; sparse and custom IDs preserve
the original tree behavior. Four balanced paired runs against the preceding actual
binary show large change time 3.88% lower and swap time 6.05% lower overall, with
every fork improving for both. Mixed stream is 3.21% slower initially and 3.0% slower
in one longer confirmation; the unchanged dense control also drifts slower.
These targeted gains and mixed tradeoff do not establish a universal speedup.
Exact artifacts, tests and observations are in `work/round244-fixed-index/`.

The preceding source-batch change coalesces bounded planning-source batches using
cached integer IDs and input positions, and publishes source outputs directly.
Generic source key ownership and Clone/Drop behavior remain unchanged. Two
repeatable whole-process captures show 3.17% fewer allocation requests for changes
and 5.06% fewer for swaps (518,500 fewer requests per swap benchmark process).
Across this and the preceding fixed-link change, swap requests fall from 11,458,845
to 9,730,733, or 15.08%. Mixed allocation counts are unchanged. These are intercepted
malloc/calloc/realloc counts including setup and validation, not allocated bytes
or live/peak memory. Scratch buffers retain their largest batch capacity without
retaining payloads.

For that source-batch change, throughput improvement is unproven. Its four balanced
runs pinned to CPU 2 show change time 0.34% lower and swap time 1.33% lower overall, with slower outliers in
both cases. Mixed stream time is 1.35% slower overall and dense control 1.55%
slower. These observations do not establish a general speedup. The previous241
fixed-link timing gains and mixed tradeoff remain recorded in the handoff.
Evidence is preserved under `work/round242-source-batch/`.

A subsequent erased-event cache experiment was rejected without changing the
retained solver. Its final version removed 25.66% of whole-process swap allocation
requests, but slowed large swaps 15.45% and changes 8.86%, with all four paired
runs slower for both. Candidate-only passing tests are excluded from the counts
above. Frozen sources, actual artifacts and observations remain under
`work/round243-event-pool/`. A fresh CPU attribution capture of the retained242
binary is in `work/round243-retained-profile/`; this is diagnostic evidence and
has not refreshed or relabeled any canonical benchmark.

The preceding bounded-terminal experiment remains unintegrated. Its final variant
reduced mixed allocation requests by only 22 out of 2,515,687 (0.000875%) and
slowed large-change searches by 8.24% in all four paired runs. Its modest mixed
improvement did not justify that regression and the added implementation
complexity. All candidate variants and focused checks remain under
`work/round240-bounded-terminals/`. It was rejected without changing native
sources; the subsequent scratch-buffer change and qualification above form the
current retained checkpoint.

The pending source assertions and explicit Java behavior differences above remain
open compatibility limits. Broad equivalence cannot be inferred from current
hashes, the percentage, green representative cases or the absence of a newly
discovered algorithm gap. A final completion claim still needs direct evidence
for every remaining original-scope requirement.
