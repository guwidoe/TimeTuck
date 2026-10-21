# Getting started

## Run the scheduling example

Install Git and the current stable Rust toolchain using
[rustup](https://rustup.rs/). Rustup supplies Cargo; the repository requests
Rustfmt and Clippy through `rust-toolchain.toml`. The Rust library uses edition
2024. A minimum supported Rust version has not been established.

```sh
git clone https://github.com/guwidoe/TimeTuck.git
cd TimeTuck
cargo run --locked --example tiny_schedule
```

While the repository is private, cloning requires access and GitHub authentication.
The first build downloads the dependencies locked in `Cargo.lock`; later runs can
reuse Cargo's cache. Java, Python, and the Timefold submodule are unnecessary for
this example.

The [complete program](../crates/solver/examples/tiny_schedule.rs) models six jobs:
two simultaneous jobs in each of three time slots. Each job has three possible
workers. The first job is pinned to worker 1; the other five decisions produce
`3^5 = 243` complete assignments.

Its score has two levels:

- Hard: penalize assigning both simultaneous jobs to the same worker.
- Soft: penalize departing from each job's preferred worker and uneven total loads.

Scores are maximized lexicographically: any hard-score improvement outweighs every
soft-score improvement. A hard score of zero is feasible for this model.

The program constructs `Variable` values and a `FiniteProblem`, then passes the
scoring function and a 243-evaluation budget to `exhaustive_search`. It prints:

```text
243 evaluations; Exhausted; optimal: true
Score: 0hard/-3soft (feasible: true)
```

It also prints the worker assigned to every job. Exhaustion proves optimality for
this small finite problem. If you lower the budget, inspect the termination reason
and `is_optimal()`; the retained best result alone is not proof of an optimum.

## Choose the next example

Run these commands from the repository root:

| Problem | Command | What it adds |
| --- | --- | --- |
| Small route | `cargo run --locked --example tiny_routes` | Ordered-list exact search |
| Depots and routes | `cargo run --locked --example mixed_routes` | Coupled decisions and lists; both scoring backends |
| Graph coloring | `cargo run --locked --release --example graph_coloring` | Construction and incremental local search |
| Graph coloring with streams | `cargo run --locked --release --example stream_coloring` | Typed graph scoring |
| List routing with streams | `cargo run --locked --release --example stream_list_routes` | Stream-scored list construction and improvement |
| Configured scheduling | `cargo run --locked --example owned_roster_workflow` | Files, configuration, and managed solving |

The roster workflow writes native input/output JSON and a schema; its
[guide](owned-roster-workflow.md) explains those files. Use release builds for
timing larger examples; a debug run is a correctness demonstration.

## Use the library in another Rust project

The crate is unpublished and its API may change. With this checkout next to your
application, use a local dependency:

```toml
[dependencies]
timetuck = { path = "../TimeTuck/crates/solver" }
```

Adapt the path to your layout and begin with the scheduling example's imports,
model, and score callback. Consult the [architecture](architecture.md) and the
[modeling contracts](README.md#modeling-and-solving-contracts) as the model grows.
An application migrated from Java must explicitly translate its bindings and
constraints; see [Timefold compatibility](timefold-compatibility.md).

## Development tools and reference checks

Python 3 and Bash are needed for the repository's verification/reporting scripts.
Use [the contribution guide](../CONTRIBUTING.md) for focused development checks.

Only the optional Timefold reference comparisons need Java and downloaded Maven
artifacts. Initialize the pinned test reference when working on conformance:

```sh
git submodule update --init upstream/timefold-solver
python3 scripts/upstream_inventory.py
```

This inventories test sources and verifies the pin; it does not execute JUnit or
establish a compatibility pass. See [performance](performance.md) for the additional
requirements of the Java/Rust comparison tools.
