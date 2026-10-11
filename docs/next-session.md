# TimeTuck implementation handoff

## Objective and boundaries

Build an independent Rust constraint optimization solver using pinned Timefold
Community tests and public API documentation as specifications. Maintain the
visual compatibility/performance dashboard. The goal is active and incomplete.
Do not copy or translate upstream production implementation. Community only;
no formal clean-room claim. Keep the upstream submodule unchanged.

Pinned upstream: v2.7.0, `8b815bc6b5f3eb81feedc94dea0885a93b267762`.
Tests/fixtures need complete provenance in `compat/provenance.json`; partial,
original, skipped or empty tests earn no upstream-method credit. All crates
remain unpublished. Use Conventional Commits; never amend/rebase/push.

The user wants three concurrent GPT-6 Astra/high implementation agents,
performance progress, occasional commits and selective verification reuse.
Avoid repeated checks, benchmark sweeps and procedural overhead.

## Latest verified checkpoint

Round 23 gate 8358 and recorder 96507 completed successfully:

- 6,303 Rust tests and 276 doctests.
- 3,363/4,855 upstream methods passed (69.269%); 1,228 pending, 264 excluded,
  zero failed/unverified. These are annotated methods, not expanded invocations.
- 809 Rust verification jobs ran; 99 Rust and 46 Python jobs reused valid evidence.
- Source: `81c7cdad85a04fd8432971879af762413a8cf03d203dc7c15c638aee20b27c23`.
- Verification: `4032f5d57fa3a00fdad95b49aacbf158b67bf71d7111ed45293af82d72ecee60`.
- Evidence: `work/parallel-round23-evidence.json` and its evidence directory.

Integrated round23 commits: a9fab92 streamed child cursors; 68af365 group rebasing;
8a67fd1 owned calculator edit batches; eb3ed76 compact primitive swap undo;
21a9a27 builtin owner/rebase tests; d146073 live restart publication fix;
bd388a4 provenance (+18 complete methods, +22 fixtures). Parent reviews, artifact
hashes and integrated zero-context stable patch IDs are recorded.

The dashboard is served at http://localhost:8765. Reuse the server. Backend-only
refreshes require report/freshness inspection, not a repeated browser check.

## Current integration batch — round24

Four reviewed slices are committed on main after the verified checkpoint:

- 42a56b8 (agent 52dadff): opt-in lazy weighted union. First candidate opens/polls
 one enabled family instead of 64; default prefetched behavior stays unchanged.
 11 focused tests + strict Clippy passed; final docs-only delta separately retained.
- a1096d9 (agent b31a484): CalculatorSession temporary change/score/visit with
 independent snapshot rollback on success/error/panic. 29 focused tests + 1 doctest,
 strict Clippy passed. Independent Clone state required; no source-method credit.
- 261a4ff (agent 71113a3): compact primitive Change undo for large list edits,
 including mixed moves. Removed payload stays armed across record callbacks;
 original rows clone lazily. 19 focused tests + Clippy; final materializer reran its
 unit + 5 integrations. 792 paired samples / 44 workloads preserve results/work/payloads.

- ed7a5a4 (agent 4afa8bf): persistent dependency consumer sets. 14 focused tests
 + strict Clippy passed; narrow paired timings retain construction/tiny costs.

All four slices are frozen at ed7a5a4. Combined job 57247 is LIVE: prebuild formatting,
strict Clippy and affected-target compile, then one scripts/check.sh. Logs are
work/parallel-round24-{prebuild,final-check}.log. Do not mutate crates/provenance
until terminal 0. Then run the stable launcher with work/record_parallel_round24.py
once (prepared, NOT RUN). Expected 3363 source methods; no new method credit here.

Ready/copied evidence: `work/parallel-round24-reviewed.json`,
`work/parallel-round24-evidence/`, `work/parallel-round24-commits.json`.
Draft reviews: `work/parallel-round24-draft-reviews.json`.
Prebuild/recorder scripts are prepared for the four final schemas and expected
source count 3363. Never relabel old result hashes as new verification.

## Performance evidence and limits

