//! Stable, bounded groups of source occurrences, reusable within a lifecycle scope.
use crate::{SelectionLifecycleEvent, SelectionProduct, SelectionProductError, SelectionSource};
use std::{collections::HashMap, sync::Arc};

/// An ordered group of separately owned occurrences. Equal payloads may occur
/// more than once; identity is retained by each Arc without requiring T: Clone.
pub type SelectionGroup<T> = Arc<[Arc<T>]>;

pub struct GroupedSelection<T> {
    groups: Arc<[SelectionGroup<T>]>,
}
impl<T> Clone for GroupedSelection<T> {
    fn clone(&self) -> Self {
        Self {
            groups: self.groups.clone(),
        }
    }
}
impl<T> GroupedSelection<T> {
    pub fn groups(&self) -> &[SelectionGroup<T>] {
        &self.groups
    }
    pub fn len(&self) -> usize {
        self.groups.len()
    }
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
    pub fn get(&self, index: usize) -> Option<&SelectionGroup<T>> {
        self.groups.get(index)
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &SelectionGroup<T>> {
        self.groups.iter()
    }
}

/// Group a finite source by a context-dependent equivalence relation. Groups
/// follow the first occurrence of each class; members retain source order and
/// duplicate occurrences. The comparator must be an equivalence relation. Each
/// candidate is compared to existing representatives until a class is found.
/// The default comparison cost is quadratic; no Hash or Clone bound is imposed.
/// Opt in with `with_hash` when an equivalence-consistent contextual hash is
/// available. Hash indexing preserves class/member order and duplicate occurrences;
/// collisions still compare representatives in encounter order. Index storage is
/// temporary per rebuild, O(classes); worst-case collisions remain quadratic.
///
/// The first size/open/try_groups call collects and groups the source, subject to
/// its finite declaration, exact size and the input cap. Subsequent calls share
/// immutable handles until invalidated. Every lifecycle event invalidates first,
/// then forwards once to the child, including for empty sources. Child/comparator
/// edits invalidate too. Direct callers must invalidate when grouping facts in
/// the context change; lifecycle-driven CachedMoveProvider automatically does
/// so at each scan. Use a Step cache for assignment-dependent pillar grouping.
///
/// Held snapshots survive invalidation. A failing collection or panicking
/// comparator publishes no partial groups; callback side effects are not undone.
/// try_groups returns collection errors. SelectionSource size/open panic on error
/// instead of misreporting an empty source. Grouped output is finite; apply random
/// sampling to the groups through the ordinary selection cache if desired.
pub struct GroupedSelectionSource<C, T, S, F, H = fn(&C, &T) -> u64> {
    input: SelectionProduct<C, T, S>,
    equivalent: F,
    hash: Option<H>,
    prepared: Option<GroupedSelection<T>>,
}
impl<C, T, S: SelectionSource<C, T>, F: FnMut(&C, &T, &T) -> bool>
    GroupedSelectionSource<C, T, S, F>
{
    pub fn new(source: S, maximum_items: usize, equivalent: F) -> Self {
        Self {
            input: SelectionProduct::new(vec![source], false, maximum_items),
            equivalent,
            hash: None,
            prepared: None,
        }
    }
}
impl<C, T, S, F, H> GroupedSelectionSource<C, T, S, F, H>
where
    S: SelectionSource<C, T>,
    F: FnMut(&C, &T, &T) -> bool,
    H: FnMut(&C, &T) -> u64,
{
    /// Index representatives by a context-dependent hash. Equivalent items MUST
    /// have the same hash during a rebuild; unequal items may collide. Both hash
    /// and equivalence must be stable for that immutable context. No Hash or Clone
    /// bound is added to payloads, and callbacks may borrow caller-owned state.
    ///
    /// Hash runs once per source occurrence in encounter order; equivalence runs
    /// only against representatives in its bucket. This deliberately changes
    /// comparator invocation counts, so comparator side effects must not determine
    /// equivalence. Default construction keeps its original comparison path.
    ///
    /// Switching hash functions invalidates prepared groups. After context changes,
    /// send a lifecycle event or explicitly invalidate before reopening. Hash and
    /// comparator panics publish no partial groups/index; external callback side
    /// effects cannot be rolled back. Previously held snapshots remain valid.
    pub fn with_hash<N: FnMut(&C, &T) -> u64>(
        self,
        hash: N,
    ) -> GroupedSelectionSource<C, T, S, F, N> {
        GroupedSelectionSource {
            input: self.input,
            equivalent: self.equivalent,
            hash: Some(hash),
            prepared: None,
        }
    }
    /// Access the optional hash callback, invalidating prepared groups first.
    pub fn hash_mut(&mut self) -> Option<&mut H> {
        self.invalidate();
        self.hash.as_mut()
    }
    pub fn source(&self) -> &S {
        &self.input.children()[0]
    }
    pub fn source_mut(&mut self) -> &mut S {
        self.invalidate();
        &mut self.input.children_mut()[0]
    }
    pub fn equivalent_mut(&mut self) -> &mut F {
        self.invalidate();
        &mut self.equivalent
    }
    pub fn invalidate(&mut self) {
        self.prepared = None;
    }
    pub fn try_groups(
        &mut self,
        context: &C,
    ) -> Result<GroupedSelection<T>, SelectionProductError> {
        if let Some(groups) = &self.prepared {
            return Ok(groups.clone());
        }
        let input = self.input.snapshot(context)?;
        let mut groups: Vec<Vec<Arc<T>>> = vec![];
        let items = input.child_pool(0).unwrap_or(&[]);
        if let Some(hash) = &mut self.hash {
            let mut buckets: HashMap<u64, Vec<usize>> = HashMap::new();
            for item in items {
                let bucket = buckets.entry(hash(context, item)).or_default();
                if let Some(&index) = bucket
                    .iter()
                    .find(|&&index| (self.equivalent)(context, &groups[index][0], item))
                {
                    groups[index].push(item.clone());
                } else {
                    bucket.push(groups.len());
                    groups.push(vec![item.clone()]);
                }
            }
        } else {
            for item in items {
                if let Some(group) = groups
                    .iter_mut()
                    .find(|group| (self.equivalent)(context, &group[0], item))
                {
                    group.push(item.clone());
                } else {
                    groups.push(vec![item.clone()]);
                }
            }
        }
        let result = GroupedSelection {
            groups: groups.into_iter().map(Arc::from).collect(),
        };
        self.prepared = Some(result.clone());
        Ok(result)
    }
}
impl<C, T, S, F, H> SelectionSource<C, SelectionGroup<T>> for GroupedSelectionSource<C, T, S, F, H>
where
    S: SelectionSource<C, T>,
    F: FnMut(&C, &T, &T) -> bool,
    H: FnMut(&C, &T) -> u64,
{
    fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        self.invalidate();
        self.input.event(event, context);
    }
    fn size(&mut self, context: &C) -> Option<usize> {
        Some(
            self.try_groups(context)
                .unwrap_or_else(|e| panic!("grouped selection: {e}"))
                .len(),
        )
    }
    fn open<'a>(&'a mut self, context: &'a C) -> Box<dyn Iterator<Item = SelectionGroup<T>> + 'a>
    where
        SelectionGroup<T>: 'a,
    {
        let groups = self
            .try_groups(context)
            .unwrap_or_else(|e| panic!("grouped selection: {e}"));
        Box::new((0..groups.len()).map(move |i| groups.groups[i].clone()))
    }
}
