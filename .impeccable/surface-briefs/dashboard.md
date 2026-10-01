# Dashboard surface brief

Primary target: `dashboard/index.html`. Related targets: `dashboard/styles.css`, `dashboard/app.js`.

This brief records the delivered implementation. The user asked for a visual overview of passing rewrite tests by subsystem and performance against Timefold. Layout, palette, and typography are implementation decisions; they are not recorded as separate user approvals. No image comp was used.

## Direction contract

**THESIS:** Make rewrite progress visible as a measured evidence board, with the unfinished scope as legible as the passing subset.

**OWN-WORLD:** A dark laboratory progress board with blue-gray tonal surfaces, mint evidence marks, fine rectangular bars and method squares, Barlow headings, system UI controls, and monospace test identifiers.

**STORY:** Read total passing coverage, compare subsystems, select one to inspect individual methods, then examine heuristic solution quality beside timing, the independently measured exact-search and score workloads, and their reproducibility notes.

**FIRST VIEWPORT:** A compact masthead places reload at the right. The page title and report timestamp precede a wide aggregate panel. Below, subsystem rows sit to the left of a sticky method map and readable test browser. On narrow screens, these regions stack and choosing a subsystem brings its details into view.

**FORM:** Code-led laboratory progress board. No randomized form position or seed key was supplied to this documentation pass; neither is fabricated retrospectively.

**FINISH:** unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

## Surface-specific evidence rules

- Tracks show each state's proportion within the relevant method total; one map square represents one discovered method. Search and result selection filter the list, not the map.
- Passed, failed, pending, excluded, and unverified are explicit states. The denominator retains pending and excluded methods; runtime JUnit invocations are not represented as a method count.
- Score microbenchmarks are separate from coverage. Ratios are Java median divided by Rust median. Show runtime names, nanoseconds, observed ranges, near-1× noise language, matched-checksum context, freshness, and workload limitations.
- A separate heuristic section measures four millisecond workloads: hill climbing and tabu search, each with 32 and 96 variables. Each article keeps actual Rust/Java hard-soft scores, feasibility, known optimum, score-call counts, observed ranges, and median timing together. Both engines recalculate the full score with a limit of 50 completed steps; these are bounded full-scoring runs, not incremental-scoring measurements. Equal final scores do not imply optimality. The current 32-variable runs reach 0hard/0soft; the 96-variable runs remain infeasible at -44hard/-22soft in both engines.
- A separate millisecond section measures two small complete exact-search workloads. Keep its scope, setup costs and algorithm differences visible; never imply a general solver speedup. Large-instance performance and memory remain unmeasured. Original generated differential cases are separate evidence and do not inflate upstream coverage.
- The evidence section reports 64 independent original heuristic checks separately from adapted upstream methods: 64 valid, non-regressing results and equal Java/Rust scores, with independently proven optima reached in 56 cases by each engine. This contributes no additional upstream coverage credit.
- Report refresh is every 15 seconds, preserves relevant focus, and reports missing or stale evidence. A failed fetch leaves the last report visible.

## Finish record

The parent task reports that the finish reviewer resolved three fixes and marked this scope ready to ship. This documentation pass extracted the final HTML, CSS, and JavaScript; it did not run another review. Root `DESIGN.md` and `.impeccable/design.json` record the delivered visual system. No shipping raster art was introduced; review screenshots are evidence captures.

The exact-search increment added a separate millisecond comparison section and
complete-planning differential evidence while preserving the existing design.
A bounded desktop/mobile browser pass verified rendering, no horizontal overflow
or console errors, method filtering and focus preservation on reload. The prior
external finish verdict above covers its original three fixes only; this increment
was checked by the implementation agent.

The subsequent heuristic extension adds the four workloads and independent checks
described above. The parent task reports a fresh finish-review disposition of
**ship** for this extension: no blocking visual or claims defects, with readability
and measurement scope verified. This verdict covers the heuristic extension only
and does not broaden the earlier three-fix review. Current evidence captures are
`.impeccable/review/desktop.png` (1440px wide) and
`.impeccable/review/mobile.png` (390px wide); this documentation pass inspected
both alongside `dashboard/index.html`, `dashboard/app.js`, the unchanged
stylesheet, and the saved report.

This is a code-led extension of the incumbent board, with no image comp or new
design world. It reuses the existing two-column solver benchmark grid, single-column
mobile layout, mint Rust and blue Java bars, Barlow headings, system UI labels,
tabular timing numerals, fine dividers, and scope tags. The existing palette,
12px metadata, 28px desktop grid gap, and 760px stacking breakpoint agree with
the incumbent tokens and documentation. `DESIGN.md` and `.impeccable/design.json`
remain unchanged. No token drift was found; the sidecar's provenance still
describes its earlier extraction pass and has no heuristic-specific component
example. That documentation scope difference is recorded here, not repaired by
regenerating the visual system.
