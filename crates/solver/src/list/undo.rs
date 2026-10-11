//! Row snapshots for general edits; compact inverses for large swaps and changes.
use super::{ListAssignment, ListMove, ListPosition};
use std::sync::OnceLock;

type Rows<V> = Vec<(usize, Vec<V>)>;
type Materialize<V> = fn(&ListAssignment<V>, ListPosition, ListPosition) -> Rows<V>;

pub(crate) enum ListUndo<V> {
    Rows(Rows<V>),
    Swap(Box<SwapUndo<V>>),
    Change(Box<ChangeUndo<V>>),
}
pub(crate) struct SwapUndo<V> {
    left: ListPosition,
    right: ListPosition,
    applied: bool,
    original: OnceLock<Rows<V>>,
    materialize: Materialize<V>,
}

pub(crate) struct ChangeUndo<V> {
    source: ListPosition,
    destination: ListPosition,
    applied: bool,
    // Record callbacks run after removal. Retain the actual owned payload while
    // they execute, so a panic can restore it without calling Clone or Drop.
    removed: Option<V>,
    original: OnceLock<Rows<V>>,
    materialize: Materialize<V>,
}

impl<V: Clone> ListUndo<V> {
    pub(super) fn new(
        assignment: &ListAssignment<V>,
        movement: &ListMove<V>,
        entities: Vec<usize>,
    ) -> Self {
        if let ListMove::Swap { left, right } = movement
            && let (Some(a), Some(b)) =
                (assignment.list(left.entity), assignment.list(right.entity))
            && left.index < a.len()
            && right.index < b.len()
            && a.len().saturating_add(if left.entity == right.entity {
                0
            } else {
                b.len()
            }) >= 64
        {
            return Self::Swap(Box::new(SwapUndo {
                left: *left,
                right: *right,
                applied: false,
                original: OnceLock::new(),
                materialize: materialize::<V>,
            }));
        }
        if let ListMove::Change {
            source,
            destination,
        } = movement
            && let (Some(a), Some(b)) = (
                assignment.list(source.entity),
                assignment.list(destination.entity),
            )
            && source.index < a.len()
            && destination.index <= b.len() - usize::from(source.entity == destination.entity)
            && a.len()
                .saturating_add(if source.entity == destination.entity {
                    0
                } else {
                    b.len()
                })
                >= 64
        {
            return Self::Change(Box::new(ChangeUndo {
                source: *source,
                destination: *destination,
                applied: false,
                removed: None,
                original: OnceLock::new(),
                materialize: materialize_change::<V>,
            }));
        }
        Self::Rows(
            entities
                .into_iter()
                .filter_map(|entity| assignment.list(entity).map(|row| (entity, row.to_vec())))
                .collect(),
        )
    }
}
impl<V> ListUndo<V> {
    #[inline]
    pub(crate) fn entity_count(&self) -> usize {
        match self {
            Self::Rows(rows) => rows.len(),
            Self::Swap(s) => 1 + usize::from(s.left.entity != s.right.entity),
            Self::Change(c) => 1 + usize::from(c.source.entity != c.destination.entity),
        }
    }
    #[inline]
    pub(crate) fn entities(&self) -> impl Iterator<Item = usize> + '_ {
        let (rows, pair): (&[(usize, Vec<V>)], _) = match self {
            Self::Rows(rows) => (rows, [None, None]),
            Self::Swap(s) => (
                &[],
                [
                    Some(s.left.entity.min(s.right.entity)),
                    (s.left.entity != s.right.entity).then_some(s.left.entity.max(s.right.entity)),
                ],
            ),
            Self::Change(c) => (
                &[],
                [
                    Some(c.source.entity.min(c.destination.entity)),
                    (c.source.entity != c.destination.entity)
                        .then_some(c.source.entity.max(c.destination.entity)),
                ],
            ),
        };
        rows.iter()
            .map(|(entity, _)| *entity)
            .chain(pair.into_iter().flatten())
    }
    #[inline]
    pub(super) fn mark_swapped(&mut self) {
        if let Self::Swap(s) = self {
            s.applied = true;
        }
    }
    #[inline]
    pub(crate) fn assigned_delta(&self, assignment: &ListAssignment<V>) -> i64 {
        match self {
            Self::Rows(rows) => rows
                .iter()
                .map(|(entity, before)| {
                    assignment.lists[*entity].len() as i64 - before.len() as i64
                })
                .sum(),
            Self::Swap(_) | Self::Change(_) => 0,
        }
    }
    pub(crate) fn original_list<'a>(
        &'a self,
        assignment: &'a ListAssignment<V>,
        entity: usize,
    ) -> Option<&'a [V]> {
        let rows = match self {
            Self::Rows(rows) => rows,
            Self::Swap(s) => {
                if !s.applied
                    || s.left == s.right
                    || (entity != s.left.entity && entity != s.right.entity)
                {
                    return assignment.list(entity);
                }
                s.original
                    .get_or_init(|| (s.materialize)(assignment, s.left, s.right))
            }
            Self::Change(c) => {
                if !c.applied
                    || c.source == c.destination
                    || (entity != c.source.entity && entity != c.destination.entity)
                {
                    return assignment.list(entity);
                }
                c.original
                    .get_or_init(|| (c.materialize)(assignment, c.source, c.destination))
            }
        };
        match rows.binary_search_by_key(&entity, |(id, _)| *id) {
            Ok(i) => Some(&rows[i].1),
            Err(_) => assignment.list(entity),
        }
    }
    pub(super) fn change_recording(
        &mut self,
        assignment: &mut ListAssignment<V>,
        source: ListPosition,
        destination: ListPosition,
        record: &mut impl FnMut(&V),
    ) {
        if let Self::Change(c) = self {
            c.removed = Some(assignment.lists[source.entity].remove(source.index));
            record(c.removed.as_ref().expect("removed change value"));
            // Bounds are already validated; insertion cannot run user code.
            let value = c.removed.take().expect("recorded change value");
            assignment.lists[destination.entity].insert(destination.index, value);
            c.applied = true;
        } else {
            let value = assignment.lists[source.entity].remove(source.index);
            record(&value);
            assignment.lists[destination.entity].insert(destination.index, value);
        }
    }
    pub(crate) fn rollback(&mut self, assignment: &mut ListAssignment<V>) {
        match self {
            Self::Rows(rows) => {
                for (entity, row) in rows {
                    std::mem::swap(&mut assignment.lists[*entity], row);
                }
            }
            Self::Swap(s) => {
                if s.applied {
                    swap(assignment, s.left, s.right);
                    s.applied = false;
                }
            }
            Self::Change(c) => {
                if let Some(value) = c.removed.take() {
                    assignment.lists[c.source.entity].insert(c.source.index, value);
                } else if c.applied {
                    let value = assignment.lists[c.destination.entity].remove(c.destination.index);
                    // Removal retained the source's capacity; no allocation or
                    // user code is needed to undo either a same-row or cross-row edit.
                    assignment.lists[c.source.entity].insert(c.source.index, value);
                    c.applied = false;
                }
            }
        }
    }
}

