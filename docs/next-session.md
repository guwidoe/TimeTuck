# Implementation session handoff

## User intent

Implement Timefold-like constraint optimization from scratch in Rust and validate
behavior using the open-source Timefold Community test suite. This session only
created the scaffold. The user will start a separate session here for execution.

## Prepared

- Rust 2024 workspace, one empty library crate, no third-party Rust dependencies.
- Stable Rust toolchain configuration, formatting/Clippy/test check script and CI.
- Timefold Community 2.7.0 submodule pinned to commit
  `8b815bc6b5f3eb81feedc94dea0885a93b267762`.
- A revision-checked upstream test-source inventory command.
- Compatibility strategy and attribution rules; empty provenance ledger.
- No solver implementation, ported tests, Java adapter, or performance claims.
- Local Git repository only; no remote hosting or package publication configured.

## Suggested first slice

1. Read `AGENTS.md` and the compatibility/provenance documents.
2. Run `bash scripts/check.sh` and `python3 scripts/upstream_inventory.py`.
3. Inspect selected upstream score tests and public score API documentation.
4. Agree score semantics, including initialization, feasibility, ordering,
   arithmetic/overflow, parsing, and multi-level scores; avoid premature API breadth.
5. Record/adapt the chosen cases, preserving notices, then implement the smallest
   useful score abstraction from scratch and verify those cases.
6. Build a Java oracle and neutral fixture schema once this contract is concrete.
7. Grow from scoring to moves/undo, deterministic small problems, then search.

Keep a coverage/status ledger. Passing local tests is not evidence that the full
Timefold suite passes. The unmodified JUnit suite is not wired to Rust.

## Available tools at scaffold time

Rust/Cargo 1.92.0, rustfmt, Clippy, Python 3, Git, and OpenJDK 21 were installed.
The toolchain tracks stable; no minimum supported Rust version has been promised.
Upstream Java build/test setup and its Maven wrapper have not been validated.
