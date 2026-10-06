//! Bracketed change notifications for reversible list edits.
use crate::{ListAssignment, ListError, ListMove, ListProblem, ListRange};
use std::{
    ops::Deref,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListChangePhase {
    Before,
    After,
}

/// One notification for the whole edit. Ranges refer to this event's snapshot.
/// Undo reverses the before/after ranges. Entity order is ascending. List-length
/// changes report the removed/inserted region, omitting unchanged ends.
/// An observer maintaining indices must also shift the suffix by the size delta.
/// No events are emitted for an unchanged final assignment or invalid move.
#[derive(Clone, Copy, Debug)]
pub struct ListChangeEvent<'a, V> {
    pub phase: ListChangePhase,
    pub undo: bool,
    pub assignment: &'a ListAssignment<V>,
    pub ranges: &'a [ListRange],
}

/// A reversible observed edit. Drop restores the original lists; commit retains
/// the new lists. Observer panics restore assignment state before propagating.
/// During unrelated unwinding, rollback is silent to avoid a second panic. Observer
/// side effects are the observer's responsibility; callbacks cannot mutate lists.
pub struct ObservedListMove<'a, V, O: FnMut(ListChangeEvent<'_, V>)> {
    assignment: &'a mut ListAssignment<V>,
    original: Option<ListAssignment<V>>,
    observer: &'a mut O,
    before: Vec<ListRange>,
    after: Vec<ListRange>,
}
impl<V, O: FnMut(ListChangeEvent<'_, V>)> Deref for ObservedListMove<'_, V, O> {
    type Target = ListAssignment<V>;
    fn deref(&self) -> &Self::Target {
        self.assignment
    }
}
impl<V, O: FnMut(ListChangeEvent<'_, V>)> ObservedListMove<'_, V, O> {
    pub fn assignment(&self) -> &ListAssignment<V> {
        self.assignment
    }
    pub fn commit(mut self) {
        self.original = None;
    }
    pub fn rollback(self) {
        drop(self);
    }
    fn notify(&mut self, phase: ListChangePhase, undo: bool) {
        let ranges = if (phase == ListChangePhase::Before) != undo {
            &self.before
        } else {
            &self.after
        };
        if !ranges.is_empty() {
            (self.observer)(ListChangeEvent {
                phase,
                undo,
                assignment: self.assignment,
                ranges,
            });
        }
    }
}
impl<V, O: FnMut(ListChangeEvent<'_, V>)> Drop for ObservedListMove<'_, V, O> {
    fn drop(&mut self) {
        if self.original.is_none() {
            return;
        }
        if std::thread::panicking() {
            *self.assignment = self.original.take().unwrap();
            return;
        }
        let before = catch_unwind(AssertUnwindSafe(|| {
            self.notify(ListChangePhase::Before, true)
        }));
        *self.assignment = self.original.take().unwrap();
        if let Err(panic) = before {
            resume_unwind(panic);
        }
        self.notify(ListChangePhase::After, true);
    }
}
impl<V: Eq + Clone> ListProblem<V> {
    /// Apply a fully validated edit with before/after change ranges and matching
    /// undo notifications. Build a candidate privately before notifying listeners;
    /// each callback sees either the entire old or the entire new assignment.
    /// Composite edits produce a single bracket. Pins, ranges and dependency
    /// policies are enforced before any notification. No score work is implied.
    ///
    /// ```
    /// use timetuck::*;
    /// let p = ListProblem::new(vec![1,2,3],vec![ListEntity::new(vec![1,2,3])])?;
    /// let mut a = p.initial_assignment();
    /// let mut events = vec![];
    /// let mut listener = |e:ListChangeEvent<'_,i32>| events.push((e.phase,e.undo,e.ranges.to_vec()));
    /// {
    ///     let edit = p.apply_observed(&mut a,&ListMove::Change {
    ///         source:ListPosition {entity:0,index:2}, destination:ListPosition {entity:0,index:0}
    ///     },&mut listener)?;
    ///     assert_eq!(edit.lists()[0],vec![3,1,2]);
    /// }
    /// assert_eq!(a.lists()[0],vec![1,2,3]);
    /// assert_eq!(events.len(),4);
    /// # Ok::<(), ListError>(())
    /// ```
    pub fn apply_observed<'a, O: FnMut(ListChangeEvent<'_, V>)>(
        &self,
        assignment: &'a mut ListAssignment<V>,
        movement: &ListMove<V>,
        observer: &'a mut O,
    ) -> Result<ObservedListMove<'a, V, O>, ListError> {
        let mut candidate = assignment.clone();
        self.apply(&mut candidate, movement)?.commit();
        let mut before = vec![];
        let mut after = vec![];
        for (entity, (old, new)) in assignment.lists().iter().zip(candidate.lists()).enumerate() {
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
            before.push(ListRange {
                entity,
                start,
                end: old.len() - suffix,
            });
            after.push(ListRange {
                entity,
                start,
                end: new.len() - suffix,
            });
        }
        // Before-apply panic sees the untouched assignment and needs no rollback.
        if !before.is_empty() {
            observer(ListChangeEvent {
                phase: ListChangePhase::Before,
                undo: false,
                assignment,
                ranges: &before,
            });
        }
        let original = std::mem::replace(assignment, candidate);
        let mut edit = ObservedListMove {
            assignment,
            original: Some(original),
            observer,
            before,
            after,
        };
        edit.notify(ListChangePhase::After, false);
        Ok(edit)
    }
}

impl<V: Eq + Clone> ListProblem<V> {
    /// Relocate one value, returning its stable identity alongside the reversible edit.
    pub fn relocate_observed<'a, O: FnMut(ListChangeEvent<'_, V>)>(
        &self,
        assignment: &'a mut ListAssignment<V>,
        source: crate::ListPosition,
        destination: crate::ListPosition,
        observer: &'a mut O,
    ) -> Result<(V, ObservedListMove<'a, V, O>), ListError> {
        let value = assignment
            .list(source.entity)
            .and_then(|list| list.get(source.index))
            .ok_or(ListError::ElementIndex(source))?
            .clone();
        let edit = self.apply_observed(
            assignment,
            &ListMove::Change {
                source,
                destination,
            },
            observer,
        )?;
        Ok((value, edit))
    }
    /// Shift a value within its list by a nonzero signed offset.
    pub fn shift_observed<'a, O: FnMut(ListChangeEvent<'_, V>)>(
        &self,
        assignment: &'a mut ListAssignment<V>,
        source: crate::ListPosition,
        offset: isize,
        observer: &'a mut O,
    ) -> Result<(V, ObservedListMove<'a, V, O>), crate::ListEditError> {
        let mut draft = crate::ListMoveBuilder::new(self, assignment)?;
        let value = draft.shift(source, offset)?;
        let edit = self.apply_observed(assignment, &draft.finish(), observer)?;
        Ok((value, edit))
    }
    /// Relocate a nonempty span and return its values in insertion order.
    pub fn relocate_span_observed<'a, O: FnMut(ListChangeEvent<'_, V>)>(
        &self,
        assignment: &'a mut ListAssignment<V>,
        source: ListRange,
        destination: crate::ListPosition,
        reverse: bool,
        observer: &'a mut O,
    ) -> Result<(Vec<V>, ObservedListMove<'a, V, O>), crate::ListEditError> {
        let mut draft = crate::ListMoveBuilder::new(self, assignment)?;
        draft.relocate_span(source, destination, reverse)?;
        let mut values = assignment.lists()[source.entity][source.start..source.end].to_vec();
        if reverse {
            values.reverse();
        }
        let edit = self.apply_observed(assignment, &draft.finish(), observer)?;
        Ok((values, edit))
    }
}
