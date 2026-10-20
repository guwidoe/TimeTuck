# TimeTuck implementation handoff

## Objective and boundaries

Build an independent Rust constraint optimization solver using pinned Timefold
Community tests/public APIs as specifications, with a visual compatibility and
performance dashboard. Objective incomplete; continue closing the remaining compatibility gaps.
Keep pursuing the full objective; this batch is not completion. Never copy or translate
upstream production; Community only, no formal clean-room claim. Upstream pin
remains v2.7.0, `8b815bc6b5f3eb81feedc94dea0885a93b267762`.

Record complete adapted tests/fixtures in `compat/provenance.json`. Partial,
original, skipped or empty tests earn no upstream-method credit. Crates remain
unpublished. Conventional Commits, never amend/rebase/push. The current user direction is to finish integrating completed work, then close
compatibility gaps so the pending backlog shrinks. Keep performance important,
commit occasionally and reuse unchanged verification. Use up to three concurrent GPT-6.1 Sol implementation agents and distribute
builds/tests across available machines. The user has re-enabled runner-c; use local, runner-a and runner-c for
independent build/test work. Preserve live runs across observation timeouts.

## Current state — 303 same-context pooled scores

Explicitly cached required-basic Dense/Pooled selection can reuse a successful
ordinary score only for an exact normalized base and unchanged active inputs.
Fixed integer scores opt in through a default-false capability, Checked only.
Real assignment validation, fresh structural state, complete saved updates and
all budgets/ties/actual callback counts remain. Eight original regressions add
no upstream-method credit. Native `91293273b471934d676f56b677e5fe18db01a29af86f1ec37f724ae5868fb941`.

Qualified batch: **8,600 Rust tests +292 doctests**, 2 RUN /
146 REUSE, 98.618s. The build-only precompile took 742.150s with eight workers;
normal gate environment retains four. The initial 392.839s gate hit five existing 10 ms archive
fixture deadlines under load; failed suite 02 passed unchanged on selective retry.
Other successful Rust jobs were reused; initial failure evidence is retained.
Source/artifacts/complete controls are bound in
work/round303-qualification/ and compat/observations/pooled-score-memo.json.
All four original repair workloads and three native representations retain four balanced forks.
Whole 96 cached is 21.55% faster in all four forks; 64 cases in three native modes retain
full results/physical calls. Uncached half 96 initially 8.74% slower; targeted eight-fork
follow-up 1.99% slower, mixed forks. No general speed or memory saving claim.

Actual same-CPU runner-a Java comparison: **4/4 repair workloads at time parity**,
all timed scores `[0,0]` equal. Whole 96 **26.238 ms Rust /27.762 ms Java**, still
149,001 logical evaluations, eight repairs, 131,511 actual callbacks. All 64 validation cases
valid, 36 equal scores, 47 native/35 Java optima under documented sampling differences.
Only repair refreshed; other 17 raw reports unchanged and relevant stale keys remain.
Dashboard measured index is 86.276%, with 1/18 reports current and 17 stale.
This is a tracked-target index, not remaining effort or an ETA.
Dashboard UI/data contract unchanged; reuse actual 296 rendering plus current HTTP.

runner-c accepted the worker then went offline; SSH exited 255 and worker outcome remains
unresolved. Preserve snapshot 1907025058c609b7804eb5b66b8e97cefcdd6d579d3e6bebc5f32011fb66bb6a,
collect original receipts if it returns, do not restart blindly. runner-a release
and Java jobs are terminal. Local broad gate covers the retained shared change.
Both remote shared Cargo targets need genuine builds before other source claims;
runner-a current repair artifact 3ccd28f7f19f897f793b4d0498b2fbb4b0f2f502b04f9294f72d89ab71bede2a.
Do not transplant binaries/cache fingerprints or restore Rust mtimes.

Goal remains incomplete. Inventory unchanged 4410/4855, 10 pending, 435 excluded;
connected-range/history and other recorded behavior gaps remain unresolved.
Next: close another relevant compatibility/performance gap using this qualified
baseline; no repeat of 301/302 microtrials or unchanged behavior audits.

## Previous implementation — 299 coalesced checked-sum replacements

SumCollector defers one logical primitive replacement. Checked chains compose;
inverses cancel. Each public operation still checks its own signed difference,
total and current membership before any change. Other successful mutations
materialize the pending frequency edit. Logical equality ignores representation;
clones retain the complete pending state and shared roots. Duplicate/zero captures,
checked overflow, Send/Sync, no-pending removal and graph rollback remain intact.
Five original regressions supplement all seven existing checked-sum regressions;
no upstream-method credit is added.

