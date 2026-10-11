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

- `crates/solver/tests/indexed_int_array_cases.rs` adapts all twenty nested methods
  in `DynamicIntArrayTest`, including all negative-index parameter values.
  The additional sparse-map reference regression is original. Native typed errors
  replace Java exceptions; complete attribution is in `compat/provenance.json`.

- `crates/solver/tests/brute_force_phase_cases.rs` adapts four complete methods
  from `BruteForceTest`, covering required/optional basic and list variables, and
  their domain and easy-score fixtures. Native traversal preserves the source's
  final assignment, score and lifecycle assertions; substitutions are in provenance.

- `crates/solver/tests/optional_list_union_solver_cases.rs` adapts the complete
  `AllowsUnassignedValuesListVariableSolverTest.runSolver` parameterized method,
  all six move-family configurations and its optional-list test-domain fixtures
  and easy scorer. Native configuration/sampling substitutions are recorded in
  `compat/provenance.json`; the additional partially assigned scenario is original.

- `crates/solver/tests/consumer_support_cases.rs` adapts all four
  `ConsumerSupportTest` methods, its problem-change/publication helpers, and the
  initialized `TestdataSolution.generateSolution` fixture. Native manager callbacks,
  revision receipts and Arc identity replace Java consumer/future/domain objects.
  Exact source identities and substitutions are in `compat/provenance.json`.

- `crates/solver/tests/benchmark_ranking_cases.rs` adapts all eight
  `TotalRankSolverRankingWeightFactoryTest` methods and both individual-result
  comparator methods from the Community benchmark tool. Score and shared-problem
  setup adapts `AbstractSolverRankingComparatorTest`. Native snapshots are rebuilt
  after score/group updates instead of retaining live Java weight objects.
  Complete source identities and modifications are in `compat/provenance.json`.

- `crates/solver/tests/benchmark_results_cases.rs` adapts all methods from
  `TotalScoreSolverRankingComparatorTest`, `WorstScoreSolverRankingComparatorTest`,
  `ResilientScoreComparatorTest`, and `ScoreDifferencePercentageTest` in the
  Community benchmark tool, plus score/initialization setup from
  `AbstractSolverRankingComparatorTest`. Native owned summaries replace Java
  report objects; unasserted helper metadata is omitted. The source method named
  `divide` actually calls multiplication, and those assertions remain unchanged.
  Exact upstream paths, methods, and modifications are in `compat/provenance.json`.

- `crates/solver/tests/unique_sampling_cases.rs` adapts five methods from
  `DefaultUniqueRandomIteratorTest` and `DefaultRepeatingRandomIteratorTest`,
  plus `AbstractIndexerTest.toEntries`. Native iterators preserve occurrence
  membership, bounded repeating draws and exhaustion; Java PRNG traces differ.

- `crates/solver/tests/placement_queue_cases.rs` adapts five methods from
  `QueuedEntityPlacerTest`, `QueuedValuePlacerTest` and `PooledEntityPlacerTest`,
  with `PlacementAssertions` and `PlannerAssert` helper assertions. Native keys,
  candidate snapshots and explicit lifecycle events replace Java selector mocks.

- `crates/solver/tests/list_ruin_recreate_cases.rs` adapts the complete
  `ListRuinRecreateMoveSelectorTest.testRuiningOnlyOneAssignedValues` method,
  its list-size penalty and the optional list solution-generator fixture. The
  rustdoc example on `ruin_recreate_list_search_from` in
  `crates/solver/src/list_ruin_recreate.rs` reuses this attributed test fixture;
  the surrounding implementation is independently written.

- `crates/solver/tests/ruin_recreate_cases.rs` adapts
  all four methods in `RuinRecreateMoveSelectorTest`, its embedded assigned-value
  penalty, required/optional solution-generator fixtures, `TestdataConstraintProvider`
  and `TestdataAllowsUnassignedEasyScoreCalculator`. See provenance for native
  work-counter, bounded-search and integer-identity substitutions.

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

Connected-range test adaptations (`range_tree_cases.rs`, `range_collector_cases.rs`)
derive from the pinned Apache-2.0 Community ConnectedRangeTrackerTest,
Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest and AbstractConstraintCollectorsTest.
RangeTraceFixture.java adapts the original randomized input-selection scaffolding
and TestRange equality/hash fixture; tests/fixtures/range-traces.txt contains its
10,000 generated operations. Source identities, revision, license and adjustments
are recorded in compat/provenance.json. Native production range collection,
independent references and public-API comparison drivers are original work.

`crates/solver/tests/load_balance_cases.rs` adapts the four pinned Apache-2.0
Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest.loadBalance methods, including the
uni LoadBalanced record's key/load fixture fields and each arity's tuple mapping.
Source commit, paths, test identities and adjustments are in compat/provenance.json.
The production collector, original regressions and public comparison drivers are
independent implementations, with no extra upstream-method credit.

