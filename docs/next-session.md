# TimeTuck implementation handoff

## Objective and boundaries

Build an independent Rust constraint optimization solver using pinned Timefold
Community tests/public APIs as specifications, with a visual compatibility and
performance dashboard. Objective incomplete; goal tool confirmed active this turn.
Keep pursuing the full objective; this batch is not completion. Never copy or translate
upstream production; Community only, no formal clean-room claim. Upstream pin
remains v2.7.0, `8b815bc6b5f3eb81feedc94dea0885a93b267762`.

Record complete adapted tests/fixtures in `compat/provenance.json`. Partial,
original, skipped or empty tests earn no upstream-method credit. Crates remain
unpublished. Conventional Commits, never amend/rebase/push. The current user direction is to finish integrating completed work, then close
compatibility gaps so the pending backlog shrinks. Keep performance important,
commit occasionally and reuse unchanged verification. No new subagents or
assignments; all previous agents have finished.

## Latest verified checkpoint — round33

Main 0f347ba adds compiled shadow-source structure inspection and explicit Next
source-member/parent support. Complete per-entity supplier bindings distinguish
empty, directional, single-type and general declarations. Empty groups suppress
only their grouped paths; populated groups retain declared directions even when
later links are null. Nondeclarative entity types do not enlarge the target set.
Inspection reads no terminal getters, invokes no suppliers and clones no entities.
Shape is snapshot-dependent declaration structure, not cycle detection or a solver
execution-plan switch. Existing evaluation and scoring algorithms are unchanged.

All twelve GraphStructureTest methods now have complete adaptations and provenance
for eleven fixture classes. Five original regressions cover forward dependency
keys, live group reclassification, local/mixed directions, invalid binding checks
and forbidden getter invocation. All 43 focused old/new checks passed on runner-a;
Clippy subsequently caught a test initializer style issue, which was fixed before
the final runner-c check. Failed attempt logs/manifests remain available.

Final runner-c gate 1515 and collector 56317 passed on frozen inputs:

- 6,534 Rust tests and 279 doctests.
- 3,419/4,855 source methods passing (70.422%); 870 pending, 566 excluded,
  zero failed/unverified. Backlog 882→870; exclusions unchanged.
- 839 Rust jobs ran; 105 Rust and 46 Python jobs reused unchanged runner-c evidence.
- Source `526f0d19b7e1119c3a065868edcab9e8598640145401b9f126e8e0496a562528`.
- Verification `92a5945e51ddc5e72e529f92dccdeecd117d3388f304fdc2e89e65a81b80e35d`.
- Evidence work/parallel-round33-evidence.json and freeze metadata; original
  manifest/log and downloaded results archive retained. Gate took 709.71 seconds.
  This compiler change selected broad shared solver/shadow-session dependencies;
  do not repeat the completed check without relevant changed inputs or concerns.

Dashboard http://localhost:8765 is current. Reuse its server. Backend report
refreshes need data/freshness inspection, not browser/Impeccable. Original-Java
list measurements remain visibly stale. No new performance claim in this batch.

Prior round32 passed: 6,517 Rust/279 docs, 3,407 source methods, 882 pending.
All completed agent work is integrated and committed. No new subagents or follow-up
assignments. Continue closing complete pending Community cases while preserving
native performance. This goal turn made progress (twelve verified domain methods);
the full objective remains incomplete and active.

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

## Verified integration batch — round29

Seven already-reviewed slices committed together as main `2facd15` (29files,
3442insertions/74deletions). No cherry-pick conflicts; one test expression formatted.
Focused and combined checks passed; do not repeat them.

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

## Completed batch — round30

All completed agent work is now integrated. No agent tasks remain.

- 741ed79 cherry-picks31fbffc without conflicts: retained basic exact search fully
  refreshes source rows on its first candidate, then propagates only changed rows.
  Public replacement/repair retain full refresh. Full implementation/tests/probe
  review completed;17artifacts and3sourcefiles hashed/copied under
  `work/parallel-round30-evidence/sparse-stream-exact`.16focused tests/strictClippy
  reused;100paired native samples/10workloads independently match outputs/logical
  work. Wide128row/4096candidate workload ratio0.037–0.039, tiny/first roughlyflat.
  These are isolated native before/after timings, not combined/Java claims.
- 5b11ce8 adds native IndexedIntArray and20complete DynamicIntArrayTest methods,
  including all3negative-index parameter values, plus one complete optional basic
  BruteForce method. Preserve lifecycle ceiling3 and first-unassigned tie choice.
  Provenance includes3adapted optional fixtures. Independent sparse-map window
  reference is original and earns no method credit.25focused tests passed94854.
  Verified source count3389, pending900; exclusions unchanged566.

The combined gate, report refresh and input/freshness checks all passed. Full
fmt/Clippy, affected runtime targets and doctests passed; unchanged results reused.
No outstanding integration or agent task remains. Review/evidence directories
retain the final optimization's source/probe checks and native paired samples.
Do not repeat source reviews, focused tests or these completed combined checks.

The next work should close remaining compatibility gaps, prioritizing meaningful
solver/domain/lifecycle behavior and complete source assertions. Keep performance
important without replacing backlog work with additional optimization sweeps.
No new subagents or follow-up assignments. Public API/ABI parity with Java remains
out of scope; avoid counting partial mock/internals substitutes as full methods.

Verification stays on runner-c. CARGO_HOME points to the unchanged round29 vendor setup,
so environment keys remain stable. Remote cache log filenames are relocated after
checksum validation; original input keys, output hashes and timestamps remain.
run_job independently checks each new input/environment key before reuse. Remote
execution is not relabeled as local execution. New commits alone do not invalidate
checks. Keep original manifests/archives and failure logs. No new subagents.

Compact derived-output experiment797eed0 is DEFERRED: about19%fewer allocations
but only0–1.5%large gains and up9.24%regressions. Evidence preserved under
`work/deferred-compact-outputs/`; not on main. Do not resume tuning. Redundant
construction-order API also remains deferred in the termination worktree.

Do not repeat completed factory/reflection/undo-count/temporary-flaw/ring-buffer/
exact Java k-opt descriptor audits. Native cancellation extension remains deferred.

Local20threads/32GB/Rust1.92stable; integration builds moved to runner-c with six jobs.
No local timing during parent builds. runner-a developer@runner-a.internal,8threads/48GB/glibc2.44;
coordinate exclusive timing. runner-c developer@runner-c.internal,16threads/16GB, now reachable
(hostnameNB-FH1MY); available for integration verification. TimeTuck builds finished; all agents stopped.
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
