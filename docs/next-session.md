# TimeTuck implementation handoff

## Objective and boundaries

Build an independent Rust constraint optimization solver using pinned Timefold
Community tests/public APIs as specifications, with a visual compatibility and
performance dashboard. The original native acceptance requirements are demonstrated in 305. Preserve
those results and the original strict compatibility boundaries. Never copy or translate
upstream production; Community only, no formal clean-room claim. Upstream pin
remains v2.7.0, `8b815bc6b5f3eb81feedc94dea0885a93b267762`.

Record complete adapted tests/fixtures in `compat/provenance.json`. Partial,
original, skipped or empty tests earn no upstream-method credit. Crates remain
unpublished. Conventional Commits, never amend/rebase/push. The current user direction is to finish integrating completed work, then close
compatibility gaps so the pending backlog shrinks. Keep performance important,
commit occasionally and reuse unchanged verification. Do not create new subagents. Distribute builds/tests across available machines. Use local and runner-a; runner-c remains parked
after its actual SSH timeout. Preserve live runs across observation timeouts.

## Active RealWorld throughput goal — 2026-10-21

Reach Java streams evaluations/s on both frozen public RealWorld workloads.
Current original matrix proves 1/2 targets: Golfer meets parity; Sailing is below.
Preserve original same-machine serial
rotating Java/Rust pairs, seeds 7/11/23 x10s, CPU 2,3 and all 12 independent
validations. Scores/feasibility remain separate. Model, fixtures, declarations,
runner, compiler, CPU, PGO, panic and batch settings stay unchanged. Local + runner-a;
runner-c parked. No new subagents. A commit/context reset warrants no repeat checks.
Latest user testing policy: run only directly relevant tests and compile checks;
no full suites unless explicitly requested. The completed suite below predates
that update and must not be repeated on commit or integration.

Selected batch: direct flat-map output. Whole ownership-consumed exact numeric
nodes collect the original erased Events from standard tail/row iterators,
eliminating the intermediate Vec<TupleChange>. Original mapper/iterator
collection and length-cache preparation remain. Full owned Event erasure finishes
before cache publication; zero-length presence, full IDs/fields, output order,
callbacks/work, capture and rollback remain. Public/custom/retaining paths keep
ordinary staging. No unsafe, new production vtable entry or model change.

Native `aee829a5a1faaf0d6c5918a69f742c4dd4a33ac37e8953b935a36309816f6d27`;
production `59be6ab0e5ce028af5b31d9f31d7e855b3f519ba2a2fce4ce17d4f6a2e69d9f3`;
snapshot `a32b7ed7a7a64f7c0ae9e61356a90aaa21702825bb47884b74662b10bd5c89e9`;
artifact `6d29717bc078561672f138fa1dc102b685c037ceeb1c634c02bf5d8468d005ab`.
Actual 15 flat_map +2 tally +4 external application tests and lib/example Clippy
pass. Three original native regressions add no upstream method credit. Current
log credits 21 named passes from those actual source-bound logs, assembled without
rerunning tests. **No full suite ran.** Previous ec026 source's 229 focused tests
are preserved unchanged, not relabeled.

Four rotating3s native pairs per case measured median +1.569% Sailing/+1.127%
Golfer, but only 2/4 pairs faster in each: modest mixed signal, not a universal
gain. All16 outputs independently valid. Original default same-machine serial
rotating3seeds10s CPU2,3 Java/Rust qualification also completed, all12 outputs
independently valid: Golfer185780.693/121549.795 = **1.528433x**;
Sailing63370.579/74417.313 = **.851557x**. One of two targets met; Sailing needs
17.432% additional Rust throughput on this measured baseline. Rust absolute rate
is nearly unchanged versus previous matrix (+.250% Sailing/-.445% Golfer); the
larger Sailing ratio change is mostly lower Java throughput, not an attributable
optimization gain. Scores/feasibility stay separate (Sailing Rust2/3,Java0/3;
Golfer3/3 each), no equal-score gate. Same original configuration/toolchain/Java
classes and dependencies, actual Cargo release/javac, byte-identical measured
candidate artifact. Genuine report5725a1 is current; previous16b0 bytes and focused
logs preserved in proof. No unchanged checks or qualifier repeat for commit/reset.
Proof: `compat/observations/real_world-direct-flatmap-output.json` and
`work/real_world-goal/standard-flatmap-current-java-qualification/`.

The custom OutputRows iterator variant is closed unselected:17 relevant tests,
16 valid controls, Sailing+.507%/Golfer-.881%; no application/Clippy/Java on it.
A malformed intermediate standard-iterator scaffold failed closure-arity compile;
original failed source/log/receipt preserved, corrected source separately frozen
and verified. All current worker handles are terminal and collected.

Prior direct group publication ec026/218/874a remains integrated as the preceding
runtime change. Its historical controls were +2.536% Sailing/+2.315% Golfer,
225 graph+4 app tests/Clippy and actual originalJava16b0 (G1.528353,S.783320).
Those results remain source-bound historical evidence; gains are not additive.
[Publication evidence](../compat/observations/real_world-direct-group-publication.json)
and [historical qualification](../compat/observations/real_world-direct-group-java-qualification.json).

