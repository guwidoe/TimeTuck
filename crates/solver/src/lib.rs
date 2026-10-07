//! An independent Rust constraint optimization library under development.
//!
//! Scores maximize lexicographically: structural integrity first, then hard and
//! softer levels. Compatibility is limited to the cases in the provenance ledger.
//!
//! ```
//! use timetuck::HardSoftScore;
//! let feasible = HardSoftScore::new(0, -1_000_000);
//! let infeasible = HardSoftScore::new(-1, 0);
//! assert!(feasible > infeasible);
//! assert!(feasible.is_feasible());
//! assert_eq!("0hard/-1000000soft".parse(), Ok(feasible));
//! ```

mod score_level;
pub use score_level::ScoreLevel;
pub mod score;
pub use score::{
    AdditiveScore, HardMediumSoftScore, HardSoftScore, ParseScoreError, Score, SimpleScore,
};

pub mod model;
pub mod moves;
pub mod search;
pub use model::{FiniteProblem, ModelError, Variable};
pub use moves::{AppliedMove, Assignment, Move, MoveError, PriorValue};
pub use search::{ScoredAssignment, SearchResult, Termination, exhaustive_search};

pub mod scoring;
pub use scoring::{ConstraintNetwork, ScoreDirector, ScoredMove, ScoringError};

mod pillar_schema;
pub use pillar_schema::{PillarSchema, pillar_swap_moves};
mod subpillar;
pub use subpillar::{
    PillarChangeMode, SubPillarConfig, SubPillarSelection, SubPillarSizeError, subpillar_moves,
};
mod pillar;
pub use pillar::{Pillar, PillarError, pillar_moves};

pub mod neighborhood;
pub use neighborhood::{change_moves, swap_moves};
pub mod local_search;
mod step_termination;
pub use local_search::{
    Acceptance, LocalSearchConfig, LocalSearchError, LocalSearchResult, LocalSearchTermination,
    MoveOrder, MoveSelection, Neighborhood, RejectedMovePolicy, local_search, local_search_from,
};
pub use step_termination::StepTermination;

pub mod acceptance;
pub use acceptance::{AcceptanceError, Acceptor, LateAcceptance};
mod forager;
mod random;
pub use forager::{FinalistRanking, ForagerConfig, InvalidFinalistIndex, MoveForager, TieBreaking};

pub mod construction;
pub use construction::{
    ConstructionConfig, ConstructionInterruption, ConstructionPickEarly, ConstructionPlacement,
    ConstructionResult, ConstructionTermination, OptionalConstruction, greedy_construction,
    greedy_construction_from,
};

pub mod move_acceptance;
pub use move_acceptance::{MoveAcceptor, ValueAcceptor};
pub mod tabu;
pub use tabu::TabuMemory;
mod fading_tabu;
pub use fading_tabu::FadingTabuMemory;

pub mod annealing;
pub use annealing::SimulatedAnnealing;

pub mod bendable;
pub use bendable::BendableScore;
mod bendable_decimal;
pub use bendable_decimal::BendableDecimalScore;

pub mod list;
mod list_dependencies;
mod list_lifecycle;
mod list_observation;
pub use list::{
    AppliedListMove, ListAssignment, ListElement, ListEntity, ListError, ListMove, ListPosition,
    ListProblem, ListRange,
};
pub use list_lifecycle::{
    AppliedListLifecycle, ListLifecycleChange, ListLifecycleEvent, ListLifecyclePlan, StaleListPlan,
};
pub use list_observation::{ListChangeEvent, ListChangePhase, ObservedListMove};
mod list_gather_neighborhood;
mod list_mass_change;
pub use list_gather_neighborhood::list_gather_moves;
pub use list_mass_change::{
    ListMassChange, ListMassChangeConfig, ListSampleSize, list_mass_change_moves,
};
mod list_gather;
pub use list_gather::ListGatherError;
pub mod list_search;
pub use list_search::{ListSearchResult, ScoredListAssignment, exhaustive_list_search};

pub mod mixed;
pub use mixed::{
    AppliedMixedMove, MixedAssignment, MixedError, MixedMove, MixedPlanningValue, MixedProblem,
};
pub mod mixed_search;
pub use mixed_search::{MixedSearchResult, ScoredMixedAssignment, exhaustive_mixed_search};

