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
The latest original matrix proves 1/2 targets on previous source aee829:
Golfer meets parity and Sailing is below. It is historical/stale on the new
selected source; neither current-source target has been qualified yet.
Preserve original same-machine serial
rotating Java/Rust pairs, seeds 7/11/23 x10s, CPU 2,3 and all 12 independent
validations. Scores/feasibility remain separate. Model, fixtures, declarations,
runner, compiler, CPU, PGO, panic and batch settings stay unchanged. Local + runner-a;
runner-c parked. No new subagents. A commit/context reset warrants no repeat checks.
Latest user testing policy: run only directly relevant tests and compile checks;
no full suites unless explicitly requested. The completed suite below predates
that update and must not be repeated on commit or integration.

The current selected collector backend stores consumed integer values in a
reusable contiguous arena. Pages retain canonical Arc keys and slot indices;
ordered scalar undo reaches the slot directly, while structural undo retains
retired slots until the corresponding frame finishes. Cold captures retain full
positions, full snapshots copy only live values, and public/custom/shared paths
retain their prior contract. Private value addresses can move on growth and
high-water arena capacity remains allocated; snapshot copies scan slot capacity.

The longer native control measured **Sailing +1.183% / Golfer +3.641%**, all four
pairs faster on both, all16 outputs independently validated. It reused eight
completed Golfer outputs after an ordering-only diagnostic validation error and
then ran four complete Sailing pairs. The orphan Sailing baseline remains
preserved/excluded. Original3s controls and the failed helper receipt remain
unchanged. These are native gains, not an extrapolated Java comparison.

Selected native source is
`d4663b6e9a14bab1d57ddbf2446ad35bbcab8021f378f603f7adae7ce3fa40ba`,
production `465509b3c683afcb038718d70761765d2a9ab7b729978f4f6630aba879167a37`,
artifact `84e98daeb7da5f13ebad83b5c3f7248e7a43107100ac0b8e4ca7678ea875aec7`.
17 scoped tests pass across two sources:12 earlier checks and5 final-source
passes (new independent growth/retirement/full+delta regression plus4 application
cases). Actual final lib/example Clippy passes and the release is byte-identical
to the measured runtime;only a cfg(test) regression was appended. Current focused
log credits5 actual final-source passes;earlier12 remain source-bound. No full
suite or upstream-method credit. The fresh original default3seed10s Java/Rust
matrix now proves **Golfer1.603315x/Sailing.873827x**,all12 outputs independently
valid,**1of2 targets met** on selectedd466/465509/84e98d. Sailing needs
14.439% more Rust throughput on this actual comparison. Rust2/3 and Java0/3
Sailing runs are feasible;quality remains separate. Report4795a7 is current,
original5725 preserved historical. Goal remains active/incomplete.
[Evidence](../compat/observations/real_world-owned-collector-arena.json).

The preceding selected batch writes complete numeric group summaries directly
into existing stable arena slots and reuses one empty checkpoint carrier per
group node. It retains full IDs/payloads, scalar FIFO/fanout, callbacks, arithmetic
checks and nested/full/current rollback. Public/custom paths keep their contracts.
Actual four rotating native3s pairs measured **Sailing +0.599% (4/4 faster),
Golfer +0.303% (3/4)**; all16 outputs independently validated. This is a small
native improvement, not Java parity or an additive component estimate.

Selected native source is
`0b4fcc680d1bd73933ddf6ece3e73322b42991d7577f097e1df293f063ee99a2`,
production `55e146abc739fea5f4d7ef99ac09bd7866cc9607fc5586911daae41993980cf6`,
artifact `5a19112f10ab4e74c11cb23116ee7a6b5a2e80c2c78ac7b72bdeae7ef60f655d`.
18 focused cases pass across two test sources, plus4 application tests and
lib/example Clippy. The first run had17 passes and one new test fixture failure:
generic tuple literals inferred i32 coordinates. Only that test was rerun after
explicit usize annotations. Its actual final-source log credits1 pass; the
original failed-run log and successful checks remain source-bound. Actual final
release is byte-identical to the measured binary and production digest unchanged.
No repeated successful checks/timings, full suite or upstream-method credit.
Original Java5725 remains historical/stale; neither current-source target is
qualified. Goal remains active. [Evidence](../compat/observations/real_world-summary-carrier-reuse.json).

