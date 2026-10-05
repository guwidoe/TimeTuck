//! Explicit reusable grouping and collection definitions.
use super::shared::Registrations;
use super::*;
type Mapping<'a, T, U> = dyn Fn(&T) -> U + 'a;

/// One row per distinct mapped key. Tuple keys express multi-key grouping.
///
/// Reapplying a definition or clone to the same parent in the same builder
/// reuses its complete pipeline. Fresh definitions, changed parents and other
/// builders remain independent. Ordinary builder calls are unaffected.
/// Mappings must be pure and deterministic; application does not invoke them.
/// Graph checkpoints cover shared state and all consumers. Only weak builder
/// identities are retained, pruned on application; lookup scans active bindings.
/// Built nodes retain the mapping after the definition is dropped.
pub struct SharedGroupKeys<'a, I, T, K> {
    callback: Rc<Mapping<'a, T, K>>,
    registrations: Registrations,
    marker: PhantomData<fn() -> I>,
}
impl<I, T, K> Clone for SharedGroupKeys<'_, I, T, K> {
    fn clone(&self) -> Self {
        Self {
            callback: self.callback.clone(),
            registrations: self.registrations.clone(),
            marker: PhantomData,
        }
    }
}
impl<'a, I: Ord + Clone + 'static, T: Clone + 'static, K: Ord + Clone + 'static>
    SharedGroupKeys<'a, I, T, K>
{
    pub fn new(mapping: impl Fn(&T) -> K + 'a) -> Self {
        Self {
            callback: Rc::new(mapping),
            registrations: Registrations::default(),
            marker: PhantomData,
        }
    }
    pub fn apply<S: AdditiveScore + 'static>(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        input: &StreamPort<I, T>,
    ) -> Result<StreamPort<K, K>, GraphError> {
        b.validate(input)?;
        if let Some(port) = self.registrations.find(&b.owner, input.node) {
            return Ok(port);
        }
        let callback = self.callback.clone();
        let mapped = b.map(input, move |row| callback(row))?;
        let out = b.distinct(&mapped)?;
        self.registrations.remember(input.node, &out);
        Ok(out)
    }
}

/// Keyed reversible aggregation. Tuple collectors support multiple outputs.
///
/// ```
/// use timetuck::*;
/// type Both = (CountCollector, SumCollector);
/// let grouped = SharedGroup::<usize, (i32, i64), i32, ((), i64), Both>::new(
///     |row| (row.0, ((), row.1)));
/// let mut b = StreamGraphBuilder::<SimpleScore>::new();
/// let s = b.source::<usize, (i32, i64)>();
/// let first = grouped.apply(&mut b, s.port())?;
/// let second = grouped.clone().apply(&mut b, s.port())?;
/// assert!(first.same_node(&second));
/// b.score("cost", &first, |(_, (_, sum))| SimpleScore::new(-sum))?;
/// let mut graph = b.build();
/// let mut trial = graph.transaction();
/// trial.apply(&s, TupleChange::put(0, (7, 12)))?;
/// assert_eq!(trial.score()?, SimpleScore::new(-12));
/// # Ok::<(), GraphError>(())
/// ```
///
/// Reapplying a definition or clone to the same parent in the same builder
/// reuses its complete pipeline. Fresh definitions, changed parents and other
/// builders remain independent. Ordinary builder calls are unaffected.
/// Mappings must be pure and deterministic; application does not invoke them.
/// Graph checkpoints cover shared state and all consumers. Only weak builder
/// identities are retained, pruned on application; lookup scans active bindings.
/// Built nodes retain the mapping after the definition is dropped.
pub struct SharedGroup<'a, I, T, K, V, C> {
    callback: Rc<Mapping<'a, T, (K, V)>>,
    registrations: Registrations,
    marker: PhantomData<fn() -> (I, C)>,
}
impl<I, T, K, V, C> Clone for SharedGroup<'_, I, T, K, V, C> {
    fn clone(&self) -> Self {
        Self {
            callback: self.callback.clone(),
            registrations: self.registrations.clone(),
            marker: PhantomData,
        }
    }
}
impl<
    'a,
    I: Ord + Clone + 'static,
    T: Clone + 'static,
    K: Ord + Clone + 'static,
    V: Clone + 'static,
    C: ReversibleCollector<V> + 'static,
