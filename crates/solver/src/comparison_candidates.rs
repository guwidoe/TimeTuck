//! Relation-oriented queries over borrowed candidate indexes.
use crate::{CandidateIndex, IndexedCandidate, JoinComparison, LazyConcat};
use std::ops::Bound::{Excluded, Included, Unbounded};

/// Which side of `left_key comparison right_key` the stored candidates occupy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateSide {
    Left,
    Right,
}

/// Query candidate occurrences by a join relation. When indexing the right side,
/// the query key is the left operand and the relation is reversed automatically.
/// This uses ordered range bounds, never subtraction, negated comparator results,
/// or increment/decrement of boundary keys. Equal-key occurrences stay distinct.
/// Groups are borrowed in ascending key order; sampling is over all occurrences.
/// Keys use native Ord, including its explicit Option ordering when used.
///
/// ```
/// use timetuck::*;
/// let mut ages=CandidateIndex::new();
/// ages.insert(0,20,"Ann")?;
/// ages.insert(1,30,"Beth")?;
/// let right=comparison_candidates(&ages,20,JoinComparison::LessThan,CandidateSide::Right);
/// assert_eq!(right.len(),1);
/// assert_eq!(right.get(0).unwrap().value,"Beth");
/// # Ok::<(), CandidateIndexError<usize>>(())
/// ```
pub fn comparison_candidates<K: Ord + Clone, I: Ord + Clone, T>(
    index: &CandidateIndex<K, I, T>,
    query: K,
    comparison: JoinComparison,
    side: CandidateSide,
) -> LazyConcat<'_, IndexedCandidate<I, T>> {
    let comparison = match side {
        CandidateSide::Left => comparison,
        CandidateSide::Right => comparison.reversed(),
    };
    let bounds = match comparison {
        JoinComparison::Equal => (Included(query.clone()), Included(query)),
        JoinComparison::LessThan => (Unbounded, Excluded(query)),
        JoinComparison::LessThanOrEqual => (Unbounded, Included(query)),
        JoinComparison::GreaterThan => (Excluded(query), Unbounded),
        JoinComparison::GreaterThanOrEqual => (Included(query), Unbounded),
    };
    index.range(bounds)
}