The fused collector-summary lookupaeb600/36622 is closed,not integrated:
7 focused regressions pass;16 outputs valid;Sailing+.318%(3/4faster),Golfer+.735%
(4/4). Small mixed Sailing signal,no retry/app/Clippy/full/Java. Proof in
`compat/observations/real_world-collector-summary-fusion.json`. v32 baselinebinding
succeeded. The direction-in-tag queue7c9/dfe830 is closed:16valid outputs,
Sailing-2.386%/Golfer-4.100%,all4pairs slowerboth. Corrected cfg(test) sourceb4a395
passes23 focused cases;first4de compilefailure retained,zero executed. No
integration,app/Clippy/finalreleasebridge/full/Java/retry. The packed receiver
alternative6d5a922/149f1e is also closed:23 actual focused passes,16 valid outputs,
Sailing-1.846%/Golfer-1.269%,every pair slowerboth. No further checks or integration.
Do not retry either queue representation unchanged. Proofs:
`compat/observations/real_world-directional-queue.json` and
`compat/observations/real_world-packed-receiver-queue.json`.
The consumed collector arena described above is now selected. Split address/value
arena68799/959e is closed:4 actual directly relevant tests and16 valid outputs,
Sailing-.949%(all4 slower),Golfer+.828%(all4 faster). No integration/retry/app/
Clippy/Java/fullsuite. v33 binding succeeded on its first dispatch and retains
selectedd466/465509/84e98d plus originalJava4795 (1/2targets). Proof:
`compat/observations/real_world-owned-collector-split-arena.json`.
Bounded Quad score-page reordering91c414/20bab1 is also closed:26 actual focused
passes/16valid outputs,Sailing-1.782%(all4 slower),Golfer+.817%(2/4,variable).
Initial eager then_some draft26a compilepassed butwas fixed before any timing/test
because out-of-range debug arithmeticcouldoverflow. No receipt rekey,retry,app,
Clippy,Java,fullsuite or integration. Proof:
`compat/observations/real_world-quad-score-pages.json`.
Fresh selected-arena Sailing profile is collected:994cyclesamples,zeroLost,
one fulloutput independently valid including initialstate. Immutable84e98d,
d466/465509,snapshot446c;no build,Java or timinggain claim. New profile confirms
propagation74.17%inclusive and terminalapply_change remains a useful target.
Consumed-builtin equal-contribution total identity556741/ee74a1 is closed:
16valid outputs,Sailing-.763%(all4slower),Golfer-1.771%(1/4faster). Test-ready
7e61sameproduction hasraw16passed including builtinextremes/structuralreset/
presence,but wrapper143terminated before finalreceipt. Cargoexit unobserved;
explicit post-completion source/log recovery preserves that limitation. No
rerun,rekey,completewrappergate,app/Clippy/Java/fullsuite or integration. Proof:
`compat/observations/real_world-equal-contribution-total.json`.
Owned-inputset/replace inlining5087d9/30f189 is closed:16 validoutputs,
Sailing-.0077%/Golfer+.0716%,flat. Only2attributes;methodbodyidentity asserted,
unchangedunit functionality notrerun. No integration/retry/app/Clippy/Java/fullsuite.
Proof: `compat/observations/real_world-owned-input-inline.json`.
Branch-free single-score adjustmentc67279/332144 is closed:17 actualfocused
passes/16valid outputs,Sailing+.126%(2/4faster),Golfer-.754%(2/4faster). New
providedhook/defaultcustomdelegation,builtinwide1-pairimplementation andcaller
were notintegrated becauseSailingwasflat. Cargoexit0andfinalreceipt0areboth
captured;earlier equal-totalwrapper143remainsseparate. No retry/app/Clippy/Java/
fullsuite/newupstreamcredit. Proof:
`compat/observations/real_world-single-score-adjust.json`.
All currentbuild/test/profile jobs are terminal andcollected. Mainruntime763da12
is d466/465509/84e98d,actual originalJava4795 stillcurrent:all12valid,
Golfer1.603315x/Sailing.873827x,1/2targets,14.439%extraRustSthroughputneeded.
DashboardHTTP/freshness verifiedcurrent;do notrerununchangedtests/timings onturn/
commit/contextreset. v33helpers bindgenuinecurrentarena+Java proof. Local+runner-a,
EFSparked,no newsubagents;preserveotherwriterchanges.

