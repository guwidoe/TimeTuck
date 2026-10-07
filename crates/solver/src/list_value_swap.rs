//! Ordered and sampled value-driven list swaps over an immutable assignment.
use crate::{
    ListAssignment, ListError, ListMove, ListPosition, ListProblem, MixedAssignment, MixedMove,
    MixedProblem, Move, MoveProvider, SampleRandom,
};
use std::num::NonZeroUsize;

/// Pair each selected left value with every right value, or consume the two
/// filtered streams in lockstep. Paired stops when either stream ends. Pinning
/// and required-unassigned filtering happen independently before pairing; range
/// and dependency filtering happen afterward. Duplicates retain their multiplicity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListSwapPairing {
    Cartesian,
    Paired,
}

struct ValueSwapSelection<'a, V> {
    problem: &'a ListProblem<V>,
    assignment: &'a ListAssignment<V>,
    locations: Vec<Option<ListPosition>>,
    selectable: Vec<bool>,
}
impl<'a, V: Eq + Clone> ValueSwapSelection<'a, V> {
    fn new(
        problem: &'a ListProblem<V>,
        assignment: &'a ListAssignment<V>,
    ) -> Result<Self, ListError> {
        problem.validate(assignment)?;
        let locations: Vec<_> = problem
            .values()
            .iter()
            .map(|v| assignment.location(v))
            .collect();
        let selectable = locations
            .iter()
            .map(|p| match p {
                None => problem.allows_unassigned(),
                Some(p) => {
                    !problem.entities()[p.entity].is_pinned()
                        && p.index >= problem.entities()[p.entity].pinned_prefix()
                }
            })
            .collect();
        Ok(Self {
            problem,
            assignment,
            locations,
            selectable,
        })
    }
    fn order(&self, values: &[V]) -> Result<Vec<usize>, ListError> {
        let mut result = Vec::with_capacity(values.len());
        for v in values {
            let id = self
                .problem
                .values()
                .iter()
                .position(|x| x == v)
                .ok_or(ListError::ValueOutsideDomain)?;
            if self.selectable[id] {
                result.push(id);
            }
        }
        Ok(result)
    }
    fn movement(&self, left: usize, right: usize, include_noops: bool) -> Option<ListMove<V>> {
        let p = self.problem;
        let movement = match (self.locations[left], self.locations[right]) {
            (None, None) => return include_noops.then(|| ListMove::Composite(vec![])),
            (Some(l), Some(r)) if l == r => {
                return include_noops.then(|| ListMove::Composite(vec![]));
            }
            (Some(l), Some(r)) => {
                if !p.accepts(l.entity, &p.values()[right])
                    || !p.accepts(r.entity, &p.values()[left])
                {
                    return None;
                }
                ListMove::Swap { left: l, right: r }
            }
            (Some(at), None) | (None, Some(at)) => {
                let value = &p.values()[if self.locations[left].is_none() {
                    left
                } else {
                    right
                }];
                if !p.accepts(at.entity, value) {
                    return None;
                }
                ListMove::Composite(vec![
                    ListMove::Unassign {
                        entity: at.entity,
                        start: at.index,
                        end: at.index + 1,
                    },
                    ListMove::Assign {
                        value: value.clone(),
                        destination: at,
                    },
                ])
            }
        };
        // Dependency constraints may reject a geometrically valid swap. Prepare
        // only the requested candidate; no Cartesian product is materialized.
        let mut trial = self.assignment.clone();
        drop(p.apply(&mut trial, &movement).ok()?);
        Some(movement)
    }
}

