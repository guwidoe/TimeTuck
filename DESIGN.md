---
name: TimeTuck
description: A dark laboratory board for measured rewrite progress.
colors:
  bg: "#111a20"
  surface: "#17232b"
  raised: "#1d2d36"
  line: "#33444e"
  text: "#e7eeef"
  muted: "#b0c0c8"
  pass: "#85e3bd"
  pending: "#455c6a"
  fail: "#ff9b90"
  excluded: "#8a7b9c"
  excluded-text: "#c8b3df"
  unverified: "#e9c875"
  java: "#91accb"
  warning-bg: "#3a3223"
  warning-text: "#f3d69b"
typography:
  display:
    fontFamily: 'Barlow, "Segoe UI", sans-serif'
    fontSize: "34px"
    fontWeight: 600
    lineHeight: 1.5
    letterSpacing: "-0.035em"
  headline:
    fontFamily: 'Barlow, "Segoe UI", sans-serif'
    fontSize: "21px"
    fontWeight: 600
    lineHeight: 1.5
    letterSpacing: "-0.02em"
  title:
    fontFamily: 'Barlow, "Segoe UI", sans-serif'
    fontSize: "17px"
    fontWeight: 600
    lineHeight: 1.5
  body:
    fontFamily: '"Segoe UI", system-ui, sans-serif'
    fontSize: "15px"
    lineHeight: 1.5
  control:
    fontFamily: '"Segoe UI", system-ui, sans-serif'
    fontSize: "13px"
    lineHeight: 1.5
  label:
    fontFamily: '"Segoe UI", system-ui, sans-serif'
    fontSize: "12px"
    lineHeight: 1.5
  identifier:
    fontFamily: 'ui-monospace, "Cascadia Code", monospace'
    fontSize: "12px"
    lineHeight: 1.5
rounded:
  panel: "12px"
  control: "7px"
  field: "6px"
  tag: "5px"
  coverage: "3px"
  bar: "2px"
  cell: "1px"
spacing:
  cell-gap: "3px"
  inline: "8px"
  controls: "12px"
  compact: "18px"
  columns: "28px"
  section: "34px"
components:
  button-quiet:
    backgroundColor: "transparent"
    textColor: "{colors.text}"
    typography: "{typography.control}"
    rounded: "{rounded.control}"
    padding: "8px 13px"
  button-quiet-hover:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text}"
  field:
    backgroundColor: "{colors.bg}"
    textColor: "{colors.text}"
    typography: "{typography.control}"
    rounded: "{rounded.field}"
    padding: "9px 10px"
  tag:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.muted}"
    typography: "{typography.label}"
    rounded: "{rounded.tag}"
    padding: "5px 9px"
  detail-panel:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.panel}"
    padding: "22px 23px"
  family-selected:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text}"
    rounded: "{rounded.control}"
    padding: "13px 16px"
---

# Design System: TimeTuck

## Overview

**Creative North Star: "The Laboratory Progress Board"**

TimeTuck presents measured progress through restrained dark surfaces, fine dividers, compact labels, and mint evidence marks. Its character comes from the data's geometry: segmented tracks and individual method squares make both completed and remaining work visible.

This document records the implemented dashboard, not an independently approved brand identity. The descriptive north star and palette names are documentation choices. The build uses self-hosted Barlow headings, system UI controls, and monospace identifiers; no photographic or generated image composition underlies the interface. Route-specific reading order and measurement caveats live in the dashboard surface brief.

**Key Characteristics:**
- Dark blue-gray surfaces with tonal separation.
- Compact, readable evidence with tabular numbers.
- Mint highlights paired with explicit result labels.
- Fine rectangular marks and gently rounded working panels.

## Colors

The palette resembles a quiet instrument panel: cool neutrals support a bright mint accent, while result colors retain distinct meanings.

### Primary
- **Evidence Mint** (`pass`): passing adaptations, Rust timing bars, key measured values, links, selection, and keyboard focus.

### Secondary
- **Reference Blue** (`java`): Java timing bars and their legend key.
- **Failure Coral** (`fail`), **Exclusion Mauve** (`excluded`), and **Unverified Amber** (`unverified`): state marks in coverage tracks and method maps. Excluded text uses the lighter `excluded-text` token.
- **Warning Ochre** (`warning-bg`, `warning-text`): report and benchmark messages requiring attention.

### Neutral
- **Deep Slate** (`bg`): canvas and inset fields.
- **Panel Slate** (`surface`): overview and detail surfaces; row hover.
- **Raised Slate** (`raised`): selected rows, quiet-button hover, and scope tags.
- **Divider Slate** (`line`): borders and section rules.
- **Soft White** (`text`): primary content; **Cool Gray** (`muted`): supporting text.
- **Pending Slate** (`pending`): unimplemented method marks and tracks. Pending text uses `muted` for readability.

**The Labeled Evidence Rule.** Color supports explicit state names, counts, and accessible chart descriptions; it never replaces them.

## Typography

Barlow SemiBold is self-hosted from `dashboard/assets/Barlow-SemiBold.ttf`, with `font-display: swap`. It gives headings and the wordmark a distinct face while controls remain familiar. Identifiers use the monospace stack in the frontmatter and wrap anywhere when necessary.

The desktop hierarchy runs from display to headline to title. Controls and explanatory paragraphs commonly use the control role; metadata, status, legends, and identifiers use the label scale. Paragraphs normally cap at 75 characters. The overview number uses system UI at 38px with a 1.2 line height and tabular numerals; timing ratios use 27px. Benchmark headings use Barlow at 15px. Font synthesis is disabled; the shipped Barlow asset is the 600 face, including where legacy declarations request other heading weights.

