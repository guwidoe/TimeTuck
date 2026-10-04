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

- `crates/solver/tests/concurrent_cases.rs` adapts all seven methods in
  `ConcurrentValuesShadowVariableTest`. Its reusable `concurrent_support` example
  module adapts the concurrent domain, suppliers and expected-score fixtures.
  `compat/oracle/ConcurrentOracle.java` adapts the public annotated domain and
  scoring fixtures; `ConcurrentOriginalBoundary.java` is an original diagnostic
  driver compiling those pinned fixtures unchanged. Generated inputs are original;
  all fixture use is recorded in provenance. Original `ConcurrentBenchmark.java`
  and `concurrent_benchmark.rs` measurement drivers reuse these attributed fixtures.

- `crates/solver/tests/list_element_cases.rs` adapts all six methods in
  `ListElementShadowVariableTest`. Its reusable
  `crates/solver/examples/list_element_support/mod.rs` fixture and
  `compat/oracle/ListElementOracle.java` adapt the seven `list_element` domain,
  supplier, scoring and solution fixtures as recorded in provenance. Generated
  inputs in `scripts/list_element_checks.py` are original.

- `crates/solver/tests/follower_set_cases.rs` adapts both methods in
  `FollowerSetValuesShadowVariableTest`. Its reusable
  `examples/collection_follower_support/mod.rs` fixture and
  `compat/oracle/CollectionFollowerOracle.java` adapt the collection follower
  supplier/domain and existing follower interface/scoring fixtures. Seeded inputs
  in `scripts/collection_follower_checks.py` are original; see provenance for
  inherited value identity/code and solution-generator attribution.

- `crates/solver/tests/dynamic_follower_cases.rs` adapts all three methods in
  `DynamicFollowerValuesShadowVariableTest` and its five `dynamic_follower`
  fixtures. `compat/oracle/FollowerOracle.java` and
  `crates/solver/examples/follower_oracle.rs` adapt both dynamic and fixed follower
  domain/scoring fixtures for generated public-API comparisons. One boundary in
  `scripts/follower_checks.py` retains the static testMove fixture; other generated
  inputs are original and earn no upstream method credit.

- `crates/solver/tests/follower_shadow_cases.rs` adapts all three methods in
  `FollowerValuesShadowVariableTest` and the five `testdomain/shadow/follower`
  fixtures, including aligned supplier counts and the seeded solution generator.

- `crates/solver/tests/dependency_schedule_cases.rs` adapts all three methods in
  `DependencyValuesShadowVariableTest` and its four `testdomain/shadow/dependency`
  fixtures. The fixed-cycle diagnostic prefix in `crates/solver/src/dependency_graph.rs`
  follows that test contract; the graph algorithms are independently implemented.

- `crates/solver/tests/basic_reference_cases.rs` adapts the complete basic inverse
  shadow propagation method from `BasicInverseShadowVariableTest`, with the
  entity, group, owner and solution fixtures in `testdomain/shadow/declarative/basicinverse`.

- `crates/solver/tests/pillar_schema_cases.rs` adapts eight complete whole-pillar
  swap provider methods and three multivariable subpillar swap provider methods,
  from `PillarSwapMoveProviderTest` and `SubPillarSwapMoveProviderTest`, with their
  basic, multivariable, per-entity-range, optional and pinned testdomain fixtures.

- `crates/solver/tests/subpillar_cases.rs` adapts thirteen membership, diversity,
  pinning, null-target and selection-size methods from `SubPillarChangeMoveProviderTest`,
  `SubPillarUnassignMoveProviderTest` and `SubPillarSwapMoveProviderTest`, with
  their inline basic/optional/pinned testdomain fixtures.

- `crates/solver/tests/pillar_cases.rs` adapts four complete swap/undo methods
  from `PillarSwapMoveTest.java` and two range/execution methods from
  `SelectorBasedPillarChangeMoveTest.java`, with their variable-selection helpers
  and basic, multivariable and optional entity-local-range testdomain fixtures.

