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

pub mod neighborhood;
pub use neighborhood::{change_moves, swap_moves};
pub mod local_search;
pub use local_search::{
    Acceptance, LocalSearchConfig, LocalSearchError, LocalSearchResult, LocalSearchTermination,
    MoveOrder, MoveSelection, Neighborhood, RejectedMovePolicy, local_search, local_search_from,
};

pub mod acceptance;
pub use acceptance::{AcceptanceError, Acceptor, LateAcceptance};
mod random;

pub mod construction;
pub use construction::{
    ConstructionConfig, ConstructionPickEarly, ConstructionResult, ConstructionTermination,
    greedy_construction,
};

pub mod tabu;
pub use tabu::TabuMemory;

pub mod annealing;
pub use annealing::SimulatedAnnealing;

pub mod bendable;
pub use bendable::BendableScore;

pub mod list;
pub use list::{
    AppliedListMove, ListAssignment, ListElement, ListEntity, ListError, ListMove, ListPosition,
    ListProblem,
};
pub mod list_search;
pub use list_search::{ListSearchResult, ScoredListAssignment, exhaustive_list_search};

pub mod list_scoring;
pub use list_scoring::{
    ListConstraintNetwork, ListFact, ListScope, ListScoreDirector, ListScoringError, ScoredListMove,
};

pub mod list_neighborhood;
pub use list_neighborhood::{
    list_assign_moves, list_change_moves, list_moves, list_swap_moves, list_unassign_moves,
};

pub mod list_construction;
pub use list_construction::{ListConstructionResult, greedy_list_construction};
pub mod list_local_search;
pub use list_local_search::{ListLocalSearchError, ListLocalSearchResult, list_local_search_from};

pub mod stream;
pub use stream::{ConstraintStream, IntCollector, IntSummary, StreamScoreError};

mod distinct_stream;
pub use distinct_stream::DistinctNode;
mod existence_stream;
pub use existence_stream::{ExistenceMode, ExistsNode};
pub mod incremental_stream;
pub use incremental_stream::{
    FilterNode, GroupNode, JoinNode, ScoreNode, StructuralStreamContribution, TupleChange,
};

pub mod stream_graph;
pub use stream_graph::{
    GraphError, GraphTransaction, JoinedStreamPort, StreamConstraint, StreamGraph,
    StreamGraphBuilder, StreamPort, StreamSource,
};

pub mod stream_scoring;
pub use stream_scoring::{
    PlanningStreamSource, StreamScoreDirector, StreamScoredMove, StreamScoringError,
    greedy_stream_construction, stream_local_search, stream_local_search_from,
};

mod search_scoring;

pub mod snapshot_map;
pub use snapshot_map::SnapshotMap;