`crates/solver/tests/period_collector_cases.rs` adapts sumPeriod and sumPeriodUpdate
from the four pinned Apache-2.0 Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest classes.
All original arity-specific day inputs and expected values remain; native captured
values replace Java handles. Identities, revision, license and adjustments are in
compat/provenance.json. CalendarPeriod, PeriodSumCollector, independent regressions
and public comparison drivers are original code based on tests and public contracts.

`crates/solver/tests/extrema_collector_cases.rs` adapts minNotComparable,
maxNotComparable and their Update methods from all four pinned Apache-2.0
Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest classes. String fixtures, nullable
trailing tuple fields and all asserted operation sequences remain represented.
Revision, paths, identities and modifications are in compat/provenance.json.
Native production collection, original regressions and public oracle drivers are
independently implemented from tests and public API contracts.

`crates/solver/tests/composed_collector_cases.rs` adapts compose2/3/4, compose2Update,
collectAndThen and its Update method, and natural minComparableUpdate/maxComparableUpdate
from all four pinned Apache-2.0 Inner*ConstraintCollectorsTest classes. Original
assertions and mapped tuple/date fixtures are represented as recorded in
compat/provenance.json. FinishingCollector, original regression tests, independent
references and public comparison drivers are original implementations.

`crates/solver/tests/set_sum_collector_cases.rs` adapts toSet, toSortedSet,
toSortedMapMerged, countUpdate, sumUpdate, countDistinctUpdate, toSetUpdate and
toSortedSetUpdate from all four pinned Apache-2.0 Inner*ConstraintCollectorsTest
classes. Tuple/string fixtures and the uni alternating merged-value counter retain
source attribution in compat/provenance.json. SumCollector and separate original
regressions, references and public comparison drivers were independently implemented.

`crates/solver/tests/tuple_collector_cases.rs` adapts count, countDistinct, sum,
minComparable and maxComparable from the pinned Apache-2.0 InnerBi/Tri/Quad
ConstraintCollectorsTest classes. Numeric tuple values and fixed-date extrema
fixtures retain attribution in compat/provenance.json. The aggregate benchmark,
its validation cases and references are original and add no upstream-method credit.


`crates/solver/tests/fading_tabu_cases.rs` adapts the pinned Apache-2.0
EntityTabuAcceptorTest.fadingTabuSize method. Step fixtures, supplied random draws
and expected decisions are attributed in compat/provenance.json. FadingTabuMemory,
search integration and separate reference regressions are independent implementations.


`crates/solver/tests/late_level_cases.rs` adapts six Apache-2.0
DefaultLevelScoreStateTest methods from the pinned Community revision. Complete
score fixtures and reset/no-reset expectations are recorded in compat/provenance.json.
The separate history and search regressions are original and add no source-method credit.

`crates/solver/tests/composite_acceptance_cases.rs` adapts the Apache-2.0
CompositeAcceptorTest.isAccepted method at the pinned Community revision. All five
truth configurations retain source attribution in compat/provenance.json. Native
policy composition, its state regressions and combined heuristic benchmark are
independent implementations; Java lifecycle callback equivalence is not claimed.

`crates/solver/tests/move_tabu_cases.rs` adapts all three Apache-2.0
MoveTabuAcceptorTest methods at the pinned Community revision. Identity/score/step
fixtures and supplied draws retain attribution in compat/provenance.json. Typed
MoveAcceptor, structural-identity search integration, original regressions and
public Java/native benchmark extension are independently implemented.

`crates/solver/tests/value_tabu_cases.rs` adapts all five Apache-2.0
ValueTabuAcceptorTest methods at the pinned Community revision. Repeated queries,
multi-value steps, null identity and fixed fading draws retain attribution in
compat/provenance.json. Typed value acceptance, metadata capture, search integration,
original regressions and public Java/native comparisons are independent implementations.

The private-state unit tests in crates/solver/src/diversified_late.rs adapt both
Apache-2.0 DiversifiedLateAcceptanceAcceptorTest methods at the pinned Community
revision. The Cargo library test target and fully qualified test identities are
recorded in compat/provenance.json. All score/state fixtures and assertions retain
attribution. The acceptance implementation and original regressions are independent.

crates/solver/tests/tabu_size_cases.rs adapts all three Apache-2.0 methods in
EntityRatioTabuSizeStrategyTest and FixedTabuSizeStrategyTest at the pinned
Community revision. Complete ratio/population/fixed-size fixtures and rejection
assertions are recorded in compat/provenance.json. Native sizing and phase
integration, original regressions and benchmark configuration are independent.

crates/solver/tests/move_value_cases.rs adapts six Apache-2.0 planning-value
metadata methods from MassChangeMoveTest, PillarSwapMoveTest, ListChangeMoveTest,
SubListChangeMoveTest, SubListSwapMoveTest and SubListUnassignMoveTest at the
pinned Community revision. Fixture identities, lifecycle mapping and assertions
are recorded in compat/provenance.json. Sparse native capture and its clone-cost,
reference and panic-safety regressions are independent implementations.

