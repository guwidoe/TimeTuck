# TimeTuck implementation handoff

## Objective and boundaries

Build an independent Rust constraint optimization solver using pinned Timefold
Community tests/public APIs as specifications, with a visual compatibility and
performance dashboard. Objective incomplete; goal tool currently reports paused.
Do not claim it is active or use update_goal to resume it. Never copy or translate
upstream production; Community only, no formal clean-room claim. Upstream pin
remains v2.7.0, `8b815bc6b5f3eb81feedc94dea0885a93b267762`.

Record complete adapted tests/fixtures in `compat/provenance.json`. Partial,
original, skipped or empty tests earn no upstream-method credit. Crates remain
unpublished. Conventional Commits, never amend/rebase/push. The user requests
three concurrent GPT-6 Astra/high implementation agents, performance progress,
occasional commits and selective verification without repeated checks/process overhead.

## Latest verified checkpoint — round28

Main159957f combined gate31794 and recorder25771 passed:

- 6,434 Rust tests and 277 doctests.
- 3,368/4,855 upstream methods passing (69.372%); 921 pending, 566 excluded,
  zero failed/unverified. No new source-method credit in this performance batch.
- 883 Rust jobs ran; 48 Rust and 46 Python jobs reused unchanged evidence.
- Source `c749386e88a54c50f22cc05928f27072d29b4878b0ba5c17166ebc34a05edb48`.
- Verification `0d3ddfdf25ed9d59bd6a278bbc28f923886f833278d2ea10f7650ab02714e0d3`.
- Evidence `work/parallel-round28-evidence.json` and associated freeze/review files.

Dashboard http://localhost:8765; reuse its server. Backend-only report refreshes
need report/freshness inspection, not browser/Impeccable. Original-Java list report
was current at25 and remains honestly stale; no automatic benchmark refresh.

## Verified implementation batch — round28

Six reviewed slices committed on main:

- 53bddb5 ←11e0e4c: configurable integer k-opt size weights across both samplers,
  neighborhoods and providers. Arc-shared weights prepared per opening, impossible
  and zero-weight sizes excluded, uniform default seeded draws preserved. Seven
  public configs now Clone-only (authorized unpublished API migration).135files,
  predominantly caller clones/borrows/format;34focused tests, strictClippy,1doctest.
- a4846a0 ←6b000c3: opt-in best constraint totals across scoped/stream basic/list/mixed,
  fresh+retained construction/local/custom/exact/repair. Cached effective weights,
  capture before best publication, no rescoring.21focused tests+strictClippy. Java
  match-policy tests differ from native publication semantics: zero source credit.
- f9ee928 ←82c3182: stream foragers retain existing sparse assignment snapshots,
  preserving full-copy tiny/dense fallback and callback-free winner installation.
  17focused tests+strictClippy;31reference solves and42Clone panic sites. Final
  test-only cfg.clone delta coordinated with non-Copy configs, no redundant rerun.
- 75aa9cd ←a757ca7: balanced O(n) initial binding for large empty unobserved stream
  sources. Keep source rows/handles for inspection/reactivation; ordinary observed,
  activity/batched/small/nonempty paths retain order.15focused tests+strictClippy.
- c72724d ←242e1fe: opt-in contextual hash index inside GroupedSelectionSource,
  preserving class/member order, duplicate occurrences and owned snapshots.
  Default Eq-only path retained, collisions checked, panic-safe publication.
  17focused tests+strictClippy; no new provider facade.
- 335c715 ←3b568ee: remove repeated domain/pin/shape validation only for trusted
  fresh scoped basic exact generators after entry validation. Public calculate and
  list/mixed paths stay validated; shared arithmetic/structure/metrics code.
  19focused tests+strictClippy; full native generator invariant inspected.

Full implementation/tests/probes reviewed once; final deltas checked, source and
artifacts hashed/copied. All six earn zero new source-method credit; expected3368.
Bundles `work/parallel-round28-evidence/`; draft snapshots prevent repeat reviews
under `work/parallel-round28-draft/`. Metadata `-reviewed.json`, `-commits.json` and
`-freeze.json` record agent/main commits and evidence indexes.

