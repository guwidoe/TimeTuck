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
pub use local_search::{
    Acceptance, LocalSearchConfig, LocalSearchError, LocalSearchResult, LocalSearchTermination,
    MoveOrder, MoveSelection, Neighborhood, RejectedMovePolicy, local_search, local_search_from,
};

pub mod acceptance;
pub use acceptance::{AcceptanceError, Acceptor, LateAcceptance};
mod forager;
mod random;
pub use forager::{FinalistRanking, ForagerConfig, InvalidFinalistIndex, MoveForager, TieBreaking};

pub mod construction;
pub use construction::{
    ConstructionConfig, ConstructionPickEarly, ConstructionResult, ConstructionTermination,
    OptionalConstruction, greedy_construction, greedy_construction_from,
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
pub use list::{
    AppliedListMove, ListAssignment, ListElement, ListEntity, ListError, ListMove, ListPosition,
    ListProblem, ListRange,
};
mod list_gather_neighborhood;
pub use list_gather_neighborhood::list_gather_moves;
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
    JoinedStreamPort, StreamConstraint, StreamGraph, StreamGraphBuilder, StreamPort, StreamSource,
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
pub use mixed_neighborhood::mixed_moves;
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
    ExactPhaseConfig, ExactPhaseTermination, MixedSolverPhase, MixedSolverPlan,
    MixedSolverSequence, SolverPhase, SolverPlan, SolverSequence,
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
pub use branch_and_bound::{
    BranchAndBoundConfig, BranchAndBoundResult, BranchEvaluation, BranchTermination,
    InvalidOptimisticBound, NodeExploration, SearchNodeKey, branch_and_bound,
    branch_and_bound_with_observer,
};