Next worthwhile implementation direction:compact consumed collector pages to
8-byte optionalnonzero slotindices,and move canonicalArc keys into their arena
cells. This would preservekeyowners/rollbackwhilehalving pagecells andmaking the
orderedOwnedUndo enumfreeofArcdropfields. It is a plan,notimplemented ormeasured;
keep public/custom paths andcanonicalkeyobserver contract unchanged. Existing
selectedprofile locatesGroupState/Owned kernels asremaininghot work. Do not retry
closedgetter/closure/scorepage/identity/inlining/layout trials unchanged.


The preceding consumed-graph collector backend owns each mutable integer
collector in a box and records scalar and structural undo in one ordered journal.
Canonical keys retain their Arc owners; full and delta observations copy collector
values. Exact builtin I/K and empty ownership-consumed graph activation keep the
public/custom/shared collector contract on the original path. The native page
directory now accepts a page type; routing and arbitrary-key fallback are unchanged.
Private value allocation identities differ, and copied snapshots may cost more.

Four rotating native 3s pairs measured **Sailing +2.059% / Golfer +2.379%**, every
pair faster on both workloads; all16 full outputs independently validated. These
are native controls, not Java parity. Final source is
`3c4c5374f41eceedb9b7493c2519a77961f1718720ecf32a4af1bd3690b0e335`,
production `6d8ea76460b246a9cce512136d8889e2edaf9e43238f1d786ea8ab9812f8d27a`.
The genuine final release is byte-identical to measured artifact
`93906595c63e90a53264f09f51829a1e226d017401b39b60fb24634e1e103e3e`.
The only post-pilot edit is an explicit Clippy exemption for the retired-box pool.
Original receipts, sources and timings are preserved without rekeying.

Actual11 final-source directory/group-entry checks and lib/example Clippy pass.
The2 new independent history/overflow cases and4 external application cases pass
on pre-lint source49a390/c41fc8; they remain bound to that source and were not
repeated or credited as final-source executions. Current focused log credits11
passes, not a full gate. No upstream-method credit;2 native regressions added.
Original Java report5725 remains byte-identical and stale: neither current-source
target is yet qualified. Goal active/incomplete; do not add native gains to old
ratios. [Source and evidence](../compat/observations/real_world-owned-collector-box.json).

All new build/verification jobs are terminal and collected. Source integration
reuses their actual evidence. Existing Java5725 remains historical; no full suite,
new timing or repeated successful check is needed for commit. v31 pilot helpers
now bind genuine3c4/6d8/939 release/equivalence,11 final checks/Clippy and original
raw controls. First v31 dispatch passed its source/artifact/equivalence assertions. v30 remains historical
3fd/e40;usev31 for the next incremental candidate. A fresh profile can now inspect the changed ownership backend;
the older flat-map profile still describes its preceding runtime.

A fresh selected-artifact Sailing profile is available at
`work/real_world-goal/selected-owned-collector-sailing-profile`:actual10s seed7,
CPU2,3,994 cycle samples,zero lost,one independently valid full output. It is a
diagnostic,not another timing or Java qualifier. Owned set/record/entry/insertion
helpers account for about6% self samples. The held-page experiment65762/d1be
removes duplicate lookups while preserving scalar/structural order, but is closed:
Sailing+.456%(2/4faster),Golfer-1.603%(all4slower),16valid outputs. No Rust tests,
Clippy,app,Java or integration on that source. Main664745f unchanged. A separate
adjacent-scalar undo cancellation in the new ordered boxed journal6dca/b79e is
also closed:16valid outputs,Sailing-.937%(all4slower)/Golfer+.386%(3/4faster).
No further checks or retry. Do not reopen old shared-owner cancellation caches
or sparse HashMap prepared-entry trials unchanged. Direct summary-arena slot
writinga807/9db5 measured Sailing+.227%(4/4faster)/Golfer+1.776%(3/4),16valid.
The whole summary/carrier batch is now integrated as described above.
The compact-score-key whole batchae596/ee838 was closed:16 valid outputs,
Sailing+.357%(3/4 faster),Golfer-.654%(0/4). No tests/app/Clippy/Java or retry
on that rejected source. Both final summary jobs are terminal/collected.
Before another incremental pilot update genuine v31 baseline binding to selected
0b4fcc/55e146/5a1911 with its actual final release/equivalence and original controls;
do not rekey old receipts. No new subagents;local+runner-a,EFSparked.
Proofs: `compat/observations/real_world-owned-collector-held-page.json` and
`compat/observations/real_world-owned-adjacent-undo.json`.