Closed owner-cache trial: bounded per-side individual Rc reuse requires strong
and Weak uniqueness, preserving original Event/queue/carrier widths. Actual
226 graph tests and 16 outputs passed, but Sailing+.259%/Golfer-.061% was flat;
not integrated, no additional Clippy/application/full-suite/Java checks. Proof:
`compat/observations/real_world-member-owner-pool-investigation.json`.
Retained slot-reuse tradeoff: use the matching fixed-index bitposition directly
instead of remapping its ID. 30 relevant join tests/16 valid outputs; Sailing
+1.484% every 4 pairs, Golfer-1.378% every 4. Not integrated. This differs from old
static mapper specialization. Immutable source/proof is in
`compat/observations/real_world-indexed-join-row-visit-investigation.json`.
The actual combined direct-group + indexed-row source208da/f5b05 was then measured
against selectedec026: Sailing+.484%/Golfer-.318% effectively flat, 37 relevant
join/publisher tests and 16 valid outputs. Not integrated. Close this combination
and the unchanged standalone indexed candidate; never add component gains or
repeat these timings. Proof:
`compat/observations/real_world-group-publication-indexed-row-combination.json`.
The contiguous member-bucket hypothesis was implemented and measured, then closed
without integration. Exact Pair IDs/Quad rows/Pair keys use sorted full-row vectors
per key and an ID registry, with scalar nested undo/captures. All 7 relevant node,
consumed-graph and external application tests pass; all 16 native outputs validate.
Sailing +0.885% and Golfer -0.085% were mixed (2/4 faster each), insufficient to
justify the extra storage. Original Clippy flagged only type complexity; a
separate alias-only repair passed lib/example Clippy and was not timed or tested.
Neither variant earns main test or Java-parity credit. All handles terminal and
collected. No full suite or Java qualification on the rejected backend. Preserve
both prototypes and receipts; do not repeat unchanged variants.
Proof: `compat/observations/real_world-contiguous-member-buckets.json`.

The next current-source profile is `work/real_world-goal/selected-flatmap-sailing-profile/`:
one 10s seed7 CPU2,3 cycle sample of selected6d297, 994 samples, zero lost and a
locally independently valid full output. It is diagnostic only. Contribution
updates/restore, group edits and scalar propagation remain substantial.

A distinct numeric undo-packing candidate was then measured and rejected:
collector inverses 48 to40 bytes by packing their operation into alignment bits
of an identity-only integer, and contribution undo positions minus32 bytes with
full-width fallback/rebasing. Every preimage, full ID/score and logical edit
remains. Actual 29 directly relevant tests and 16 output validations pass, but
Sailing -.401%/Golfer -.520%, all4 pairs slower in each. No integration,
application/Clippy/full-suite/Java run. Initial5-test diagnostic passed but static
review caught missing side-vector cleanup before spare reuse; separate corrected
source adds cleanup/regression and is the only measured version. Preserve both.
Proof: `compat/observations/real_world-compact-numeric-undo.json`.

Next candidate in progress: consecutive exact numeric filter inputs share one
dispatch while retaining each predicate, cache update and guard, in FIFO order.
Only private output routing is staged to the end of that consecutive run; no
downstream callback could run between these already queued inputs. Public,
retaining, activity and wide fallback remain scalar. No fixture/model/settings
change. Prototype `work/real_world-goal/filter-run-flatmap-next-prototype/`;
actual focused tests and native controls are still active. Do not assume a gain
or integrate before collecting the original handles in the state ledger.

Do not reopen closed static mapped reads, concrete-key callbacks, member-owner
pools, buffer handoffs, indexed-row variants, duplicated contiguous-bucket
registry or undo-packing source unchanged.

Existing v29 helpers bind actual selected aee829/59be6/6d297, its 21 focused tests,
Clippy and original Java report5725. They built/measured the rejected contiguous
member candidate. Historical v28 remains unchanged for ec026. No full suite,
unchanged qualifier, repeated browser check or runner-c probe. Goal active/incomplete.

The following deferred-totals baseline is historical. Its runtime,
focused evidence and native observations remain preserved for their own source.

Previous batch: drain-local deferred terminal totals. Only whole ownership-
consuming graphs with existing builtin-score metadata enter. Each terminal's
starting total is captured on its first event; its final total is read after the
queue succeeds. Every callback/event/guard/counter and terminal arithmetic check
still executes in the original order. Final graph-wide arithmetic retains its
original end-of-drain phase. Public/custom-score/probe-None fallback code remains.
A reusable seen vector and old-total list add workspace memory; clear on success/
error, local-owner drop on panic. No persistent score cache or event batching.

Native `ab916410936c1e10c29225a58facc7a587550c2e54392dc7254dbe5f86cf1d16`;
production `1b2a1184fec54e86f7e49823c4ac95442aa069f67ee220817d9387792a31ada5`;
snapshot `50fc4d70dd2867ecd52666eefda76a4832c7256024205ca4765ddfaecf36a2bc`;
artifact `0ea2876347e63e8d9289536cbae44201c13b228772a365747e0a8032709c654e`.
Actual four rotating 3s seed 7 native pairs per case on runner-a CPU2,3 measured
**+1.147% Sailing /+1.241% Golfer**, three of four pairs faster in each case;
all 16 assignments independently valid. Modest workload-specific signal, no Java
parity credit or additive claim. Proof:
`compat/observations/real_world-deferred-terminal-totals.json` and
`work/real_world-goal/deferred-terminal-totals-05a-fixed-results/`.

