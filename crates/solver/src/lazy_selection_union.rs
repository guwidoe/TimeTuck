//! Draw-before-poll weighted interleaving for expensive or rarely visited families.
use crate::{WeightedSelection, WeightedSelectionError};
use std::iter::FusedIterator;

/// Opt-in weighted union without per-family lookahead. The first draw chooses
/// among all enabled families, including those whose emptiness is not yet known.
/// Only the chosen child is polled. Exhaustion permanently retires that family
/// and causes another draw against the remaining weights. No exhausted child is
/// reopened, including non-fused children. Zero-weight children are never polled.
///
/// This preserves each child's occurrence order and selects families by weight,
/// not individual candidates. Conditional on a successful draw, surviving family
/// weights determine the distribution. Compared to WeightedInterleave, empty
/// families consume draws when discovered and callback/draw order differs.
/// Existing prefetched union semantics are unchanged.
///
/// Construction validates weights but invokes no child or draw callbacks. Each
/// next() polls at most one live child plus previously undiscovered exhausted
/// children; total exhaustion probes are bounded by the initial family count.
/// Dropping the cursor does no further polling. Successful repeating sources need
/// a consumer limit. Source callbacks remain cooperative.
///
/// Weight setup is O(families), selection is O(log families), and retirement
/// rebuilds the distribution in O(families). No end-to-end sparse-work claim is
/// made. Invalid draws/reweight errors return one error then fuse the cursor.
/// A caught callback panic also fuses it; external effects cannot be rolled back.
pub struct LazyWeightedInterleave<I: Iterator, D> {
    children: Vec<I>,
    weights: Vec<f64>,
    distribution: WeightedSelection<usize>,
    draw: D,
    ended: bool,
}
impl<I: Iterator, D: FnMut() -> f64> LazyWeightedInterleave<I, D> {
    pub fn new(children: Vec<(I, f64)>, draw: D) -> Result<Self, WeightedSelectionError> {
        let (children, weights): (Vec<_>, Vec<_>) = children.into_iter().unzip();
        let distribution = WeightedSelection::new(weights.iter().copied().enumerate().collect())?;
        Ok(Self {
            children,
            weights,
            distribution,
            draw,
            ended: false,
        })
    }
}
impl<I: Iterator, D: FnMut() -> f64> Iterator for LazyWeightedInterleave<I, D> {
    type Item = Result<I::Item, WeightedSelectionError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }
        // Any unwind/error leaves this cursor fused, so callers cannot resume
        // after an incompletely observed child operation.
        self.ended = true;
        while self.distribution.active_len() != 0 {
            let index = match self.distribution.choose((self.draw)()) {
                Ok(Some(index)) => *index,
                Ok(None) => return None,
                Err(error) => return Some(Err(error)),
            };
            if let Some(value) = self.children[index].next() {
                self.ended = false;
                return Some(Ok(value));
            }
            self.weights[index] = 0.0;
            match self.distribution.reweighted(|i, _| self.weights[i]) {
                Ok(distribution) => self.distribution = distribution,
                Err(error) => return Some(Err(error)),
            }
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.ended.then_some(0))
    }
}
impl<I: Iterator, D: FnMut() -> f64> FusedIterator for LazyWeightedInterleave<I, D> {}