crates/solver/tests/list_gather_cases.rs adapts five Apache-2.0 execution and undo
methods from pinned Community MassListChangeMoveTest. Full identities and native
API substitutions are recorded in compat/provenance.json. The gather builder,
original exhaustive/scoring regressions and generated public-API comparisons are
independently implemented; equality/hash/rebase methods remain unadapted.

crates/solver/tests/forager_cases.rs adapts all seven Apache-2.0 methods in pinned
Community AcceptedLocalSearchForagerTest. Complete candidate/score/acceptance and
early-stop fixtures, supplied-index substitutions and source identities are
recorded in compat/provenance.json. Native candidate retention, search integration
and original reference/failure/owned-score regressions are independent work.

`crates/solver/tests/oscillation_cases.rs` adapts all four methods in pinned
Community `StrategicOscillationByLevelFinalistPodiumTest` (Apache-2.0). All score
fixtures, candidate order, singleton identities and reference updates are retained;
explicit native references replace mocked Java phase/move scopes. The source's
`alwaysPickImprovingMove` candidate uses soft -3, as in its executable fixture.
The independent native ranking, generated diagnostics and public Java adapter are
original work. Observed Community exact-reference ordering differences are separate
from adapted-method coverage. See `compat/provenance.json` for paths and revision.

`crates/solver/tests/score_history_cases.rs` adapts all seven Apache-2.0 methods
in pinned Community `LateAcceptanceScoreBufferTest`. Every score, operation and
slot/current assertion is retained; native ScoreHistory replaces Java wrappers.
Paths, revision and API substitutions are recorded in compat/provenance.json.
Native history, acceptance integration, generated reference and failure/cost
regressions are independently implemented.

`crates/solver/tests/node_order_cases.rs` adapts the four Apache-2.0 compare methods
from DepthFirstNodeComparatorTest, BreadthFirstNodeComparatorTest,
ScoreFirstNodeComparatorTest and OptimisticBoundFirstNodeComparatorTest, plus
AbstractNodeComparatorTest fixture defaults/assertion loops, at the pinned Community
revision. Complete fixture attribution and API substitutions appear in
compat/provenance.json. The native branch-and-bound engine, original reference/
failure tests and public-API benchmark adapters are independently implemented.

`crates/solver/tests/score_trend_cases.rs` adapts all three Apache-2.0 methods in
pinned Community `InitializingScoreTrendTest`. All ordered tokens and predicate
assertions are retained; native parsing/predicates replace Java APIs. Revision,
source paths and substitutions are recorded in `compat/provenance.json`. The
symbolic bounds, engine integration and reference regressions are original work.

`crates/solver/tests/diminished_returns_cases.rs` adapts all eight Apache-2.0
methods in pinned Community `DiminishedReturnsTerminationTest`. Score fixtures,
query sequences and assertions are retained. Native explicit time and lifecycle
methods replace Java scopes; provenance records each substitution. The native
sliding-window policy, search integration and independent regressions are original
work. No production implementation or internal ring-buffer tests were ported.

`crates/solver/tests/significant_improvement_cases.rs` adapts the two scoreImproves
methods from pinned Community UnimprovedTimeMillisSpentScoreDifferenceThresholdTerminationTest
(Apache-2.0). Every score/time fixture, repeated termination query and gradient
assertion remains. Explicit native observations replace mocked scope/clock plumbing;
paths and substitutions are in compat/provenance.json. The native threshold policy,
exact arithmetic, ignored suffixes, integration and reference tests are original.

`crates/solver/tests/list_ruin_recreate_cases.rs` now adapts all four methods in
pinned Community `ListRuinRecreateMoveSelectorTest` (Apache-2.0), its embedded
required/optional scoring, and `TestdataListSolution`'s generated-name fixture.
The attributed `tests/fixtures/list-repair-hashes.csv` records public Java hash
results for all 300 fixture positions. Provenance records native metrics, weighted
selection and attempt-limit substitutions. The native interleaving engine and
independent budget, cancellation, history and managed-statistics checks are
original work; no upstream union-selector lifecycle methods receive credit.

`crates/solver/tests/mass_list_cases.rs` adapts all ten methods in pinned Community
`MassListChangeMoveProviderTest` (Apache-2.0), its in-method fixtures, and the ranged
list fixture's constructor semantics. Every method's assertions and stream limit
are retained; native finite enumeration and candidate metadata replace the Java
sampled stream and metamodel. `compat/provenance.json` records paths, revision and
substitutions. The generator, size/null policy, search integration and independent
successor/laziness/control regressions are original work.