pub mod list_scoring;
pub use list_scoring::{
    ListConstraintNetwork, ListFact, ListScope, ListScoreDirector, ListScoringError, ScoredListMove,
};

pub mod list_neighborhood;
pub use list_neighborhood::{
    list_assign_moves, list_change_moves, list_moves, list_sublist_change_moves,
    list_sublist_swap_moves, list_swap_moves, list_tail_swap_moves, list_two_opt_moves,
    list_unassign_moves,
};

pub mod list_construction;
pub use list_construction::{
    ListConstructionResult, greedy_list_construction, greedy_list_construction_from,
};
pub mod list_local_search;
pub use list_local_search::{ListLocalSearchError, ListLocalSearchResult, list_local_search_from};

pub mod stream;
pub use stream::{ConstraintStream, IntCollector, IntSummary, StreamScoreError};

mod conditional_collector;
pub use conditional_collector::ConditionalCollector;

mod collecting_stream;
pub use collecting_stream::{
    CollectNode, CountCollector, PairCollector, ReversibleCollector, SetCollector,
};
mod map_collector;
pub use map_collector::MapCollector;
mod list_collector;
pub use list_collector::{ListCollector, ListSnapshot};
pub use num_bigint::BigInt;
mod decimal;
pub use decimal::Decimal;
mod numeric_decimal;
pub use numeric_decimal::NumericDecimal;
mod decimal_math;
pub use decimal_math::{DecimalArithmeticError, ParseDecimalError};
mod decimal_score;
pub use decimal_score::{
    HardMediumSoftDecimalScore, HardSoftDecimalScore, ParseDecimalScoreError, SimpleDecimalScore,
};
mod big_number_collector;
pub use big_number_collector::{
    BigIntAverageCollector, BigIntSumCollector, DecimalAverageCollector, DecimalSumCollector,
};
mod duration_collector;
pub use duration_collector::{
    DurationAverageCollector, DurationOverflow, DurationSumCollector, SignedDuration,
};
mod average_collector;
pub use average_collector::{AverageCollector, AverageOverflow};
mod distinct_stream;
pub use distinct_stream::DistinctNode;
mod existence_stream;
pub use existence_stream::{ExistenceMode, ExistsNode};
pub mod incremental_stream;
pub use incremental_stream::{
    FilterNode, GroupNode, JoinComparison, JoinNode, PartitionedJoinNode, ScoreNode,
    StructuralStreamContribution, TupleChange,
};

pub mod stream_graph;
pub use stream_graph::{
    ConcatId, ConcatenatedStreamPort, GraphError, GraphTransaction, GroupedListStreamPort,
    JoinedStreamPort, NodeActivity, SharedFilter, SharedFlatMap, SharedMap, StreamActivity,
    StreamConstraint, StreamDataset, StreamGraph, StreamGraphBuilder, StreamPort, StreamSource,
};

pub mod stream_scoring;
pub use stream_scoring::{
    PlanningStreamSource, StreamScoreDirector, StreamScoredMove, StreamScoringError,
    greedy_stream_construction, greedy_stream_construction_from, stream_local_search,
    stream_local_search_from,
};

mod search_scoring;

pub mod snapshot_map;
pub use snapshot_map::SnapshotMap;

mod list_search_scoring;
pub mod list_stream_scoring;
pub use list_stream_scoring::{
    ListStreamScoreDirector, ListStreamScoredMove, ListStreamScoringError, ListStreamSources,
    ListValueRow, greedy_list_stream_construction, greedy_list_stream_construction_from,
    list_stream_local_search_from,
};

pub mod mixed_scoring;
pub use mixed_scoring::{
    MixedConstraintNetwork, MixedFact, MixedScope, MixedScoreDirector, MixedScoringError,
    ScoredMixedMove,
};

pub mod mixed_neighborhood;
pub use mixed_neighborhood::{
    MixedNeighborhood, MixedNeighborhoodError, mixed_moves, mixed_moves_configured,
    mixed_moves_configured_union,
};
pub mod mixed_local_search;
pub use mixed_local_search::{
    MixedLocalSearchConfig, MixedLocalSearchError, MixedLocalSearchResult, mixed_local_search_from,
};

