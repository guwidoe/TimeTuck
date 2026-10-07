//! Typed reusable projections and normalized compound indexed joins.
use super::*;
use std::{
    any::Any,
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
};

type KeyFn<'a, T, K> = dyn Fn(&T) -> K + 'a;

/// A typed, reusable join projection. Clones retain callback identity; separately
/// constructed projections are distinct even when their callback bodies match.
/// Keys need not implement Clone. Callbacks must be pure and deterministic.
pub struct JoinProjection<'a, T, K> {
    callback: Rc<KeyFn<'a, T, K>>,
    identity: Rc<()>,
}
impl<T, K> Clone for JoinProjection<'_, T, K> {
    fn clone(&self) -> Self {
        Self {
            callback: self.callback.clone(),
            identity: self.identity.clone(),
        }
    }
}
impl<'a, T, K> JoinProjection<'a, T, K> {
    pub fn new(callback: impl Fn(&T) -> K + 'a) -> Self {
        Self {
            callback: Rc::new(callback),
            identity: Rc::new(()),
        }
    }
}

trait OrderedKey: Any {
    fn any(&self) -> &dyn Any;
    fn compare_same_type(&self, other: &dyn Any) -> Ordering;
}
impl<K: Ord + 'static> OrderedKey for K {
    fn any(&self) -> &dyn Any {
        self
    }
    fn compare_same_type(&self, other: &dyn Any) -> Ordering {
        self.cmp(other.downcast_ref::<K>().expect("matching join key types"))
    }
}
#[derive(Clone)]
struct Key(Rc<dyn OrderedKey>);
impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .any()
            .type_id()
            .cmp(&other.0.any().type_id())
            .then_with(|| self.0.compare_same_type(other.0.any()))
    }
}
impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for Key {}

#[derive(Clone)]
struct Identity {
    comparison: JoinComparison,
    left: Rc<()>,
    right: Rc<()>,
}
impl PartialEq for Identity {
    fn eq(&self, other: &Self) -> bool {
        self.comparison == other.comparison
            && Rc::ptr_eq(&self.left, &other.left)
            && Rc::ptr_eq(&self.right, &other.right)
    }
}
impl Eq for Identity {}
impl Hash for Identity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.comparison as u8).hash(state);
        Rc::as_ptr(&self.left).hash(state);
        Rc::as_ptr(&self.right).hash(state);
    }
}

/// One comparison between typed left/right projections. Both projections must
/// return the same key type; separate conditions may use different key types.
pub struct JoinCondition<'a, L, R> {
    identity: Identity,
    left: Rc<KeyFn<'a, L, Key>>,
    right: Rc<KeyFn<'a, R, Key>>,
}
impl<L, R> Clone for JoinCondition<'_, L, R> {
    fn clone(&self) -> Self {
        Self {
            identity: self.identity.clone(),
            left: self.left.clone(),
            right: self.right.clone(),
        }
    }
}
impl<'a, L: 'a, R: 'a> JoinCondition<'a, L, R> {
    pub fn new<K: Ord + 'static>(
        comparison: JoinComparison,
        left: &JoinProjection<'a, L, K>,
        right: &JoinProjection<'a, R, K>,
    ) -> Self {
        let identity = Identity {
            comparison,
            left: left.identity.clone(),
            right: right.identity.clone(),
        };
        let left = left.callback.clone();
        let right = right.callback.clone();
        Self {
            identity,
            left: Rc::new(move |row| Key(Rc::new(left(row)))),
            right: Rc::new(move |row| Key(Rc::new(right(row)))),
        }
    }
    pub fn comparison(&self) -> JoinComparison {
        self.identity.comparison
    }
    pub fn left_projection_is<K>(&self, projection: &JoinProjection<'a, L, K>) -> bool {
        Rc::ptr_eq(&self.identity.left, &projection.identity)
    }
    pub fn right_projection_is<K>(&self, projection: &JoinProjection<'a, R, K>) -> bool {
        Rc::ptr_eq(&self.identity.right, &projection.identity)
    }
    pub fn matches(&self, left: &L, right: &R) -> bool {
        self.comparison()
            .matches(&(self.left)(left), &(self.right)(right))
    }
}

