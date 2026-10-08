// Adapted from ChangeMoveSelectorTest and test assertion/lifecycle helpers.
// Timefold Community, Apache-2.0. Typed slots replace reflected Java entities.
use SelectionLifecycleEvent as E;
use timetuck::*;
struct Entities {
    count: usize,
    events: Vec<E>,
}
impl SelectionSource<Vec<Option<i32>>, usize> for Entities {
    fn event(&mut self, e: E, _: &Vec<Option<i32>>) {
        self.events.push(e);
    }
    fn size(&mut self, _: &Vec<Option<i32>>) -> Option<usize> {
        Some(self.count)
    }
    fn open<'a>(&'a mut self, _: &'a Vec<Option<i32>>) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        Box::new(0..self.count)
    }
}
struct Values {
    count: i32,
    events: Vec<E>,
}
impl PlacementSource<Vec<Option<i32>>, usize, i32> for Values {
    fn event(&mut self, e: E, _: &Vec<Option<i32>>) {
        self.events.push(e);
    }
    fn size(&mut self, _: &Vec<Option<i32>>, _: &usize) -> Option<usize> {
        Some(self.count as usize)
    }
    fn open<'a>(
        &'a mut self,
        _: &'a Vec<Option<i32>>,
        _: &'a usize,
    ) -> Box<dyn Iterator<Item = i32> + 'a>
    where
        i32: 'a,
    {
        Box::new(1..=self.count)
    }
}
fn code((key, value): KeyedSelectionPair<usize, i32>) -> String {
    format!("{}->{}", ["a", "b", "c", "d"][*key], value)
}
fn exercise(entity_count: usize, value_count: i32, random: bool) {
    let mut s = KeyedSelectionSource::new(
        Entities {
            count: entity_count,
            events: vec![],
        },
        Values {
            count: value_count,
            events: vec![],
        },
        KeyedSelectionLimits {
            maximum_keys: 4,
            maximum_values_per_key: 3,
            maximum_values: 12,
        },
    );
    if random {
        s = s.random(0);
    }
    let mut context = vec![None; 4];
    s.event(E::SolveStarted, &context);
    for (phase, steps) in [2, 3].into_iter().enumerate() {
        s.event(E::PhaseStarted, &context);
        for step in 0..steps {
            if phase == 0 && step == 1 {
                context[0] = Some(1);
                context[1] = Some(3);
            }
            s.event(E::StepStarted, &context);
            if random {
                // Mock entities repeat in original order; each value opening
                // starts at its first item. Supply those two policies explicitly.
                let mut next_key = 0;
                let snapshot = s.try_snapshot(&context).unwrap();
                let mut iter = snapshot.samples_with(|draw| match draw {
                    KeyedSelectionDraw::Key { bound } => {
                        let key = next_key % bound;
                        next_key += 1;
                        key
                    }
                    KeyedSelectionDraw::Value { .. } => 0,
                });
                for expected in [
                    "a->1", "b->1", "c->1", "d->1", "a->1", "b->1", "c->1", "d->1",
                ] {
                    assert_eq!(code(iter.next().unwrap().unwrap()), expected);
                }
                assert!(iter.next().unwrap().is_ok());
                assert!(s.is_never_ending());
            } else {
                let expected: Vec<_> = if entity_count == 0 || value_count == 0 {
                    vec![]
                } else {
                    vec![
                        "a->1", "a->2", "a->3", "b->1", "b->2", "b->3", "c->1", "c->2", "c->3",
                        "d->1", "d->2", "d->3",
                    ]
                };
                let mut iter = s.open(&context);
                for expected in &expected {
                    assert_eq!(&code(iter.next().unwrap()), expected);
                }
                assert!(iter.next().is_none());
                assert!(iter.next().is_none());
                drop(iter);
                assert!(!s.is_never_ending());
                assert_eq!(s.size(&context), Some(expected.len()));
            }
            s.event(E::StepEnded, &context);
        }
        if phase == 0 {
            context[2] = Some(2);
            context[3] = Some(1);
        }
        s.event(E::PhaseEnded, &context);
    }
    s.event(E::SolveEnded, &context);
    for events in [&s.key_source().events, &s.value_source().events] {
        for (e, count) in [
            (E::SolveStarted, 1),
            (E::SolveEnded, 1),
            (E::PhaseStarted, 2),
            (E::PhaseEnded, 2),
            (E::StepStarted, 5),
            (E::StepEnded, 5),
        ] {
            assert_eq!(events.iter().filter(|&&actual| actual == e).count(), count);
        }
    }
}
#[test]
fn original() {
    exercise(4, 3, false);
}
#[test]
fn empty_entity_selector_original() {
    exercise(0, 3, false);
}
#[test]
fn empty_value_selector_original() {
    exercise(4, 0, false);
}
#[test]
fn random_selection() {
    exercise(4, 3, true);
}
