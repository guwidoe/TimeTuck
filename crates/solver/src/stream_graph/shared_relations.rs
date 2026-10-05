//! Reusable indexed relation definitions with ordered two-parent identity.
use super::*;
use std::{cell::RefCell, rc::Weak};
type Projection<'a, T, K> = dyn Fn(&T) -> K + 'a;
type Predicate<'a, L, R> = dyn Fn(&L, &R) -> bool + 'a;
struct Registration {
    owner: Weak<()>,
    parents: [usize; 2],
    output: usize,
}
#[derive(Clone, Default)]
pub(super) struct Registrations(Rc<RefCell<Vec<Registration>>>);
impl Registrations {
    pub(super) fn find<I, T>(
        &self,
        owner: &Rc<()>,
        parents: [usize; 2],
    ) -> Option<StreamPort<I, T>> {
        let mut entries = self.0.borrow_mut();
        entries.retain(|e| e.owner.strong_count() != 0);
        let identity = Rc::downgrade(owner);
        entries
            .iter()
            .find(|e| e.parents == parents && e.owner.ptr_eq(&identity))
            .map(|e| StreamPort {
                owner: owner.clone(),
                node: e.output,
                marker: PhantomData,
            })
    }
    pub(super) fn remember<I, T>(&self, parents: [usize; 2], out: &StreamPort<I, T>) {
        self.0.borrow_mut().push(Registration {
            owner: Rc::downgrade(&out.owner),
            parents,
            output: out.node,
        });
    }
}
struct Definition<'a, L, R, K> {
    left: Rc<Projection<'a, L, K>>,
    right: Rc<Projection<'a, R, K>>,
    predicate: Option<Rc<Predicate<'a, L, R>>>,
    registrations: Registrations,
}
impl<L, R, K> Clone for Definition<'_, L, R, K> {
    fn clone(&self) -> Self {
        Self {
            left: self.left.clone(),
            right: self.right.clone(),
            predicate: self.predicate.clone(),
            registrations: self.registrations.clone(),
        }
    }
}
impl<'a, L, R, K> Definition<'a, L, R, K> {
    fn new(left: impl Fn(&L) -> K + 'a, right: impl Fn(&R) -> K + 'a) -> Self {
        Self {
            left: Rc::new(left),
            right: Rc::new(right),
            predicate: None,
            registrations: Registrations::default(),
        }
    }
    fn filtered(mut self, predicate: impl Fn(&L, &R) -> bool + 'a) -> Self {
        self.predicate = Some(Rc::new(predicate));
        self.registrations = Registrations::default();
        self
    }
}

/// Reusable equality or ordered-key join. A definition/clone applied to the same
/// ordered pair of parent ports in the same builder shares its relation and optional
/// post-join filter. Changing either parent or creating a fresh definition creates
/// an independent node. Ordinary join builder calls are unaffected.
///
/// No callbacks run during apply. Key and predicate callbacks must be deterministic
/// and read immutable inputs. Graph journals cover shared state and each downstream
/// score independently. Definitions retain only callbacks and weak builder/node
/// registrations; graph nodes retain the callbacks after definitions are dropped.
/// Registrations prune dead owners on apply; lookup is linear in active bindings.
/// Self joins deliver each source change to both sides, as ordinary native joins do.
///
/// ```
/// use timetuck::*;
/// let same_machine = SharedJoin::new(|a: &i32| *a, |b: &i32| *b);
/// let mut b = StreamGraphBuilder::<SimpleScore>::new();
/// let jobs = b.source::<usize, i32>();
/// let pairs = same_machine.apply(&mut b, jobs.port(), jobs.port())?;
/// let again = same_machine.clone().apply(&mut b, jobs.port(), jobs.port())?;
/// assert_eq!(b.dataset(&pairs)?, b.dataset(&again)?);
/// # Ok::<(), GraphError>(())
/// ```
pub struct SharedJoin<'a, I, J, L, R, K> {
    definition: Definition<'a, L, R, K>,
    comparison: JoinComparison,
    ids: PhantomData<fn() -> (I, J)>,
}
impl<I, J, L, R, K> Clone for SharedJoin<'_, I, J, L, R, K> {
    fn clone(&self) -> Self {
        Self {
            definition: self.definition.clone(),
            comparison: self.comparison,
            ids: PhantomData,
        }
    }
}
impl<
    'a,
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    K: Ord + Clone + 'static,
> SharedJoin<'a, I, J, L, R, K>
{
    pub fn new(left: impl Fn(&L) -> K + 'a, right: impl Fn(&R) -> K + 'a) -> Self {
        Self::comparing(JoinComparison::Equal, left, right)
    }
    pub fn comparing(
        comparison: JoinComparison,
        left: impl Fn(&L) -> K + 'a,
        right: impl Fn(&R) -> K + 'a,
    ) -> Self {
        Self {
            definition: Definition::new(left, right),
            comparison,
            ids: PhantomData,
        }
    }
    /// Return a fresh definition with this post-join predicate. Prior clones and
    /// already-built nodes keep their original behavior; this does not mutate them.
    pub fn with_filter(mut self, predicate: impl Fn(&L, &R) -> bool + 'a) -> Self {
        self.definition = self.definition.filtered(predicate);
        self
    }
    pub fn apply<S: AdditiveScore + 'static>(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        b.validate(left)?;
        b.validate(right)?;
        let parents = [left.node, right.node];
        if let Some(port) = self.definition.registrations.find(&b.owner, parents) {
            return Ok(port);
        }
        let lk = self.definition.left.clone();
        let rk = self.definition.right.clone();
        let joined =
            b.join_compare(left, right, self.comparison, move |v| lk(v), move |v| rk(v))?;
        let out = if let Some(predicate) = self.definition.predicate.clone() {
            b.filter(&joined, move |(l, r)| predicate(l, r))?
        } else {
            joined
        };
        self.definition.registrations.remember(parents, &out);
        Ok(out)
    }
}

