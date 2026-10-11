//! Explicit selected-value repair candidates for list move providers.
use crate::{ListAssignment, ListEntity, ListError, ListMove, ListPosition, ListProblem};
use std::{error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListRuinPlanError {
    Model(ListError),
    EmptySelection,
    UnknownValue { value: usize, count: usize },
    DuplicateValue { value: usize },
    PinnedValue { value: usize },
    IncompleteInitialAssignment,
    IncompleteReconstruction,
    UnselectedValuesChanged { entity: usize },
}
impl From<ListError> for ListRuinPlanError {
    fn from(error: ListError) -> Self {
        Self::Model(error)
    }
}
impl fmt::Display for ListRuinPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(error) => error.fmt(f),
            _ => write!(f, "invalid list ruin plan: {self:?}"),
        }
    }
}
impl Error for ListRuinPlanError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Model(error) => Some(error),
            _ => None,
        }
    }
}

/// Prepare an explicit ruin/recreate candidate without changing the caller's state.
///
/// Values are selected by registry index, independently of their current positions.
/// The repair model retains registry order, ranges, dependencies and pins, removes
/// selected assigned values, and excludes unselected missing values from all ranges.
/// Use its initial assignment with ordinary or stream list construction, or a custom
/// repair algorithm. Required models need a complete input and reconstruction;
/// optional selected values may remain missing or become newly assigned.
///
/// Finishing checks that unselected values retain owners and relative order. The
/// resulting ordinary [`ListMove`] edits only selected values and is bound to the
/// input snapshot, like [`crate::ListMoveBuilder`]. Score it against the original
/// model with [`crate::TemporaryMoves`] to evaluate and undo a candidate, or record
/// it there for guarded forward/undo replay. No search policy or scoring cache is
/// retained by the plan. Construction performed inside a custom provider needs
/// its own explicit budget; provider-internal work is not a scored outer proposal.
#[derive(Debug)]
pub struct ListRuinPlan<'a, V> {
    problem: &'a ListProblem<V>,
    before: &'a ListAssignment<V>,
    repair: ListProblem<V>,
    selected: Vec<usize>,
    removed: Vec<Vec<usize>>,
}
impl<'a, V: Eq + Clone> ListRuinPlan<'a, V> {
    pub fn new(
        problem: &'a ListProblem<V>,
        before: &'a ListAssignment<V>,
        selected: &[usize],
    ) -> Result<Self, ListRuinPlanError> {
        problem.validate(before)?;
        if !problem.allows_unassigned() && population(before) != problem.values().len() {
            return Err(ListRuinPlanError::IncompleteInitialAssignment);
        }
        if selected.is_empty() {
            return Err(ListRuinPlanError::EmptySelection);
        }
        let mut seen = vec![false; problem.values().len()];
        let locations = problem.value_locations(before);
        let mut removed = vec![vec![]; problem.entities().len()];
        for &value in selected {
            let count = seen.len();
            let seen = seen
                .get_mut(value)
                .ok_or(ListRuinPlanError::UnknownValue { value, count })?;
            if std::mem::replace(seen, true) {
                return Err(ListRuinPlanError::DuplicateValue { value });
            }
            if let Some(at) = locations[value] {
                let entity = &problem.entities()[at.entity];
                if entity.is_pinned() || at.index < entity.pinned_prefix() {
                    return Err(ListRuinPlanError::PinnedValue { value });
                }
                removed[at.entity].push(at.index);
            }
        }
        for row in &mut removed {
            row.sort_unstable();
        }
        let repair = repair_model(problem, before, selected)?;
        Ok(Self {
            problem,
            before,
            repair,
            selected: selected.to_vec(),
            removed,
        })
    }

    /// Independent model suitable for existing list construction APIs.
    pub fn repair_problem(&self) -> &ListProblem<V> {
        &self.repair
    }