`crates/solver/tests/mass_change_cases.rs` adapts all ten methods of Community
`MassChangeMoveProviderTest`, its in-method fixtures and `TestdataSolution`'s
initialized fixture generator, under Apache-2.0. Provenance records the pin,
paths, native metadata/error substitutions and finite enumeration in place of
sampled streams. Independent subset references, search integration and the native
group generator are original work; no production implementation was adapted.

`crates/solver/tests/mass_assignment_cases.rs` adapts all three methods in Community
`MassAssignMoveProviderTest` and all four in `MassUnassignMoveProviderTest`, their
in-method fixtures, and `TestdataAllowsUnassignedSolution.generateSolution(3,5)`.
They remain Apache-2.0 material. Provenance records the exact revision, paths and
native substitutions, including bounded seeded sampling preserving the fifty-draw
singleton assertion. Native generators and independent regressions are original.

`crates/solver/tests/list_mass_assignment_cases.rs` adapts all five methods in each
of Community `MassListAssignMoveProviderTest` and `MassListUnassignMoveProviderTest`,
their in-method fixtures, and the ranged-list fixture's empty-list constructor.
Apache-2.0 notices and precise paths/revision/substitutions are in provenance.
The singleton unassignment assertion retains fifty draws through a bounded seeded
sampler. Native generators and independent correctness regressions are original.

The single-list k-opt feasibility case and eight ordered-output fixtures in
`crates/solver/tests/list_k_opt_cases.rs` adapt `SelectorBasedKOptListMoveTest`
from the pinned Community revision (Apache-2.0). Exact identities, substitutions,
and the output fixtures' unadapted notification assertions are recorded in
`compat/provenance.json`. Original exhaustive k-opt regressions are separate.

Two multi-entity k-opt ordered-output fixtures in
`crates/solver/tests/list_k_opt_repartition_cases.rs` adapt the pinned Community
`SelectorBasedKOptListMoveTest` under Apache-2.0. Explicit cycle direction and
partition lengths replace implicit Java choices; notification assertions remain
pending. Provenance records these substitutions and no method credit is assigned.
The native combined-cycle implementation and independent references are original.

The twelve cases in `crates/solver/tests/composite_termination_cases.rs` adapt
`AndCompositeTerminationTest` and `OrCompositeTerminationTest`, including the
applicability helper from `AbstractCompositeTerminationTest`, at the pinned
Timefold Community revision (Apache-2.0). Every truth-table and gradient assertion
is retained; native evaluators replace Java mocks and scope objects. Exact source
identities and substitutions appear in `compat/provenance.json`. Native expression
and solve-control implementations and additional integration tests are original.

`crates/solver/tests/score_explanation_cases.rs` adapts 23 scoring/justification
methods and two shared assertion helpers from Community
`AbstractUniConstraintStreamTest`, with the seven-entity fixtures from
`TestdataLavishSolution` and `TestdataSimpleBigDecimalScoreSolution` (Apache-2.0).
Exact paths, revision and native substitutions are in `compat/provenance.json`.
The explanation implementation and separate incremental regressions are original.

`crates/solver/tests/constraint_weight_cases.rs` adapts three methods from Community
`ConstraintWeightOverridesTest` and the zero-weight/duplicate-name methods from
`AbstractUniConstraintStreamTest`, including their test-domain weight provider,
solution and entity fixtures. Apache-2.0 attribution, exact revision and all native
substitutions are recorded in `compat/provenance.json`. The weighting and pruning
implementation and separate regressions are original.

`crates/solver/tests/decimal_weight_cases.rs` adapts all sixteen methods in the
Simple, HardSoft, HardMediumSoft and Bendable BigDecimal `ScoreInlinerTest` classes
from pinned Timefold Community, with their singleton-constraint and bendable-score
helpers (Apache-2.0). Exact paths and substitutions are in provenance. The exact
weighting implementation and fractional-impact regressions are original.

`crates/solver/tests/scoped_weight_cases.rs` adapts sixteen non-overflow methods
from the pinned Simple, HardSoft, HardMediumSoft and Bendable `ScoreInlinerTest`
classes and their singleton-constraint/bendable-score helpers (Apache-2.0).
Exact paths, revision and native substitutions appear in `compat/provenance.json`.
The scoped weighting implementation and separate regression tests are original.

`crates/solver/tests/constraint_match_total_cases.rs` adapts the complete
`ConstraintMatchTotalTest.getScoreTotal` method from pinned Timefold Community
(Apache-2.0). Exact identity, revision and native substitutions are in provenance.
Scoped explanation construction, snapshot comparison and their regressions are original.

`crates/solver/tests/score_inliner_overflow_cases.rs` adapts all eight integer
ScoreInliner overflow methods from pinned Timefold Community (Apache-2.0), with
singleton-constraint and bendable helper attribution in `compat/provenance.json`.
Checked scoring policies, widened arithmetic and separate regressions are original.

