//! One recording selection feeding independent dependent-choice cursors.
use crate::{SelectionLifecycleEvent, SelectionSource};
use std::{cell::RefCell, error::Error, fmt, marker::PhantomData, rc::Rc, sync::Arc};

enum Publication<T> {
    Empty,
    Announced {
        ticket: Rc<()>,
        value: Option<Arc<T>>,
    },
    Exhausted,
}
struct Channel<T> {
    stream: Rc<()>,
    publication: Publication<T>,
    size: Option<usize>,
    repeating: bool,
}
impl<T> Channel<T> {
    fn reset(&mut self) {
        self.stream = Rc::new(());
        self.publication = Publication::Empty;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionReplayError {
    NotRecorded,
    StaleCursor,
}
impl fmt::Display for SelectionReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotRecorded => "the recording cursor has not published a choice",
            Self::StaleCursor => "the recording traversal or lifecycle scope has changed",
        })
    }
}
impl Error for SelectionReplayError {}

/// Own a candidate source and publish each selected occurrence to its replay views.
/// Lifecycle events are forwarded once by this owner; replay views share its
/// invalidation and never forward duplicate events. Opening a traversal or delivering
/// any lifecycle event invalidates existing replay cursors. Create replay cursors
/// after opening the recording traversal. Metadata describes the child selection,
/// not the single item available to an individual replay cursor.
///
/// A source can borrow context as usual. A source item is moved into Arc once, so
/// neither recording nor replaying requires Clone. Channels are local to one thread;
/// held Arc values may outlive the channel. Events remain explicitly caller-driven.
pub struct RecordingSelection<C, T, S> {
    source: S,
    channel: Rc<RefCell<Channel<T>>>,
    context: PhantomData<fn(&C)>,
}
impl<C, T, S: SelectionSource<C, T>> RecordingSelection<C, T, S> {
    pub fn new(source: S) -> Self {
        let repeating = source.is_never_ending();
        Self {
            source,
            channel: Rc::new(RefCell::new(Channel {
                stream: Rc::new(()),
                publication: Publication::Empty,
                size: None,
                repeating,
            })),
            context: PhantomData,
        }
    }
    pub fn source(&self) -> &S {
        &self.source
    }
    pub fn source_mut(&mut self) -> &mut S {
        &mut self.source
    }
    pub fn replaying(&self) -> ReplayingSelection<T> {
        ReplayingSelection {
            channel: self.channel.clone(),
        }
    }
    pub fn is_never_ending(&self) -> bool {
        self.channel.borrow().repeating
    }
    pub fn size(&mut self, context: &C) -> Option<usize> {
        let size = self.source.size(context);
        self.channel.borrow_mut().size = size;
        size
    }
    pub fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        {
            let mut channel = self.channel.borrow_mut();
            channel.reset();
            channel.size = None;
        }
        self.source.event(event, context);
    }
    pub fn open<'a>(&'a mut self, context: &'a C) -> RecordingSelectionIter<'a, T>
    where
        T: 'a,
    {
        {
            let mut channel = self.channel.borrow_mut();
            channel.reset();
            channel.size = None;
        }
        self.size(context);
        let source = self.source.open(context);
        RecordingSelectionIter {
            source,
            channel: self.channel.clone(),
            buffered: None,
            exhausted: false,
        }
    }
}

/// Shared replay view. Cloning the view does not clone payloads; each cursor has
/// its own consumption marker. Replay is of the latest publication, not a queue:
/// a slow consumer skips choices superseded by the recorder's next announcement.
pub struct ReplayingSelection<T> {
    channel: Rc<RefCell<Channel<T>>>,
}
impl<T> Clone for ReplayingSelection<T> {
    fn clone(&self) -> Self {
        Self {
            channel: self.channel.clone(),
        }
    }
}
impl<T> ReplayingSelection<T> {
    pub fn size(&self) -> Option<usize> {
        self.channel.borrow().size
    }
    pub fn is_never_ending(&self) -> bool {
        self.channel.borrow().repeating
    }
    pub fn cursor(&self) -> SelectionReplayCursor<T> {
        SelectionReplayCursor {
            channel: self.channel.clone(),
            stream: self.channel.borrow().stream.clone(),
            consumed: None,
        }
    }
}