    /// Convert a reconstruction into an atomic move against the original snapshot.
    ///
    /// Unselected values retain their original payloads, even when `Eq` ignores
    /// fields that differ in supplied reconstruction copies. Selected inserted
    /// values use the supplied payloads. Always score the resulting move against
    /// the original model; a reconstruction's score is not installed or trusted.
    /// Failure leaves both assignments and the plan unchanged and permits retry.
    /// Reconstructing the original ordering may return a no-op candidate; callers
    /// can filter it with [`ListProblem::is_move_doable`] if their provider requires
    /// every proposal to change the assignment.
    pub fn finish(&self, rebuilt: &ListAssignment<V>) -> Result<ListMove<V>, ListRuinPlanError> {
        self.repair.validate(rebuilt)?;
        if !self.problem.allows_unassigned() && population(rebuilt) != self.problem.values().len() {
            return Err(ListRuinPlanError::IncompleteReconstruction);
        }
        let locations = self.problem.value_locations(rebuilt);
        let mut inserted = vec![vec![]; self.problem.entities().len()];
        for &value in &self.selected {
            if let Some(at) = locations[value] {
                inserted[at.entity].push(at.index);
            }
        }
        for (entity, indices) in inserted.iter_mut().enumerate() {
            indices.sort_unstable();
            let mut selected = indices.iter().copied().peekable();
            let kept = rebuilt.lists()[entity]
                .iter()
                .enumerate()
                .filter_map(|(index, value)| {
                    if selected.peek() == Some(&index) {
                        selected.next();
                        None
                    } else {
                        Some(value)
                    }
                });
            if !kept.eq(self.repair.entities()[entity].initial()) {
                return Err(ListRuinPlanError::UnselectedValuesChanged { entity });
            }
        }
        let mut steps = Vec::new();
        for (entity, indices) in self.removed.iter().enumerate() {
            // Reverse contiguous runs retain the original indices during removal.
            let mut remaining = indices.as_slice();
            while let Some((&last, rest)) = remaining.split_last() {
                let mut start = last;
                remaining = rest;
                while remaining.last().is_some_and(|&index| index + 1 == start) {
                    start -= 1;
                    remaining = &remaining[..remaining.len() - 1];
                }
                steps.push(ListMove::Unassign {
                    entity,
                    start,
                    end: last + 1,
                });
            }
        }
        for (entity, indices) in inserted.iter().enumerate() {
            for &index in indices {
                steps.push(ListMove::Assign {
                    value: rebuilt.lists()[entity][index].clone(),
                    destination: ListPosition { entity, index },
                });
            }
        }
        let movement = ListMove::Composite(steps);
        // Check original constraints too: repair pin/range metadata is derived
        // from the supplied snapshot, whereas the move belongs to the original.
        self.problem
            .move_checker(self.before)?
            .is_doable(&movement)?;
        Ok(movement)
    }
}

fn population<V>(assignment: &ListAssignment<V>) -> usize {
    assignment.lists().iter().map(Vec::len).sum()
}

// Shared with seeded ruin/recreate search; callers validate selected identities.
pub(crate) fn repair_model<V: Eq + Clone>(
    problem: &ListProblem<V>,
    a: &ListAssignment<V>,
    selected: &[usize],
) -> Result<ListProblem<V>, ListError> {
    let locations = problem.value_locations(a);
    let mut selected_ids = vec![false; problem.values().len()];
    let mut removed = vec![Vec::new(); problem.entities().len()];
    for &id in selected {
        selected_ids[id] = true;
        if let Some(at) = locations[id] {
            removed[at.entity].push(at.index);
        }
    }
    for row in &mut removed {
        row.sort_unstable();
        row.dedup();
    }
    let entities = problem
        .entities()
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let kept = a.lists()[i]
                .iter()
                .enumerate()
                .filter(|(index, _)| removed[i].binary_search(index).is_err())
                .map(|(_, value)| value.clone())
                .collect();
            // Exclude only unselected missing values; changing the registry would
            // invalidate value-scoped constraints and stream identity indices.
            let range = problem
                .values()
                .iter()
                .enumerate()
                .filter(|(id, _)| {
                    (selected_ids[*id] || locations[*id].is_some())
                        && problem.accepts_registered(i, *id)
                })
                .map(|(_, value)| value.clone())
                .collect();
            ListEntity::new(kept)
                .with_pinned(m.is_pinned())
                .with_pinned_prefix(m.pinned_prefix())
                .with_value_range(range)
        })
        .collect();
    let mut repair = problem.rebuild_with_entities(entities)?;
    // Repair retains registry identities, including missing values. Preserve
    // fixed edges so reconstruction filters cyclic predecessor orderings too.
    repair.dependencies = problem.dependencies.clone();
    Ok(if problem.allows_unassigned() {
        repair.allowing_unassigned()
    } else {
        repair
    })
}