pub(super) fn swap<V>(assignment: &mut ListAssignment<V>, left: ListPosition, right: ListPosition) {
    if left.entity == right.entity {
        assignment.lists[left.entity].swap(left.index, right.index);
    } else {
        let (a, b) = if left.entity < right.entity {
            let (low, high) = assignment.lists.split_at_mut(right.entity);
            (&mut low[left.entity][left.index], &mut high[0][right.index])
        } else {
            let (low, high) = assignment.lists.split_at_mut(left.entity);
            (
                &mut high[0][left.index],
                &mut low[right.entity][right.index],
            )
        };
        std::mem::swap(a, b);
    }
}
fn materialize<V: Clone>(
    assignment: &ListAssignment<V>,
    left: ListPosition,
    right: ListPosition,
) -> Rows<V> {
    let first = left.entity.min(right.entity);
    let mut rows = vec![(first, assignment.lists[first].clone())];
    if left.entity == right.entity {
        rows[0].1.swap(left.index, right.index);
    } else {
        let second = left.entity.max(right.entity);
        rows.push((second, assignment.lists[second].clone()));
        let (a, b) = rows.split_at_mut(1);
        let (first_index, second_index) = if left.entity < right.entity {
            (left.index, right.index)
        } else {
            (right.index, left.index)
        };
        std::mem::swap(&mut a[0].1[first_index], &mut b[0].1[second_index]);
    }
    rows
}

