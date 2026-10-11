//! Direct random draws from bounded, reusable groups of occurrences.
use crate::{
    SelectionGroup, SelectionLifecycleEvent, SelectionProduct, SelectionProductError,
    SelectionSource, SnapshotMap, SubPillarSelection,
};
use std::{
    cmp::Ordering, collections::BTreeSet, error::Error, fmt, iter::FusedIterator, sync::Arc,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubgroupSelectionError {
    Collection(SelectionProductError),
    MemberLimit { maximum: usize },
    InvalidDraw { bound: usize, actual: usize },
}
impl fmt::Display for SubgroupSelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "subgroup selection: {self:?}")
    }
}
impl Error for SubgroupSelectionError {}
impl From<SelectionProductError> for SubgroupSelectionError {
    fn from(e: SelectionProductError) -> Self {
        Self::Collection(e)
    }
}

/// Sample a finite source of groups with replacement without enumerating subsets.
/// Choose an eligible group uniformly, then a size uniformly within clipped bounds,
/// then members without replacement (or a contiguous window). This distribution is
/// not uniform across all possible subsets. Empty/undersized groups are omitted.
/// Consecutive selection uses member order; ordered_by sorts members and then
/// groups by their first member before drawing. Comparator ties retain source order.
///
/// Collection is cached until an event, explicit invalidation or source edit.
/// Every event invalidates before forwarding once. Keys/context facts must stay
/// stable within that scope. Sources, contexts and members need not be Clone.
/// Snapshots/outputs retain occurrence identity; sampling does not copy payloads.
///
/// The source reports never-ending even when empty; size is Some(0) for no eligible
/// groups and None otherwise. Bound it with LimitedSelectionSource before feeding
/// a Step-scoped CachedMoveProvider. Collection has separate group/member caps.
/// Native seeded streams continue across openings/events; reseed explicitly to
/// restart. Native seeds do not reproduce Java PRNG traces. try_open_with accepts
/// bounded index draws for other RNGs and reports an invalid draw once then fuses.
/// Trait methods panic on collection/draw errors, never report them as exhaustion.
/// A draw uses O(k) additional memory and O(k log k) time for k arbitrary
/// members. Small selections retain the original bounded linear rank walk; large
/// selections use an ordered exclusion index. Both consume identical rank draws
/// and retain occurrence order. Contiguous/full groups need no combinatorial search.
pub struct SubgroupSelectionSource<C, T, S, O = fn(&C, &T, &T) -> Ordering> {
    input: SelectionProduct<C, SelectionGroup<T>, S>,
    selection: SubPillarSelection,
    maximum_members: usize,
    ordering: Option<O>,
    prepared: Option<Arc<[SelectionGroup<T>]>>,
    random: crate::random::SplitMix64,
}
impl<C, T, S: SelectionSource<C, SelectionGroup<T>>> SubgroupSelectionSource<C, T, S> {
    pub fn new(
        source: S,
        maximum_groups: usize,
        maximum_members: usize,
        selection: SubPillarSelection,
        seed: u64,
    ) -> Self {
        Self {
            input: SelectionProduct::new(vec![source], false, maximum_groups),
            selection,
            maximum_members,
            ordering: None,
            prepared: None,
            random: crate::random::SplitMix64::new(seed),
        }
    }
}
impl<C, T, S: SelectionSource<C, SelectionGroup<T>>, O: FnMut(&C, &T, &T) -> Ordering>
    SubgroupSelectionSource<C, T, S, O>
{
    pub fn ordered_by<N: FnMut(&C, &T, &T) -> Ordering>(
        self,
        ordering: N,
    ) -> SubgroupSelectionSource<C, T, S, N> {
        SubgroupSelectionSource {
            input: self.input,
            selection: self.selection,
            maximum_members: self.maximum_members,
            ordering: Some(ordering),
            prepared: None,
            random: self.random,
        }
    }
    pub fn source(&self) -> &S {
        &self.input.children()[0]
    }
    pub fn source_mut(&mut self) -> &mut S {
        self.invalidate();
        &mut self.input.children_mut()[0]
    }
    pub fn invalidate(&mut self) {
        self.prepared = None;
    }
    pub fn reseed(&mut self, seed: u64) {
        self.random = crate::random::SplitMix64::new(seed);
    }
    fn prepare(&mut self, context: &C) -> Result<Arc<[SelectionGroup<T>]>, SubgroupSelectionError> {
        if let Some(groups) = &self.prepared {
            return Ok(groups.clone());
        }
        let snapshot = self.input.snapshot(context)?;
        let mut groups = Vec::new();
        let mut remaining = self.maximum_members;
        for group in snapshot.child_pool(0).unwrap_or(&[]) {
            remaining =
                remaining
                    .checked_sub(group.len())
                    .ok_or(SubgroupSelectionError::MemberLimit {
                        maximum: self.maximum_members,
                    })?;
            if group.len() < self.selection.minimum() {
                continue;
            }
            if let Some(order) = &mut self.ordering {
                let mut members = group.to_vec();
                members.sort_by(|a, b| order(context, a, b));
                groups.push(Arc::from(members));
            } else {
                groups.push((**group).clone());
            }
        }
        if let Some(order) = &mut self.ordering {
            groups.sort_by(|a, b| order(context, &a[0], &b[0]));
        }
        let groups: Arc<[SelectionGroup<T>]> = groups.into();
        self.prepared = Some(groups.clone());
        Ok(groups)
    }
    pub fn try_open_with<D: FnMut(usize) -> usize>(
        &mut self,
        context: &C,
        draw: D,
    ) -> Result<SubgroupSamples<T, D>, SubgroupSelectionError> {
        Ok(SubgroupSamples {
            groups: self.prepare(context)?,
            selection: self.selection,
            draw,
            failed: false,
        })
    }
}
impl<C, T, S: SelectionSource<C, SelectionGroup<T>>, O: FnMut(&C, &T, &T) -> Ordering>
    SelectionSource<C, SelectionGroup<T>> for SubgroupSelectionSource<C, T, S, O>
{
    fn is_never_ending(&self) -> bool {
        true
    }
    fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        self.invalidate();
        self.input.event(event, context);
    }
    fn size(&mut self, context: &C) -> Option<usize> {
        self.prepare(context)
            .unwrap_or_else(|e| panic!("{e}"))
            .is_empty()
            .then_some(0)
    }
    fn open<'a>(&'a mut self, context: &'a C) -> Box<dyn Iterator<Item = SelectionGroup<T>> + 'a>
    where
        SelectionGroup<T>: 'a,
    {
        let groups = self.prepare(context).unwrap_or_else(|e| panic!("{e}"));
        let random = &mut self.random;
        let samples = SubgroupSamples {
            groups,
            selection: self.selection,
            draw: move |n| random.below(n as u64) as usize,
            failed: false,
        };
        Box::new(samples.map(|r| r.unwrap_or_else(|e| panic!("{e}"))))
    }
}

