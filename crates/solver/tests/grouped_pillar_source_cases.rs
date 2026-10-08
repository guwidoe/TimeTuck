// Adapted from DefaultPillarSelectorTest and PlannerAssert (Community, Apache-2.0).
// Native entity-slot grouping replaces reflection-based pillar descriptors.
use SelectionLifecycleEvent as E;
use timetuck::*;
struct Entities {
    count: usize,
    events: Vec<E>,
}
impl SelectionSource<Vec<i32>, usize> for Entities {
    fn event(&mut self, e: E, _: &Vec<i32>) {
        self.events.push(e);
    }
    fn size(&mut self, _: &Vec<i32>) -> Option<usize> {
        Some(self.count)
    }
    fn open<'a>(&'a mut self, _: &'a Vec<i32>) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        Box::new(0..self.count)
    }
}
fn exercise(empty: bool) {
    let mut values = vec![1, 2, 3, 2, 3, 3];
    let source = Entities {
        count: if empty { 0 } else { 6 },
        events: vec![],
    };
    let mut selector =
        GroupedSelectionSource::new(source, 6, |v: &Vec<i32>, &a: &usize, &b: &usize| {
            v[a] == v[b]
        });
    selector.event(E::SolveStarted, &values);
    for (phase, steps) in [2, 1].into_iter().enumerate() {
        selector.event(E::PhaseStarted, &values);
        for step in 0..steps {
            if phase == 0 && step == 1 {
                values[1] = 3;
                values[5] = 4;
            }
            selector.event(E::StepStarted, &values);
            let expected: Vec<&str> = if empty {
                vec![]
            } else if phase == 0 && step == 0 {
                vec!["[a]", "[b, d]", "[c, e, f]"]
            } else {
                vec!["[a]", "[b, c, e]", "[d]", "[f]"]
            };
            let mut iterator = selector.open(&values);
            for code in &expected {
                let group = iterator.next().unwrap();
                let actual = format!(
                    "[{}]",
                    group
                        .iter()
                        .map(|i| ["a", "b", "c", "d", "e", "f"][**i])
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                assert_eq!(&actual, code);
            }
            assert!(iterator.next().is_none());
            assert!(iterator.next().is_none());
            drop(iterator);
            assert!(!selector.is_never_ending());
            assert_eq!(selector.size(&values), Some(expected.len()));
            selector.event(E::StepEnded, &values);
        }
        selector.event(E::PhaseEnded, &values);
    }
    selector.event(E::SolveEnded, &values);
    for (event, count) in [
        (E::SolveStarted, 1),
        (E::SolveEnded, 1),
        (E::PhaseStarted, 2),
        (E::PhaseEnded, 2),
        (E::StepStarted, 3),
        (E::StepEnded, 3),
    ] {
        assert_eq!(
            selector
                .source()
                .events
                .iter()
                .filter(|&&e| e == event)
                .count(),
            count
        );
    }
}
#[test]
fn original_no_subs() {
    exercise(false);
}
#[test]
fn empty_entity_selector_original_no_subs() {
    exercise(true);
}