`crates/solver/tests/temporal_range_cases.rs` adapts twenty-one methods from the
pinned Community `TemporalValueRangeTest` and two `PlannerAssert` iterator helpers
(Apache-2.0). Exact revision, paths, identities and native substitutions are in
`compat/provenance.json`. Civil calendar arithmetic, temporal ranges and the separate
regressions are independently implemented; no production implementation was used.

`crates/solver/tests/list_cascade_cases.rs` adapts four methods from Community
`SingleCascadingUpdateShadowVariableTest` and `DifferentCascadingUpdateShadowVariableTest`,
with their solution-generation and value-update fixtures (Apache-2.0). Exact paths,
revision and substitutions are recorded in `compat/provenance.json`. The native
list cascade cache, transactions and separate regressions are independently written.

`crates/solver/tests/dependency_queue_cases.rs` adapts all eight methods and two helpers
from the pinned Community `NodeTopologicalOrderQueueTest` (Apache-2.0). Provenance
records exact identities and native randomness/tie-breaking substitutions. The
heap-based queue and its derived-value cache integration are independently written.

`crates/solver/tests/weighted_selection_cases.rs` adapts the cached-size method
from Community `ProbabilityEntitySelectorTest` and the scripted random sequences
from that class and `ProbabilityMoveSelectorTest` (Apache-2.0). The two sequence
fixtures remain partial and earn no method credit; exact omissions and identities
are recorded in `compat/provenance.json`. The weighted sampler and regressions are
independently implemented.

`crates/solver/tests/selection_cache_cases.rs` adapts twenty methods from pinned
Community `CachingEntitySelectorTest`, `CachingValueSelectorTest`,
`CachingMoveSelectorTest`, `ProbabilityEntitySelectorTest` and
`ProbabilityMoveSelectorTest`, with their lifecycle/sequence helpers and
`PlannerAssert` helpers (Apache-2.0). Exact paths, identities and native
substitutions are recorded in `compat/provenance.json`. This completes the two
previously partial weighted-sequence methods in a separate lifecycle adaptation.
The native cache implementation and additional regressions are independently written.

`crates/solver/tests/ordered_selection_cases.rs` adapts thirteen methods from pinned
Community `SortingEntitySelectorTest`, `SortingMoveSelectorTest`,
`ShufflingEntitySelectorTest` and `ShufflingMoveSelectorTest`, with their lifecycle
helpers, `PlannerAssert` helpers and the list overloads of `TestdataObjectSorter`
and `CodeAssertableSorter` (Apache-2.0). Exact revision, identities and native
substitutions are in `compat/provenance.json`. Ordered snapshots and native
cache ordering are independently implemented; separate regressions are original.

`crates/solver/tests/selection_filter_cases.rs` adapts twenty-one methods from pinned
Community entity/move/value `Filtering*SelectorTest` and
`SelectedCountLimit*SelectorTest` classes, plus `IterableFilteringValueSelectorTest`,
with filtering/lifecycle/sequence helpers (Apache-2.0). Exact paths, identities,
revision and native substitutions are in `compat/provenance.json`. Source wrappers,
provider filtering/statistics and separate regressions are independently written.

`crates/solver/tests/selection_replay_cases.rs` adapts original-selection methods
and assertion helpers from pinned Community `MimicReplayingEntitySelectorTest`
and `MimicReplayingValueSelectorTest`, with `PlannerAssert` lifecycle/code helpers
(Apache-2.0). Exact paths, revision and native lifecycle/iterator substitutions
appear in `compat/provenance.json`. The shared publication channel, recording/replay
API and separate regression tests are independently written.

`crates/solver/tests/selection_product_cases.rs` adapts twenty independent-product
methods and eight fixture helpers from pinned Community
`CartesianProductMoveSelectorTest`, plus `PlannerAssert` sequence/metadata/lifecycle
helpers (Apache-2.0). Exact paths, revision and native snapshot/randomness
substitutions are in `compat/provenance.json`. Earlier MoveProduct fixtures retain
their separate partial attribution. Snapshot composition and additional regressions
are independently implemented.

`crates/solver/tests/dependent_selection_cases.rs` adapts both original-order mimic
methods and their helper from pinned Community `CartesianProductMoveSelectorTest`,
with `PlannerAssert` sequence/lifecycle helpers (Apache-2.0). Exact paths, revision
and native explicit-prefix substitutions are recorded in `compat/provenance.json`.
Dependent traversal, bounded conditional sampling and separate regressions are
independently implemented; random mimic methods remain pending.

`crates/solver/tests/reachable_values_cases.rs` adapts five complete methods and one
partial membership fixture from pinned Community `ReachableValuesTest`, plus the
code-comparator behavior of test helper `TestdataObjectSorter` (Apache-2.0).
`compat/provenance.json` records exact paths, revision, substitutions and omitted
runtime-type assertions. The inverse-domain implementation and additional regression
tests are independently written.

