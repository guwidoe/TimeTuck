//! Bounded independent candidate snapshots with indexed Cartesian composition.
use crate::{SelectionLifecycleEvent, SelectionSource, random::SplitMix64};
use std::{error::Error, fmt, iter::FusedIterator, marker::PhantomData, sync::Arc};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectionProductError {
    UnboundedChild {
        child: usize,
    },
    CandidateLimit {
        child: usize,
        maximum: usize,
    },
    SizeMismatch {
        child: usize,
        declared: usize,
        actual: usize,
    },
    CardinalityOverflow,
    InvalidDraw {
        child: usize,
        index: usize,
        bound: usize,
    },
}
impl fmt::Display for SelectionProductError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "candidate product: {self:?}")
    }
}
impl Error for SelectionProductError {}

/// Collect each finite child independently under a per-child limit, never the
/// combinations. Child lifecycle events are explicitly forwarded once each.
/// Children must be independent: context-dependent suffix choices belong in a
/// dependent traversal, not this snapshot. Use bounded wrappers for repeating
/// sources. Collection failure publishes no snapshot, but source callback effects
/// are not reversible. Existing snapshots survive source edits and lifecycle ends.
pub struct SelectionProduct<C, T, S> {
    children: Vec<S>,
    ignore_empty: bool,
    maximum: usize,
    marker: PhantomData<fn(&C) -> T>,
}
impl<C, T, S: SelectionSource<C, T>> SelectionProduct<C, T, S> {
    pub fn new(children: Vec<S>, ignore_empty: bool, maximum_per_child: usize) -> Self {
        Self {
            children,
            ignore_empty,
            maximum: maximum_per_child,
            marker: PhantomData,
        }
    }
    pub fn children(&self) -> &[S] {
        &self.children
    }
    pub fn children_mut(&mut self) -> &mut [S] {
        &mut self.children
    }
    pub fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        for child in &mut self.children {
            child.event(event, context);
        }
    }
    pub fn snapshot(&mut self, context: &C) -> Result<ProductSnapshot<T>, SelectionProductError> {
        let mut pools = Vec::with_capacity(self.children.len());
        let mut has_empty = false;
        for (index, child) in self.children.iter_mut().enumerate() {
            if child.is_never_ending() {
                return Err(SelectionProductError::UnboundedChild { child: index });
            }
            let declared = child.size(context);
            if declared.is_some_and(|n| n > self.maximum) {
                return Err(SelectionProductError::CandidateLimit {
                    child: index,
                    maximum: self.maximum,
                });
            }
            let mut pool = vec![];
            for item in child.open(context) {
                if pool.len() == self.maximum {
                    return Err(SelectionProductError::CandidateLimit {
                        child: index,
                        maximum: self.maximum,
                    });
                }
                pool.push(Arc::new(item));
            }
            if let Some(declared) = declared
                && declared != pool.len()
            {
                return Err(SelectionProductError::SizeMismatch {
                    child: index,
                    declared,
                    actual: pool.len(),
                });
            }
            if pool.is_empty() {
                has_empty = true;
            } else {
                pools.push((index, Arc::<[Arc<T>]>::from(pool)));
            }
        }
        if (has_empty && !self.ignore_empty) || pools.is_empty() {
            return Ok(ProductSnapshot {
                pools: Arc::new([]),
                count: 0,
            });
        }
        let count = pools
            .iter()
            .try_fold(1u128, |n, (_, pool)| n.checked_mul(pool.len() as u128))
            .ok_or(SelectionProductError::CardinalityOverflow)?;
        Ok(ProductSnapshot {
            pools: pools.into(),
            count,
        })
    }
}