/// Owned group snapshot and caller-owned index generator. A malformed draw returns
/// one error and terminates this iterator; empty groups never invoke the generator.
pub struct SubgroupSamples<T, D> {
    groups: Arc<[SelectionGroup<T>]>,
    selection: SubPillarSelection,
    draw: D,
    failed: bool,
}
impl<T, D: FnMut(usize) -> usize> SubgroupSamples<T, D> {
    fn below(&mut self, bound: usize) -> Result<usize, SubgroupSelectionError> {
        let actual = (self.draw)(bound);
        if actual < bound {
            Ok(actual)
        } else {
            Err(SubgroupSelectionError::InvalidDraw { bound, actual })
        }
    }
    fn sample(&mut self) -> Result<SelectionGroup<T>, SubgroupSelectionError> {
        let index = self.below(self.groups.len())?;
        let group = self.groups[index].clone();
        let n = group.len();
        if n == 1 {
            return Ok(group);
        }
        let size = self.selection.minimum()
            + self.below(self.selection.maximum().min(n) - self.selection.minimum() + 1)?;
        if size == n {
            return Ok(group);
        }
        let indices: Vec<_> = if self.selection.is_consecutive() {
            let start = self.below(n - size + 1)?;
            (start..start + size).collect()
        } else if size >= 2048 {
            let mut selected = SnapshotMap::new();
            for taken in 0..size {
                let rank = self.below(n - taken)?;
                let index = selected
                    .missing_key_at(rank)
                    .expect("valid remaining rank fits the finite group");
                debug_assert!(index < n);
                selected.insert(index, ());
            }
            selected.keys().copied().collect()
        } else {
            // Draw a rank among still-unselected positions, preserving occurrence
            // order. Sparse selected positions avoid copying the complete group.
            let mut selected = BTreeSet::new();
            for taken in 0..size {
                let mut index = self.below(n - taken)?;
                for &prior in &selected {
                    if prior <= index {
                        index += 1;
                    } else {
                        break;
                    }
                }
                selected.insert(index);
            }
            selected.into_iter().collect()
        };
        Ok(indices.into_iter().map(|i| group[i].clone()).collect())
    }
}
impl<T, D: FnMut(usize) -> usize> Iterator for SubgroupSamples<T, D> {
    type Item = Result<SelectionGroup<T>, SubgroupSelectionError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.groups.is_empty() {
            return None;
        }
        let result = self.sample();
        self.failed = result.is_err();
        Some(result)
    }
}
impl<T, D: FnMut(usize) -> usize> FusedIterator for SubgroupSamples<T, D> {}
