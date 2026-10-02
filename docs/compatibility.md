# Compatibility testing

## Target

Initial reference: Timefold Solver Community 2.7.0, exact commit recorded in
`compat/upstream.json` and the Git submodule. The target is planning/optimization
behavior implemented in native Rust. Java API, binary compatibility, reflection,
framework integration, and all Enterprise functionality are outside the initial scope.

## Why the existing suite needs an adapter

The upstream suite is Java/JUnit and exercises Java objects, internal types,
reflection, and JVM integrations. Running it unchanged tests Timefold itself,
not this Rust implementation. Its cases must be classified and adapted.

1. Inventory upstream test-source paths with `scripts/upstream_inventory.py`.
2. Select a small score-semantics test family and write an observable specification.
3. Adapt those cases into Rust tests or neutral fixtures, preserving attribution
   and recording provenance. Do not copy production implementations.
4. For differential tests, build a separate Java adapter using the pinned Community
   artifact and public APIs. Export versioned inputs/expected results as JSON.
5. Feed the same cases into a Rust adapter. Compare deterministic results exactly
   where the agreed contract permits it. The Java runtime must stay out of the
   Rust library's production dependency graph.
6. Expand to incremental scoring, move application/undo, small planning problems,
   then heuristic solving. Check incremental scores against full recomputation.

Twenty-two list-move methods additionally preserve ordered assignment, relocation,
swap and span-unassignment states and undo. Native derived owner/index/neighbor
views and exact list enumeration are described in `docs/list-planning-contract.md`.
Nineteen list-provider methods additionally preserve move membership and
exclusion assertions, including per-entity ranges. Native greedy list construction and bounded local search
are verified separately with original regressions. Java move metadata, rebase,
custom listeners, optional-list and mixed-model solver comparisons remain pending.
Original generated list-move and required-list-solving comparisons are separate
from coverage; the latter check complete valid solutions, final quality and
independently enumerated small-case optima.

The first three integer score families have 47 adapted tests; BendableScore adds
27, using native compile-time dimensions in place of Java score definitions; basic change/swap
moves add ten adapted cases; provider behavior and hill-climbing acceptance add
nine more. Four late-acceptance methods validate history behavior and hard-score
resets. Four construction-forager methods preserve early-pick decisions. Three
entity-tabu methods preserve 96 assertions for expiry and aspiration, using native
variable IDs as stable entity keys. Two annealing methods preserve all 12 scalar
acceptance decisions and negative-temperature rejection. Native multi-level
annealing energy and seeded traces are not covered by those upstream cases. A
fifth late-acceptance method checks BendableScore history resets with nine
assertions. See
`docs/incremental-search-contract.md` for native incremental scoring
and local-search evidence. Original finite-domain and exact-search regression
tests are kept separate from upstream coverage. See `docs/planning-contract.md`. Separate Java
and Rust development adapters compare generated operations through a tab-delimited
line protocol; versioned JSON records the inputs and results. See
`docs/score-contract.md` for the current contract. The unchanged upstream JUnit
suite has not been run against Rust, and whole-solver compatibility is not claimed.

Five integer collector methods additionally preserve every count, distinct-count,
sum, minimum and maximum assertion across duplicate insertions and retractions.
Original stream regressions verify snapshot joins, groups and solver scoring; they
do not grant credit for upstream incremental stream-graph methods. See
`docs/constraint-stream-contract.md` for this distinction.

## Case records and coverage

Every adapted/copied case belongs in `compat/provenance.json`. Include a stable ID,
source commit/path/test method, local path, license, preserved notice paths, and a
plain description of changes. Record pending and excluded cases with reasons.
Implemented cases live in the provenance ledger. Pending methods and explicit
exclusions are listed in the generated dashboard report with reasons. Passing
a selected family never implies that the full upstream suite passes.

The inventory lists all tracked Java files under `src/test/java`; that includes
fixtures, base classes, and utilities, not only executable tests. Do not use its
file count as a count of tests or coverage denominator.

For heuristic search, compare feasibility, independently recomputed scores, and
agreed quality thresholds. Equal random seeds across JVM and Rust do not guarantee
identical PRNG streams or search traces. Record the seed, algorithm/configuration,
termination limits, reference revision, and environment with every result.