> SharedGroup<'a, I, T, K, V, C>
where
    C::Output: 'static,
{
    pub fn new(mapping: impl Fn(&T) -> (K, V) + 'a) -> Self {
        Self {
            callback: Rc::new(mapping),
            registrations: Registrations::default(),
            marker: PhantomData,
        }
    }
    pub fn apply<S: AdditiveScore + 'static>(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        input: &StreamPort<I, T>,
    ) -> Result<StreamPort<K, (K, C::Output)>, GraphError> {
        b.validate(input)?;
        if let Some(port) = self.registrations.find(&b.owner, input.node) {
            return Ok(port);
        }
        let callback = self.callback.clone();
        let out = b.group_collect_with::<I, T, K, V, C>(input, move |row| callback(row))?;
        self.registrations.remember(input.node, &out);
        Ok(out)
    }
}

/// Global reversible aggregation. Empty input emits no row.
///
/// Reapplying a definition or clone to the same parent in the same builder
/// reuses its complete pipeline. Fresh definitions, changed parents and other
/// builders remain independent. Ordinary builder calls are unaffected.
/// Mappings must be pure and deterministic; application does not invoke them.
/// Graph checkpoints cover shared state and all consumers. Only weak builder
/// identities are retained, pruned on application; lookup scans active bindings.
/// Built nodes retain the mapping after the definition is dropped.
pub struct SharedCollect<'a, I, T, V, C> {
    callback: Rc<Mapping<'a, T, V>>,
    registrations: Registrations,
    marker: PhantomData<fn() -> (I, C)>,
}
impl<I, T, V, C> Clone for SharedCollect<'_, I, T, V, C> {
    fn clone(&self) -> Self {
        Self {
            callback: self.callback.clone(),
            registrations: self.registrations.clone(),
            marker: PhantomData,
        }
    }
}
impl<
    'a,
    I: Ord + Clone + 'static,
    T: Clone + 'static,
    V: Clone + 'static,
    C: ReversibleCollector<V> + 'static,
> SharedCollect<'a, I, T, V, C>
where
    C::Output: 'static,
{
    pub fn new(mapping: impl Fn(&T) -> V + 'a) -> Self {
        Self {
            callback: Rc::new(mapping),
            registrations: Registrations::default(),
            marker: PhantomData,
        }
    }
    pub fn apply<S: AdditiveScore + 'static>(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        input: &StreamPort<I, T>,
    ) -> Result<StreamPort<(), C::Output>, GraphError> {
        b.validate(input)?;
        if let Some(port) = self.registrations.find(&b.owner, input.node) {
            return Ok(port);
        }
        let callback = self.callback.clone();
        let out = b.collect::<I, T, V, C>(input, move |row| callback(row))?;
        self.registrations.remember(input.node, &out);
        Ok(out)
    }
}

/// Keyed lists, retaining duplicate occurrences in ascending input-ID order.
///
/// Reapplying a definition or clone to the same parent in the same builder
/// reuses its complete pipeline. Fresh definitions, changed parents and other
/// builders remain independent. Ordinary builder calls are unaffected.
/// Mappings must be pure and deterministic; application does not invoke them.
/// Graph checkpoints cover shared state and all consumers. Only weak builder
/// identities are retained, pruned on application; lookup scans active bindings.
/// Built nodes retain the mapping after the definition is dropped.
pub struct SharedGroupList<'a, I, T, K, V> {
    callback: Rc<Mapping<'a, T, (K, V)>>,
    registrations: Registrations,
    marker: PhantomData<fn() -> I>,
}
impl<I, T, K, V> Clone for SharedGroupList<'_, I, T, K, V> {
    fn clone(&self) -> Self {
        Self {
            callback: self.callback.clone(),
            registrations: self.registrations.clone(),
            marker: PhantomData,
        }
    }
}
impl<'a, I: Ord + Clone + 'static, T: Clone + 'static, K: Ord + Clone + 'static, V: Clone + 'static>
    SharedGroupList<'a, I, T, K, V>
{
    pub fn new(mapping: impl Fn(&T) -> (K, V) + 'a) -> Self {
        Self {
            callback: Rc::new(mapping),
            registrations: Registrations::default(),
            marker: PhantomData,
        }
    }
    pub fn apply<S: AdditiveScore + 'static>(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        input: &StreamPort<I, T>,
    ) -> Result<GroupedListStreamPort<K, I, V>, GraphError> {
        b.validate(input)?;
        if let Some(port) = self.registrations.find(&b.owner, input.node) {
            return Ok(port);
        }
        let callback = self.callback.clone();
        let out = b.group_list(input, move |row| callback(row))?;
        self.registrations.remember(input.node, &out);
        Ok(out)
    }
}

