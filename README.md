# Timetuck

An independent constraint optimization solver being written from scratch in Rust.
The goal is behavioral compatibility with selected **Timefold Solver Community**
features, using its open-source tests as a conformance reference.

**Status: scaffold only.** No solver functionality, adapted tests, or compatibility
claims yet. This project is not affiliated with or endorsed by Timefold.

## Development

Install Rust via rustup, Git, and Python 3. Run from the repository root:

```sh
bash scripts/check.sh
```

This checks formatting, Clippy, compilation, and Rust tests. There are currently
zero solver tests; a green check only validates the scaffold.

## Upstream test reference

Timefold Community **2.7.0** is pinned as a separate Git submodule. It is not a
Rust dependency and is not linked into the library. Normal Rust checks need no JVM.

```sh
git submodule update --init upstream/timefold-solver
python3 scripts/upstream_inventory.py
```

The inventory verifies the pinned revision and writes a list of upstream Java
test-source files to `work/upstream-tests.json`. It does not execute tests or
measure coverage. Upstream JUnit tests cannot directly exercise a Rust library:
we need adapted behavioral cases and a Java/Rust comparison harness.

## Layout

- `crates/solver/`: original Rust library; add APIs during implementation.
- `upstream/timefold-solver/`: unmodified, pinned upstream reference.
- `compat/`: provenance ledger and future fixtures/Java oracle adapter.
- `docs/compatibility.md`: testing strategy and compatibility boundaries.
- `docs/legal-and-provenance.md`: naming, licensing, and test attribution.
- `docs/next-session.md`: starting point for the implementation session.

Original project files use Apache-2.0; see `LICENSE`. Upstream material retains
its own notices and license. See `THIRD_PARTY_NOTICES.md` before adapting tests.
