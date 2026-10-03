//! Indexed strict cross-endpoint overlap with optional equality partitions.
mod index;
use crate::{SnapshotMap, TupleChange};
use index::IntervalIndex;
use std::any::Any;
type Projection<'a, T, E, K> = dyn Fn(&T) -> (E, K, K) + 'a;
type Changes<I, J, L, R> = Vec<TupleChange<(I, J), (L, R)>>;
#[derive(Clone)]
struct Side<I, T, K, E> {
    rows: SnapshotMap<I, (E, K, K, T)>,
    groups: SnapshotMap<E, IntervalIndex<I, K>>,
}
impl<I, T, K, E> Side<I, T, K, E> {
    fn new() -> Self {
        Self {
            rows: SnapshotMap::new(),
            groups: SnapshotMap::new(),
        }
    }
}
impl<I: Ord + Clone, T: Clone, K: Ord + Clone, E: Ord + Clone> Side<I, T, K, E> {
    fn matching<'a>(&'a self, e: &E, start: &'a K, end: &'a K) -> impl Iterator<Item = &'a I> {
        self.groups
            .get(e)
            .into_iter()
            .flat_map(move |index| index.query(start, end))
    }
    fn apply<J: Ord + Clone, U: Clone>(
        &mut self,
        other: &Side<J, U, K, E>,
        change: TupleChange<I, T>,
        project: &dyn Fn(&T) -> (E, K, K),
    ) -> Changes<I, J, T, U> {
        let new = change.value.map(|row| {
            let (e, start, end) = project(&row);
            (e, start, end, row)
        });
        let old = self.rows.get(&change.id);
        let same = old
            .zip(new.as_ref())
            .is_some_and(|(a, b)| a.0 == b.0 && a.1 == b.1 && a.2 == b.2);
        let mut out = Vec::new();
        if !same && let Some((e, start, end, _)) = old {
            for id in other.matching(e, start, end) {
                out.push(TupleChange::retract((change.id.clone(), id.clone())));
            }
        }
        if let Some((e, start, end, row)) = &new {
            for id in other.matching(e, start, end) {
                out.push(TupleChange::put(
                    (change.id.clone(), id.clone()),
                    (
                        row.clone(),
                        other.rows.get(id).expect("indexed row").3.clone(),
                    ),
                ));
            }
        }
        let mut candidate = self.clone();
        if !same {
            if let Some((e, start, _, _)) = old {
                let mut index = candidate.groups.get(e).expect("indexed partition").clone();
                index.remove(&(start.clone(), change.id.clone()));
                if index.is_empty() {
                    candidate.groups.remove(e);
                } else {
                    candidate.groups.insert(e.clone(), index);
                }
            }
            if let Some((e, start, end, _)) = &new {
                let mut index = candidate
                    .groups
                    .get(e)
                    .cloned()
                    .unwrap_or_else(IntervalIndex::new);
                index.insert(start.clone(), change.id.clone(), end.clone());
                candidate.groups.insert(e.clone(), index);
            }
        }
        if let Some(row) = new {
            candidate.rows.insert(change.id, row);
        } else {
            candidate.rows.remove(&change.id);
        }
        *self = candidate;
        out
    }
}
/// Join rows when `left_start < right_end && right_start < left_end`, optionally
/// within equal partitions. Touching endpoints do not match. Keys use native Ord.
/// Endpoints are not normalized or validated: zero-length and reversed endpoints
/// follow the same two comparisons, not mathematical empty-set intersection.
///
/// A persistent AVL index ordered by `(start, ID)` caches subtree maximum ends.
/// Queries prune starts beyond the query end and subtrees ending before its start.
/// Memory is linear in indexed rows; dense results can still be quadratic.
/// IDs preserve duplicate occurrences. Retractions use cached endpoints; same-key
/// replacements update surviving pairs once. Candidate roots and all output
/// clones precede installation, so ordinary callback/clone/ordering panics preserve
/// this standalone node. Graph transactions roll back downstream propagation.
/// Callbacks, keys and payloads must remain deterministic and immutable.
///
/// ```
/// use timetuck::{OverlapJoinNode,TupleChange};
/// let mut join=OverlapJoinNode::new(|r:&(i32,i32)| *r,|r:&(i32,i32)| *r);
/// join.apply_left(TupleChange::put(0,(1,3)));
/// assert!(join.apply_right(TupleChange::put(0,(3,5))).is_empty());
/// assert_eq!(join.apply_right(TupleChange::put(0,(2,5))),
///     vec![TupleChange::put((0,0),((1,3),(2,5)))]);
/// ```
pub struct OverlapJoinNode<'a, I, J, L, R, K, E = ()> {
    left: Side<I, L, K, E>,
    right: Side<J, R, K, E>,
    left_key: Box<Projection<'a, L, E, K>>,
    right_key: Box<Projection<'a, R, E, K>>,
}
impl<'a, I: Ord + Clone, J: Ord + Clone, L: Clone, R: Clone, K: Ord + Clone>
    OverlapJoinNode<'a, I, J, L, R, K>
{
    pub fn new(left: impl Fn(&L) -> (K, K) + 'a, right: impl Fn(&R) -> (K, K) + 'a) -> Self {
        Self::partitioned(
            move |row| {
                let (s, e) = left(row);
                ((), s, e)
            },
            move |row| {
                let (s, e) = right(row);
                ((), s, e)
            },
        )
    }
}
impl<'a, I: Ord + Clone, J: Ord + Clone, L: Clone, R: Clone, K: Ord + Clone, E: Ord + Clone>
    OverlapJoinNode<'a, I, J, L, R, K, E>
{
    /// Projections return `(equality_key,start,end)`. Tuple equality keys express
    /// multiple equalities; neither endpoint comparison is lexicographic.
    pub fn partitioned(
        left: impl Fn(&L) -> (E, K, K) + 'a,
        right: impl Fn(&R) -> (E, K, K) + 'a,
    ) -> Self {
        Self {
            left: Side::new(),
            right: Side::new(),
            left_key: Box::new(left),
            right_key: Box::new(right),
        }
    }
    pub fn apply_left(&mut self, c: TupleChange<I, L>) -> Changes<I, J, L, R> {
        self.left.apply(&self.right, c, &self.left_key)
    }
    pub fn apply_right(&mut self, c: TupleChange<J, R>) -> Changes<I, J, L, R> {
        self.right
            .apply(&self.left, c, &self.right_key)
            .into_iter()
            .map(|c| TupleChange {
                id: (c.id.1, c.id.0),
                value: c.value.map(|(r, l)| (l, r)),
            })
            .collect()
    }
    /// Inspect pairs in left-ID, right-start, right-ID order.
    pub fn matches(&self) -> impl Iterator<Item = ((&I, &J), (&L, &R))> {
        self.left
            .rows
            .iter()
            .flat_map(move |(id, (e, s, end, row))| {
                self.right.matching(e, s, end).map(move |other| {
                    (
                        (id, other),
                        (row, &self.right.rows.get(other).expect("indexed row").3),
                    )
                })
            })
    }
    pub(crate) fn clear(&mut self) {
        self.left = Side::new();
        self.right = Side::new();
    }
    pub(crate) fn checkpoint(&self) -> Box<dyn Any>
    where
        I: 'static,
        J: 'static,
        L: 'static,
        R: 'static,
        K: 'static,
        E: 'static,
    {
        Box::new((self.left.clone(), self.right.clone()))
    }
    pub(crate) fn restore(&mut self, c: Box<dyn Any>)
    where
        I: 'static,
        J: 'static,
        L: 'static,
        R: 'static,
        K: 'static,
        E: 'static,
    {
        (self.left, self.right) = *c.downcast().expect("overlap checkpoint type");
    }
}
