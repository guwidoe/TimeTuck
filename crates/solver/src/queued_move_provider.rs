//! Assignment-aware queues of construction placements.
use crate::MoveProvider;

/// How placement keys are revisited after their candidate pools are processed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlacementQueueOrder {
    /// Visit each key once, in supplied order.
    #[default]
    Once,
    /// Wrap to the first key. One entirely empty rotation ends the current request.
    Cycle,
}

/// Generate one placement's candidates from the current immutable assignment.
/// Iterators may borrow the problem and assignment, but not this factory or the
/// key. Copy an identifier or clone an Arc when a lazy cursor needs the key.
/// This lets a queue skip empty pools without retaining mutable factory borrows.
/// Callbacks and iterator polling are cooperative, just like MoveProvider.
/// Closures returning Vec<M> implement this trait for convenience.
pub trait PlacementMoveFactory<P, A, K, M> {
    fn candidates<'a>(
        &mut self,
        problem: &'a P,
        assignment: &'a A,
        key: &K,
    ) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a;
}
impl<P, A, K, M, F: FnMut(&P, &A, &K) -> Vec<M>> PlacementMoveFactory<P, A, K, M> for F {
    fn candidates<'a>(&mut self, p: &'a P, a: &'a A, key: &K) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        Box::new(self(p, a, key).into_iter())
    }
}

/// A custom construction provider with one candidate pool per queued key.
/// Keys can identify values, entities, entity/variable pairs or entire groups.
/// Their order and duplicate occurrences are preserved; keys need not be Clone.
/// The factory decides eligibility using the assignment supplied for each call.
/// Empty pools are skipped, with at most keys.len() factory calls per request.
/// A nonempty pool is prefetched by exactly one item, then returned lazily.
/// In construction, rejected optional placements advance to the next key. A full
/// cyclic rotation of rejected/empty placements ends the phase; an installation
/// resets that rotation. Direct users of moves() can deliver the same result via
/// construction_placement_completed() if they need this rejection stopping rule.
///
/// An increased completed-step counter advances past the preceding nonempty
/// placement. Repeating the counter retries the same key with fresh candidates;
/// decreasing it rewinds the queue. Call reset explicitly for a new phase/solve
/// whose counter equals the previous one, or to revisit a once-exhausted queue.
/// No selector lifecycle events are fabricated or inferred; deliver any required
/// events to the factory explicitly through factory_mut().
///
/// Use with construction_from_with_provider or bind to a construction phase's
/// move provider. All ordinary scoring, atomic interruption and pin validation
/// stay in the construction engine. Invalid/non-initializing proposals are still
/// handled by that engine, not skipped by this queue. A cyclic queue should filter
/// already initialized receivers in its factory.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![1,2]); 2])?;
/// let n = ConstraintNetwork::<_, SimpleScore>::new(2);
/// let mut provider = QueuedMoveProvider::new(vec![1usize,0], PlacementQueueOrder::Once,
///     |p: &FiniteProblem<i32>, a: &Assignment<i32>, &variable: &usize| {
///         if a.values()[variable].is_some() { return vec![]; }
///         p.variables()[variable].domain().iter().map(|v| Move::Change {
///             variable, value: Some(*v) }).collect()
///     });
/// let r = construction_from_with_provider(&p, &n, p.initial_assignment(),
///     ProviderConstructionConfig { max_steps: Some(1), ..Default::default() }, &mut provider)?;
/// assert_eq!(r.assignment.values(), &[None, Some(1)]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct QueuedMoveProvider<K, F> {
    keys: Vec<K>,
    order: PlacementQueueOrder,
    factory: F,
    cursor: usize,
    last_step: Option<u64>,
    had_placement: bool,
    unproductive: Option<usize>,
}
impl<K, F> QueuedMoveProvider<K, F> {
    pub fn new(keys: Vec<K>, order: PlacementQueueOrder, factory: F) -> Self {
        Self {
            keys,
            order,
            factory,
            cursor: 0,
            last_step: None,
            had_placement: false,
            unproductive: None,
        }
    }
    pub fn reset(&mut self) {
        self.cursor = 0;
        self.last_step = None;
        self.had_placement = false;
        self.unproductive = None;
    }
    pub fn factory(&self) -> &F {
        &self.factory
    }
    pub fn factory_mut(&mut self) -> &mut F {
        &mut self.factory
    }
    /// The next/current key, or None after a finite queue is exhausted.
    pub fn current_key(&self) -> Option<&K> {
        self.keys.get(self.cursor)
    }
    fn advance(&mut self) {
        self.cursor = self.cursor.saturating_add(1);
        if self.order == PlacementQueueOrder::Cycle && self.cursor >= self.keys.len() {
            self.cursor = 0;
        }
    }
}
impl<P, A, M, K, F: PlacementMoveFactory<P, A, K, M>> MoveProvider<P, A, M>
    for QueuedMoveProvider<K, F>
{
    fn construction_placement_completed(&mut self, installed: bool) -> bool {
        if installed {
            self.unproductive = None;
            return true;
        }
        let missed = self.unproductive.unwrap_or(0).saturating_add(1);
        self.unproductive = Some(missed);
        match self.order {
            PlacementQueueOrder::Once => self.cursor.saturating_add(1) < self.keys.len(),
            PlacementQueueOrder::Cycle => missed < self.keys.len(),
        }
    }
    fn moves<'a>(
        &'a mut self,
        p: &'a P,
        a: &'a A,
        completed_steps: u64,
    ) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        if let Some(previous) = self.last_step {
            if completed_steps < previous {
                self.reset();
            } else if completed_steps > previous && self.had_placement {
                self.advance();
            }
        }
        self.last_step = Some(completed_steps);
        self.had_placement = false;
        for _ in 0..self.keys.len() {
            if self.order == PlacementQueueOrder::Cycle
                && self.unproductive.is_some_and(|n| n >= self.keys.len())
            {
                break;
            }
            let Some(key) = self.keys.get(self.cursor) else {
                break;
            };
            let mut candidates = self.factory.candidates(p, a, key);
            if let Some(first) = candidates.next() {
                self.had_placement = true;
                return Box::new(std::iter::once(first).chain(candidates));
            }
            if let Some(missed) = &mut self.unproductive {
                *missed = missed.saturating_add(1);
            }
            self.advance();
        }
        Box::new(std::iter::empty())
    }
}
