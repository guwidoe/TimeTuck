//! Value-driven assignment, relocation and unassignment of list elements.
use crate::{
    ListAssignment, ListError, ListMove, ListPosition, ListProblem, MixedAssignment, MixedMove,
    MixedProblem, Move, MoveProvider, SampleRandom, list_destinations,
};
use std::num::NonZeroUsize;
mod correlated;
pub use correlated::{
    CorrelatedListChangeLimits, CorrelatedListValueChangeSource, ListDestinationFactory,
};
mod source;
pub use source::{ListChangeSourceLimits, ListSelectionView, ListValueChangeSource};

/// Pair every value with every destination, or zip independently pin-filtered
/// streams. Both modes are finite and retain duplicate input occurrences.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListChangePairing {
    Cartesian,
    Paired,
}

/// Snapshot candidate retaining the selected PRE-removal destination. Its move
/// uses native POST-removal indices, so adjacent destinations can produce the
/// same no-op while retaining different selection metadata. Reopen selection
/// after changing the assignment; these positions do not follow later edits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListValueChange<V> {
    value: V,
    source: Option<ListPosition>,
    destination: Option<ListPosition>,
    movement: ListMove<V>,
}
impl<V> ListValueChange<V> {
    pub fn value(&self) -> &V {
        &self.value
    }
    pub fn source(&self) -> Option<ListPosition> {
        self.source
    }
    pub fn destination(&self) -> Option<ListPosition> {
        self.destination
    }
    pub fn movement(&self) -> &ListMove<V> {
        &self.movement
    }
    pub fn into_move(self) -> ListMove<V> {
        self.movement
    }
}
struct ValueChangeSelection<'a, V> {
    problem: &'a ListProblem<V>,
    assignment: &'a ListAssignment<V>,
    locations: Vec<Option<ListPosition>>,
    checker: crate::ListMoveChecker<'a, V>,
}
impl<'a, V: Clone + Eq> ValueChangeSelection<'a, V> {
    fn new(
        problem: &'a ListProblem<V>,
        assignment: &'a ListAssignment<V>,
    ) -> Result<Self, ListError> {
        let checker = problem.move_checker(assignment)?;
        Ok(Self {
            checker,
            problem,
            assignment,
            locations: problem.value_locations(assignment),
        })
    }
    fn movable(&self, p: ListPosition) -> bool {
        !self.problem.entities()[p.entity].is_pinned()
            && p.index >= self.problem.entities()[p.entity].pinned_prefix()
    }
    fn values(&self, values: &[V]) -> Result<Vec<usize>, ListError> {
        values
            .iter()
            .map(|v| {
                self.problem
                    .value_index(v)
                    .ok_or(ListError::ValueOutsideDomain)
            })
            .collect()
    }
    fn sources(&self, values: &[V]) -> Result<Vec<usize>, ListError> {
        Ok(self
            .values(values)?
            .into_iter()
            .filter(|&i| self.locations[i].is_none_or(|p| self.movable(p)))
            .collect())
    }
    fn destination_selectable(&self, position: Option<ListPosition>) -> Result<bool, ListError> {
        if let Some(position) = position {
            let row = self
                .assignment
                .list(position.entity)
                .ok_or(ListError::EntityIndex {
                    entity: position.entity,
                    count: self.problem.entities().len(),
                })?;
            if position.index > row.len() {
                return Err(ListError::InsertionIndex(position));
            }
            Ok(self.movable(position))
        } else if self.problem.allows_unassigned() {
            Ok(true)
        } else {
            Err(ListError::UnassignmentNotAllowed)
        }
    }
    fn destinations(
        &self,
        positions: &[Option<ListPosition>],
    ) -> Result<Vec<Option<ListPosition>>, ListError> {
        let mut out = vec![];
        for &position in positions {
            if self.destination_selectable(position)? {
                out.push(position);
            }
        }
        Ok(out)
    }
    fn candidate(
        &self,
        id: usize,
        destination: Option<ListPosition>,
        include_noops: bool,
    ) -> Option<ListValueChange<V>> {
        let source = self.locations[id];
        let value = &self.problem.values()[id];
        if let Some(to) = destination
            && !self.problem.accepts(to.entity, value)
        {
            return None;
        }
        let (movement, unchanged) = match (source, destination) {
            (None, None) => (ListMove::Composite(vec![]), true),
            (None, Some(destination)) => (
                ListMove::Assign {
                    value: value.clone(),
                    destination,
                },
                false,
            ),
            (Some(from), None) => (
                ListMove::Unassign {
                    entity: from.entity,
                    start: from.index,
                    end: from.index + 1,
                },
                false,
            ),
            (Some(from), Some(to)) => {
                let adjusted = ListPosition {
                    entity: to.entity,
                    index: to.index
                        - usize::from(from.entity == to.entity && from.index < to.index),
                };
                (
                    ListMove::Change {
                        source: from,
                        destination: adjusted,
                    },
                    from == adjusted,
                )
            }
        };
        if unchanged && !include_noops {
            return None;
        }
        // Existing include_noops handling decides whether unchanged moves survive.
        self.checker.is_doable(&movement).ok()?;
        Some(ListValueChange {
            value: value.clone(),
            source,
            destination,
            movement,
        })
    }
    fn anchored(
        &self,
        id: usize,
        anchors: &[usize],
        include_unassigned: bool,
    ) -> Vec<Option<ListPosition>> {
        let value = &self.problem.values()[id];
        let mut out = vec![];
        for (entity, meta) in self.problem.entities().iter().enumerate() {
            if !meta.is_pinned() && self.problem.accepts(entity, value) {
                out.push(Some(ListPosition {
                    entity,
                    index: meta.pinned_prefix(),
                }));
            }
        }
        for &anchor in anchors {
            if anchor == id {
                continue;
            }
            if let Some(at) = self.locations[anchor]
                && self.movable(at)
                && self.problem.accepts(at.entity, value)
            {
                out.push(Some(ListPosition {
                    entity: at.entity,
                    index: at.index + 1,
                }));
            }
        }
        if include_unassigned {
            out.push(None);
        }
        out
    }
}