pub mod mixed_construction;
pub use mixed_construction::{
    MixedConstructionConfig, MixedConstructionOrder, MixedConstructionResult,
    greedy_mixed_construction, greedy_mixed_construction_from,
};

mod mixed_search_scoring;
pub mod mixed_stream_scoring;
pub use mixed_stream_scoring::{
    MixedStreamScoreDirector, MixedStreamScoredMove, MixedStreamScoringError, MixedStreamSources,
    greedy_mixed_stream_construction, greedy_mixed_stream_construction_from,
    mixed_stream_local_search_from,
};

mod canonical_flat_map;
mod flat_map_stream;
pub use flat_map_stream::FlatMapNode;

mod overlap_join;
pub use overlap_join::OverlapJoinNode;

mod equality_existence;
pub use equality_existence::ExistsEqualNode;

mod step_progress;
pub use step_progress::{StepCountLimit, StepProgress};

mod score_progress;
pub use score_progress::{FeasibilityProgress, ScoreProgressError, ScoreTargetProgress};

mod time_limit;
pub use time_limit::ElapsedTimeLimit;

#[cfg(test)]
mod timed_search_tests;

pub mod cancellation;
pub use cancellation::CancellationToken;

pub use list_search::exhaustive_list_search_with_config;
pub use mixed_search::exhaustive_mixed_search_with_config;
pub use search::{ExactSearchConfig, exhaustive_search_with_config};

pub mod count_limit;
pub use count_limit::CountLimit;

pub mod solver;
pub use solver::{
    MixedSolverConfig, PhaseReport, PhaseTermination, SolverConfig, SolverError, SolverLimits,
    SolverResult, SolverTermination, SolverWork, solve, solve_list, solve_list_stream, solve_mixed,
    solve_mixed_stream, solve_stream,
};

#[cfg(test)]
mod timed_construction_tests;

mod membership_join;
pub use membership_join::MembershipJoinNode;

#[cfg(test)]
mod timed_exact_tests;

mod solver_plan;
pub use solver_plan::{
    BoundedPhaseConfig, ExactPhaseConfig, ExactPhaseTermination, ExactSearchStrategy,
    MixedSolverPhase, MixedSolverPlan, MixedSolverSequence, PartialScoreBound, SolverPhase,
    SolverPlan, SolverSequence,
};

mod step_counting;
pub use step_counting::{StepCountingHillClimbing, StepCountingMode};

mod great_deluge;
pub use great_deluge::{GreatDeluge, GreatDelugeRise};

mod solver_manager;
pub use solver_manager::{
    SolverJob, SolverJobError, SolverJobResult, SolverManager, SolverStatus, SolverThreadRole,
    SolverThreadSpec, SolverThreadTask, SubmitError,
};

mod solver_factory;
pub use solver_factory::{
    ConfiguredSolverJob, ConfiguredSolverJobBuildError, ConfiguredSolverJobBuilder,
    ConfiguredSolverManager, SolverFactory, SolverFactoryError,
};

mod solver_job_builder;
pub use solver_job_builder::{SolverJobBuildError, SolverJobBuilder};

mod solver_manager_callbacks;
pub use solver_manager_callbacks::{
    SolverBestEvent, SolverCallbackFailure, SolverCallbackStage, SolverEventProducer,
    SolverJobCallbacks, SolverJobFailure, SolverProgress,
};

pub use list_search::exhaustive_list_search_with_observer;
pub use mixed_search::exhaustive_mixed_search_with_observer;
pub use search::exhaustive_search_with_observer;

mod solution_observation;
pub use solution_observation::*;

pub use solver::{
    SolverPhaseKind, SolverUpdate, solve_list_stream_with_observer, solve_list_with_observer,
    solve_mixed_stream_with_observer, solve_mixed_with_observer, solve_stream_with_observer,
    solve_with_observer,
};

pub use solver::{
    SolverEvent, SolverInitialization, solve_list_stream_with_events, solve_list_with_events,
    solve_mixed_stream_with_events, solve_mixed_with_events, solve_stream_with_events,
    solve_with_events,
};

