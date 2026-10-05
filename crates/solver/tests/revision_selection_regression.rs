use SelectionLifecycleEvent::*;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
struct Context {
    revision: u64,
    values: Vec<i32>,
    declared: Option<usize>,
    panic: bool,
    repeat: bool,
}
fn context(revision: u64, values: Vec<i32>) -> Context {
    Context {
        revision,
        values,
        declared: None,
        panic: false,
        repeat: false,
    }
}
#[derive(Default)]
struct Source {
    opens: usize,
    events: usize,
    unbounded: bool,
    polls: Rc<Cell<usize>>,
}
impl SelectionSource<Context, i32> for Source {
    fn is_never_ending(&self) -> bool {
        self.unbounded
    }
    fn event(&mut self, _: SelectionLifecycleEvent, _: &Context) {
        self.events += 1;
    }
    fn size(&mut self, c: &Context) -> Option<usize> {
        c.declared
    }
    fn open<'a>(&'a mut self, c: &'a Context) -> Box<dyn Iterator<Item = i32> + 'a>
    where
        i32: 'a,
    {
        self.opens += 1;
        assert!(!c.panic, "source panic");
        let p = self.polls.clone();
        let it = c.values.iter().copied();
        if c.repeat {
            Box::new(it.cycle().inspect(move |_| p.set(p.get() + 1)))
        } else {
            Box::new(it.inspect(move |_| p.set(p.get() + 1)))
        }
    }
}
#[test]
fn revisions_refresh_at_selected_boundaries_and_phase_restart_always_extracts() {
    for policy in [RevisionRefresh::Phase, RevisionRefresh::Step] {
        let mut c = context(1, vec![1, 2]);
        let mut s = RevisionedSelection::new(
            Source::default(),
            policy,
            CachedSelectionOrder::Original,
            5,
            |c: &Context| c.revision,
        )
        .unwrap();
        s.event(SolveStarted, &c).unwrap();
        s.event(PhaseStarted, &c).unwrap();
        let held = s.snapshot().unwrap();
        c.values = vec![8];
        s.event(StepStarted, &c).unwrap();
        assert_eq!(s.snapshot().unwrap().items(), [1, 2]);
        s.event(StepEnded, &c).unwrap();
        c.revision = 2;
        s.event(StepStarted, &c).unwrap();
        assert_eq!(
            s.snapshot().unwrap().items(),
            if policy == RevisionRefresh::Step {
                &[8][..]
            } else {
                &[1, 2][..]
            }
        );
        s.event(StepEnded, &c).unwrap();
        s.event(PhaseEnded, &c).unwrap();
        assert!(s.snapshot().is_err());
        assert_eq!(held.items(), [1, 2]);
        s.event(PhaseStarted, &c).unwrap();
        assert_eq!(s.snapshot().unwrap().items(), [8]);
        assert_eq!(
            s.source().opens,
            if policy == RevisionRefresh::Step {
                3
            } else {
                2
            }
        );
        assert_eq!(s.revision(), Some(&2));
    }
}
#[test]
fn failed_and_panicking_builds_withdraw_old_data_and_recover_next_step() {
    let mut c = context(1, vec![1]);
    c.declared = Some(2);
    let mut s = RevisionedSelection::new(
        Source::default(),
        RevisionRefresh::Step,
        CachedSelectionOrder::Original,
        3,
        |c: &Context| c.revision,
    )
    .unwrap();
    s.event(SolveStarted, &c).unwrap();
    assert!(matches!(
        s.event(PhaseStarted, &c),
        Err(SelectionCacheError::SizeMismatch {
            declared: 2,
            actual: 1
        })
    ));
    assert!(s.snapshot().is_err());
    assert_eq!(s.revision(), None);
    c.declared = None;
    s.event(StepStarted, &c).unwrap();
    let old = s.snapshot().unwrap();
    s.event(StepEnded, &c).unwrap();
    c.revision = 2;
    c.repeat = true;
    let before = s.source().polls.get();
    assert!(matches!(
        s.event(StepStarted, &c),
        Err(SelectionCacheError::Weights(
            WeightedSelectionError::CandidateLimit { maximum: 3 }
        ))
    ));
    assert_eq!(s.source().polls.get() - before, 4);
    assert!(s.snapshot().is_err());
    s.event(StepEnded, &c).unwrap();
    c.repeat = false;
    c.panic = true;
    assert!(catch_unwind(AssertUnwindSafe(|| s.event(StepStarted, &c))).is_err());
    assert!(s.snapshot().is_err());
    s.event(StepEnded, &c).unwrap();
    c.panic = false;
    c.values = vec![9];
    s.event(StepStarted, &c).unwrap();
    assert_eq!(s.snapshot().unwrap().items(), [9]);
    assert_eq!(old.items(), [1]);
    let events = s.source().events;
    assert!(matches!(
        s.event(PhaseStarted, &c),
        Err(SelectionCacheError::InvalidTransition { .. })
    ));
    assert_eq!(s.source().events, events);
    assert_eq!(s.snapshot().unwrap().items(), [9]);
}
#[test]
fn random_availability_is_passive_and_bad_draw_fuses_only_its_cursor() {
    let c = context(7, vec![1, 2, 3]);
    let mut s = RevisionedSelection::new(
        Source::default(),
        RevisionRefresh::Step,
        CachedSelectionOrder::Random,
        3,
        |c: &Context| c.revision,
    )
    .unwrap();
    s.event(SolveStarted, &c).unwrap();
    s.event(PhaseStarted, &c).unwrap();
    let calls = Cell::new(0);
    let mut cursor = s
        .iter_with(|| {
            calls.set(calls.get() + 1);
            1.0
        })
        .unwrap();
    assert!(cursor.has_next());
    assert!(cursor.has_next());
    assert_eq!(calls.get(), 0);
    assert_eq!(
        cursor.next(),
        Some(Err(WeightedSelectionError::InvalidDraw))
    );
    assert!(!cursor.has_next());
    assert_eq!(cursor.next(), None);
    assert_eq!(calls.get(), 1);
    let a = s
        .iter(77)
        .unwrap()
        .take(30)
        .map(|v| *v.unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        a,
        s.iter(77)
            .unwrap()
            .take(30)
            .map(|v| *v.unwrap())
            .collect::<Vec<_>>()
    );
    assert_eq!(s.size().unwrap(), 3);
}
#[test]
fn empty_bounds_and_declared_unbounded_sources_have_explicit_outcomes() {
    assert!(matches!(
        RevisionedSelection::new(
            Source {
                unbounded: true,
                ..Default::default()
            },
            RevisionRefresh::Step,
            CachedSelectionOrder::Original,
            0,
            |c: &Context| c.revision
        ),
        Err(SelectionCacheError::UnboundedSource)
    ));
    let c = context(1, vec![]);
    for order in [CachedSelectionOrder::Original, CachedSelectionOrder::Random] {
        let mut s = RevisionedSelection::new(
            Source::default(),
            RevisionRefresh::Step,
            order,
            0,
            |c: &Context| c.revision,
        )
        .unwrap();
        s.event(SolveStarted, &c).unwrap();
        s.event(PhaseStarted, &c).unwrap();
        assert_eq!(s.size().unwrap(), 0);
        let mut i = s.iter_with(|| panic!("empty draw")).unwrap();
        assert!(!i.has_next());
        assert_eq!(i.next(), None);
        assert_eq!(s.is_never_ending(), order == CachedSelectionOrder::Random);
    }
}
#[test]
fn nonclone_occurrences_and_indexed_views_survive_refresh_and_release() {
    struct Item(i32);
    struct Factory;
    impl SelectionSource<Context, Item> for Factory {
        fn size(&mut self, c: &Context) -> Option<usize> {
            Some(c.values.len())
        }
        fn open<'a>(&'a mut self, c: &'a Context) -> Box<dyn Iterator<Item = Item> + 'a>
        where
            Item: 'a,
        {
            Box::new(c.values.iter().map(|v| Item(*v)))
        }
    }
    let mut c = context(1, vec![4, 4, 7]);
    let mut s = RevisionedSelection::new(
        Factory,
        RevisionRefresh::Step,
        CachedSelectionOrder::Original,
        3,
        |c: &Context| c.revision,
    )
    .unwrap();
    s.event(SolveStarted, &c).unwrap();
    s.event(PhaseStarted, &c).unwrap();
    let held = s.snapshot().unwrap();
    assert_eq!(
        s.list_iter_from(1)
            .unwrap()
            .rev()
            .map(|v| v.0)
            .collect::<Vec<_>>(),
        [7, 4]
    );
    assert!(s.list_iter_from(3).unwrap().next().is_none());
    assert!(matches!(
        s.list_iter_from(4),
        Err(SelectionCacheError::IndexOutOfBounds { index: 4, len: 3 })
    ));
    c.revision = 2;
    c.values = vec![9];
    s.event(StepStarted, &c).unwrap();
    assert_eq!(s.list_iter().unwrap().map(|v| v.0).collect::<Vec<_>>(), [9]);
    s.event(StepEnded, &c).unwrap();
    s.event(PhaseEnded, &c).unwrap();
    s.event(SolveEnded, &c).unwrap();
    drop(s);
    assert_eq!(
        held.items().iter().map(|v| v.0).collect::<Vec<_>>(),
        [4, 4, 7]
    );
}
#[test]
fn revisioned_candidate_pools_feed_current_assignment_search_in_both_backends() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2, 3]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(i64::from(*v[0].unwrap()))
    })
    .unwrap();
    for stream in [false, true] {
        let c = context(0, vec![1, 2, 3]);
        let mut s = RevisionedSelection::new(
            Source::default(),
            RevisionRefresh::Step,
            CachedSelectionOrder::Original,
            3,
            |c: &Context| c.revision,
        )
        .unwrap();
        s.event(SolveStarted, &c).unwrap();
        s.event(PhaseStarted, &c).unwrap();
        let mut provider = |_: &FiniteProblem<i32>, a: &Assignment<i32>, step: u64| {
            let c = context(
                step,
                (0..=3).filter(|v| Some(*v) != a.values()[0]).collect(),
            );
            s.event(StepStarted, &c).unwrap();
            let pool = s.snapshot().unwrap();
            s.event(StepEnded, &c).unwrap();
            pool.items()
                .iter()
                .map(|v| Move::Change {
                    variable: 0,
                    value: Some(*v),
                })
                .collect::<Vec<_>>()
        };
        let config = LocalSearchConfig {
            max_steps: 2,
            selection: MoveSelection::BestAccepted,
            assert_scores: true,
            ..Default::default()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("value", source.port(), |r| {
                SimpleScore::new(i64::from(r.1.unwrap()))
            })
            .unwrap();
            stream_local_search_from_with_provider(
                &p,
                b.build(),
                source,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut provider)
                .unwrap()
        };
        assert_eq!(result.best.unwrap().score, SimpleScore::new(3));
        assert_eq!(s.source().opens, 2);
        s.event(PhaseEnded, &c).unwrap();
        s.event(SolveEnded, &c).unwrap();
    }
}
