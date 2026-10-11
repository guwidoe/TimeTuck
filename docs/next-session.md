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

## Latest verified checkpoint — round36

Main 0a87b48 adds TimedScoreRing and connects it to diminished-returns history.
It supports inclusive window-predecessor eviction, cursor/slot checkpoints,
validated restoration, wrapped growth, explicit full/empty lengths and zero/
duplicate/u128::MAX timestamps. Backwards insertions are rejected. Runtime active
samples use Rust's deque; imported inactive slots are retained separately and
checkpoint layout is materialized on demand. Growth moves scores without Clone.

All eighteen DiminishedReturnsScoreRingBufferTest methods retain all 101 source
assertions, with complete provenance. Four original regressions check 120000
mixed operations against an independent deque, restored snapshots, move-only
payload release, extreme times, capacity failures and malformed states. Actual
termination retains same-time replacement, backwards-clock clamping, grace/reset,
exact rate arithmetic and pre-drop cache invalidation on panicking evicted scores.
Forty-eight focused tests passed on the earlier slot-backed draft. The final gate
verified the final deque-backed implementation and its native runtime integration.
An initial wrong target name and test inference errors were fixed; failed logs
remain available. Do not repeat these checks without changed relevant inputs.

Final runner-c gate 11740 and collector 38548 both finished successfully:

- 6,580 Rust tests and 282 doctests.
- 3,454/4,855 source methods passing (71.143%); 835 pending,
  566 excluded, zero failed/unverified. Backlog 853→835; exclusions unchanged.
- 846 Rust jobs ran because shared termination inputs changed; 105 Rust and
  46 Python jobs reused unchanged runner-c evidence. Strict format/Clippy passed.
- Source `b514316d5a2ed116259a36adb77cf4fee8a8bc1227c14a9c2db5513e03dbdad7`.
- Verification `5233ae8bac10a909506bb7317cbebaa643f223235e75b6d549f26dcb3b7b8d50`.
- Remote snapshot `3b90f5c33a11c9457cc84132ed9805f554427b7120341bd9e40852dc228861a3`.
- Evidence work/parallel-round36-evidence.json, freeze metadata, original
  manifest/log and work/round36-runner-c-results.tar.gz retained. Final gate took
  722.899 seconds, including broad target compilation. No worker/collector remains
  running. Next runner-c batch seeds these real logs, using this output root and the
  existing stable CARGO_HOME from work/round36-runner-c-check.py; never rewrite keys.

Performance was checked on runner-a, separately from source-method credit:

- Rejected custom slot draft: history microbench ring/deque ratios1.162–2.013.
  Final design retains the native deque for active samples.
- Paired standalone exact policies (28 samples/four workloads), intermediate
  deque-ring snapshot58e13615: changing-score workloads cost1.011–1.034x baseline;
  repeated cached polls cost1.245x, approximately10.61→13.28ns per poll.
  All termination decisions/sample counts match. Not full solver/Java timing;
  this intermediate benchmark uses checked push, while final native insertion
  skips its duplicated chronological check. No final-state speedup claim.
  Evidence work/round36-policy-performance-summary.json and original manifest/log.
- A NOT INTEGRATED generic-clock/guarded-pruning candidate reduces cached polls to
  1.015x baseline (11.14→11.25ns); changing-score workloads1.019–1.035x.
  All benchmark decision/sample counts match. Snapshotfd912d8b; evidence
  work/round36-clock-candidate-performance-summary.json and original manifest/log.
  Candidate files work/round36-generic-ring-candidate.rs and
  work/round36-native-clock-policy-candidate.rs are saved. The latter contains
  only the policy prefix before LocalTimeBudget; preserve the remaining production
  module if applying it. Source cases/solver regression verification has NOT run
  for the candidate. Consider it with the next coherent batch; do not claim it is
  current or spend time repeating old baseline checks. Temporary examples were
  removed from crates and retained as work/*-perf-example.rs.

Dashboard http://localhost:8765 is current; report data/freshness checked. Reuse its
server. Backend refreshes need data inspection, not browser/Impeccable. Existing
original-Java list measurements remain visibly stale; no refreshed Java speed claim.
This goal turn was progress (eighteen verified cases plus runtime history storage).
Full objective remains incomplete and active, confirmed by the goal tool this turn.
No new subagents or assignments.

Prior round35 (707539c/c0dbbd1) passed: 6,558 Rust/281 docs, 3,436 source methods,
853 pending. All six AcceptorFactoryTest methods and two negative-size methods
already work with prepared native policies, canonical/deduplicated selection and
checked annealing losses; 27 real manual/prepared solver comparisons cover nine
backends and fresh per-search state. Native numeric defaults/keys/PRNG schedules
are independent choices, without Java reflection/environment-instrumentation
parity claims. Do not repeat its final gate or focused tests unchanged.

Prior round34 (6b9c4dc/a14b345) passed: 6,547 Rust/280 docs, 3,428 source methods,
861 pending. All nine EnvironmentModeUtilTest query/validation cases are already
integrated. Prepared NoAssert/PhaseAssert policies select native phase score
checks with independent caller scoring (18 real solves). Global policy remains
separate from phase policy; synthetic initial scoring uses global mode.
Step/undo/tracked/full/nonintrusive diagnostics, reseeding and score-director
replacement remain pending. Do not claim runtime parity from mode validation.
Do not repeat its gate or focused checks without changed relevant inputs.

Prior round33 passed: 6,534 Rust/279 docs, 3,419 source methods, 870 pending.
Compiled shadow-structure inspection/Next links and all twelve GraphStructureTest
methods are already integrated and verified; do not repeat that 839-target gate.
All completed agent work is integrated and committed. No new subagents or follow-up
assignments. Continue closing complete pending Community cases while preserving
native performance. This goal turn was progress (eight verified configuration
methods and real solver binding). Full objective remains incomplete and active,
confirmed by the goal tool this turn.

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