Compact Change on runner-a, actual 64-proposal solves with 4096 payload-owned values:
cross-row 3.740x list / 3.258x mixed; same-row 1.898x / 2.109x. Cross-row peak allocation
falls 720,760 bytes. Countercases remain: list value scopes+9.175%(+0.770ms), tiny
mixed usize+11.62%(+3.769us), same-row usize list+5.54%, prior Swap payload
+6.54%/+5.95%, composite payload+4.70%/+3.47%. No further tuning this batch.
These are native baseline/current measurements, not Java comparisons.

Persistent consumers: runner-a 31453 terminal 0; 108 timing rows / 12 allocation rows.
Large repeated checkpoint rewires 32.00→3.03us; allocation calls 712→45 and peak
extra requested live bytes 81,792→2,352. Tradeoffs: large setup 4.131→5.172 ms,
setup peak 1,995,136→2,341,072 bytes; tiny rewire 0.854→0.977us. Large propagation
is effectively unchanged. Final evidence is copied and hashed under
work/parallel-round24-evidence/consumers. No whole-solver speed claim.

Original-Java list report was freshly measured at round22 on runner-a:
320 valid / 317 equal scores; optima Rust 266 / Java 263. Nine matching-score timing
workloads Java/Rust 2.738–76.668x; three unequal-quality workloads are separate.
Bundle: `work/parallel-round22-list-aligned/`. This report is now visibly stale
for changed solver inputs. Do not automatically refresh every benchmark.
Remote absolute-path verification records were not imported as local hits.

## Three active agents and build machines

- dense_scoring: `/home/ralph/.codex/worktrees/dense-list-scoring/TimeTuck`.
  Next: sparse phase-best assignment refresh in retained scoped search, including
  intervening non-best commits, Clone panic staging and dense/tiny fallbacks.
- dependency_graph: `/home/ralph/.codex/worktrees/dependency-graph/TimeTuck`.
  Inspecting the next missing Community selection/dependency behavior family.
  runner-a 31453 is terminal and released.
- termination: `/home/ralph/.codex/worktrees/diminished-returns/TimeTuck`.
  Implementing opt-in best constraint-total metrics through fresh AND retained
  calculator solves, basic/list/mixed and plan/exact/repair transitions, without
  extra scoring. Preinitialized matching-disabled directors report unavailable;
  noncalculator backends may be explicitly unsupported. Fresh calculator entry
  points must enable requested matching; retained-only support is insufficient.

Local: 20 threads / 32 GB, Rust 1.92 stable. Parent combined prebuild uses -j12;
isolated focused -j2 agent work is allowed. Avoid local timings during builds.
runner-a: developer@runner-a.internal, 8 threads / 48 GB, glibc 2.44. Coordinate benchmark windows.
runner-c: developer@runner-c.internal, 16 threads / 16 GB; latest 30780 SSH timeout. User plans to wake
it; retry opportunistically, do not repeatedly ask or restart unknown old jobs.
Use BatchMode=yes,ConnectTimeout=10,StrictHostKeyChecking=yes for SSH.

`work/remote_worker.py` builds immutable manifests on remote workers. It injects
root.cargo/config.toml, which changes dashboard measurement fingerprints. For
original-Java measurements use the aligned launcher with isolated CARGO_HOME
vendor config and an exact expected digest preflight. Keep binaries/manifests.

## Verification and continuation

Batch related Rust changes, run focused checks while implementing, then one
`bash scripts/check.sh` on the frozen coherent batch. Use
`python3 work/with-verification-environment.py COMMAND ARGS` for stable PATH and
environment. Reuse unchanged evidence; a commit/turn/reset is not an invalidator.
Shared solver changes still need broad regression coverage. Benchmarks run only
for a relevant performance question; old reports remain visibly stale.

The full historical handoff is preserved verbatim in
[the archive](archive/next-session-through-round23.md). Consult it only for a
specific older result. Keep this current handoff concise by replacing outdated
status rather than appending duplicate histories.

Read `docs/compatibility.md` for methodology and existing adaptations. Relevant
behavior contracts include solver-contract.md, incremental-search-contract.md,
constraint-stream-contract.md, planning-contract.md, list-planning-contract.md,
mixed-planning-contract.md and score-contract.md. All live under docs/.
