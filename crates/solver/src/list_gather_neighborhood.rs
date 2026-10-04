//! Lazy bounded permutations for list gathering.
use crate::{ListAssignment, ListError, ListMove, ListPosition, ListProblem, SubPillarSelection};

// Increasing length, then lexicographic tuples without repeated indices.
struct Selections {
    n: usize,
    max: usize,
    next: Option<Vec<usize>>,
}
impl Selections {
    fn new(n: usize, max: usize) -> Self {
        Self {
            n,
            max: max.min(n),
            next: (n > 0 && max > 0).then(|| vec![0]),
        }
    }
}
impl Iterator for Selections {
    type Item = Vec<usize>;
    fn next(&mut self) -> Option<Self::Item> {
        let result = self.next.take()?;
        let mut following = result.clone();
        for i in (0..following.len()).rev() {
            if let Some(value) = (following[i] + 1..self.n).find(|v| !following[..i].contains(v)) {
                following[i] = value;
                for j in i + 1..following.len() {
                    following[j] = (0..self.n)
                        .find(|v| !following[..j].contains(v))
                        .expect("unused index");
                }
                self.next = Some(following);
                return Some(result);
            }
        }
        if result.len() < self.max {
            self.next = Some((0..result.len() + 1).collect());
        }
        Some(result)
    }
}
impl std::iter::FusedIterator for Selections {}

fn movable<V: Eq + Clone>(p: &ListProblem<V>, a: &ListAssignment<V>, value: &V) -> bool {
    a.location(value).is_none_or(|position| {
        let entity = &p.entities()[position.entity];
        !entity.is_pinned() && position.index >= entity.pinned_prefix()
    })
}
fn unchanged<V: Eq>(a: &ListAssignment<V>, members: &[&V], destination: ListPosition) -> bool {
    let Some(first) = a.location(members[0]) else {
        return false;
    };
    first.entity == destination.entity
        && (first.index..=first.index + members.len()).contains(&destination.index)
        && a.lists()[first.entity]
            .get(first.index..first.index + members.len())
            .is_some_and(|span| span.iter().zip(members).all(|(a, b)| a == *b))
}
/// Gather groups of one through max_size movable identities. Zero yields no moves;
/// oversized bounds are clipped to available values. Destinations come first in
/// entity/index order, then group size and registry-index permutation order.
/// Equivalent adjacent original boundaries and unchanged states are skipped.
/// Different samples may still produce the same successor state.
///
/// Optional unassignments come last, once per assigned identity subset in increasing
/// size/registry order. Required inputs stay complete. Missing values may be inserted
/// into partial inputs. Pins and ranges restrict candidates before move construction.
/// Permutations are generated lazily; no factorial collection is allocated. Each
/// yielded composite is validated and bound to the supplied immutable assignment.
///
/// ```
/// use timetuck::*;
/// let p = ListProblem::new(vec![0,1], vec![ListEntity::new(vec![0,1])])?;
/// let a = p.initial_assignment();
/// assert!(list_gather_moves(&p, &a, 2)?.next().is_some());
/// # Ok::<(), ListError>(())
/// ```
pub fn list_gather_moves<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    max_size: usize,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    p.validate(a)?;
    Ok(ordered(p, a, max_size, 0..p.entities().len()))
}
pub(crate) fn ordered<'a, V: Eq + Clone, I: Iterator<Item = usize> + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    max_size: usize,
    order: I,
) -> Box<dyn Iterator<Item = ListMove<V>> + 'a> {
    if max_size == 0 {
        return Box::new(std::iter::empty());
    }
    let insertions =
        crate::list_neighborhood::insertions(p, a, order).flat_map(move |destination| {
            // A sample containing the preceding value has the same result at
            // an earlier boundary. Exclude that value before enumerating; filtering
            // afterward would scan entire factorial buckets that cannot yield.
            let preceding = destination
                .index
                .checked_sub(1)
                .map(|i| &a.lists()[destination.entity][i]);
            let eligible: Vec<_> = p
                .values()
                .iter()
                .filter(|v| {
                    preceding != Some(*v) && movable(p, a, v) && p.accepts(destination.entity, v)
                })
                .collect();
            Selections::new(eligible.len(), max_size).filter_map(move |indices| {
                let members: Vec<_> = indices.iter().map(|&i| eligible[i]).collect();
                if unchanged(a, &members, destination) {
                    return None;
                }
                let members: Vec<_> = members.into_iter().cloned().collect();
                Some(
                    p.gather_move(a, &members, Some(destination))
                        .expect("prevalidated gather"),
                )
            })
        });
    // Delay optional subset setup until insertion candidates are exhausted.
    let removed = p
        .allows_unassigned()
        .then_some(())
        .into_iter()
        .flat_map(move |_| {
            let eligible: Vec<_> = p
                .values()
                .iter()
                .filter(|v| a.location(v).is_some() && movable(p, a, v))
                .collect();
            SubPillarSelection::combinations(1, max_size)
                .expect("positive bound")
                .indices(eligible.len())
                .map(move |indices| {
                    let members: Vec<_> = indices.iter().map(|&i| eligible[i].clone()).collect();
                    p.gather_move(a, &members, None)
                        .expect("prevalidated optional unassignment")
                })
        });
    Box::new(insertions.chain(removed))
}

#[cfg(test)]
mod tests {
    use super::Selections;
    #[test]
    fn permutations_are_unique_length_then_lexicographic_and_fused() {
        for n in 0..=6 {
            for cap in [0, 1, 2, 3, usize::MAX] {
                let mut iter = Selections::new(n, cap);
                let rows: Vec<_> = iter.by_ref().collect();
                assert!(iter.next().is_none());
                assert!(iter.next().is_none());
                let mut product = 1;
                let expected: usize = (1..=n.min(cap))
                    .map(|k| {
                        product *= n - k + 1;
                        product
                    })
                    .sum();
                assert_eq!(rows.len(), expected);
                for row in &rows {
                    assert!(row.iter().all(|i| *i < n));
                    assert_eq!(
                        row.iter().collect::<std::collections::BTreeSet<_>>().len(),
                        row.len()
                    );
                }
                for pair in rows.windows(2) {
                    assert!((pair[0].len(), &pair[0]) < (pair[1].len(), &pair[1]));
                }
            }
        }
    }
}
