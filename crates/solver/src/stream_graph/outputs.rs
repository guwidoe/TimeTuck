//! Small erased-output batches. Only Many owns a vector; no payload is cloned.
use super::Event;
use std::{iter::FusedIterator, ops::Deref};
#[derive(Default)]
pub(super) enum Outputs {
    #[default]
    Empty,
    One(Event),
    Many(Vec<Event>),
}
impl Outputs {
    pub(super) fn new() -> Self {
        Self::Empty
    }
    pub(super) fn one(event: Event) -> Self {
        Self::One(event)
    }
    pub(super) fn for_each(self, mut callback: impl FnMut(Event)) {
        match self {
            Self::Empty => {}
            Self::One(event) => callback(event),
            Self::Many(events) => events.into_iter().for_each(callback),
        }
    }
}
impl Deref for Outputs {
    type Target = [Event];
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Empty => &[],
            Self::One(event) => std::slice::from_ref(event),
            Self::Many(events) => events,
        }
    }
}
impl From<Vec<Event>> for Outputs {
    fn from(mut events: Vec<Event>) -> Self {
        match events.len() {
            0 => Self::Empty,
            1 => Self::One(events.pop().unwrap()),
            _ => Self::Many(events),
        }
    }
}
impl FromIterator<Event> for Outputs {
    fn from_iter<I: IntoIterator<Item = Event>>(events: I) -> Self {
        let mut events = events.into_iter();
        if events.size_hint().0 >= 2 {
            return Self::Many(events.collect());
        }
        let Some(first) = events.next() else {
            return Self::Empty;
        };
        let Some(second) = events.next() else {
            return Self::One(first);
        };
        let mut values = vec![first, second];
        values.extend(events);
        Self::Many(values)
    }
}
pub(super) enum IntoIter {
    Empty,
    One(std::option::IntoIter<Event>),
    Many(std::vec::IntoIter<Event>),
}
impl Iterator for IntoIter {
    type Item = Event;
    fn next(&mut self) -> Option<Event> {
        match self {
            Self::Empty => None,
            Self::One(iter) => iter.next(),
            Self::Many(iter) => iter.next(),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Empty => (0, Some(0)),
            Self::One(iter) => iter.size_hint(),
            Self::Many(iter) => iter.size_hint(),
        }
    }
}
impl DoubleEndedIterator for IntoIter {
    fn next_back(&mut self) -> Option<Event> {
        match self {
            Self::Empty => None,
            Self::One(iter) => iter.next_back(),
            Self::Many(iter) => iter.next_back(),
        }
    }
}
impl ExactSizeIterator for IntoIter {}
impl FusedIterator for IntoIter {}
impl IntoIterator for Outputs {
    type Item = Event;
    type IntoIter = IntoIter;
    fn into_iter(self) -> IntoIter {
        match self {
            Self::Empty => IntoIter::Empty,
            Self::One(event) => IntoIter::One(Some(event).into_iter()),
            Self::Many(events) => IntoIter::Many(events.into_iter()),
        }
    }
}
impl<'a> IntoIterator for &'a Outputs {
    type Item = &'a Event;
    type IntoIter = std::slice::Iter<'a, Event>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::RefCell,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };
    type Trace = Rc<RefCell<Vec<usize>>>;
    struct Payload(usize, Trace); // Deliberately non-Clone.
    impl Drop for Payload {
        fn drop(&mut self) {
            self.1.borrow_mut().push(self.0);
        }
    }
    fn event(id: usize, trace: &Trace) -> Event {
        Event::new(Payload(id, trace.clone()))
    }
    fn ids(events: &[Event]) -> Vec<usize> {
        events
            .iter()
            .map(|event| event.downcast_ref::<Payload>().unwrap().0)
            .collect()
    }
    #[test]
    fn small_variant_selection_construction_borrow_and_forward_iteration_match_vec() {
        let trace = Trace::default();
        let empty = Outputs::new();
        assert!(empty.is_empty());
        assert!(matches!(empty, Outputs::Empty));
        let one = Outputs::one(event(0, &trace));
        assert!(matches!(one, Outputs::One(_)));
        assert_eq!(one.len(), 1);
        assert_eq!(ids(&one), [0]);
        one.for_each(drop);
        let many = Outputs::from(vec![event(1, &trace), event(2, &trace)]);
        assert!(matches!(many, Outputs::Many(_)));
        assert_eq!(ids(&many), [1, 2]);
        assert_eq!((&many).into_iter().count(), 2);
        many.for_each(drop);
        assert_eq!(*trace.borrow(), [0, 1, 2]);
        for count in 0..4 {
            let exact: Outputs = (0..count).map(|id| event(id, &trace)).collect();
            let lazy: Outputs = (0..count)
                .filter(|_| true)
                .map(|id| event(id, &trace))
                .collect();
            let expected = match count {
                0 => 0,
                1 => 1,
                _ => 2,
            };
            for collection in [exact, lazy] {
                assert_eq!(
                    match &collection {
                        Outputs::Empty => 0,
                        Outputs::One(_) => 1,
                        Outputs::Many(_) => 2,
                    },
                    expected
                );
                assert_eq!(ids(&collection), (0..count).collect::<Vec<_>>());
                drop(collection);
            }
            let converted =
                Outputs::from((0..count).map(|id| event(id, &trace)).collect::<Vec<_>>());
            assert_eq!(ids(&converted), (0..count).collect::<Vec<_>>());
            assert_eq!(
                match converted {
                    Outputs::Empty => 0,
                    Outputs::One(_) => 1,
                    Outputs::Many(_) => 2,
                },
                expected
            );
        }
    }
    #[test]
    fn partial_owned_iteration_keeps_aliases_and_drops_remaining_events_in_order() {
        for count in [0, 1, 4] {
            let trace = Trace::default();
            let events: Vec<_> = (0..count).map(|id| event(id, &trace)).collect();
            let weak: Vec<_> = events
                .iter()
                .map(|event| Rc::downgrade(event.shared_ref()))
                .collect();
            let alias = events.first().cloned();
            let mut iter = Outputs::from(events).into_iter();
            assert_eq!(iter.len(), count);
            if count > 0 {
                let first = iter.next().unwrap();
                assert_eq!(Rc::strong_count(first.shared_ref()), 2);
                drop(first);
            }
            drop(iter);
            assert_eq!(*trace.borrow(), (1..count).collect::<Vec<_>>());
            if count > 0 {
                assert!(weak[0].upgrade().is_some());
            }
            drop(alias);
            assert!(weak.iter().all(|event| event.upgrade().is_none()));
            let expected: Vec<_> = (1..count).chain((count > 0).then_some(0)).collect();
            assert_eq!(*trace.borrow(), expected);
        }
    }
    fn panic_trace(small: bool, during_collection: bool, failure: usize) -> Vec<usize> {
        let trace = Trace::default();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let produced = (0..4).filter(|_| true).map(|id| {
                    assert!(!(during_collection && id == failure), "later producer");
                    event(id, &trace)
                });
                if small {
                    let output: Outputs = produced.collect();
                    output.for_each(|event| {
                        let id = event.downcast_ref::<Payload>().unwrap().0;
                        assert!(id != failure, "later consumer");
                        drop(event);
                    });
                } else {
                    let output: Vec<Event> = produced.collect();
                    output.into_iter().for_each(|event| {
                        let id = event.downcast_ref::<Payload>().unwrap().0;
                        assert!(id != failure, "later consumer");
                        drop(event);
                    });
                }
            }))
            .is_err()
        );
        trace.borrow().clone()
    }
    #[test]
    fn later_lazy_callback_panics_preserve_produced_payload_drop_order_against_vec() {
        for during_collection in [false, true] {
            for failure in [1, 2, 3] {
                let actual = panic_trace(true, during_collection, failure);
                assert_eq!(actual, panic_trace(false, during_collection, failure));
                assert_eq!(
                    actual,
                    (0..if during_collection { failure } else { 4 }).collect::<Vec<_>>()
                );
            }
        }
    }
}
