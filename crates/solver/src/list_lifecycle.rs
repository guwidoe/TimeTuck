//! Prepared list edits with ordered value/range notifications and one flush.
use crate::{
    ListAssignment, ListChangePhase, ListEditError, ListError, ListGatherError, ListMove,
    ListMoveBuilder, ListPosition, ListProblem, ListRange,
};
use std::{
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListLifecycleChange<'a, V> {
    Assigned(&'a V),
    Unassigned(&'a V),
    Changed(ListRange),
    /// All stages have finished. Always paired with phase After.
    Flush,
}
#[derive(Clone, Copy, Debug)]
pub struct ListLifecycleEvent<'a, V> {
    pub phase: ListChangePhase,
    pub undo: bool,
    pub change: ListLifecycleChange<'a, V>,
    pub assignment: &'a ListAssignment<V>,
}
/// A prepared plan rejects a starting assignment that differs under V::Eq.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaleListPlan;
impl fmt::Display for StaleListPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("list edit plan does not match the current assignment")
    }
}
impl Error for StaleListPlan {}

#[derive(Clone, Debug)]
struct LifecycleStage<V> {
    before: ListAssignment<V>,
    after: ListAssignment<V>,
    before_ranges: Vec<ListRange>,
    after_ranges: Vec<ListRange>,
    assigned: Vec<V>,
    unassigned: Vec<V>,
}
impl<V: Eq> LifecycleStage<V> {
    fn new(
        before: ListAssignment<V>,
        after: ListAssignment<V>,
        assigned: Vec<V>,
        unassigned: Vec<V>,
    ) -> Self {
        let mut before_ranges = vec![];
        let mut after_ranges = vec![];
        for (entity, (old, new)) in before.lists().iter().zip(after.lists()).enumerate() {
            let start = old.iter().zip(new).take_while(|(a, b)| a == b).count();
            if start == old.len() && start == new.len() {
                continue;
            }
            let suffix = old[start..]
                .iter()
                .rev()
                .zip(new[start..].iter().rev())
                .take_while(|(a, b)| a == b)
                .count();
            before_ranges.push(ListRange {
                entity,
                start,
                end: old.len() - suffix,
            });
            after_ranges.push(ListRange {
                entity,
                start,
                end: new.len() - suffix,
            });
        }
        Self {
            before,
            after,
            before_ranges,
            after_ranges,
            assigned,
            unassigned,
        }
    }
}

