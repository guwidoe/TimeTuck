//! Lifecycle-aware concatenation and weighted interleaving of candidate sources.
use crate::{
    SelectionLifecycleEvent, SelectionSource, WeightedSelection, WeightedSelectionError,
    random::SplitMix64,
};
use std::{
    iter::{FusedIterator, Peekable},
    marker::PhantomData,
};

/// Interleave child streams without replacement within any child. Weights select
/// families, not individual candidates; each family's internal order is retained.
/// One item per enabled child is prefetched to discover exhaustion before drawing.
/// Exhausted children are permanently retired, even if their iterator is not fused.
/// Zero weights disable polling. Infinite children require a consumer limit.
///
/// Construction never polls children. Each next() polls at most once per live
/// child, although an individual child next() remains cooperative. Invalid draws
/// return one error and terminate; prefetched items are dropped with the cursor.
pub struct WeightedInterleave<I: Iterator, D> {
    children: Vec<Peekable<I>>,
    weights: Vec<f64>,
    distribution: WeightedSelection<usize>,
    draw: D,
    ended: bool,
}
impl<I: Iterator, D: FnMut() -> f64> WeightedInterleave<I, D> {
    pub fn new(children: Vec<(I, f64)>, draw: D) -> Result<Self, WeightedSelectionError> {
        let (children, weights): (Vec<_>, Vec<_>) = children.into_iter().unzip();
        let distribution = WeightedSelection::new(weights.iter().copied().enumerate().collect())?;
        Ok(Self {
            children: children.into_iter().map(Iterator::peekable).collect(),
            weights,
            distribution,
            draw,
            ended: false,
        })
    }
    fn advance(&mut self) -> Result<Option<I::Item>, WeightedSelectionError> {
        let mut changed = false;
        for (child, weight) in self.children.iter_mut().zip(&mut self.weights) {
            if *weight > 0.0 && child.peek().is_none() {
                *weight = 0.0;
                changed = true;
            }
        }
        if changed {
            self.distribution = self.distribution.reweighted(|i, _| self.weights[i])?;
        }
        if self.distribution.active_len() == 0 {
            return Ok(None);
        }
        let index = *self
            .distribution
            .choose((self.draw)())?
            .expect("active family");
        Ok(self.children[index].next())
    }
}
impl<I: Iterator, D: FnMut() -> f64> Iterator for WeightedInterleave<I, D> {
    type Item = Result<I::Item, WeightedSelectionError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }
        match self.advance() {
            Ok(Some(value)) => Some(Ok(value)),
            Ok(None) => {
                self.ended = true;
                None
            }
            Err(error) => {
                self.ended = true;
                Some(Err(error))
            }
        }
    }
}
impl<I: Iterator, D: FnMut() -> f64> FusedIterator for WeightedInterleave<I, D> {}

/// A weighted cursor borrowing its child sources.
pub type SelectionUnionIter<'a, T, D> = WeightedInterleave<Box<dyn Iterator<Item = T> + 'a>, D>;

/// Compose candidate sources with explicit lifecycle forwarding. Events reach all
/// children, including disabled ones; zero weights suppress size/open/poll calls.
/// Ordered iteration opens children lazily and drains each before the next.
/// Random iteration uses one buffered item per enabled child and never reopens an
/// exhausted child. Neither mode materializes complete pools or deduplicates moves.
/// Child membership/weights are fixed; source state may change at lifecycle events.
pub struct SelectionUnion<C, T, S> {
    children: Vec<(S, f64)>,
    marker: PhantomData<fn(&C) -> T>,
}
impl<C, T, S: SelectionSource<C, T>> SelectionUnion<C, T, S> {
    pub fn new(children: Vec<(S, f64)>) -> Result<Self, WeightedSelectionError> {
        WeightedSelection::new(children.iter().map(|(_, weight)| ((), *weight)).collect())?;
        Ok(Self {
            children,
            marker: PhantomData,
        })
    }
    pub fn children(&self) -> impl Iterator<Item = &S> {
        self.children.iter().map(|(s, _)| s)
    }
    pub fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        for (child, _) in &mut self.children {
            child.event(event, context);
        }
    }
    pub fn is_never_ending(&self) -> bool {
        self.children
            .iter()
            .any(|(s, w)| *w > 0.0 && s.is_never_ending())
    }
    /// Sum child-reported counts; unknown counts or usize overflow return None.
    /// This is source metadata, not a bound on repeated random child streams.
    pub fn size(&mut self, context: &C) -> Option<usize> {
        self.children
            .iter_mut()
            .filter(|(_, w)| *w > 0.0)
            .try_fold(0usize, |sum, (s, _)| sum.checked_add(s.size(context)?))
    }
    pub fn ordered<'a>(&'a mut self, context: &'a C) -> impl Iterator<Item = T> + 'a
    where
        T: 'a,
    {
        self.children
            .iter_mut()
            .filter(|(_, w)| *w > 0.0)
            .flat_map(move |(s, _)| s.open(context).fuse())
    }
    pub fn random_with<'a, D: FnMut() -> f64>(
        &'a mut self,
        context: &'a C,
        draw: D,
    ) -> SelectionUnionIter<'a, T, D>
    where
        T: 'a,
    {
        let children = self
            .children
            .iter_mut()
            .map(|(s, w)| {
                let cursor: Box<dyn Iterator<Item = T> + 'a> =
                    Box::new(std::iter::once_with(move || s.open(context)).flatten());
                (cursor, *w)
            })
            .collect();
        WeightedInterleave::new(children, draw).expect("validated union weights")
    }
    pub fn random<'a>(
        &'a mut self,
        context: &'a C,
        seed: u64,
    ) -> impl FusedIterator<Item = Result<T, WeightedSelectionError>> + 'a
    where
        T: 'a,
    {
        let mut random = SplitMix64::new(seed);
        self.random_with(context, move || random.unit())
    }
}