Measured component proofs:
`compat/observations/real_world-summary-slot-write.json` and
`compat/observations/real_world-summary-carrier-reuse.json`.

Preceding selected batch: outline the existing Triple join publisher loop and make direction
constant per call. Same original scratch Vec, complete rows, Event construction,
scalar queue/FIFO/fanout, guards, trim/abandon, forced-shared tests and ordinary
shared/member/public/custom routes. No new graph field/vtable entry or event
grouping. Native source
`3fd07b3a6a8475be19697a2fa1d41a62a4f04d5e9aa67d4422bf48c80786ed98`;
production `e40ef237bbf32c86815e670362aad49f6ed5b10a288f9e68907e8d21b438e526`;
snapshot `3718d0698909b5ea9ed8615bd80d034ddcfd38cb6523315e507704b8616c783a`;
artifact `30da62d1dff19e9717d0c24f0d1e9870cd2cc1a6428a0cad57cd52b5a2e57700`.

Four actual rotating3s seed7 native pairs measured **Sailing +.84014%** (all4
faster), **Golfer -.34593%** (1/4 faster), all16 outputs independently valid.
This is a modest Sailing gain with a small Golfer tradeoff, not a universal gain
or extrapolated Java parity. Exact source passes 7 existing carrier +4 external
application tests and lib/example Clippy. **No full suite or new upstream-method
credit.** Current focused log credits only those 11 genuine source-bound passes;
previous21 log/metadata preserved unchanged. No repeated successful carrier tests,
release build, native timings, original Java matrix or browser check for commit.
OriginalJava report5725 remains byte-identical and historical/stale: it proves
G1.528433x/S.851557x on previous aee829, not this source. Neither current-source
throughput target is qualified; do not multiply native gains into old ratios.
Defer the original12-run qualifier until materially closer to Sailing parity.
Proof: `compat/observations/real_world-outlined-triple-publisher.json` and
`work/real_world-goal/outlined-triple-publisher-flatmap-next-results/`.

The optional all-None counter-table companion was measured as a whole combined
source70ffa/8949 and rejected: 21 relevant tests/16 valid outputs, Sailing-6.099%
(1/4 faster),Golfer-1.234% (0/4). No app/Clippy/Java or integration on that source.
Original real counters/owners were preserved; this result gives no isolated
counter-only speed estimate. Close it unchanged; no current-main test credit.
Proof: `compat/observations/real_world-optional-counters-outlined-publisher.json`.
All batch jobs are terminal and collected. Goal remains active/incomplete.

Two later transient-empty collector designs are closed without integration.
Both allow existing scalar inverses to remove the last occurrence inside a
consumed numeric trial, retain a private zero collector, emit the original
retraction and prune at outermost commit. The side-position-list version passed
16 focused tests and 16 output validations but measured Sailing -1.549% and
Golfer -1.450%. The leaner version derives cleanup from original inverse records
and normalizes full/delta observations; its 4 original regressions and 16 outputs
pass, but Sailing -2.046% and Golfer -1.235%. No app, Clippy, full suite, Java
qualification or main credit on either source. Preserve both; do not repeat.
Proofs: `compat/observations/real_world-transient-empty-collector-side-list.json`
and `compat/observations/real_world-inverse-derived-empty-collectors.json`.

