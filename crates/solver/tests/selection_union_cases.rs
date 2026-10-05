// Adapted from pinned Community UnionMoveSelectorTest and PlannerAssert (Apache-2.0).
// Native explicit source events and scripted unit draws replace Java mocks.
use SelectionLifecycleEvent::*;
use timetuck::*;
struct Source {
    values: Vec<&'static str>,
    events: Vec<SelectionLifecycleEvent>,
}
impl SelectionSource<(), &'static str> for Source {
    fn event(&mut self, e: SelectionLifecycleEvent, _: &()) {
        self.events.push(e);
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(self.values.len())
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = &'static str> + 'a>
    where
        &'static str: 'a,
    {
        Box::new(self.values.iter().copied())
    }
}
fn check(empty: bool, mode: u8) {
    let values = if empty {
        vec![vec![], vec![]]
    } else {
        vec![vec!["a1", "a2", "a3"], vec!["b1", "b2"]]
    };
    let weights = if mode == 2 {
        [1000.0, 20.0]
    } else {
        [1.0, 1.0]
    };
    let mut union = SelectionUnion::new(
        values
            .into_iter()
            .zip(weights)
            .map(|(values, w)| {
                (
                    Source {
                        values,
                        events: vec![],
                    },
                    w,
                )
            })
            .collect(),
    )
    .unwrap();
    for e in [SolveStarted, PhaseStarted, StepStarted] {
        union.event(e, &());
    }
    assert!(!union.is_never_ending());
    assert_eq!(union.size(&()), Some(if empty { 0 } else { 5 }));
    let expected = if empty {
        vec![]
    } else if mode == 0 {
        vec!["a1", "a2", "a3", "b1", "b2"]
    } else {
        vec!["a1", "b1", "b2", "a2", "a3"]
    };
    if mode == 0 {
        let mut cursor = union.ordered(&());
        assert_eq!(cursor.by_ref().collect::<Vec<_>>(), expected);
        assert_eq!(cursor.next(), None);
        assert_eq!(cursor.next(), None);
    } else {
        let draws = if mode == 2 {
            vec![
                1.0 / 1020.0,
                1019.0 / 1020.0,
                1000.0 / 1020.0,
                0.0,
                999.0 / 1020.0,
            ]
        } else {
            vec![0.0, 0.75, 0.75, 0.0, 0.0]
        };
        let mut draws = draws.into_iter();
        let mut cursor = union.random_with(&(), || draws.next().expect("one draw per output"));
        assert_eq!(
            cursor.by_ref().collect::<Result<Vec<_>, _>>().unwrap(),
            expected
        );
        assert_eq!(cursor.next(), None);
        assert_eq!(cursor.next(), None);
    }
    for e in [StepEnded, PhaseEnded, SolveEnded] {
        union.event(e, &());
    }
    for child in union.children() {
        assert_eq!(
            child.events,
            [
                SolveStarted,
                PhaseStarted,
                StepStarted,
                StepEnded,
                PhaseEnded,
                SolveEnded
            ]
        );
    }
}
#[test]
fn original_selection() {
    check(false, 0)
}
#[test]
fn empty_original_selection() {
    check(true, 0)
}
#[test]
fn biased_random_selection() {
    check(false, 2)
}
#[test]
fn uniform_random_selection() {
    check(false, 1)
}
#[test]
fn empty_uniform_random_selection() {
    check(true, 1)
}
#[test]
fn empty_biased_random_selection() {
    check(true, 2)
}
