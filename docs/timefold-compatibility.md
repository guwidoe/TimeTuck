# Timefold compatibility and project origin

## Why TimeTuck exists

The original idea was a Rust rewrite of Timefold Solver Community: preserve useful
planning behavior in a native library and pursue strong runtime performance.
TimeTuck implements that idea through independently written Rust production code.
Pinned Community tests and public APIs provide behavioral specifications; adapted
tests and fixtures are attributed in the provenance ledger. This development
arrangement does not establish a formal clean-room process.

The reference is Community **2.7.0**, commit
`8b815bc6b5f3eb81feedc94dea0885a93b267762`, recorded in
[`compat/upstream.json`](../compat/upstream.json). The submodule remains unmodified.
Enterprise implementation, artifacts, and functionality are outside the authorized
reference and implementation scope. TimeTuck has no Timefold affiliation or endorsement.

## What compatibility means here

| Dimension | Commitment and limits |
| --- | --- |
| Planning behavior | Native implementations are checked against specific adapted Community assertions and public-API comparisons. |
| Rust API | Native Rust types, ownership, explicit bindings, and typed errors; the API may change. |
| Java API and ABI | No interchangeable Java dependency, JVM class loading, annotation processing, or reflection compatibility. |
| Frameworks and configuration | Explicit native profiles and bindings cover documented cases; arbitrary Java configuration and framework integration need migration. |
| Search traces | Equal seeds do not imply identical random streams, candidate order, stop times, or solutions. |
| Solution quality | Recorded feasibility and quality checks apply to their inputs and limits; heuristics do not guarantee an optimum. |
| Performance | Measurements support claims for named workloads and captured sources, not general superiority. |

“Drop-in compatible” would imply that an existing integration can switch
dependencies without migration. That is not the current contract. The useful
claim is behavioral compatibility for documented native cases, with a migration
path based on familiar modeling concepts.

## Migrating a model

1. Identify decisions, legal ranges, pins, and immutable facts. Choose a basic,
   ordered-list, or mixed native model.
2. Express the objective in native score types and constraints. Keep an independent
   full-score implementation while introducing incremental streams.
3. Replace Java annotations, reflection, and service wiring with explicit native
   bindings and configuration.
4. Compare representative initial, candidate, committed, and rolled-back states.
   Validate final schedules independently before comparing speed or quality.
5. Record seeds, budgets, termination, input data, and expected quality. Reassess
   search settings for the application instead of expecting identical traces.

The [getting-started examples](getting-started.md) demonstrate native usage;
the [contracts](README.md#modeling-and-solving-contracts) specify individual behaviors.

## Reading the evidence

- [`compat/provenance.json`](../compat/provenance.json) records adapted methods,
  fixture sources, modifications, the pinned commit, and notices.
- [`compat/exclusions.json`](../compat/exclusions.json) records explicit scope
  exclusions. Excluded methods earn no pass credit.
- [Detailed compatibility records](compatibility.md) retain individual behavior
  comparisons, native substitutions, and observed reference discrepancies.
- [Completion evidence](completion.md) records the original native acceptance
  cohort and later source-specific checks.
- [Dashboard methodology](dashboard.md#reading-the-numbers) distinguishes static
  upstream method counts, executed adaptations, and broader comparison results.

A mapping is not an execution result. Pending, skipped, missing, or empty cases
never establish compatibility. Statically inventoried annotated Java methods are
not a count of every parameterized or inherited JUnit invocation; the original
complete JUnit suite has not been run by this pipeline.

Historical acceptance results do not automatically verify a newer solver.
Generated reports retain source hashes and freshness checks. Likewise, additional
application tests are tracked separately and earn no upstream-method credit.
Connected-range and concurrent-scheduling records include explicit disagreements
or reference errors; consult their individual contracts before relying on those
edge cases.