Private graph input-address specialization is closed without integration. Exact
source passed 2 original lookup/bounds tests plus 7 owned-input and 5 group-entry
controls. First 16-output native run had substantial baseline drift: Sailing
+.375%, Golfer +7.567%. One timing-only retry reused the same built artifacts,
recorded CPU activity and validated another 16 outputs: Sailing -1.277%, Golfer
+3.954%. Preserve both and the fixed median-of-rates statistic. Possible Golfer
benefit does not establish a Sailing gain; no third unchanged run, app/Clippy,
full suite, Java qualification or current-main credit. All jobs collected.
Proof: `compat/observations/real_world-static-group-input-lookup.json`.

Two collector representation experiments are closed without integration. Binary
0/1 frequency storage regressed Sailing -1.255% and Golfer -.859%; all16 outputs
valid. Its local test build was interrupted and authoritatively stopped before a
result: no pass credit or restart. A separate thin uniform-value IntCollector
with general boxed frequency fallback measured Sailing -.203%/Golfer +1.328%,
all16 outputs valid. Benchmark-first rejection avoided further tests on that
source. No application/Clippy/full suite/Java run or main credit on either.
Proofs: `compat/observations/real_world-binary-frequency-counts.json` and
`compat/observations/real_world-uniform-thin-collector.json`.

The primitive group_entry inline(never) trial is also closed. Same entire body
and phases, just one compiler attribute. Actual ELF contains2 outlined entry
functions. Original native controls measured Sailing+1.844%/Golfer-.980%, but
Sailing baseline drifted47.7k-61.4k. One timing-only retry reused the same built
binaries with CPU telemetry: Sailing-1.836%/Golfer-.453%,all4 pairs slower each.
All32 outputs independently valid;no Rust tests/app/Clippy/full suite/Java run
or integration. No third unchanged retry or favorable-run selection. Proof:
`compat/observations/real_world-outlined-group-entry.json`.
All current jobs are terminal/collected. Main runtime remains daa8a18 (3fd/e40),
its11 focused passes and stale originalJava5725 unchanged. Goal incomplete.
Next work must remove actual propagation/rollback work; reuse existing profiling
and do not repeat closed compiler-hint or collector representation variants.

A separate private group-cell kernel was implemented using a sealed cell policy
and the original shared GroupNode/GroupCollectors algorithms. Public default
storage remains atomic/Send+Sync;only a consumed graph's empty certified numeric
groups become RefCell cells. Existing strong/weak fallback and dirty snapshot
copying remain. Actual4 new focused controls pass,16 native outputs validate,
but direct enum sourceb0da/28045 regresses Sailing-.698%/Golfer-1.378%,all4 pairs
slower each. It also has a known Default inference issue and2 visibility warnings;
no application/Clippy/full suite/Java or integration credit. Close it unchanged.
Actual compiled group-summary handlers grew22,871B to43,483B;that is code-size
evidence,not causal proof of cache misses or an isolated CAS cost.
Proof: `compat/observations/real_world-local-group-cell-diagnostic.json`.
The separate local-only outline refinement288cb/b5a8/890954 is also closed.
Atomic finish path unchanged;Local shared apply returns full old/new numeric
summaries after remaining exact builtin IK locals drop. Mapping,mutation,poison,
history and downstream guards remain;Default inference/visibility issues fixed.
Actual4 named focused cases pass including Default inference;wrapper terminated
143 after completed test log,so separate collection verifies source/log and records
missing child exit receipt. No rerun or fabricated successful wrapper receipt.
Original3s controls S+6.241%/G-2.957% had substantial Sailing drift. One longer
10s native-only control used same artifacts: S-2.783%,all4slower;G-1.055%,2/4faster.
All32 outputs independently valid;no application/Clippy/full suite/Java or main
credit. Raw worker prose erroneously says3s;actual16commands/child summaries and
collector assert10s. Explicit sidecar corrects note without altering raw evidence.
Actual handlers23770B plus32432B local helpers;not a causal speed explanation.
Proof: `compat/observations/real_world-outlined-local-group-cell.json`.
Both local-cell sources are closed unchanged;all jobs terminal/collected. No
current-main test or throughput credit;3fd/e40 and originalJava5725 unchanged.
Goal incomplete. Reuse current profile and avoid retrying these kernels unchanged.