**Current verification is focused: 224 Cargo graph tests and lib/RealWorld
example Clippy pass. No full suite ran.** Two new original regressions preserve
callbacks/work/scores, full/current/nested captures, callback panic, structural/
terminal/final-total overflow, checked/wrapping/canceling totals, rollback/retry.
An initial cfg-only test borrow error was fixed before the genuine selected
build/pilot. Original failed source/receipt preserved. Source is exactly frozen.
Concurrent nocapture diagnostics interleave seven status lines: the original
Cargo summary proves 224 passed, while the unchanged named-log reader recognizes
217, including both new regressions. The raw log is not repaired or synthesized.
Only those 217 named passes get current report credit. Full native05a618 evidence
is preserved as `previous-full-rust-tests.log[.json]` in the proof, not relabeled.
Declared 4,412 upstream adaptations,4 external cases and previous native controls
remain; this focused run earns no current upstream-method verification, no new
upstream adaptations.0pending/443excluded unchanged. Dashboard0 current upstream
passes means4412 unverified for this source, **not new failures or a reopened
implementation backlog**. Latest user forbids full suites; do not rerun for UI
percentages. Preserve focused versus historical verification honestly.

The previous Java report866338 remains byte-identical and stale: source67f/378/
7f439 measured Golfer1.467427x and Sailing.730242x, all 12 outputs valid, previous
1/2 targets met. **Neither current target is proved on this new source.** Keep
scores/feasibility separate, no equal-score gate. Defer another original qualifier
until materially closer to Sailing parity; do not extrapolate native gains.

Future native helpers: `pilot-v26-artifact-{dispatch,remote}.py` and
`pilot-v26-master-dispatch.py`. Their actual baseline is current ab916/1b2a/0ea287
release +16 validated native controls, with explicit local focused-only test/
Clippy receipts; no full-gate claim. Helpers actually built and measured the closed
contiguous-summary and all three member-buffer variants below. v25 stays historical
for05a618. All worker handles terminal and collected. Existing SSH Master socket
`work/real_world-goal/ssh-control/runner-a` was reused; local+runner-a, runner-c parked,
no new agents. Commit/reset alone warrants no repeated checks.

Closed this round: inline directory rows (private4 page columns in prefix row,
higher columns keep originalVec) passed 48 relevant primitive-row tests but were
slower: Sailing-.960% /Golfer-1.885%, all 16 valid. No runtime integration, full
suite or Clippy repair of prototype-only visibility warning. Saved beeb/498
hardware samples were decoded to instruction-level sites; no new profile run.
Proof: `compat/observations/real_world-inline-directory-row-investigation.json`.

Further closed experiments: contiguous full summary arena was flat on Sailing
(-.047%) and modest on Golfer (+.608%); 224 graph tests/16 valid outputs. Whole
member-buffer handoff avoided per-row Rc and full-row arena copies, but EventView
measured -.642%S/-2.342%G; the smaller single-holder version was flatS/-1.342%G.
The final direct receiver preserves original Event/input signatures, certifies
transitive receivers via existing summary metadata and isolates member handling;
it measured -1.037%S/-5.498%G. Each passed 226 graph tests/16 valid outputs; direct
also passed 4 RealWorld application regressions. First EventView passed lib/example
Clippy; it is not evidence for other sources. Initial scaffold/test import failures
are preserved. All three are rejected, not integrated; no full suite or new Java
comparison. Original Java866338 is byte-identical/stale. Proof:
`compat/observations/real_world-contiguous-summary-arena-investigation.json` and
`compat/observations/real_world-member-buffer-handoff-investigation.json`.
Do not retry these buffer/representation/dispatch variants unchanged. Main still
ab916 with its original 224/217 focused evidence; candidate passes earn no main
credit. All local/runner-a sessions terminal/collected; authenticated master works.

The alternative deferred node/old/new single-buffer cells passed 224 graph tests
and 16 valid native controls but were flat: Sailing+.269% /Golfer-.197%. This was
measured as a whole source versus05a618, not incremental to the selected version.
It is not integrated; no repeated unchanged measurements or added component gains.
The selected deferred totals are distinct from earlier adjacent receiver caches
and borrowed-post-total ABIs, which stayed closed.

Historical selected filter forwarding remains in
`compat/observations/real_world-shared-filter-forwarding.json`: +3.083%S/Gflat,
8881 named/8887Cargo/292doc/54tool at05a618. Its complete native gate and selected
runtime remain preserved. That older gate predates the latest no-full-suite
user policy and is not current verification of the new source. Prior typed join
publication, compact inverses and other accepted work remain unchanged.

Concrete join-key specialization is CLOSED (-1.392%S/-1.705%G,16valid,58 relevant
tests) in `compat/observations/real_world-concrete-join-key-investigation.json`.
Do not restore it from older handoffs. Six67f trials stay closed in
`compat/observations/real_world-performance-investigation-67f.json` (accounting,
touch marks, checkpoint fast, static join reads, both prepared-score designs).
No unchanged repeats of ownership/layout/queue/counts/preimage candidates.

