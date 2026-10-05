// Adapted from CartesianProductMoveSelectorTest.originalMimic and PlannerAssert.
// Timefold Community, Apache-2.0. Explicit prefixes replace Java mimic selectors.
use SelectionLifecycleEvent::*;
use std::{cell::RefCell, rc::Rc, sync::Arc};
use timetuck::*;
#[derive(Default)]
struct Fixture {
    events: [Vec<SelectionLifecycleEvent>; 3],
}
fn original(ignore: bool) {
    let fixture = Rc::new(RefCell::new(Fixture::default()));
    let hook = fixture.clone();
    let factory = |_: &(), depth: usize, prefix: &[Option<Arc<String>>]| {
        let values = match depth {
            0 => vec!["a".to_string(), "b".to_string()],
            1 => ["1", "2", "3"]
                .map(|v| format!("{}->{v}", prefix[0].as_ref().unwrap()))
                .to_vec(),
            2 => ["8", "9"]
                .map(|v| format!("{}->{v}", prefix[0].as_ref().unwrap()))
                .to_vec(),
            _ => unreachable!(),
        };
        values.into_iter()
    };
    let mut selection = DependentSelection::new(3, ignore, factory, move |event, _: &()| {
        for events in &mut hook.borrow_mut().events {
            events.push(event);
        }
    });
    selection.event(SolveStarted, &());
    selection.event(PhaseStarted, &());
    for _ in 0..2 {
        selection.event(StepStarted, &());
        let mut iter = selection.iter(&(), 1000);
        let expected = [
            "a->1+a->8",
            "a->1+a->9",
            "a->2+a->8",
            "a->2+a->9",
            "a->3+a->8",
            "a->3+a->9",
            "b->1+b->8",
            "b->1+b->9",
            "b->2+b->8",
            "b->2+b->9",
            "b->3+b->8",
            "b->3+b->9",
        ];
        for expected in expected {
            let row = iter.next().unwrap();
            assert_eq!(
                format!("{}+{}", row[1].as_ref().unwrap(), row[2].as_ref().unwrap()),
                expected
            );
        }
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.stats().stop, Some(DependentStop::Exhausted));
        drop(iter);
        selection.event(StepEnded, &());
    }
    selection.event(PhaseEnded, &());
    selection.event(SolveEnded, &());
    for events in &fixture.borrow().events {
        for (e, n) in [
            (SolveStarted, 1),
            (SolveEnded, 1),
            (PhaseStarted, 1),
            (PhaseEnded, 1),
            (StepStarted, 2),
            (StepEnded, 2),
        ] {
            assert_eq!(events.iter().filter(|&&v| v == e).count(), n);
        }
    }
}
#[test]
fn original_mimic_not_ignoring_empty() {
    original(false);
}
#[test]
fn original_mimic_ignoring_empty() {
    original(true);
}
