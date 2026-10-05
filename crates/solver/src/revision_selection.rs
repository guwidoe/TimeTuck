//! Phase snapshots with optional revision-triggered refresh at step boundaries.
use crate::{
    CachedSelectionOrder, SelectionCacheError, SelectionCacheState, SelectionLifecycleEvent,
    SelectionSource, WeightedSelection, WeightedSelectionError, random::SplitMix64,
};
use std::{iter::FusedIterator, marker::PhantomData};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevisionRefresh {
    Phase,
    Step,
}
/// Explicit lifecycle owner for a finite source whose membership has an Eq
/// revision token. Every phase start captures a new snapshot. Step refresh checks
/// the token at step start, reusing the phase snapshot when equal. Phase refresh
/// ignores mid-phase changes. The caller must change the token whenever any fact
/// affecting the source changes; editing source_mut alone does not invalidate it.
///
/// Failed/panicking builds publish no snapshot. Invalid transitions change nothing.
/// Valid events advance state before callbacks; close the scope normally after an
/// error. Step refresh may recover at a later step; phase refresh at the next phase.
/// Held snapshots own their items independently, including non-Clone payloads.
/// Events reach the child once each. Random selection repeats within the current
/// snapshot and requires a consumer limit; no Java PRNG trace parity is implied.
pub struct RevisionedSelection<C, T, S, R, F> {
    source: S,
    revision: F,
    policy: RevisionRefresh,
    order: CachedSelectionOrder,
    maximum: usize,
    state: SelectionCacheState,
    token: Option<R>,
    snapshot: Option<WeightedSelection<T>>,
    marker: PhantomData<fn(&C)>,
}
impl<C, T, S: SelectionSource<C, T>, R: Eq, F: FnMut(&C) -> R> RevisionedSelection<C, T, S, R, F> {
    pub fn new(
        source: S,
        policy: RevisionRefresh,
        order: CachedSelectionOrder,
        maximum: usize,
        revision: F,
    ) -> Result<Self, SelectionCacheError> {
        if source.is_never_ending() {
            return Err(SelectionCacheError::UnboundedSource);
        }
        Ok(Self {
            source,
            revision,
            policy,
            order,
            maximum,
            state: SelectionCacheState::Idle,
            token: None,
            snapshot: None,
            marker: PhantomData,
        })
    }
    pub fn source(&self) -> &S {
        &self.source
    }
    pub fn source_mut(&mut self) -> &mut S {
        &mut self.source
    }
    pub fn state(&self) -> SelectionCacheState {
        self.state
    }
    pub fn revision(&self) -> Option<&R> {
        self.snapshot.as_ref().and(self.token.as_ref())
    }
    pub fn is_never_ending(&self) -> bool {
        self.order == CachedSelectionOrder::Random
    }
    pub fn snapshot(&self) -> Result<WeightedSelection<T>, SelectionCacheError> {
        self.snapshot.clone().ok_or(SelectionCacheError::NotReady)
    }
    pub fn size(&self) -> Result<usize, SelectionCacheError> {
        Ok(self.current()?.len())
    }
    fn current(&self) -> Result<&WeightedSelection<T>, SelectionCacheError> {
        self.snapshot.as_ref().ok_or(SelectionCacheError::NotReady)
    }
    fn build(&mut self, context: &C, token: R) -> Result<(), SelectionCacheError> {
        self.snapshot = None;
        self.token = None;
        let declared = self.source.size(context);
        if declared.is_some_and(|n| n > self.maximum) {
            return Err(WeightedSelectionError::CandidateLimit {
                maximum: self.maximum,
            }
            .into());
        }
        let snapshot = WeightedSelection::from_bounded_iter(
            self.source.open(context).map(|v| (v, 1.0)),
            self.maximum,
        )?;
        if let Some(declared) = declared
            && declared != snapshot.len()
        {
            return Err(SelectionCacheError::SizeMismatch {
                declared,
                actual: snapshot.len(),
            });
        }
        self.snapshot = Some(snapshot);
        self.token = Some(token);
        Ok(())
    }
    pub fn event(
        &mut self,
        event: SelectionLifecycleEvent,
        context: &C,
    ) -> Result<(), SelectionCacheError> {
        use SelectionCacheState as State;
        use SelectionLifecycleEvent as Event;
        let next = match (self.state, event) {
            (State::Idle, Event::SolveStarted) => State::Solving,
            (State::Solving, Event::PhaseStarted) => State::Phase,
            (State::Phase, Event::StepStarted) => State::Step,
            (State::Step, Event::StepEnded) => State::Phase,
            (State::Phase, Event::PhaseEnded) => State::Solving,
            (State::Solving, Event::SolveEnded) => State::Idle,
            _ => {
                return Err(SelectionCacheError::InvalidTransition {
                    state: self.state,
                    event,
                });
            }
        };
        self.state = next;
        // Temporarily withdraw the snapshot at starts so callback unwinding cannot
        // expose a snapshot whose revision was not successfully checked.
        let previous = if matches!(
            event,
            Event::PhaseStarted | Event::StepStarted | Event::PhaseEnded | Event::SolveEnded
        ) {
            self.snapshot.take()
        } else {
            None
        };
        if matches!(
            event,
            Event::PhaseStarted | Event::PhaseEnded | Event::SolveEnded
        ) {
            self.token = None;
        }
        self.source.event(event, context);
        match event {
            Event::PhaseStarted => {
                let token = (self.revision)(context);
                self.build(context, token)?;
            }
            Event::StepStarted if self.policy == RevisionRefresh::Step => {
                let token = (self.revision)(context);
                if previous.is_some() && self.token.as_ref() == Some(&token) {
                    self.snapshot = previous;
                } else {
                    self.build(context, token)?;
                }
            }
            Event::StepStarted => self.snapshot = previous,
            _ => {}
        }
        Ok(())
    }
    pub fn list_iter(&self) -> Result<std::slice::Iter<'_, T>, SelectionCacheError> {
        self.list_iter_from(0)
    }
    pub fn list_iter_from(
        &self,
        index: usize,
    ) -> Result<std::slice::Iter<'_, T>, SelectionCacheError> {
        if self.order == CachedSelectionOrder::Random {
            return Err(SelectionCacheError::RandomListAccess);
        }
        let items = self.current()?.items();
        Ok(items
            .get(index..)
            .ok_or(SelectionCacheError::IndexOutOfBounds {
                index,
                len: items.len(),
            })?
            .iter())
    }
    pub fn iter_with<D: FnMut() -> f64>(
        &self,
        draw: D,
    ) -> Result<RevisionSelectionIter<'_, T, D>, SelectionCacheError> {
        Ok(RevisionSelectionIter {
            snapshot: self.current()?,
            order: self.order,
            position: 0,
            draw,
            failed: false,
        })
    }
    pub fn iter(
        &self,
        seed: u64,
    ) -> Result<RevisionSelectionIter<'_, T, impl FnMut() -> f64>, SelectionCacheError> {
        let mut random = SplitMix64::new(seed);
        self.iter_with(move || random.unit())
    }
}
/// Passive availability checks do not consume random draws. An invalid unit draw
/// produces one error and fuses the cursor; original iteration never calls draw.
pub struct RevisionSelectionIter<'a, T, D> {
    snapshot: &'a WeightedSelection<T>,
    order: CachedSelectionOrder,
    position: usize,
    draw: D,
    failed: bool,
}
impl<T, D> RevisionSelectionIter<'_, T, D> {
    pub fn has_next(&self) -> bool {
        !self.failed
            && match self.order {
                CachedSelectionOrder::Original => self.position < self.snapshot.len(),
                CachedSelectionOrder::Random => self.snapshot.active_len() > 0,
            }
    }
}
impl<'a, T, D: FnMut() -> f64> Iterator for RevisionSelectionIter<'a, T, D> {
    type Item = Result<&'a T, WeightedSelectionError>;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.has_next() {
            return None;
        }
        match self.order {
            CachedSelectionOrder::Original => {
                let item = &self.snapshot.items()[self.position];
                self.position += 1;
                Some(Ok(item))
            }
            CachedSelectionOrder::Random => match self.snapshot.choose((self.draw)()) {
                Ok(Some(item)) => Some(Ok(item)),
                Ok(None) => None,
                Err(e) => {
                    self.failed = true;
                    Some(Err(e))
                }
            },
        }
    }
}
impl<T, D: FnMut() -> f64> FusedIterator for RevisionSelectionIter<'_, T, D> {}
