//! Cyclic edge reconnection within one list's movable suffix.
use crate::{ListAssignment, ListError, ListMove, ListPosition, ListProblem, SubPillarSelection};
use std::{error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListKOptError {
    List(ListError),
    TooFewValues,
    EdgeCount,
    InvalidEdge,
    DuplicateEdge,
    Degree,
    Disconnected,
    FirstSuccessor,
    Entities,
    Partition,
}
impl From<ListError> for ListKOptError {
    fn from(value: ListError) -> Self {
        Self::List(value)
    }
}
impl fmt::Display for ListKOptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl Error for ListKOptError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::List(error) => Some(error),
            _ => None,
        }
    }
}

/// Exact edge difference bounds, with 2 <= minimum <= maximum. Oversized bounds
/// are clipped to the movable suffix length; there is no fixed upper k limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListKOptConfig {
    minimum: usize,
    maximum: usize,
}
impl ListKOptConfig {
    pub fn new(minimum: usize, maximum: usize) -> Result<Self, ListError> {
        if minimum < 2 || minimum > maximum {
            return Err(ListError::InvalidSampleSize { minimum, maximum });
        }
        Ok(Self { minimum, maximum })
    }
    pub fn minimum(self) -> usize {
        self.minimum
    }
    pub fn maximum(self) -> usize {
        self.maximum
    }
}
pub(crate) fn edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

// Reorder with ordinary relocations, preserving assigned payloads and transaction
// rollback. All positions and target indices refer to the immutable input list.
pub(crate) fn movement<V: Eq + Clone>(
    a: &ListAssignment<V>,
    entity: usize,
    target: &[usize],
) -> ListMove<V> {
    let mut current: Vec<_> = (0..a.lists()[entity].len()).collect();
    let mut moves = Vec::new();
    for (index, &wanted) in target.iter().enumerate() {
        let source = current
            .iter()
            .position(|&i| i == wanted)
            .expect("permutation");
        if source != index {
            moves.push(ListMove::Change {
                source: ListPosition {
                    entity,
                    index: source,
                },
                destination: ListPosition { entity, index },
            });
            current.remove(source);
            current.insert(index, wanted);
        }
    }
    ListMove::Composite(moves)
}
impl<V: Eq + Clone> ListProblem<V> {
    /// Reconnect a cyclic movable suffix by removing and adding equal numbers of
    /// undirected edges. Endpoints are absolute indices in `before`, including the
    /// wrap edge from the last value to the first unpinned value. At least three
    /// movable values are required. Duplicate edges, self edges, malformed degrees
    /// and disconnected subtours are rejected without changing the input.
    ///
    /// The first unpinned identity remains the anchor. `first_successor` chooses
    /// its next neighbor explicitly, since a list score may distinguish the two
    /// directions of an undirected cycle. Pins and all assigned payloads remain
    /// unchanged. Returns an ordinary snapshot-bound composite; regenerate it after
    /// mutation. This API handles one entity, not multi-entity tour repartitioning.
    ///
    /// ```
    /// use timetuck::*;
    /// let p = ListProblem::new((0..6).collect(), vec![ListEntity::new((0..6).collect())])?;
    /// let mut a = p.initial_assignment();
    /// let m = p.k_opt_move(&a, 0, &[(5,0),(1,2),(3,4)], &[(0,2),(1,4),(3,5)], 1)?;
    /// { let trial = p.apply(&mut a, &m)?;
    ///   assert_eq!(trial.lists()[0], [0,1,4,5,3,2]); }
    /// assert_eq!(a, p.initial_assignment());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn k_opt_move(
        &self,
        before: &ListAssignment<V>,
        entity: usize,
        removed: &[(usize, usize)],
        added: &[(usize, usize)],
        first_successor: usize,
    ) -> Result<ListMove<V>, ListKOptError> {
        self.validate(before)?;
        let row = before.list(entity).ok_or(ListError::EntityIndex {
            entity,
            count: self.entities().len(),
        })?;
        let meta = &self.entities()[entity];
        if meta.is_pinned() {
            return Err(ListError::PinChanged { entity }.into());
        }
        let start = meta.pinned_prefix();
        let tour = cycle_order(start, row.len(), removed, added, start, first_successor)?;
        let target: Vec<_> = (0..start).chain(tour).collect();
        Ok(movement(before, entity, &target))
    }
}

/// Build a single-list reconnection with orientation derived from the supplied
/// edge order. Traverse from the first removed edge's first endpoint through its
/// first incident added edge, then rotate the resulting cycle to the first
/// unpinned value. Thus callers need not supply a successor from an expected tour.
/// Reordering edge pairs can deliberately choose the opposite traversal direction.
/// All other validity, pin and snapshot-binding rules match k_opt_move.
pub fn list_k_opt_edge_move<V: Eq + Clone>(
    problem: &ListProblem<V>,
    before: &ListAssignment<V>,
    entity: usize,
    removed: &[(usize, usize)],
    added: &[(usize, usize)],
) -> Result<ListMove<V>, ListKOptError> {
    problem.validate(before)?;
    let row = before.list(entity).ok_or(ListError::EntityIndex {
        entity,
        count: problem.entities().len(),
    })?;
    let meta = &problem.entities()[entity];
    if meta.is_pinned() {
        return Err(ListError::PinChanged { entity }.into());
    }
    let first = removed.first().ok_or(ListKOptError::EdgeCount)?.0;
    let successor = added
        .iter()
        .find_map(|&(a, b)| {
            if a == first {
                Some(b)
            } else if b == first {
                Some(a)
            } else {
                None
            }
        })
        .ok_or(ListKOptError::FirstSuccessor)?;
    let start = meta.pinned_prefix();
    let mut tour = cycle_order(start, row.len(), removed, added, first, successor)?;
    let anchor = tour
        .iter()
        .position(|&i| i == start)
        .expect("complete validated cycle");
    tour.rotate_left(anchor);
    let target: Vec<_> = (0..start).chain(tour).collect();
    Ok(movement(before, entity, &target))
}

