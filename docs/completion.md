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

The latest integrated checkpoint has **8,425 passing Rust tests and 290 doctests**,
source digest `8cd2a947af64e9e75e75b8ef13f473239571c6a012614521f2db7488ff840b82`. The required final `bash scripts/check.sh` passed
on runner-a in 91.71 seconds, running the affected checks and genuinely reusing
142 unchanged jobs. Source-bound output and checksum metadata were collected
unchanged; no duplicate local full check or rewritten evidence keys. The live
report remains the authority for later counts and freshness.

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
timestamps and input keys and remain visibly stale. Correctness comparisons
reuse matching earlier evidence; only the affected mixed benchmark was refreshed
in this batch. Known discrepancies/errors remain unchanged. No old measurement
was rekeyed.

Fresh larger mixed measurements: warm Stream **23.481 ms vs Java
17.032 ms**, Dense 5.650 ms; all score [0,-338].
Cold Stream 33.118 ms vs Java 20.566 ms,
all score [0,-60]. The larger reference [0,-72] is a feasible certificate, not
an optimum. The user-confirmed target is Java median-time parity or better at
equal-or-better solution quality. These bounded shared-host measurements use
different work/policies; the Stream gap remains visible.

The dashboard's configurable progress index is **84.6%**: feature families 40%,
behavior 20%, performance 30%, current evidence 10%. It is an explicit index of
tracked targets, not estimated engineering effort or remaining time. Family
balancing and weakest-workload scoring prevent numerous easy/fast cases from
hiding gaps. Stale measurements retain their last-demonstrated contribution with
visible freshness debt; 100% requires every target and current evidence. See
[the calculation](dashboard.md#project-completion-index).

The latest retained change filters unassigned customer rows before their constant
cold Stream penalty using an existing operator. It preserves score semantics,
assignments, logical work and budgets; graph shape and physical callbacks differ.
Balanced native comparisons show the larger cold Stream 22.45% faster locally and
21.55% on runner-a in all four forks each; other workload controls and variation
remain in the evidence. Fifty-six partial/final native controls and one original
Rust regression independently check arithmetic, pins, completeness and phase
budgets. This is a constraint declaration improvement, not general Java parity.

Evidence: work/round259-cold-filter/verdict.json, partial-proof.json,
collection.json and performance-receipt.json. Historical optimization results and
retained tradeoffs remain in [next-session.md](next-session.md). Goal incomplete.