The previous-source hardware profile at
`work/real_world-goal/current-compact-inverse-hardware-profile/` is actual beeb/498
on runner-a: two valid 10s seed 7 CPU2,3 runs, zero lost samples, no rebuild; 2.24 IPC,
.73% branch misses/3.42% LLC misses at 100% counter residency. It is not a profile
of the new source; enclosing samples are not removable costs. Reuse it to avoid
closed ownership/layout/queue/prepared-score experiments.
Backend HTTP/data/freshness checked; unchanged UI rendering proof reused.
Commit/reset alone warrants no repeated checks.

## Previous verified group-edit inlining — 2026-10-21

Selected batch: two private compiler-inline attributes. The unchanged primitive
group-entry helper and unchanged scalar inverse-edit callee are both requested
inline, exposing concrete edit variants and summary-return work to the caller
optimizer. Every body, canonical owner, callback/error/overflow phase, emission,
capture, nesting and rollback remains. No new test or upstream credit. Prior wide
page directories, membership pages, bit inverses and score hashing remain.

Native `0149456570a1866a3f10f64e744d668bc07897e603a53ea5d5d2cfcbd1ba76e2`;
production `ff8cd55e7fba18690680d9560eb45e53778c7dcfb31414ccb329cc7bceb6d846`;
final snapshot `6ffbf83fb9a124c28af4c114a947e232ef9a5e29cce7121b4fb806fc7e921e6a`;
measured/final artifact `78ea0deac0536545c518faa1dea6b21ac3d2db9dec9935662397071abd35d9e2`.
Actual Clippy, all-target no-run prebuild and one normal jobs 4
`bash scripts/check.sh`: **8,874 named /8,880 Cargo executions /292 doctests
/54 tooling jobs**. Only genuine aggregate Rust log/metadata promoted; no job-cache
transplants. Upstream: 4,412 passed /0 pending /443 excluded; external: 4;
native regressions: 4,458. Observation:
`compat/observations/real_world-group-edit-inlining.json`. Proof:
`work/real_world-goal/group-edit-003188-next-final-results/`.
Backend report/freshness checked; unchanged rendering proof reused.

Actual native controls against fully verified1972/003188/1aa4/4b3f:
**+2.672% Sailing /+1.376% Golfer**, every paired trial faster, all 16 independently
valid rotating 3s seed 7 outputs. Exact same frozen source and actual release
artifact used for controls/final verification. Caller-only inline was separately
modest (+.744% S /+1.012% G), not additive. Inlining may increase code/compile cost;
no universal speed or memory claim. No repeated timing or Java qualifier.

Inline fanout packets, prepared singleton rebirth, equal-group summary views,
compact owned score undo, adjacent summary total caching and compact membership
undo were discarded after flat/regressive native controls. Focused controls and
failures stay recorded; no broad gates were spent on those candidates. Do not
retry unchanged. Trial ledger:
`work/real_world-goal/active-performance-state.json`.

Original Java reportb14a remains unchanged/stale: historical G1.253663/S.609042,
all 12 valid, Sailing feasible Rust 2/3 Java 0/3 on old021a. Defer the qualifier until
materially closer to Sailing parity. No current Java target credit or multiplying
native gains into historical ratios. The goal is incomplete.

The previous014945/ff8cd5/78ea0d release artifact has one local Sailing10s
seed 7 CPU2,3 profile at `work/real_world-goal/current-group-edit-profile/`.
The output independently validates (0/94400), zero samples were lost, and the
actual gated artifact was reused without a build. This recording replaces the
old003188 wide-directory profile for current diagnosis. Reuse it. The current
native gate and report still apply; no new runtime source was selected below.

## Latest performance investigation — 2026-10-21

Seven additional native trials were measured and discarded; do not retry them
unchanged. Borrowed terminal totals were flat for Sailing; paired and consecutive
terminal dispatch were slower. Singleton retirement was mixed (+1.541%S/-1.232%G),
and the removal-only version was flat for Sailing. A full member-row arena was
flat for Sailing and slower for Golfer; selective queue/boxed workspace routing
was slower on both and failed an unwind/cleanup check. Its producer also needs
lazy arena allocation after an unwound workspace before any future use. None is
integrated, and no full native gate was spent on these rejected candidates.

The original member arena passed219 focused graph tests after a cfg-only Vec API
correction. The selective variant passed2/failed1 and is not qualified. Original
Run-name and collector capture-API compile failures remain recorded; corrected
run dispatch218 and retirement103 focused controls passed with the same respective
production bytes. No failed test, candidate test or diagnostic earns integrated
native/upstream credit. Proof and actual source/artifact keys:
`compat/observations/real_world-performance-investigation-014945.json`.

An instrumented seed 7 x3s incidence diagnostic found **zero equal group
projections in13,291,499 updates** across Golfer and Sailing, including initial
population; both outputs independently validate. This is occurrence evidence,
not speed or universal-input proof. Do not implement the old proposed equal-
projection shortcut for these workloads: it has no demonstrated runtime scope.
Its proof is `work/real_world-goal/group-projection-density-014945-results/`.

Current profile: propagation12.29%self, group apply4.93%+2.11%, inlined group
edit closures4.43%+1.76%+1.16%+1.11%, membership4.58%+1.31%, enqueue4.23%,
terminal apply3.58%, join apply3.32%; enclosing shares are not removable costs.
The compact slot-aware inverse lead has now been implemented and measured below;
it is flat and unselected. Do not treat it as unimplemented or retry it unchanged.
Avoid all closed ownership/layout/queue retries.

