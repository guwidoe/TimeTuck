//! Native lease ownership, failure recovery and real solver integration.
use SelectionLifecycleEvent as E;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
use timetuck::*;

struct Payload(Rc<Cell<usize>>);
impl Drop for Payload {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
#[test]
fn snapshots_outlive_demands_and_pool_without_preventing_rebuilds() {
    let pool = SharedSelectionPool::default();
    let drops = Rc::new(Cell::new(0));
    let first = pool
        .try_acquire(1, || Ok::<_, ()>(Payload(drops.clone())))
        .unwrap();
    let second = pool
        .try_acquire(1, || -> Result<Payload, ()> { panic!("must reuse") })
        .unwrap();
    let third = second.clone();
    assert_eq!(pool.active_count(&1), 3);
    let held = first.snapshot();
    assert!(Arc::ptr_eq(&held, &second.snapshot()));
    drop((first, second, third));
    assert!(pool.is_empty());
    assert_eq!(drops.get(), 0);
    let rebuilt = pool
        .try_acquire(1, || Ok::<_, ()>(Payload(drops.clone())))
        .unwrap();
    let other = pool
        .try_acquire(2, || Ok::<_, ()>(Payload(drops.clone())))
        .unwrap();
    assert!(!Arc::ptr_eq(&held, &rebuilt.snapshot()));
    assert_eq!(pool.len(), 2);
    drop(pool);
    assert_eq!(drops.get(), 0);
    drop((rebuilt, other));
    assert_eq!(drops.get(), 2);
    drop(held);
    assert_eq!(drops.get(), 3);
}
#[test]
fn builders_can_fail_panic_and_reenter_without_partial_publication() {
    let pool = SharedSelectionPool::<u8, i32>::default();
    assert!(pool.try_acquire(0, || Err("failed")).is_err());
    assert!(pool.is_empty());
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pool.try_acquire(0, || -> Result<i32, ()> { panic!("builder") })
        }))
        .is_err()
    );
    assert!(pool.is_empty());
    let mut inner = None;
    let outer = pool
        .try_acquire(0, || {
            inner = Some(pool.try_acquire(0, || Ok::<_, ()>(7)).unwrap());
            Ok::<_, ()>(9)
        })
        .unwrap();
    assert_eq!(*outer.value(), 7);
    assert_eq!(pool.active_count(&0), 2);
    assert!(Arc::ptr_eq(
        &outer.snapshot(),
        &inner.as_ref().unwrap().snapshot()
    ));
    drop((outer, inner));
    assert!(pool.is_empty());
}
struct Source {
    opens: Rc<Cell<usize>>,
    events: Vec<E>,
    declared: Option<usize>,
}
impl Source {
    fn new(opens: Rc<Cell<usize>>) -> Self {
        Self {
            opens,
            events: vec![],
            declared: Some(2),
        }
    }
}
impl<C> SelectionSource<C, usize> for Source {
    fn event(&mut self, e: E, _: &C) {
        self.events.push(e);
    }
    fn size(&mut self, _: &C) -> Option<usize> {
        self.declared
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        self.opens.set(self.opens.get() + 1);
        Box::new(0..2)
    }
}
#[test]
fn views_share_collection_but_keep_cursors_and_lifecycles_independent() {
    let pool = SharedSelectionPool::default();
    let opens = Rc::new(Cell::new(0));
    let make = || {
        SharedStepSelectionSource::new(
            pool.clone(),
            Source::new(opens.clone()),
            2,
            |revision: &u8| *revision,
        )
    };
    let (mut a, mut b) = (make(), make());
    assert!(a.try_event(E::StepStarted, &0).is_err());
    assert_eq!(a.state(), SelectionCacheState::Idle);
    assert!(a.source().events.is_empty());
    for s in [&mut a, &mut b] {
        for e in [E::SolveStarted, E::PhaseStarted, E::StepStarted] {
            s.try_event(e, &0).unwrap();
        }
    }
    assert_eq!(opens.get(), 1);
    let held = a.snapshot().unwrap();
    let mut ai = a.open(&0);
    let mut bi = b.open(&0);
    let first = ai.next().unwrap();
    assert_eq!(*ai.next().unwrap(), 1);
    assert!(ai.next().is_none());
    assert!(Arc::ptr_eq(&first, &bi.next().unwrap()));
    drop((ai, bi));
    a.try_event(E::PhaseEnded, &0).unwrap();
    assert_eq!(pool.active_count(&0), 1);
    b.try_event(E::SolveEnded, &0).unwrap();
    assert!(pool.is_empty());
    a.try_event(E::SolveEnded, &0).unwrap();
    for s in [&a, &b] {
        assert_eq!(
            s.source().events,
            [
                E::SolveStarted,
                E::PhaseStarted,
                E::StepStarted,
                E::StepEnded,
                E::PhaseEnded,
                E::SolveEnded
            ]
        );
    }
    for s in [&mut a, &mut b] {
        for e in [E::SolveStarted, E::PhaseStarted] {
            s.try_event(e, &0).unwrap();
        }
    }
    a.try_event(E::StepStarted, &1).unwrap();
    b.try_event(E::StepStarted, &2).unwrap();
    assert_eq!(opens.get(), 3);
    assert_eq!(pool.len(), 2);
    assert_eq!(held.child_pool(0).unwrap().len(), 2);
    drop((a, b));
    assert!(pool.is_empty());
}
#[test]
fn failed_start_can_be_closed_and_retried() {
    let pool = SharedSelectionPool::default();
    let mut s = SharedStepSelectionSource::new(
        pool.clone(),
        Source::new(Rc::new(Cell::new(0))),
        2,
        |_: &()| 0,
    );
    s.source_mut().declared = Some(1);
    for e in [E::SolveStarted, E::PhaseStarted] {
        s.try_event(e, &()).unwrap();
    }
    assert!(matches!(
        s.try_event(E::StepStarted, &()),
        Err(SharedSelectionError::Collection(
            SelectionProductError::SizeMismatch { .. }
        ))
    ));
    assert_eq!(s.state(), SelectionCacheState::Step);
    assert!(s.snapshot().is_err());
    assert!(pool.is_empty());
    s.try_event(E::PhaseEnded, &()).unwrap();
    s.source_mut().declared = Some(2);
    for e in [E::PhaseStarted, E::StepStarted] {
        s.try_event(e, &()).unwrap();
    }
    assert_eq!(pool.active_count(&0), 1);
    s.try_event(E::SolveEnded, &()).unwrap();
    assert!(pool.is_empty());
}
#[test]
fn union_providers_share_one_collection_per_real_solver_scan() {
    let p = FiniteProblem::new(
        (0..2)
            .map(|_| Variable::new(vec![0, 1, 2]).with_initial(0))
            .collect(),
    )
    .unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("sum", vec![0, 1], |v| {
        SimpleScore::new(v.iter().map(|v| i64::from(v.copied().unwrap())).sum())
    })
    .unwrap();
    let opens = Rc::new(Cell::new(0));
    let pool = SharedSelectionPool::default();
    let seen = RefCell::new(vec![]);
    let make = |delta| {
        let seen = &seen;
        CachedMoveProvider::uniform(
            SharedStepSelectionSource::new(
                pool.clone(),
                Source::new(opens.clone()),
                2,
                |step: &u64| *step,
            ),
            CachedProviderConfig::default(),
            |_: &FiniteProblem<i32>, _: &Assignment<i32>, step: u64| step,
            move |_: &FiniteProblem<i32>, a: &Assignment<i32>, step: u64, id: &Arc<usize>| {
                seen.borrow_mut().push((step, delta, **id));
                let value = a.values()[**id].unwrap() + delta;
                (value <= 2).then_some(Move::Change {
                    variable: **id,
                    value: Some(value),
                })
            },
        )
    };
    let mut a = make(1).unwrap();
    let mut b = make(2).unwrap();
    let mut union = ProviderUnion::new(
        vec![(&mut a as &mut dyn MoveProvider<_, _, _>, 1), (&mut b, 1)],
        UnionOrder::Sequential,
    )
    .unwrap();
    let mut ext = SolverExtensions::default();
    ext.move_providers.bind(0, &mut union);
    let result = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 5,
                selection: MoveSelection::BestAccepted,
                acceptance: Acceptance::HillClimbing,
                assert_scores: true,
                ..Default::default()
            })],
            ..Default::default()
        },
        ext,
    )
    .unwrap();
    assert_eq!(result.assignment.values(), &[Some(2); 2]);
    assert_eq!(result.score, Some(SimpleScore::new(4)));
    assert_eq!(opens.get(), 3);
    assert!(pool.is_empty());
    assert_eq!(seen.borrow().len(), 12);
    for step in 0..3 {
        assert_eq!(
            seen.borrow().iter().filter(|&&(s, _, _)| s == step).count(),
            4
        );
    }
}
