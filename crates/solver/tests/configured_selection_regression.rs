//! Original executable pipeline regressions; no upstream credits.
use SelectionCacheScope::{Phase, Step, Uncached};
use SelectionLifecycleEvent::*;
use SelectionOrder::*;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
struct Rows {
    values: Vec<i32>,
    opens: Vec<i32>,
    events: Vec<SelectionLifecycleEvent>,
    repeating: bool,
}
impl Rows {
    fn new(values: Vec<i32>) -> Self {
        Self {
            values,
            opens: vec![],
            events: vec![],
            repeating: false,
        }
    }
}
impl SelectionSource<i32, i32> for Rows {
    fn is_never_ending(&self) -> bool {
        self.repeating
    }
    fn event(&mut self, e: SelectionLifecycleEvent, _: &i32) {
        self.events.push(e);
    }
    fn size(&mut self, _: &i32) -> Option<usize> {
        Some(self.values.len())
    }
    fn open<'a>(&'a mut self, c: &'a i32) -> Box<dyn Iterator<Item = i32> + 'a>
    where
        i32: 'a,
    {
        self.opens.push(*c);
        Box::new(self.values.iter().copied())
    }
}
fn config(order: SelectionOrder) -> SelectionConfig {
    SelectionConfig {
        cache_scope: Some(Step),
        order: Some(order),
        ..Default::default()
    }
}
fn policy() -> SelectionPolicy {
    SelectionPolicy {
        source_minimum_scope: Phase,
        ..Default::default()
    }
}
fn phase(s: &mut ConfiguredSelection<i32, i32, Rows>, c: i32) {
    s.event(SolveStarted, &c).unwrap();
    s.event(PhaseStarted, &c).unwrap();
}
fn items(s: &mut ConfiguredSelection<i32, i32, Rows>, c: i32) -> Vec<i32> {
    s.iter(&c, 9).unwrap().take(10).collect()
}
#[test]
fn phase_membership_and_step_order_have_independent_lifetimes() {
    let mut s = SelectionBuilder::new(config(Shuffled))
        .build(policy(), |_| Rows::new(vec![1, 2, 3, 4, 5, 6]))
        .unwrap();
    phase(&mut s, 0);
    assert_eq!(s.source().opens, [0]);
    assert!(s.iter(&0, 0).is_err());
    let mut orders = std::collections::BTreeSet::new();
    for c in 1..=8 {
        s.event(StepStarted, &c).unwrap();
        let order = items(&mut s, c);
        assert_eq!(items(&mut s, c), order);
        let mut sorted = order.clone();
        sorted.sort();
        assert_eq!(sorted, [1, 2, 3, 4, 5, 6]);
        orders.insert(order);
        s.source_mut().values = vec![99];
        s.event(StepEnded, &c).unwrap();
        assert!(s.iter(&c, 0).is_err());
    }
    assert!(orders.len() > 1);
    assert_eq!(s.source().opens, [0]);
    s.event(PhaseEnded, &8).unwrap();
    s.event(PhaseStarted, &9).unwrap();
    s.event(StepStarted, &9).unwrap();
    assert_eq!(items(&mut s, 9), [99]);
    assert_eq!(s.source().opens, [0, 9]);
    s.event(StepEnded, &9).unwrap();
    s.event(PhaseEnded, &9).unwrap();
    s.event(SolveEnded, &9).unwrap();
    assert_eq!(s.source().events.len(), 24);
}
#[test]
fn comparator_factory_rebuilds_from_context_and_stable_sort_preserves_ties() {
    let builds = Rc::new(Cell::new(0));
    let count = builds.clone();
    let mut cfg = config(Sorted);
    cfg.sort_order = Some(SelectionSortOrder::Descending);
    let mut s = SelectionBuilder::new(cfg)
        .with_comparator_factory(move |c: &i32| {
            count.set(count.get() + 1);
            let divisor = *c;
            move |a: &i32, b: &i32| (a / divisor).cmp(&(b / divisor))
        })
        .build(policy(), |_| Rows::new(vec![21, 11, 22, 12]))
        .unwrap();
    phase(&mut s, 0);
    assert_eq!(builds.get(), 0);
    s.event(StepStarted, &10).unwrap();
    assert_eq!(items(&mut s, 10), [21, 22, 11, 12]);
    assert_eq!(builds.get(), 1);
    assert_eq!(items(&mut s, 10), [21, 22, 11, 12]);
    assert_eq!(builds.get(), 1);
    s.event(StepEnded, &10).unwrap();
    s.event(StepStarted, &1).unwrap();
    assert_eq!(items(&mut s, 1), [22, 21, 12, 11]);
    assert_eq!(builds.get(), 2);
    assert_eq!(s.source().opens, [0]);
    s.event(StepEnded, &1).unwrap();
    s.event(PhaseEnded, &1).unwrap();
    s.source_mut().values.clear();
    s.event(PhaseStarted, &1).unwrap();
    s.event(StepStarted, &1).unwrap();
    assert!(items(&mut s, 1).is_empty());
    assert_eq!(builds.get(), 3);
}
#[test]
fn probabilities_refresh_at_order_scope_and_bad_weights_leave_no_stale_output() {
    let mut s = SelectionBuilder::new(config(Probabilistic))
        .with_weight(|c: &i32, v: &i32| {
            if *c < 0 {
                f64::NAN
            } else if c == v {
                1.0
            } else {
                0.0
            }
        })
        .build(policy(), |_| Rows::new(vec![1, 2]))
        .unwrap();
    phase(&mut s, 0);
    for c in [1, 2, 0, -1, 1] {
        let result = s.event(StepStarted, &c);
        if c < 0 {
            assert!(matches!(result, Err(SelectionCacheError::Weights(_))));
            assert!(s.iter(&c, 0).is_err());
        } else {
            result.unwrap();
            assert_eq!(items(&mut s, c), if c == 0 { vec![] } else { vec![c; 10] });
        }
        s.event(StepEnded, &c).unwrap();
    }
    assert_eq!(s.source().opens, [0]);
}
#[test]
fn configuration_errors_precede_factory_and_scope_inheritance_is_explicit() {
    let mut cfg = config(Random);
    cfg.cache_scope = Some(Uncached);
    let b = SelectionBuilder::<i32, i32>::new(cfg).with_weight(|_, _| 1.0);
    assert_eq!(
        b.resolve(policy()),
        Err(SelectionBuildError::WeightingRequiresProbabilisticOrder)
    );
    let b = SelectionBuilder::<i32, i32>::new(config(Probabilistic));
    assert_eq!(b.resolve(policy()), Err(SelectionBuildError::MissingWeight));
    let b = SelectionBuilder::<i32, i32>::new(config(Sorted));
    assert!(matches!(
        b.build(policy(), |_| -> Rows { panic!("factory must not run") }),
        Err(SelectionBuildError::MissingComparator)
    ));
    let b = SelectionBuilder::<i32, i32>::new(SelectionConfig::default());
    let p = b
        .resolve(SelectionPolicy {
            minimum_scope: Phase,
            inherited_order: Original,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(p.cache_scope, Phase);
    assert_eq!(p.order, Original);
    assert!(!p.source.random);
    let mut s = SelectionBuilder::new(cfg)
        .build(SelectionPolicy::default(), |r| {
            assert!(r.random);
            Rows::new(vec![1])
        })
        .unwrap();
    assert!(s.iter(&0, 0).is_err());
    phase(&mut s, 0);
    s.event(StepStarted, &0).unwrap();
    assert_eq!(items(&mut s, 0), [1]);
    assert_eq!(items(&mut s, 0), [1]);
    assert_eq!(s.source().opens, [0, 0]);
    let before = s.source().events.clone();
    assert!(s.event(StepStarted, &0).is_err());
    assert_eq!(s.source().events, before);
    let result = SelectionBuilder::new(config(Original)).build(policy(), |_| {
        let mut r = Rows::new(vec![1]);
        r.repeating = true;
        r
    });
    assert!(matches!(
        result,
        Err(SelectionBuildError::Cache(
            SelectionCacheError::UnboundedSource
        ))
    ));
}
#[test]
fn bounded_builds_and_comparator_panics_can_recover_without_republishing_old_order() {
    let mut cfg = config(Original);
    cfg.maximum_candidates = 1;
    let mut s = SelectionBuilder::new(cfg)
        .build(policy(), |_| Rows::new(vec![1, 2]))
        .unwrap();
    s.event(SolveStarted, &0).unwrap();
    assert!(matches!(
        s.event(PhaseStarted, &0),
        Err(SelectionCacheError::Weights(
            WeightedSelectionError::CandidateLimit { maximum: 1 }
        ))
    ));
    assert!(s.iter(&0, 0).is_err());
    s.event(PhaseEnded, &0).unwrap();
    s.source_mut().values = vec![7];
    s.event(PhaseStarted, &0).unwrap();
    s.event(StepStarted, &0).unwrap();
    assert_eq!(items(&mut s, 0), [7]);
    let fail = Rc::new(Cell::new(false));
    let f = fail.clone();
    let mut s = SelectionBuilder::new(config(Sorted))
        .with_comparator(move |_, a: &i32, b: &i32| {
            assert!(!f.get(), "comparator fault");
            a.cmp(b)
        })
        .build(policy(), |_| Rows::new(vec![2, 1]))
        .unwrap();
    phase(&mut s, 0);
    s.event(StepStarted, &0).unwrap();
    assert_eq!(items(&mut s, 0), [1, 2]);
    s.event(StepEnded, &0).unwrap();
    fail.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| s.event(StepStarted, &0))).is_err());
    assert_eq!(s.state(), SelectionCacheState::Step);
    assert!(s.iter(&0, 0).is_err());
    s.event(StepEnded, &0).unwrap();
    fail.set(false);
    s.event(StepStarted, &0).unwrap();
    assert_eq!(items(&mut s, 0), [1, 2]);
    let before = s.source().events.clone();
    assert!(s.event(SolveEnded, &0).is_err());
    assert_eq!(s.source().events, before);
    assert_eq!(items(&mut s, 0), [1, 2]);
}
#[test]
fn configured_order_drives_real_solver_and_closes_both_cache_layers() {
    let source = SelectionBuilder::new(config(Sorted))
        .with_comparator(|_, a: &i32, b: &i32| b.cmp(a))
        .build(policy(), |_| Rows::new(vec![1, 2]))
        .unwrap();
    let mut provider = CachedMoveProvider::uniform(
        source,
        CachedProviderConfig::default(),
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, step: u64| step as i32,
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64, v: &i32| {
            Some(Move::Change {
                variable: 0,
                value: Some(*v),
            })
        },
    )
    .unwrap();
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::<_, SimpleScore>::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(i64::from(*v[0].unwrap()))
    })
    .unwrap();
    let mut ext = SolverExtensions::default();
    ext.move_providers.bind_shared([0], &mut provider).unwrap();
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            limits: SolverLimits::default(),
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 1,
                acceptance: Acceptance::HillClimbing,
                ..Default::default()
            })],
        },
        ext,
    )
    .unwrap();
    assert!(r.complete);
    assert_eq!(r.work.completed_steps, 1);
    assert_eq!(r.work.move_evaluations, 1);
    assert_eq!(r.assignment.values(), &[Some(2)]);
    assert_eq!(r.score, Some(SimpleScore::new(2)));
    assert_eq!(provider.cache().source().state(), SelectionCacheState::Idle);
    assert_eq!(provider.cache().source().source().opens, [0]);
    assert_eq!(
        provider.cache().source().source().events,
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

#[test]
fn cache_and_order_builds_do_not_clone_payloads_and_iteration_clones_only_yielded_items() {
    struct Item(i32, Rc<Cell<usize>>);
    impl Clone for Item {
        fn clone(&self) -> Self {
            self.1.set(self.1.get() + 1);
            Self(self.0, self.1.clone())
        }
    }
    struct Items {
        rows: Vec<Item>,
    }
    impl SelectionSource<(), Item> for Items {
        fn size(&mut self, _: &()) -> Option<usize> {
            Some(self.rows.len())
        }
        fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = Item> + 'a>
        where
            Item: 'a,
        {
            Box::new(self.rows.drain(..))
        }
    }
    let clones = Rc::new(Cell::new(0));
    let mut s = SelectionBuilder::new(config(Sorted))
        .with_comparator(|_: &(), a: &Item, b: &Item| a.0.cmp(&b.0))
        .build(SelectionPolicy::default(), |_| Items {
            rows: vec![Item(2, clones.clone()), Item(1, clones.clone())],
        })
        .unwrap();
    for e in [SolveStarted, PhaseStarted, StepStarted] {
        s.event(e, &()).unwrap();
    }
    assert_eq!(clones.get(), 0);
    assert_eq!(s.iter(&(), 0).unwrap().next().unwrap().0, 1);
    assert_eq!(clones.get(), 1);
}
