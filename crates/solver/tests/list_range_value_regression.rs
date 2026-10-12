//! Original raw-union ordering, shared draws, lifecycle and actual list search.
use std::{
    cell::{Cell, RefCell},
    num::NonZeroUsize,
    rc::Rc,
    sync::Arc,
};
use timetuck::*;
fn ctx(p: ListProblem<i32>) -> (Arc<ListProblem<i32>>, ListAssignment<i32>) {
    let a = p.initial_assignment();
    (Arc::new(p), a)
}
fn source(order: ListRangeValueOrder) -> ListRangeValueSource {
    ListRangeValueSource::new(order)
}
#[test]
fn explicit_union_is_distinct_in_registry_order_and_omits_unreachable_values() {
    let c = ctx(ListProblem::new(
        vec![3, 2, 1, 99],
        vec![
            ListEntity::new(vec![3]).with_value_range(vec![1, 3, 2]),
            ListEntity::new(vec![2, 1]).with_value_range(vec![1, 2]),
        ],
    )
    .unwrap()
    .allowing_unassigned());
    let mut s = source(ListRangeValueOrder::Original);
    assert_eq!(s.size(&c), Some(3));
    assert_eq!(s.open(&c).collect::<Vec<_>>(), vec![3, 2, 1]);
    let mut draws = Vec::new();
    let out = ListRangeValueSource::select_with(&c, |n| {
        draws.push(n.get());
        0
    })
    .take(8)
    .collect::<Vec<_>>();
    assert_eq!(out, vec![3; 8]);
    assert_eq!(draws, vec![3; 8]);
}
#[test]
fn no_entities_or_empty_domains_never_call_the_draw_callback() {
    for p in [
        ListProblem::new(vec![1, 2], vec![])
            .unwrap()
            .allowing_unassigned(),
        ListProblem::new(
            vec![1, 2],
            vec![ListEntity::new(vec![]).with_value_range(vec![])],
        )
        .unwrap()
        .allowing_unassigned(),
    ] {
        let c = ctx(p);
        let mut s = source(ListRangeValueOrder::NativeRandom { seed: 0 });
        assert_eq!(s.size(&c), Some(0));
        assert!(s.open(&c).next().is_none());
        assert!(
            ListRangeValueSource::select_with(&c, |_| panic!("empty union must not draw"))
                .next()
                .is_none()
        );
    }
}
#[test]
fn raw_selection_retains_pinned_and_missing_values_for_downstream_filtering() {
    let c = ctx(ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1]).with_pinned(true),
            ListEntity::new(vec![2]),
        ],
    )
    .unwrap()
    .allowing_unassigned());
    let mut s = source(ListRangeValueOrder::Original);
    assert_eq!(s.open(&c).collect::<Vec<_>>(), vec![1, 2, 3]);
}
#[test]
fn shared_draw_state_advances_once_per_value_and_bad_draws_fuse() {
    let c = ctx(ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![1, 2, 3])]).unwrap());
    let state = Rc::new(Cell::new(0));
    let mut it = ListRangeValueSource::select_with(&c, {
        let state = state.clone();
        move |n| {
            let i = state.get();
            state.set(i + 1);
            i % n.get()
        }
    });
    assert_eq!(state.get(), 0);
    assert_eq!(it.next(), Some(1));
    assert_eq!(it.next(), Some(2));
    assert_eq!(state.get(), 2);
    state.set(8);
    assert_eq!(it.next(), Some(3));
    drop(it);
    let mut bad = ListRangeValueSource::select_with(&c, |n| n.get());
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| bad.next())).is_err());
    assert!(bad.next().is_none());
}
#[test]
fn random_reopen_repeats_each_step_and_solve_start_resets_the_stream() {
    let c =
        ctx(ListProblem::new((0..20).collect(), vec![ListEntity::new((0..20).collect())]).unwrap());
    let mut s = source(ListRangeValueOrder::Lcg48Random { seed: 0 });
    let first = s.open(&c).take(32).collect::<Vec<_>>();
    assert_eq!(first, s.open(&c).take(32).collect::<Vec<_>>());
    s.event(SelectionLifecycleEvent::StepStarted, &c);
    assert_ne!(first, s.open(&c).take(32).collect::<Vec<_>>());
    s.event(SelectionLifecycleEvent::SolveStarted, &c);
    assert_eq!(first, s.open(&c).take(32).collect::<Vec<_>>());
}
#[derive(Eq, PartialEq)]
struct Payload {
    value: i32,
    panic: Rc<Cell<bool>>,
}
impl Clone for Payload {
    fn clone(&self) -> Self {
        assert!(!self.panic.get(), "payload clone failed");
        Self {
            value: self.value,
            panic: self.panic.clone(),
        }
    }
}
#[test]
fn original_and_random_clone_panics_fuse_without_eager_value_copies() {
    let panic = Rc::new(Cell::new(false));
    let make = |v| Payload {
        value: v,
        panic: panic.clone(),
    };
    let p = ListProblem::new(
        vec![make(1), make(2)],
        vec![ListEntity::new(vec![make(1), make(2)])],
    )
    .unwrap();
    let assignment = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &assignment).unwrap();
    for order in [
        ListRangeValueOrder::Original,
        ListRangeValueOrder::NativeRandom { seed: 0 },
    ] {
        let mut s = source(order);
        let mut it = s.open(&c);
        panic.set(true);
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| it.next())).is_err());
        panic.set(false);
        assert!(it.next().is_none());
    }
}
struct DestinationCycle(Vec<Option<ListPosition>>);
impl<C> SelectionSource<C, Option<ListPosition>> for DestinationCycle {
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.0.len())
    }
    fn is_never_ending(&self) -> bool {
        true
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = Option<ListPosition>> + 'a>
    where
        Option<ListPosition>: 'a,
    {
        Box::new(self.0.iter().copied().cycle())
    }
}
fn provider(
    shared: Arc<ListProblem<i32>>,
    captures: Rc<RefCell<Vec<ListAssignment<i32>>>>,
) -> impl MoveProvider<ListProblem<i32>, ListAssignment<i32>, ListMove<i32>> {
    let limits = ListChangeSourceLimits {
        reads_per_side: NonZeroUsize::new(16).unwrap(),
        pairs_per_item: NonZeroUsize::new(16).unwrap(),
    };
    let src = ListValueChangeSource::new(
        source(ListRangeValueOrder::NativeRandom { seed: 7 }),
        DestinationCycle(vec![
            Some(ListPosition {
                entity: 1,
                index: 0,
            }),
            Some(ListPosition {
                entity: 1,
                index: 1,
            }),
        ]),
        limits,
        false,
    );
    let configured = SelectionBuilder::new(SelectionConfig {
        order: Some(SelectionOrder::Original),
        ..Default::default()
    })
    .build(ConfiguredSelectionPolicy::default(), |_| src)
    .unwrap();
    ConfiguredMoveProvider::new(
        configured,
        ConfiguredProviderConfig {
            maximum_draws: 16,
            seed: 0,
        },
        move |_: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| {
            captures.borrow_mut().push(a.clone());
            (shared.clone(), a.clone())
        },
        |_: &ListProblem<i32>, _: &ListAssignment<i32>, _: u64, c: &ListValueChange<i32>| {
            Some(c.movement().clone())
        },
    )
}
fn config() -> LocalSearchConfig<'static> {
    LocalSearchConfig {
        max_steps: 3,
        acceptance: Acceptance::HillClimbing,
        assert_scores: true,
        ..Default::default()
    }
}
#[test]
fn raw_union_source_drives_dense_and_stream_changes_with_fresh_layouts() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1, 2]).with_value_range(vec![1, 2]),
            ListEntity::new(vec![3]).with_value_range(vec![1, 2, 3]),
        ],
    )
    .unwrap();
    let captures = Rc::new(RefCell::new(vec![]));
    let mut dense_provider = provider(Arc::new(p.clone()), captures.clone());
    let mut n = ListConstraintNetwork::new(2, 3);
    n.add_constraint("reward", vec![ListScope::Entity(1)], |facts| {
        let ListFact::Entity(v) = facts[0] else {
            panic!()
        };
        SimpleScore::new(v.iter().map(|&x| i64::from(x)).sum())
    })
    .unwrap();
    let dense = list_local_search_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        config(),
        &mut dense_provider,
    )
    .unwrap()
    .best
    .unwrap();
    assert_eq!(dense.score, SimpleScore::new(6));
    assert!(captures.borrow().len() >= 2);
    assert_ne!(captures.borrow()[0], captures.borrow()[1]);
    let mut b = StreamGraphBuilder::new();
    let sources = ListStreamSources::new(&mut b);
    b.score("reward", sources.entities.port(), |(e, v)| {
        SimpleScore::new(if *e == 1 {
            v.iter().map(|&x| i64::from(x)).sum()
        } else {
            0
        })
    })
    .unwrap();
    let mut director =
        ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    let stream = director
        .run_local_search_with_provider(
            config(),
            &mut provider(Arc::new(p.clone()), Rc::new(RefCell::new(vec![]))),
        )
        .unwrap()
        .best
        .unwrap();
    assert_eq!(stream.score, dense.score);
    assert_eq!(stream.assignment, dense.assignment);
    director.assert_consistent().unwrap();
}
