//! Candidate state retained by scoped foragers over one unchanged base.
//!
//! Capture completes before the trial rolls back. Installing a snapshot uses
//! owned values only: no move replay, validation or scoring callback. Snapshots
//! must never be installed over a different base. Tiny/dense candidates retain
//! the ordinary full copy; sparse candidates copy only written slots/saved rows.
use crate::{Assignment, ListAssignment, PriorValue};

pub(crate) enum BasicSnapshot<V> {
    Full(Assignment<V>),
    Sparse(Vec<(usize, Option<V>)>),
}
impl<V: Clone> BasicSnapshot<V> {
    pub(crate) fn capture(assignment: &Assignment<V>, writes: &[PriorValue<V>]) -> Self {
        if !writes.is_empty()
            && (assignment.values.len() <= 32 || writes.len() > assignment.values.len() / 8)
        {
            return Self::Full(assignment.clone());
        }
        // Repeated writes retain the final payload, including Eq-equal values.
        // Sorting/deduplication precedes user clones and needs no identity hashing.
        let mut slots: Vec<_> = writes.iter().map(|write| (write.variable, None)).collect();
        slots.sort_unstable_by_key(|(variable, _)| *variable);
        slots.dedup_by_key(|(variable, _)| *variable);
        for (variable, value) in &mut slots {
            *value = assignment.values[*variable].clone();
        }
        Self::Sparse(slots)
    }
}
impl<V> BasicSnapshot<V> {
    pub(crate) fn record_retained(
        &self,
        changes: &mut crate::retained_scoped_phase::AssignmentChanges,
    ) {
        match self {
            Self::Full(_) => changes.basic.mark_all(),
            Self::Sparse(slots) => changes.basic.record(slots.iter().map(|(index, _)| *index)),
        }
    }

    pub(crate) fn install(self, assignment: &mut Assignment<V>) {
        match self {
            Self::Full(saved) => *assignment = saved,
            Self::Sparse(slots) => {
                for (variable, value) in slots {
                    assignment.values[variable] = value;
                }
            }
        }
    }
}

pub(crate) enum ListSnapshot<V> {
    Full(ListAssignment<V>),
    Sparse(Vec<(usize, Vec<V>)>),
}
impl<V: Clone> ListSnapshot<V> {
    pub(crate) fn capture(
        assignment: &ListAssignment<V>,
        entity_count: usize,
        entities: impl Iterator<Item = usize>,
    ) -> Self {
        if entity_count != 0
            && (assignment.lists.len() <= 32 || entity_count > assignment.lists.len() / 8)
        {
            return Self::Full(assignment.clone());
        }
        // The transaction has already deduplicated and validated row indices.
        Self::Sparse(
            entities
                .map(|entity| (entity, assignment.lists[entity].clone()))
                .collect(),
        )
    }
}
impl<V> ListSnapshot<V> {
    pub(crate) fn record_retained(
        &self,
        changes: &mut crate::retained_scoped_phase::AssignmentChanges,
    ) {
        match self {
            Self::Full(_) => changes.lists.mark_all(),
            Self::Sparse(slots) => changes.lists.record(slots.iter().map(|(index, _)| *index)),
        }
    }

    pub(crate) fn install(self, assignment: &mut ListAssignment<V>) {
        match self {
            Self::Full(saved) => *assignment = saved,
            Self::Sparse(rows) => {
                for (entity, row) in rows {
                    assignment.lists[entity] = row;
                }
            }
        }
    }
}

pub(crate) struct MixedSnapshot<B, L> {
    pub(crate) basic: BasicSnapshot<B>,
    pub(crate) lists: ListSnapshot<L>,
}

impl<B, L> MixedSnapshot<B, L> {
    pub(crate) fn record_retained(
        &self,
        changes: &mut crate::retained_scoped_phase::AssignmentChanges,
    ) {
        self.basic.record_retained(changes);
        self.lists.record_retained(changes);
    }
}