pub mod custom_phase;
pub use custom_phase::*;

pub use solver::{
    solve_list_stream_with_commands, solve_list_stream_with_commands_and_events,
    solve_list_with_commands, solve_list_with_commands_and_events,
    solve_mixed_stream_with_commands, solve_mixed_stream_with_commands_and_events,
    solve_mixed_with_commands, solve_mixed_with_commands_and_events, solve_stream_with_commands,
    solve_stream_with_commands_and_events, solve_with_commands, solve_with_commands_and_events,
};

pub use solver::IncompleteHandoffPolicy;

pub mod live_solver;
pub use live_solver::*;

mod statistics;
pub use statistics::{
    AssignmentSpace, ProblemSizeStatistics, ProblemStatistics, RevisionProblemSize,
    SolverJobStatistics,
};

mod reference_stream;
pub use reference_stream::ReferenceNode;

mod dependency_graph;
pub use dependency_graph::{DependencyError, DependencyGraph, DerivedValue};

mod dependency_topology;
pub use dependency_topology::DependencyTopology;

mod dependency_cache;
pub use dependency_cache::DependencyCache;
mod dependency_node;
pub use dependency_node::{DependencyNode, DerivedChange};

mod sequence_collector;
pub use sequence_collector::{
    ConsecutiveSequence, IntegerSequenceMetric, SequenceBreak, SequenceChain, SequenceCollector,
    SequenceItem, SequenceMetric,
};

mod range_collector;
pub use range_collector::{
    ConnectedRange, ConnectedRangeChain, ConnectedRangeCollector, IntegerRangeMetric, RangeGap,
    RangeItem, RangeMetric,
};

mod load_balance_collector;
pub use load_balance_collector::{LoadBalance, LoadBalanceCollector};

mod period_collector;
pub use period_collector::{CalendarPeriod, PeriodOverflow, PeriodSumCollector};

mod extrema_collector;
pub use extrema_collector::{Extremum, ExtremumCollector, MaxByCollector, MinByCollector};

mod finishing_collector;
pub use finishing_collector::{CollectorFinisher, FinishingCollector};

mod sum_collector;
pub use sum_collector::{SumCollector, SumOverflow};

mod diversified_late;
pub use diversified_late::DiversifiedLateAcceptance;

mod tabu_size;
pub use tabu_size::TabuSize;

mod score_history;
pub use score_history::ScoreHistory;

mod branch_and_bound;
mod branch_tree;
mod list_branch_and_bound;
mod mixed_branch_and_bound;
pub use branch_and_bound::{
    BoundedSearchResult, BranchAndBoundConfig, BranchAndBoundResult, BranchEvaluation,
    BranchTermination, InvalidOptimisticBound, NodeExploration, SearchNodeKey, branch_and_bound,
    branch_and_bound_with_observer,
};
pub use list_branch_and_bound::{
    ListBranchAndBoundResult, branch_and_bound_lists, branch_and_bound_lists_with_observer,
};
pub use mixed_branch_and_bound::{
    MixedBranchAndBoundResult, branch_and_bound_mixed, branch_and_bound_mixed_with_observer,
};

mod score_trend;
pub use score_trend::{
    InitializingScoreBound, InitializingScoreTrend, InitializingScoreTrendLevel, ScoreTrendError,
};

mod diminished_returns;
pub use diminished_returns::{DiminishedReturns, DiminishedReturnsConfig, DiminishedReturnsError};

mod significant_improvement;
pub use significant_improvement::{
    SignificantImprovement, SignificantImprovementConfig, SignificantImprovementError,
};

mod value_construction_queue;

mod ruin_engine;
mod ruin_recreate;
pub use ruin_recreate::{
    ListRuinRecreateResult, RepairMoveWeights, RepairNeighborhood, RepairPool, RuinRecreateConfig,
    RuinRecreateError, RuinRecreateOutcome, RuinRecreateResult, RuinRecreateTermination,
    ruin_recreate_search_from, ruin_recreate_search_from_with_observer,
    ruin_recreate_stream_search_from, ruin_recreate_stream_search_from_with_observer,
};