- `crates/solver/tests/construction_optional_cases.rs` adapts two optional-variable
  methods from `DefaultConstructionHeuristicPhaseTest.java`, with model and scoring
  fixtures from `testdomain/unassignedvar` and `testdomain/list/unassignedvar`.

- `crates/solver/tests/solver_manager_cases.rs` adapts thirty-one parallel-solving,
  job-status, duplicate-ID, callback, initialization-event, live-change, cancellation, statistics, thread-factory, job-builder, config-override, batch-consumer, skip-ahead and error-handler methods from `SolverManagerTest.java`, with fixtures
  from that test, `PlannerTestUtils.java`, `PlannerAssert.java` and
  `DummySimpleScoreEasyScoreCalculator.java`, and the optional-list model/scorer
  fixtures in `testdomain/list/unassignedvar`, plus required-variable `TestdataSolution`,
  `TestdataEntity` and `TestdataConstraintProvider` fixtures in `testdomain`.

- `crates/solver/tests/great_deluge.rs` preserves all three methods from
  `GreatDelugeAcceptorTest.java`, with initialized score fixtures from
  `AbstractAcceptorTest.java`.

- `crates/solver/tests/step_counting_hill_climbing.rs` preserves three step-mode
  methods and the zero-size check from `StepCountingHillClimbingAcceptorTest.java`,
  with initialized score fixtures from `AbstractAcceptorTest.java`. The signed
  negative-size method remains pending; native periods use u64.

- `crates/solver/tests/membership_join_cases.rs` from eight complete deterministic
  size, membership and key-reuse methods (and their inline fixtures) in
  `ContainingIndexerTest.java`, `ContainingAnyOfIndexerTest.java` and
  `ContainedInIndexerTest.java`.

- `crates/solver/tests/count_limit_cases.rs` from all five methods in
  `MoveCountTerminationTest.java` and `ScoreCalculationCountTerminationTest.java`.

- `crates/solver/tests/two_opt_provider_cases.rs` from twelve complete methods in
  `ListTailSwapMoveProviderTest.java` and `TwoOptListMoveProviderTest.java`, with
  their inline/two-entity fixtures and `TestdataListEntityProvidingSolution` ranges.

- `crates/solver/tests/sublist_swap_cases.rs` from five complete state, undo and
  entity-membership methods in `SubListSwapMoveTest.java`.

- `crates/solver/tests/sublist_change_cases.rs` from seven state, undo and entity-
  membership methods in `SubListChangeMoveTest.java`. The forward/backward fixture
  is also reused in `scripts/differential_lists.py`; the forward fixture is reused
  in `scripts/test_differential_lists.py`. These supporting uses are recorded as
  fixtures and grant no additional source-method credit.

- `crates/solver/tests/elapsed_time_cases.rs` from
  `TimeMillisSpentTerminationTest.java`.

- `crates/solver/tests/score_progress_cases.rs` from
  `BestScoreTerminationTest.java` and `BestScoreFeasibleTerminationTest.java`.

- `crates/solver/tests/step_termination_cases.rs` from
  `StepCountTerminationTest.java` and `UnimprovedStepCountTerminationTest.java`.

- `crates/solver/tests/simple_score.rs` from `SimpleScoreTest.java`.
- `crates/solver/tests/simple_decimal_score.rs`, `hard_soft_decimal_score.rs`, and
  `hard_medium_soft_decimal_score.rs` from their corresponding
  `SimpleBigDecimalScoreTest.java`, `HardSoftBigDecimalScoreTest.java`, and
  `HardMediumSoftBigDecimalScoreTest.java` classes. All assertions use native
  Decimal literals/factors, integer powers, constructors, operators and Rust
  equality/hash checks; full method mappings are in `compat/provenance.json`.
- `crates/solver/tests/bendable_decimal_score.rs` from
  `BendableBigDecimalScoreTest.java`. All 26 methods retain their assertions;
  const-generic native dimensions replace score definitions, exact Decimal
  factors and u32 powers replace tested double literals, and Rust Eq/Ord/Hash
  helpers replace Java assertion plumbing. See `compat/provenance.json`.
