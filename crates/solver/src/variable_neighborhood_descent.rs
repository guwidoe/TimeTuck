//! First-improvement descent over an ordered neighborhood union.
use crate::{
    Acceptance, LocalSearchConfig, MoveOrder, MoveSelection, NeighborhoodUnion, RejectedMovePolicy,
    UnionOrder,
};

/// Random child selection cannot preserve descent's neighborhood priority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DescentRequiresSequentialOrder;
impl std::fmt::Display for DescentRequiresSequentialOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("variable-neighborhood descent requires sequential neighborhood order")
    }
}
impl std::error::Error for DescentRequiresSequentialOrder {}

/// Configure bounded first-improvement variable-neighborhood descent.
///
/// Scan enabled neighborhoods in order until a strictly improving move is
/// found, install it, then restart at the first neighborhood. Stop when all
/// enabled neighborhoods have been exhausted without improvement, or a search
/// limit is reached. Equal-score moves are rejected. This establishes only a
/// local optimum over the configured candidates when the scan exhausts;
/// sampled neighborhoods and early limits give no global-optimality guarantee.
///
/// Zero weights disable children; positive magnitudes do not affect priority.
/// An empty/all-disabled union stops without moves. Random unions are rejected.
/// All models and both scoring backends use their normal incremental scoring,
/// rollback and shared budgets. Custom providers replace this union, as usual.
///
/// The returned config has the ordinary default budgets. Set limits, score
/// goals, cancellation or assertions with struct update syntax. Overriding
/// acceptance, move order, selection, rejected fallback or the neighborhood
/// union changes the algorithm; the returned config is not an immutable policy.
///
/// ```
/// use timetuck::*;
/// let neighborhoods = NeighborhoodUnion {
///     children: &[(Neighborhood::Change, 1), (Neighborhood::Swap, 1)],
///     order: UnionOrder::Sequential,
/// };
/// let config = LocalSearchConfig::<SimpleScore> {
///     max_steps: 100,
///     ..variable_neighborhood_descent(&neighborhoods)?
/// };
/// let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]).with_initial(0)])?;
/// let mut n = ConstraintNetwork::new(1);
/// n.add_constraint("value", vec![0], |v| SimpleScore::new(*v[0].unwrap()))?;
/// let result = local_search(&p, &n, config)?;
/// assert_eq!(result.best.unwrap().score, SimpleScore::new(1));
/// assert_eq!(result.termination, LocalSearchTermination::NoAcceptedMove);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn variable_neighborhood_descent<'a, S>(
    neighborhoods: &'a NeighborhoodUnion<'a>,
) -> Result<LocalSearchConfig<'a, S>, DescentRequiresSequentialOrder> {
    if neighborhoods.order != UnionOrder::Sequential {
        return Err(DescentRequiresSequentialOrder);
    }
    Ok(LocalSearchConfig {
        neighborhood_union: Some(neighborhoods),
        acceptance: Acceptance::StrictImprovement,
        move_order: MoveOrder::Original,
        selection: MoveSelection::FirstAccepted,
        rejected_moves: RejectedMovePolicy::Stop,
        ..Default::default()
    })
}