/// Reusable existence/absence check. Without a predicate this uses the equality
/// count index with linear retained state; with_filter uses the indexed predicate
/// relation. Constant keys test any right occurrence. Stable-ID inequality in a
/// predicate expresses "other"; filtering unassigned rows is explicit upstream.
/// Mode and callbacks are fixed per definition. Sharing and lifetime rules are
/// those of SharedJoin; an existence result keeps each qualifying left ID once.
pub struct SharedExistence<'a, I, J, L, R, K> {
    definition: Definition<'a, L, R, K>,
    mode: ExistenceMode,
    ids: PhantomData<fn() -> (I, J)>,
}
impl<I, J, L, R, K> Clone for SharedExistence<'_, I, J, L, R, K> {
    fn clone(&self) -> Self {
        Self {
            definition: self.definition.clone(),
            mode: self.mode,
            ids: PhantomData,
        }
    }
}
impl<
    'a,
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    K: Ord + Clone + 'static,
> SharedExistence<'a, I, J, L, R, K>
{
    pub fn new(
        mode: ExistenceMode,
        left: impl Fn(&L) -> K + 'a,
        right: impl Fn(&R) -> K + 'a,
    ) -> Self {
        Self {
            definition: Definition::new(left, right),
            mode,
            ids: PhantomData,
        }
    }
    /// Produce a fresh predicate-bearing definition without changing old clones.
    pub fn with_filter(mut self, predicate: impl Fn(&L, &R) -> bool + 'a) -> Self {
        self.definition = self.definition.filtered(predicate);
        self
    }
    pub fn apply<S: AdditiveScore + 'static>(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
    ) -> Result<StreamPort<I, L>, GraphError> {
        b.validate(left)?;
        b.validate(right)?;
        let parents = [left.node, right.node];
        if let Some(port) = self.definition.registrations.find(&b.owner, parents) {
            return Ok(port);
        }
        let lk = self.definition.left.clone();
        let rk = self.definition.right.clone();
        let out = if let Some(predicate) = self.definition.predicate.clone() {
            b.existence(
                left,
                right,
                self.mode,
                move |v| lk(v),
                move |v| rk(v),
                move |l, r| predicate(l, r),
            )?
        } else {
            b.existence_equal(left, right, self.mode, move |v| lk(v), move |v| rk(v))?
        };
        self.definition.registrations.remember(parents, &out);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SimpleScore;
    #[test]
    fn relation_registrations_validate_both_parents_and_release_old_builders() {
        let join = SharedJoin::<usize, usize, i32, i32, i32>::new(|a| *a, |b| *b);
        let exists = SharedExistence::<usize, usize, i32, i32, i32>::new(
            ExistenceMode::Exists,
            |a| *a,
            |b| *b,
        );
        for _ in 0..100 {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let l = b.source::<usize, i32>();
            let r = b.source::<usize, i32>();
            join.apply(&mut b, l.port(), r.port()).unwrap();
            exists.apply(&mut b, l.port(), r.port()).unwrap();
            assert_eq!(join.definition.registrations.0.borrow().len(), 1);
            assert_eq!(exists.definition.registrations.0.borrow().len(), 1);
            let owner = Rc::downgrade(&b.owner);
            drop(l);
            drop(r);
            drop(b);
            assert!(owner.upgrade().is_none());
        }
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let l = b.source::<usize, i32>();
        let mut foreign = StreamGraphBuilder::<SimpleScore>::new();
        let r = foreign.source::<usize, i32>();
        assert!(matches!(
            exists.apply(&mut b, l.port(), r.port()),
            Err(GraphError::ForeignPort)
        ));
        assert_eq!(b.nodes.len(), 1);
    }
}