/// A replay may report available after recorder.has_next(), but next_replayed()
/// succeeds only after recorder.next() publishes the item. Repeated availability
/// calls are passive. Each cursor consumes a publication at most once; a later
/// recorder advance makes it available again. No Iterator implementation hides
/// the distinction between an unrecorded choice, temporary absence and stale scope.
pub struct SelectionReplayCursor<T> {
    channel: Rc<RefCell<Channel<T>>>,
    stream: Rc<()>,
    consumed: Option<Rc<()>>,
}
impl<T> SelectionReplayCursor<T> {
    pub fn has_next(&self) -> bool {
        let channel = self.channel.borrow();
        if !Rc::ptr_eq(&channel.stream, &self.stream) {
            return false;
        }
        match &channel.publication {
            Publication::Announced { ticket, .. } => !self
                .consumed
                .as_ref()
                .is_some_and(|t| Rc::ptr_eq(t, ticket)),
            _ => false,
        }
    }
    pub fn next_replayed(&mut self) -> Result<Option<Arc<T>>, SelectionReplayError> {
        let channel = self.channel.borrow();
        if !Rc::ptr_eq(&channel.stream, &self.stream) {
            return Err(SelectionReplayError::StaleCursor);
        }
        match &channel.publication {
            Publication::Empty => Err(SelectionReplayError::NotRecorded),
            Publication::Exhausted => Ok(None),
            Publication::Announced { ticket, value } => {
                if self
                    .consumed
                    .as_ref()
                    .is_some_and(|t| Rc::ptr_eq(t, ticket))
                {
                    return Ok(None);
                }
                let value = value.clone().ok_or(SelectionReplayError::NotRecorded)?;
                self.consumed = Some(ticket.clone());
                Ok(Some(value))
            }
        }
    }
}

/// has_next() prefetches at most one Rust source item and announces availability;
/// next() publishes it. Repeated probes do not advance the source. A source None
/// fuses this recorder. Dropping it retains the last published item but discards
/// an announced, unpublished lookahead. Callback effects are not rolled back.
pub struct RecordingSelectionIter<'a, T> {
    source: Box<dyn Iterator<Item = T> + 'a>,
    channel: Rc<RefCell<Channel<T>>>,
    buffered: Option<Arc<T>>,
    exhausted: bool,
}
impl<T> RecordingSelectionIter<'_, T> {
    pub fn has_next(&mut self) -> bool {
        if self.exhausted {
            return false;
        }
        if self.buffered.is_some() {
            return true;
        }
        // Invalidate the old choice before invoking user code, including unwinding.
        self.channel.borrow_mut().publication = Publication::Empty;
        match self.source.next() {
            Some(item) => {
                self.buffered = Some(Arc::new(item));
                self.channel.borrow_mut().publication = Publication::Announced {
                    ticket: Rc::new(()),
                    value: None,
                };
                true
            }
            None => {
                self.exhausted = true;
                self.channel.borrow_mut().publication = Publication::Exhausted;
                false
            }
        }
    }
}
impl<T> Iterator for RecordingSelectionIter<'_, T> {
    type Item = Arc<T>;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.has_next() {
            return None;
        }
        let item = self.buffered.take().unwrap();
        if let Publication::Announced { value, .. } = &mut self.channel.borrow_mut().publication {
            *value = Some(item.clone());
        }
        Some(item)
    }
}
impl<T> std::iter::FusedIterator for RecordingSelectionIter<'_, T> {}
impl<T> Drop for RecordingSelectionIter<'_, T> {
    fn drop(&mut self) {
        if self.buffered.is_some() {
            self.channel.borrow_mut().publication = Publication::Exhausted;
        }
    }
}