Use `pilot-v21-artifact-{dispatch,remote}.py` for actual native comparisons against
current014945/ff8cd5/78ea0d and full gate80a1fa4. No Java qualifier was refreshed.
No new agents; local+runner-a, runner-c parked. All investigation jobs are terminal.
The goal remains incomplete. A commit/reset warrants no repeat checks. For an
identical frozen pilot and final Rust source, reuse the genuine release build
receipt directly; a second release build is needed only for changed relevant
source or an unresolved binding concern. Never relabel old measurements.

## Latest compact-edit investigation — 2026-10-21

Four further candidates were measured against the unchanged fully gated 014945
artifact on runner-a. Each used four serial rotating 3s seed 7 pairs per case on
CPU2,3, with all 16 outputs independently valid. No runtime change was selected.
These native diagnostics establish no Java parity and cannot be added to older gains.

| Candidate | Sailing | Golfer | Result |
| --- | ---: | ---: | --- |
| Slot-aware collector inverse tokens, original record size | -0.446% | +0.126% | Flat; 105 focused group tests passed |
| Producer-packed pair of full summaries, single receiver | +0.629% | -0.257% | Modest signal only; 218 graph tests passed |
| Separate event drain from root setup, one compiler hint | -0.425% | +1.312% | Flat for Sailing; unchanged logic |
| Explicit group branches replacing Option combinators | -1.844% | -0.249% | Slower; 102 focused group tests passed |

Slot tokens reuse the position found by the original forward search, retain every
edit and normal-history boundary, and use scalar inverses after tree promotion.
The packed producer retains both complete rows and each original callback/check;
multiple receivers keep original FIFO delivery. It differs from earlier consumer
pair/run dispatch by removing one actual queue cell, push and pop. Its small
Sailing gain does not justify standalone integration. The two compiler candidates
retain the original algorithms. None changes fixtures, model, declarations,
settings, compiler, target CPU, PGO, panic or batch policy. No unchanged retries.

Proof: `compat/observations/real_world-compact-edit-investigation.json` and the
source-bound raw receipts linked there. There are 64 valid outputs across these
four diagnostics, but no new integrated test or upstream-method credit. The
existing 8,874 named/8,880 Cargo/292 doctest/54 tooling native gate remains current; it was
not repeated. The original Java report remains unchanged/stale. All local and
remote jobs are terminal, runner-c parked, no new agents. Goal active/incomplete.
Reuse the current artifact/profile; audit previous candidates before another
runtime mechanism. This investigation produced no selected throughput gain.

## Previous verified typed-output source — 2026-10-21

Reach Java streams evaluations/s on both frozen public RealWorld workloads.
Neither current target is proved after this source change. The previous qualified
source met Golfer parity and remained below Sailing parity. Preserve model,
fixtures, app declarations, runner, settings, seeds7/11/23 x10s and CPU2,3 pairing.
Scores/feasibility stay separate; no equal-score gate or compiler/CPU/PGO/panic/
batch-policy changes. Local + runner-a; runner-c remains parked. No new subagents.

Selected typed-output storage batch: twelve Rust files, eight original native
regressions. Exact consuming Pair/Quad Group summaries stage both original outputs
at the existing common apply finish callback, after input edits, output preparation
and poison reset, before remaining local drops. They publish after common apply
returns into stable 64-row boxed typed chunks; original full fields, key enrichment
and old/new FIFO remain. Whole outgoing cohorts must be nonempty exact nonretaining
terminals with left edges and no shared producer, certified after consuming-storage
activation. Terminal receivers share the original apply body and score/guard order.
Exact builtin fixed usize-ID Triple/Triple joins return a lifetime-bound carrier
over their completed typed Vec, draining into original events after guard release.
Successful bounded scratch capacity is reused; unwind releases it. Public/custom/
retaining/batch/unsupported paths retain original representations and lifetimes.
Event remains72B, queued24B and edge/queued40B. Group rows remain88/120B; private
peak chunk capacity and old NoDrop rows persist until overwrite. No memory claim.

Native `076517ff1ae9cd82fe187a2e02dcb7a997e7221998b5ef5b9b33c98b19c1726e`;
production `b280f04d7d4fea2ec6455ea071c17bc27f7755acdbf4126f09e438dd7bee95b2`;
final snapshot `22cdd2d7f3437cdc55aa65d6091aad10ec1d9161e11f0ec2a96c95240aeb13b5`;
measured artifact `570309bde160f41f2f10eed336e65f2de31a3ca24f426fc12184d870a1f992ee`.
Actual unchanged-source Clippy, all-target no-run prebuild and normal jobs4
`bash scripts/check.sh` pass: **8,866 named native /8,872 Cargo executions
/292 doctests /54 tooling jobs**. Eight new tests cover full-range metadata,
stable chunk growth/reuse, complete staging/FIFO/fanout, abandoned carriers,
real graph callbacks/owners, full/current/nested rollback, errors/panic retries,
input-borrow independence, shared-owner promotion and fallback boundaries.
Only genuine aggregate Rust log/metadata promoted, no per-job cache transplants.
Observation: `compat/observations/real_world-typed-output-storage.json`.
Proof: `work/real_world-goal/summary-carriers-2ed-next-final-results/`. Backend HTTP report/freshness checked; unchanged
UI proof reused. Upstream4412passes/0pending/443excluded and external4 unchanged.

