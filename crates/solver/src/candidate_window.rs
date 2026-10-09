//! Bounded move-ordering adapter over the independent random generator.
use crate::random::SplitMix64;

/// Bounded candidate lookahead, with no eager work at construction. Consuming
/// the entire iterator preserves exactly the original multiset of candidates.
pub(crate) fn order_moves<'a, I>(
    input: I,
    order: crate::MoveOrder,
    random: Option<&'a mut SplitMix64>,
    remaining: u64,
) -> impl Iterator<Item = I::Item> + 'a
where
    I: Iterator + 'a,
    I::Item: 'a,
{
    let window = match order {
        crate::MoveOrder::ShuffledMoves { window, .. } => window
            .get()
            .min(usize::try_from(remaining).unwrap_or(usize::MAX).max(1)),
        _ => 1,
    };
    WindowShuffle {
        input: input.fuse(),
        random,
        window,
        buffer: Vec::new(),
        exhausted: false,
    }
}
struct WindowShuffle<'a, I: Iterator> {
    input: std::iter::Fuse<I>,
    random: Option<&'a mut SplitMix64>,
    window: usize,
    buffer: Vec<I::Item>,
    exhausted: bool,
}
impl<I: Iterator> Iterator for WindowShuffle<'_, I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        if self.window == 1 {
            return self.input.next();
        }
        while !self.exhausted && self.buffer.len() < self.window {
            if let Some(item) = self.input.next() {
                self.buffer.push(item);
            } else {
                self.exhausted = true;
            }
        }
        if self.buffer.is_empty() {
            return None;
        }
        let index = self
            .random
            .as_deref_mut()
            .expect("seeded move ordering")
            .below(self.buffer.len() as u64) as usize;
        Some(self.buffer.swap_remove(index))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, num::NonZeroUsize};
    fn order(seed: u64, window: usize) -> crate::MoveOrder {
        crate::MoveOrder::ShuffledMoves {
            seed,
            window: NonZeroUsize::new(window).unwrap(),
        }
    }
    #[test]
    fn windows_preserve_every_candidate_and_repeat_exactly_for_the_seed() {
        let mut firsts = std::collections::HashSet::new();
        for seed in 0..64 {
            for window in [1, 2, 7, 64, usize::MAX] {
                let run = || {
                    order_moves(
                        0..31,
                        order(seed, window),
                        Some(&mut SplitMix64::new(seed)),
                        100,
                    )
                    .collect::<Vec<_>>()
                };
                let values = run();
                assert_eq!(values, run());
                let mut sorted = values.clone();
                sorted.sort_unstable();
                assert_eq!(sorted, (0..31).collect::<Vec<_>>());
                if window == 1 {
                    assert_eq!(values, sorted);
                }
                if window == 64 {
                    firsts.insert(values[0]);
                }
            }
        }
        assert!(firsts.len() > 16);
    }
    #[test]
    fn shuffle_is_lazy_and_lookahead_is_bounded_by_window_and_budget() {
        for (window, remaining, bound) in [(7, 100, 7), (usize::MAX, 2, 2), (100, 0, 1)] {
            let pulls = Cell::new(0);
            let input = (0..).inspect(|_| pulls.set(pulls.get() + 1));
            let mut rng = SplitMix64::new(4);
            let mut moves = order_moves(input, order(4, window), Some(&mut rng), remaining);
            assert_eq!(pulls.get(), 0);
            assert!(moves.next().is_some());
            assert_eq!(pulls.get(), bound);
            assert!(moves.next().is_some());
            assert_eq!(pulls.get(), bound + 1);
            drop(moves);
            assert_eq!(pulls.get(), bound + 1);
        }
    }
    #[test]
    fn empty_singleton_and_non_fused_inputs_terminate_without_duplicates() {
        let mut rng = SplitMix64::new(5);
        assert_eq!(
            order_moves(std::iter::empty::<i32>(), order(5, 8), Some(&mut rng), 10).next(),
            None
        );
        assert_eq!(
            order_moves([9].into_iter(), order(5, 8), Some(&mut rng), 10).collect::<Vec<_>>(),
            vec![9]
        );
        let mut items = [Some(3), None, Some(8)].into_iter();
        let mut shuffled = order_moves(
            std::iter::from_fn(|| items.next().flatten()),
            order(5, 8),
            Some(&mut rng),
            10,
        );
        assert_eq!(shuffled.next(), Some(3));
        assert_eq!(shuffled.next(), None);
        assert_eq!(shuffled.next(), None);
    }
}