/// A snapshot-bound edit with value assignment/removal and list-range brackets.
/// Prepare validates the final result, including pins, ranges and dependencies,
/// before any observer runs. The borrowed model cannot change while a plan lives.
/// Gather batches removals per source in first-member order, then inserts the
/// complete selection in input order. Destination indices refer to the original
/// lists. Within each stage, value-before callbacks precede range-before callbacks;
/// mutation precedes range-after and value-after callbacks. A single final Flush
/// lets a listener refresh derived state after the entire edit, never between stages.
/// Intermediate gather snapshots may have missing required values or displaced pins;
/// observers must defer full-model validation and scoring until Flush.
///
/// This owns snapshot copies, not a constant-time journal. It supplies explicit
/// listener hooks; it does not discover shadows or attach listeners to solvers.
/// Equality uses V::Eq; include all meaningful payload fields in equality, or keep
/// them immutable while a plan lives. Use movement() with existing scoring directors
/// when notifications are not needed.
///
/// ```
/// use timetuck::*;
/// let p = ListProblem::new(vec![1,2,3], vec![ListEntity::new(vec![1,2,3])])?;
/// let mut a = p.initial_assignment();
/// let plan = ListLifecyclePlan::unassign(&p, &a, ListRange {entity:0,start:1,end:3})?;
/// let mut flushes = 0;
/// let mut listener = |e: ListLifecycleEvent<'_, i32>| {
///     if matches!(e.change,ListLifecycleChange::Flush) { flushes += 1; }
/// };
/// plan.apply(&mut a, &mut listener)?.commit();
/// assert_eq!(a.lists(), &[vec![1]]);
/// assert_eq!(flushes, 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct ListLifecyclePlan<'p, V> {
    _problem: &'p ListProblem<V>,
    initial: ListAssignment<V>,
    result: ListAssignment<V>,
    movement: ListMove<V>,
    returned: Vec<V>,
    stages: Vec<LifecycleStage<V>>,
}
impl<'p, V: Eq + Clone> ListLifecyclePlan<'p, V> {
    /// Observe a normal move as a single atomic bracket. Value transitions are
    /// reported in source/destination list order; unchanged membership has no
    /// assigned/unassigned notifications, even for a move between owners.
    pub fn prepare(
        problem: &'p ListProblem<V>,
        initial: &ListAssignment<V>,
        movement: ListMove<V>,
    ) -> Result<Self, ListError> {
        problem.validate(initial)?;
        let mut result = initial.clone();
        problem.apply(&mut result, &movement)?.commit();
        let assigned = result
            .lists()
            .iter()
            .flatten()
            .filter(|v| initial.location(v).is_none())
            .cloned()
            .collect();
        let unassigned = initial
            .lists()
            .iter()
            .flatten()
            .filter(|v| result.location(v).is_none())
            .cloned()
            .collect();
        let stages = if initial == &result {
            vec![]
        } else {
            vec![LifecycleStage::new(
                initial.clone(),
                result.clone(),
                assigned,
                unassigned,
            )]
        };
        Ok(Self {
            _problem: problem,
            initial: initial.clone(),
            result,
            movement,
            returned: vec![],
            stages,
        })
    }
    pub fn replace(
        problem: &'p ListProblem<V>,
        initial: &ListAssignment<V>,
        source: ListPosition,
        replacement: ListPosition,
    ) -> Result<Self, ListEditError> {
        let mut draft = ListMoveBuilder::new(problem, initial)?;
        let displaced = draft.replace(source, replacement)?;
        let mut plan = Self::prepare(problem, initial, draft.finish())?;
        plan.returned = vec![displaced];
        Ok(plan)
    }
    pub fn unassign(
        problem: &'p ListProblem<V>,
        initial: &ListAssignment<V>,
        range: ListRange,
    ) -> Result<Self, ListEditError> {
        let mut draft = ListMoveBuilder::new(problem, initial)?;
        draft.unassign(range)?;
        let mut plan = Self::prepare(problem, initial, draft.finish())?;
        plan.returned = initial.lists()[range.entity][range.start..range.end].to_vec();
        Ok(plan)
    }
    pub fn assign(
        problem: &'p ListProblem<V>,
        initial: &ListAssignment<V>,
        values: &[V],
        destination: ListPosition,
    ) -> Result<Self, ListEditError> {
        let mut draft = ListMoveBuilder::new(problem, initial)?;
        draft.assign_many(values.iter().cloned(), destination)?;
        let mut plan = Self::prepare(problem, initial, draft.finish())?;
        plan.returned = values.to_vec();
        Ok(plan)
    }
    pub fn gather(
        problem: &'p ListProblem<V>,
        initial: &ListAssignment<V>,
        members: &[V],
        destination: Option<ListPosition>,
    ) -> Result<Self, ListGatherError> {
        let movement = problem.gather_move(initial, members, destination)?;
        let mut plan = Self::prepare(problem, initial, movement)?;
        plan.returned = members
            .iter()
            .map(|v| {
                initial.location(v).map_or_else(
                    || v.clone(),
                    |pos| initial.lists()[pos.entity][pos.index].clone(),
                )
            })
            .collect();
        if plan.stages.is_empty() {
            return Ok(plan);
        }
        let mut entities = vec![];
        for member in members {
            if let Some(pos) = initial.location(member)
                && !entities.contains(&pos.entity)
            {
                entities.push(pos.entity);
            }
        }
        plan.stages.clear();
        let mut current = initial.clone();
        for entity in entities {
            let removed: Vec<V> = plan
                .returned
                .iter()
                .filter(|v| initial.location(v).is_some_and(|p| p.entity == entity))
                .cloned()
                .collect();
            let mut after = current.clone();
            after.lists[entity].retain(|v| !removed.contains(v));
            plan.stages
                .push(LifecycleStage::new(current, after.clone(), vec![], removed));
            current = after;
        }
        if destination.is_some() && !members.is_empty() {
            plan.stages.push(LifecycleStage::new(
                current,
                plan.result.clone(),
                plan.returned.clone(),
                vec![],
            ));
        }
        Ok(plan)
    }
    pub fn returned_values(&self) -> &[V] {
        &self.returned
    }
    pub fn assignment(&self) -> &ListAssignment<V> {
        &self.result
    }
    /// Coordinates are bound to the initial snapshot, like ListMoveBuilder::finish.
    pub fn movement(&self) -> &ListMove<V> {
        &self.movement
    }
    pub fn apply<'a, O: FnMut(ListLifecycleEvent<'_, V>)>(
        &'a self,
        assignment: &'a mut ListAssignment<V>,
        observer: &'a mut O,
    ) -> Result<AppliedListLifecycle<'a, V, O>, StaleListPlan> {
        if assignment != &self.initial {
            return Err(StaleListPlan);
        }
        let original = assignment.clone();
        let mut edit = AppliedListLifecycle {
            assignment,
            observer,
            stages: &self.stages,
            original: Some(original),
        };
        for stage in edit.stages {
            edit.stage(stage, false);
        }
        edit.flush(false);
        Ok(edit)
    }
}

