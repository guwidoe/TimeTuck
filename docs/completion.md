# Completion evidence and remaining gaps

The original target is native Rust planning and optimization behavior, using
pinned Timefold Community tests and public APIs as specifications. Java API/ABI,
reflection, framework integration and Enterprise were excluded in the initial
[scope](compatibility.md#target). These boundaries have not changed.

The implementation now has executable evidence for the following requirements.
Each row describes verified behavior within its contract, rather than a promise
about every problem size, search trace or finite-budget optimum.

| Requirement | Executable evidence | Limits |
| --- | --- | --- |
| Independent native solver, Community reference and attribution | Rust-only [crate](../crates/solver/Cargo.toml), pinned [reference](../compat/upstream.json), [provenance](../compat/provenance.json), notices and report validation | No Java runtime in the production library; no formal clean-room or whole-Timefold equivalence claim. Crates retain `publish = false`. |
| Scores and independently checked arithmetic | Integer/decimal fixed and bendable score tests; [score comparisons](../scripts/differential_scores.py) and native checked-scoring regressions | Overflow, exact decimal and initialization policies are specified in the [score contract](score-contract.md). Cross-runtime transcendental rounding is not established. |
| Basic/list/mixed models, moves, pins, ranges and rollback | [Temporary move tests](../crates/solver/tests/temporary_moves.rs), independent sublist/pillar/group successor tests, physical k-opt execution and exact search | Equality/explicit IDs define native identity. Private Java descriptor arrays and reflection are not native APIs. |
| Construction, heuristic improvement, repair and exact optimization | [Search contract](incremental-search-contract.md), independently enumerated optima, retained-best/budget-prefix tests, [branch-and-bound](../crates/solver/tests/branch_and_bound.rs) | Heuristics retain bounded best results; only exhausted exact search with valid bounds proves optimality. Seeds do not imply identical Java traces. |
| Incremental scoring, constraint streams, collectors and derived values | Transactional graph/collector tests, full recomputation checks, [stream comparisons](../scripts/differential_streams.py), dependency/cycle recovery tests | Explicit native policies and observed Java discrepancies below remain visible. User callback side effects and allocation aborts are outside rollback guarantees. |
| Configured applications and solver services | Owned XML/model factories, live daemon changes, cancellation and manager tests; [roster file workflow](owned-roster-workflow.md), [mixed cold-start qualification](../crates/solver/tests/mixed_benchmark_regression.rs), canonical solution persistence | Native typed bindings replace Java frameworks. Scheduling/time limits are cooperative. Larger-case optimum and arbitrary-scale quality are not proved. |
| Visual test and performance dashboard | [Report pipeline](dashboard.md), desktop/mobile rendering evidence, actual native/Community timings with quality, work, timestamps and input hashes | Method coverage uses statically discovered annotated source methods, not all JUnit runtime invocations. Stale measurements remain stale until rerun. |

The current native verification contains **8,587 Rust tests and 292 doctests**,
including original regressions and adapted upstream assertions. Source
`5342c37ce8eaf6b7449b4dda42bfc8512110e94b3aaf7685302de07d302fe48c`. The final check passed in 329.803s,
executing 96 jobs and reusing 52 unchanged results. The live report remains
the authority for later counts and freshness.

The applications establish concrete quality criteria: both roster backends solve
128 initially unassigned shifts to the planted zero-penalty certificate; mixed
cold construction reaches the independently enumerated small optimum and beats
the larger feasible certificate while preserving pins. These results establish
bounded application behavior, rather than universal optimality or performance.

The inventory has **4,410/4,855 passing adapted methods (90.834%), 10 pending,
435 excluded and zero failed/unverified**. The percentage counts statically
discovered annotated source methods, not all JUnit runtime invocations or overall
project completion. Even clearing all 10 pending methods would give only 91.04%
with the existing exclusions; exclusions remain visible in the denominator.
The 10 pending methods include eight JVM/loading/framework seams and two exact
private k-opt endpoint/partner-array assertions. They are not evidence of ten
missing native algorithms, and native implementation does not make those original
assertions pass. One complete Community version-identity method now preserves
its literal edition assertion using existing native Cargo package diagnostics.
This adds no runtime feature, JVM loading or Enterprise functionality.

Known differential limits remain in connected-range duplicate/empty-interval
semantics, default exact arithmetic versus Community overflow and other
recorded identity/ranking policies. Shared cyclic-supplier comparisons retain
21 actual Java runtime errors separately from successful-result agreement.
See the [stream contracts](constraint-stream-contract.md) and
[compatibility evidence](compatibility.md). Native-reference passes do not prove
Java agreement. Completion still needs evidence for every original-scope
requirement and resolution of the remaining relevant behavior differences.

The last solver292 comparison (now stale) retains all 4,824 exact encounter-order
observations plus the original total-order controls (4,499 agreements and 325
differences). Explicit binary64 Great Deluge now agrees in 276/276 original cases,
25/25 boundary prefixes and 184/184 additional probes; exact-default controls stay
275/276. These solver inputs are unchanged by the optional mixed graph declaration.

The last stream comparison (289) is stale. Explicit signed-32-bit wrapping metrics
retain 65/65 sequence agreements and 66/197 range agreements; all 197 native range
cases pass independent modular calculations. The remaining 131 Java differences
stay visible, including contradictory upstream histories. Exact-default controls
retain their separate overflow differences. Binary64/wrapping load balance retains
101/101 agreements, wrapping-exact-root 100/101 and exact-default 97/101. Native
regressions and policy options do not earn extra upstream-method credit.

## Latest qualification — 296 checked ordinal list registry

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

## Previous qualification — 294 fused owner-link application graph

`--native-stream-graph-policy owner-links-fused` selects direct fixed-link
projection and scoring in one tracked node. Ordinary joins remain the default;
owner-link v1 remains available. Fused v2 retains the same triple match IDs,
full payloads/scores, source rows and list shadows as v1. Projection/scoring now
interleave; ownership, cloning, callback/error timing and journal installation
can differ. This is an explicit application declaration, not generic operator
trace equivalence or equal Java search work. Dense and EqOnly remain unchanged.

All four existing policy regressions now cover all three declarations, including
snapshots, callback boundaries, panic/recovery, partial objectives and all factory
paths. Three additional Python contracts retain both full controls, all original
cases/profiles, unknown fields and actual v1/v2 physical call equality. No tests,
fixtures, budgets, quality floors or upstream assertions were removed.

One controlled three-declaration comparison retains all 16 original rows across
both search profiles, four rotating CPU2 forks. Larger component Stream medians
improve **7.82% warm / 14.18% cold** against v1; original joint improves
**13.27% / 7.36%**. All four larger-case forks improve. Small controls remain,
including mixed small joint/cold forks; Dense jitter earns no improvement claim.
Every full output and physical scoring-call count matches v1, apart from elapsed
time and declared graph metadata. Its actual native source/artifact are also the
final native source/artifact; the later full-driver input key is recorded separately.

Final `bash scripts/check.sh`: **8,578 Rust +292 doctests**, **5 RUN/143 REUSE**,
81.859s, source `46069f24e4025134fff09c60de7f5f3b3d1659f965fea3bbabdd767190d7e62e`.
Only mixed-performance was refreshed; other 17 raw reports retain bytes/dates/keys.
Solver292 remains current. Fresh Java comparison: **7/8** meet median time
parity, equal scores in all eight. Larger Stream warm is **16.365 ms Rust /
14.959 ms Java**; cold **15.790 / 19.402 ms**.
All ordinary and v1 observations/samples remain in the report, with actual scoring
calls and unchanged full search outputs checked. No general speed, memory or
larger-optimum claim follows.

Dashboard index **86.285%**, **2/18 current**, 16 stale, Java target **1**.
This is a tracked-target index, not remaining effort or an ETA. Inventory stays
4410/4855, 10 pending, 435 excluded, no failures/unverified. Actual desktop/mobile
checks cover selected fused and both prior declarations. Goal incomplete.
Evidence: work/round294-qualification/{integration,candidate-build-receipt,
final-build-receipt,pair-results,gate-receipt,mixed-performance-receipt,collection,
browser-proof}.json; work/parallel-round294-evidence.json.

## Investigation 295 — shared captures rejected; position lookup hotspot

The experimental shared-row projection API and mixed v3 policy were built and
checked privately, then removed: the larger warm case was 0.675% slower than
same-source v2; cold was 1.266% lower with mixed forks. Full outputs and physical
callbacks matched across all 16 original workload/profile rows. Six new core tests
and four expanded mixed tests passed focused checks, but no broad gate or Java
qualification was run for this rejected source. The complete source/binary/results
remain in `work/round295-qualification/rejected-sharing/` and `pair-results.json`.

A CPU-clock profile of the actual qualified v2 binary recorded 3,178 samples,
zero lost. Changed-value-position discovery accounts for 10.98% self /14.88%
inclusive sampled CPU. All normalized native output matches its qualified sample.
This is diagnostic whole-program sampling, with no matched timing or general
speed claim. [Recorded evidence](../compat/observations/mixed-stream-value-position-profile.json).
Exact qualified294 source, all 18 measurement keys and raw reports were restored;
existing verification and browser evidence remain valid. No current feature or
upstream-method credit is added. The goal remains incomplete.

## Previous qualification — 293 explicit owner-link application graph

The mixed example now offers an explicit `owner-links` graph declaration while
ordinary joins remain the default. Shared fixed owner relationships and a fixed
tracked terminal retain full match payloads/scores, planning rows and list shadows.
Actual match IDs, endpoint ownership, callback order/error points and final batch
publication differ. Dense and EqOnly construction stay unchanged. This is an
application declaration option, not an ordinary-join implementation or equal-work
speedup. Warm/cold setup remains inside the original solve timing boundary.

Four original Rust tests cover actual typed IDs, full source/match snapshots,
shadow metadata, joint callback boundaries, rollback, callback panic/recovery,
partial objectives and every warm/cold/profile/backend factory. Five new original
Python contracts cover explicit metadata, runtime rotation, full controls and
unknown output fields. They add no upstream-method credit.

A controlled same-artifact native comparison retains all eight original rows for
each of both search profiles, four balanced CPU2 forks. Larger Stream component
refinement is **11.44% faster warm / 21.56% cold**; original joint search is
**17.88% / 16.44% faster**, all four larger-case forks improve. Complete native
outputs match except declared graph metadata, elapsed time and actual physical
score calls. Small controls are retained; small component warm has mixed forks.
Dense time variation earns no optimization claim. These trials retain their actual
pre-test source/artifact receipt; final Java evidence below has its own final build.
An explicit HashIndexed client trial was rejected after complete 16-row evidence:
larger component Stream was 35.69% slower warm and 27.21% cold. Do not repeat it.

The final `bash scripts/check.sh` passed: **8,578 Rust +292 doctests**,
**5 RUN/143 REUSE**, 82.993s. Native
`645c98d839097fd86e965e69a56e46df6dc9d63583fbdd867e5960b0ab6bed1b`. Only two Rust test executables needed rebuilding.
One earlier actual Clippy clone-on-copy failure is archived separately; the final
source uses a primitive copy. No successful unchanged gate or trial was repeated.

Fresh original Java mixed comparison: **7/8** at measured Java time parity,
equal final scores in all eight. Larger Stream warm is
**18.377 ms Rust / 15.330 ms Java**; cold
**19.272 / 20.957 ms**. Every ordinary Stream control
sample and full observation remains alongside the selected owner-link result.
All original inputs, pins, budgets, quality floors and both native backends remain.
No general speed, identical search work, memory or larger-optimum claim follows.
The fresh small cold Stream median is 0.506 ms selected versus 0.479 ms for its
ordinary control (5.51% slower); this regression remains in the full report.

Only mixed-performance was refreshed; all other 17 raw reports retain bytes,
dates and keys. The relevant solver292 key is unchanged and remains current.
The dashboard index is **86.114%**, **2/18 current**, 16 stale, Java multiplier
**1**, with selected graph policy, control medians and physical calls visible.
The index measures tracked targets, not remaining effort or an ETA. Inventory
stays **4410/4855**, 10 pending, 435 excluded, no failures/unverified. The recorded
behavior gaps remain; goal incomplete. Actual desktop/mobile browser checks cover
selected and prior ordinary declarations plus source/model/live freshness.

Evidence: work/round293-qualification/{integration,prebuild-receipt,
final-build-receipt,owner-links-pair-results,mixed-performance-receipt,
gate-receipt,collection,browser-proof}.json; rejected-registry/ and
initial-clippy-failure/; work/parallel-round293-evidence.json.

## Previous qualification — 292 explicit binary64 Great Deluge

`GreatDelugeRise::Binary64Ratio` adds an explicit floating accumulation policy;
exact `Ratio` and configured defaults remain unchanged. Integer initial water
must have scale zero and fit i64. Separate operand casts, cumulative ratio,
absolute-base multiplication, base addition and floor define the operation order.
Wide finite thresholds decode exactly to BigInt without i64 saturation. Initial
conversion may lower the first threshold; later thresholds can stagnate. A
u64-bounded rise count keeps operations finite and freezes at MAX. All six
basic/list/mixed scoped/stream phase paths use the existing shared acceptor.

Actual Java and independent Python comparisons agree in **276/276 original
cases**, **25/25 boundary prefixes**, and **184/184 separate additional ratio/budget
probes**. Complete exact-default controls remain **275/276** and **23/25 prefixes**;
their full Java/native outputs and exact references match prior actual outputs.
No fixtures, budgets, order, pins, quality checks or old differences were removed.
No Java internal thresholds/traces, general arithmetic/overflow/decimal equivalence
or additional upstream-method credit is claimed. The explicit policy closes the
observed boundary; the original exact arithmetic contract was not a native bug.

Final `bash scripts/check.sh`: **8,574 Rust +292 doctests**,
**107 RUN/41 REUSE**, 856.146s, source
`838a5d9408978312d16a475f30512b14b8bf42a4164ece84aaf76f87f9e64ad3`. Four original arithmetic regressions,
a private counter-saturation boundary and expanded six-engine phase coverage are
included. Actual desktop/mobile browser checks cover selected and prior exact
presentations plus current source/model/data checks; the UI detector is clear.

Only solver-differential was refreshed. All other 17 raw reports retain bytes,
dates and original keys; mixed 291 and stream 289 are now stale. Inventory stays
4410/4855, 10 pending, 435 excluded, no failed/unverified. The dashboard index is
**85.448%**, **1/18 current**, Java multiplier **1**. It measures tracked targets,
not remaining effort or an ETA. Larger mixed Stream parity and recorded range,
list-metadata and Java-error gaps remain. Goal incomplete. All execution was local;
runner-c remains parked, no new remote authentication attempt or new subagent was made.

Evidence: work/round292-qualification/{integration,solver-differential-receipt,
gate-receipt,collection,browser-proof,current-state}.json, retained complete
controls and work/round292-deluge-review/{README,FOLLOW-UP}.md;
work/parallel-round292-evidence.json.

## Previous load-balance qualification (289; now stale)

`WrappingLoadBalanceRootPolicy::Binary64` is an opt-in observation policy on
`WrappingLoadBalanceCollector`. Captures and modular moments retain the existing
transactional paths. Separate binary64 conversion, division, subtraction and
square root precede six-significant-digit half-even rounding of the exact binary
root value. Exact arithmetic and wrapping-exact-root defaults remain unchanged.
Typed negative radicands retain captures and permit recovery; snapshots and clone
policy are covered. Four original Rust regressions add no upstream-method credit.

The complete original load-balance corpus now agrees in **101/101 cases across
8,290 states** with actual Java and the independent integer/Fraction reference.
The large-offset cancellation difference closes under this explicit policy.
Both complete control arrays remain identical to their prior actual outputs:
**97/101 exact-default**, **100/101 wrapping-exact-root**. Java failures receive
no agreement credit. No general Java formula, conversion-order, decimal-scale,
nonfinite, live-map or timing equivalence is claimed.

One final `bash scripts/check.sh` passed: **8,566 Rust tests and
292 doctests**, **9 RUN/139 REUSE**, 254.298s. Native
`fbd259a1f3d6a96c72f9a92d2eb1ff54fb9ad92cfe812ede6b901a02dc554ad9`. The already-qualified unchanged
reference-model tests were reused; no successful unchanged gate was repeated.
Actual desktop/mobile browser checks cover selected and both prior policy
presentations; current source/data/model HTTP checks pass.

Only stream-differential was refreshed. All other 17 raw reports retain actual
bytes/dates/keys; mixed 288 remains current under its unchanged relevant key,
still **6/8** at Java time parity and equal quality. Larger warm/cold Stream
remain performance gaps. Readiness **85.814%**, **2/18 current**, 16 stale;
Java multiplier **1**. This is a tracked-target index, not engineering effort or
an ETA. Inventory stays **4410/4855**, 10 pending, 435 excluded, no failures or
unverified methods. Goal incomplete; other recorded behavior gaps remain.

Evidence: work/round289-qualification/{integration,release-build-receipt,
offline-replay,stream-differential-receipt,gate-receipt,collection,verdict,
browser-proof}.json and work/parallel-round289-evidence.json.

## Previous qualification — 291 bounded source arena

Private bounded planning sources retain complete lazy-shared rows in reusable
arena cells; active slots and every undo entry move optional integer handles.
Every intermediate version, source ID, metadata row, scoring callback and snapshot
Arc identity remains. Clone/callback failure contracts are retained; panicking user
destructors and allocator aborts remain unsupported. Normal replacement, rollback and final
node destruction retain their distinct original payload-release order. Extra
indirection and high-water storage may increase memory; no memory saving is claimed.
Three original ownership/Clone-panic regressions passed against original storage,
then all 13 focused source tests passed against the retained arena. A later test-only
Clippy loop repair preserves identical production inputs and all measurement keys;
original build/performance receipts retain their actual pre-repair source. They add no
upstream-method credit.

Initial all-eight-workload comparisons were mixed: larger warm Stream +2.56%,
cold -4.76%. One bounded CPU2 follow-up retains all four Stream controls across
16 forks: larger warm **2.069% faster** (14/16 forks), cold **1.552% faster** with
mixed forks, small warm **0.502% slower**, small cold **4.702% slower**, both mixed.
All full assignments, scores, callbacks, evaluations, steps and phase metadata
match the qualified original artifact. These are bounded artifact observations,
not a general speed claim. Extra in-place release tuning was rejected: small warm
Stream +13.21% in all four triad forks and larger cold +5.01% versus initial arena.
All initial/follow-up/triad records and actual source/artifacts remain separate.

After fixing the test-only Clippy failure, the final `bash scripts/check.sh`: **8,569 Rust +292 doctests**,
**95 RUN/53 REUSE**, 374.629s. Source
`1be98a828da84045af124b47f62490e4ff04478fce7e116740ad016341b60277`. A build-only precompile used eight local
compiler jobs; verification records were neither fabricated nor transplanted.

Fresh original Java mixed comparison: **6/8** at time parity, equal quality in
all eight. Larger warm Stream 22.042 ms vs Java 17.253 ms;
cold 23.970 ms vs 19.124 ms. Inputs, profiles, budgets and
quality floors remain unchanged. Cross-run Java ratios do not isolate native gains.
Only mixed-performance was refreshed; other 17 raw reports retain actual bytes,
dates and keys. The 289 load comparison remains 101/101 selected, 100/101 wrapping
exact-root, 97/101 default, now visibly stale. Backend/UI contracts are unchanged:
reuse actual 289 desktop/mobile proof plus current source/data/model HTTP checks.
Readiness **85.430%**, **1/18 current**, 17 stale, Java multiplier **1**. Inventory
4410/4855, 10 pending, 435 excluded, no failures/unverified. Goal incomplete.

A first final-comparison attempt was invalid: restoring source with preserved
mtime made Cargo reuse the rejected tuning binary. The post-run artifact guard
caught it; its raw report/cache were archived and invalidated, prior report restored.
After advancing source mtime, actual rebuild reproduced selected binary84a3356b…;
the valid comparison verified it before and after measurement. The invalid attempt
earns no performance/currentness credit. On future source restoration, write bytes
or advance mtime; verify actual compiler artifact before measuring. Do not copy
old binaries/cache metadata into target or rekey old successes.

Evidence: work/round291-qualification/{integration,baseline-focused-receipt,
candidate-focused-receipt,candidate-build-receipt,restored-build-receipt,
pair-results,prebuild-receipt,mixed-performance-receipt,gate-receipt,collection,
verdict}.json; control-followup/, inplace-release/, invalid-cargo-reuse/;
work/parallel-round291-evidence.json.

Rejected 290: a lone inline(always) annotation on source replacement made larger
warm Stream 4.40% slower (3/4 forks), cold 1.34% slower with mixed forks. Exact 289
source/keys and all 18 report bytes were restored before 291; no final gate/Java
refresh for that rejected source. Evidence: work/round290-qualification/verdict.json.

The last full load-balance comparison (286; now stale) follows.

The additional `WrappingLoadBalanceCollector` uses signed i64 wrapping loads,
totals and squared moments, then a widened exact rational root. It returns typed
negative-radicand Results without losing captures. Exact defaults remain unchanged.
The complete original load-balance comparison now agrees in **100/101 cases**
across **8,290 states**; default controls retain **97/101**, with identical output
to their previous observations. Independent Python validates all 101 native cases
under each policy. Three recorded overflow differences close; large-offset binary
floating cancellation remains unresolved. No Java scale, live-map or nonfinite
behavior claim; eight original regressions and one doctest add no method credit.
Evidence: work/round286-qualification/{collection,verdict,gate-receipt,
browser-proof,stream-differential-receipt,mixed-performance-receipt}.json.

The retained 285 aggregate optimization and its original evidence follow.

Primitive frequency replacement now checks the old class once, decrements a
duplicate or detaches a singleton, then inserts or merges in one destination
traversal. A small inline helper exposes this scalar update to its callers.
Checked/wrapping arithmetic, captured occurrences, shared snapshots, physical
node reuse and inline demotion hysteresis are preserved. Three original
regressions supplement eight retained internal tests, with no upstream-method
credit. Existing `SortedSetCollector`/`SnapshotSet`, `SetCollector` and defaults
remain unchanged.

Actual four-fork comparisons against qualified 282 measure **16.59% lower large
checked-sum pair time**, improving in every fork. Small sum has a **1.14% higher
pooled median**, with mixed forks. Initial large set/map controls were
21.34%/3.20% slower; one bounded fixed-artifact 16-fork follow-up measured
0.49%/0.69% lower medians, with mixed forks. Both records remain visible. No
small-sum, set/map, general collector or whole-solve speedup is claimed.

Last measured aggregate Java comparison (285; now stale):

| Collector / captures | Rust µs / pair | Java µs / pair |
| --- | ---: | ---: |
| Checked sum · 64 captures | 0.296 | 0.318 |
| Checked sum · 512 captures | 0.435 | 0.393 |
| Sorted set · 64 captures | 0.868 | 1.007 |
| Sorted set · 512 captures | 3.178 | 5.293 |
| Merged map · 64 captures | 3.096 | 9.294 |
| Merged map · 512 captures | 6.194 | 10.896 |

Five of six cases meet measured Java parity. Larger sum remains about 10.9%
slower than Java in this run. Cross-run ratios vary and do not isolate the
native change. Earlier 282 set gains and 281 sum gains retain their original
artifact/source bindings. Evidence: work/round285-qualification/{collection,
pair-results,verdict,gate-receipt}.json and large-control-followup/pair-results.json.

The retained 288 Java comparison (now stale) retains all eight mixed workloads, complete
inputs, warm/cold/backend settings, budgets, quality floors and full observations.
**6/8** meet measured Java parity. Larger Stream warm is
**22.273 ms vs Java 15.646 ms** (42.4% slower), cold
**23.547 ms vs Java 19.906 ms** (18.3% slower).
Scores remain [0,-338] warm and [0,-60] cold. Larger Dense is
5.667/5.782 ms warm/cold. The larger reference [0,-72] is a feasible
certificate, not an optimum. Different strategies and callback counts remain
explicit; cross-run Java ratios do not isolate a native implementation gain.

The dashboard's configurable progress index is **85.4%**: feature families 40%,
behavior 20%, performance 30%, current evidence 10%. It is an explicit index of
tracked targets, not estimated engineering effort or remaining time. Family
balancing and weakest-workload scoring prevent numerous easy/fast cases from
hiding gaps. Stale measurements retain their last-demonstrated contribution with
visible freshness debt; 100% requires every target and current evidence. See
[the calculation](dashboard.md#project-completion-index).

The retained 261 optimization scans touched rows to discover sparse Eq-only
value updates, preserving full metadata and registry-ordered source callbacks.
It adds no Hash/Ord bounds or benchmark-profile changes. Controlled 512-value
temporary updates improve 35–45% locally and 35–48% on runner-a, depending on whether
value rows are observed, in all four forks. These timings include hot updates and
rollback, exclude setup/full diagnostics, and are not Java or whole-solve speed
claims. Smaller mixed workloads remain roughly flat with some slower controls.
Sparse, nested and callback-panic regressions include 32 registered Eq-only values.

The explicit contribution-only scoring APIs retain IDs and cached contributions
without terminal tuple payloads. Match inspection/explanations return a named
unavailable error; tracked defaults and full upstream source rows remain intact.
Twelve original tests cover replacement/retraction, transactional failure and
rollback, saved candidates, exact weights/replay, occurrence metrics and per-ID
consistency. Whole-solve private controls preserve assignments, quality and actual
callbacks. Larger mixed timings show no useful gain; canonical profiles remain
unchanged. No general speed or whole-graph memory claim is established.

The earlier checked-sum implementation avoids internal snapshot copies and extrema
summaries while retaining capture frequencies and external persistent snapshots.
Controlled native sum timings fall 73.6% locally and 71.1–73.3% on runner-a, across
four balanced forks. A four-rank inline queue for single-source batches reduces
large change-solve timings by 5.57–7.68% locally; multi-change routing is unchanged,
and swap controls include slower observations. Original inputs, quality and physical
callbacks remain intact. These native comparisons establish no general speedup.
The last measured round273 stream comparison (now stale) retains equal scores on all workloads.
Large cases (Rust/Java): hill/change 27.418/33.607 ms; tabu/change 28.272/31.665 ms; hill/swap 371.908/363.011 ms; tabu/swap 375.247/375.265 ms.
Java parity remains the target, multiplier 1.

The earlier 265 sorted-set replacement used one staged snapshot, preserving multiplicities,
representatives, exact comparison/clone sequence and failure rollback. All 96
original aggregate controls and complete timed observations pass. On runner-a,
64/512-capture updates improve 11.01%/5.58%, all four forks; unchanged map controls
are flat. Local and sum controls vary; there is no general or whole-solve claim.
Last measured round265 Java comparisons, now stale, show gaps: set64 Rust 7.671µs/Java 4.752µs,
set512 25.733µs/15.925µs, sum512 1.976µs/1.520µs. Shared-host measurements vary;
do not attribute cross-run Java ratio changes to this optimization.

Fixed-link batches now resolve replacement inputs directly by endpoint slot,
retaining prior callback/ownership traces and panic recovery. One usize per
endpoint and small private vectors trade space for repeated lookup work. The
controlled large hill/swap confirmation improves 1.44% across all four forks;
other timings are variable. This is a modest native gain, not general Java parity.
No benchmark fixture, work, budget or validation is reduced.

Explicit sum-only grouping now retains each source ID's captured key/value and
private membership count/total. Checked and wrapping modes, final publication,
zero membership, rollback, saved candidates and reconstruction are covered by 17
original tests. The canonical adapter selects the production wrapping operator
because its constraint needs only the total; original inputs, move limits,
independent quality checks and physical score callbacks remain intact. Existing
full-summary APIs stay available. Empty checkpoint carriers add bounded idle
allocation space while releasing user values at their original points; three
original ownership/nesting regressions cover the change. Native controlled results
are recorded per workload and host; local controls include a slower private
hill/change case, so no general or standalone checkpoint gain is claimed.

Evidence: work/round281-compact-counts/{collection,verdict,gate-receipt,job-accounting}.json;
work/round279-allocation-probe/candidate-receipt.json;
work/round275-qualification/collection.json, gate-receipt.json,
solver-differential-receipt.json, browser-proof.json and verdict.json;
work/round273-qualification/collection.json, gate-receipt.json,
verdict.json, final-build-receipt.json, match-validation.json, pair-results.json,
remote-pair-results.json and stream-performance-receipt.json;
work/round272-qualification/collection.json, gate-receipt.json,
verdict.json, build-receipt.json, pair-results.json, remote-pair-results.json
and stream-performance-receipt.json;
work/round270-qualification/collection.json, gate-receipt.json,
verdict.json, build-receipt.json, pair-results.json, remote-pair-results.json,
profile-receipt.json and stream-performance-receipt.json;
work/round267-qualification/collection.json, gate-receipt.json,
verdict.json, build-receipt.json, pair-results.json, remote-pair-results.json
and stream-performance-receipt.json;
work/round266-qualification/collection.json, gate-receipt.json,
verdict.json, stream-remote-results.json,
stream-confirmation/stream-remote-results.json and stream-performance-receipt.json;
work/round265-set-qualification/collection.json, gate-receipt.json,
verdict.json, aggregate-paired/results.json, aggregate-remote-results.json and
aggregate-performance-receipt.json;
work/round264-parity/collection.json, gate-receipt.json,
aggregate-paired/results.json, aggregate-remote-results.json,
final-stream-paired/results.json and both performance receipts;
work/round263-score-only/collection.json, verdict.json and paired/results.json;
work/round261-touched-scan/verdict.json, collection.json,
performance-receipt.json and java-diagnostic-difference.json;
work/round262-community-version/collection.json and gate-receipt.json. Historical results
and retained tradeoffs remain in [next-session.md](next-session.md). Goal incomplete.

The 268 explicit batched filter-map fuses the canonical assigned-row projection
while retaining source facts, cached acceptance and normal rollback/inspection.
Controlled same-library runner-a large 96-variable hill/change and hill/swap timings are
2.69%/3.10% lower in all four forks, with unchanged full output/callback signatures.
Local large swaps improve 3.40%; local changes vary and have a 1.69% higher median.
These are bounded native improvements. Its then-current Java large hill/swap
measured 571.280 ms Rust versus 342.414 ms Java at equal score; the newer
round270 measurement is above. Cross-run timings do not isolate gains.


Optional shared-input fixed-link scoring now combines relationship lookup,
projection and tracked terminal evaluation, retaining source facts, complete match
inspection, zero/equal contributions, checked arithmetic and transactional
rollback. Its explicit callback/ownership contract differs from separate nodes;
the older declarations remain available. Four shared-input and twelve fused
original regressions cover state, arithmetic, replay, actual work and panic paths.
The adopted fused declaration reduces controlled large hill/change and hill/swap
elapsed times by 14.21%/8.70% on runner-a and 11.27%/9.78% locally, all four forks.
Sharing alone makes runner-a swaps 5.24% slower; the unchanged declaration control
is roughly flat there and variable locally. Full output and physical-callback
signatures agree. No general, tabu timing, standalone sharing, memory or Java-parity
gain is claimed. Only the affected stream comparison was refreshed.


Validated bounded usize sum groups now retain low integer-key summaries in indexed
primitive slots, with ordered fallback for arbitrary larger keys and unchanged
generic/unbound maps. Complete input captures and graph rollback remain intact.
Batched sum/filter-map boundaries reuse integer input-position scratch; generic
identity coalescing and ordinary filter-map remain unchanged. Eight original
regressions cover state histories, boundary keys, ownership and panic recovery.
Actual controlled native timings across all eight original stream workloads are
recorded by host in the latest handoff. The fixed cache adds bounded retained
space and copies primitive slots for saved states; no memory-saving, standalone
component or general speed claim follows. Only the affected stream comparison
was refreshed; other 17 raw comparison records retain their actual keys and dates.


Explicit fixed-universe filtering now retains byte membership and declared IDs,
forwarding exact accepted event owners and retaining complete facts upstream.
The batched preference path combines it with existing fixed scoring; full match/
rollback states and actual callback counts remain independently checked. Eight
original tests add no upstream-method credit. Against qualified 272 executables,
all eight original stream workloads improve 14.15–20.18% on runner-a and
13.63–23.06% locally, all four forks. These are combined workload-specific native
gains, not general performance or memory claims. Contracts, limits and per-workload
results are in the latest handoff.


Explicit encounter-order finalist policies now cover the observed exact-reference
boundary while retaining original total-order modes/defaults. All 4,824 original
public-metric cases agree with pinned Community and an independent segment oracle
under the explicit selection; full original 4499/325 controls remain recorded.
Five original regressions cover state saving, structural/accepted priority,
exact numeric boundaries and all six native search paths. These checks add native
behavior and close a measured policy gap; they add no upstream-method credit or
general trace, PRNG or performance claim.