/// Select assignment/relocation/unassignment candidates using explicit value and
/// PRE-removal destination orders. Required missing values can be assigned. None
/// destinations require optional lists. Opening rejects unknown values or invalid
/// positions before returning an iterator; pins are filtered on each input before
/// pairing. Receiver ranges and dependency constraints filter complete candidates.
/// `include_noops` retains adjacent same-list positions and unassigned-to-unassigned.
/// Each requested candidate validates a temporary assignment; the product is lazy.
pub fn list_value_changes<'a, V: Clone + Eq>(
    problem: &'a ListProblem<V>,
    assignment: &'a ListAssignment<V>,
    values: &[V],
    destinations: &[Option<ListPosition>],
    pairing: ListChangePairing,
    include_noops: bool,
) -> Result<impl Iterator<Item = ListValueChange<V>> + 'a, ListError> {
    let selection = ValueChangeSelection::new(problem, assignment)?;
    let values = selection.sources(values)?;
    let destinations = selection.destinations(destinations)?;
    let (mut i, mut j) = (0, 0);
    Ok(std::iter::from_fn(move || {
        while i < values.len() && j < destinations.len() {
            let (v, d) = (values[i], destinations[j]);
            match pairing {
                ListChangePairing::Cartesian => {
                    j += 1;
                    if j == destinations.len() {
                        j = 0;
                        i += 1;
                    }
                }
                ListChangePairing::Paired => {
                    i += 1;
                    j += 1;
                }
            }
            if let Some(c) = selection.candidate(v, d, include_noops) {
                return Some(c);
            }
        }
        None
    }))
}