The combined check and report refresh succeeded. Do not repeat them. Integration
needed39 test-only caller clones after the non-Copy config migration; commits
 d272346,1677bff,159957f and failed compile logs preserve that history. No production
behavior changed during integration. Metrics has a reviewed zero-context patch
boundary exception; winner-module conflict retained the existing conversion once.

## Performance evidence and limits

These are separate native baseline/current paired measurements, not combined or
original-Java speed claims. Keep countercases and old evidence genuinely stale.

- Round28 stream winners, runner-a540samples/30workloads:32768 list sparse solve
  47.949→27.050ms(1.773x), peak57.27→7.61MB; basic1.165x,mixed1.101x. Large first
  accepted roughly flat. Tiny mixed dense+17.76%(+7.697us), list first+13.39%
  (+1.154us), list optional+9.05%(+10.055us). No tuning sweep.
- Round28 unused-source binding, runner-a540samples/30workloads:32768 list setup
  17.065→8.913ms(1.915x), sparse solve26.917→20.734ms(1.298x), first1.426x;
  mixed sparse1.158x. List solve peak7.61→8.65MB; setup peak6.82→8.65MB.
  Tiny list observed+4.11%(+6.143us), sparse+3.68%(+4.369us); observed large+0.80%.
- Round28 basic exact, runner-a120samples/12workloads:wide-domain enumeration
  current/baseline0.437(solve)/0.432(setup+solve), bounded0.527/0.532. Tiny0.822–0.899;
  expensive-score0.952–0.991, with per-batch1.258outlier retained. Both variants
  include result formatting; exact result/work strings match all60pairs.
- Round28 grouping, exclusive local108samples/54pairs:4096distinct actual provider
  consumption25.99ms→0.988ms (median paired27.07x). Few groups+19.6%,tiny indexed+26%.
  Forced collisions remain quadratic/no gain claim; default4096+2.47%,default8+9.41%
  noisy. Group/member checksums and counts match. Opt-in use case, not solver timing.
- Round27 value lookup: full configured provider6.46x, first flat, raw tiny+11–13%.
  Structural cache:32768 actual basic/mixed/list solves1.093x/1.140x/1.005x;
  tiny basic+7.23%, optional4096list+7.56%. Full evidence retained in27.

Original-Java list round25 runner-a report:320valid/317equal scores; Rust266/Java263
optima. Nine equal-score workloads Java/Rust2.725–76.538x; three unequal-quality
workloads remain separate. Bundle `work/parallel-round25-list-aligned/`, remote67038
and collector88545 terminal0. Measurement digest
`0e4893f822f7cde04d47c6d59b24f6475cab6ef3a3a3c517d20bc723180227af`.
Remote absolute-path cache entries were not imported as local hits.

## Integrated batch — round29 (combined check running on runner-c)

Seven already-reviewed slices committed together as main `2facd15` (29files,
3442insertions/74deletions). No cherry-pick conflicts; one test expression formatted.
Focused checks passed before integration; full batch status remains pending.

- abc9331: reuse propagation queue/score-delta buffers. Large workloads allocate
  about20%less, modest1–3%timing gains; tiny mixed sparse+6.40%, peak+96–1024B.
- aa67735: indexed missing-rank subgroup selection for k>=2048. Large provider
  2.4566x faster; small controls nearflat. RNG ranks/order preserved.
- a97a7fd: forward validated source events directly. One allocation removed per
  ordinary row, large sparse solves1.015–1.027x; tiny fanout regressions retained.
- 464b443: correlated list-swap partners with retained cursors/shared validation.
- 42160b6: typed per-phase optimistic bound callbacks across18 solver routes;
  deterministic capacity example127nodes→13–65nodes with identical optimum.
- 0a0c16c: correlated sublist destinations, bounded rejection and real provider use.
- 2ca2149: preserve unchanged existence witness links. Dense256 retained solve
  440.780→148.073ms(2.977x), peak41.91→17.75MB; tiny churn+2.91%, sparse setup+8.88%.