Against the actual qualified 296 artifact, original sum update/restore workloads
improve **87.65% / 91.26%** at 64/512 captures,
every four-fork comparison faster. All 96 original full validation cases and all 6
original timing inputs/settings/outputs remain. Separate public-API controls retain
full frequency reconstruction and drains: inverse and connected-chain paths
improve every fork, while alternating independent captures have **1.24% /
6.23% higher medians**, mixed forks. Unchanged set/map controls retain
2–5.5% higher pooled medians with mixed forks. Deferred work can move onto unrelated
edits or equality; the collector grows by one bounded field. No general speed,
whole-solver, memory or Java execution-work equivalence claim follows.

One final `bash scripts/check.sh`: **8,592 Rust +292 doctests**,
**95 RUN/53 REUSE**, 337.394s; build-only precompile
uses eight workers and the standard gate retains its normal four-worker environment.
Native `5c53146bd6f7704b324616f13f2bfe87772277ba4abe1941cca64f090f6cb827`. One actual Java aggregate refresh retains all 96 checks and
all 6 original workloads; **6/6 meet measured time parity**, with full checked
trial/restore/commit observations and checksums. Other 17 raw reports retain bytes,
dates and actual keys. Mixed296 becomes stale under the shared input change;
its earlier 8/8 result is historical, not refreshed here. Dashboard UI/data contract
is unchanged: reuse actual 296 desktop/mobile proof and check current HTTP report,
source/model hashes and freshness. No repeated browser suite or unchanged Rust gate.

Dashboard index **86.124%**, **1/18 current**, 17 stale, Java multiplier **1**.
This is a tracked-target index, not remaining effort or an ETA. Inventory remains
4410/4855 (90.834%), 10 pending, 435 excluded, zero failed/unverified. Remaining behavior
gaps are not waived; goal active/incomplete. The 297 profile on qualified 296 found
Arc::make_mut at 67.32% self (2,038 samples, zero lost), motivating deferred edits. The 298 cardinality prototype was rejected; its changes are
absent. Evidence: work/round297-aggregate-gap/, work/round298-qualification/;
work/round299-qualification/{integration,candidate-build-receipt,pair-results,
control-pair-results,gate-receipt,aggregate-performance-receipt,collection}.json.

Current aggregate release artifact `d641790ff61042938ef558c56f7d29a69ab501523c6e6abc0185f161d09b6659` is actually built
from the native source above. The mixed296 artifact remains historical. Next:
use this qualified baseline, close another relevant behavior/performance gap,
and reuse unchanged successful checks. No new agent was created; one existing
agent prepared the overlays. All qualification ran locally. runner-a SSH now works; its Rust 1.92 toolchain
is in `/home/developer/.cache/timetuck-build/rust-1.92.0/bin` (prepend to PATH).
No remote build/auth job is live. runner-c remains parked. The dashboard is localhost8765.

## Previous qualification — 296 checked ordinal list registry

`ListProblem::new_ordinal_indexed` offers checked ordinal lookup for Eq+Clone
values without Hash/Ord. Registry entries declare their own stable positions;
queries check bounds and equality. Invalid ordinals return a typed error. Clones
share the index; entity rebuilds preserve the exact function and recompute ranges.
Keys must be pure, stable and equality-consistent. Eq-only defaults and existing
Hash indexing remain. Changed positions use a bounded bitset/table up to 64 values
and a sorted buffer above that boundary, preserving removals and registry order.
The mixed example's explicit `--list-registry-policy ordinal` applies to both
backends and cold reconstruction; Basic registries remain Eq-only. Complete rows,
shadows, full match payloads/scores, pins, search work and budgets remain intact.

Nine original Rust regressions cover non-Hash/arbitrary-order registries, errors,
range/pin atomicity, rebuilds, both storage paths, full source/match/shadow rollback,
real search and pinned cold handoffs. Three Python contracts retain complete
same-graph Eq-only Dense/Stream controls and compare physical calls and unknown
fields. Original graph controls remain. No upstream-method credit is added.

All 16 original workload/profile rows retain four rotating CPU2 forks. Larger
warm Stream improves **11.35%** with component refinement and **10.81%** with
joint search against same-source Eq-only, all four forks in each. Small component
warm Stream is **13.40% slower**; joint larger cold is **3.52% slower**, with mixed
forks. Component larger cold improves 1.02%, also mixed. Dense results vary. Every
full output and actual physical call count matches except time/registry metadata.
No general speed, callback-trace or memory claim follows. Initial tree and sorted
buffer versions were rejected; the latter's actual profile found sorting at 6.82%
self and position discovery at 14.74% inclusive (3,358 samples, zero lost), motivating
the retained bitset. That profile is not of the final bitset artifact.

