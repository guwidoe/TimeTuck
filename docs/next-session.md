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

## Latest verified checkpoint — round24

Combined gate 57247 and recorder 3055 completed successfully:

- 6,323 Rust tests and 277 doctests.
- 3,363/4,855 upstream methods passed (69.269%); 1,228 pending, 264 excluded,
  zero failed/unverified. These are annotated methods, not expanded invocations.
- 815 Rust jobs ran; 97 Rust and 46 Python jobs reused unchanged evidence.
- Source: `b52f091f6836b2ad832c51d30d3bc826e10d66eb5a02929712478264fd62d7e7`.
- Verification: `24fad1388eb223c0f33ad15b14da6ff302059519fdcc3a235bcf6a7d704c2f33`.
- Evidence: `work/parallel-round24-evidence.json` and its evidence directory.

Integrated round24: 42a56b8 lazy weighted union, a1096d9 temporary calculator
what-if rollback, 261a4ff compact primitive Change undo, ed7a5a4 persistent
consumer sets. Full parent reviews, focused evidence and performance tradeoffs
are retained. These native additions earn zero source-method credit.
Previous round23: 6,303 Rust/276 docs, same 3,363 source methods; evidence retained.

The dashboard is served at http://localhost:8765. Reuse the server. Backend-only
refreshes require report/freshness inspection, not a repeated browser check.

## Current integration batch — round25

Three reviewed slices plus provenance are committed on main:

- 072934f (agent e73896a): opt-in best constraint-total metrics for fresh AND
  retained calculator solves across basic/list/mixed construction, local, custom,
  repair and exact routes. Immutable totals exclude justifications; matching-off,
  unsupported and no-best states are explicit. No extra scoring/factory/reset/clone
  work in the paired runtime matrix. 29 focused runtime tests and strict Clippy;
  final three lint allowances separately proved. 18 artifacts/11 files hashed.
- fe320ee (agent db37a1b): sparse phase-best assignment refresh tracks committed
  changes including intervening non-best/net-zero steps, stages user Clone before
  publication, and uses full fallback for tiny/dense/whole replacements. Lazy
  bitmap allocation. 22 focused tests; final changed six + strict Clippy. Full
  production, unit/public differential tests and actual-solve probe reviewed.
  58 hashed artifacts/10 files; 864 paired samples/48 exact-equality workloads.
- 1617a51 (agent 9200aa1): live list value/destination sources, pin filtering before
  pairing, bounded rejected work, range/dependency validation, PRE-removal metadata
  normalized to executable Change/Assign/Unassign. Borrowed and owned contexts;
  real ConfiguredMoveProvider opens against current assignment. 18 distinct
  focused tests + strict Clippy; final Rustdoc-only delta separately recorded.
  14 artifacts/5 files hashed. Per-value factories/mimic semantics remain pending.
- 7dcaa7c: provenance for one COMPLETE randomWithPinning method and six fixtures.
  Parent reviewed full source method/helper assertions and local adaptation.

Main Rust+provenance frozen at 7dcaa7c. Combined job **96362 LIVE**: fmt/strict Clippy passed (31.47s);
affected-target prebuild then one scripts/check.sh. Logs:
`work/parallel-round25-{prebuild,final-check}.log`. Do not mutate crates/provenance
until terminal zero. Then run:
`python3 work/with-verification-environment.py python3 work/record_parallel_round25.py`
once (prepared, NOT RUN). Expected source count 3364, not yet verified on main.

Evidence/indexes are copied under `work/parallel-round25-evidence/`; all final
committed files/artifacts hashed, integrated zero-context stable patch IDs match.
Metadata: `work/parallel-round25-{reviewed,commits,freeze}.json`. Reviewed draft
snapshots: `work/parallel-round25-draft/`. No need to repeat completed reviews.

## Performance evidence and limits

Round25 sparse phase-best on runner-a, final job28365 terminal0/released:
large4096 sparse usize1.411x/1.443x/1.446x basic/list/mixed; payload1.521x/1.464x/1.431x;
wide list/mixed payload1.491x/1.469x. Countercases: basicusize64+14.3%(3.411us),
basicusize single4096+17.8%(18.859us), mixedusize single+5.29%, tiny/32 payload+3.7–8.4%.
Sparse peak +4240B one component/+8368B mixed; no-best final basicpayload+1.42%,
list+0.23%, mixed−2.16%, peak overhead128B. All48 workload results/work/payload
checksums match across864 samples. Original v1 basic no-best+71.8% and isolated
clone diagnostic retained separately; cause not established. No tuning sweep.
These are native baseline/current measurements, not original-Java comparisons.

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

## Three active GPT-6 Astra/high agents and build machines

- dense_scoring: `/home/ralph/.codex/worktrees/dense-list-scoring/TimeTuck`.
  Sparse phase-best slice committed db37a1b, all jobs terminal. Follow-up active:
  choose next substantive scoped-scoring/list behavior family from pending tests,
  preferably complete adaptations; no more snapshot tuning, avoid calculator and
  list_value_change source modules. Awaiting proposed scope.
- dependency_graph: `/home/ralph/.codex/worktrees/dependency-graph/TimeTuck`.
  Live source slice committed 9200aa1. Approved next: correlated value→destination
  factory source with bounded per-value and total work; empty/rejected branches
  advance values; actual provider integration. Reuse destination index. Remaining
  Java random range traces do not justify method credit; avoid forced mappings.
- termination: `/home/ralph/.codex/worktrees/diminished-returns/TimeTuck`.
  Metrics committed e73896a. Approved next: reproduce and fix retained calculator
  exact/repair assignment==incoming shortcut losing Eq-by-ID payload differences,
  returning stale scores. Keep fix at transition boundary with fresh-reference
  and notification/accounting checks; no broad sparse exact optimization needed.

Local: 20 threads / 32 GB, Rust 1.92 stable. Parent combined prebuild uses -j12;
isolated focused -j2 agent work is allowed. Avoid local timings during builds.
runner-a: developer@runner-a.internal, 8 threads / 48 GB, glibc 2.44. Coordinate benchmark windows.
runner-c: developer@runner-c.internal, 16 threads / 16 GB; SSH37898 and14302 timed out this turn. User plans to wake
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