/// Value-driven swaps in caller-supplied order, including optional assigned/
/// unassigned replacement. Unknown values or invalid assignments fail at opening.
/// No candidates are built until iteration. Opening caches locations and filtered
/// orders; each emitted nontrivial candidate validates a temporary assignment.
/// `include_noops` retains equal-value and two-unassigned pairs as empty composites.
/// Reverse pairs and duplicate inputs remain distinct candidates. Output indices
/// belong only to this borrowed assignment snapshot.
///
/// ```
/// use timetuck::*;
/// let p=ListProblem::new(vec![1,2,3],vec![ListEntity::new(vec![2,1])])?.allowing_unassigned();
/// let a=p.initial_assignment();
/// let moves=list_value_swap_moves(&p,&a,&[3],&[1,2],ListSwapPairing::Cartesian,false)?;
/// assert_eq!(moves.count(),2);
/// # Ok::<(),ListError>(())
/// ```
pub fn list_value_swap_moves<'a, V: Clone + Eq>(
    problem: &'a ListProblem<V>,
    assignment: &'a ListAssignment<V>,
    left: &[V],
    right: &[V],
    pairing: ListSwapPairing,
    include_noops: bool,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    let selection = ValueSwapSelection::new(problem, assignment)?;
    let left = selection.order(left)?;
    let right = selection.order(right)?;
    let (mut i, mut j) = (0, 0);
    Ok(std::iter::from_fn(move || {
        while i < left.len() && j < right.len() {
            let pair = (left[i], right[j]);
            match pairing {
                ListSwapPairing::Cartesian => {
                    j += 1;
                    if j == right.len() {
                        j = 0;
                        i += 1;
                    }
                }
                ListSwapPairing::Paired => {
                    i += 1;
                    j += 1;
                }
            }
            if let Some(m) = selection.movement(pair.0, pair.1, include_noops) {
                return Some(m);
            }
        }
        None
    }))
}

/// Draw at most `attempts` independent ordered pairs with replacement from all
/// selectable registered values. Rejected pairs consume attempts, so unreachable
/// ranges always terminate. Empty/singleton selections consume no randomness.
/// This samples pairs, not uniformly from the set of valid resulting moves.
/// Opening consumes no randomness; the caller owns the reproducible random stream.
pub fn sample_list_value_swaps<'a, V: Clone + Eq>(
    problem: &'a ListProblem<V>,
    assignment: &'a ListAssignment<V>,
    random: &'a mut SampleRandom,
    attempts: usize,
    include_noops: bool,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    let selection = ValueSwapSelection::new(problem, assignment)?;
    let values: Vec<_> = selection
        .selectable
        .iter()
        .enumerate()
        .filter_map(|(i, &yes)| yes.then_some(i))
        .collect();
    let mut remaining = attempts;
    Ok(std::iter::from_fn(move || {
        let count = NonZeroUsize::new(values.len())?;
        while remaining > 0 {
            remaining -= 1;
            let l = if values.len() == 1 {
                0
            } else {
                random.index(count)
            };
            let r = if values.len() == 1 {
                0
            } else {
                random.index(count)
            };
            if let Some(m) = selection.movement(values[l], values[r], include_noops) {
                return Some(m);
            }
        }
        None
    }))
}

/// Reopen ordered value selection on each search step. Values must remain in the
/// model registry. Use with list/mixed provider APIs or phase provider bindings;
/// mixed moves leave basic variables unchanged. Selection is finite and stores
/// only the supplied orders and one assignment's index, not all candidate moves.
#[derive(Clone, Debug)]
pub struct ListValueSwapProvider<V> {
    pub left: Vec<V>,
    pub right: Vec<V>,
    pub pairing: ListSwapPairing,
    pub include_noops: bool,
}
impl<V: Clone + Eq> MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>
    for ListValueSwapProvider<V>
{
    fn moves<'a>(
        &'a mut self,
        p: &'a ListProblem<V>,
        a: &'a ListAssignment<V>,
        _: u64,
    ) -> Box<dyn Iterator<Item = ListMove<V>> + 'a>
    where
        ListMove<V>: 'a,
    {
        Box::new(
            list_value_swap_moves(
                p,
                a,
                &self.left,
                &self.right,
                self.pairing,
                self.include_noops,
            )
            .expect("value swap provider requires valid model and registered values"),
        )
    }
}
impl<B: Clone + Eq, V: Clone + Eq>
    MoveProvider<MixedProblem<B, V>, MixedAssignment<B, V>, MixedMove<B, V>>
    for ListValueSwapProvider<V>
{
    fn moves<'a>(
        &'a mut self,
        p: &'a MixedProblem<B, V>,
        a: &'a MixedAssignment<B, V>,
        _: u64,
    ) -> Box<dyn Iterator<Item = MixedMove<B, V>> + 'a>
    where
        MixedMove<B, V>: 'a,
    {
        p.validate(a)
            .expect("value swap provider requires a valid mixed assignment");
        Box::new(
            list_value_swap_moves(
                p.lists(),
                a.lists(),
                &self.left,
                &self.right,
                self.pairing,
                self.include_noops,
            )
            .expect("value swap provider requires registered values")
            .map(|lists| MixedMove {
                basic: Move::Composite(vec![]),
                lists,
            }),
        )
    }
}