fn materialize_change<V: Clone>(
    assignment: &ListAssignment<V>,
    source: ListPosition,
    destination: ListPosition,
) -> Rows<V> {
    // Clone the original ordering in bulk slices rather than shifting both
    // cloned final rows again. Clone invocation order is not a public promise;
    // failures only discard this cache construction, leaving inverse undo armed.
    if source.entity == destination.entity {
        let row = &assignment.lists[source.entity];
        let mut original = Vec::with_capacity(row.len());
        if source.index < destination.index {
            original.extend_from_slice(&row[..source.index]);
            original.push(row[destination.index].clone());
            original.extend_from_slice(&row[source.index..destination.index]);
            original.extend_from_slice(&row[destination.index + 1..]);
        } else {
            original.extend_from_slice(&row[..destination.index]);
            original.extend_from_slice(&row[destination.index + 1..source.index + 1]);
            original.push(row[destination.index].clone());
            original.extend_from_slice(&row[source.index + 1..]);
        }
        return vec![(source.entity, original)];
    }
    [
        source.entity.min(destination.entity),
        source.entity.max(destination.entity),
    ]
    .into_iter()
    .map(|entity| {
        let row = &assignment.lists[entity];
        let original = if entity == source.entity {
            let mut original = Vec::with_capacity(row.len() + 1);
            original.extend_from_slice(&row[..source.index]);
            original.push(assignment.lists[destination.entity][destination.index].clone());
            original.extend_from_slice(&row[source.index..]);
            original
        } else {
            let mut original = Vec::with_capacity(row.len() - 1);
            original.extend_from_slice(&row[..destination.index]);
            original.extend_from_slice(&row[destination.index + 1..]);
            original
        };
        (entity, original)
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use crate::{ListEntity, ListMove, ListPosition, ListProblem};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn change_record_panic_restores_removed_value_before_insertion() {
        let p = ListProblem::new_indexed(
            (0..128).collect(),
            vec![
                ListEntity::new((0..64).collect()),
                ListEntity::new((64..128).collect()),
            ],
        )
        .unwrap();
        for (source, destination) in [
            (
                ListPosition {
                    entity: 0,
                    index: 0,
                },
                ListPosition {
                    entity: 1,
                    index: 64,
                },
            ),
            (
                ListPosition {
                    entity: 1,
                    index: 63,
                },
                ListPosition {
                    entity: 0,
                    index: 0,
                },
            ),
            (
                ListPosition {
                    entity: 0,
                    index: 63,
                },
                ListPosition {
                    entity: 0,
                    index: 0,
                },
            ),
        ] {
            let mut assignment = p.initial_assignment();
            let before = assignment.clone();
            let mut calls = 0;
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _trial = p.apply_recording(
                        &mut assignment,
                        &ListMove::Change {
                            source,
                            destination,
                        },
                        |value| {
                            calls += 1;
                            assert_eq!(*value, before.lists()[source.entity][source.index]);
                            panic!("record");
                        },
                    );
                }))
                .is_err()
            );
            assert_eq!(calls, 1);
            assert_eq!(assignment, before);
        }
    }
}