`crates/solver/tests/selection_union_cases.rs` adapts six methods from pinned
Community `UnionMoveSelectorTest`, with `PlannerAssert` sequence/exhaustion,
metadata and lifecycle helpers (Apache-2.0). Exact source identities and native
scripted-draw substitutions are recorded in `compat/provenance.json`. Lifecycle
union composition, fractional stream interleaving and regression tests are
independently implemented.

`crates/solver/tests/reachability_cache_cases.rs` adapts six methods and two fixture
helpers from pinned Community `IterableFromEntityPropertyValueSelectorTest` and
`FilteringValueRangeSelectorTest`, plus `PlannerAssert` sequence/metadata helpers
and the ascending comparator intent of `TestdataObjectSorter` (Apache-2.0).
Exact source identities and native lifecycle/query substitutions are recorded in
`compat/provenance.json`. Cache/index composition, native domain sources and
additional regressions are independently written.

`crates/solver/tests/assignment_selection_cases.rs` adapts four methods from pinned
Community `ReinitializeVariableValueSelectorTest` and `UnassignedListValueSelectorTest`,
plus `PlannerAssert` sequence/metadata/lifecycle helpers (Apache-2.0). Exact paths,
revision and native context/gate/snapshot substitutions are recorded in
`compat/provenance.json`. Native assignment sources, gating and additional regression
tests are independently implemented.

`crates/solver/tests/revision_selection_cases.rs` adapts ten methods and two fixture
helpers from pinned Community `FromSolutionEntitySelectorTest`, plus `PlannerAssert`
sequence/metadata helpers (Apache-2.0). Exact paths, revision and native revision-token,
lifecycle and scripted-random substitutions are recorded in `compat/provenance.json`.
Revision-aware snapshot selection and additional regression tests are independently
implemented.

`crates/solver/tests/unique_pair_cases.rs` adapts two methods from pinned Community
`AbstractUniConstraintStreamTest`, with assertion and zero-entity fixture helpers
from `AbstractConstraintStreamTest` and `TestdataLavishSolution` (Apache-2.0).
Exact identities and native substitutions are recorded in `compat/provenance.json`.
The indexed unique-pair implementation and additional regressions are independent.

`crates/solver/tests/grouped_tuple_cases.rs` adapts eighteen grouping methods from
pinned Community `AbstractUniConstraintStreamTest` and `AbstractBiConstraintStreamTest`
(Apache-2.0), plus assertion/set/fixture/code helpers from
`AbstractConstraintStreamTest`, `TestdataLavishSolution` and `TestdataObject`.
Exact identities, revision and native substitutions are in `compat/provenance.json`.
Flat tuple collectors, distinct counting and additional regressions are independent.

`crates/solver/tests/grouped_multiway_cases.rs` adapts twenty grouping methods from
pinned Community `AbstractTriConstraintStreamTest` and `AbstractQuadConstraintStreamTest`
(Apache-2.0), plus assertion/fixture/code helpers from `AbstractConstraintStreamTest`,
`TestdataLavishSolution` and `TestdataObject`. Source helper membership semantics,
including its disregard for justification order/multiplicity, are preserved and
recorded with the exact source identities in `compat/provenance.json`.

`crates/solver/tests/shared_transform_cases.rs` adapts eighteen node-sharing methods
and setup helpers from pinned Community `AbstractUni/Bi/Tri/QuadConstraintStreamNodeSharingTest`
(Apache-2.0). Exact identities and the native reusable-definition/canonical-handle
substitutions are recorded in `compat/provenance.json`. Shared transform definitions,
weak registration storage and additional runtime regressions are independent.

`crates/solver/tests/shared_relation_cases.rs` adapts forty-five join/existence
node-sharing methods and setup helpers from pinned Community
`AbstractUni/Bi/Tri/QuadConstraintStreamNodeSharingTest`, with code-projection semantics
from `TestdataObject.toString` (Apache-2.0). Exact identities and native substitutions
are in `compat/provenance.json`. Reusable relation definitions and runtime regression
cases are independently implemented.

`crates/solver/tests/shared_aggregation_cases.rs` adapts twenty-four grouping
node-sharing methods and four setup helpers from pinned Community
`AbstractUni/Bi/Tri/QuadConstraintStreamNodeSharingTest` (Apache-2.0). Exact methods
and native substitutions are recorded in `compat/provenance.json`. The aggregation
definitions and runtime regression cases are independently implemented.

`crates/solver/tests/shared_set_cases.rs` adapts fifty-seven transformation,
distinct and concatenation node-sharing methods plus setup helpers from pinned
Community `AbstractUni/Bi/Tri/QuadConstraintStreamNodeSharingTest` (Apache-2.0).
Exact methods and substitutions are recorded in `compat/provenance.json`.
Shared set/concatenation definitions and runtime comparisons are independent.

`crates/solver/tests/shared_existence_scope_cases.rs` adapts seventy-two existence,
absence, other-entity and including-unassigned node-sharing methods plus setup
helpers from pinned Community `AbstractUni/Bi/Tri/QuadConstraintStreamNodeSharingTest`
(Apache-2.0). Exact identities, explicit native scopes and the unary mixed-mode
fixture are documented in `compat/provenance.json`. Runtime comparisons are original.

