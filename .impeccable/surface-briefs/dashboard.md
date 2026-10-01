# Dashboard surface brief

Primary target: `dashboard/index.html`. Related targets: `dashboard/styles.css`, `dashboard/app.js`.

This brief records the delivered implementation. The user asked for a visual overview of passing rewrite tests by subsystem and performance against Timefold. Layout, palette, and typography are implementation decisions; they are not recorded as separate user approvals. No image comp was used.

## Direction contract

**THESIS:** Make rewrite progress visible as a measured evidence board, with the unfinished scope as legible as the passing subset.

**OWN-WORLD:** A dark laboratory progress board with blue-gray tonal surfaces, mint evidence marks, fine rectangular bars and method squares, Barlow headings, system UI controls, and monospace test identifiers.

**STORY:** Read total passing coverage, compare subsystems, select one to inspect individual methods, then examine the independently measured exact-search and score workloads and their reproducibility notes.

**FIRST VIEWPORT:** A compact masthead places reload at the right. The page title and report timestamp precede a wide aggregate panel. Below, subsystem rows sit to the left of a sticky method map and readable test browser. On narrow screens, these regions stack and choosing a subsystem brings its details into view.

**FORM:** Code-led laboratory progress board. No randomized form position or seed key was supplied to this documentation pass; neither is fabricated retrospectively.

**FINISH:** unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

## Surface-specific evidence rules

- Tracks show each state's proportion within the relevant method total; one map square represents one discovered method. Search and result selection filter the list, not the map.
- Passed, failed, pending, excluded, and unverified are explicit states. The denominator retains pending and excluded methods; runtime JUnit invocations are not represented as a method count.
- Score microbenchmarks are separate from coverage. Ratios are Java median divided by Rust median. Show runtime names, nanoseconds, observed ranges, near-1× noise language, matched-checksum context, freshness, and workload limitations.
- A separate millisecond section measures two small complete exact-search workloads. Keep its scope, setup costs and algorithm differences visible; never imply a general solver speedup. Large-instance/heuristic speed and memory are unmeasured. Original generated differential cases are separate evidence and do not inflate upstream coverage.
- Report refresh is every 15 seconds, preserves relevant focus, and reports missing or stale evidence. A failed fetch leaves the last report visible.

## Finish record

The parent task reports that the finish reviewer resolved three fixes and marked this scope ready to ship. This documentation pass extracted the final HTML, CSS, and JavaScript; it did not run another review. Root `DESIGN.md` and `.impeccable/design.json` record the delivered visual system. No shipping raster art was introduced; review screenshots are evidence captures.

The exact-search increment added a separate millisecond comparison section and
complete-planning differential evidence while preserving the existing design.
A bounded desktop/mobile browser pass verified rendering, no horizontal overflow
or console errors, method filtering and focus preservation on reload. The prior
external finish verdict above covers its original three fixes only; this increment
was checked by the implementation agent. Current captures are in ignored
`.impeccable/review/desktop.png` and `mobile.png`.
