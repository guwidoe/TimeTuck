//! Row snapshots for general edits; constant-sized inverse for large swaps.
use super::{ListAssignment, ListMove, ListPosition};
use std::sync::OnceLock;

type Rows<V> = Vec<(usize, Vec<V>)>;
type Materialize<V> = fn(&ListAssignment<V>, ListPosition, ListPosition) -> Rows<V>;

pub(crate) enum ListUndo<V> {
    Rows(Rows<V>),
    Swap(Box<SwapUndo<V>>),
}
pub(crate) struct SwapUndo<V> {
    left: ListPosition,
    right: ListPosition,
    applied: bool,
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
            Self::Swap(_) => 0,
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
        };
        match rows.binary_search_by_key(&entity, |(id, _)| *id) {
            Ok(i) => Some(&rows[i].1),
            Err(_) => assignment.list(entity),
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