/// Immutable conjunction of join conditions. Equality-first normalization is
/// stable within each group and preserves projection identity. Equality and hash
/// describe the declared order; normalize before comparing reordered definitions.
/// No reordering occurs within the equality group or the ordered-comparison group.
/// An empty conjunction matches every pair.
///
/// Applying to a graph indexes all equalities together and the first inequality;
/// remaining inequalities are independent pair filters, never lexicographic tuple
/// comparisons. Normalized definitions with identical projection handles share
/// nodes for the same ordered parent ports in the same builder. Different builders
/// and freshly constructed projections remain independent. Key storage uses owned,
/// type-erased Ord values; ordinary typed join APIs remain available.
///
/// ```
/// use timetuck::*;
/// let employee = JoinProjection::new(|row: &(String, i32)| row.0.clone());
/// let start = JoinProjection::new(|row: &(String, i32)| row.1);
/// let conditions = JoinConditions::new([
///     JoinCondition::new(JoinComparison::LessThan, &start, &start),
///     JoinCondition::new(JoinComparison::Equal, &employee, &employee),
/// ]);
/// let mut b = StreamGraphBuilder::<SimpleScore>::new();
/// let shifts = b.source::<usize, (String, i32)>();
/// let pairs = conditions.apply(&mut b, shifts.port(), shifts.port())?;
/// b.score("ordered employee pairs", &pairs, |_| SimpleScore::new(1))?;
/// let mut graph = b.build();
/// let mut tx = graph.transaction();
/// tx.apply(&shifts, TupleChange::put(0, ("Ann".into(), 1)))?;
/// tx.apply(&shifts, TupleChange::put(1, ("Ann".into(), 2)))?;
/// tx.commit()?;
/// assert_eq!(graph.score(), SimpleScore::new(1));
/// # Ok::<(), GraphError>(())
/// ```
pub struct JoinConditions<'a, L, R>(Rc<Vec<JoinCondition<'a, L, R>>>);
impl<L, R> Clone for JoinConditions<'_, L, R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<L, R> PartialEq for JoinConditions<'_, L, R> {
    fn eq(&self, other: &Self) -> bool {
        self.0
            .iter()
            .map(|c| &c.identity)
            .eq(other.0.iter().map(|c| &c.identity))
    }
}
impl<L, R> Eq for JoinConditions<'_, L, R> {}
impl<L, R> Hash for JoinConditions<'_, L, R> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.len().hash(state);
        for c in self.0.iter() {
            c.identity.hash(state);
        }
    }
}
impl<L, R> fmt::Debug for JoinConditions<'_, L, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.0.iter().map(|c| c.identity.comparison))
            .finish()
    }
}
impl<'a, L: 'a, R: 'a> JoinConditions<'a, L, R> {
    pub fn new(conditions: impl IntoIterator<Item = JoinCondition<'a, L, R>>) -> Self {
        Self(Rc::new(conditions.into_iter().collect()))
    }
    pub fn and(&self, condition: JoinCondition<'a, L, R>) -> Self {
        Self::new(self.0.iter().cloned().chain([condition]))
    }
    pub fn conditions(&self) -> &[JoinCondition<'a, L, R>] {
        &self.0
    }
    pub fn same_instance(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
    pub fn equals_first(&self) -> Self {
        let mut saw_ordered = false;
        let needs_reorder = self.0.iter().any(|c| {
            if c.comparison() == JoinComparison::Equal {
                saw_ordered
            } else {
                saw_ordered = true;
                false
            }
        });
        if !needs_reorder {
            return self.clone();
        }
        Self::new(
            self.0
                .iter()
                .filter(|c| c.comparison() == JoinComparison::Equal)
                .chain(
                    self.0
                        .iter()
                        .filter(|c| c.comparison() != JoinComparison::Equal),
                )
                .cloned(),
        )
    }
    pub fn matches(&self, left: &L, right: &R) -> bool {
        self.0.iter().all(|c| c.matches(left, right))
    }
}

#[derive(Default)]
pub(super) struct Registrations(Vec<Registration>);
struct Registration {
    parents: [usize; 2],
    conditions: Vec<Identity>,
    output: usize,
}
impl<'a, L: Clone + 'static, R: Clone + 'static> JoinConditions<'a, L, R> {
    pub fn apply<I: Ord + Clone + 'static, J: Ord + Clone + 'static, S: AdditiveScore + 'static>(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        b.validate(left)?;
        b.validate(right)?;
        let normalized = self.equals_first();
        let parents = [left.node, right.node];
        let identities: Vec<_> = normalized.0.iter().map(|c| c.identity.clone()).collect();
        if let Some(entry) = b
            .compound_joins
            .0
            .iter()
            .find(|entry| entry.parents == parents && entry.conditions == identities)
        {
            return Ok(StreamPort {
                owner: b.owner.clone(),
                node: entry.output,
                marker: PhantomData,
            });
        }
        let equal_count = normalized
            .0
            .iter()
            .take_while(|c| c.comparison() == JoinComparison::Equal)
            .count();
        let indexed_count = (equal_count + 1).min(normalized.0.len());
        let comparison = normalized
            .0
            .get(equal_count)
            .map_or(JoinComparison::Equal, |c| c.comparison());
        let lc = normalized.clone();
        let rc = normalized.clone();
        let joined = b.join_partitioned(
            left,
            right,
            comparison,
            move |row| {
                (
                    lc.0[..equal_count]
                        .iter()
                        .map(|c| (c.left)(row))
                        .collect::<Vec<_>>(),
                    lc.0.get(equal_count).map(|c| (c.left)(row)),
                )
            },
            move |row| {
                (
                    rc.0[..equal_count]
                        .iter()
                        .map(|c| (c.right)(row))
                        .collect::<Vec<_>>(),
                    rc.0.get(equal_count).map(|c| (c.right)(row)),
                )
            },
        )?;
        let out = if indexed_count < normalized.0.len() {
            b.filter(&joined, move |(l, r)| {
                normalized.0[indexed_count..]
                    .iter()
                    .all(|c| c.matches(l, r))
            })?
        } else {
            joined
        };
        b.compound_joins.0.push(Registration {
            parents,
            conditions: identities,
            output: out.node,
        });
        Ok(out)
    }
}
