// Original regressions; no upstream method credit.
use std::{
    cell::{Cell, RefCell},
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
};
use timetuck::*;
#[derive(Clone)]
struct Items<T> {
    items: Vec<T>,
    cycle: bool,
}
impl<T> Items<T> {
    fn finite(items: Vec<T>) -> Self {
        Self {
            items,
            cycle: false,
        }
    }
    fn repeated(items: Vec<T>) -> Self {
        Self { items, cycle: true }
    }
}
impl<C, T: Clone> SelectionSource<C, T> for Items<T> {
    fn size(&mut self, _: &C) -> Option<usize> {
        (!self.cycle).then_some(self.items.len())
    }
    fn is_never_ending(&self) -> bool {
        self.cycle
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        if self.cycle {
            Box::new(self.items.iter().cloned().cycle())
        } else {
            Box::new(self.items.iter().cloned())
        }
    }
}
fn bound(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn limits() -> ReplayedListChangeLimits {
    ReplayedListChangeLimits {
        values_per_item: bound(8),
        destinations_per_value: bound(8),
        reads_per_child: bound(8),
    }
}
fn native_source()
-> ReplayedListValueChangeSource<Items<i32>, Items<usize>, Items<i32>, ListChangeDestinationOrder> {
    ReplayedListValueChangeSource::new(
        Items::repeated(vec![1, 2]),
        Items::repeated(vec![1]),
        Items::finite(vec![]),
        ListChangeDestinationOrder::Original,
        limits(),
        false,
        false,
    )
}
fn model() -> ListProblem<i32> {
    ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![3])],
    )
    .unwrap()
}
fn config() -> LocalSearchConfig<'static, SimpleScore> {
    LocalSearchConfig {
        max_steps: 2,
        selection: MoveSelection::BestAccepted,
        assert_scores: true,
        ..Default::default()
    }
}
#[test]
fn per_value_replay_filters_ranges_and_pins_without_context_cells() {
    let p = Arc::new(
        ListProblem::new(
            vec![1, 2, 3, 4],
            vec![
                ListEntity::new(vec![1, 2])
                    .with_pinned_prefix(1)
                    .with_value_range(vec![1, 2, 3]),
                ListEntity::new(vec![3]).with_value_range(vec![2, 3]),
                ListEntity::new(vec![4])
                    .with_pinned(true)
                    .with_value_range(vec![4]),
            ],
        )
        .unwrap(),
    );
    let ctx = (p.clone(), p.initial_assignment());
    let mut source = ReplayedListValueChangeSource::new(
        Items::finite(vec![1, 2, 3, 4, 2]),
        Items::repeated(vec![0, 1, 2]),
        Items::repeated(vec![1, 2, 3, 4]),
        ListChangeDestinationOrder::Original,
        limits(),
        false,
        true,
    );
    let actual = source.open(&ctx).collect::<Vec<_>>();
    assert_eq!(
        actual.iter().map(|c| *c.value()).collect::<Vec<_>>(),
        vec![2, 3, 2]
    );
    for c in actual {
        let id = p.value_index(c.value()).unwrap();
        assert!(
            p.entities()[c.destination().unwrap().entity]
                .value_range()
                .is_none_or(|range| range.contains(&p.values()[id]))
        );
        assert_ne!(c.source().unwrap().entity, 2);
        assert_ne!(
            c.source(),
            Some(ListPosition {
                entity: 0,
                index: 0
            })
        );
        assert!(
            p.move_checker(&ctx.1)
                .unwrap()
                .is_doable(c.movement())
                .is_ok()
        );
    }
}
#[test]
fn empty_receivers_for_one_value_do_not_exhaust_later_values() {
    let p = Arc::new(
        ListProblem::new(
            vec![1, 2],
            vec![
                ListEntity::new(vec![])
                    .with_pinned(true)
                    .with_value_range(vec![1]),
                ListEntity::new(vec![2]).with_value_range(vec![2]),
            ],
        )
        .unwrap(),
    );
    let ctx = (p.clone(), p.initial_assignment());
    let mut source = ReplayedListValueChangeSource::new(
        Items::finite(vec![1, 2]),
        Items::repeated(vec![0, 1]),
        Items::repeated(vec![1, 2]),
        ListChangeDestinationOrder::Original,
        limits(),
        false,
        true,
    );
    let actual = source.open(&ctx).collect::<Vec<_>>();
    assert_eq!(actual.len(), 1);
    assert_eq!(*actual[0].value(), 2);
    assert_eq!(
        actual[0].destination(),
        Some(ListPosition {
            entity: 1,
            index: 0
        })
    );
}
#[test]
fn optional_missing_and_assigned_values_retain_distinct_noop_metadata() {
    let p = Arc::new(
        ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![1])])
            .unwrap()
            .allowing_unassigned(),
    );
    let ctx = (p.clone(), p.initial_assignment());
    let policy = |_: &(Arc<ListProblem<i32>>, ListAssignment<i32>), _: u64| {
        |_: DestinationAvailability| DestinationDraw::Unassigned
    };
    let mut source = ReplayedListValueChangeSource::new(
        Items::finite(vec![1, 2]),
        Items::repeated(vec![0]),
        Items::finite(vec![]),
        policy,
        limits(),
        true,
        true,
    );
    let actual = source.open(&ctx).collect::<Vec<_>>();
    assert_eq!(actual.len(), 2);
    assert_eq!(
        actual[0].source(),
        Some(ListPosition {
            entity: 0,
            index: 0
        })
    );
    assert_eq!(actual[0].destination(), None);
    assert!(matches!(actual[0].movement(), ListMove::Unassign { .. }));
    assert_eq!(actual[1].source(), None);
    assert_eq!(actual[1].destination(), None);
    assert_eq!(actual[1].movement(), &ListMove::Composite(vec![]));
}
struct CountingEntities(Rc<Cell<usize>>);
impl<C> SelectionSource<C, usize> for CountingEntities {
    fn size(&mut self, _: &C) -> Option<usize> {
        None
    }
    fn is_never_ending(&self) -> bool {
        true
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        Box::new(std::iter::repeat(0).inspect(|_| self.0.set(self.0.get() + 1)))
    }
}
#[test]
fn rejection_budgets_bound_infinite_children_and_fuse_the_opening() {
    let p = Arc::new(
        ListProblem::new(
            vec![1, 2],
            vec![
                ListEntity::new(vec![1]).with_pinned(true),
                ListEntity::new(vec![]),
            ],
        )
        .unwrap(),
    );
    let ctx = (p.clone(), p.initial_assignment());
    let polls = Rc::new(Cell::new(0));
    let mut source = ReplayedListValueChangeSource::new(
        Items::repeated(vec![2]),
        CountingEntities(polls.clone()),
        Items::finite(vec![]),
        ListChangeDestinationOrder::Original,
        ReplayedListChangeLimits {
            values_per_item: bound(4),
            destinations_per_value: bound(2),
            reads_per_child: bound(3),
        },
        false,
        false,
    );
    let mut it = source.open(&ctx);
    assert_eq!(polls.get(), 0);
    assert!(it.next().is_none());
    assert_eq!(polls.get(), 12);
    assert!(it.next().is_none());
    assert_eq!(polls.get(), 12);
}
#[test]
fn native_random_openings_replay_seed_and_reopen_step_changes() {
    let p = Arc::new(model());
    let ctx = (p.clone(), p.initial_assignment());
    let mut source = ReplayedListValueChangeSource::new(
        Items::repeated(vec![1, 2, 3]),
        Items::repeated(vec![0, 1]),
        Items::repeated(vec![1, 2, 3]),
        ListChangeDestinationOrder::Random { seed: 42 },
        limits(),
        false,
        true,
    );
    let first = source.open(&ctx).take(120).collect::<Vec<_>>();
    assert_eq!(first.len(), 120);
    assert_eq!(source.open(&ctx).take(120).collect::<Vec<_>>(), first);
    source.event(SelectionLifecycleEvent::StepStarted, &ctx);
    let second = source.open(&ctx).take(120).collect::<Vec<_>>();
    assert_ne!(first, second);
    let checker = p.move_checker(&ctx.1).unwrap();
    for c in first.iter().chain(&second) {
        assert!(checker.is_doable(c.movement()).is_ok());
    }
    source.event(SelectionLifecycleEvent::SolveStarted, &ctx);
    assert_eq!(source.open(&ctx).take(120).collect::<Vec<_>>(), first);
}
#[test]
fn caught_policy_panics_do_not_resume_a_partial_pair() {
    let p = Arc::new(model());
    let ctx = (p.clone(), p.initial_assignment());
    let draws = Rc::new(Cell::new(0));
    let observed = draws.clone();
    let policy = move |_: &(Arc<ListProblem<i32>>, ListAssignment<i32>), _: u64| {
        let count = observed.clone();
        move |_: DestinationAvailability| -> DestinationDraw {
            count.set(count.get() + 1);
            panic!("draw")
        }
    };
    let mut source = ReplayedListValueChangeSource::new(
        Items::repeated(vec![1]),
        Items::repeated(vec![0]),
        Items::finite(vec![]),
        policy,
        limits(),
        false,
        false,
    );
    let mut it = source.open(&ctx);
    assert!(catch_unwind(AssertUnwindSafe(|| it.next())).is_err());
    assert_eq!(draws.get(), 1);
    assert!(it.next().is_none());
    assert_eq!(draws.get(), 1);
}
#[test]
fn view_policy_identity_is_stable_and_clear_preserves_owned_handles() {
    let index = ReachableValues::new(vec![
        (0, vec!["a", "b", "c"], false),
        (1, vec!["a", "b", "c"], false),
    ])
    .unwrap();
    let mut bank = ReachabilityViews::new(index);
    let raw = bank.unsorted_view();
    let sorted = bank.sorted_view(ReachabilityViewKey(1), |a, b| b.cmp(a));
    assert!(Arc::ptr_eq(
        &sorted,
        &bank.sorted_view(ReachabilityViewKey(1), Ord::cmp)
    ));
    assert_eq!(sorted.ordered_co_values(&"b"), vec![&"c", &"a"]);
    assert_eq!(raw.ordered_co_values(&"b"), vec![&"a", &"c"]);
    assert!(raw.matches_value_payload(&"unknown"));
    assert!(!raw.matches_value_payload(&String::from("a")));
    let a = raw.reachable_index().domain(&0).unwrap();
    let b = sorted.reachable_index().domain(&1).unwrap();
    assert!(Arc::ptr_eq(&a, &b));
    assert_eq!(raw.reachable_index().entities(&"a"), [0, 1]);
    bank.clear_views();
    assert!(!Arc::ptr_eq(&raw, &bank.unsorted_view()));
    assert_eq!(sorted.ordered_co_values(&"b"), vec![&"c", &"a"]);
}
fn list_provider(
    shared: Arc<ListProblem<i32>>,
    captures: Rc<RefCell<Vec<ListAssignment<i32>>>>,
) -> impl MoveProvider<ListProblem<i32>, ListAssignment<i32>, ListMove<i32>> {
    let selected = SelectionBuilder::new(SelectionConfig {
        order: Some(SelectionOrder::Original),
        ..Default::default()
    })
    .build(ConfiguredSelectionPolicy::default(), |_| native_source())
    .unwrap();
    ConfiguredMoveProvider::new(
        selected,
        ConfiguredProviderConfig {
            maximum_draws: 2,
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
#[test]
fn automatic_replay_runs_actual_dense_list_search() {
    let p = model();
    let captures = Rc::new(RefCell::new(vec![]));
    let mut provider = list_provider(Arc::new(p.clone()), captures.clone());
    let mut n = ListConstraintNetwork::new(2, 3);
    n.add_constraint("reward", vec![ListScope::Entity(1)], |rows| {
        let ListFact::Entity(values) = rows[0] else {
            panic!()
        };
        SimpleScore::new(10 * values.iter().map(|&v| i64::from(v)).sum::<i64>())
    })
    .unwrap();
    let result = list_local_search_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        config(),
        &mut provider,
    )
    .unwrap();
    assert_eq!(result.best.unwrap().score, SimpleScore::new(60));
    assert_eq!(captures.borrow().len(), 2);
    assert_ne!(captures.borrow()[0], captures.borrow()[1]);
}
fn stream_reward(b: &mut StreamGraphBuilder<'_, SimpleScore>, s: &ListStreamSources<i32>) {
    b.score("reward", s.entities.port(), |(e, values)| {
        SimpleScore::new(if *e == 1 {
            10 * values.iter().map(|&v| i64::from(v)).sum::<i64>()
        } else {
            0
        })
    })
    .unwrap();
}
#[test]
fn automatic_replay_runs_actual_stream_list_search() {
    let p = model();
    let mut provider = list_provider(Arc::new(p.clone()), Rc::new(RefCell::new(vec![])));
    let mut b = StreamGraphBuilder::new();
    let s = ListStreamSources::new(&mut b);
    stream_reward(&mut b, &s);
    let mut d = ListStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    let result = d
        .run_local_search_with_provider(config(), &mut provider)
        .unwrap();
    assert_eq!(result.best.unwrap().score, SimpleScore::new(60));
    d.assert_consistent().unwrap();
}
fn mixed_model() -> MixedProblem<i32, i32> {
    MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![7]).with_initial(7)]).unwrap(),
        model(),
    )
}
fn mixed_provider(
    shared: Arc<ListProblem<i32>>,
) -> impl MoveProvider<MixedProblem<i32, i32>, MixedAssignment<i32, i32>, MixedMove<i32, i32>> {
    let selected = SelectionBuilder::new(SelectionConfig {
        order: Some(SelectionOrder::Original),
        ..Default::default()
    })
    .build(ConfiguredSelectionPolicy::default(), |_| native_source())
    .unwrap();
    ConfiguredMoveProvider::new(
        selected,
        ConfiguredProviderConfig {
            maximum_draws: 2,
            seed: 0,
        },
        move |_: &MixedProblem<i32, i32>, a: &MixedAssignment<i32, i32>, _: u64| {
            (shared.clone(), a.lists().clone())
        },
        |_: &MixedProblem<i32, i32>,
         _: &MixedAssignment<i32, i32>,
         _: u64,
         c: &ListValueChange<i32>| {
            Some(MixedMove {
                basic: Move::Composite(vec![]),
                lists: c.movement().clone(),
            })
        },
    )
}
#[test]
fn automatic_replay_runs_actual_dense_mixed_search() {
    let p = mixed_model();
    let mut provider = mixed_provider(Arc::new(p.lists().clone()));
    let mut n = MixedConstraintNetwork::new(1, 2, 3);
    n.add_constraint("reward", vec![MixedScope::ListEntity(1)], |rows| {
        let MixedFact::ListEntity(values) = rows[0] else {
            panic!()
        };
        SimpleScore::new(10 * values.iter().map(|&v| i64::from(v)).sum::<i64>())
    })
    .unwrap();
    let result = mixed_local_search_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        MixedLocalSearchConfig {
            search: config(),
            ..Default::default()
        },
        &mut provider,
    )
    .unwrap();
    let best = result.best.unwrap();
    assert_eq!(best.score, SimpleScore::new(60));
    assert_eq!(best.assignment.basic().values(), &[Some(7)]);
}
#[test]
fn automatic_replay_runs_actual_stream_mixed_search() {
    let p = mixed_model();
    let mut provider = mixed_provider(Arc::new(p.lists().clone()));
    let mut b = StreamGraphBuilder::new();
    let s = MixedStreamSources::new(&mut b);
    stream_reward(&mut b, &s.lists);
    let mut d = MixedStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    let result = d
        .run_local_search_with_provider(
            MixedLocalSearchConfig {
                search: config(),
                ..Default::default()
            },
            &mut provider,
        )
        .unwrap();
    let best = result.best.unwrap();
    assert_eq!(best.score, SimpleScore::new(60));
    assert_eq!(best.assignment.basic().values(), &[Some(7)]);
    d.assert_consistent().unwrap();
}

