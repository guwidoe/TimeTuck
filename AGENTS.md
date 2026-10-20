# Timetuck project instructions

## Goal and boundaries

Build an independent Rust constraint optimization solver from scratch. Use the
pinned Timefold Community tests to establish behavior. Do not copy, translate,
or mechanically port Timefold production implementation into `crates/`.
Use tests and public API documentation as specifications. Do not claim a formal
clean-room process: no organizational separation has been established.

The repository contains the independent native solver. Read `docs/completion.md`,
`docs/next-session.md` and `docs/compatibility.md` before implementation. Keep the scope to Community;
Enterprise code, artifacts, and features are not an authorized source.

## Work conventions

- Keep Rust implementations idiomatic; no commitment to Java API or ABI parity.
- Keep the upstream submodule unmodified. Change its pinned revision only as an
  explicit compatibility update, together with `compat/upstream.json`.
- Record each copied/adapted test or fixture in `compat/provenance.json` with its
  upstream commit, path, test identity, local path, license, and modifications.
  Preserve applicable notices; a translation is not automatically original work.
- Never claim compatibility from a passing scaffold, skipped tests, or an empty
  suite. Track implemented, pending, and intentionally excluded cases separately.
- Prefer deterministic correctness cases. Stochastic solver tests need explicit
  seeds, termination limits, and quality criteria; do not assume identical traces.
- Run `bash scripts/check.sh` after Rust changes. Run the inventory after pin changes.
- All crates remain `publish = false` until release preparation.

## Verification workflow

- Batch related behavior changes. Use focused tests while developing, then run
  `bash scripts/check.sh` once on the final Rust state of each coherent batch.
- Reuse successful verification when its relevant inputs are unchanged. Repeat
  checks for changed inputs, failures, unresolved concerns, or an explicit user
  request; a new commit, turn, or context reset alone is not a reason to repeat.
  Shared solver changes require broad regression coverage; file paths alone do
  not establish that behavior is independent.
- Run benchmarks when needed to assess a performance change or support a claim.
  Do not refresh every benchmark after every increment. Keep older measurements
  visibly stale until a deliberate refresh; never relabel old evidence as current.
- Verify the browser after UI or rendering/data-contract changes. For backend-only
  report refreshes, inspect report data and freshness without repeating UI checks.
- Repeat correctness tests in release mode only for a release-specific concern.
  Documentation-only edits need diff review, not a Rust suite or benchmark rerun.
- Record completed checks, their source state, and remaining gaps concisely in the
  handoff. Evidence reuse must respect the reporting tools' freshness checks;
  changing hashes on old results is not verification.

## Commits

Inspect staged changes and use Conventional Commits: `<type>(<scope>): <summary>`.
Use imperative subjects under 72 characters. Never amend or rewrite commits.