/// For each selected value, visit eligible entity starts (at their pinned prefix)
/// in model order, then positions immediately after movable assigned anchors in
/// `anchors` order, then optional unassignment. The moved value never anchors its
/// own destination. Missing/pinned anchors contribute nothing. Duplicate anchors
/// retain duplicate positions. Caller-supplied orders are checked on opening.
///
/// ```
/// use timetuck::*;
/// let p=ListProblem::new(vec![1,2,3],vec![ListEntity::new(vec![2,1]),ListEntity::new(vec![3])])?;
/// let a=p.initial_assignment();
/// let moves:Vec<_>=list_anchored_value_changes(&p,&a,&[2],&[1,2,3],false,false)?.collect();
/// assert_eq!(moves.len(),3);
/// # Ok::<(),ListError>(())
/// ```
pub fn list_anchored_value_changes<'a, V: Clone + Eq>(
    problem: &'a ListProblem<V>,
    assignment: &'a ListAssignment<V>,
    values: &[V],
    anchors: &[V],
    include_unassigned: bool,
    include_noops: bool,
) -> Result<impl Iterator<Item = ListValueChange<V>> + 'a, ListError> {
    let selection = ValueChangeSelection::new(problem, assignment)?;
    if include_unassigned && !problem.allows_unassigned() {
        return Err(ListError::UnassignmentNotAllowed);
    }
    let values = selection.sources(values)?;
    let anchors = selection.values(anchors)?;
    let mut values = values.into_iter();
    let mut current = None;
    let mut destinations = vec![].into_iter();
    Ok(std::iter::from_fn(move || {
        loop {
            if let Some(d) = destinations.next() {
                if let Some(c) =
                    selection.candidate(current.expect("selected source"), d, include_noops)
                {
                    return Some(c);
                }
            } else {
                let id = values.next()?;
                current = Some(id);
                destinations = selection
                    .anchored(id, &anchors, include_unassigned)
                    .into_iter();
            }
        }
    }))
}

/// Draw independent source values and PRE-removal slots with replacement. Sources
/// include missing values; slots include all unpinned insertions and optionally
/// None. Invalid/no-op pairs consume attempts, bounding work even with no valid
/// move. Opening and empty/singleton draws do not consume randomness. Distribution
/// is uniform over input pairs, not over accepted moves or distinct successors.
pub fn sample_list_value_changes<'a, V: Clone + Eq>(
    problem: &'a ListProblem<V>,
    assignment: &'a ListAssignment<V>,
    random: &'a mut SampleRandom,
    attempts: usize,
    include_unassigned: bool,
    include_noops: bool,
) -> Result<impl Iterator<Item = ListValueChange<V>> + 'a, ListError> {
    let selection = ValueChangeSelection::new(problem, assignment)?;
    let values = selection.sources(problem.values())?;
    let destinations: Vec<_> =
        list_destinations(problem, assignment, include_unassigned)?.collect();
    let mut remaining = attempts;
    Ok(std::iter::from_fn(move || {
        let n = NonZeroUsize::new(values.len())?;
        let m = NonZeroUsize::new(destinations.len())?;
        while remaining > 0 {
            remaining -= 1;
            let v = if n.get() == 1 { 0 } else { random.index(n) };
            let d = if m.get() == 1 { 0 } else { random.index(m) };
            if let Some(c) = selection.candidate(values[v], destinations[d], include_noops) {
                return Some(c);
            }
        }
        None
    }))
}

/// Rebuild anchored destinations against each search-step assignment. Usable in
/// both construction and local-search provider bindings. Mixed candidates leave
/// basic variables unchanged. All configured values/anchors must be registered.
#[derive(Clone, Debug)]
pub struct ListValueChangeProvider<V> {
    pub values: Vec<V>,
    pub anchors: Vec<V>,
    pub include_unassigned: bool,
    pub include_noops: bool,
}
impl<V: Clone + Eq> MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>
    for ListValueChangeProvider<V>
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
            list_anchored_value_changes(
                p,
                a,
                &self.values,
                &self.anchors,
                self.include_unassigned,
                self.include_noops,
            )
            .expect("change provider requires valid model and selection")
            .map(ListValueChange::into_move),
        )
    }
}
impl<B: Clone + Eq, V: Clone + Eq>
    MoveProvider<MixedProblem<B, V>, MixedAssignment<B, V>, MixedMove<B, V>>
    for ListValueChangeProvider<V>
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
            .expect("change provider requires valid mixed assignment");
        Box::new(
            list_anchored_value_changes(
                p.lists(),
                a.lists(),
                &self.values,
                &self.anchors,
                self.include_unassigned,
                self.include_noops,
            )
            .expect("change provider requires valid selection")
            .map(|c| MixedMove {
                basic: Move::Composite(vec![]),
                lists: c.into_move(),
            }),
        )
    }
}