`crates/solver/tests/precompute_filter_cases.rs` adapts thirteen pinned Community
precompute filter/lifecycle and fact-removal methods, including two unique-pair
methods, with Lavish fixture and Tri/Quad helper attribution (Apache-2.0).
`compat/provenance.json` records exact methods and the explicit fact-key/live-join
substitutions. The precompute cache, graph integration and runtime regressions
are independently implemented.

`crates/solver/tests/composite_move_builder_cases.rs` adapts six methods from
pinned Community `SelectorBasedCompositeMoveTest` (Apache-2.0). Native variants,
ordered write journals and stable variable/value identities replace Java classes
and mocks. Exact method identities and substitutions are recorded in
`compat/provenance.json`. The checked builder and its regressions are original.

`crates/solver/tests/iterator_bias_cases.rs` adapts all six pinned Community
`IteratorBiasIT` methods and test helpers from `AbstractBiasIT` and `BiasReport`
(Apache-2.0). Trial counts, layout fixtures, expected shares and statistical bounds
are preserved; native indexed queries and PRNGs replace Java objects and traces.
Exact method/helper identities and substitutions are in `compat/provenance.json`.

`crates/solver/tests/dataset_bucket_bias_cases.rs` adapts the pinned Community
`DatasetBucketBiasIT` method and statistical helpers from `BiasReport`
(Apache-2.0). Native indexed joins and retirement replace the Java dataset/session
surface; exact source identities and substitutions are in `compat/provenance.json`.
The sparse retirement implementation and native regressions are original.

`crates/solver/tests/sampling_iterator_bias_cases.rs` adapts both pinned Community
`SamplingIteratorBiasIT` methods and statistical helpers from `BiasReport`
(Apache-2.0). Native datasets, keyed sample assembly and PRNG streams replace Java
objects/session APIs. All parameters, trial counts and probability bounds are
preserved; exact identities and substitutions are in `compat/provenance.json`.

`crates/solver/tests/sublist_sampling_bias_cases.rs` adapts both pinned Community
`SubListSamplingBiasIT` methods and `BiasReport` statistical helpers (Apache-2.0).
Native value ownership and span selection replace Java score-director plumbing;
exact identities, parameters and substitutions are in `compat/provenance.json`.
The indexed value-seeded selection implementation and regressions are original.

`crates/solver/tests/filtered_pair_bias_cases.rs` adapts pinned Community
`LeftRetirementBiasIT` and the filtering-only `PairFairnessBiasIT` method, with
`BucketedFixture`, `AbstractBiasIT` and `BiasReport` helpers (Apache-2.0). Native
filtered sampling and change moves replace Java stream/session plumbing; exact
identities and substitutions are recorded in `compat/provenance.json`.

`crates/solver/tests/provider_group_cases.rs` adapts three pinned Community
`MixedMoveSelectorTest` methods, `MixedMoveSelectorSplitBiasIT`, and its endless
provider/classification fixtures plus `BiasReport` statistics (Apache-2.0).
Native explicit provider groups replace Java selector introspection. Exact test
and helper identities and substitutions are in `compat/provenance.json`.

`crates/solver/tests/best_recaller_cases.rs` adapts all eight pinned Community
`BestSolutionRecallerTest` methods and their shared retention helpers (Apache-2.0).
Native snapshot identities and explicit required-unassigned counts replace mocked
Java solutions and score-director scopes. Exact identities and substitutions are
recorded in `compat/provenance.json`. The tracker and native regressions are original.

- `crates/solver/tests/phase_termination_cases.rs` adapts six Community
  `TerminationTest` methods for construction step limits and phase applicability,
  plus their two-entity setup and the `TestdataEasyScoreCalculator` test fixture.
  Native structural completeness metadata remains separate from ordinary scores;
  typed profile errors replace Java exceptions. See `compat/provenance.json`.

- `crates/solver/tests/basic_change_doability_cases.rs` adapts two methods from
  Community `SelectorBasedChangeMoveTest` and the entity-range doability method
  from `SelectorBasedSwapMoveTest` (Apache-2.0). Native basic variables and checked
  move queries replace descriptor/director plumbing. Exact identities and
  substitutions are recorded in `compat/provenance.json`.

- `crates/solver/tests/list_move_doability_cases.rs` adapts five pinned Community
  doability methods from `SelectorBasedListChangeMoveTest`,
  `SelectorBasedListSwapMoveTest` and `SelectorBasedSubListChangeMoveTest`
  (Apache-2.0). Native validation errors map to false for boolean assertions;
  identities and substitutions are recorded in `compat/provenance.json`.