Actual combined native controls against qualifieda7cbb: **+3.081%G/+4.076%S**,
four rotating 3s seed 7 controls per case, all 16 independently valid. Individual
Group-only +2.922%G/+2.446%S and Join-only +.772%G/+.795%S are historical,
not additive. Original combined Clippy failed ptr_arg/test type complexity; a
new frozen source uses a lifetime-bound dyn Any constructor with original Vec
storage and test aliases. No assertion weakened. The actual corrected source
was measured and fully gated; original failure remains recorded.

Default Java qualification deferred until materially closer to Sailing parity.
Previous report SHA `b14a66d1006038f200c7c8db63f53e3aadfde0cbdb1116cd2d1f146eae8e1b0a`
is unchanged/stale: historical G1.253663/S.609042, all 12valid, Sailing feasible
Rust2/3 Java0/3. No current parity credit and no multiplication of native gains
into those ratios. Final success still requires both targets on current source
under the original Java protocol. Do not rerun full gates or qualifiers for a
commit/context reset alone. Ignored state: `work/real_world-goal/active-performance-state.json`.

The selected artifact already has one local Sailing10s seed 7 profile at
`work/real_world-goal/current-typed-output-profile/`: actual570309 binary,
independently valid0/91600, 1991 samples, zero lost. It was recorded while the
native gate ran, and source/artifact identity is joined to the later successful
gate in the observation. No rebuild or repeat profile is needed for this source.
Read-only decode: `work/real_world-goal/typed-output-profile-audit/`.
Largest sampled queue-copy cluster follows generic Node apply at vtable0x128,
not the new summary receiver at0x120. Do not infer a summary-hook result change
would remove that cost or repeat the old generic emit-sink experiment unchanged.
Propagation self12.91% is an enclosing sampled region, not a removable-cost bound.

## Previous qualified fixed-bucket source and completed trials

Reach Java streams evaluations/s on both frozen public RealWorld workloads.
Historical original-protocol proof met **1/2 targets**: Golfer met parity, Sailing remained below parity. This source is superseded by the typed-output batch above.
Preserve the model/fixtures/runner/settings, seeds7/11/23, ten-second limits and
CPU2,3 pairing. Scores/feasibility stay separate; no equal-score gate or compiler,
PGO, panic or batch-policy changes. Local + runner-a; runner-c remains parked. Only
existing authorized agents; do not spawn new agents. Preserve live handles.

Selected fixed-bucket directory batch: six Rust files, three original native
controls. Existing exact primitive graph equality joins use PrimitiveRows for
fixed outer buckets. Constructor transfers actual canonical key/value Arcs;
Members words/key Clone and callback/error/matching order stay intact. Ordered,
custom, Drop and borrowed paths remain Tree without new public bounds. Paged
journals/captures/full/current/nested installs reuse existing row undo; early Tree
full saves can install into later Paged bindings. Original input-ID Arcs remain.
Private intermediate bucket owners may live until frame end rather than AVL root
promotion. They have no public escape. Full page clones and larger per-write logs
are memory tradeoffs, not an allocation-elimination or universal speed claim.

Native `47f4f43290299a097b4ce18813e6145e952a7eaaa78fbb55e2773bae644777bf`; production `021a4cd324ecd8c8f29204bfa4de9733c8eb5fed1b16b9b6e8f6a9dee26a4b57`;
final snapshot `4816cd6ae95ab2172b070b2403a899faa54c9bbbb5a924d083394c557bac2be2`; qualified artifact `a7cbb8e0ba724e46f5bf76e28f50bbf89c6e025a9b7af6398ba23efa9f608d0b`.
Actual unchanged-source Clippy + all-target prebuild + normal jobs4
`bash scripts/check.sh` + original default Java qualifier all pass.
**8,858 named Rust identities /8,864 Cargo executions /292 doctests /54 tooling jobs**.
Genuine aggregate Rust logs/metadata and actual qualified report promoted; no
per-job cache records transplanted. Proof: `work/real_world-goal/bucket-storage-a31-next-final-results/`.
Observation: `compat/observations/real_world-fixed-bucket-directory.json`.
Dashboard backend data/freshness updated; unchanged UI proof reused.

Historical original-protocol Java medians, all 12 independently valid:

| Workload | Rust evaluations/s | Java evaluations/s | Rust/Java | Throughput target |
| --- | ---: | ---: | ---: | --- |
| Golfer | 152,426 | 121,584 | 1.254x | met |
| Sailing | 50,061 | 82,196 | 0.609x | below target |

Sailing feasibility 2/3 Rust versus0/3 Java. Raw scores stay separate.
The original protocol was deliberately refreshed after a material native gain,
not after every small increment. Future source changes make these results stale;
never relabel them as current or multiply diagnostic gains into Java ratios.

Selected original native3s controls against actual a31/0bb8: **+18.470%G/+5.587%S**,
all 16 valid. Nativepilot54bf/prod021a was followed by a CFG-test State-alias fix
only, final47f4 with identical production bytes. Corrected3 focused controls pass
91.244s; final gate and qualifier build/measure the actual final source afresh.
Actual pilot ELF a7cbb8e0...f608d0b confirms directory operations and Tree fallback;
Members word cloning and fresh value allocations remain. Compiled proof:
`work/real_world-goal/fixed-bucket-directory-a31-next-evidence/compiled-bucket-proof.json`.

