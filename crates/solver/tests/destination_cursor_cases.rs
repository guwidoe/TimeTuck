// Adapted from Community ElementDestinationSelectorTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{cell::RefCell, num::NonZeroUsize, rc::Rc, sync::Arc};
use timetuck::*;
fn limit() -> NonZeroUsize {
    NonZeroUsize::new(128).unwrap()
}
fn p(e: usize, i: usize) -> Option<ListPosition> {
    Some(ListPosition {
        entity: e,
        index: i,
    })
}
fn ranged() -> ListProblem<usize> {
    ListProblem::new(
        (1..=5).collect(),
        vec![
            ListEntity::new(vec![1, 2]).with_value_range(vec![1, 2]),
            ListEntity::new(vec![3]).with_value_range(vec![2, 3]),
            ListEntity::new(vec![4, 5]).with_value_range(vec![3, 4, 5]),
        ],
    )
    .unwrap()
}
#[test]
fn random_with_entity_value_range() {
    let model = ranged();
    let a = model.initial_assignment();
    // Native semantic entity/anchor choices replace TestRandom's Java decorator
    // call sequence. The complete source outputs and repeated availability stay.
    for (value, draw, expected) in [
        (3, DestinationDraw::EntityStart { skip: 1 }, p(2, 0)),
        (1, DestinationDraw::AfterValue { fallback_rank: 0 }, p(0, 2)),
        (2, DestinationDraw::AfterValue { fallback_rank: 0 }, p(1, 1)),
        (5, DestinationDraw::AfterValue { fallback_rank: 0 }, p(2, 1)),
    ] {
        let mut c = ListDestinationCursor::new(
            &model,
            &a,
            [0, 1, 2].into_iter().cycle(),
            [3, 1, 2, 4, 5].into_iter().cycle(),
            move |_| draw,
            false,
            limit(),
        )
        .unwrap();
        c.select_replay(DestinationReplay {
            epoch: value as u64,
            value: Some(value - 1),
        })
        .unwrap();
        assert!(c.has_next().unwrap());
        assert_eq!(c.next_required().unwrap(), expected);
        assert!(c.has_next().unwrap());
    }
}
#[test]
fn random_partially_pinned_and_unassigned() {
    let model = ListProblem::new(
        (1..=7).collect(),
        vec![
            ListEntity::new(vec![1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![]),
            ListEntity::new(vec![3]).with_pinned_prefix(1),
            ListEntity::new(vec![4, 5, 6]).with_pinned_prefix(2),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = model.initial_assignment();
    let mut choices = vec![
        DestinationDraw::Unassigned,
        DestinationDraw::EntityStart { skip: 0 },
        DestinationDraw::EntityStart { skip: 0 },
        DestinationDraw::EntityStart { skip: 0 },
        DestinationDraw::EntityStart { skip: 0 },
        DestinationDraw::AfterValue { fallback_rank: 0 },
        DestinationDraw::AfterValue { fallback_rank: 0 },
        DestinationDraw::EntitySuffix { skip: 0, rank: 0 },
        DestinationDraw::EntitySuffix { skip: 0, rank: 1 },
    ]
    .into_iter();
    // Entity choices correspond to source random IDs 0,1,2,3,3,3. Value child
    // retains source reverse order with missing 7 and pinned-prefix anchors.
    let mut c = ListDestinationCursor::new(
        &model,
        &a,
        [0, 1, 2, 3, 3, 3].into_iter(),
        [7, 6, 5, 4, 3, 2, 1].into_iter().cycle(),
        move |_| choices.next().unwrap_or(DestinationDraw::Unassigned),
        true,
        limit(),
    )
    .unwrap();
    let actual = (&mut c).take(9).collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            None,
            p(0, 1),
            p(1, 0),
            p(2, 1),
            p(3, 2),
            p(3, 3),
            p(0, 2),
            p(3, 2),
            p(3, 3)
        ]
    );
    assert!(c.has_next().unwrap());
}
struct Child<T> {
    items: Vec<T>,
    cycling: bool,
    events: Rc<RefCell<Vec<SelectionLifecycleEvent>>>,
}
impl<T: Clone, C> SelectionSource<C, T> for Child<T> {
    fn event(&mut self, e: SelectionLifecycleEvent, _: &C) {
        self.events.borrow_mut().push(e);
    }
    fn size(&mut self, _: &C) -> Option<usize> {
        (!self.cycling).then_some(self.items.len())
    }
    fn is_never_ending(&self) -> bool {
        self.cycling
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        if self.cycling {
            Box::new(self.items.iter().cloned().cycle())
        } else {
            Box::new(self.items.iter().cloned())
        }
    }
}
fn child<T>(items: Vec<T>, cycling: bool) -> Child<T> {
    Child {
        items,
        cycling,
        events: Rc::new(RefCell::new(vec![])),
    }
}
#[test]
fn random_unassigned_single_entity() {
    let model = Arc::new(
        ListProblem::new(vec![3], vec![ListEntity::new(vec![])])
            .unwrap()
            .allowing_unassigned(),
    );
    let a = model.initial_assignment();
    let ctx = (model, a);
    let mut source = ListDestinationSelectionSource::new(
        child(vec![0], true),
        child(vec![3], true),
        true,
        true,
        0,
        limit(),
    );
    assert!(
        <ListDestinationSelectionSource<usize, _, _> as SelectionSource<
            (Arc<ListProblem<usize>>, ListAssignment<usize>),
            Option<ListPosition>,
        >>::is_never_ending(&source)
    );
    let mut it = source.open(&ctx);
    assert_eq!(it.next(), Some(p(0, 0)));
    assert_eq!(it.next(), Some(None));
    assert!(it.next().is_some());
}
#[test]
fn random_fully_pinned() {
    let model = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1, 2]).with_pinned_prefix(2),
            ListEntity::new(vec![]),
            ListEntity::new(vec![3]).with_pinned(true),
        ],
    )
    .unwrap();
    let a = model.initial_assignment();
    let mut draws = [
        DestinationDraw::EntityStart { skip: 0 },
        DestinationDraw::AfterValue { fallback_rank: 0 },
    ]
    .into_iter();
    let c = ListDestinationCursor::new(
        &model,
        &a,
        [0, 1].into_iter(),
        [3, 2, 1].into_iter().cycle(),
        move |_| draws.next().expect("only two destinations exist"),
        false,
        limit(),
    )
    .unwrap();
    assert_eq!(c.collect::<Vec<_>>(), vec![p(0, 2), p(1, 0)]);
}
#[test]
fn refresh_reachable_entities() {
    let model = ListProblem::new(
        vec![1, 2],
        vec![
            ListEntity::new(vec![1])
                .with_value_range(vec![1])
                .with_pinned(true),
            ListEntity::new(vec![2]).with_value_range(vec![2]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = model.initial_assignment();
    let mut c = ListDestinationCursor::new(
        &model,
        &a,
        [0, 1].into_iter().cycle(),
        [1, 2].into_iter().cycle(),
        |h| {
            if h.entity_start {
                DestinationDraw::EntityStart { skip: 0 }
            } else {
                DestinationDraw::Unassigned
            }
        },
        true,
        limit(),
    )
    .unwrap();
    c.select_replay(DestinationReplay {
        epoch: 1,
        value: Some(0),
    })
    .unwrap();
    assert!(c.has_next().unwrap());
    assert_eq!(c.next_required().unwrap(), None);
    c.select_replay(DestinationReplay {
        epoch: 2,
        value: Some(1),
    })
    .unwrap();
    assert!(c.has_next().unwrap());
    let at = c.next_required().unwrap().unwrap();
    assert_eq!(at, p(1, 0).unwrap());
    assert!(std::ptr::eq(
        &model.entities()[at.entity],
        &model.entities()[1]
    ));
}
#[test]
fn phase_lifecycle() {
    let model = Arc::new(ListProblem::<usize>::new(vec![], vec![]).unwrap());
    let ctx = (model, ListAssignment::new(vec![]));
    let mut source = ListDestinationSelectionSource::new(
        child(Vec::<usize>::new(), false),
        child(Vec::<usize>::new(), false),
        false,
        false,
        0,
        limit(),
    );
    let expected = [
        SelectionLifecycleEvent::SolveStarted,
        SelectionLifecycleEvent::PhaseStarted,
        SelectionLifecycleEvent::StepStarted,
        SelectionLifecycleEvent::StepEnded,
        SelectionLifecycleEvent::StepStarted,
        SelectionLifecycleEvent::StepEnded,
        SelectionLifecycleEvent::PhaseEnded,
        SelectionLifecycleEvent::SolveEnded,
    ];
    for e in expected {
        source.event(e, &ctx);
    }
    assert_eq!(*source.entity_source().events.borrow(), expected);
    assert_eq!(*source.anchor_source().events.borrow(), expected);
}
#[test]
fn discard_old_values() {
    let model = ranged();
    let a = model.initial_assignment();
    let mut c = ListDestinationCursor::new(
        &model,
        &a,
        [0].into_iter(),
        [3, 3].into_iter(),
        |_| DestinationDraw::AfterValue { fallback_rank: 0 },
        false,
        limit(),
    )
    .unwrap();
    // Source's children are mocked as already filtered: replay IDs invalidate
    // lookahead but do not enforce ranges again on the mocked anchor stream.
    c.select_replay(DestinationReplay {
        epoch: 1,
        value: None,
    })
    .unwrap();
    assert_eq!(c.next_required().unwrap(), p(1, 1));
    c.select_replay(DestinationReplay {
        epoch: 3,
        value: None,
    })
    .unwrap();
    assert_eq!(c.next_required().unwrap(), p(1, 1));
    assert!(!c.has_next().unwrap());
    assert_eq!(c.discarded_replay_lookahead(), 1);
}
#[test]
fn discard_old_values_and_reset_state() {
    let model = ListProblem::new(
        vec![1, 2],
        vec![ListEntity::new(vec![]).with_value_range(vec![])],
    )
    .unwrap();
    let a = model.initial_assignment();
    let mut raw = [Some(0), None, Some(0), None].into_iter();
    let entities = std::iter::from_fn(move || raw.next().flatten());
    let mut c = ListDestinationCursor::new(
        &model,
        &a,
        entities,
        [1].into_iter(),
        |_| DestinationDraw::EntityStart { skip: 0 },
        false,
        limit(),
    )
    .unwrap();
    c.select_replay(DestinationReplay {
        epoch: 1,
        value: None,
    })
    .unwrap();
    assert!(c.has_next().unwrap());
    assert!(c.next_required().unwrap().is_some());
    assert!(!c.has_next().unwrap());
    assert_eq!(c.next_required(), Err(DestinationCursorError::Exhausted));
    c.select_replay(DestinationReplay {
        epoch: 2,
        value: None,
    })
    .unwrap();
    assert!(c.has_next().unwrap());
    assert!(c.next_required().unwrap().is_some());
    assert!(!c.has_next().unwrap());
    assert_eq!(c.next_required(), Err(DestinationCursorError::Exhausted));
}