- `crates/solver/tests/list_assignment_lifecycle_cases.rs` adapts three pinned
  Community methods from `SelectorBasedListAssignMoveTest` and
  `SelectorBasedListUnassignMoveTest` (Apache-2.0). Native lifecycle event counts
  replace Mockito callback verification, and borrowed checks replace range-manager
  plumbing. Exact assertions and lifecycle differences are recorded in
  `compat/provenance.json`; no callback-order equivalence is claimed.

- `crates/solver/tests/list_execution_notification_cases.rs` adapts nine Community
  execution methods from selector-based list/sublist change and swap tests, plus
  the six-row same-entity change parameter fixture (Apache-2.0). Native event
  counters preserve exact callback counts and ranges; detailed mappings appear in
  `compat/provenance.json`. No callback-order equivalence is claimed.

- `crates/solver/tests/comparator_sorting_cases.rs` adapts four Community comparator
  and move-sorting methods, plus the numeric comparator helper and two-row
  comparator configuration/helper fixtures (Apache-2.0). Native configured selection callbacks replace reflective
  Java classes; full mappings and assertion details are in `compat/provenance.json`.

- `crates/solver/tests/list_ruin_plan_cases.rs` adapts the pinned Community
  `SelectorBasedListRuinRecreateMoveTest.executeAndUndoNewDestinationEntityWithPinnedPrefix`
  method and its in-method five-value fixture and penalty (Apache-2.0). Native
  explicit repair plans, queued construction and temporary scored moves preserve
  both exact restored-list assertions. Full provenance and API substitutions
  are recorded in `compat/provenance.json`. The plan implementation and
  `list_ruin_plan_regression.rs` are original work.

- `crates/solver/tests/shadow_notifier_undo_cases.rs` adapts all seven inputs of
  Community `MoveDirectorUndoShadowVariableTest.undoRestoresListShadowVariablesAndScore`
  and the squared-list-length fixture from
  `TestdataListWithShadowHistoryConstraintProvider.defineConstraints` (Apache-2.0).
  Native temporary calculator moves and cached declarative list projections retain
  list, owner/index/previous/next and fresh-score restoration assertions. Complete
  provenance and substitutions are recorded in `compat/provenance.json`.

- `crates/solver/tests/termination_time_config_cases.rs` adapts all six methods
  from Community `TerminationConfigTest` and four time-limit methods from
  `TerminationFactoryTest` (Apache-2.0). Native immutable profiles and real runtime
  limit binding replace Java setters, inheritance and factory class assertions.
  Exact input/assertion mappings appear in `compat/provenance.json`; the separate
  boundary and clocked-solve regressions are original work.

- `crates/solver/tests/termination_factory_profile_cases.rs` adapts eight more
  Community `TerminationFactoryTest` methods (Apache-2.0). Native borrowed
  composition, prepared score-aware profiles and actual runtime controls replace
  Java factory/class assertions. Complete identity, input, diagnostic and
  substitution mappings are in `compat/provenance.json`; the accompanying
  `scored_termination_profile_regression.rs` cases are original work.

- `crates/solver/tests/shadow_structure_cases.rs` adapts all twelve Community
  `GraphStructureTest` methods (Apache-2.0). Its shared fixture adapts the relevant
  supplier-source declarations and default/group membership inputs from eleven
  pinned test-domain classes. Typed bindings replace descriptor reflection;
  exact mappings, fixture subsets and substitutions appear in
  `compat/provenance.json`. The classifier and separate regression cases are
  original work; classification does not claim a solver fast-path or cycle proof.

- `crates/solver/tests/environment_profile_cases.rs` adapts all nine Community
  `EnvironmentModeUtilTest` methods and the relevant two-phase declaration layout
  from `PlannerTestUtils.buildSolverConfig` (Apache-2.0). Native immutable metadata
  preserves queries, defaults, overrides and exact asserted rejection diagnostics.
  Complete mappings are in `compat/provenance.json`; runtime mode scheduling and
  score-director lifecycle cases earn no credit. Implementation and separate
  preparation regressions are original work.

- `crates/solver/tests/acceptor_profile_cases.rs` adapts all six Community
  `AcceptorFactoryTest` methods and the negative-size methods from
  `LateAcceptanceAcceptorTest` and `StepCountingHillClimbingAcceptorTest`
  (Apache-2.0). Native policy preparation replaces factory/class assertions;
  complete input, order, failure and substitution mappings appear in
  `compat/provenance.json`. The implementation and real-solver/boundary regressions
  are original work. Java defaults, random schedules and environment-mode
  instrumentation are not claimed.

- `crates/solver/tests/timed_score_ring_cases.rs` adapts all eighteen Community
  `DiminishedReturnsScoreRingBufferTest` methods (Apache-2.0). Physical storage,
  cursor, growth, eviction and error assertions are retained. Native explicit
  length/Option values replace timestamp sentinels/null. Complete mappings are in
  `compat/provenance.json`. The ring implementation and separate reference-model,
  ownership and boundary regressions are original work.