type HookLog = Rc<RefCell<Vec<(&'static str, Option<SelectionLifecycleEvent>)>>>;
struct Traced<T> {
    name: &'static str,
    items: Vec<T>,
    log: HookLog,
}
impl<C, T: Clone> SelectionSource<C, T> for Traced<T> {
    fn event(&mut self, event: SelectionLifecycleEvent, _: &C) {
        self.log.borrow_mut().push((self.name, Some(event)));
    }
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.items.len())
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        self.log.borrow_mut().push((self.name, None));
        Box::new(self.items.iter().cloned())
    }
}
struct TracedPolicy(HookLog);
impl<C> ListChangeDrawFactory<C> for TracedPolicy {
    fn change_draw_event(&mut self, event: SelectionLifecycleEvent, _: &C) {
        self.0.borrow_mut().push(("policy", Some(event)));
    }
    fn change_draws<'a>(
        &'a mut self,
        _: &'a C,
        _: u64,
    ) -> Box<dyn FnMut(DestinationAvailability) -> DestinationDraw + 'a> {
        self.0.borrow_mut().push(("policy", None));
        Box::new(|_| DestinationDraw::EntityStart { skip: 0 })
    }
}
#[test]
fn lifecycle_forwarding_and_lazy_open_preserve_all_four_child_orders() {
    let p = Arc::new(ListProblem::new(vec![1], vec![ListEntity::new(vec![1])]).unwrap());
    let ctx = (p.clone(), p.initial_assignment());
    let log = Rc::new(RefCell::new(vec![]));
    let mut source = ReplayedListValueChangeSource::new(
        Traced {
            name: "values",
            items: vec![1],
            log: log.clone(),
        },
        Traced {
            name: "entities",
            items: vec![0],
            log: log.clone(),
        },
        Traced {
            name: "anchors",
            items: Vec::<i32>::new(),
            log: log.clone(),
        },
        TracedPolicy(log.clone()),
        limits(),
        false,
        true,
    );
    let events = [
        SelectionLifecycleEvent::SolveStarted,
        SelectionLifecycleEvent::PhaseStarted,
        SelectionLifecycleEvent::StepStarted,
        SelectionLifecycleEvent::StepEnded,
        SelectionLifecycleEvent::PhaseEnded,
        SelectionLifecycleEvent::SolveEnded,
    ];
    for e in events {
        source.event(e, &ctx);
    }
    let expected = events
        .into_iter()
        .flat_map(|e| ["values", "entities", "anchors", "policy"].map(|name| (name, Some(e))))
        .collect::<Vec<_>>();
    assert_eq!(*log.borrow(), expected);
    log.borrow_mut().clear();
    let mut it = source.open(&ctx);
    assert!(log.borrow().is_empty());
    assert!(it.next().is_some());
    assert_eq!(
        *log.borrow(),
        vec![
            ("values", None),
            ("entities", None),
            ("anchors", None),
            ("policy", None)
        ]
    );
}
#[test]
fn filtering_anchor_owners_does_not_remove_entity_destinations() {
    let p = Arc::new(
        ListProblem::new(
            vec![1, 2, 3],
            vec![
                ListEntity::new(vec![1, 2]).with_value_range(vec![1, 2, 3]),
                ListEntity::new(vec![3]).with_value_range(vec![2, 3]),
            ],
        )
        .unwrap(),
    );
    let ctx = (p.clone(), p.initial_assignment());
    // Anchor child excludes B-owned values; B remains an entity receiver.
    let anchor_policy = |_: &(Arc<ListProblem<i32>>, ListAssignment<i32>), _: u64| {
        |_: DestinationAvailability| DestinationDraw::AfterValue { fallback_rank: 0 }
    };
    let mut source = ReplayedListValueChangeSource::new(
        Items::repeated(vec![3]),
        Items::repeated(vec![1]),
        Items::repeated(vec![1, 2]),
        anchor_policy,
        limits(),
        false,
        true,
    );
    let actual = source.open(&ctx).take(4).collect::<Vec<_>>();
    assert_eq!(
        actual
            .iter()
            .map(|c| c.destination().unwrap())
            .collect::<Vec<_>>(),
        vec![
            ListPosition {
                entity: 0,
                index: 1
            },
            ListPosition {
                entity: 0,
                index: 2
            },
            ListPosition {
                entity: 0,
                index: 1
            },
            ListPosition {
                entity: 0,
                index: 2
            }
        ]
    );
    let mut source = ReplayedListValueChangeSource::new(
        Items::finite(vec![3]),
        Items::repeated(vec![1]),
        Items::repeated(vec![1, 2]),
        ListChangeDestinationOrder::Original,
        limits(),
        false,
        true,
    );
    assert_eq!(
        source.open(&ctx).next().unwrap().destination(),
        Some(ListPosition {
            entity: 1,
            index: 0
        })
    );
}