All timing claims are separate native before/after workloads, not combined or Java
comparisons. All seven earn zero additional source-method credit. Reviews and
copied focused evidence: `work/parallel-round29-reviewed.json` and
`work/parallel-round29-evidence/`; frozen main inputs `work/parallel-round29-freeze.json`.
Do not repeat completed source/probe reviews or isolated tests.

User authorized moving integration verification to runner-c. Local gate64078 was stopped
during Clippy (no runtime tests); remote final launcher29321 runs immutable main
snapshot with six build jobs, then unchanged `bash scripts/check.sh`. First remote
launcher39729 was stopped before runtime tests to correct a helper-file mutation;
its logs are retained. Final label `round29-integration-final`, manifest/log under
`work/remote-round29-integration-final*`. Vendoring config is in remote CARGO_HOME,
keeping Rust repository input hashes identical. Collector48317 (`work/collect_round29_runner-c.py`) waits for this one build, downloads
actual logs/metadata, checks the source digest and generates the local report;
do not pretend remote cache records are local execution. Source tests remain
unchanged during the gate. Python tests run remotely; upstream inventory/report
runs locally because the worker does not contain the upstream Git submodule.

## Existing agents — finish only, no new work

User explicitly says to stop creating agents or assigning new work once current
tasks finish. Do not refill slots. All existing agents are GPT-6 Astra/high.

- dense_scoring and dependency_graph completed their tasks; do not restart them.
- termination finished and stopped at31fbffc (based42160b6): private generated-basic
  retained-stream exact transition optimization. Two production files plus original
  sparse_stream_exact tests;16focused tests and strictClippy passed. One bounded
  runner-a before/after comparison completed: wide128row/4096candidate exact solves
  about0.037–0.039x baseline time, tiny/one-candidate cases roughlyflat. Agent claim,
  not yet parent-accepted or integrated; production25line delta inspected, full new
  test/probe/evidence review pending. Index `work/sparse-stream-exact-evidence.json`
  in `/home/ralph/.codex/worktrees/diminished-returns/TimeTuck`. Worktree clean, both
  workers released. Do not start another agent or task. Do not change frozen main
  during the29gate; this last slice is for subsequent integration.

Compact derived-output experiment797eed0 is DEFERRED: about19%fewer allocations
but only0–1.5%large gains and up9.24%regressions. Evidence preserved under
`work/deferred-compact-outputs/`; not on main. Do not resume tuning. Redundant
construction-order API also remains deferred in the termination worktree.

Do not repeat completed factory/reflection/undo-count/temporary-flaw/ring-buffer/
exact Java k-opt descriptor audits. Native cancellation extension remains deferred.

Local20threads/32GB/Rust1.92stable; integration builds moved to runner-c with six jobs.
No local timing during parent builds. runner-a developer@runner-a.internal,8threads/48GB/glibc2.44;
coordinate exclusive timing. runner-c developer@runner-c.internal,16threads/16GB, now reachable
(hostnameNB-FH1MY); assigned parent integration gate. runner-a is free; all agents have stopped.
SSH BatchMode=yes,ConnectTimeout=10,StrictHostKeyChecking=yes.

`work/remote_worker.py` uses immutable manifests/vendor sources. Injected root Cargo
config affects measurement fingerprints; original-Java launcher uses isolated
CARGO_HOME/aligned source digest. Preserve actual binaries/logs/manifests.

## Verification and continuation

Batch related behavior; focused checks during development, one parent
`bash scripts/check.sh` per final coherent batch. Use
`python3 work/with-verification-environment.py COMMAND ARGS` for stable PATH/env.
Reuse unchanged evidence; new commits/turns/resets do not invalidate it. Shared
solver changes still need broad regression coverage; never rehash stale results.
Read `docs/compatibility.md` for methodology. Older history is retained in Git,
work bundles and [the archive](archive/next-session-through-round23.md); consult
only for a specific result rather than re-reading its1.7MB wholesale.