Rejected, no broad gates or combination: four-word Position **+.309%G/+.249%S**,
53 focused pass after two stale test-field references were corrected; bitmap Pair
pages **−1.411%G/+.393%S**,11 focused pass. Bitmap actual size1544vs2048B, but new
certified copies on removal/page seed/full clone remain real costs; no causal
attribution of timing. Original failed test compilations remain recorded.
Do not retry unchanged or integrate their unselected tests.

Read-only audits ruled out direct owned collector rows as an old regressive
experiment; a scalar+Birth/Death lifecycle backend was subsequently measured and rejected,
as recorded below. Fixed-slot index calls and owned-input static
indexing are distinct but occupy narrow sampled regions; no prototype. Generic
sink/alternate queues/leveled emit overlap earlier failed work. State retains all
source-bound receipts and stop decisions; do not repeat these audits unchanged.

The current a7cbb local Sailing10s profile, seed 7, valid0/88300 and zero lost
samples is `work/real_world-goal/current-2ed-profile/`. It samples the actual
qualified binary without rebuilding and earns no Java timing credit. The older
0bb8 profile remains historical. Ignored state:
`work/real_world-goal/active-performance-state.json`, round_2ed.

Four separate native trials against the actual qualified a7cbb baseline each
completed sixteen independently valid rotating controls. Scalar inverse folding
measured **−0.934%G/−2.378%S**; Membership first-word preimages
**−0.245%G/−1.444%S**; owned scalar/Birth/Death collector lifecycle
**+2.012%G/−0.079%S**. All three are unselected; their focused checks passed,
but no source/tests were integrated and no broad gate is needed. Do not repeat.
The borrowed fixed-Join output carrier measured **+0.772%G/+0.795%S** and passed
four focused controls; retain it only as a modest candidate for a larger measured
batch, with no integration or Java credit. Proof directories use the prefixes
`group-inverse-fold-2ed-next`, `membership-preimage-2ed-next`,
`group-owned-lifecycle-2ed-next` and `join-output-carrier-2ed-next` under the goal
work directory. Each keeps its own frozen source, actual builds and raw outputs.

The wide Group summary and borrowed Join carrier candidates were subsequently
combined, measured afresh, and selected as the typed-output batch above. Individual
measurements remain historical; no separate gain is added to the combined result.

## Additional RealWorld application coverage — 2026-10-20

The public golfer32x8x10 and sailing115-person cases are now retained under
`tests/application/real_world/` with original approved manifests and raw hashes.
`compat/additional-tests.json` records application origin separately from Timefold
adaptations. The upstream provenance ledger and 4412 passed/0 pending/443 excluded
method inventory remain unchanged. Four native application regressions check
full independent score, trial/rollback, seeded bounded solving and fresh graph
consistency. Additional tests earn zero upstream-method credit.

The shared example `real_world_benchmark` and independent Python scorer/actual
Community Java model support a separate benchmark job, `real_world-performance`.
Do not add it to the original18 comparison job/109-workload progress universe.
The dashboard now displays the three test-origin categories and additional
application test/fixture results and throughput gaps separately. Runtime limits,
actual scores and source/fixture provenance remain visible; a complete benchmark
protocol does not establish throughput parity or feasible sailing solutions.

Model/runner details are in `docs/real_world-model.md` and
`benchmarks/real_world/README.md`. Use the job-specific application digest and
retain original comparisons' actual timestamps/keys. No shared solver production
code changed for this integration. Fresh runner-a baseline: all18 native/Java
streams/Java incremental outputs independently validate; Rust/Java streams
throughput0.129x golfer and0.063x sailing, both targets unmet. Sailing feasibility
is1/3 Rust,0/3 both Java modes; quality is explicitly not equal-gated. Baseline
receipt: `compat/observations/real_world-application-baseline.json`; raw evidence
under `work/real_world-integration/` and `work/real_world-performance.json`.

## K-opt native test adaptations — 2026-10-20

The final two source methods now have native adaptations in
`crates/solver/tests/sequential_kopt_cases.rs`, with the original TEST-derived
fixture in `tests/fixtures/nonsequential_kopt_fixture.rs`. Preserve original
owner roles, min2/max6/distribution/draw2, selected-value/direction/repair order,
k6, every removed endpoint and every active partner connection. Execute,
validate and undo each actual proposal. The raw17-entry original expectation is
retained, but Java sentinel/four out-of-domain storage slots translate to native
active12-port matching; no raw getter-array ABI claim. A separate native control
proves sensitivity to active matching, invariance to spare storage and invalid
active-index rejection. No production Rust changed.

Inventory: **4412/4855 passed, 0 pending, 443 excluded, zero failed/unverified**.
Strict index **95.809%**, behavior **79.047%**. All18 measurement keys and raw
reports are unchanged; reuse the existing runtime/performance qualification.
The final standard gate uses selective job reuse. Current native requirements
also require both new named source adaptations to pass in the full gate.
Dashboard UI bytes are unchanged, so reuse its actual desktop/mobile proof and
check the regenerated report/live freshness only.

## Scope classification correction — 2026-10-20

