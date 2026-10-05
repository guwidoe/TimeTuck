//! Original live-position and lazy provider-composition integration checks.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use timetuck::*;

fn positions(p: &ListProblem<i32>, a: &ListAssignment<i32>) -> Vec<Option<ListPosition>> {
    let mut out = vec![None];
    for (entity, list) in a.lists().iter().enumerate() {
        for index in 0..=list.len() {
            if !p.entities()[entity].is_pinned() && index >= p.entities()[entity].pinned_prefix() {
                out.push(Some(ListPosition { entity, index }));
            }
        }
    }
    out
}
#[test]
fn live_destinations_follow_all_single_moves_rollback_and_held_snapshots() {
    let p = ListProblem::new(
        (0..6).collect(),
        vec![
            ListEntity::new(vec![0]).with_pinned(true),
            ListEntity::new(vec![1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![]).with_value_range(vec![2, 3, 4, 5]),
            ListEntity::new(vec![4, 5]).with_pinned_prefix(2),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = ListStreamSources::new(&mut b);
    let data = list_destination_dataset(&mut b, &sources, &p, true).unwrap();
    let mut d =
        ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    let read = |g: &StreamGraph<'_, SimpleScore>| {
        data.iter(g)
            .unwrap()
            .map(|p| p.copied())
            .collect::<Vec<_>>()
    };
    let before = data.snapshot(d.graph()).unwrap();
    assert_eq!(before.len(), 5);
    for movement in list_moves(&p, d.assignment(), Neighborhood::ChangeAndSwap)
        .unwrap()
        .collect::<Vec<_>>()
    {
        let trial = d.apply(&movement).unwrap();
        let expected = positions(&p, trial.assignment());
        assert_eq!(read(trial.graph()), expected);
        assert_eq!(
            list_destinations(&p, trial.assignment(), true)
                .unwrap()
                .collect::<Vec<_>>(),
            expected
        );
        drop(trial);
        assert_eq!(read(d.graph()), before.iter().copied().collect::<Vec<_>>());
    }
    d.apply(&ListMove::Assign {
        value: 3,
        destination: ListPosition {
            entity: 2,
            index: 0,
        },
    })
    .unwrap()
    .commit();
    assert_eq!(read(d.graph()), positions(&p, d.assignment()));
    assert_eq!(data.len(d.graph()).unwrap(), 6);
    assert_eq!(before.len(), 5);
    d.assert_consistent().unwrap();
}
#[test]
fn destination_validation_empty_models_and_fully_pinned_models() {
    let p = ListProblem::<i32>::new(vec![], vec![]).unwrap();
    assert_eq!(
        list_destinations(&p, &p.initial_assignment(), false)
            .unwrap()
            .count(),
        0
    );
    assert!(matches!(
        list_destinations(&p, &p.initial_assignment(), true),
        Err(ListError::UnassignmentNotAllowed)
    ));
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = ListStreamSources::new(&mut b);
    assert!(matches!(
        list_destination_dataset(&mut b, &sources, &p, true),
        Err(ListStreamScoringError::Model(
            ListError::UnassignmentNotAllowed
        ))
    ));
    assert!(list_destinations(&p, &ListAssignment::new(vec![vec![]]), false).is_err());
    for p in [
        p.allowing_unassigned(),
        ListProblem::new(vec![1], vec![ListEntity::new(vec![1]).with_pinned(true)])
            .unwrap()
            .allowing_unassigned(),
    ] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let sources = ListStreamSources::new(&mut b);
        let data = list_destination_dataset(&mut b, &sources, &p, true).unwrap();
        let director =
            ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
        assert_eq!(
            data.iter(director.graph()).unwrap().collect::<Vec<_>>(),
            [None]
        );
    }
}

type Event = (usize, usize, u64);
struct Recorded {
    id: usize,
    count: usize,
    calls: Rc<RefCell<Vec<Event>>>,
    drops: Rc<Cell<usize>>,
}
struct Cursor {
    value: Event,
    remaining: usize,
    drops: Rc<Cell<usize>>,
}
impl Iterator for Cursor {
    type Item = Event;
    fn next(&mut self) -> Option<Event> {
        if self.remaining == 0 {
            None
        } else {
            self.remaining -= 1;
            Some(self.value)
        }
    }
}
impl Drop for Cursor {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
impl MoveProvider<(), usize, Event> for Recorded {
    fn moves<'a>(
        &'a mut self,
        _: &'a (),
        a: &'a usize,
        step: u64,
    ) -> Box<dyn Iterator<Item = Event> + 'a>
    where
        Event: 'a,
    {
        assert_ne!(self.id, 99, "disabled provider called");
        let value = (self.id, *a, step);
        self.calls.borrow_mut().push(value);
        Box::new(Cursor {
            value,
            remaining: self.count,
            drops: self.drops.clone(),
        })
    }
}
fn combine<'p, P, A, M>(
    a: &'p mut dyn MoveProvider<P, A, M>,
    b: &'p mut dyn MoveProvider<P, A, M>,
    order: UnionOrder,
) -> ProviderUnion<'p, P, A, M> {
    ProviderUnion::new(vec![(a, 1), (b, 3)], order).unwrap()
}
#[test]
fn child_factories_are_lazy_disabled_children_are_inert_and_drop_closes_cursors() {
    let calls = Rc::new(RefCell::new(vec![]));
    let drops = Rc::new(Cell::new(0));
    let mut left = Recorded {
        id: 0,
        count: 2,
        calls: calls.clone(),
        drops: drops.clone(),
    };
    let mut right = Recorded {
        id: 1,
        count: 1,
        calls: calls.clone(),
        drops: drops.clone(),
    };
    let mut disabled = Recorded {
        id: 99,
        count: 1,
        calls: calls.clone(),
        drops: drops.clone(),
    };
    let children: Vec<WeightedMoveProvider<'_, (), usize, Event>> =
        vec![(&mut left, 1), (&mut disabled, 0), (&mut right, 1)];
    let mut union = ProviderUnion::new(children, UnionOrder::Sequential).unwrap();
    assert!(calls.borrow().is_empty());
    let mut it = union.moves(&(), &4, 7);
    assert!(calls.borrow().is_empty());
    assert_eq!(it.next(), Some((0, 4, 7)));
    assert_eq!(*calls.borrow(), [(0, 4, 7)]);
    drop(it);
    assert_eq!(drops.get(), 1);
    assert_eq!(
        union.moves(&(), &9, 8).collect::<Vec<_>>(),
        [(0, 9, 8), (0, 9, 8), (1, 9, 8)]
    );
    assert_eq!(drops.get(), 3);
    assert_eq!(*calls.borrow(), [(0, 4, 7), (0, 9, 8), (1, 9, 8)]);
}
#[test]
fn zero_weight_unions_do_no_work_and_nested_unions_preserve_duplicates() {
    let mut bad = |_: &(), _: &(), _: u64| -> Vec<i32> { panic!("disabled") };
    let children: Vec<WeightedMoveProvider<'_, (), (), i32>> = vec![(&mut bad, 0)];
    let mut disabled = ProviderUnion::new(children, UnionOrder::Random { seed: 1 }).unwrap();
    assert_eq!(disabled.moves(&(), &(), 0).next(), None);
    let mut a = |_: &(), _: &(), _: u64| vec![1, 1];
    let mut b = |_: &(), _: &(), _: u64| vec![2];
    let mut c = |_: &(), _: &(), _: u64| vec![3];
    let mut inner = combine(&mut a, &mut b, UnionOrder::Sequential);
    let mut outer = combine(&mut inner, &mut c, UnionOrder::Sequential);
    assert_eq!(outer.moves(&(), &(), 5).collect::<Vec<_>>(), [1, 1, 2, 3]);
}
#[test]
fn random_provider_weights_are_repeatable_and_seeded_by_completed_step() {
    let run = |seed, step| {
        let calls = Rc::new(RefCell::new(vec![]));
        let drops = Rc::new(Cell::new(0));
        let mut a = Recorded {
            id: 0,
            count: usize::MAX,
            calls: calls.clone(),
            drops: drops.clone(),
        };
        let mut b = Recorded {
            id: 1,
            count: usize::MAX,
            calls: calls.clone(),
            drops: drops.clone(),
        };
        let mut union = combine(&mut a, &mut b, UnionOrder::Random { seed });
        let result = union
            .moves(&(), &17, step)
            .take(2000)
            .map(|(id, a, seen_step)| {
                assert_eq!((a, seen_step), (17, step));
                id
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.borrow().len(), 2);
        assert_eq!(drops.get(), 2);
        result
    };
    let first = run(42, 0);
    assert_eq!(first, run(42, 0));
    assert_ne!(first, run(42, 1));
    assert_eq!(run(u64::MAX, 1), run(0, 0));
    assert!((400..600).contains(&first.iter().filter(|&&id| id == 0).count()));
}

fn basic() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap()
}
fn lists() -> ListProblem<i32> {
    ListProblem::new(
        vec![7],
        vec![
            ListEntity::new(vec![7]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
}
fn bm(a: &Assignment<i32>, target: i32) -> Vec<Move<i32>> {
    (a.values()[0] == Some(target - 1))
        .then_some(Move::Change {
            variable: 0,
            value: Some(target),
        })
        .into_iter()
        .collect()
}
fn lm(a: &ListAssignment<i32>, target: usize) -> Vec<ListMove<i32>> {
    let source = a.location(&7).unwrap();
    (source.entity + 1 == target)
        .then_some(ListMove::Change {
            source,
            destination: ListPosition {
                entity: target,
                index: 0,
            },
        })
        .into_iter()
        .collect()
}
fn mm(a: &MixedAssignment<i32, i32>, target: i32) -> Vec<MixedMove<i32, i32>> {
    bm(a.basic(), target)
        .into_iter()
        .zip(lm(a.lists(), target as usize))
        .map(|(basic, lists)| MixedMove { basic, lists })
        .collect()
}
#[test]
fn composed_providers_solve_in_all_six_paths_with_both_union_orders() {
    let p = basic();
    let l = lists();
    let m = MixedProblem::new(p.clone(), l.clone());
    let mut bn = ConstraintNetwork::new(1);
    bn.add_constraint("value", vec![0], |v| {
        SimpleScore::new(i64::from(*v[0].unwrap()))
    })
    .unwrap();
    let mut ln = ListConstraintNetwork::new(3, 1);
    ln.add_constraint("owner", vec![ListScope::Value(0)], |f| {
        let ListFact::Value { element, .. } = &f[0] else {
            unreachable!()
        };
        SimpleScore::new(element.as_ref().unwrap().position.entity as i64)
    })
    .unwrap();
    let mut mn = MixedConstraintNetwork::new(1, 3, 1);
    mn.add_constraint(
        "both",
        vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
        |f| {
            let (MixedFact::Basic(v), MixedFact::ListValue { element, .. }) = (&f[0], &f[1]) else {
                unreachable!()
            };
            SimpleScore::new(
                i64::from(*v.unwrap()) + element.as_ref().unwrap().position.entity as i64,
            )
        },
    )
    .unwrap();
    let config = LocalSearchConfig {
        max_steps: 3,
        max_move_evaluations: Some(20),
        assert_scores: true,
        ..Default::default()
    };
    for order in [UnionOrder::Sequential, UnionOrder::Random { seed: 8 }] {
        for stream in [false, true] {
            let mut left = |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| bm(a, 1);
            let mut right = |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| bm(a, 2);
            let mut union = combine(&mut left, &mut right, order);
            let r = if stream {
                let mut b = StreamGraphBuilder::new();
                let s = b.source::<usize, (usize, Option<i32>)>();
                b.score("value", s.port(), |(_, v)| {
                    SimpleScore::new(i64::from(v.unwrap()))
                })
                .unwrap();
                stream_local_search_from_with_provider(
                    &p,
                    b.build(),
                    s,
                    p.initial_assignment(),
                    config,
                    &mut union,
                )
                .unwrap()
            } else {
                local_search_from_with_provider(&p, &bn, p.initial_assignment(), config, &mut union)
                    .unwrap()
            };
            assert_eq!(r.best.unwrap().score, SimpleScore::new(2));
            assert_eq!(r.completed_steps, 2);
            let mut left = |_: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| lm(a, 1);
            let mut right = |_: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| lm(a, 2);
            let mut union = combine(&mut left, &mut right, order);
            let r = if stream {
                let mut b = StreamGraphBuilder::new();
                let s = ListStreamSources::new(&mut b);
                b.score("owner", s.values.port(), |r| {
                    SimpleScore::new(r.position.unwrap().entity as i64)
                })
                .unwrap();
                list_stream_local_search_from_with_provider(
                    &l,
                    b.build(),
                    s,
                    l.initial_assignment(),
                    config,
                    &mut union,
                )
                .unwrap()
            } else {
                list_local_search_from_with_provider(
                    &l,
                    &ln,
                    l.initial_assignment(),
                    config,
                    &mut union,
                )
                .unwrap()
            };
            assert_eq!(r.best.unwrap().score, SimpleScore::new(2));
            assert_eq!(r.completed_steps, 2);
            let mut left =
                |_: &MixedProblem<i32, i32>, a: &MixedAssignment<i32, i32>, _: u64| mm(a, 1);
            let mut right =
                |_: &MixedProblem<i32, i32>, a: &MixedAssignment<i32, i32>, _: u64| mm(a, 2);
            let mut union = combine(&mut left, &mut right, order);
            let mc = MixedLocalSearchConfig {
                search: config,
                joint_moves: false,
            };
            let r = if stream {
                let mut b = StreamGraphBuilder::new();
                let s = MixedStreamSources::new(&mut b);
                b.score("value", s.basic.port(), |(_, v): &(usize, Option<i32>)| {
                    SimpleScore::new(i64::from(v.unwrap()))
                })
                .unwrap();
                b.score("owner", s.lists.values.port(), |r| {
                    SimpleScore::new(r.position.unwrap().entity as i64)
                })
                .unwrap();
                mixed_stream_local_search_from_with_provider(
                    &m,
                    b.build(),
                    s,
                    m.initial_assignment(),
                    mc,
                    &mut union,
                )
                .unwrap()
            } else {
                mixed_local_search_from_with_provider(
                    &m,
                    &mn,
                    m.initial_assignment(),
                    mc,
                    &mut union,
                )
                .unwrap()
            };
            assert_eq!(r.best.unwrap().score, SimpleScore::new(4));
            assert_eq!(r.completed_steps, 2);
        }
    }
}
