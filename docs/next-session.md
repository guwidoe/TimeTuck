# TimeTuck implementation handoff

## Active user goal

Rewrite Timefold behavior in independent native Rust, using pinned Community tests
as specifications. Also maintain a visual dashboard of coverage by subsystem and
performance versus Java. The whole rewrite remains unfinished; do not mark the
active goal complete after one slice. No zero-mistake guarantee is possible.

## Current implementation

- Rust library has SimpleScore, HardSoftScore, HardMediumSoftScore with structural
  values, ordering/feasibility, parsing/formatting, arithmetic and checked arithmetic.
- 70 fully adapted source test methods (47 score + 10 move + 8 provider + 5 acceptor), recorded in
  `compat/provenance.json`.
- Finite domains, initial values, pinning, optional values and reversible
  change/swap/composite transactions. Read `docs/planning-contract.md`.
- Iterative exact search with explicit evaluation budgets and truthful
  exhaustion/optimality reporting. Original `tiny_schedule` example.
- Scoped incremental score networks and atomic scored-move transactions.
- Lazy change/swap neighborhoods and deterministic first-accepted local search,
  strict-improvement, hill-climbing or late acceptance; explicit evaluation/step
  limits and original or seeded shuffled-variable ordering.
  Read `docs/incremental-search-contract.md` before modifying this layer.
- 45 original regression methods plus one public API documentation test.
- `graph_coloring` example solves 100 variables using 400 scoped constraints.
- Public-API Java oracle: 5,409 generated comparisons pass with seed 20261001.
- 64 original complete-planning cases agree across Java, Rust and independent
  Python enumeration; movable initial values reset before both solves.
- Separate score microbenchmarks and two small matched exact-search workloads.
  The latter compare BRUTE_FORCE/ORIGINAL_ORDER Java with Rust finite enumeration.
  No heuristic, incremental-scoring or large-instance performance claim.
- Local visual dashboard: `python3 scripts/dashboard_server.py`, port 8765.
  Server was restarted after the reporting-code update and may still be running.
  Reuse it; restart after changing imported fingerprint/reporting Python code.
- Coverage denominator: 4,855 statically discovered annotated Java methods, not
  expanded runtime JUnit invocations. 70 passed; 282 JVM integration exclusions.
  Everything else is pending. `docs/dashboard.md` explains caveats and commands.
- Pinned upstream remains unmodified at 8b815bc6b5f3eb81feedc94dea0885a93b267762.

Read `docs/score-contract.md` before changing scores. The Java oracle showed that
score arithmetic CLEARS structural values, constructors retain positive values,
and full strings suppress positive structural prefixes. Do not reintroduce older
initialization-score assumptions. No production implementation has been ported.

## Next substantive work

1. Extend native exact-search/model coverage to more applicable upstream tests.
   Java trace/descriptor-specific assertions must not be credited by looser tests.
2. Expand the Java oracle to optional variables and entity-specific ranges using
   public APIs. Current complete-planning comparisons use homogeneous ranges/pins.
3. Add score-aware construction and more capable metaheuristics (tabu and
   annealing) with deterministic seeds, limits, quality checks and applicable
   upstream acceptance tests. Keep exact Java trace tests pending when not preserved.
4. Add matched Java local-search quality/performance evidence; current benchmarks
   still exercise only score operations and exact search. Scoped callback counts in
   the graph example are not runtime benchmarks.
5. Extend score coverage to Bendable and exact decimal families, inspecting tests
   and public documentation only and preserving all test provenance.

Run `bash scripts/check.sh` after Rust changes. It records all-target results and
regenerates coverage if the submodule is present. Java comparisons and benchmarks
are separate explicit commands; refresh them when measured sources change, then
regenerate the report using the captured log. Never treat a stale or skipped case
as passing. `scripts/adapt_score_tests.py` regenerates only the first three test
families and preserves other provenance entries; its deliberately limited parser
fails on unfamiliar assertions.

All crates remain unpublished. The user requested occasional commits; checkpoint
verified milestones with Conventional Commits after inspecting staged changes.
Never amend or rewrite commits. No remote publication has been requested.

Latest verification: `scripts/check.sh` passed (115 Rust integration methods,
one doctest, eight Python regressions). Incremental-scoring, neighborhood, acceptance and local-search regressions
also passed in release mode. Desktop 1440px and mobile 390px browser QA passed,
including move filtering and focus preservation on refresh. Current saved Java
evidence includes 5,409 score cases and 64 complete-planning cases. The benchmark
ratios are workload-specific and noisy; retain raw sample ranges and scope labels.

Late-acceptance scope: history uses completed-step scores; new hard-score bests
reset every slot. Public acceptance configuration now creates a session with
`start(initial_best)`; the session owns `accepts` and `step_ended`. Seeded variable
ordering keeps value order unchanged. Negative Java history-size and Bendable
late-acceptance cases remain pending, not excluded or counted as passing.