- `crates/solver/tests/hard_soft_score.rs` from `HardSoftScoreTest.java`.
- `crates/solver/tests/hard_medium_soft_score.rs` from `HardMediumSoftScoreTest.java`.
- `crates/solver/tests/bendable_score.rs` from `BendableScoreTest.java`, including
  inlined feasibility assertions from `AbstractScoreTest.java`.
- `crates/solver/tests/basic_moves.rs` from `ChangeMoveTest.java`, `SwapMoveTest.java`,
  and initial states from the `TestdataSolution.generateSolution` test fixture.

- `crates/solver/tests/list_moves.rs` from `ListAssignMoveTest.java`,
  `ListSwapMoveTest.java`, nine state methods in `ListChangeMoveTest.java`,
  two state methods in `SubListUnassignMoveTest.java`, and initial states from
  the `TestdataListSolution` test fixture.

- `crates/solver/tests/neighborhoods.rs` from `ChangeMoveProviderTest.java` and
  `SwapMoveProviderTest.java`, with initial states from `TestdataSolution`,
  `TestdataPinnedSolution` and `TestdataAllowsUnassignedSolution` test fixtures.
- `crates/solver/tests/list_neighborhoods.rs` from `ListAssignMoveProviderTest.java`,
  `ListChangeMoveProviderTest.java`, `ListSwapMoveProviderTest.java` and
  `ListUnassignMoveProviderTest.java`, with the list, pinned-list, optional-list
  and pinned-optional-list solution fixtures recorded in the provenance ledger.
  Entity-range cases additionally use `TestdataListEntityProvidingSolution`,
  `TestdataListUnassignedEntityProvidingSolution` and their entity fixtures.
- `crates/solver/tests/hill_climbing_acceptor.rs` from `HillClimbingAcceptorTest.java`.
- `crates/solver/tests/late_acceptance.rs` from `LateAcceptanceAcceptorTest.java`.
- `crates/solver/tests/simulated_annealing.rs` from `SimulatedAnnealingAcceptorTest.java`.
- `crates/solver/tests/entity_tabu.rs` from `EntityTabuAcceptorTest.java`.
- `crates/solver/tests/construction_forager.rs` from
  `DefaultConstructionHeuristicForagerTest.java`.

- `crates/solver/tests/stream_collectors.rs` from five count/sum/extrema methods
  in `InnerUniConstraintCollectorsTest.java`.

- `crates/solver/tests/incremental_stream_cases.rs` from seven methods in
  `AbstractUniConstraintStreamTest.java`, with initial states from
  `TestdataLavishSolution.java` and score/match assertions from
  `AbstractConstraintStreamTest.java`.

- `crates/solver/tests/stream_graph_cases.rs` from `join_1_mirrored` and
  `join_2Equal` in `AbstractUniConstraintStreamTest.java`, with initial states
  from `TestdataLavishSolution.java` and score/match assertions from
  `AbstractConstraintStreamTest.java`.

- `crates/solver/tests/existence_stream_cases.rs` from thirteen existence methods
  in `AbstractUniConstraintStreamTest.java`, with fixture states from
  `TestdataLavishSolution.java` and match assertions from
  `AbstractConstraintStreamTest.java`.

- `crates/solver/tests/distinct_stream_cases.rs` from twenty mapping/distinct methods
  in `AbstractUniConstraintStreamTest.java`, `AbstractBiConstraintStreamTest.java`,
  `AbstractTriConstraintStreamTest.java` and `AbstractQuadConstraintStreamTest.java`.
  Fixtures and assertion helpers come from `TestdataLavishSolution.java` and
  `AbstractConstraintStreamTest.java`.

- `crates/solver/tests/collecting_stream_cases.rs` from sixteen global aggregation
  methods in the uni/bi/tri/quad `Abstract*ConstraintStreamTest.java` test classes.
  Fixtures and assertion helpers come from `TestdataLavishSolution.java` and
  `AbstractConstraintStreamTest.java`.

