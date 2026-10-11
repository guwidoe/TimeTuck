// Original stateful destination/source integration; no upstream credit.
use std::{
    cell::{Cell, RefCell},
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
};
use timetuck::*;
fn bound(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn at(e: usize, i: usize) -> Option<ListPosition> {
    Some(ListPosition {
        entity: e,
        index: i,
    })
}
#[test]
fn repeated_peeks_do_not_draw_or_poll_children_again() {
    let p = ListProblem::new(
        vec![1, 2],
        vec![ListEntity::new(vec![1]), ListEntity::new(vec![2])],
    )
    .unwrap();
    let a = p.initial_assignment();
    let er = Rc::new(Cell::new(0));
    let ar = Rc::new(Cell::new(0));
    let dr = Rc::new(Cell::new(0));
    let observed = er.clone();
    let entities = [0, 1]
        .into_iter()
        .inspect(move |_| observed.set(observed.get() + 1));
    let observed = ar.clone();
    let anchors = [1, 2]
        .into_iter()
        .inspect(move |_| observed.set(observed.get() + 1));
    let observed = dr.clone();
    let draw = move |_| {
        observed.set(observed.get() + 1);
        DestinationDraw::EntityStart { skip: 0 }
    };
    let mut c =
        ListDestinationCursor::new(&p, &a, entities, anchors, draw, false, bound(4)).unwrap();
    assert!(c.has_next().unwrap());
    assert!(c.has_next().unwrap());
    assert_eq!((er.get(), ar.get(), dr.get()), (1, 1, 1));
    assert_eq!(c.next_required().unwrap(), at(0, 0));
    assert_eq!((er.get(), ar.get(), dr.get()), (1, 1, 1));
    assert!(c.has_next().unwrap());
    assert_eq!((er.get(), ar.get(), dr.get()), (2, 1, 2));
    c.select_replay(DestinationReplay {
        epoch: 1,
        value: Some(0),
    })
    .unwrap();
    assert_eq!(c.discarded_replay_lookahead(), 1);
    assert!(matches!(
        c.select_replay(DestinationReplay {
            epoch: 2,
            value: Some(20)
        }),
        Err(DestinationCursorError::Model(ListError::ValueOutsideDomain))
    ));
}
#[test]
fn bounded_unbounded_rejection_and_large_skips_are_explicit() {
    let p = ListProblem::new(
        vec![1, 2],
        vec![
            ListEntity::new(vec![1]).with_pinned(true),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let reads = Rc::new(Cell::new(0));
    let observed = reads.clone();
    let e = std::iter::repeat(0).inspect(move |_| observed.set(observed.get() + 1));
    let mut c = ListDestinationCursor::new(
        &p,
        &a,
        e,
        std::iter::empty(),
        |_| DestinationDraw::Unassigned,
        true,
        bound(3),
    )
    .unwrap();
    assert_eq!(c.next_required().unwrap(), None);
    assert_eq!(reads.get(), 3);
    assert!(c.reached_read_limit());
    let p = ListProblem::new(vec![1], vec![ListEntity::new(vec![1])]).unwrap();
    let a = p.initial_assignment();
    let mut c = ListDestinationCursor::new(
        &p,
        &a,
        std::iter::repeat(0),
        std::iter::empty(),
        |_| DestinationDraw::EntityStart { skip: usize::MAX },
        false,
        bound(2),
    )
    .unwrap();
    assert_eq!(c.try_next().unwrap(), None);
    assert!(c.reached_read_limit());
}
#[test]
fn bad_child_metadata_and_choices_leave_the_layout_unchanged() {
    let p = ListProblem::new(vec![1], vec![ListEntity::new(vec![1])]).unwrap();
    let a = p.initial_assignment();
    let mut c = ListDestinationCursor::new(
        &p,
        &a,
        [4].into_iter(),
        std::iter::empty(),
        |_| DestinationDraw::EntityStart { skip: 0 },
        false,
        bound(2),
    )
    .unwrap();
    assert!(matches!(
        c.try_next(),
        Err(DestinationCursorError::Model(ListError::EntityIndex {
            entity: 4,
            ..
        }))
    ));
    let mut c = ListDestinationCursor::new(
        &p,
        &a,
        [0].into_iter(),
        [9].into_iter(),
        |_| DestinationDraw::EntityStart { skip: 0 },
        false,
        bound(2),
    )
    .unwrap();
    assert_eq!(
        c.try_next(),
        Err(DestinationCursorError::Model(ListError::ValueOutsideDomain))
    );
    let mut c = ListDestinationCursor::new(
        &p,
        &a,
        [0].into_iter(),
        std::iter::empty(),
        |_| DestinationDraw::EntitySuffix {
            skip: 0,
            rank: usize::MAX,
        },
        false,
        bound(2),
    )
    .unwrap();
    assert!(matches!(
        c.try_next(),
        Err(DestinationCursorError::Model(ListError::InsertionIndex(_)))
    ));
    let mut c = ListDestinationCursor::new(
        &p,
        &a,
        [0].into_iter(),
        std::iter::empty(),
        |_| DestinationDraw::Unassigned,
        false,
        bound(2),
    )
    .unwrap();
    assert_eq!(c.try_next(), Err(DestinationCursorError::InvalidChoice));
    assert_eq!(a.lists(), &[vec![1]]);
}
#[test]
fn a_caught_child_or_draw_panic_poison_the_current_opening() {
    let p = ListProblem::new(vec![1], vec![ListEntity::new(vec![1])]).unwrap();
    let a = p.initial_assignment();
    let e = std::iter::from_fn(|| -> Option<usize> { panic!("child") });
    let mut c = ListDestinationCursor::new(
        &p,
        &a,
        e,
        std::iter::empty(),
        |_| DestinationDraw::EntityStart { skip: 0 },
        false,
        bound(2),
    )
    .unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| c.has_next())).is_err());
    assert_eq!(c.try_next(), Err(DestinationCursorError::Poisoned));
    assert_eq!(
        c.select_replay(DestinationReplay {
            epoch: 1,
            value: None
        }),
        Err(DestinationCursorError::Poisoned)
    );
    let mut c = ListDestinationCursor::new(
        &p,
        &a,
        [0].into_iter(),
        std::iter::empty(),
        |_| panic!("draw"),
        false,
        bound(2),
    )
    .unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| c.has_next())).is_err());
    assert_eq!(c.try_next(), Err(DestinationCursorError::Poisoned));
}
struct World {
    model: Arc<ListProblem<i32>>,
    layout: ListAssignment<i32>,
    replay: Cell<DestinationReplay>,
}
impl ListSelectionView<i32> for World {
    fn problem(&self) -> &ListProblem<i32> {
        &self.model
    }
    fn assignment(&self) -> &ListAssignment<i32> {
        &self.layout
    }
}
impl DestinationSelectionView<i32> for World {
    fn list_destination_replay(&self) -> DestinationReplay {
        self.replay.get()
    }
}
struct Items<T> {
    items: Vec<T>,
    cycle: bool,
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
struct ReplayedValues {
    values: Vec<i32>,
}
impl SelectionSource<World, i32> for ReplayedValues {
    fn size(&mut self, _: &World) -> Option<usize> {
        None
    }
    fn is_never_ending(&self) -> bool {
        true
    }
    fn open<'a>(&'a mut self, c: &'a World) -> Box<dyn Iterator<Item = i32> + 'a>
    where
        i32: 'a,
    {
        Box::new(self.values.iter().copied().cycle().inspect(move |value| {
            let mut r = c.replay.get();
            let id = c.model.value_index(value).unwrap();
            if r.value != Some(id) {
                r.epoch = r.epoch.wrapping_add(1);
                r.value = Some(id);
                c.replay.set(r);
            }
        }))
    }
}
#[test]
fn original_optional_selection_finishes_and_replay_reopens_reporting() {
    let model = Arc::new(
        ListProblem::new(vec![1], vec![ListEntity::new(vec![1])])
            .unwrap()
            .allowing_unassigned(),
    );
    let layout = model.initial_assignment();
    let c = World {
        model,
        layout,
        replay: Cell::new(DestinationReplay::default()),
    };
    let mut source = ListDestinationSelectionSource::new(
        Items {
            items: vec![0],
            cycle: false,
        },
        Items {
            items: vec![1],
            cycle: false,
        },
        false,
        true,
        0,
        bound(4),
    );
    let mut it = source.open(&c);
    assert_eq!(it.next(), Some(at(0, 0)));
    assert_eq!(it.next(), Some(at(0, 1)));
    assert_eq!(it.next(), Some(None));
    assert_eq!(it.next(), None);
    c.replay.set(DestinationReplay {
        epoch: 1,
        value: Some(0),
    });
    assert_eq!(it.next(), Some(None));
    assert_eq!(it.next(), None);
}
fn source() -> ListValueChangeSource<
    ReplayedValues,
    ListDestinationSelectionSource<i32, Items<usize>, Items<i32>>,
> {
    ListValueChangeSource::new(
        ReplayedValues { values: vec![1, 2] },
        ListDestinationSelectionSource::new(
            Items {
                items: vec![1],
                cycle: true,
            },
            Items {
                items: vec![],
                cycle: false,
            },
            false,
            false,
            0,
            bound(8),
        ),
        ListChangeSourceLimits {
            reads_per_side: bound(4),
            pairs_per_item: bound(4),
        },
        false,
    )
}
fn config() -> LocalSearchConfig<'static, SimpleScore> {
    LocalSearchConfig {
        max_steps: 2,
        selection: MoveSelection::BestAccepted,
        assert_scores: true,
        ..Default::default()
    }
}
fn model() -> ListProblem<i32> {
    ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![3])],
    )
    .unwrap()
}
#[test]
fn replayed_source_generates_checked_moves_in_actual_dense_list_search() {
    let p = model();
    let shared = Arc::new(p.clone());
    let mut n = ListConstraintNetwork::new(2, 3);
    n.add_constraint("reward", vec![ListScope::Entity(1)], |rows| {
        SimpleScore::new(
            10 * match &rows[0] {
                ListFact::Entity(row) => row.iter().map(|&v| i64::from(v)).sum::<i64>(),
                _ => unreachable!(),
            },
        )
    })
    .unwrap();
    let captures = Rc::new(RefCell::new(vec![]));
    let observed = captures.clone();
    let selected = SelectionBuilder::new(SelectionConfig {
        order: Some(SelectionOrder::Original),
        ..Default::default()
    })
    .build(ConfiguredSelectionPolicy::default(), |_| source())
    .unwrap();
    let mut provider = ConfiguredMoveProvider::new(
        selected,
        ConfiguredProviderConfig {
            maximum_draws: 2,
            seed: 0,
        },
        move |_: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| {
            observed.borrow_mut().push(a.lists().to_vec());
            World {
                model: shared.clone(),
                layout: a.clone(),
                replay: Cell::new(DestinationReplay::default()),
            }
        },
        |_: &ListProblem<i32>, _: &ListAssignment<i32>, _: u64, c: &ListValueChange<i32>| {
            Some(c.movement().clone())
        },
    );
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
fn list_provider(
    shared: Arc<ListProblem<i32>>,
) -> impl MoveProvider<ListProblem<i32>, ListAssignment<i32>, ListMove<i32>> {
    let selected = SelectionBuilder::new(SelectionConfig {
        order: Some(SelectionOrder::Original),
        ..Default::default()
    })
    .build(ConfiguredSelectionPolicy::default(), |_| source())
    .unwrap();
    ConfiguredMoveProvider::new(
        selected,
        ConfiguredProviderConfig {
            maximum_draws: 2,
            seed: 0,
        },
        move |_: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| World {
            model: shared.clone(),
            layout: a.clone(),
            replay: Cell::new(DestinationReplay::default()),
        },
        |_: &ListProblem<i32>, _: &ListAssignment<i32>, _: u64, c: &ListValueChange<i32>| {
            Some(c.movement().clone())
        },
    )
}
fn stream_reward(b: &mut StreamGraphBuilder<'_, SimpleScore>, source: &ListStreamSources<i32>) {
    b.score("reward", source.entities.port(), |(e, values)| {
        SimpleScore::new(if *e == 1 {
            10 * values.iter().map(|&v| i64::from(v)).sum::<i64>()
        } else {
            0
        })
    })
    .unwrap();
}
#[test]
fn replayed_source_generates_checked_moves_in_actual_stream_list_search() {
    let p = model();
    let mut provider = list_provider(Arc::new(p.clone()));
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
fn mixed_provider(
    shared: Arc<ListProblem<i32>>,
) -> impl MoveProvider<MixedProblem<i32, i32>, MixedAssignment<i32, i32>, MixedMove<i32, i32>> {
    let selected = SelectionBuilder::new(SelectionConfig {
        order: Some(SelectionOrder::Original),
        ..Default::default()
    })
    .build(ConfiguredSelectionPolicy::default(), |_| source())
    .unwrap();
    ConfiguredMoveProvider::new(
        selected,
        ConfiguredProviderConfig {
            maximum_draws: 2,
            seed: 0,
        },
        move |_: &MixedProblem<i32, i32>, a: &MixedAssignment<i32, i32>, _: u64| World {
            model: shared.clone(),
            layout: a.lists().clone(),
            replay: Cell::new(DestinationReplay::default()),
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
fn mixed_model() -> MixedProblem<i32, i32> {
    MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![7]).with_initial(7)]).unwrap(),
        model(),
    )
}
#[test]
fn replayed_source_generates_checked_moves_in_actual_dense_mixed_search() {
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
fn replayed_source_generates_checked_moves_in_actual_stream_mixed_search() {
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