type Pool<T> = (usize, Arc<[Arc<T>]>);
/// Immutable non-Clone candidate pools. Occurrences, including equal payloads,
/// remain distinct. Cardinality uses checked u128 arithmetic. Ignored empty
/// children are omitted; zero children/all empty children produce zero rows.
pub struct ProductSnapshot<T> {
    pools: Arc<[Pool<T>]>,
    count: u128,
}
impl<T> Clone for ProductSnapshot<T> {
    fn clone(&self) -> Self {
        Self {
            pools: self.pools.clone(),
            count: self.count,
        }
    }
}
impl<T> ProductSnapshot<T> {
    pub fn cardinality(&self) -> u128 {
        self.count
    }
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    /// Retained occurrences for an original child index. Empty/omitted children
    /// are absent; when an empty child annihilates the product, all pools are absent.
    pub fn child_pool(&self, child: usize) -> Option<&[Arc<T>]> {
        self.pools
            .iter()
            .find(|(index, _)| *index == child)
            .map(|(_, pool)| &**pool)
    }
    /// Indexed original order, with the last nonempty child varying fastest.
    pub fn get(&self, mut index: u128) -> Option<Vec<Arc<T>>> {
        if index >= self.count {
            return None;
        }
        let mut row = Vec::with_capacity(self.pools.len());
        for (_, pool) in self.pools.iter().rev() {
            row.push(pool[(index % pool.len() as u128) as usize].clone());
            index /= pool.len() as u128;
        }
        row.reverse();
        Some(row)
    }
    pub fn iter(&self) -> ProductSelectionIter<T, fn(usize, usize) -> usize> {
        ProductSelectionIter {
            snapshot: self.clone(),
            next: 0,
            random: false,
            draw: |_, _| 0,
        }
    }
    /// Independent uniform draws in every nonempty child, with replacement.
    /// Native seeds do not reproduce Java PRNG traces. Bound provider consumption.
    pub fn samples(&self, seed: u64) -> ProductSelectionIter<T, impl FnMut(usize, usize) -> usize> {
        let mut rng = SplitMix64::new(seed);
        self.samples_with(move |_, bound| rng.below_wide(bound as u128) as usize)
    }
    /// Callback receives original child index and exclusive positive bound.
    /// Empty products consume no draws. Invalid callback draws panic in iteration;
    /// use try_sample_with() to receive a checked error instead.
    pub fn samples_with<D: FnMut(usize, usize) -> usize>(
        &self,
        draw: D,
    ) -> ProductSelectionIter<T, D> {
        ProductSelectionIter {
            snapshot: self.clone(),
            next: 0,
            random: true,
            draw,
        }
    }
    pub fn try_sample_with(
        &self,
        mut draw: impl FnMut(usize, usize) -> usize,
    ) -> Result<Option<Vec<Arc<T>>>, SelectionProductError> {
        if self.is_empty() {
            return Ok(None);
        }
        let mut row = Vec::with_capacity(self.pools.len());
        for (child, pool) in self.pools.iter() {
            let index = draw(*child, pool.len());
            let item = pool.get(index).ok_or(SelectionProductError::InvalidDraw {
                child: *child,
                index,
                bound: pool.len(),
            })?;
            row.push(item.clone());
        }
        Ok(Some(row))
    }
}
pub struct ProductSelectionIter<T, D> {
    snapshot: ProductSnapshot<T>,
    next: u128,
    random: bool,
    draw: D,
}
impl<T, D: FnMut(usize, usize) -> usize> ProductSelectionIter<T, D> {
    /// Random semantics remain repeating even for an empty product.
    pub fn is_never_ending(&self) -> bool {
        self.random
    }
    pub fn has_next(&self) -> bool {
        self.snapshot.count > 0 && (self.random || self.next < self.snapshot.count)
    }
}
impl<T, D: FnMut(usize, usize) -> usize> Iterator for ProductSelectionIter<T, D> {
    type Item = Vec<Arc<T>>;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.has_next() {
            return None;
        }
        if self.random {
            self.snapshot
                .try_sample_with(&mut self.draw)
                .expect("product draw must be below its bound")
        } else {
            let row = self.snapshot.get(self.next);
            self.next += 1;
            row
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        if !self.has_next() {
            return (0, Some(0));
        }
        if self.random {
            return (usize::MAX, None);
        }
        match usize::try_from(self.snapshot.count - self.next) {
            Ok(n) => (n, Some(n)),
            Err(_) => (usize::MAX, None),
        }
    }
}
impl<T, D: FnMut(usize, usize) -> usize> FusedIterator for ProductSelectionIter<T, D> {}