Exact inverse-pair cancellationf601/c50 is closed. An8-slot numeric owner hint
matches only live owner/full position/normal-history boundary;opposing inverse
pairs may cancel while every forward edit/summary/callback remains. Dirty/current
capture/rollback/snapshot adapters skip canceled private holes;parent append
refreshes hints,stale/colliding hints miss only. Actual3 original tests and16
outputs pass,but Sailing-2.230%(all4slower)/Golfer-.914%(2/4faster). No app/Clippy/
full suite/Java or main credit. Initial test-only visibility/helper errors were
fixed before frozenf601;failed source/logs retained. Close unchanged.
Proof: `compat/observations/real_world-inverse-pair-cancellation.json`.

Next candidate is membership word first-preimage journaling,distinct from earlier
compact membership record packing. Each forward bit/predicate/publication/length
update remains. A small exact-key-validated frame hint can reuse an earlier word
preimage;collisions retain extra records. Rollback restores raw words then the
frame's saved original length,avoiding transient cross-word count overflow.
Nested/current/full capture and explicit clear remain supported. Prototype
`work/real_world-goal/membership-first-preimage-next-prototype/`;new focused checks
and native controls active. No performance or source credit before collection.

Previous selected batch: direct flat-map output. Whole ownership-consumed exact numeric
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
candidate artifact. Genuine report5725a1 was current on that source and is now historical; previous16b0 bytes and focused
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

Consecutive numeric filter dispatch was also implemented and rejected. Each
original predicate/cache update/guard still runs in FIFO order, but private
numeric input lifetimes and routing publication move to the run boundary. Two
original regressions preserve full callback traces, work, fanout owners, captures,
panic/errors and retry; all16 native outputs validate. Sailing -3.181%/Golfer
-4.342%, all4 pairs slower each. No integration or further checks/qualification.
Proof: `compat/observations/real_world-consecutive-filter-dispatch.json`.

Do not reopen closed static mapped reads, concrete-key callbacks, member-owner
pools, buffer handoffs, indexed-row variants, duplicated contiguous-bucket
registry, undo-packing, staged filter-run or optional counter-table source unchanged.

Future v30 helpers bind actual selected3fd/e40/30da, its11 focused tests,
Clippy and16 valid native outputs. They syntax-check and remain unrun. Java5725
is explicitly historical/stale and zero current-source targets are proven.
Historical v29 stays bound to aee829/59be6/6d297, v28 to ec026. No full suite,
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

Word first-preimage hashing17a81/32cca/bfe567 is closed. Its8-slot exact-key
hint cache coalesces only private membership word histories;all forward updates
remain. Actual2 focused checks and16 independent outputs pass,but Sailing-1.486%
and Golfer-1.288%,all4 pairs slower each. No more checks,unchanged retry or main
credit. The separate epoch-stamped u64-word alternative4176/48db/9b749 is also closed:
2 focused checks pass (including nested/capture/clone/exhaustion),16 outputs valid,
Sailing-.721%/Golfer-1.548%. No further checks or retry. The simpler consecutive-
word variant7446/7108 passes16 output validations but Sailing+.137%/Golfer+.893%
is flat/mixed;benchmark-first rejection avoided Rust tests. Both preserved and
unintegrated. Main3fd/e40 stays selected,goal incomplete.

The collector prepared-entry experimentcf12/47986 is closed too: one lookup
serves inverse or cloned replacement, but returning the large prepared entry
measured Sailing-5.809%/Golfer-5.814%,all4 slower each,16 outputs valid. No Rust
tests/app/Clippy/full suite/Java or integration. Earlier Result-interface source
passed a lib/example check;not a frozen-source test claim. The separate refinement
dc35/184ed keeps the entry and entire fallback internal and returns only summary.
It still regresses Sailing-1.365%(all4slower)/Golfer-2.373%(1/4faster),16 outputs
valid. Both sources closed;no further checks or unchanged timing retry. All jobs
are terminal/collected. Main runtime and historicalJava5725 unchanged;goal active.
Next work needs a different mechanism;do not revisit these membership histories
or collector-entry shapes unchanged. Existing cycle profile remains available.
Proofs: `compat/observations/real_world-membership-first-preimage.json`,
`compat/observations/real_world-membership-epoch.json`,
`compat/observations/real_world-membership-consecutive.json`,
`compat/observations/real_world-group-prepared-edit.json` and
`compat/observations/real_world-group-held-entry.json`.

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
