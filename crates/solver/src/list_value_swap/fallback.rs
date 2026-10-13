//! Repeating attempts with bounded rejection and a shared no-change fallback.
use super::ValueSwapSelection;
use crate::{ListAssignment, ListError, ListMove, ListProblem, SampleRandom};
use std::{iter::FusedIterator, num::NonZeroUsize};

#[derive(Debug)]
pub struct ListSwapNoChange;
pub static LIST_SWAP_NO_CHANGE: ListSwapNoChange = ListSwapNoChange;

/// A real candidate or the canonical no-change marker after bounded rejection.
/// A fallback is not evidence that no improving swap exists.
#[derive(Clone, Debug)]
pub enum ListSwapAttempt<V> {
    Candidate(ListMove<V>),
    NoChange(&'static ListSwapNoChange),
}
impl<V> ListSwapAttempt<V> {
    pub fn into_movement(self) -> ListMove<V> {
        match self {
            Self::Candidate(movement) => movement,
            Self::NoChange(_) => ListMove::Composite(Vec::new()),
        }
    }
}
/// Each output draws one left occurrence and at most `partner_attempts` right
/// occurrences. Receiving ranges, pins, unassignment and dependencies use existing
/// native swap validation. Failed probes yield the same no-change marker, leaving
/// later outputs available. Empty selectable left pools exhaust immediately;
/// empty right pools yield fallbacks without a right draw. Duplicate occurrences
/// retain sampling weight. Reopen after committing assignment changes.
///
/// Opening validates identities and builds one existing assignment index, without
/// constructing a Cartesian product. Each attempt reuses it and the native checker.
/// Source callbacks/random Java traces are not reproduced. Ordinary swap APIs stay
/// unchanged. Bound consumption with take(limit) or a provider's candidate limit.
pub struct RepeatingListSwapAttempts<'a, V> {
    selection: ValueSwapSelection<'a, V>,
    left: Vec<usize>,
    right: Vec<usize>,
    partner_attempts: NonZeroUsize,
    include_noops: bool,
    random: SampleRandom,
}
impl<V> RepeatingListSwapAttempts<'_, V> {
    /// Passive availability; a nonempty left pool can always produce a fallback.
    pub fn has_next(&self) -> bool {
        !self.left.is_empty()
    }
}
pub fn repeating_list_swap_attempts<'a, V: Eq + Clone>(
    problem: &'a ListProblem<V>,
    before: &'a ListAssignment<V>,
    left: &[V],
    right: &[V],
    partner_attempts: NonZeroUsize,
    include_noops: bool,
    seed: u64,
) -> Result<RepeatingListSwapAttempts<'a, V>, ListError> {
    let selection = ValueSwapSelection::new(problem, before)?;
    let left = selection.order(left)?;
    let right = selection.order(right)?;
    Ok(RepeatingListSwapAttempts {
        selection,
        left,
        right,
        partner_attempts,
        include_noops,
        random: SampleRandom::new(seed),
    })
}
impl<V: Eq + Clone> Iterator for RepeatingListSwapAttempts<'_, V> {
    type Item = ListSwapAttempt<V>;
    fn next(&mut self) -> Option<Self::Item> {
        let left_count = NonZeroUsize::new(self.left.len())?;
        let left = self.left[self.random.index(left_count)];
        if let Some(right_count) = NonZeroUsize::new(self.right.len()) {
            for _ in 0..self.partner_attempts.get() {
                let right = self.right[self.random.index(right_count)];
                if let Some(movement) = self.selection.movement(left, right, self.include_noops) {
                    return Some(ListSwapAttempt::Candidate(movement));
                }
            }
        }
        Some(ListSwapAttempt::NoChange(&LIST_SWAP_NO_CHANGE))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.has_next() {
            (usize::MAX, None)
        } else {
            (0, Some(0))
        }
    }
}
impl<V: Eq + Clone> FusedIterator for RepeatingListSwapAttempts<'_, V> {}
impl<V: Eq + Clone> crate::ReadOnlyRandomCursor for RepeatingListSwapAttempts<'_, V> {}
impl<V: Eq + Clone> crate::RepeatingRandomCursor for RepeatingListSwapAttempts<'_, V> {}
