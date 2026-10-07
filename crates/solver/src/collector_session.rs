//! Captured contributions with session-scoped removal handles.
use crate::ReversibleCollector;
use std::{collections::BTreeMap, rc::Rc};

/// Identifies one captured contribution. Clones identify the same contribution;
/// retaining a handle does not keep its contribution alive after removal/clear.
#[derive(Clone, Debug)]
pub struct CollectorHandle {
    owner: Rc<()>,
    ordinal: u64,
}
impl CollectorHandle {
    /// Monotonically allocated within its session, never reused after clear.
    pub fn ordinal(&self) -> u64 {
        self.ordinal
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectorSessionError {
    ForeignHandle,
    MissingHandle,
    RejectedContribution,
    IdExhausted,
}
impl std::fmt::Display for CollectorSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::ForeignHandle => "collector handle belongs to a different session",
            Self::MissingHandle => "collector contribution has already been removed",
            Self::RejectedContribution => "collector rejected a captured contribution",
            Self::IdExhausted => "collector contribution identities exhausted",
        })
    }
}
impl std::error::Error for CollectorSessionError {}

/// Own the exact contribution of each insertion, so removing a handle never needs
/// to reevaluate mutable source facts. Equal contributions remain separate slots.
/// Works with every ReversibleCollector, including list, pair and finisher adapters.
/// V needs neither Clone nor Eq. Contributions and collector snapshots must obey
/// the immutable-value contract of ReversibleCollector.
///
/// Collector mutations are staged on an isolated clone. A rejected removal/update
/// or ordinary mapper/collector panic preserves the previous session. As with the
/// underlying collectors, shared interior mutation and panicking destructors are
/// outside this guarantee. Handles are scoped to a session, with no cloning of the
/// session itself. Removal is explicit, not automatic on handle drop.
///
/// ```
/// use timetuck::{CollectorSession, ListCollector};
/// let mut c = CollectorSession::<(u64, &str), ListCollector<u64, &str>>::new();
/// let first = c.insert_with(|id| (id,"same"))?;
/// let second = c.insert_with(|id| (id,"same"))?;
/// let snapshot = c.finish();
/// c.remove(&first)?;
/// assert_eq!(c.finish().len(),1);
/// assert_eq!(snapshot.len(),2);
/// c.replace_with(&second, |id| (id,"changed"))?;
/// assert_eq!(c.finish()[0],"changed");
/// # Ok::<(), timetuck::CollectorSessionError>(())
/// ```
pub struct CollectorSession<V, C> {
    collector: C,
    captured: BTreeMap<u64, V>,
    owner: Rc<()>,
    next: u64,
}
impl<V, C: ReversibleCollector<V>> Default for CollectorSession<V, C> {
    fn default() -> Self {
        Self::new()
    }
}
impl<V, C: ReversibleCollector<V>> CollectorSession<V, C> {
    pub fn new() -> Self {
        Self {
            collector: C::default(),
            captured: BTreeMap::new(),
            owner: Rc::new(()),
            next: 0,
        }
    }
    pub fn len(&self) -> usize {
        self.captured.len()
    }
    pub fn is_empty(&self) -> bool {
        self.captured.is_empty()
    }
    pub fn finish(&self) -> C::Output {
        self.collector.finish()
    }
    fn validate(&self, handle: &CollectorHandle) -> Result<&V, CollectorSessionError> {
        if !Rc::ptr_eq(&handle.owner, &self.owner) {
            return Err(CollectorSessionError::ForeignHandle);
        }
        self.captured
            .get(&handle.ordinal)
            .ok_or(CollectorSessionError::MissingHandle)
    }
    pub fn contribution(&self, handle: &CollectorHandle) -> Result<&V, CollectorSessionError> {
        self.validate(handle)
    }
    pub fn insert(&mut self, contribution: V) -> Result<CollectorHandle, CollectorSessionError> {
        self.insert_with(|_| contribution)
    }
    /// Map once, using the new slot's ordinal when an ordered list key is needed.
    /// Mapping is complete before the collector changes.
    pub fn insert_with(
        &mut self,
        map: impl FnOnce(u64) -> V,
    ) -> Result<CollectorHandle, CollectorSessionError> {
        let next = self
            .next
            .checked_add(1)
            .ok_or(CollectorSessionError::IdExhausted)?;
        let value = map(self.next);
        let mut collector = self.collector.clone();
        collector.insert(&value);
        let handle = CollectorHandle {
            owner: self.owner.clone(),
            ordinal: self.next,
        };
        self.captured.insert(self.next, value);
        self.collector = collector;
        self.next = next;
        Ok(handle)
    }
    pub fn remove(&mut self, handle: &CollectorHandle) -> Result<V, CollectorSessionError> {
        let old = self.validate(handle)?;
        let mut collector = self.collector.clone();
        if !collector.remove(old) {
            return Err(CollectorSessionError::RejectedContribution);
        }
        let old = self
            .captured
            .remove(&handle.ordinal)
            .expect("validated captured slot");
        self.collector = collector;
        Ok(old)
    }
    /// Replace a slot while preserving its handle. Returns the captured old value.
    pub fn replace_with(
        &mut self,
        handle: &CollectorHandle,
        map: impl FnOnce(u64) -> V,
    ) -> Result<V, CollectorSessionError> {
        let old = self.validate(handle)?;
        let value = map(handle.ordinal);
        let mut collector = self.collector.clone();
        if !collector.replace(old, &value) {
            return Err(CollectorSessionError::RejectedContribution);
        }
        let old = self
            .captured
            .insert(handle.ordinal, value)
            .expect("validated captured slot");
        self.collector = collector;
        Ok(old)
    }
    /// Invalidate all current handles without recycling ordinals. Saved immutable
    /// results remain valid. Default construction runs before changing the session.
    pub fn clear(&mut self) {
        let empty = C::default();
        self.collector = empty;
        self.captured.clear();
    }
}