**The Numeric Alignment Rule.** Coverage counts, percentages, measured timings, and ratios use tabular numerals so comparisons remain aligned as reports change.

## Layout

The implementation shares a centered container capped at 1500px across masthead, main content, and footer. Standard horizontal padding is 48px; at a viewport of at least 1600px it becomes 55px. The desktop masthead is 92px tall.

The dashboard's coverage region uses two columns in a 1.13:1 ratio, separated by the columns spacing token. The detail panel sticks 16px from the viewport top. Subsystem rows align names, counts, and percentages in columns; their segmented bars span the row. Detail filters share a row, with a flexible search field and fixed-width result selector. The method map and method list scroll independently, capped at 180px and 325px respectively.

At 1050px and below, container padding becomes 25px, coverage columns become equal with an 18px gap, and labels, row padding, and timing columns tighten. At 760px and below, coverage, benchmarks, evidence, and benchmark notes stack; the detail panel stops sticking. The masthead becomes 76px tall, its reference label hides, the timestamp moves beneath the page title, the title becomes 29px, section headings become 20px, and the map cap becomes 150px. Choosing a subsystem scrolls its details into view. At 420px and below, gutters become 17px, controls tighten, and the result selector becomes 117px wide. Labels and identifiers remain readable at 12px; they do not shrink to fit.

These breakpoints describe the implemented surface, not a required composition for every future screen.

## Elevation & Depth

There are no box shadows. Depth comes from the canvas, panel, and raised tonal levels, with thin borders identifying rows and section boundaries. Keyboard focus uses a mint outline, 2px wide with a 4px offset. A selected subsystem has a visible slate border in addition to its raised fill.

**The Tonal Depth Rule.** Distinguish working surfaces with fill and border before introducing elevation effects.

## Shapes

The reusable panel radius is the panel token. Controls and selected rows use the control token; inset fields are slightly tighter. Tags are compact rectangles, not pills. Coverage strips, timing bars, and method squares retain small corner radii so their geometry reads as quantitative marks. The wordmark uses an inline SVG T silhouette.

## Components

### Quiet buttons

Reload and pagination share the outlined quiet-button treatment: transparent fill, text ink, line border, and a minimum height of 40px. Hover raises the fill and lightens the border. Disabled reload uses 0.55 opacity and a wait cursor. Focus uses the shared outline. There is no separate filled primary-button variant in the build.

### Inputs and result selection

Search and the native select share inset canvas fill, a line border, and a 40px minimum height. Labels stay visible above them. Placeholder text uses muted ink, the caret uses mint, and keyboard focus uses the shared outline. Filtering changes the readable method list; the method map continues to show the complete selected subsystem.

### Navigation and scope tags

The masthead wordmark links to the page top. Method class links lead to the pinned upstream source; methodology and report links expose evidence. Links are underlined where appropriate with a 4px underline offset. The raised scope tag is informational, not an interactive filter.

### Working panels

The overview and selected-detail panels share the surface fill and panel radius. The overview has more room for its aggregate number and legend; the detail panel contains the method map, filters, list, and pagination. Benchmark articles use top rules and open space rather than enclosing cards.

### Subsystem rows and method map

Each subsystem is a real button with `aria-pressed`, explicit counts, and a proportional state track. Hover changes the surface; selection adds the raised fill, border, and rounded outline. The background transition lasts 150ms with ease timing. The dense map uses one square per discovered method, a 3px gap, and columns of at least 6px; the readable list supplies identities and status names. The squares are not individual controls.

### Timing comparisons and feedback

Paired timing bars use mint for Rust and reference blue for Java, with explicit runtime names and units: milliseconds for small exact-search workloads and nanoseconds for score operations. Exact-search articles use two columns on desktop; score operations use three. Ratios, observed ranges, and measurement scope remain adjacent to their charts. Missing, stale, and failed-refresh states use text messages; the last loaded report remains visible after a refresh error. Reduced-motion preference disables transitions and smooth scrolling.

## Do's and Don'ts

### Do:
- **Do** preserve textual names and counts alongside every status color.
- **Do** keep the selected subsystem visually distinct and keyboard reachable.
- **Do** use Barlow for headings, system UI for controls, and monospace for identifiers.
- **Do** preserve focus visibility, readable metadata, and the reduced-motion behavior.
- **Do** keep scope and freshness information beside measured results.

### Don't:
- **Don't** present pending marks as passed or hide excluded methods from the coverage view.
- **Don't** use dark pending or exclusion chart colors as small text; use their readable text treatments.
- **Don't** imply that a microbenchmark ratio measures whole-solver performance.
- **Don't** turn method squares into the sole way to inspect a result.

## Native readiness and strict comparisons

The native readiness panel precedes the strict comparison index. It uses the
existing surface fill, mint/amber status text and a three-column desktop evidence
summary, becoming two columns on mobile. Counts describe original capability
requirements, measured Java-parity workloads and current comparison jobs.
Capability proofs and accepted native contracts expand below; raw Java agreements,
errors, unsupported cases and pending methods retain their original accounting.
Missing or stale evidence never displays readiness. The strict comparison panel
keeps its existing geometry, formula and percentages.

## Test origins and additional application workloads

The test-origin table separates mapped upstream adaptations, additional native
regressions and declared external application checks. Counts are recorded native
identities, while the Community coverage region retains its source-method unit.
RealWorld follows in the same open section with individual test results, fixture
status and a separate throughput table. Existing mint/amber result labels convey
met/open targets. Horizontal table overflow stays inside keyboard-focusable
regions on narrow screens. The native overview explicitly refers to original
targets; application speed gaps do not become hidden by that readiness result.