mod list_ruin_recreate;
pub use list_ruin_recreate::{
    ruin_recreate_list_search_from, ruin_recreate_list_search_from_with_observer,
    ruin_recreate_list_stream_search_from, ruin_recreate_list_stream_search_from_with_observer,
};

mod mixed_ruin_recreate;
pub use mixed_ruin_recreate::{
    MixedRuinRecreateConfig, MixedRuinRecreateResult, ruin_recreate_mixed_search_from,
    ruin_recreate_mixed_search_from_with_observer, ruin_recreate_mixed_stream_search_from,
    ruin_recreate_mixed_stream_search_from_with_observer,
};

mod mass_change;
pub use mass_change::{MassChange, MassChangeConfig, mass_change_moves};

mod mass_assignment;
pub use mass_assignment::{
    MassAssignmentSize, mass_assign_moves, mass_unassign_moves, sample_mass_unassign_moves,
};

mod list_mass_assignment;
pub use list_mass_assignment::{
    list_mass_assign_moves, list_mass_unassign_moves, sample_list_mass_unassign_moves,
};

mod list_k_opt;
pub use list_k_opt::{ListKOptConfig, ListKOptError, list_k_opt_moves};

mod list_k_opt_sampling;
pub use list_k_opt_sampling::{ListKOptSampling, sample_list_k_opt_moves};

mod list_k_opt_repartition;
pub use list_k_opt_repartition::sample_list_k_opt_repartitions;

mod composite_termination;
pub use composite_termination::{
    SearchLimit, SearchProgress, SearchTermination, TerminationExpression, TerminationState,
};

mod score_explanation;
pub use score_explanation::{
    ConstraintExplanation, ConstraintExplanationChange, ConstraintMatchRef, DefaultJustification,
    ExplainedMatch, ExplanationChange, Indictment, ScopedScoreExplanation, ScoreExplanation,
    ScoreExplanationDiff,
};

mod constraint_weights;
pub use constraint_weights::{ConstraintWeightOverrides, DecimalWeightedScore};

mod scoped_weights;
pub use scoped_weights::ConstraintWeightError;

mod scoring_arithmetic;
pub use scoring_arithmetic::{ScoreOverflow, ScoringArithmetic};

mod variable_neighborhood_descent;
pub use variable_neighborhood_descent::{
    DescentRequiresSequentialOrder, variable_neighborhood_descent,
};
mod move_union;
pub use move_union::{MoveUnion, NeighborhoodUnion, UnionOrder};

mod move_product;
pub use move_product::MoveProduct;
mod change_product;
pub use change_product::change_product_moves;

mod move_provider;
pub use move_provider::MoveProvider;
mod queued_move_provider;
pub use queued_move_provider::{PlacementMoveFactory, PlacementQueueOrder, QueuedMoveProvider};
mod provider_search;
pub use provider_search::{
    list_local_search_from_with_provider, list_stream_local_search_from_with_provider,
    local_search_from_with_provider, mixed_local_search_from_with_provider,
    mixed_stream_local_search_from_with_provider, stream_local_search_from_with_provider,
};

mod solver_extensions;
pub use solver_extensions::{
    MixedSolverExtensions, PhaseMoveProviders, SolverCommandCallback, SolverEventCallback,
    SolverExtensions,
};

pub use solver::{
    solve_list_stream_with_extensions, solve_list_with_extensions,
    solve_mixed_stream_with_extensions, solve_mixed_with_extensions, solve_stream_with_extensions,
    solve_with_extensions,
};

mod list_sublist;
pub use list_sublist::{
    SubListChangeConfig, SubListSize, SubListSwapConfig, list_sublist_change_moves_with_config,
    list_sublist_swap_moves_with_config, list_sublist_unassign_moves,
};

mod assignment_neighborhood;
pub use assignment_neighborhood::{
    PillarChangeConfig, assign_moves, pillar_change_moves, pillar_unassign_moves, unassign_moves,
};

mod neighborhood_sample;
pub use neighborhood_sample::{
    KeyedSampler, NeighborhoodSample, PillarSampler, SampleDecision, SampleError, SampleRandom,
    Sampler, SizeSampler, assemble_keyed_sample, assemble_sample,
};

