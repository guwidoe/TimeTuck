//! Phase-cached placement queues driven by actual solver lifecycle events.
use crate::{
    MoveProvider, MoveProviderEvent, PlacementError, PlacementOrder, PlacementQueue,
    PlacementSnapshot, PlacementSource, SelectionCacheState, SelectionLifecycleEvent,
    SelectionSource,
};
use std::{error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlacementProviderError {
    OutsideStep,
    Queue(PlacementError),
}
impl From<PlacementError> for PlacementProviderError {
    fn from(e: PlacementError) -> Self {
        Self::Queue(e)
    }
}
impl fmt::Display for PlacementProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutsideStep => f.write_str("placement provider requires an active engine scan"),
            Self::Queue(e) => e.fmt(f),
        }
    }
}
impl Error for PlacementProviderError {}

/// Connect phase-cached keys and per-key candidate sources to construction solving.
/// Each scan captures an owned context and opens the next nonempty placement. A
/// mapper converts its candidate tokens to moves using the current problem and
/// assignment; None rejects a token. Empty and entirely rejected pools are skipped.
/// Source collection retains PlacementQueue's exact-size checks and memory caps.
/// Neither the context, key nor candidate token needs Clone.
///
/// Reopening within an active scan replays the selected snapshot with a fresh mapper
/// traversal. Starting a new scan advances the queue, even if its completed-step
/// argument is unchanged. Every new phase rebuilds keys and starts at the first
/// placement. This is phase-owned scheduling, not a resumable QueuedMoveProvider.
/// Cyclic queues stop after a full pass without installation, counting skipped,
/// rejected and optional no-change placements; an installation resets that count.
///
/// Source events open lazily with the first scan. A shared solver binding keeps a
/// single source solve scope across phases; standalone engines own one private
/// session per phase. End callbacks see the captured pre-scan context. A candidate
/// snapshot keeps tokens alive during iteration and is released at the next scan
/// or phase end. Source errors propagate from try_moves; the infallible provider
/// trait panics on those errors instead of reporting false exhaustion. After a
/// caught panic, reset closes remaining scopes before reuse. Callbacks must be
/// cooperative; their side effects cannot be rolled back.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![1]); 2])?;
/// let n = ConstraintNetwork::<_, SimpleScore>::new(2);
/// let queue = PlacementQueue::new(SinglePlacementKey,
///     vec![PlacementFn(|_: &(), _: &()| vec![0usize, 1])],
///     PlacementOrder::Cycle, 1, 2)?;
/// let mut provider = PlacementQueueProvider::new(queue,
///     |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| (),
///     |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64,
///      _: &(), _: usize, &variable: &usize| {
///         a.values()[variable].is_none().then_some(Move::Change {
///             variable, value: Some(1) })
///     });
/// let result = construction_from_with_provider(&p, &n, p.initial_assignment(),
///     ProviderConstructionConfig::default(), &mut provider)?;
/// assert_eq!(result.assignment.values(), &[Some(1), Some(1)]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct PlacementQueueProvider<C, K, T, KS, S, F, G> {
    queue: PlacementQueue<C, K, T, KS, S>,
    capture: F,
    mapper: G,
    context: Option<C>,
    current: Option<PlacementSnapshot<K, T>>,
    opened: bool,
    visited: u128,
    solve: bool,
    phase: bool,
    step: bool,
}
impl<C, K, T, KS: SelectionSource<C, K>, S: PlacementSource<C, K, T>, F, G>
    PlacementQueueProvider<C, K, T, KS, S, F, G>
{
    /// Take ownership of an idle queue. Panics if a source session is already open.
    pub fn new(queue: PlacementQueue<C, K, T, KS, S>, capture: F, mapper: G) -> Self {
        assert_eq!(
            queue.state(),
            SelectionCacheState::Idle,
            "placement queue must be idle before binding"
        );
        Self {
            queue,
            capture,
            mapper,
            context: None,
            current: None,
            opened: false,
            visited: 0,
            solve: false,
            phase: false,
            step: false,
        }
    }
    pub fn queue(&self) -> &PlacementQueue<C, K, T, KS, S> {
        &self.queue
    }
    pub fn key_source_mut(&mut self) -> &mut KS {
        self.queue.key_source_mut()
    }
    pub fn children_mut(&mut self) -> &mut [S] {
        self.queue.children_mut()
    }
    fn source_event(&mut self, event: SelectionLifecycleEvent) -> Result<(), PlacementError> {
        self.queue.event(
            event,
            self.context.as_ref().expect("opened selection context"),
        )
    }
    /// End a private session, including one interrupted by a caught panic or a
    /// failed cache build. Valid end events update cache state before callbacks;
    /// if a callback panics, calling reset again continues from the remaining state.
    /// Do not reset while an engine is using the provider.
    pub fn reset(&mut self) -> Result<(), PlacementError> {
        loop {
            let event = match self.queue.state() {
                SelectionCacheState::Step => SelectionLifecycleEvent::StepEnded,
                SelectionCacheState::Phase => SelectionLifecycleEvent::PhaseEnded,
                SelectionCacheState::Solving => SelectionLifecycleEvent::SolveEnded,
                SelectionCacheState::Idle => break,
            };
            self.source_event(event)?;
        }
        self.context = None;
        self.current = None;
        self.opened = false;
        self.visited = 0;
        self.solve = false;
        self.phase = false;
        self.step = false;
        Ok(())
    }
    fn lifecycle(&mut self, event: MoveProviderEvent) {
        match event {
            MoveProviderEvent::SolveStarted => {
                assert!(
                    !self.solve && !self.phase && self.queue.state() == SelectionCacheState::Idle,
                    "placement provider needs reset before a new solve"
                );
                self.solve = true;
            }
            MoveProviderEvent::SolveEnded => {
                assert!(
                    self.solve && !self.phase && !self.step,
                    "placement provider solve ended during a phase"
                );
                self.reset().expect("balanced source solve");
            }
            MoveProviderEvent::PhaseStarted => {
                assert!(
                    !self.phase
                        && (self.queue.state() == SelectionCacheState::Idle
                            || (self.solve && self.queue.state() == SelectionCacheState::Solving)),
                    "placement provider needs reset before a new phase"
                );
                self.phase = true;
                self.visited = 0;
            }
            MoveProviderEvent::StepStarted => {
                assert!(
                    self.phase && !self.step,
                    "placement provider scan already active or phase missing"
                );
                self.step = true;
                self.opened = false;
                self.current = None;
            }
            MoveProviderEvent::StepEnded => {
                assert!(self.phase && self.step, "placement provider scan missing");
                self.step = false;
                if self.queue.state() == SelectionCacheState::Step {
                    self.source_event(SelectionLifecycleEvent::StepEnded)
                        .expect("balanced source scan");
                }
            }
            MoveProviderEvent::PhaseEnded => {
                assert!(
                    self.phase && !self.step,
                    "placement provider phase ended during a scan"
                );
                if self.solve {
                    if self.queue.state() == SelectionCacheState::Phase {
                        self.source_event(SelectionLifecycleEvent::PhaseEnded)
                            .expect("balanced source phase");
                    }
                    self.phase = false;
                    self.current = None;
                    self.opened = false;
                } else {
                    self.reset().expect("balanced source session");
                }
            }
        }
    }

    pub fn try_moves<'a, P, A, M: 'a>(
        &'a mut self,
        p: &'a P,
        a: &'a A,
        step: u64,
    ) -> Result<Box<dyn Iterator<Item = M> + 'a>, PlacementProviderError>
    where
        F: FnMut(&P, &A, u64) -> C,
        G: FnMut(&P, &A, u64, &K, usize, &T) -> Option<M>,
    {
        if !self.phase || !self.step {
            return Err(PlacementProviderError::OutsideStep);
        }
        if self.opened {
            let Some(snapshot) = self.current.clone() else {
                return Ok(Box::new(std::iter::empty()));
            };
            let mapper = &mut self.mapper;
            return Ok(Box::new((0..snapshot.len()).filter_map(move |i| {
                mapper(
                    p,
                    a,
                    step,
                    snapshot.key(),
                    snapshot.child_index(),
                    &snapshot.moves()[i],
                )
            })));
        }
        self.context = Some((self.capture)(p, a, step));
        if self.queue.state() == SelectionCacheState::Idle {
            self.source_event(SelectionLifecycleEvent::SolveStarted)?;
        }
        if self.queue.state() == SelectionCacheState::Solving {
            self.source_event(SelectionLifecycleEvent::PhaseStarted)?;
        }
        if self.queue.state() == SelectionCacheState::Phase {
            self.source_event(SelectionLifecycleEvent::StepStarted)?;
        }
        let count = self.queue.placement_count()?;
        while self.visited < count {
            let Some(snapshot) = self.queue.next_placement(self.context.as_ref().unwrap())? else {
                break;
            };
            self.visited += 1;
            let first = snapshot.moves().iter().enumerate().find_map(|(i, token)| {
                (self.mapper)(p, a, step, snapshot.key(), snapshot.child_index(), token)
                    .map(|m| (i, m))
            });
            if let Some((index, movement)) = first {
                self.current = Some(snapshot.clone());
                self.opened = true;
                let mapper = &mut self.mapper;
                return Ok(Box::new(std::iter::once(movement).chain(
                    ((index + 1)..snapshot.len()).filter_map(move |i| {
                        mapper(
                            p,
                            a,
                            step,
                            snapshot.key(),
                            snapshot.child_index(),
                            &snapshot.moves()[i],
                        )
                    }),
                )));
            }
        }
        self.opened = true;
        Ok(Box::new(std::iter::empty()))
    }
    fn placement_completed(&mut self, installed: bool) -> bool {
        if installed {
            self.visited = 0;
            return true;
        }
        match self.queue.order() {
            PlacementOrder::Once => self.queue.has_next().expect("healthy placement queue"),
            PlacementOrder::Cycle => {
                self.visited
                    < self
                        .queue
                        .placement_count()
                        .expect("healthy placement queue")
            }
        }
    }
}
impl<P, A, M, C, K, T, KS: SelectionSource<C, K>, S: PlacementSource<C, K, T>, F, G>
    MoveProvider<P, A, M> for PlacementQueueProvider<C, K, T, KS, S, F, G>
where
    F: FnMut(&P, &A, u64) -> C,
    G: FnMut(&P, &A, u64, &K, usize, &T) -> Option<M>,
{
    fn event(&mut self, event: MoveProviderEvent) {
        self.lifecycle(event);
    }
    fn construction_placement_completed(&mut self, installed: bool) -> bool {
        self.placement_completed(installed)
    }
    fn moves<'a>(&'a mut self, p: &'a P, a: &'a A, step: u64) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        self.try_moves(p, a, step)
            .unwrap_or_else(|e| panic!("placement queue provider: {e}"))
    }
}
