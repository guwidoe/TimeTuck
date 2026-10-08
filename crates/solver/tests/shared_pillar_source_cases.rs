// Adapted from DefaultPillarSelectorTest (Timefold Community, Apache-2.0).
// Native shared snapshot leases replace Java SupplyManager demands.
use SelectionLifecycleEvent as E;
use timetuck::*;
struct Entities;
impl SelectionSource<Vec<i32>, usize> for Entities {
    fn size(&mut self, _: &Vec<i32>) -> Option<usize> {
        Some(6)
    }
    fn open<'a>(&'a mut self, _: &'a Vec<i32>) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        Box::new(0..6)
    }
}
fn exercise(skip_step_end: bool) {
    let values = vec![1, 2, 3, 2, 3, 3];
    let pool = SharedSelectionPool::default();
    let make = || {
        SharedStepSelectionSource::new(
            pool.clone(),
            GroupedSelectionSource::new(Entities, 6, |v: &Vec<i32>, &a: &usize, &b: &usize| {
                v[a] == v[b]
            }),
            6,
            |_: &Vec<i32>| 0u8,
        )
    };
    let (mut first, mut second) = (make(), make());
    for s in [&mut first, &mut second] {
        s.try_event(E::SolveStarted, &values).unwrap();
    }
    for s in [&mut first, &mut second] {
        s.try_event(E::PhaseStarted, &values).unwrap();
    }
    assert_eq!(pool.active_count(&0), 0);
    first.try_event(E::StepStarted, &values).unwrap();
    assert_eq!(pool.active_count(&0), 1);
    assert_eq!(pool.active_count(&0), 1);
    second.try_event(E::StepStarted, &values).unwrap();
    assert_eq!(pool.active_count(&0), 2);
    assert_eq!(pool.active_count(&0), 2);
    let end = if skip_step_end {
        E::PhaseEnded
    } else {
        E::StepEnded
    };
    first.try_event(end, &values).unwrap();
    assert_eq!(pool.active_count(&0), 1);
    assert_eq!(pool.active_count(&0), 1);
    second.try_event(end, &values).unwrap();
    assert_eq!(pool.active_count(&0), 0);
    if !skip_step_end {
        first.try_event(E::PhaseEnded, &values).unwrap();
        second.try_event(E::PhaseEnded, &values).unwrap();
    }
    first.try_event(E::SolveEnded, &values).unwrap();
    if skip_step_end {
        assert_eq!(pool.active_count(&0), 0);
    }
    second.try_event(E::SolveEnded, &values).unwrap();
    if skip_step_end {
        assert_eq!(pool.active_count(&0), 0);
    }
}
#[test]
fn reuse_pillar_cache() {
    exercise(false);
}
#[test]
fn step_cache_cleared_on_phase_ended() {
    exercise(true);
}
