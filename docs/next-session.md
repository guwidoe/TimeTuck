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

## Latest verified checkpoint — round39

Main e32a854 integrates borrowed sample views, passive dataset availability/bulk
consumption guards, signed external set-size validation, and live selection
catalog-bound metadata. This batch is verified progress; the full goal remains
active and incomplete. No agents or further assignments were created.

- NeighborhoodSampleView borrows a slice or live ScalingSequencedSet. Members
  retain original allocations; no T: Clone requirement. Set edits remain visible
  between traversals; active iterators retain the existing mutation guard. Slice
  duplicates are skipped lazily. Bidirectional equality/hash match owned samples,
  and a retained set member remains valid after removal/clear. Live hashes change
  with the backing set and must not be used as stable keys while it changes.
- StreamingSelection already had persistent receiver-cursor behavior. Its new
  catalog_upper_bound uses checked declared contribution bounds without opening
  children/events. Zero/unknown/overflow and ignored holes are distinguished.
  Bound24 preserves the source declaration without inventing actual dependent
  combination/draw cardinality; size remains unknown. Native factories generate
  the complete six-code replay prefix, preserving receiver a through b->1+a->9.
- DatasetRandom::has_next and try_for_each_remaining consume no randomness.
  Nonempty unbounded bulk consumption rejects before callbacks/draws; direct
  owned Iterator::for_each raises a typed error panic. Bounded take(limit) works;
  empty datasets finish normally. This does not guard every iterator adapter or
  unbounded collection operation. Existing next/draw behavior remains unchanged.
- try_with_expected_size validates positive signed external hints, with typed
  zero/negative errors and source message substrings. Native with_capacity(0)
  remains valid and initializes real mutable storage.

Seven complete source cases passed: both remaining Cartesian randomMimic methods,
all three pending Sample.wrap methods, Dataset's random replacement/repetition
method, and ScalingSequencedSet's expected-size validation. All 24 Cartesian and
all 12 Sample source methods now have complete adaptations. Five additional original
regressions cover move-only ownership, storage transitions, allocation identity,
nullable/duplicate equality/hash, mutation guards, no-draw bulk errors/bounded
adapters, catalog bounds and positive/native-zero construction. They earn no
source-method credit. Test catalogs, helpers and domain identities are attributed.

The final focused run passed 12 tests on runner-c. Two initial compile-only failures in
one original metadata regression were fixed with explicit generic/callback types;
logs retained. A fixture review corrected the generated entities to initialized
values before that final run. No runtime test or Clippy failure remains.
Do not repeat focused checks or completed source/helper reviews unchanged.

Final runner-c gate and collector completed successfully:

- 6,620 Rust tests and 282 doctests; strict fmt/Clippy passed.
- 3,474/4,855 source methods passing (71.555%);
  815 pending, 566 excluded, zero failed/unverified. Backlog 822→815.
- 862 affected Rust jobs ran; 98 Rust and
  46 Python jobs reused unchanged runner-c evidence.
- Final gate 683.551 seconds including broad compilation.
- Source `605ab8fbb568180b047e89f93b21b94eeb3f2d1a4e5536a370f8bc8eba9f42b3`.
- Verification `7280a76fa77ef012cefd9c14f3e1d6753bd9409703024e6dec934624d4a9d770`.
- Remote snapshot `cc302875e31074954094aba5fd99ead53b7c81645865f4a8f7877bacab56d26e`.
- Actual logs/manifests/freeze/evidence are retained in work/parallel-round39-*,
  work/remote-round39-integration* and work/round39-runner-c-results.tar.gz. No worker
  or collector remains running. Seed the next runner-c check from this archive/root and
  the unchanged CARGO_HOME in work/round39-runner-c-check.py. Preserve original input
  keys, checksums and timestamps; relocate output filenames only after validation.

Dashboard http://localhost:8765 is current; report data/freshness checked. Reuse
its server. Backend-only refreshes need data inspection, not browser/Impeccable.
Original-Java list measurements remain visibly stale. No new Java/performance
measurement or speedup claim. Prior clock-policy code/measurements are unchanged.

Next substantive candidate: inherited planning-field composition. The 15 pending
SolverEntityInheritanceTest methods were reviewed once; PlanningSchema currently
explicitly leaves inheritance separate. Six methods assert actual final solve
scores 2/4; nine assert invalid variable-defining ancestors, redefinition and
multi-level/mixed declarations. They require real native metadata/solver behavior,
complete individual test-domain/provider fixtures and accurate scope, not static
errors for Java annotation shapes. No new inheritance code/provenance/credit yet.
The interface-negative source method actually generates the class fixture; preserve
that anomaly when adapting. Notes in work/round39-next-gap-notes.json.
LinkedIdentityHashSetTest source was also reviewed once; retainAll is an unsupported
Java method and must not become a constant error API solely for compatibility credit.
Do not repeat completed RootVariableSource getter/reflection precedence audits.

Prior round38 (0ed9037/23ed7bb) integrated provider composition, bounded cached moves,
reversible filtering and real Original/Random cached iterator variants. It passed
6,608Rust/282docs,3,467source methods,822pending;850RustRUN/108RustREUSE/46PythonREUSE,
671.172seconds. runner-c snapshot7c31881236c8a4d595ed8564cbccc4bf4777540bb369236e14def5a6172cf326,
work/round38-runner-c-results.tar.gz and full evidence retained. All10MixedMoveSelector,
all7CachingEntitySelector and all5FilteringEntitySelector source methods adapted.
No measurements were refreshed or Java class identity claimed. Completed checks,
source reviews and the old gate must not be repeated solely for a new turn/commit.

Prior round37 (df0da15/72bf2e3) integrated generic Duration clock storage/guarded
pruning and four full custom structural-routing methods, including independent
full rescoring without replay, rollback/failure/quota accounting across all nine
backends. 6,592 Rust/282docs passed; source3,458, pending831. Gate850RustRUN/
105RustREUSE/46PythonREUSE,686.630seconds. runner-c snapshote75ff8becfaec9463601341e8443920da27b21d629928782aac40152a864efb6,
archive work/round37-runner-c-results.tar.gz and full evidence retained. No checks remain
running. Custom phases reserve local score slots; shared compositions are cooperative.

The integrated clock candidate's prior standalone native policy measurements remain
available, without rerunning/relabeling:28paired samples/four workloads; cached polls
1.015x old pre-round36 baseline (11.14→11.25ns), intermediate ring1.245x old baseline;
changing-score workloads1.019–1.035x. Decisions/sample counts identical. Not whole-
solver/Java timing. Evidence work/round36-clock-candidate-performance-summary.json,
original snapshotfd912d8b and work/round37-clock-evidence-reuse.json. No new changes
to those policy/ring files in round38. Do not repeat the probe without a real concern.

Prior round36 (0a87b48/0aa4de3) integrated timestamped score-ring storage and all
18 DiminishedReturnsScoreRingBufferTest methods with all101source assertions,
plus120000-operation reference, ownership and boundary regressions. 6,580 Rust/
282docs passed; source3,454, pending835. Gate846RustRUN/105RustREUSE/46PythonREUSE,
722.899seconds. runner-c snapshot3b90f5c33a11c9457cc84132ed9805f554427b7120341bd9e40852dc228861a3,
archive work/round36-runner-c-results.tar.gz and full evidence retained. Do not rerun
unchanged checks or rejected custom-slot performance experiments.

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
