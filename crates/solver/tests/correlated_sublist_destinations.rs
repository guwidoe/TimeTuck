//! Original block-dependent relocation, bounded work and provider solve regressions.
use std::{
    cell::{Cell, RefCell},
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
};
use timetuck::*;
type World = (Arc<ListProblem<i32>>, ListAssignment<i32>);
fn span(entity: usize, start: usize, end: usize) -> ListRange {
    ListRange { entity, start, end }
}
fn pos(entity: usize, index: usize) -> Option<ListPosition> {
    Some(ListPosition { entity, index })
}
fn limits(reads: usize, branches: usize) -> SubListChangeSourceLimits {
    SubListChangeSourceLimits {
        reads_per_side: NonZeroUsize::new(reads).unwrap(),
        spans_per_item: NonZeroUsize::new(branches).unwrap(),
    }
}
struct Spans {
    values: Vec<ListRange>,
    reads: Rc<Cell<usize>>,
    events: Rc<RefCell<Vec<&'static str>>>,
}
impl Spans {
    fn new(values: Vec<ListRange>) -> Self {
        Self {
            values,
            reads: Rc::new(Cell::new(0)),
            events: Rc::new(RefCell::new(vec![])),
        }
    }
}
impl SelectionSource<World, ListRange> for Spans {
    fn event(&mut self, _: SelectionLifecycleEvent, _: &World) {
        self.events.borrow_mut().push("span");
    }
    fn size(&mut self, _: &World) -> Option<usize> {
        Some(self.values.len())
    }
    fn open<'a>(&'a mut self, _: &'a World) -> Box<dyn Iterator<Item = ListRange> + 'a>
    where
        ListRange: 'a,
    {
        let reads = self.reads.clone();
        Box::new(
            self.values
                .iter()
                .copied()
                .inspect(move |_| reads.set(reads.get() + 1)),
        )
    }
}
fn world(optional: bool) -> World {
    let p = ListProblem::new(
        vec![0, 1, 2, 3, 4],
        vec![
            ListEntity::new(vec![0, 1, 2, 3]).with_pinned_prefix(1),
            ListEntity::new(vec![4]).with_value_range(vec![1, 2, 4]),
            ListEntity::new(vec![]).with_pinned(true),
        ],
    )
    .unwrap();
    let p = Arc::new(if optional { p.allowing_unassigned() } else { p });
    let a = p.initial_assignment();
    (p, a)
}
#[test]
fn destinations_match_transactional_reference_and_retain_span_without_lookahead() {
    let c = world(true);
    let spans = vec![
        span(0, 0, 2),
        span(99, 0, 1),
        span(0, 2, 2),
        span(0, 1, 3),
        span(0, 2, 4),
    ];
    let source = Spans::new(spans.clone());
    let reads = source.reads.clone();
    let opened = Cell::new(0);
    let draws = Cell::new(0);
    let destinations = vec![pos(0, 2), pos(0, 4), pos(1, 1), pos(2, 0), pos(99, 0), None];
    let mut s = SubListChangeSource::correlated_with_draw(
        source,
        |_: &World, _: ListRange| {
            opened.set(opened.get() + 1);
            destinations.clone().into_iter()
        },
        limits(8, 8),
        true,
        |bound| {
            assert_eq!(bound, 2);
            draws.set(draws.get() + 1);
            1
        },
    );
    let mut cursor = s.open(&c);
    assert_eq!((reads.get(), opened.get(), draws.get()), (0, 0, 0));
    let first = cursor.next().unwrap();
    assert_eq!((reads.get(), opened.get(), draws.get()), (4, 1, 2));
    let second = cursor.next().unwrap();
    assert_eq!((reads.get(), opened.get(), draws.get()), (4, 1, 3));
    let mut actual = vec![first, second];
    actual.extend(cursor);
    let mut expected = vec![];
    for source in [span(0, 1, 3), span(0, 2, 4)] {
        for destination in &destinations {
            if let Some(m) = (SubListChangeProposal {
                source,
                destination: *destination,
                reverse: destination.is_some(),
            })
            .into_move()
            {
                let mut after = c.1.clone();
                if let Ok(guard) = c.0.apply(&mut after, &m) {
                    guard.commit();
                } else {
                    continue;
                }
                if after != c.1 {
                    expected.push(m);
                }
            }
        }
    }
    assert_eq!(actual, expected);
    assert_eq!(opened.get(), 2);
    assert_eq!(draws.get(), 10);
    assert!(
        actual
            .iter()
            .any(|m| matches!(m, ListMove::Unassign { .. }))
    );
}
#[test]
fn rejected_and_empty_branches_advance_with_finite_budgets() {
    let c = world(false);
    let calls = Cell::new(0);
    let polls = Rc::new(Cell::new(0));
    let p = polls.clone();
    let mut source = SubListChangeSource::correlated(
        Spans::new(vec![span(0, 1, 2), span(0, 2, 3)]),
        move |_: &World, s: ListRange| {
            let p = p.clone();
            std::iter::repeat_with(move || {
                p.set(p.get() + 1);
                if s.start == 1 { None } else { pos(1, 1) }
            })
        },
        limits(3, 2),
        false,
        0,
    );
    let mut cursor = source.open(&c);
    assert!(cursor.next().is_some());
    assert_eq!(polls.get(), 4);
    assert!(cursor.next().is_some());
    assert_eq!(polls.get(), 5);
    drop(cursor);
    let mut rejected = SubListChangeSource::correlated(
        Spans::new(vec![span(0, 1, 2); 100]),
        |_: &World, _: ListRange| {
            calls.set(calls.get() + 1);
            std::iter::repeat(None)
        },
        limits(3, 2),
        false,
        0,
    );
    let mut cursor = rejected.open(&c);
    assert!(cursor.next().is_none());
    assert!(cursor.next().is_none());
    assert_eq!(calls.get(), 2);
    let mut empty = SubListChangeSource::correlated(
        Spans::new(vec![span(0, 1, 2), span(0, 2, 3)]),
        |_: &World, s: ListRange| {
            if s.start == 1 {
                vec![]
            } else {
                vec![pos(1, 1)]
            }
            .into_iter()
        },
        limits(3, 2),
        false,
        0,
    );
    assert!(empty.open(&c).next().is_some());
    let mut invalid = SubListChangeSource::correlated(
        Spans::new(vec![span(0, 0, 1); 100]),
        |_: &World, _: ListRange| {
            panic!("pinned span factory called");
            #[allow(unreachable_code)]
            std::iter::empty()
        },
        limits(3, 2),
        false,
        0,
    );
    assert!(invalid.open(&c).next().is_none());
    assert_eq!(invalid.left().reads.get(), 3);
}
struct Borrowed(Rc<RefCell<Vec<&'static str>>>);
impl SubListDestinationFactory<World> for Borrowed {
    fn event(&mut self, _: SelectionLifecycleEvent, _: &World) {
        self.0.borrow_mut().push("factory");
    }
    fn open<'a>(
        &mut self,
        c: &'a World,
        s: ListRange,
    ) -> Box<dyn Iterator<Item = Option<ListPosition>> + 'a> {
        Box::new(
            c.1.lists()
                .iter()
                .enumerate()
                .filter(move |(e, _)| *e != s.entity)
                .map(|(e, row)| pos(e, row.len())),
        )
    }
}
#[test]
fn cursor_borrows_context_and_lifecycle_runs_in_order() {
    let c = world(false);
    let spans = Spans::new(vec![span(0, 1, 3)]);
    let events = spans.events.clone();
    let mut s =
        SubListChangeSource::correlated(spans, Borrowed(events.clone()), limits(4, 4), false, 0);
    s.event(SelectionLifecycleEvent::SolveStarted, &c);
    s.event(SelectionLifecycleEvent::PhaseStarted, &c);
    assert_eq!(*events.borrow(), ["span", "factory", "span", "factory"]);
    let moves = s.open(&c).collect::<Vec<_>>();
    assert_eq!(moves.len(), 1);
    assert_eq!(
        moves[0],
        SubListChangeProposal {
            source: span(0, 1, 3),
            destination: pos(1, 1),
            reverse: false
        }
        .into_move()
        .unwrap()
    );
}
struct Bomb {
    kind: usize,
    at: usize,
    armed: Rc<Cell<bool>>,
}
impl Iterator for Bomb {
    type Item = Option<ListPosition>;
    fn next(&mut self) -> Option<Self::Item> {
        self.at += 1;
        if self.kind == 1 && self.at == 2 && self.armed.replace(false) {
            panic!("destination poll")
        };
        (self.at <= 2).then(|| pos(1, 1))
    }
}
impl Drop for Bomb {
    fn drop(&mut self) {
        if self.kind == 2 && self.armed.replace(false) {
            panic!("destination drop")
        }
    }
}
#[test]
fn factory_poll_drop_and_draw_failures_fuse_and_fresh_open_recovers() {
    for kind in 0..4 {
        let c = world(false);
        let armed = Rc::new(Cell::new(true));
        let a = armed.clone();
        let drawarm = armed.clone();
        let calls = Cell::new(0);
        let mut s = SubListChangeSource::correlated_with_draw(
            Spans::new(vec![span(0, 1, 3); 2]),
            |_: &World, _: ListRange| {
                calls.set(calls.get() + 1);
                if kind == 0 && a.replace(false) {
                    panic!("factory")
                };
                Bomb {
                    kind,
                    at: 0,
                    armed: a.clone(),
                }
            },
            limits(4, 4),
            true,
            move |_| {
                if kind == 3 && drawarm.replace(false) {
                    2
                } else {
                    0
                }
            },
        );
        let mut cursor = s.open(&c);
        if kind == 1 || kind == 2 {
            assert!(cursor.next().is_some());
        }
        if kind == 2 {
            assert!(cursor.next().is_some());
        }
        assert!(catch_unwind(AssertUnwindSafe(|| cursor.next())).is_err());
        let before = calls.get();
        assert!(cursor.next().is_none());
        assert_eq!(calls.get(), before);
        drop(cursor);
        armed.set(false);
        assert!(s.open(&c).next().is_some());
    }
}
struct Blocks;
impl SelectionSource<World, ListRange> for Blocks {
    fn size(&mut self, _: &World) -> Option<usize> {
        None
    }
    fn open<'a>(&'a mut self, c: &'a World) -> Box<dyn Iterator<Item = ListRange> + 'a>
    where
        ListRange: 'a,
    {
        Box::new(c.1.lists().iter().enumerate().flat_map(|(e, row)| {
            (0..row.len()).step_by(2).filter_map(move |start| {
                (start + 2 <= row.len()).then_some(span(e, start, start + 2))
            })
        }))
    }
}
#[test]
fn configured_provider_routes_whole_blocks_over_multiple_committed_steps() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![
            ListEntity::new(vec![1, 2, 3, 4]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let shared = Arc::new(p.clone());
    let seen = Rc::new(RefCell::new(vec![]));
    let trace = seen.clone();
    let source = SubListChangeSource::correlated(
        Blocks,
        move |c: &World, s: ListRange| {
            let block = &c.1.lists()[s.entity][s.start..s.end];
            trace.borrow_mut().push((block.to_vec(), s.entity));
            let target = if block[0] <= 2 { 1 } else { 2 };
            if s.entity == target {
                vec![]
            } else {
                vec![pos(target, c.1.lists()[target].len())]
            }
            .into_iter()
        },
        limits(8, 8),
        false,
        0,
    );
    let configured = SelectionBuilder::new(SelectionConfig {
        order: Some(SelectionOrder::Original),
        ..Default::default()
    })
    .build(ConfiguredSelectionPolicy::default(), |_| source)
    .unwrap();
    let mut provider = ConfiguredMoveProvider::new(
        configured,
        ConfiguredProviderConfig {
            maximum_draws: 16,
            seed: 17,
        },
        move |_: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| (shared.clone(), a.clone()),
        |_: &ListProblem<i32>, _: &ListAssignment<i32>, _: u64, m: &ListMove<i32>| Some(m.clone()),
    );
    let mut network = ListConstraintNetwork::new(3, 4);
    network
        .add_constraint(
            "block routes",
            vec![
                ListScope::Entity(0),
                ListScope::Entity(1),
                ListScope::Entity(2),
            ],
            |facts| {
                let [
                    ListFact::Entity(a),
                    ListFact::Entity(b),
                    ListFact::Entity(c),
                ] = facts
                else {
                    panic!()
                };
                SimpleScore::new(
                    a.iter().map(|v| i64::from(*v)).sum::<i64>()
                        + 10 * b.iter().map(|v| i64::from(*v)).sum::<i64>()
                        + 100 * c.iter().map(|v| i64::from(*v)).sum::<i64>(),
                )
            },
        )
        .unwrap();
    let result = list_local_search_from_with_provider(
        &p,
        &network,
        p.initial_assignment(),
        LocalSearchConfig {
            max_steps: 4,
            selection: MoveSelection::BestAccepted,
            acceptance: Acceptance::HillClimbing,
            assert_scores: true,
            ..Default::default()
        },
        &mut provider,
    )
    .unwrap();
    let best = result.best.unwrap();
    assert_eq!(best.score, SimpleScore::new(730));
    assert_eq!(best.assignment.lists(), &[vec![], vec![1, 2], vec![3, 4]]);
    assert!(seen.borrow().contains(&(vec![3, 4], 2)));
    assert!(seen.borrow().contains(&(vec![1, 2], 1)));
    assert_eq!(seen.borrow().len(), 6);
}

#[test]
fn cyclic_destinations_are_rejected_before_valid_later_destinations() {
    let p = Arc::new(
        ListProblem::new(
            vec![0, 1],
            vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![])],
        )
        .unwrap()
        .with_dependencies(vec![vec![], vec![0]])
        .unwrap(),
    );
    let c = (p.clone(), p.initial_assignment());
    let mut source = SubListChangeSource::correlated(
        Spans::new(vec![span(0, 1, 2)]),
        |_: &World, _: ListRange| vec![pos(0, 0), pos(1, 0)].into_iter(),
        limits(4, 4),
        false,
        0,
    );
    let moves = source.open(&c).collect::<Vec<_>>();
    assert_eq!(
        moves,
        vec![
            SubListChangeProposal {
                source: span(0, 1, 2),
                destination: pos(1, 0),
                reverse: false
            }
            .into_move()
            .unwrap()
        ]
    );
    let cycle = SubListChangeProposal {
        source: span(0, 1, 2),
        destination: pos(0, 0),
        reverse: false,
    }
    .into_move()
    .unwrap();
    assert!(matches!(
        p.is_move_doable(&c.1, &cycle),
        Err(ListError::DependencyCycle { .. })
    ));
    assert_eq!(c.1.lists(), &[vec![0, 1], vec![]]);
}
