# TimeTuck

An experimental constraint optimization solver written in Rust. Model decisions,
describe hard constraints and preferences, and search for a good assignment or
ordered plan. Example problems include staff scheduling, grouping, graph coloring,
and routing.

TimeTuck began with the idea of **rewriting Timefold Solver Community in Rust**:
bring its planning capabilities to a native library and investigate how fast they
can run. The implementation is independently written, using pinned Community tests
and public APIs as behavioral specifications. Adapted tests and fixtures retain
their attribution.

**Status:** under active development; Rust APIs may change. The crate is not yet
published to crates.io.

## Try a complete problem

Install a current stable Rust toolchain through [rustup](https://rustup.rs/) and
Git.

```sh
git clone https://github.com/guwidoe/TimeTuck.git
cd TimeTuck
cargo run --locked --example tiny_schedule
```

This assigns six jobs across three time slots to three workers. Two simultaneous
jobs cannot share a worker; preferences and balanced workloads improve the soft
score. One job is pinned to its initial worker. The solver enumerates the 243
remaining assignments and reports a feasible optimum:

```text
243 evaluations; Exhausted; optimal: true
Score: 0hard/-3soft (feasible: true)
```

See the [complete example](crates/solver/examples/tiny_schedule.rs) and
[getting-started guide](docs/getting-started.md). Building and running the Rust
library requires neither Java nor the upstream submodule.

## What you can model

| Capability | Examples and details |
| --- | --- |
| Decisions drawn from finite ranges, with optional values and pins | [Scheduling](crates/solver/examples/tiny_schedule.rs), [planning contract](docs/planning-contract.md) |
| Ordered lists with unique ownership and pinned prefixes | [Routes](crates/solver/examples/list_routes.rs), [list planning](docs/list-planning-contract.md) |
| Coupled basic decisions and ordered lists | [Depot and route planning](crates/solver/examples/mixed_routes.rs), [mixed planning](docs/mixed-planning-contract.md) |
| Hard/soft scores, multiple priority levels, and exact decimal arithmetic | [Score contract](docs/score-contract.md) |
| Construction, local search, bounded repair, and exact search | [Search](docs/incremental-search-contract.md), [multi-phase solving](docs/solver-contract.md) |
| Incremental constraint streams, joins, collectors, and derived values | [Stream coloring](crates/solver/examples/stream_coloring.rs), [stream contract](docs/constraint-stream-contract.md) |
| Configured jobs, managed solving, and solution persistence | [Roster workflow](docs/owned-roster-workflow.md), [persistence](docs/solution-persistence.md) |

Heuristic search retains the best solution found within its limits. Optimality is
proved only by exhausted exact search, with admissible bounds where applicable.

## Relationship to Timefold

The compatibility target is **Timefold Solver Community 2.7.0**, pinned at
`8b815bc6b5f3eb81feedc94dea0885a93b267762`. Compatibility is established for
specific adapted behaviors and comparison cases; there is no whole-Timefold
equivalence guarantee.

TimeTuck provides native Rust APIs. It is **not a drop-in replacement for a Java
Timefold dependency**: existing applications need their domain bindings,
constraints, and integration code migrated. Java API/ABI compatibility, Java
reflection and framework integrations, and Enterprise functionality are outside
the project's scope.

TimeTuck is independent of, and is not affiliated with or endorsed by, Timefold.
Read the [compatibility overview](docs/timefold-compatibility.md) for migration
expectations, evidence, and known differences.

## Performance

Performance is a core goal, and the repository includes reproducible Java/Rust
comparisons with score and validity checks. Recorded results include native wins
on particular workloads. They do not establish that TimeTuck is faster for every
application, or that more score evaluations produce better solutions.

Measurements record their source, inputs, machine, configuration, and date.
Historical results remain historical when the solver changes. Start with
[performance and reproducibility](docs/performance.md) before quoting a number;
the [performance dashboard on GitHub Pages](https://guwidoe.github.io/TimeTuck/)
is the planned public snapshot location (prepared; hosting not yet enabled).
Its application section uses anonymized “Real-world use cases” labels. The
[local dashboard](docs/dashboard.md) retains detailed evidence and live freshness
checks; see [hosting and snapshot updates](docs/github-pages.md).

## Documentation and contributions

- [Documentation index](docs/README.md): guides, architecture, and API contracts.
- [Roadmap](docs/roadmap.md): current priorities and release boundaries.
- [Contributing](CONTRIBUTING.md): focused checks, test provenance, and change review.
- [Security](SECURITY.md): reporting vulnerabilities privately.
- [Public-release preparation](docs/public-release-preparation.md): review results
  and remaining publication decisions.

## License

Original project files are licensed under [Apache-2.0](LICENSE). Adapted upstream
tests and other imported material retain their applicable licenses and notices;
see [third-party notices](THIRD_PARTY_NOTICES.md) and
[source provenance](docs/legal-and-provenance.md).