mod neighborhood_dataset;
pub use neighborhood_dataset::{
    CachedNeighborhoodJoin, DatasetPermutation, DatasetRandom, NeighborhoodDataset,
    NeighborhoodDatasetStore, NeighborhoodJoin, repeated_keyed_samples, repeated_samples,
};

mod dataset_enumeration;
pub use dataset_enumeration::{
    BasicEntityDataset, DatasetEnumerationOptions, EnumeratedDataset, ListEntityDataset,
    basic_entity_dataset, list_entity_dataset, list_value_dataset,
};

mod retiring_sampling;
pub use retiring_sampling::{
    BoundedFilter, FilterProbe, NoDrawToRetire, RetiringPairs, RetiringRandom,
};

mod list_destinations;
pub use list_destinations::{ListDestinationDataset, list_destination_dataset, list_destinations};
mod provider_union;
pub use provider_union::{EmptyMoveProviders, ProviderUnion, WeightedMoveProvider};

mod value_exchange;
pub use value_exchange::{
    ValueExchangeConfig, ValueExchangeError, value_exchange_moves, value_exchange_moves_in_order,
};

mod list_move_builder;
pub use list_move_builder::{ListEditError, ListMoveBuilder};

mod sublist_sampling;
pub use sublist_sampling::{
    SubListSelection, sampled_sublist_change_moves, sampled_sublist_swap_moves,
};

mod value_range;
pub use value_range::{
    BooleanValueRange, CompositeValueRange, EnumeratedValueRange, IntegerValueRange,
    OptionalValueRange, ValueRange, ValueRangeError, ValueRangeIter, variable_from_range,
};

mod common_values;
pub use common_values::{
    CommonValueBudget, CommonValueError, CommonValueRanges, SharedValueRange,
    intersection_probe_budget, sample_common_change,
};

mod exact_value_range;
pub use exact_value_range::{BigIntegerValueRange, DecimalValueRange, ExactRangeError};

mod temporal_value_range;
pub use temporal_value_range::{
    CalendarYear, CivilDate, CivilDateTime, CivilTime, TemporalRangeError, TemporalUnit,
    TemporalValue, TemporalValueRange,
};

mod list_cascade;
pub use list_cascade::{ListCascadeCache, ListCascadeContext, ListCascadeError, ListCascadeMove};

mod node_order_queue;
pub use node_order_queue::{NodeOrderQueue, UnknownOrderNode};

mod weighted_selection;
pub use weighted_selection::{WeightedSelection, WeightedSelectionError};

mod selection_cache;
pub use selection_cache::{
    CachedSelectionIter, CachedSelectionOrder, SelectionCache, SelectionCacheError,
    SelectionCacheScope, SelectionCacheState, SelectionLifecycleEvent, SelectionSource,
};

mod ordered_selection;
pub use ordered_selection::{OrderedSelection, OrderedSelectionCache, OrderedSelectionError};

mod selection_filters;
pub use selection_filters::{
    CandidateFilterStats, CandidateFilterStop, FilteredMoveProvider, FilteredSelectionSource,
    LimitedSelectionSource,
};

mod selection_replay;
pub use selection_replay::{
    RecordingSelection, RecordingSelectionIter, ReplayingSelection, SelectionReplayCursor,
    SelectionReplayError,
};

mod selection_product;
pub use selection_product::{
    ProductSelectionIter, ProductSnapshot, SelectionProduct, SelectionProductError,
};

mod dependent_selection;
pub use dependent_selection::{
    DependentPrefix, DependentRow, DependentSampleError, DependentSelection,
    DependentSelectionIter, DependentSelectionStats, DependentStop,
};

mod reachable_values;
pub use reachable_values::{
    ReachabilityError, ReachableValues, basic_reachability, list_reachability,
};

mod selection_union;
pub use selection_union::{SelectionUnion, SelectionUnionIter, WeightedInterleave};

mod reachability_cache;
pub use reachability_cache::{
    BasicRangeSource, ListRangeSource, ReachabilityCache, ReachabilityCacheError,
    ReachabilityQuery, ReachabilityRange,
};