- `crates/solver/tests/list_stream_cases.rs` from four list-value matching methods
  in `AbstractUniConstraintStreamTest.java`, with entity fixtures from
  `TestdataListEntity.java` and `TestdataAllowsUnassignedValuesListEntity.java`,
  and assertions from `AbstractConstraintStreamTest.java`.

- `crates/solver/tests/map_collector_cases.rs` from 25 map, sorted-map, merged-sum,
  duplicate and update methods in the uni/bi/tri/quad
  `Inner*ConstraintCollectorsTest.java` classes.

- `crates/solver/tests/list_collector_cases.rs` from twelve list insertion and
  list/ArrayList update methods in the uni/bi/tri/quad
  `Inner*ConstraintCollectorsTest.java` classes.

- `crates/solver/tests/average_collector_cases.rs` from eight average insertion
  and update methods in the uni/bi/tri/quad
  `Inner*ConstraintCollectorsTest.java` classes.

- `crates/solver/tests/duration_collector_cases.rs` from sixteen duration sum
  and average insertion/update methods in the uni/bi/tri/quad
  `Inner*ConstraintCollectorsTest.java` classes.

- `crates/solver/tests/big_number_collector_cases.rs` from thirty-two arbitrary-precision
  integer/decimal sum and average methods in the uni/bi/tri/quad
  `Inner*ConstraintCollectorsTest.java` classes.

- `crates/solver/tests/conditional_collector_cases.rs` from all eight `conditionally`
  and `conditionallyUpdate` methods in the uni/bi/tri/quad
  `Inner*ConstraintCollectorsTest.java` classes.

`crates/solver/tests/common/mod.rs` additionally adapts the equality, hash and
ordering assertion behavior of the upstream test utility `PlannerAssert.java`;
its provenance is recorded separately as a fixture, not an executable test.

All adapted source methods are recorded individually in `compat/provenance.json`.
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


## Rust arithmetic dependencies

`num-bigint` 0.5.1 supplies arbitrary-precision integer storage and arithmetic.
Its transitive dependencies are `num-integer` 0.1.47, `num-traits` 0.2.19 and
build-time `autocfg` 1.5.1. All four declare MIT OR Apache-2.0 licensing; exact
versions and registry checksums are recorded in Cargo.lock. Their sources are
resolved by Cargo rather than copied into this repository. Native decimal and
collector logic remains independently implemented; no Timefold production code
is used. See the dependency packages for their full licenses and copyright notices.

## Dependency topology test adaptations

`crates/solver/tests/topology_cases.rs`, its permutation checksum fixture, and
`compat/oracle/TopologyPermutationOracle.java` adapt the seven methods, graph
fixtures and permutation helpers in Timefold Community
`AbstractTopologicalGraphTest`; `DefaultTopologicalGraphTest` supplies the concrete
adapter specification. These upstream tests are Apache-2.0; the pinned commit,
paths and modifications are recorded in `compat/provenance.json`. No upstream
production topology implementation is used. The test-only shuffle follows the
public Java Random/Collections API contract; it is not native solver randomness.

`crates/solver/examples/concurrent_support/cache.rs` additionally adapts the
concurrent Value, Entity and Solution test fixtures to an incremental native cache.
The original Community test-fixture license and provenance apply. Its representative
start nodes and aliases are independently implemented from those fixture semantics;
no Timefold production implementation was used.

The consecutive-sequence test adaptations in `sequence_tree_cases.rs` and
`sequence_collector_cases.rs` derive from the pinned Apache-2.0 Community
`ConsecutiveSetTreeTest`, `Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest` and
`AbstractConstraintCollectorsTest` tests/fixtures. `SequenceTraceFixture.java`
and `tests/fixtures/sequence-traces.txt` preserve both source Random(1) operation
streams. The source commit, identities, license and native semantic adjustments
are recorded in `compat/provenance.json`. Production sequence collection and the
separate public-API differential drivers were independently implemented.
