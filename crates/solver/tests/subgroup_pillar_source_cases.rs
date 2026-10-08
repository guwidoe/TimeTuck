// Adapted from DefaultPillarSelectorTest and PlannerAssert (Community, Apache-2.0).
// Explicit bounded index draws replace Java Random and typed slots replace entities.
use SelectionLifecycleEvent as E;
use timetuck::*;
struct Entities {
    ids: Vec<usize>,
    events: Vec<E>,
}
impl SelectionSource<Vec<i32>, usize> for Entities {
    fn event(&mut self, e: E, _: &Vec<i32>) {
        self.events.push(e);
    }
    fn size(&mut self, _: &Vec<i32>) -> Option<usize> {
        Some(self.ids.len())
    }
    fn open<'a>(&'a mut self, _: &'a Vec<i32>) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        Box::new(self.ids.iter().copied())
    }
}
fn codes<T>(
    s: &mut SubgroupSelectionSource<Vec<i32>, usize, T>,
    v: &Vec<i32>,
    draws: &[usize],
    expected: &[&str],
) where
    T: SelectionSource<Vec<i32>, SelectionGroup<usize>>,
{
    let mut draws = draws.iter().copied();
    let mut iter = s
        .try_open_with(v, |_| draws.next().expect("script exhausted"))
        .unwrap();
    for code in expected {
        let group = iter.next().unwrap().unwrap();
        let actual = format!(
            "[{}]",
            group
                .iter()
                .map(|v| ["a", "b", "c", "d", "e", "f"][**v])
                .collect::<Vec<_>>()
                .join(", ")
        );
        assert_eq!(&actual, code);
    }
    assert!(iter.next().unwrap().is_ok()); // Java hasNext on a nonempty random selector.
    drop(iter);
    assert!(s.is_never_ending());
}
fn lifecycle(events: &[E], phases: usize, steps: usize) {
    for (e, count) in [
        (E::SolveStarted, 1),
        (E::SolveEnded, 1),
        (E::PhaseStarted, phases),
        (E::PhaseEnded, phases),
        (E::StepStarted, steps),
        (E::StepEnded, steps),
    ] {
        assert_eq!(events.iter().filter(|&&actual| actual == e).count(), count);
    }
}
#[test]
fn random_with_subs() {
    let mut v = vec![1, 2, 3, 2, 3, 3];
    let groups = GroupedSelectionSource::new(
        Entities {
            ids: (0..6).collect(),
            events: vec![],
        },
        6,
        |v: &Vec<i32>, &a: &usize, &b: &usize| v[a] == v[b],
    );
    let mut s = SubgroupSelectionSource::new(
        groups,
        6,
        6,
        SubPillarSelection::combinations(1, usize::MAX).unwrap(),
        0,
    );
    s.event(E::SolveStarted, &v);
    for (phase, steps) in [2, 1].into_iter().enumerate() {
        s.event(E::PhaseStarted, &v);
        for step in 0..steps {
            if phase == 0 && step == 1 {
                v[1] = 3;
                v[5] = 4;
            }
            s.event(E::StepStarted, &v);
            if phase == 0 && step == 0 {
                codes(
                    &mut s,
                    &v,
                    &[0, 2, 1, 0, 0, 1, 0, 0, 1, 0, 1, 1, 1],
                    &["[a]", "[c, e]", "[b]", "[d]"],
                );
            } else {
                codes(
                    &mut s,
                    &v,
                    &[3, 1, 2, 1, 0, 0, 2, 0, 0],
                    &["[f]", "[b, c, e]", "[b]", "[d]"],
                );
            }
            s.event(E::StepEnded, &v);
        }
        s.event(E::PhaseEnded, &v);
    }
    s.event(E::SolveEnded, &v);
    lifecycle(&s.source().source().events, 2, 3);
}
#[test]
fn random_with_subs_size_2() {
    let v = vec![1, 2, 3, 2, 3, 3];
    let groups = GroupedSelectionSource::new(
        Entities {
            ids: (0..6).collect(),
            events: vec![],
        },
        6,
        |v: &Vec<i32>, &a: &usize, &b: &usize| v[a] == v[b],
    );
    let mut s = SubgroupSelectionSource::new(
        groups,
        6,
        6,
        SubPillarSelection::combinations(2, 2).unwrap(),
        0,
    );
    for e in [E::SolveStarted, E::PhaseStarted, E::StepStarted] {
        s.event(e, &v);
    }
    codes(&mut s, &v, &[1, 0, 0, 0, 0, 0, 0, 0], &["[c, e]", "[b, d]"]);
    for e in [E::StepEnded, E::PhaseEnded, E::SolveEnded] {
        s.event(e, &v);
    }
    lifecycle(&s.source().source().events, 1, 1);
}
#[test]
fn sequential_unlimited() {
    let v = vec![1, 2, 3, 2, 3, 3];
    let groups = GroupedSelectionSource::new(
        Entities {
            ids: vec![0, 5, 3, 4, 2, 1],
            events: vec![],
        },
        6,
        |v: &Vec<i32>, &a: &usize, &b: &usize| v[a] == v[b],
    );
    let comparator: fn(&Vec<i32>, &usize, &usize) -> std::cmp::Ordering = |_, a, b| a.cmp(b);
    let mut s = SubgroupSelectionSource::new(
        groups,
        6,
        6,
        SubPillarSelection::consecutive(1, usize::MAX).unwrap(),
        0,
    )
    .ordered_by(comparator);
    for e in [E::SolveStarted, E::PhaseStarted, E::StepStarted] {
        s.event(e, &v);
    }
    codes(
        &mut s,
        &v,
        &[1, 1, 2, 2, 2, 1, 1, 0, 0],
        &["[b, d]", "[c, e, f]", "[e, f]", "[a]"],
    );
    for e in [E::StepEnded, E::PhaseEnded, E::SolveEnded] {
        s.event(e, &v);
    }
    lifecycle(&s.source().source().events, 1, 1);
}
#[test]
fn empty_entity_selector_random_with_subs() {
    let v = vec![];
    let groups = GroupedSelectionSource::new(
        Entities {
            ids: vec![],
            events: vec![],
        },
        6,
        |v: &Vec<i32>, &a: &usize, &b: &usize| v[a] == v[b],
    );
    let mut s = SubgroupSelectionSource::new(
        groups,
        6,
        6,
        SubPillarSelection::combinations(1, usize::MAX).unwrap(),
        0,
    );
    s.event(E::SolveStarted, &v);
    for steps in [2, 1] {
        s.event(E::PhaseStarted, &v);
        for _ in 0..steps {
            s.event(E::StepStarted, &v);
            assert!(s.open(&v).next().is_none());
            assert!(s.is_never_ending());
            s.event(E::StepEnded, &v);
        }
        s.event(E::PhaseEnded, &v);
    }
    s.event(E::SolveEnded, &v);
    lifecycle(&s.source().source().events, 2, 3);
}
