# Third-party notices

## Timefold Solver Community reference

Source: https://github.com/TimefoldAI/timefold-solver
Version: 2.7.0
Commit: 8b815bc6b5f3eb81feedc94dea0885a93b267762
Location: `upstream/timefold-solver` (Git submodule)
License: Apache-2.0; retained in `upstream/timefold-solver/LICENSE.txt`.

Copyright and attribution notices remain in the upstream checkout. Timefold's
legal notice describes its origins in OptaPlanner and OptaPy, including Red Hat,
affiliates, and contributors. Consult the pinned source for exact applicable
notices. No tracked NOTICE-named file was found at this revision; reassess on updates.

The following test files adapt the pinned Community tests:

- `crates/solver/tests/simple_score.rs` from `SimpleScoreTest.java`.
- `crates/solver/tests/hard_soft_score.rs` from `HardSoftScoreTest.java`.
- `crates/solver/tests/hard_medium_soft_score.rs` from `HardMediumSoftScoreTest.java`.
- `crates/solver/tests/basic_moves.rs` from `ChangeMoveTest.java`, `SwapMoveTest.java`,
  and initial states from the `TestdataSolution.generateSolution` test fixture.

- `crates/solver/tests/neighborhoods.rs` from `ChangeMoveProviderTest.java` and
  `SwapMoveProviderTest.java`, with initial states from `TestdataSolution`,
  `TestdataPinnedSolution` and `TestdataAllowsUnassignedSolution` test fixtures.
- `crates/solver/tests/hill_climbing_acceptor.rs` from `HillClimbingAcceptorTest.java`.
- `crates/solver/tests/late_acceptance.rs` from `LateAcceptanceAcceptorTest.java`.
- `crates/solver/tests/entity_tabu.rs` from `EntityTabuAcceptorTest.java`.
- `crates/solver/tests/construction_forager.rs` from
  `DefaultConstructionHeuristicForagerTest.java`.

`crates/solver/tests/common/mod.rs` additionally adapts the equality, hash and
ordering assertion behavior of the upstream test utility `PlannerAssert.java`;
its provenance is recorded separately as a fixture, not an executable test.

All 77 source methods are recorded individually in `compat/provenance.json`.
Changes include Rust constructors, operators, assertions, and equality/hash checks.
Adaptations use test sources only; move tests replace Java descriptors with stable
indices and MoveTester with native transactions, preserving state assertions.
The same-value swap observes two native journal writes instead of two mock calls. Their Apache-2.0 license is retained
at `compat/licenses/timefold-Apache-2.0.txt`. The upstream test files have no
individual copyright headers. The applicable upstream README attribution is:

> Timefold Solver is a derivative work of OptaPlanner and OptaPy,
> which includes copyrights of the original creator, Red Hat Inc., affiliates, and contributors,
> that were all entirely licensed under the Apache-2.0 license.
> Every source file has been modified.

The production Rust code is independently implemented from observable tests and
public API behavior. Java development adapters use the pinned Community artifact;
it is not linked into the Rust library.

This project is independent of and not endorsed by Timefold.

## Barlow dashboard heading font

`dashboard/assets/Barlow-SemiBold.ttf` is Barlow SemiBold, unmodified, obtained
from the Google Fonts `ofl/barlow` directory. Copyright 2017 The Barlow Project
Authors (https://github.com/jpt/barlow). Licensed under SIL Open Font License 1.1;
the full notice and license are retained at `dashboard/assets/Barlow-OFL.txt`.
Font source: https://github.com/google/fonts/tree/main/ofl/barlow.
