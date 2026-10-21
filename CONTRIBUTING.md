# Contributing to TimeTuck

TimeTuck is experimental and currently private during public-release preparation.
Access and collaboration are by invitation until publication. The Rust API may
change; there is no promised release cadence or support response time.

Start with the [README](README.md), [architecture](docs/architecture.md), and the
[contract](docs/README.md#modeling-and-solving-contracts) relevant to your change.
For ongoing implementation, read `AGENTS.md`, `docs/completion.md`,
`docs/next-session.md`, and `docs/compatibility.md`.

## Propose a focused change

Look for an existing issue before opening another. Describe the concrete problem,
a small reproducible model, expected and actual behavior, and the relevant source
revision. For search problems, include the seed, termination limits, and the score
or quality criterion. Use synthetic or approved public data. Keep credentials and
private application data out of issues and examples; see [security reporting](SECURITY.md).

Small fixes can include their explanation with the change. Discuss substantial
API or scope changes before making a large implementation commitment. Contributions
are reviewed against the native contracts and source boundaries below.

## Verify the behavior you changed

Use focused tests and compile checks during development. For example, for the
score parsing behavior:

```sh
cargo test --locked -p timetuck --test simple_score parse_score
```

Choose the target and test filter that exercise your change; shared solver changes
need coverage of the directly affected model and scoring paths. The generated
integration layout preserves individual target names; see [test layout](docs/test-layout.md).
Use `cargo fmt --all -- --check` for formatting and an appropriately scoped Cargo
check or Clippy command for changed code.

The existing CI gate runs `bash scripts/check.sh`, which orchestrates formatting,
Clippy, captured Rust tests, documentation tests, and optional report regeneration.
Full suites are reserved for CI, nightly runs, or an explicit request. Documentation
changes need link/example and diff review, without repeating the Rust suite or
benchmarks. Never relabel old execution evidence after changing source hashes.

When adding a top-level integration test, regenerate and commit its registration:

```sh
python3 scripts/rust_test_layout.py --write
```

Run benchmarks only when needed to assess a performance change or substantiate a
claim. Report quality and validity alongside time, and retain the original source,
inputs, samples, and freshness. See [performance](docs/performance.md).

## Preserve implementation and source boundaries

Production Rust must be independently implemented. Community tests and public API
documentation are specifications; do not copy, translate, or mechanically port
Timefold production implementation into `crates/`. Enterprise source, artifacts,
and functionality are outside scope. Do not claim a formal clean-room process or
Java API/ABI parity.

Keep the upstream submodule unmodified. A pin update must be an explicit
compatibility update accompanied by `compat/upstream.json` and a new inventory.
Every copied/adapted test or fixture needs its exact source identity, commit,
license, modifications, and notices in `compat/provenance.json`. Track additional
application material separately in `compat/additional-tests.json`; it earns no
upstream-method credit. Preserve applicable file notices and update
`THIRD_PARTY_NOTICES.md`. See [source provenance](docs/legal-and-provenance.md).

Generated scaffolds, skipped or empty tests, and partial assertion translations do
not establish compatibility. Seeds and budgets must be explicit for stochastic
tests. Prefer independent score recomputation and deterministic regressions.

## Submit and license your changes

Use Conventional Commits with an imperative subject under 72 characters:
`<type>(<scope>): <summary>`. Include what changed, why, relevant validation, and
remaining limits in the review description. Preserve other contributors' work;
do not amend, rebase, or discard someone else's changes. All crates retain
`publish = false` until a separately authorized crate release.

Submit only material you have the right to contribute. Original contributions
are under the project's Apache-2.0 license unless explicitly agreed otherwise;
imported material retains its applicable terms and attribution.
