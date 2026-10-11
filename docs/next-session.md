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

## Latest verified checkpoint — round25

Combined gate 96362 and recorder 83996 completed successfully:

- 6,347 Rust tests and 277 doctests.
- 3,364/4,855 upstream methods passed (69.289%); 1,227 pending, 264 excluded,
  zero failed/unverified. These are annotated methods, not expanded invocations.
- 813 Rust jobs ran; 103 Rust and 46 Python jobs reused unchanged evidence.
- Source: `fbacf1750114d4f0c5b2882df6ce83912428126ac5113def2e242ee6a2728bb9`.
- Verification: `5139061e739cec8cf2da942a7453fcd3f6e04f663e8f9e3b5026473d7a917050`.
- Evidence: `work/parallel-round25-evidence.json` and its evidence directory.

Verified round25 includes calculator best-constraint metrics, sparse phase-best assignment
refresh, live list changes, and one complete pinned random-list source adaptation.
Original-Java list measurements refreshed on runner-a and published current at round25.
Previous round24 evidence remains retained; do not repeat its reviews or checks.

Dashboard: http://localhost:8765. Reuse the server. Backend-only refreshes require
report/freshness inspection, not a repeated browser check.

## Current integration batch — round26

Reviewed and committed on main:

- c3a7147 (agent 5f1aaf7): refresh retained calculator payloads on first exact entry
  and each repair reconstruction; preserve actual evaluation and notification work.
  Thirty focused tests and Clippy; failing old payload/score reproducer retained.
- 76091c8 (agent e4a33a4): correlated list destination factories borrow current context,
  canonical values and shared indexes. Bounded rejected work; cursor-drop panic fusion.
  Twelve distinct focused tests and Clippy, real configured-provider integration.
- d8e4413 (agent a5ae057): atomic director-local live weight overrides across basic,
  list and mixed networks. Zero disables callbacks; reactivation scores current
  assignments. Effective weights survive retained search, explanations and rollback.
  Forty-two focused tests and Clippy, including 60 retained reference comparisons.
- 69ee64f (agent b5a8f74): owned-context streamed swap providers and paired-read panic
  fusion. Fourteen focused tests and Clippy, fresh assignment snapshots per opening.
- e7ffc15: classify 302 relocated Java migration/Quarkus methods under existing
  exclusions. Pending becomes excluded; this earns no passing-method credit.

All four production/test slices reviewed; evidence copied and hashed under
`work/parallel-round26-evidence/`. Integrated stable patch IDs match agent commits.
d3094c4 (agent 134c93c) adds completed bounded-exact expansion steps to lifecycle,
metrics and step/stagnation limits; enumeration stays step-free. 51 focused tests
and Clippy passed; full source/assertion review complete. e27f1a5 records three
complete BruteForce methods and 12 fixtures. Expected source passed count 3367.

Rust/provenance frozen at e27f1a5. Combined gate **79649 LIVE**; logs are
`work/parallel-round26-{prebuild,final-check}.log`. Do not mutate main Rust/provenance
until terminal zero. Then run prepared `work/record_parallel_round26.py` through
`work/with-verification-environment.py`. Do not publish round26 as verified before that.

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

Original-Java list report freshly measured at round25 on runner-a:
320 valid / 317 equal scores; optima Rust 266 / Java 263. Nine equal-score workloads
Java/Rust 2.725–76.538x; three unequal-quality workloads remain separate. Same-host,
same-run timing; no attribution of all differences to a particular optimization.
Bundle: `work/parallel-round25-list-aligned/`; remote 67038 and collector 88545 terminal 0.
Measurement digest `0e4893f822f7cde04d47c6d59b24f6475cab6ef3a3a3c517d20bc723180227af`.
Remote absolute-path cache records were not imported as local hits. Changed solver
inputs in round26 may make this stale again; do not automatically refresh benchmarks.

## Three active GPT-6 Astra/high agents and build machines

- dense_scoring: `/home/ralph/.codex/worktrees/dense-list-scoring/TimeTuck`.
  Live weights a5ae057 integrated. Active next: live stream-graph weight overrides,
  retaining source handles and rebuilding disabled branches on reactivation; atomic
  weight/topology/match/total updates. Graph/scoring files reserved for this lane.
- dependency_graph: `/home/ralph/.codex/worktrees/dependency-graph/TimeTuck`.
  Correlated sources and swap provider integrated. Active coherent batch: owned
  context support for ListValueSource, SubListSource, EntitySubListSource/cardinality,
  enabling existing composed sublist change/swap providers. Preserve algorithms.
- termination: `/home/ralph/.codex/worktrees/diminished-returns/TimeTuck`.
  Payload fix integrated. Bounded exact expansion lifecycle reviewed: actual
  completed expansion steps feed limits/stagnation/lifecycle/metrics; leaf enumeration
  stays step-free. 51 focused tests passed. Three complete BruteForce methods have
  real native expansion counts 3/9/15 within source ceilings 3/25/25; provenance integrated.
  Selecting next substantive phase/control capability; avoid diagnostic-only work.
  Other 4 BruteForce methods stay pending. Ring-buffer audit complete with no credit:
  Java backing-capacity details are unexposed and native exact-window behavior exists.

Local: 20 threads / 32 GB, Rust 1.92 stable. Parent combined prebuild uses -j12;
isolated focused -j2 agent work is allowed. Avoid local timings during builds.
runner-a: developer@runner-a.internal, 8 threads / 48 GB, glibc 2.44. Coordinate benchmark windows.
runner-c: developer@runner-c.internal, 16 threads / 16 GB; SSH 41403 timed out this turn. User plans to wake
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