/// Drop replays stages in reverse and restores the original assignment, then
/// flushes once. Commit retains the final assignment without undo callbacks.
/// Any observer panic restores the original assignment before propagating. During
/// unrelated unwinding restoration is silent; observer side effects are not undone.
#[must_use = "drop rolls back the edit; commit to retain it"]
pub struct AppliedListLifecycle<'a, V: Clone, O: FnMut(ListLifecycleEvent<'_, V>)> {
    assignment: &'a mut ListAssignment<V>,
    observer: &'a mut O,
    stages: &'a [LifecycleStage<V>],
    original: Option<ListAssignment<V>>,
}
impl<V: Clone, O: FnMut(ListLifecycleEvent<'_, V>)> AppliedListLifecycle<'_, V, O> {
    pub fn assignment(&self) -> &ListAssignment<V> {
        self.assignment
    }
    pub fn commit(mut self) {
        self.original = None;
    }
    pub fn rollback(self) {
        drop(self);
    }
    fn notify(&mut self, phase: ListChangePhase, undo: bool, change: ListLifecycleChange<'_, V>) {
        (self.observer)(ListLifecycleEvent {
            phase,
            undo,
            change,
            assignment: self.assignment,
        });
    }
    fn stage(&mut self, stage: &LifecycleStage<V>, undo: bool) {
        let (before, after, assigned, unassigned, target) = if undo {
            (
                &stage.after_ranges,
                &stage.before_ranges,
                &stage.unassigned,
                &stage.assigned,
                &stage.before,
            )
        } else {
            (
                &stage.before_ranges,
                &stage.after_ranges,
                &stage.assigned,
                &stage.unassigned,
                &stage.after,
            )
        };
        for value in unassigned {
            self.notify(
                ListChangePhase::Before,
                undo,
                ListLifecycleChange::Unassigned(value),
            );
        }
        for value in assigned {
            self.notify(
                ListChangePhase::Before,
                undo,
                ListLifecycleChange::Assigned(value),
            );
        }
        for &range in before {
            self.notify(
                ListChangePhase::Before,
                undo,
                ListLifecycleChange::Changed(range),
            );
        }
        *self.assignment = target.clone();
        for &range in after.iter().rev() {
            self.notify(
                ListChangePhase::After,
                undo,
                ListLifecycleChange::Changed(range),
            );
        }
        for value in unassigned {
            self.notify(
                ListChangePhase::After,
                undo,
                ListLifecycleChange::Unassigned(value),
            );
        }
        for value in assigned {
            self.notify(
                ListChangePhase::After,
                undo,
                ListLifecycleChange::Assigned(value),
            );
        }
    }
    fn flush(&mut self, undo: bool) {
        if !self.stages.is_empty() {
            self.notify(ListChangePhase::After, undo, ListLifecycleChange::Flush);
        }
    }
}
impl<V: Clone, O: FnMut(ListLifecycleEvent<'_, V>)> Drop for AppliedListLifecycle<'_, V, O> {
    fn drop(&mut self) {
        let Some(original) = self.original.take() else {
            return;
        };
        if std::thread::panicking() {
            *self.assignment = original;
            return;
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            for stage in self.stages.iter().rev() {
                self.stage(stage, true);
            }
            self.flush(true);
        }));
        *self.assignment = original;
        if let Err(payload) = outcome {
            resume_unwind(payload);
        }
    }
}