Final `bash scripts/check.sh`: **8,587 Rust +292 doctests**,
**96 RUN/52 REUSE**, 329.803s; prior build-only precompile used eight
workers with normal four-worker verification environment. Native `5342c37ce8eaf6b7449b4dda42bfc8512110e94b3aaf7685302de07d302fe48c`.
One actual Java mixed refresh: **8/8** at measured time parity, equal
scores in all eight. Larger Stream warm **15.196/16.751 ms Rust/Java**;
cold **14.990/19.594 ms**. Full Eq-only and prior graph observations/samples remain.
Eq-only controls also meet Java parity in this run; cross-run Java ratio changes
do not isolate the native gain. All four fresh Dense medians and both small Stream
medians are slower under ordinals; their controls remain visible. The larger
reference is a feasible certificate, not a proved optimum.
Private and final comparisons use the same actual native artifact; the final
Python-driver input key is recorded separately. Other 17 raw reports retain bytes,
dates and keys; changed relevant inputs remain stale. Native-reference passes and
Java errors never become Java agreement credit.

Dashboard index **85.914%**, **1/18 current**, 17 stale, Java multiplier **1**.
This index measures tracked targets, not remaining effort or an ETA. Inventory
stays 4410/4855, 10 pending, 435 excluded, zero failed/unverified. Actual final
browser evidence covers desktop/mobile and the complete older presentations.
Goal incomplete; previously recorded compatibility gaps remain.
Evidence: work/round296-qualification/{integration,candidate-build-receipt,
final-build-receipt,pair-results,prebuild-receipt,gate-receipt,
mixed-performance-receipt,collection,browser-proof}.json; tree-coalescing-trial/,
sorted-buffer-trial/ and work/parallel-round296-evidence.json.

Next: close remaining relevant compatibility and performance gaps. Preserve all
three graph declarations, both list registry choices and every complete control.
Do not repeat the rejected tree/sorted-buffer trials or resolved behavior audits.
Reuse successful unchanged verification; refresh only deliberately chosen reports.
No new upstream assertions passed in this increment. Initial scope and method
counts remain unchanged; private JVM arrays/history differences are not waived.

The target release example is now the genuinely built final bitset artifact
`357decf5df3bfaac9923c50113c79a3fc4f331aa3214f9857713834640965f23`, native `5342c37ce8eaf6b7449b4dda42bfc8512110e94b3aaf7685302de07d302fe48c`.
The old qualified294 control remains outside target at
`work/round295-qualification/qualified-v2-binary`. The sorted-buffer profile binds
its own rejected source/artifact in `sorted-buffer-trial/`; do not call it a current
profile. Never transplant artifacts or rewrite old source keys. Source restoration
must write bytes or advance mtimes, not preserve old Rust mtimes with copy2.

All builds, focused checks, final verification, native timings, fresh Java and
browser checks in296 ran locally. runner-c remains parked: no probes/wake/auth requests.
runner-a's last bounded connection needed authentication and expired; no remote job
remains live. One existing agent prepared the initial overlay and later performed
a bounded read-only review; no new agent was created. All qualification processes are terminal. The dashboard is localhost8765.

## Remaining limits and retained evidence

- Ten pending upstream methods are eight JVM/loading/framework seams and two
  private k-opt descriptor-array assertions, not ten missing native algorithms.
  Original methods/denominators stay visible; no reclassification for credit.
- Connected-range agreement remains 66/197 under the explicit wrapping metric;
  native/reference checks pass all 197. The 131 differences include Java history
  bugs. `compat/observations/connected-range-history.json` retains the contradictory
  histories; the user policy question remains unanswered. Do not infer a waiver.
- List metadata retains 425/514 agreements: 16 differences, 40 Java errors and
  33 unsupported empty cases. Prior public probes found no justified native fix;
  retain their complete observations rather than repeating the same audit.
- Cyclic-supplier checks retain 107/128 agreements and 21 actual Java errors.
  Native-reference passes never count Java failures as successful agreement.
- Great Deluge292 retains its original evidence (see live freshness): explicit binary64 276/276, 25/25 prefixes
  and 184/184 additional probes; exact default stays 275/276.
  Encounter-order ranking has 4824/4824 agreements; total-order controls retain
  4499 agreements/325 differences. Load-balance binary64/wrapping policy has
  101/101 last-observed agreements, exact-root control 100 and default 97; it is
  stale now. Do not repeat unchanged successful investigations for a new turn.
- Mixed296 is now stale; its last observation is **8/8** at measured Java parity and equal quality in all eight. Larger Stream warm 15.196/16.751 ms Rust/Java, cold 14.990/19.594 ms. Full same-graph Eq-only Dense/Stream and ordinary/v1 graph controls remain. No general/equal-work/memory claim.
- Source restoration must write bytes or advance mtime; never copy old Rust
  source mtimes or target fingerprints/binaries into a supposed new build.
  Verify actual compiler artifacts before performance comparisons.


Use docs/completion.md, docs/compatibility.md and native contracts for current scope,
requirements and original evidence. Goal remains active and incomplete.
