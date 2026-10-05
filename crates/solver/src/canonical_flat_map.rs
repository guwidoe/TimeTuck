//! Expansion with explicit equivalence keys and first-occurrence representatives.
use crate::{AdditiveScore, FlatMapNode, GraphError, StreamGraphBuilder, StreamPort};
use std::collections::BTreeMap;

fn canonicalize<U: Clone, K: Ord>(
    rows: impl IntoIterator<Item = U>,
    key: impl Fn(&U) -> K,
) -> Vec<U> {
    let mut representatives = BTreeMap::new();
    rows.into_iter()
        .map(|row| representatives.entry(key(&row)).or_insert(row).clone())
        .collect()
}
impl<'a, I: Ord + Clone, T, U: Clone + 'a> FlatMapNode<'a, I, T, U> {
    /// Expand each input, replacing equal-key payloads with a clone of the first
    /// payload for that key in this expansion. Every occurrence and its position
    /// remain present. `Rc`/`Arc` payloads therefore share representative identity.
    ///
    /// Representatives are local to one expansion, not shared across input IDs
    /// or retained across updates. Reordering may select a different representative.
    /// The mapper and key must be pure; keys define the intended equivalence.
    /// Callback/iterator/key/clone panics leave node state unchanged. Retraction
    /// never calls the mapper or key. Work is O(n log d) for n outputs/d keys.
    pub fn new_canonical<K: Ord, R: IntoIterator<Item = U>>(
        mapping: impl Fn(&T) -> R + 'a,
        key: impl Fn(&U) -> K + 'a,
    ) -> Self {
        Self::new(move |row| canonicalize(mapping(row), &key))
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Expand with first-occurrence representatives for equal keys, preserving
    /// duplicates and positional IDs. Key equivalence applies to the entire output
    /// payload; include retained tuple prefixes in the key when they vary.
    /// Each replacement chooses representatives afresh within that input's new
    /// expansion. This is not `distinct` or a cache shared between source rows.
    /// Graph transactions, rollback and activity pruning follow ordinary flat_map.
    ///
    /// ```
    /// use timetuck::*;
    /// use std::rc::Rc;
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// let s = b.source::<usize, Vec<Rc<i32>>>();
    /// let expanded = b.flat_map_canonical(s.port(), Clone::clone, |v| **v)?;
    /// let data = b.dataset(&expanded)?;
    /// let mut g = b.build();
    /// let first = Rc::new(7);
    /// let mut tx = g.transaction();
    /// tx.apply(&s, TupleChange::put(0, vec![first.clone(), Rc::new(7)]))?;
    /// tx.commit()?;
    /// let rows = g.dataset_rows(&data)?;
    /// assert_eq!(rows.len(), 2);
    /// assert!(Rc::ptr_eq(&rows[&(0, 1)], &first));
    /// # Ok::<(), GraphError>(())
    /// ```
    pub fn flat_map_canonical<
        I: Ord + Clone + 'static,
        T: Clone + 'static,
        U: Clone + 'static,
        K: Ord,
        R: IntoIterator<Item = U>,
    >(
        &mut self,
        input: &StreamPort<I, T>,
        mapping: impl Fn(&T) -> R + 'a,
        key: impl Fn(&U) -> K + 'a,
    ) -> Result<StreamPort<(I, usize), U>, GraphError> {
        self.flat_map(input, move |row| canonicalize(mapping(row), &key))
    }
}