mod assignment_selection;
pub use assignment_selection::{
    BasicEntitySelection, BasicEntitySource, BasicSelectionContext, BasicValueContext,
    BasicValueSource, GatedSelectionSource, ListSelectionContext, ListValueSelection,
    ListValueSource,
};

mod revision_selection;
pub use revision_selection::{RevisionRefresh, RevisionSelectionIter, RevisionedSelection};

mod provider_construction;
pub use provider_construction::{
    ProviderConstructionConfig, ProviderConstructionResult, ProviderConstructionTermination,
    construction_from_with_provider, list_construction_from_with_provider,
    list_stream_construction_from_with_provider, mixed_construction_from_with_provider,
    mixed_stream_construction_from_with_provider, stream_construction_from_with_provider,
};

mod placement_queue;
pub use placement_queue::{
    PlacementError, PlacementFn, PlacementOrder, PlacementQueue, PlacementSnapshot,
    PlacementSource, PooledPlacements, SinglePlacementKey, UngroupedPlacement, pooled_placements,
};

mod unique_sampling;
pub use unique_sampling::{
    UniqueIndices, UniqueRangeSamples, UniqueSampleError, UniqueSliceSamples, unique_indices,
    unique_range_samples, unique_slice_samples,
};

mod unique_pair_stream;
pub use unique_pair_stream::UniquePairNode;

mod tuple_collector;
pub use tuple_collector::DistinctCountCollector;

pub use stream_graph::{SharedExistence, SharedJoin};

pub use stream_graph::{
    SharedCollect, SharedCollectList, SharedGroup, SharedGroupKeys, SharedGroupList,
};

pub use stream_graph::{SharedConcat, SharedDistinct, SharedPaddedConcat};

mod precompute_stream;
pub use precompute_stream::PrecomputeNode;
pub use stream_graph::SharedPrecompute;

mod basic_neighborhood;
pub use basic_neighborhood::{
    BasicNeighborhood, BasicNeighborhoodError, basic_moves_configured, basic_moves_configured_union,
};

mod basic_value_order;
pub use basic_value_order::{BasicValueOrder, BasicValueOrderError};

mod temporary_moves;
pub use temporary_moves::{
    RecordedMove, RecordedTemporaryResult, ReplayDirection, RestoredScorePolicy, StaleMoveRecord,
    TemporaryMoveError, TemporaryMoveOptions, TemporaryMoves, TrialFailurePolicy,
};

mod move_session;
pub use move_session::MoveSession;

mod move_inspection;
pub use move_inspection::{MoveInspection, inspect_list_move, inspect_mixed_move};

mod list_cascade_stream;
pub use list_cascade_stream::{ListCascadeStep, ListCascadeStream, list_cascade_stream};

mod basic_inverse_stream;
pub use basic_inverse_stream::{BasicInverseStream, basic_inverse_stream};

mod working_object_lookup;
pub use working_object_lookup::{
    PlanningIdMember, WorkingLookupError, WorkingLookupSchema, WorkingLookupStrategy,
    WorkingObjectLookup,
};

mod named_statistics;
pub use named_statistics::{
    NamedProblemSize, ProblemScale, StatisticsEntityType, StatisticsSchemaError,
    StatisticsVariable, StatisticsVariableKind,
};

mod managed_value_ranges;
pub use managed_value_ranges::{
    ManagedRangeError, ManagedRangeKind, ManagedRangeOrder, ManagedRangeTarget, ManagedValueRanges,
};

mod managed_range_statistics;
pub use managed_range_statistics::{ManagedRangeStatistics, RangeInitializationStatistics};

mod planning_schema;
pub use planning_schema::{
    PlanningEntityDefinition, PlanningEntityRef, PlanningSchema, PlanningSchemaError, PlanningType,
    PlanningVariableDefinition, PlanningVariableKind, PlanningVariableRef,
};

mod source_path;
pub use source_path::{
    CompiledSourcePath, SourceMemberRole, SourceParent, SourcePathError, SourcePathRegistry,
    SourceReference,
};

