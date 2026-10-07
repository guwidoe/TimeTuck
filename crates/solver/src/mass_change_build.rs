//! Construct and rebase common-destination basic moves with stable member identity.
use crate::{MassChange, MoveRebaseError, MoveRebaseNamespace};
use std::collections::BTreeSet;

/// Construct a common-destination move from explicit variable slots. Duplicate
/// slots occur once, in first-appearance order. Empty and singleton groups are
/// supported; empty groups execute as no-ops but retain destination metadata.
/// Pin/domain checks occur when the model applies the resulting ordinary move.
/// Equality and hashing ignore member draw order while keeping execution order.
pub fn mass_change_from_members<V: Clone>(
    members: impl IntoIterator<Item = usize>,
    destination: Option<V>,
) -> MassChange<V> {
    let mut seen = BTreeSet::new();
    MassChange::new(
        members.into_iter().filter(|i| seen.insert(*i)).collect(),
        destination,
    )
}

/// Rebase explicit slot identities and the common destination. Each member is
/// looked up once in stored order, followed by the non-null destination once,
/// including for an empty group. Null never invokes the value lookup. Colliding
/// destination slots are rejected. No model or assignment is mutated; external
/// lookup callback effects are not rolled back. The target model validates the
/// resulting move on application.
pub fn rebase_mass_change<V, U: Clone, E>(
    movement: &MassChange<V>,
    mut variable: impl FnMut(usize) -> Result<usize, E>,
    mut value: impl FnMut(&V) -> Result<U, E>,
) -> Result<MassChange<U>, MoveRebaseError<E>> {
    let mut seen = BTreeSet::new();
    let mut members = Vec::with_capacity(movement.members().len());
    for &source in movement.members() {
        let target = variable(source).map_err(MoveRebaseError::Lookup)?;
        if !seen.insert(target) {
            return Err(MoveRebaseError::Collision(
                MoveRebaseNamespace::BasicVariable,
            ));
        }
        members.push(target);
    }
    let destination = movement
        .destination()
        .map(&mut value)
        .transpose()
        .map_err(MoveRebaseError::Lookup)?;
    Ok(MassChange::new(members, destination))
}