Eight JVM/reflection/class-loader/Maven methods left pending by the old module-only
exclusion rules now have exact method-level exclusions and individual reasons.
Current inventory: **4410/4855 passed, 2 pending, 443 excluded, zero failed or
unverified**. The two exact private k-opt array assertions remain pending; native
physical behavior has separate passing proof. No methods were converted to passes.
Strict index **95.801%**, behavior **79.047%**; accepted Java differences and errors
retain their actual counts. Native source and all 18 comparison job inputs are
unchanged, so the successful Rust gate and measurements are reused. The current
dashboard browser receipt is `compat/observations/scope-classification-dashboard.json`;
305 qualification and its receipts below remain historical.

## Historical state — 305 native acceptance verified

The original native acceptance targets are demonstrated: **7/7 capability
requirements, 109/109 timed workloads at measured Java time/quality parity,
and 18/18 current comparison reports**. Native readiness is separate from the
unchanged strict comparison calculation, now **95.720%**. Its pending methods,
Java differences, errors and unsupported observations retain their original counts.

Release builds use Thin LTO and one codegen unit; dev/test profiles are unchanged.
A six-fork runner-c comparison retains all 16 original mixed workload/profile rows,
full outputs and actual callback counts. Larger component Stream improves 7.97%
warm /8.82% cold against the qualified 304 artifact. Every selected row's pooled
median improves; compared with thin-only, joint larger cold Dense is 0.91% slower
with mixed forks. Final genuine runner-a builds have their own source/artifact
receipts. Fresh warm mixed Stream is **12.140 ms Rust /12.701 ms Java**, both
`[0,-338]`. These bounded measurements imply neither universal speed nor identical
Java execution work. The small arena-inlining trial was rejected and is absent.

Final source `9fd3d27a1e1654d7277a8728e9c1a8df0501edb163e15adcdf6fb63dbd4e9249` passes one
`bash scripts/check.sh`: **8,600 Rust tests and 292 doctests**, formatting, Clippy
and tooling checks; 461.148s after 99.033s build-only precompile. All 14 performance
families ran on runner-a with both runtimes there; all 4 complete differential
jobs ran on runner-c. Every original configuration, workload, quality floor, control,
raw observation and failure category remains. Successful jobs were reused when
unchanged; initial runner-c environment/fixture failures and one invalid repair-settings
run remain archived with zero qualification credit.

The native model requires source-bound named passes, the exact seven original
requirements, complete current original comparisons, reviewed exact policy cohorts
and current desktop/mobile rendering evidence. It cannot convert native success
into a Java/method pass. The visual dashboard and live native policy key were
verified on desktop/mobile; source/model changes prevent a stale ready display.

See [source-bound qualification](../compat/observations/current-source-qualification.json),
[native target definitions](../compat/native-targets.json) and
[dashboard input proof](../compat/observations/native-readiness-dashboard.json) and
[confirmed readiness](../compat/observations/native-readiness-dashboard-confirmed.json).

No further native implementation blocker was established by the final scope
trace. The strict original inventory remains 4410/4855, 10 pending, 435 excluded,
zero failed/unverified. Native range policy and list capture are explicitly
user-approved; all raw counts stay visible. Ten pending JVM/private-array
assertions are not silently converted to passes.

All qualification jobs are terminal; runner-c and runner-a were reachable and used.
The original 303 runner-c tests were collected successfully without restarting them.
runner-c now has the checksum-verified portable OpenJDK 25.0.2 used for qualification.
The dashboard is http://localhost:8765. Current model-only or documentation edits
do not justify repeating the unchanged successful Rust gate or benchmarks.

Evidence is in work/round305-final/, work/round305-lto-trial/ and
work/round305-codegen-trial/. Never transplant Cargo artifacts/fingerprints or
rekey historical verification. Restore Rust bytes with fresh mtimes if needed.

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

- Ten pending upstream methods remain eight JVM/loading/framework seams and two
  exact private k-opt descriptor-array assertions. Original identities and counts
  stay visible; native physical k-opt execution does not make private arrays pass.
- Connected ranges remain 66/197 Java agreement and 197 native-reference passes.
  The user accepted consistent native behavior; history witnesses remain in
  compat/observations/connected-range-history.json. This adds no Java passes.
- List metadata remains 425/514 agreements, 16 differing sets, 40 Java errors and
  33 unsupported empty compositions. The user accepted stable native capture and
  rollback. See compat/observations/list-metadata-policy.json; no repeated audit
  or reproduction of stale-coordinate getter failures is required.
- Cyclic suppliers retain 107/128 agreements and 21 actual Java runtime errors.
  Native independent reachability/recovery proof is separate; errors earn no
  agreement credit. All current reports retain the complete unavailable cohort.
- Current 304 solver comparisons retain binary64 Great Deluge 276/276, 25/25
  prefixes and 184/184 additional probes, with exact-default 275/276. Encounter
  ranking retains 4824/4824 and total-order controls 4499 agreements/325 differences.
  Current load-balance selected policy is 101/101; exact-root control 100, default 97.
  Do not repeat successful unchanged investigations for a new turn.
- Current mixed performance is 7/8 at Java parity. Larger warm Stream is the sole
  remaining measured timing miss; use its actual artifact and original controls
  identified above. Larger optimum and identical Java search traces are not proved.
- Restore source with fresh mtimes. Never transplant Cargo fingerprints/artifacts
  or rewrite old measurement keys to claim a new build/qualification.

Use docs/completion.md, docs/compatibility.md and native contracts for scope and
original evidence. Goal remains active and incomplete.
