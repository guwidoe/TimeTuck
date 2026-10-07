//! Identity-based list gathers resolved against the assignment at execution time.
use crate::{
    ListAssignment, ListGatherError, ListMove, ListPosition, ListProblem, MoveRebaseError,
    MoveRebaseNamespace,
};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

/// A list gather recipe containing value identities and an original insertion
/// boundary. Resolve it against the current assignment before applying it; unlike
/// a compiled ListMassChange, it does not retain source indices from an old state.
/// Members are distinct in first-appearance order. Insertion equality preserves
/// that order; unassignment equality ignores it. The destination owner and index
/// remain explicit and are validated by resolve().
#[derive(Clone, Debug)]
pub struct ListMassChangePlan<V> {
    members: Vec<V>,
    destination: Option<ListPosition>,
}
impl<V> ListMassChangePlan<V> {
    pub fn members(&self) -> &[V] {
        &self.members
    }
    pub fn destination(&self) -> Option<ListPosition> {
        self.destination
    }
}
impl<V: Eq> ListMassChangePlan<V> {
    pub fn new(members: impl IntoIterator<Item = V>, destination: Option<ListPosition>) -> Self {
        let mut unique = Vec::new();
        for member in members {
            if !unique.contains(&member) {
                unique.push(member);
            }
        }
        Self {
            members: unique,
            destination,
        }
    }
}
impl<V: Eq + Clone> ListMassChangePlan<V> {
    /// Build a validated ordinary move bound to this assignment. Existing gather
    /// semantics preserve assigned payloads, original-boundary adjustment, pins,
    /// ranges and optional values. Neither plan nor assignment is mutated.
    pub fn resolve(
        &self,
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
    ) -> Result<ListMove<V>, ListGatherError> {
        problem.gather_move(assignment, &self.members, self.destination)
    }
}
impl<V: Eq> PartialEq for ListMassChangePlan<V> {
    fn eq(&self, other: &Self) -> bool {
        self.destination == other.destination
            && if self.destination.is_some() {
                self.members == other.members
            } else {
                self.members.len() == other.members.len()
                    && self.members.iter().all(|v| other.members.contains(v))
            }
    }
}
impl<V: Eq> Eq for ListMassChangePlan<V> {}
impl<V: Hash> Hash for ListMassChangePlan<V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.destination.hash(state);
        if self.destination.is_some() {
            self.members.hash(state);
        } else {
            // Sorting hashes avoids an Ord bound on domain identities. Equal
            // unordered sets hash alike even when different members collide.
            let mut hashes = self
                .members
                .iter()
                .map(|v| {
                    let mut h = DefaultHasher::new();
                    v.hash(&mut h);
                    h.finish()
                })
                .collect::<Vec<_>>();
            hashes.sort_unstable();
            hashes.hash(state);
        }
    }
}
/// Rebase each member once in stored order, then its optional destination owner.
/// Values must remain distinct after mapping. The destination index is preserved;
/// callers select a corresponding target insertion boundary. Null destinations
/// do not invoke the owner lookup. Callback side effects are not rolled back.
pub fn rebase_list_mass_change_plan<V, U: Eq, E>(
    plan: &ListMassChangePlan<V>,
    mut value: impl FnMut(&V) -> Result<U, E>,
    mut entity: impl FnMut(usize) -> Result<usize, E>,
) -> Result<ListMassChangePlan<U>, MoveRebaseError<E>> {
    let mut members = Vec::with_capacity(plan.members.len());
    for source in &plan.members {
        let target = value(source).map_err(MoveRebaseError::Lookup)?;
        if members.contains(&target) {
            return Err(MoveRebaseError::Collision(MoveRebaseNamespace::ListValue));
        }
        members.push(target);
    }
    let destination = plan
        .destination
        .map(|p| {
            entity(p.entity).map(|entity| ListPosition {
                entity,
                index: p.index,
            })
        })
        .transpose()
        .map_err(MoveRebaseError::Lookup)?;
    Ok(ListMassChangePlan {
        members,
        destination,
    })
}
