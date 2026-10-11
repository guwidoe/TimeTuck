//! Original live-child composition, work-bound and provider integration checks.
use std::{
    cell::{Cell, RefCell},
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
fn limit(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn values(row: DependentRow<i32>) -> Vec<Option<i32>> {
    row.into_iter().map(|x| x.map(|v| *v)).collect()
}
#[test]
fn lazy_live_cursors_restart_independently_without_starving_peers() {
    let calls = Rc::new(RefCell::new(vec![]));
    let trace = calls.clone();
    let mut source = StreamingSelection::new(
        3,
        false,
        limit(2),
        move |_: &(), child, prefix: &DependentPrefix<i32>| -> Box<dyn Iterator<Item = i32>> {
            trace.borrow_mut().push((
                child,
                prefix
                    .iter()
                    .map(|x| x.as_ref().map(|v| **v))
                    .collect::<Vec<_>>(),
            ));
            match child {
                0 => Box::new(0..3),
                1 => {
                    let n = **prefix[0].as_ref().unwrap() * 10;
                    Box::new(n..n + 2)
                }
                _ => Box::new(std::iter::repeat(99)),
            }
        },
        |_, _: &()| {},
    );
    assert!(source.is_never_ending());
    assert_eq!(source.size(&()), None);
    let mut cursor = source.open(&());
    assert!(calls.borrow().is_empty());
    assert_eq!(
        (0..5)
            .map(|_| values(cursor.next().unwrap()))
            .collect::<Vec<_>>(),
        [
            vec![Some(0), Some(0), Some(99)],
            vec![Some(1), Some(1), Some(99)],
            vec![Some(2), Some(20), Some(99)],
            vec![Some(0), Some(21), Some(99)],
            vec![Some(1), Some(10), Some(99)],
        ]
    );
    assert_eq!(
        calls.borrow().iter().map(|(c, _)| *c).collect::<Vec<_>>(),
        [0, 1, 2, 1, 0, 1]
    );
    drop(cursor);
    assert_eq!(calls.borrow().len(), 6);
    assert_eq!(
        values(source.open(&()).next().unwrap()),
        [Some(0), Some(0), Some(99)]
    );
}
#[test]
fn empty_holes_recheck_dependencies_and_rejected_work_is_bounded() {
    for ignore in [false, true] {
        let opened = Rc::new(Cell::new(0));
        let count = opened.clone();
        let mut source = StreamingSelection::new(
            2,
            ignore,
            limit(3),
            move |_: &(), child, prefix: &DependentPrefix<i32>| {
                count.set(count.get() + 1);
                if child == 0 {
                    vec![0, 1].into_iter()
                } else if **prefix[0].as_ref().unwrap() == 1 {
                    vec![10].into_iter()
                } else {
                    vec![].into_iter()
                }
            },
            |_, _: &()| {},
        );
        let mut cursor = source.open(&());
        if ignore {
            assert_eq!(values(cursor.next().unwrap()), [Some(0), None]);
        }
        assert_eq!(values(cursor.next().unwrap()), [Some(1), Some(10)]);
        assert!(opened.get() <= 3);
    }
    for ignore in [false, true] {
        let opened = Rc::new(Cell::new(0));
        let count = opened.clone();
        let mut source = StreamingSelection::new(
            3,
            ignore,
            limit(4),
            move |_: &(), _, _: &DependentPrefix<i32>| {
                count.set(count.get() + 1);
                std::iter::empty()
            },
            |_, _: &()| {},
        );
        let mut cursor = source.open(&());
        assert!(cursor.next().is_none());
        assert!(cursor.next().is_none());
        assert_eq!(opened.get(), if ignore { 12 } else { 4 });
    }
    let mut source = StreamingSelection::new(
        0,
        false,
        limit(1),
        |_: &(), _, _: &DependentPrefix<i32>| -> std::iter::Empty<i32> { panic!("no factory") },
        |_, _: &()| {},
    );
    assert!(!source.is_never_ending());
    assert_eq!(source.size(&()), Some(0));
    assert!(source.open(&()).next().is_none());
}
#[test]
fn cancellation_and_caught_factory_or_poll_panics_fuse_only_one_opening() {
    for at_factory in [false, true] {
        let fail = Rc::new(Cell::new(true));
        let flag = fail.clone();
        let mut source = StreamingSelection::new(
            2,
            false,
            limit(2),
            move |_: &(), child, _: &DependentPrefix<i32>| {
                assert!(!(at_factory && child == 1 && flag.get()), "factory panic");
                let flag = flag.clone();
                std::iter::from_fn(move || {
                    assert!(!(!at_factory && child == 1 && flag.get()), "poll panic");
                    Some(child as i32)
                })
            },
            |_, _: &()| {},
        );
        let mut cursor = source.open(&());
        assert!(catch_unwind(AssertUnwindSafe(|| cursor.next())).is_err());
        fail.set(false);
        assert!(cursor.next().is_none());
        drop(cursor);
        assert_eq!(values(source.open(&()).next().unwrap()), [Some(0), Some(1)]);
    }
    let stopped = Rc::new(Cell::new(false));
    let flag = stopped.clone();
    let stop = stopped.clone();
    let mut source = StreamingSelection::new(
        2,
        false,
        limit(2),
        move |_: &(), child, _: &DependentPrefix<i32>| {
            if child == 0 {
                flag.set(true);
            }
            std::iter::once(1)
        },
        |_, _: &()| {},
    )
    .with_termination(move || stop.get());
    let mut cursor = source.open(&());
    assert!(cursor.next().is_none());
    stopped.set(false);
    assert!(cursor.next().is_none());
}
#[test]
fn configured_provider_consumes_correlated_composite_moves_under_draw_limit() {
    let events = Rc::new(RefCell::new(vec![]));
    let observed = events.clone();
    let source = StreamingSelection::new(
        2,
        false,
        limit(2),
        |_: &(), child, prefix: &DependentPrefix<Move<i32>>| -> std::vec::IntoIter<Move<i32>> {
            if child == 0 {
                vec![
                    Move::Change {
                        variable: 0,
                        value: Some(7),
                    },
                    Move::Change {
                        variable: 2,
                        value: Some(8),
                    },
                ]
                .into_iter()
            } else {
                let Move::Change { variable, value } = **prefix[0].as_ref().unwrap() else {
                    unreachable!()
                };
                vec![Move::Change {
                    variable: variable + 1,
                    value,
                }]
                .into_iter()
            }
        },
        move |event, _: &()| observed.borrow_mut().push(event),
    );
    let selection = SelectionBuilder::new(SelectionConfig {
        order: Some(SelectionOrder::Original),
        ..Default::default()
    })
    .build(ConfiguredSelectionPolicy::default(), |_| source)
    .unwrap();
    let mut provider = ConfiguredMoveProvider::new(
        selection,
        ConfiguredProviderConfig {
            maximum_draws: 3,
            seed: 0,
        },
        |_: &(), _: &(), _: u64| (),
        |_: &(), _: &(), _: u64, row: &DependentRow<Move<i32>>| {
            Some(Move::Composite(
                row.iter().flatten().map(|v| (**v).clone()).collect(),
            ))
        },
    );
    <_ as MoveProvider<(), (), Move<i32>>>::event(&mut provider, MoveProviderEvent::PhaseStarted);
    <_ as MoveProvider<(), (), Move<i32>>>::event(&mut provider, MoveProviderEvent::StepStarted);
    let moves = provider.try_moves(&(), &(), 0).unwrap().collect::<Vec<_>>();
    assert_eq!(moves.len(), 3);
    assert_eq!(
        moves[0],
        Move::Composite(vec![
            Move::Change {
                variable: 0,
                value: Some(7)
            },
            Move::Change {
                variable: 1,
                value: Some(7)
            }
        ])
    );
    assert_eq!(
        moves[1],
        Move::Composite(vec![
            Move::Change {
                variable: 2,
                value: Some(8)
            },
            Move::Change {
                variable: 3,
                value: Some(8)
            }
        ])
    );
    assert_eq!(moves[2], moves[0]);
    let problem = FiniteProblem::new(
        (0..4)
            .map(|_| Variable::new(vec![0, 7, 8]).with_initial(0))
            .collect(),
    )
    .unwrap();
    let mut assignment = problem.initial_assignment();
    for movement in &moves {
        problem.apply(&mut assignment, movement).unwrap().commit();
    }
    assert_eq!(assignment.values(), &[Some(7), Some(7), Some(8), Some(8)]);

    assert!(
        events
            .borrow()
            .contains(&SelectionLifecycleEvent::StepStarted)
    );
    provider.reset().unwrap();
}

#[test]
fn cursors_may_borrow_external_data_and_payloads_need_not_clone() {
    struct Payload(i32);
    let data = [3, 5];
    let borrowed = &data;
    let mut source = StreamingSelection::new(
        1,
        false,
        limit(1),
        move |_: &(), _, _: &DependentPrefix<Payload>| borrowed.iter().map(|n| Payload(*n)),
        |_, _: &()| {},
    );
    let rows = source.open(&()).take(3).collect::<Vec<_>>();
    assert_eq!(
        rows.iter()
            .map(|r| r[0].as_ref().unwrap().0)
            .collect::<Vec<_>>(),
        [3, 5, 3]
    );
}
