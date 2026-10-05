//! Fact-key memoization integrated with graph routing and reusable definitions.
use super::shared::Registrations;
use super::*;
use crate::PrecomputeNode;
struct Precomputed<'a, I, T, K, U>(PrecomputeNode<'a, I, T, K, U>);
impl<
    I: Ord + Clone + 'static,
    T: Clone + 'static,
    K: PartialEq + Clone + 'static,
    U: Clone + 'static,
    S: AdditiveScore,
> Node<S> for Precomputed<'_, I, T, K, U>
{
    fn clear(&mut self) {
        self.0.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.0.checkpoint()
    }
    fn restore(&mut self, c: Box<dyn Any>) {
        self.0.restore(c);
    }
    fn apply(&mut self, _: bool, event: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
        Ok(erase(self.0.apply(typed::<I, T>(event).clone())))
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Memoize a fact-only projection by stable input ID and an explicit fact key.
    /// Variable-only changes with equal keys do not rerun computation or emit
    /// downstream updates. Fact changes and removal invalidate the affected row.
    /// Use IDs in cached output and join to the live input for current variables.
    /// All key dependencies must be explicit; no reflection or mutation tracking
    /// discovers missing dependencies. Graph rollback restores the cache as well.
    ///
    /// ```
    /// use timetuck::*;
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// // (entity ID, group, current assignment)
    /// let live = b.source::<usize, (usize, i32, Option<i32>)>();
    /// let facts = b.precompute(live.port(), |r| (r.0, r.1), |r| (r.0, r.1))?;
    /// let eligible = b.filter(&facts, |(_, group)| *group == 7)?;
    /// let current = b.join_equal(&eligible, live.port(), |r| r.0, |r| r.0)?;
    /// b.score("value", &current, |(_, (_, _, value))| SimpleScore::new(value.unwrap_or(0) as i64))?;
    /// let mut graph = b.build();
    /// let mut tx = graph.transaction();
    /// tx.apply(&live, TupleChange::put(0, (0, 7, Some(3))))?;
    /// assert_eq!(tx.score()?, SimpleScore::new(3));
    /// # Ok::<(), GraphError>(())
    /// ```
    pub fn precompute<
        I: Ord + Clone + 'static,
        T: Clone + 'static,
        K: PartialEq + Clone + 'static,
        U: Clone + 'static,
    >(
        &mut self,
        input: &StreamPort<I, T>,
        key: impl Fn(&T) -> K + 'a,
        compute: impl Fn(&T) -> U + 'a,
    ) -> Result<StreamPort<I, U>, GraphError> {
        self.validate(input)?;
        let out = self.push(Precomputed(PrecomputeNode::<I, T, K, U>::new(key, compute)));
        self.connect(input.node, out.node, false);
        Ok(out)
    }
}
/// Reusable precomputation identity. Clones applied to the same parent and builder
/// share a cache and computation. Fresh definitions, parents and builders remain
/// independent. Registrations are weak, prune dead owners on apply, and retain no
/// graph state. Built nodes retain both callbacks after the definition is dropped.
/// The explicit fact-key and live-value rules of builder.precompute apply.
pub struct SharedPrecompute<'a, I, T, K, U> {
    key: Rc<dyn Fn(&T) -> K + 'a>,
    compute: Rc<dyn Fn(&T) -> U + 'a>,
    registrations: Registrations,
    marker: PhantomData<fn() -> I>,
}
impl<I, T, K, U> Clone for SharedPrecompute<'_, I, T, K, U> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            compute: self.compute.clone(),
            registrations: self.registrations.clone(),
            marker: PhantomData,
        }
    }
}
impl<
    'a,
    I: Ord + Clone + 'static,
    T: Clone + 'static,
    K: PartialEq + Clone + 'static,
    U: Clone + 'static,
> SharedPrecompute<'a, I, T, K, U>
{
    pub fn new(key: impl Fn(&T) -> K + 'a, compute: impl Fn(&T) -> U + 'a) -> Self {
        Self {
            key: Rc::new(key),
            compute: Rc::new(compute),
            registrations: Registrations::default(),
            marker: PhantomData,
        }
    }
    pub fn apply<S: AdditiveScore + 'static>(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        input: &StreamPort<I, T>,
    ) -> Result<StreamPort<I, U>, GraphError> {
        b.validate(input)?;
        if let Some(out) = self.registrations.find(&b.owner, input.node) {
            return Ok(out);
        }
        let key = self.key.clone();
        let compute = self.compute.clone();
        let out = b.precompute(input, move |v| key(v), move |v| compute(v))?;
        self.registrations.remember(input.node, &out);
        Ok(out)
    }
}
