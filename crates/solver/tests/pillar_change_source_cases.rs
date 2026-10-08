// Adapted from PillarChangeMoveSelectorTest.original and PlannerAssert.
// Timefold Community, Apache-2.0; see compat/provenance.json for provenance.
// Typed entity slots and owned assignment context replace reflected descriptors.
use timetuck::*;

struct Entities;
impl SelectionSource<Assignment<i32>, usize> for Entities {
    fn size(&mut self, _: &Assignment<i32>) -> Option<usize> {
        Some(4)
    }
    fn open<'a>(&'a mut self, _: &'a Assignment<i32>) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        Box::new(0..4)
    }
}
struct Values;
impl PlacementSource<Assignment<i32>, SelectionGroup<usize>, i32> for Values {
    fn size(&mut self, _: &Assignment<i32>, _: &SelectionGroup<usize>) -> Option<usize> {
        Some(3)
    }
    fn open<'a>(
        &'a mut self,
        _: &'a Assignment<i32>,
        _: &'a SelectionGroup<usize>,
    ) -> Box<dyn Iterator<Item = i32> + 'a>
    where
        i32: 'a,
    {
        Box::new(1..=3)
    }
}
#[test]
fn original() {
    let problem =
        FiniteProblem::new((0..4).map(|_| Variable::new(vec![1, 2, 3])).collect()).unwrap();
    let assignment = problem.initial_assignment();
    let pillars = GroupedSelectionSource::new(
        Entities,
        4,
        |a: &Assignment<i32>, &left: &usize, &right: &usize| a.get(left) == a.get(right),
    );
    let mut selector = KeyedSelectionSource::new(
        pillars,
        Values,
        KeyedSelectionLimits {
            maximum_keys: 4,
            maximum_values_per_key: 3,
            maximum_values: 12,
        },
    );
    for event in [
        SelectionLifecycleEvent::SolveStarted,
        SelectionLifecycleEvent::PhaseStarted,
        SelectionLifecycleEvent::StepStarted,
    ] {
        selector.event(event, &assignment);
    }
    let mut iter = selector.open(&assignment);
    for expected in ["[a, b, c, d]->1", "[a, b, c, d]->2", "[a, b, c, d]->3"] {
        let (group, value) = iter.next().unwrap();
        let slots = group.iter().map(|i| **i).collect::<Vec<_>>();
        let pillar = Pillar::new(slots.clone()).unwrap();
        let movement = pillar.change_move(&assignment, &[Some(*value)]).unwrap();
        let Move::Composite(changes) = movement else {
            panic!("expected pillar changes")
        };
        let mut names = vec![];
        let mut selected = None;
        for change in changes {
            let Move::Change { variable, value } = change else {
                panic!("expected change")
            };
            names.push(["a", "b", "c", "d"][variable]);
            let value = value.unwrap();
            if let Some(previous) = selected {
                assert_eq!(previous, value);
            }
            selected = Some(value);
        }
        assert_eq!(
            format!("[{}]->{}", names.join(", "), selected.unwrap()),
            expected
        );
    }
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    drop(iter);
    assert!(!selector.is_never_ending());
    assert_eq!(selector.size(&assignment), Some(3));
}