/// Bounded, range-aware sampling over configured source occurrences and ordered
/// entity starts/value anchors. Source pins are filtered before drawing. Receiver
/// ranges are filtered for each selected source before drawing its destination.
/// Empty destination sets, rejected dependencies and excluded no-ops consume an
/// attempt. An empty sample is not a proof of a local optimum.
///
/// Sampling is uniform over source occurrences, then eligible destination
/// occurrences, not over all valid moves. Duplicate inputs retain weight. Opening
/// validates all configured identities. Each opening owns a PRNG initialized with
/// seed+completed_steps (wrapping), so partial consumption does not alter replays.
/// Native sampling traces are distinct from Java's selector mixing protocol.
#[derive(Clone, Debug)]
pub struct SampledAnchoredListChangeProvider<V> {
    pub values: Vec<V>,
    pub entities: Vec<usize>,
    pub anchors: Vec<V>,
    pub include_unassigned: bool,
    pub include_noops: bool,
    pub seed: u64,
    pub attempts_per_step: u64,
}
impl<V: Clone + Eq> SampledAnchoredListChangeProvider<V> {
    pub fn select<'a>(
        &self,
        problem: &'a ListProblem<V>,
        assignment: &'a ListAssignment<V>,
        completed_steps: u64,
    ) -> Result<impl Iterator<Item = ListValueChange<V>> + use<'a, V>, ListError> {
        let selection = ValueChangeSelection::new(problem, assignment)?;
        let values = selection.sources(&self.values)?;
        let destinations = crate::ListDestinationSelector::new(
            problem,
            assignment,
            &self.entities,
            &self.anchors,
            self.include_unassigned,
        )?;
        let mut random = SampleRandom::new(self.seed.wrapping_add(completed_steps));
        let mut remaining = self.attempts_per_step;
        let include_noops = self.include_noops;
        Ok(std::iter::from_fn(move || {
            let n = NonZeroUsize::new(values.len())?;
            while remaining > 0 {
                remaining -= 1;
                let id = values[if n.get() == 1 { 0 } else { random.index(n) }];
                let choices = destinations
                    .for_values(std::slice::from_ref(&problem.values()[id]))
                    .expect("registered source");
                if let Some(destination) = choices.sample(&mut random)
                    && let Some(candidate) = selection.candidate(id, destination, include_noops)
                {
                    return Some(candidate);
                }
            }
            None
        })
        .fuse())
    }
}
impl<V: Clone + Eq> MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>
    for SampledAnchoredListChangeProvider<V>
{
    fn moves<'a>(
        &'a mut self,
        p: &'a ListProblem<V>,
        a: &'a ListAssignment<V>,
        step: u64,
    ) -> Box<dyn Iterator<Item = ListMove<V>> + 'a>
    where
        ListMove<V>: 'a,
    {
        Box::new(
            self.select(p, a, step)
                .expect("sampled anchored provider requires valid configuration")
                .map(ListValueChange::into_move),
        )
    }
}
impl<B: Clone + Eq, V: Clone + Eq>
    MoveProvider<MixedProblem<B, V>, MixedAssignment<B, V>, MixedMove<B, V>>
    for SampledAnchoredListChangeProvider<V>
{
    fn moves<'a>(
        &'a mut self,
        p: &'a MixedProblem<B, V>,
        a: &'a MixedAssignment<B, V>,
        step: u64,
    ) -> Box<dyn Iterator<Item = MixedMove<B, V>> + 'a>
    where
        MixedMove<B, V>: 'a,
    {
        p.validate(a)
            .expect("sampled anchored provider requires valid mixed assignment");
        Box::new(
            self.select(p.lists(), a.lists(), step)
                .expect("sampled anchored provider requires valid configuration")
                .map(|candidate| MixedMove {
                    basic: Move::Composite(vec![]),
                    lists: candidate.into_move(),
                }),
        )
    }
}