mod variable_state;
pub use variable_state::{
    BasicVariableState, ListVariableState, MissingVariableEvent, VariableEventTracker,
    VariableEventViolation, VariableListLocation, VariableStateError, VariableStateRegistry,
};

mod list_shadow_session;
pub use list_shadow_session::{
    ListShadowCheckpoint, ListShadowContext, ListShadowError, ListShadowSession,
};

mod list_state_supply;
pub use list_state_supply::{
    ListBuiltinChange, ListBuiltinValue, ListBuiltinValues, ListPinPolicy, ListShadowProperty,
    ListStateSupply, ListStateUpdate, ListSupplyError,
};

mod basic_state_supply;
pub use basic_state_supply::{
    BasicInverseChange, BasicStateSupply, BasicStateUpdate, BasicSupplyError,
};

mod shadow_session;
pub use shadow_session::{
    ShadowInputs, ShadowNode, ShadowNodeRef, ShadowRead, ShadowSession, ShadowSessionBuilder,
    ShadowSessionCheckpoint, ShadowSessionError,
};

mod list_change_recording;
pub use list_change_recording::{
    ListChangeNotification, ListChangeRecorder, ListRecordingError, ListRecordingUndo,
};

mod solution_tracker;
pub use solution_tracker::{
    SolutionCorruptionReport, SolutionEventViolation, SolutionSnapshot, SolutionTracker,
    SolutionTrackingError, VariableValueViolation, format_tracking_list,
};

mod sorted_construction;
pub use sorted_construction::{
    SortedBasicConstruction, SortedListConstruction, StepSortedBasicConstruction,
    StepSortedListConstruction,
};

mod mixed_construction_provider;
pub use mixed_construction_provider::MixedConstructionProvider;

mod score_consistency;
pub use score_consistency::{ConstraintScoreDifference, ScoreConsistencyReport, ScoreCorruption};

mod constraint_assertion;
pub use constraint_assertion::{
    ConstraintAssertion, ConstraintAssertionError, ConstraintComparison, ConstraintExpectation,
    JustificationMatch, TrackedConstraint, constraint_assertion, track_constraint,
    track_decimal_constraint,
};

mod score_assertion;
pub use score_assertion::{ScoreAssertion, ScoreAssertionError, score_assertion};

mod stream_consistency;
pub use stream_consistency::{
    StreamConsistencyReport, StreamNodeDifference, StreamScoreCorruption,
};

mod verification_consistency;
pub use verification_consistency::{
    MissingVerificationEntity, VerificationConsistency, VerificationConsistencyError,
    VerificationEntity, VerificationShadow, VerificationShadowRef, verification_consistency,
};

mod lazy_concat;
pub use lazy_concat::{LazyConcat, LazyConcatError};

mod candidate_index;
pub use candidate_index::{
    CandidateBucket, CandidateIndex, CandidateIndexError, IndexedCandidate, repeating_candidates,
};

mod membership_candidate_index;
pub use membership_candidate_index::{MembershipCandidateIndex, MembershipMatches};

mod neighborhood_tester;
pub use neighborhood_tester::{NeighborhoodAssertionError, NeighborhoodTester};

mod collector_session;
pub use collector_session::{CollectorHandle, CollectorSession, CollectorSessionError};

pub use list_lifecycle::ListNotificationScope;

pub use list_k_opt::list_k_opt_edge_move;

mod comparison_candidates;
pub use comparison_candidates::{CandidateSide, comparison_candidates};

mod phase_best_buffer;
pub use phase_best_buffer::{BufferedSolverUpdate, PhaseBestBuffer};

mod problem_reconciliation;
pub use problem_reconciliation::{AssignmentSnapshot, ReconciliationError, reconcile_problem};

mod list_reconciliation;
pub use list_reconciliation::{
    ListAssignmentSnapshot, ListReconciliationError, reconcile_list_problem,
};

mod directed_two_opt;
pub use directed_two_opt::list_directed_two_opt_plan;

mod directed_two_opt_provider;
pub use directed_two_opt_provider::{DirectedTwoOptProvider, list_directed_two_opt_moves};

mod list_destination_index;
pub use list_destination_index::{ListDestinationError, ListDestinationIndex, ListDestinationSet};
