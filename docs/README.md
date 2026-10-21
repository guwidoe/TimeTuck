# TimeTuck documentation

TimeTuck is an experimental native Rust constraint optimization library. Begin
with a runnable problem, then choose the contracts relevant to your model.

## Start here

| I want to… | Read |
| --- | --- |
| Run a solver and understand its output | [Getting started](getting-started.md) |
| Understand the main components | [Architecture](architecture.md) |
| Evaluate a migration from Timefold | [Timefold compatibility](timefold-compatibility.md) |
| Evaluate speed claims or reproduce a comparison | [Performance](performance.md) |
| Find priorities or help with development | [Roadmap](roadmap.md), [contributing](../CONTRIBUTING.md) |

## Modeling and solving contracts

These references specify behavior, error handling, limits, and executable evidence.

| Topic | Contract |
| --- | --- |
| Score levels, arithmetic, decimals, and overflow | [Scores](score-contract.md) |
| Finite variables, ranges, pins, moves, exact search | [Basic planning](planning-contract.md) |
| Ordered ownership, list moves, and list shadows | [List planning](list-planning-contract.md) |
| Coupled basic/list decisions | [Mixed planning](mixed-planning-contract.md) |
| Acceptance, neighborhoods, budgets, and repair | [Incremental search](incremental-search-contract.md) |
| Typed streams, collectors, dependencies, and rollback | [Constraint streams](constraint-stream-contract.md) |
| Multi-phase solves, management, cancellation, and live changes | [Solver](solver-contract.md) |

## Applications and configuration

- [Owned roster workflow](owned-roster-workflow.md): files, configuration, and a
  complete managed scheduling application.
- [Configured factories](configured-factories.md), [model configuration](model-config-binding.md),
  and [XML solver factories](xml-solver-factory.md): explicit native bindings.
- [Solution persistence](solution-persistence.md): owned model snapshots and validation.
- [RealWorld model](real_world-model.md) and [application benchmarks](../benchmarks/real_world/README.md):
  the additional approved public workloads and their narrower model contract.

## Evidence and maintenance

- [Compatibility records](compatibility.md): detailed adapted-test and differential
  evidence accumulated during implementation; historical entries retain their limits.
- [Completion evidence](completion.md): original native acceptance requirements and
  subsequent source-specific qualification records.
- [Dashboard](dashboard.md): generating reports and interpreting their freshness.
- [Compatibility assets](../compat/README.md), [legal and provenance](legal-and-provenance.md),
  and [third-party notices](../THIRD_PARTY_NOTICES.md): sources and attribution.
- [Test layout](test-layout.md): focused targets and generated integration shards.
- [Public-release preparation](public-release-preparation.md): the private preparation review.
- [Implementation handoff](next-session.md): detailed development state and retained
  experiments for maintainers; this is not the user guide or a release guarantee.

Recorded counts, timings, and source hashes describe their captured state. Consult
the generated report's freshness checks before treating historical evidence as
verification of the current solver.