pub(crate) fn cycle_order(
    start: usize,
    end: usize,
    removed: &[(usize, usize)],
    added: &[(usize, usize)],
    first: usize,
    first_successor: usize,
) -> Result<Vec<usize>, ListKOptError> {
    crate::ListKOptTopology::new(start..end, removed, added)?.order_from(first, first_successor)
}

// Boolean counter without machine-word limits. Singleton paths have one orientation.
struct Orientations {
    lengths: Vec<usize>,
    next: Option<Vec<bool>>,
}
impl Orientations {
    fn new(lengths: Vec<usize>) -> Self {
        Self {
            next: Some(vec![false; lengths.len()]),
            lengths,
        }
    }
}
impl Iterator for Orientations {
    type Item = Vec<bool>;
    fn next(&mut self) -> Option<Self::Item> {
        let result = self.next.take()?;
        let mut next = result.clone();
        for i in (1..next.len()).rev() {
            if self.lengths[i] > 1 && !next[i] {
                next[i] = true;
                self.next = Some(next);
                return Some(result);
            }
            next[i] = false;
        }
        Some(result)
    }
}
pub(crate) fn cut_paths(n: usize, cuts: &[usize]) -> Vec<Vec<usize>> {
    let k = cuts.len();
    let mut paths = Vec::with_capacity(k);
    for i in 0..k {
        let previous = if i == 0 { cuts[k - 1] } else { cuts[i - 1] };
        let mut path = Vec::new();
        let mut at = (previous + 1) % n;
        loop {
            path.push(at);
            if at == cuts[i] {
                break;
            }
            at = (at + 1) % n;
        }
        paths.push(path);
    }
    paths
}
fn reconnections(n: usize, cuts: Vec<usize>) -> impl Iterator<Item = Vec<usize>> {
    let k = cuts.len();
    let paths = cut_paths(n, &cuts);
    let removed: Vec<_> = cuts.iter().map(|&i| edge(i, (i + 1) % n)).collect();
    crate::list_gather_neighborhood::Selections::bounded(k - 1, k - 1, k - 1).flat_map(
        move |permutation| {
            let paths = paths.clone();
            let removed = removed.clone();
            Orientations::new(paths.iter().map(Vec::len).collect())
                .filter_map(move |flips| {
                    let mut tour = paths[0].clone();
                    for &index in &permutation {
                        let path = &paths[index + 1];
                        if flips[index + 1] {
                            tour.extend(path.iter().rev().copied());
                        } else {
                            tour.extend(path.iter().copied());
                        }
                    }
                    // An unchanged cut is a smaller exchange; only exact edge differences
                    // belong to this cut set. Prevent duplicate undirected cycles when the
                    // anchored path has no direction (a singleton).
                    if (paths[0].len() == 1 && tour[1] > tour[n - 1])
                        || (0..n).any(|i| removed.contains(&edge(tour[i], tour[(i + 1) % n])))
                    {
                        return None;
                    }
                    let anchor = tour.iter().position(|&v| v == 0).expect("anchor");
                    tour.rotate_left(anchor);
                    Some(tour)
                })
                .flat_map(|tour| {
                    let mut reversed = tour.clone();
                    reversed[1..].reverse();
                    [tour, reversed]
                })
        },
    )
}
/// Enumerate cyclic reconnections within each movable suffix. The first movable
/// identity stays fixed; both directions are emitted because list scores may be
/// directed. Exact changed-edge counts follow increasing k, then cut combinations,
/// path permutations and orientations. No unchanged edge sets or duplicate tours.
/// Pins, missing values and other lists are untouched. Three or fewer movable
/// values have no changed undirected cycle. This is a single-entity neighborhood;
/// combine with TwoOpt for cross-entity tail exchanges. Enumeration is lazy, with
/// linear storage per active candidate, but its exhaustive search space is factorial.
pub fn list_k_opt_moves<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    config: ListKOptConfig,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    p.validate(a)?;
    Ok(ordered(p, a, config, 0..p.entities().len()))
}
pub(crate) fn ordered<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    config: ListKOptConfig,
    order: impl Iterator<Item = usize> + 'a,
) -> impl Iterator<Item = ListMove<V>> + 'a {
    order
        .filter(move |&e| !p.entities()[e].is_pinned())
        .flat_map(move |e| {
            let start = p.entities()[e].pinned_prefix();
            let n = a.lists()[e].len() - start;
            SubPillarSelection::combinations(config.minimum, config.maximum)
                .expect("validated k bounds")
                .indices(if n < 4 { 0 } else { n })
                .flat_map(move |cuts| reconnections(n, cuts))
                .map(move |tour| {
                    let target: Vec<_> = (0..start)
                        .chain(tour.into_iter().map(|i| i + start))
                        .collect();
                    movement(a, e, &target)
                })
        })
}
