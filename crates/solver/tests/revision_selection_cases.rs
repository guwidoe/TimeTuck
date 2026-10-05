// Adapted from pinned Community FromSolutionEntitySelectorTest and PlannerAssert
// helpers (Apache-2.0). Explicit revisions replace Java score-director dirty flags.
use SelectionLifecycleEvent::*;
use timetuck::*;
struct Context {
    revision: u64,
    values: Vec<&'static str>,
}
#[derive(Default)]
struct Source {
    opens: usize,
}
impl SelectionSource<Context, &'static str> for Source {
    fn size(&mut self, c: &Context) -> Option<usize> {
        Some(c.values.len())
    }
    fn open<'a>(&'a mut self, c: &'a Context) -> Box<dyn Iterator<Item = &'static str> + 'a>
    where
        &'static str: 'a,
    {
        self.opens += 1;
        Box::new(c.values.iter().copied())
    }
}
fn run(policy: RevisionRefresh, random: bool, dirty: bool) {
    let mut c = Context {
        revision: 7,
        values: vec!["e1", "e2", "e3"],
    };
    let mut cache = RevisionedSelection::new(
        Source::default(),
        policy,
        if random {
            CachedSelectionOrder::Random
        } else {
            CachedSelectionOrder::Original
        },
        3,
        |c: &Context| c.revision,
    )
    .unwrap();
    let mut draws = [1, 0, 0, 2, 1, 2, 2, 1, 0].into_iter();
    let expected_draws = [vec![1, 0, 0, 2], vec![1, 2], vec![2], vec![1], vec![0]];
    let mut step_index = 0;
    cache.event(SolveStarted, &c).unwrap();
    for (phase, steps) in [2, 3].into_iter().enumerate() {
        cache.event(PhaseStarted, &c).unwrap();
        for step in 0..steps {
            if dirty && phase == 0 && step == 1 {
                c.revision = 8;
                c.values = vec!["f1", "f2", "f3"];
            }
            if dirty && phase == 1 && step == 2 {
                c.revision = 9;
                c.values = vec!["e1", "e2", "e3"];
            }
            cache.event(StepStarted, &c).unwrap();
            assert_eq!(cache.size().unwrap(), 3);
            assert_eq!(cache.is_never_ending(), random);
            let mut cursor = cache
                .iter_with(|| (draws.next().unwrap() as f64 + 0.25) / 3.0)
                .unwrap();
            if random {
                let expected = expected_draws[step_index]
                    .iter()
                    .map(|&i| c.values[i])
                    .collect::<Vec<_>>();
                let actual = cursor
                    .by_ref()
                    .take(expected.len())
                    .map(|v| *v.unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(actual, expected);
                assert!(cursor.has_next());
            } else {
                assert_eq!(
                    cursor.by_ref().map(|v| *v.unwrap()).collect::<Vec<_>>(),
                    c.values
                );
                assert_eq!(cursor.next(), None);
                assert!(!cursor.has_next());
                assert_eq!(cursor.next(), None);
            }
            cache.event(StepEnded, &c).unwrap();
            step_index += 1;
        }
        cache.event(PhaseEnded, &c).unwrap();
    }
    cache.event(SolveEnded, &c).unwrap();
    assert_eq!(cache.source().opens, if dirty { 4 } else { 2 });
}
#[test]
fn original_phase() {
    run(RevisionRefresh::Phase, false, false)
}
#[test]
fn original_step() {
    run(RevisionRefresh::Step, false, false)
}
#[test]
fn original_just_in_time() {
    run(RevisionRefresh::Step, false, false)
}
#[test]
fn original_dirty() {
    run(RevisionRefresh::Step, false, true)
}
#[test]
fn random_phase() {
    run(RevisionRefresh::Phase, true, false)
}
#[test]
fn random_step() {
    run(RevisionRefresh::Step, true, false)
}
#[test]
fn random_just_in_time() {
    run(RevisionRefresh::Step, true, false)
}
#[test]
fn random_dirty() {
    run(RevisionRefresh::Step, true, true)
}
#[test]
fn random_list_iterator_rejected() {
    let c = RevisionedSelection::new(
        Source::default(),
        RevisionRefresh::Step,
        CachedSelectionOrder::Random,
        3,
        |c: &Context| c.revision,
    )
    .unwrap();
    assert!(matches!(
        c.list_iter(),
        Err(SelectionCacheError::RandomListAccess)
    ));
}
#[test]
fn random_indexed_list_iterator_rejected() {
    let c = RevisionedSelection::new(
        Source::default(),
        RevisionRefresh::Step,
        CachedSelectionOrder::Random,
        3,
        |c: &Context| c.revision,
    )
    .unwrap();
    assert!(matches!(
        c.list_iter_from(0),
        Err(SelectionCacheError::RandomListAccess)
    ));
}
