// Adapted from Timefold Community SwapMoveSelectorTest and PlannerAssert.
// Apache-2.0; see compat/provenance.json. Native sources replace mocked entities.
use MoveProviderEvent as P;
use SelectionLifecycleEvent as E;
use std::{cell::RefCell, rc::Rc, sync::Arc};
use timetuck::*;
type Events = Rc<RefCell<Vec<E>>>;
struct Entities {
    ids: Vec<usize>,
    events: Events,
}
impl SelectionSource<(), usize> for Entities {
    fn event(&mut self, event: E, _: &()) {
        self.events.borrow_mut().push(event);
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(self.ids.len())
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        Box::new(self.ids.iter().copied())
    }
}
fn verify_lifecycle(events: &[E]) {
    for (event, n) in [
        (E::SolveStarted, 1),
        (E::PhaseStarted, 2),
        (E::StepStarted, 5),
        (E::StepEnded, 5),
        (E::PhaseEnded, 2),
        (E::SolveEnded, 1),
    ] {
        assert_eq!(
            events.iter().filter(|&&e| e == event).count(),
            n,
            "{event:?}"
        );
    }
}
fn finite_case(left: &[&str], right: Option<&[&str]>, expected: &[&str]) {
    let left_events = Events::default();
    let right_events = Events::default();
    let mut names = left.to_vec();
    if let Some(right) = right {
        names.extend_from_slice(right);
    }
    let left_source = Entities {
        ids: (0..left.len()).collect(),
        events: left_events.clone(),
    };
    let source = if let Some(right) = right {
        PairSelectionSource::separate(
            left_source,
            Entities {
                ids: (left.len()..left.len() + right.len()).collect(),
                events: right_events.clone(),
            },
            100,
        )
    } else {
        PairSelectionSource::shared(left_source, 100)
    };
    let p = FiniteProblem::new(vec![Variable::new(vec![0i32]); names.len()]).unwrap();
    let a = p.initial_assignment();
    let mut selector = CachedMoveProvider::uniform(
        source,
        CachedProviderConfig::default(),
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| (),
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64, pair: &(Arc<usize>, Arc<usize>)| {
            Some(Move::<i32>::Swap {
                left: *pair.0,
                right: *pair.1,
            })
        },
    )
    .unwrap();
    selector.event(P::SolveStarted);
    for steps in [2, 3] {
        selector.event(P::PhaseStarted);
        for step in 0..steps {
            selector.event(P::StepStarted);
            let mut moves = selector.moves(&p, &a, step);
            let codes: Vec<_> = moves
                .by_ref()
                .map(|movement| {
                    let Move::Swap { left, right } = movement else {
                        panic!("expected swap")
                    };
                    format!("{}<->{}", names[left], names[right])
                })
                .collect();
            assert_eq!(codes, expected);
            assert!(moves.next().is_none());
            assert!(moves.next().is_none());
            drop(moves);
            assert!(!selector.cache().is_never_ending());
            assert_eq!(selector.cache().size().unwrap(), expected.len());
            selector.event(P::StepEnded);
        }
        selector.event(P::PhaseEnded);
    }
    selector.event(P::SolveEnded);
    verify_lifecycle(&left_events.borrow());
    if right.is_some() {
        verify_lifecycle(&right_events.borrow());
    } else {
        assert!(right_events.borrow().is_empty());
    }
}
#[test]
fn original_left_equals_right() {
    finite_case(
        &["a", "b", "c", "d"],
        None,
        &["a<->b", "a<->c", "a<->d", "b<->c", "b<->d", "c<->d"],
    );
}
#[test]
fn empty_original_left_equals_right() {
    finite_case(&[], None, &[]);
}
#[test]
fn original_left_unequals_right() {
    finite_case(
        &["a", "b", "c", "d"],
        Some(&["x", "y", "z"]),
        &[
            "a<->x", "a<->y", "a<->z", "b<->x", "b<->y", "b<->z", "c<->x", "c<->y", "c<->z",
            "d<->x", "d<->y", "d<->z",
        ],
    );
}
#[test]
fn empty_right_original_left_unequals_right() {
    finite_case(&["a", "b", "c", "d"], Some(&[]), &[]);
}
