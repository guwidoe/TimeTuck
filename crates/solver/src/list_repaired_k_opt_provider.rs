//! Optional seeded discovery of repaired even-k combined tours.
use crate::random::SplitMix64;
use crate::{
    ListAssignment, ListError, ListKOptConfig, ListKOptRepairError, ListKOptSubtourRepairSelector,
    ListMove, ListPosition, ListProblem, MoveProvider,
};
use std::cell::RefCell;

/// Bounded live sampling through the existing even-k repair selector. Owners are
/// sampled uniformly among unpinned nonempty movable suffixes, then positions
/// uniformly within their suffix. Initial directions and retained forward
/// repair edges are native seeded draws. This does not promise uniform final tours or Java traces.
/// Destination suffix lengths come from the opening assignment, in selected
/// owner order; this provider does not explore varying partitions. Invalid cuts,
/// impossible repairs, range violations and no-ops consume an attempt and emit
/// nothing. Both standard list scoring backends can consume this provider.
#[derive(Clone, Debug)]
pub struct RepairedKOptProvider {
    selector: ListKOptSubtourRepairSelector,
    minimum: usize,
    weights: Vec<u64>,
    mass: u64,
    seed: u64,
    max_attempts: usize,
}
impl RepairedKOptProvider {
    pub fn new(
        sizes: ListKOptConfig,
        weights: Vec<u64>,
        seed: u64,
        max_attempts: usize,
    ) -> Result<Self, ListKOptRepairError> {
        let selector = ListKOptSubtourRepairSelector::new(sizes, weights.clone())?;
        if weights
            .iter()
            .enumerate()
            .any(|(i, &w)| w > 0 && !(sizes.minimum() + i).is_multiple_of(2))
        {
            return Err(ListKOptRepairError::InvalidConfig);
        }
        let mass = weights.iter().sum(); // Already checked by the selector.
        Ok(Self {
            selector,
            minimum: sizes.minimum(),
            weights,
            mass,
            seed,
            max_attempts,
        })
    }
    pub fn max_attempts(&self) -> usize {
        self.max_attempts
    }
    /// Validate even when disabled. Borrowing the immutable opening assignment
    /// prevents concurrent edits; reopening derives fresh owners and endpoints.
    /// Emitted native moves are positional and should be consumed before edits,
    /// rather than treated as snapshot-bound replay receipts.
    pub fn open<'a, V: Eq + Clone>(
        &'a self,
        problem: &'a ListProblem<V>,
        assignment: &'a ListAssignment<V>,
        step: u64,
    ) -> Result<RepairedKOptMoves<'a, V>, ListError> {
        problem.validate(assignment)?;
        let owners = (0..problem.entities().len())
            .filter(|&e| {
                !problem.entities()[e].is_pinned()
                    && problem.entities()[e].pinned_prefix() < assignment.lists()[e].len()
            })
            .collect();
        Ok(RepairedKOptMoves {
            provider: self,
            problem,
            assignment,
            owners,
            rng: SplitMix64::new(
                self.seed
                    .wrapping_add(step.wrapping_mul(0x9e3779b97f4a7c15)),
            ),
            attempts: 0,
        })
    }
}
/// Lazy finite scan; rejected attempts are included in the observable count.
pub struct RepairedKOptMoves<'a, V> {
    provider: &'a RepairedKOptProvider,
    problem: &'a ListProblem<V>,
    assignment: &'a ListAssignment<V>,
    owners: Vec<usize>,
    rng: SplitMix64,
    attempts: usize,
}
impl<V: Eq + Clone> RepairedKOptMoves<'_, V> {
    pub fn attempts(&self) -> usize {
        self.attempts
    }
    fn attempt(&mut self) -> Option<ListMove<V>> {
        let draw = self.rng.below(self.provider.mass);
        let mut cumulative = 0;
        let k = self
            .provider
            .weights
            .iter()
            .enumerate()
            .find_map(|(i, &w)| {
                cumulative += w;
                (draw < cumulative).then_some(self.provider.minimum + i)
            })?;
        let available: usize = self
            .owners
            .iter()
            .map(|&e| self.assignment.lists()[e].len() - self.problem.entities()[e].pinned_prefix())
            .sum();
        if k > available {
            return None;
        }
        let mut owners = Vec::new();
        let mut values = Vec::with_capacity(k);
        let mut directions = Vec::with_capacity(k);
        for _ in 0..k {
            let e = self.owners[self.rng.below(self.owners.len() as u64) as usize];
            let prefix = self.problem.entities()[e].pinned_prefix();
            let i = prefix
                + self
                    .rng
                    .below((self.assignment.lists()[e].len() - prefix) as u64)
                    as usize;
            if !owners.contains(&e) {
                owners.push(e);
            }
            values.push(self.assignment.lists()[e][i].clone());
            directions.push(self.rng.below(2) == 0);
        }
        let positions: Vec<_> = owners
            .iter()
            .flat_map(|&entity| {
                (self.problem.entities()[entity].pinned_prefix()
                    ..self.assignment.lists()[entity].len())
                    .map(move |index| ListPosition { entity, index })
            })
            .collect();
        let lengths: Vec<_> = owners
            .iter()
            .map(|&e| self.assignment.lists()[e].len() - self.problem.entities()[e].pinned_prefix())
            .collect();
        let mut values = values.into_iter();
        let mut origin = values.next();
        let mut directions = directions.into_iter();
        let rng = RefCell::new(&mut self.rng);
        let proposal = self
            .provider
            .selector
            .select(
                self.problem,
                self.assignment,
                draw,
                || origin.take(),
                || {
                    values.next().or_else(|| {
                        let p = positions[rng.borrow_mut().below(positions.len() as u64) as usize];
                        Some(self.assignment.lists()[p.entity][p.index].clone())
                    })
                },
                || directions.next().unwrap_or(true),
                |components, removed| {
                    // Enumerate actual retained forward edges of the selected
                    // combined suffix tour, never independent per-row wrap edges.
                    let edges: Vec<_> = positions
                        .iter()
                        .copied()
                        .enumerate()
                        .filter_map(|(i, a)| {
                            let b = positions[(i + 1) % positions.len()];
                            let same = components.iter().any(|c| c.contains(&a) && c.contains(&b));
                            let retained = !removed
                                .iter()
                                .any(|&(x, y)| (x == a && y == b) || (x == b && y == a));
                            (same && retained).then_some(a)
                        })
                        .collect();
                    if edges.is_empty() {
                        None
                    } else {
                        Some((
                            edges[rng.borrow_mut().below(edges.len() as u64) as usize],
                            true,
                        ))
                    }
                },
            )
            .ok()?;
        let first = positions[rng.borrow_mut().below(positions.len() as u64) as usize];
        let reverse = rng.borrow_mut().below(2) == 0;
        let movement = proposal
            .physical_plan()
            .movement(self.problem, self.assignment, first, reverse, &lengths)
            .ok()?;
        self.problem
            .move_checker(self.assignment)
            .ok()?
            .is_doable(&movement)
            .ok()?
            .then_some(movement)
    }
}
impl<V: Eq + Clone> Iterator for RepairedKOptMoves<'_, V> {
    type Item = ListMove<V>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.owners.is_empty() {
            return None;
        }
        while self.attempts < self.provider.max_attempts {
            self.attempts += 1;
            if let Some(movement) = self.attempt() {
                return Some(movement);
            }
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            0,
            Some(if self.owners.is_empty() {
                0
            } else {
                self.provider.max_attempts - self.attempts
            }),
        )
    }
}
impl<V: Eq + Clone> std::iter::FusedIterator for RepairedKOptMoves<'_, V> {}
impl<V: Eq + Clone> MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>
    for RepairedKOptProvider
{
    fn moves<'a>(
        &'a mut self,
        problem: &'a ListProblem<V>,
        assignment: &'a ListAssignment<V>,
        step: u64,
    ) -> Box<dyn Iterator<Item = ListMove<V>> + 'a>
    where
        ListMove<V>: 'a,
    {
        Box::new(
            self.open(problem, assignment, step)
                .expect("provider requires a valid list assignment"),
        )
    }
}
