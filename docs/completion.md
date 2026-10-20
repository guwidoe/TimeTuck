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

The latest integrated checkpoint has **8,533 passing Rust tests and 290 doctests**,
source digest `feeaefbcff11143b4fab573613cdae9e768ca1af58221bce2e3c52dede75cfaf`. The required final
`bash scripts/check.sh` passed locally in 888.768s, executing 95 jobs and
reusing 53 genuinely unchanged results. One successful final-source gate;
no repeated unchanged tests or transplanted cache records. The preceding 280
qualification retains its stopped prebuild and corrected regression failure;
those are separate from this batch. The live report remains the authority for
later counts and freshness.

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
semantics, default exact arithmetic versus Community overflow, large-number load balancing and other
recorded identity/ranking policies. Shared cyclic-supplier comparisons retain
21 actual Java runtime errors separately from successful-result agreement.
See the [stream contracts](constraint-stream-contract.md) and
[compatibility evidence](compatibility.md). Native-reference passes do not prove
Java agreement. Completion still needs evidence for every original-scope
requirement and resolution of the remaining relevant behavior differences.

The last solver differential demonstrates 4,824 exact observations for explicit
encounter-order policies; original total-order controls retain 4499 agreements /
325 differences, all matching independent Python. These results now remain stale
under changed shared-core inputs. Existing defaults/policy choices stay visible.
Only stream-differential was refreshed in 280, retaining every original fixture,
Java adapter and full observation. Explicit signed-32-bit wrapping metrics close
the sequence MIN/MAX break and range MIN/MAX length differences: sequences now
agree in 65/65 cases (4,692 states), ranges in 66/197 cases (11,583 states). All 197
native range cases also pass independent modular calculations. The 131 remaining
range differences stay visible. Complete exact-default controls still execute:
64 ordinary sequence agreements plus its separate overflow difference; ranges
65/197 agreements with 132 differing cases. Exact i32/i64 metrics remain default.
Seven original Rust regressions add no upstream-method credit.

Only aggregate-performance was refreshed in 281, with all original fixtures,
settings and full observations. The other 17 raw reports keep original bytes,
dates and keys; the 280 stream comparison now remains visibly stale under the
changed shared collector implementation. One of 18 measurement families is current,
17 remain stale. No old result is rekeyed. Frontend and consumed report contracts
are unchanged; reuse the 280 browser proof with current data/freshness checks.

Primitive integer frequencies now use one shared allocation per node, with the
integer key/count stored directly in it. The private AVL preserves multiplicities,
copy-on-write snapshots, logarithmic path copies and singleton node reuse. Inline
promotion copies counts directly and demotion needs no traversal allocation.
Public checked/wrapping arithmetic, captured membership and defaults are unchanged.
Eight original tests replace five obsolete helper-internal tests; no upstream
method credit is added.

Actual canonical four-fork comparisons measure **15.01% lower checked-sum pair time
at 64 captures and 14.57% at 512**, improving in every fork. Every pair includes
update, restore and both complete observations. All 96 original controls and timed
outputs/checksums agree. The separate fixed-artifact 16-fork set follow-up retains
a tradeoff: small/large set medians are 1.00%/3.83% higher, with mixed individual
forks. No general collector, memory-total or whole-solve gain is claimed. The
fresh Java comparison preserves all original inputs, limits and observations:

| Collector / captures | Rust µs / pair | Java µs / pair |
| --- | ---: | ---: |
| Checked sum · 64 captures | 0.328 | 0.393 |
| Checked sum · 512 captures | 0.502 | 0.391 |
| Sorted set · 64 captures | 2.770 | 1.563 |
| Sorted set · 512 captures | 6.781 | 6.571 |
| Merged map · 64 captures | 3.562 | 8.880 |
| Merged map · 512 captures | 6.691 | 12.601 |

The small sum and both merged-map cases meet measured Java parity. The larger
sum and both sets still miss it. Cross-run Java ratios vary and must not be
attributed solely to this change. Existing default exact and explicit wrapping
collector policies remain unchanged. The earlier 279 native sum gain remains
separate evidence for that older source, with its original binding.

Last measured larger mixed workloads (261; now stale): warm Stream **28.520 ms vs Java
22.606 ms**, all score [0,-338]. Cold Stream 27.043 ms vs Java
21.812 ms, all score [0,-60]. The larger reference [0,-72] is a
feasible certificate, not an optimum. The target remains Java median-time parity
or better at equal-or-better quality. These shared-host times vary with load;
controlled mixed-workload comparisons show no meaningful warm Stream gain.

The dashboard's configurable progress index is **84.7%**: feature families 40%,
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
