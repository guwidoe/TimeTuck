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

The retained Rust checkpoint has **8,376 passing tests and 290 doctests**, source
digest `0faa1cb000f2aca1ab8a463b8f30e163d79124e9d5c3ce11e95c67c3a591c807`.
The most recent Rust change ran two affected targets and reused 1,345 matching
targets. Documentation/report refreshes do not require rerunning unchanged tests.
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

All **18 comparison jobs** have current input hashes at this checkpoint: four
differential jobs and fourteen performance jobs. This was a deliberate refresh
of changed-input evidence; successful unchanged Rust checks were retained. Old
raw reports keep their original timestamps and source keys.

The measured larger mixed Stream cases remain performance priorities: roughly
9.1 times Java's observed warm-start time and 2.3 times its cold-start time.
Move policies, setup and work differ, so these are bounded runtime observations,
not equal-work comparisons or universal speed requirements.

The pending source assertions and explicit Java behavior differences above remain
open compatibility limits. Broad equivalence cannot be inferred from current
hashes, the percentage, green representative cases or the absence of a newly
discovered algorithm gap. A final completion claim still needs direct evidence
for every remaining original-scope requirement.