/// Global list collection, retaining duplicate occurrences in input-ID order.
///
/// Reapplying a definition or clone to the same parent in the same builder
/// reuses its complete pipeline. Fresh definitions, changed parents and other
/// builders remain independent. Ordinary builder calls are unaffected.
/// Mappings must be pure and deterministic; application does not invoke them.
/// Graph checkpoints cover shared state and all consumers. Only weak builder
/// identities are retained, pruned on application; lookup scans active bindings.
/// Built nodes retain the mapping after the definition is dropped.
pub struct SharedCollectList<'a, I, T, V> {
    callback: Rc<Mapping<'a, T, V>>,
    registrations: Registrations,
    marker: PhantomData<fn() -> I>,
}
impl<I, T, V> Clone for SharedCollectList<'_, I, T, V> {
    fn clone(&self) -> Self {
        Self {
            callback: self.callback.clone(),
            registrations: self.registrations.clone(),
            marker: PhantomData,
        }
    }
}
impl<'a, I: Ord + Clone + 'static, T: Clone + 'static, V: Clone + 'static>
    SharedCollectList<'a, I, T, V>
{
    pub fn new(mapping: impl Fn(&T) -> V + 'a) -> Self {
        Self {
            callback: Rc::new(mapping),
            registrations: Registrations::default(),
            marker: PhantomData,
        }
    }
    pub fn apply<S: AdditiveScore + 'static>(
        &self,
        b: &mut StreamGraphBuilder<'a, S>,
        input: &StreamPort<I, T>,
    ) -> Result<StreamPort<(), ListSnapshot<I, V>>, GraphError> {
        b.validate(input)?;
        if let Some(port) = self.registrations.find(&b.owner, input.node) {
            return Ok(port);
        }
        let callback = self.callback.clone();
        let out = b.collect_list(input, move |row| callback(row))?;
        self.registrations.remember(input.node, &out);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aggregation_definitions_keep_no_graph_alive_and_reject_foreign_parents() {
        let keys = SharedGroupKeys::<usize, i64, i64>::new(|v| *v);
        let groups = SharedGroup::<usize, i64, (), i64, crate::SumCollector>::new(|v| ((), *v));
        let totals = SharedCollect::<usize, i64, i64, crate::SumCollector>::new(|v| *v);
        let lists = SharedGroupList::<usize, i64, (), i64>::new(|v| ((), *v));
        let all = SharedCollectList::<usize, i64, i64>::new(|v| *v);
        let mut foreign = StreamGraphBuilder::<crate::SimpleScore>::new();
        let source = foreign.source::<usize, i64>();
        for _ in 0..100 {
            let mut b = StreamGraphBuilder::<crate::SimpleScore>::new();
            let s = b.source::<usize, i64>();
            macro_rules! check {
                ($d:expr) => {
                    assert!(matches!(
                        $d.apply(&mut b, source.port()),
                        Err(GraphError::ForeignPort)
                    ));
                    let first = $d.apply(&mut b, s.port()).unwrap();
                    assert!(first.same_node(&$d.clone().apply(&mut b, s.port()).unwrap()));
                    drop(first);
                };
            }
            check!(keys);
            check!(groups);
            check!(totals);
            check!(lists);
            check!(all);
            let weak = Rc::downgrade(&b.owner);
            drop(s);
            drop(b);
            assert!(weak.upgrade().is_none());
        }
    }
}
