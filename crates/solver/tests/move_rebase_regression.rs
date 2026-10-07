// Original rebasing regressions. No upstream method credit.
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use timetuck::*;
fn change(variable: usize, value: Option<u8>) -> Move<u8> {
    Move::Change { variable, value }
}
#[test]
fn basic_rebased_execution_commutes_with_assignment_mapping() {
    let source = FiniteProblem::new(
        (0..4)
            .map(|i| {
                Variable::new((0..4u8).collect())
                    .with_initial(i)
                    .allowing_unassigned()
            })
            .collect(),
    )
    .unwrap();
    let target = FiniteProblem::new(
        (0..4)
            .rev()
            .map(|i| {
                Variable::new((10..14u16).collect())
                    .with_initial(i + 10)
                    .allowing_unassigned()
            })
            .collect(),
    )
    .unwrap();
    let mut leaves = vec![Move::Composite(vec![])];
    for i in 0..4 {
        for value in [None, Some(0), Some(1), Some(2), Some(3)] {
            leaves.push(change(i, value));
        }
        for j in 0..4 {
            leaves.push(Move::Swap { left: i, right: j });
        }
    }
    for first in &leaves {
        for second in &leaves {
            let movement =
                Move::Composite(vec![first.clone(), Move::Composite(vec![second.clone()])]);
            let mapped = rebase_basic_move(
                &movement,
                |i| Ok::<_, ()>(3 - i),
                |v| Ok(u16::from(*v) + 10),
            )
            .unwrap();
            let mut a = source.initial_assignment();
            let mut b = target.initial_assignment();
            source.apply(&mut a, &movement).unwrap().commit();
            target.apply(&mut b, &mapped).unwrap().commit();
            assert_eq!(
                b.values(),
                a.values()
                    .iter()
                    .rev()
                    .map(|v| v.map(|v| u16::from(v) + 10))
                    .collect::<Vec<_>>()
            );
        }
    }
}
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn span(entity: usize, start: usize, end: usize) -> ListRange {
    ListRange { entity, start, end }
}
fn list_mapping(a: &ListAssignment<u8>) -> Vec<Vec<u16>> {
    a.lists()
        .iter()
        .rev()
        .map(|row| row.iter().map(|v| u16::from(*v) + 10).collect())
        .collect()
}
#[test]
fn every_list_variant_and_child_relative_positions_rebase() {
    let p = ListProblem::new(
        (0..7u8).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2]),
            ListEntity::new(vec![3, 4, 5]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let q = ListProblem::new(
        (10..17u16).collect(),
        vec![
            ListEntity::new(vec![13, 14, 15]),
            ListEntity::new(vec![10, 11, 12]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let leaves = vec![
        ListMove::Assign {
            value: 6,
            destination: pos(0, 1),
        },
        ListMove::Change {
            source: pos(0, 1),
            destination: pos(1, 2),
        },
        ListMove::Swap {
            left: pos(0, 1),
            right: pos(1, 2),
        },
        ListMove::SubListChange {
            entity: 0,
            start: 0,
            end: 2,
            destination: pos(1, 2),
            reverse: true,
        },
        ListMove::SubListChange {
            entity: 1,
            start: 1,
            end: 3,
            destination: pos(1, 0),
            reverse: false,
        },
        ListMove::SubListSwap {
            left: span(0, 0, 2),
            right: span(1, 1, 3),
            reverse: true,
        },
        ListMove::SubListSwap {
            left: span(0, 0, 1),
            right: span(1, 1, 3),
            reverse: false,
        },
        ListMove::Unassign {
            entity: 1,
            start: 0,
            end: 2,
        },
        ListMove::Composite(vec![]),
    ];
    let mut accepted = 0;
    for first in &leaves {
        for second in &leaves {
            let movement = ListMove::Composite(vec![
                first.clone(),
                ListMove::Composite(vec![second.clone()]),
            ]);
            let mapped = rebase_list_move(
                &movement,
                |i| Ok::<_, ()>(1 - i),
                |v| Ok(u16::from(*v) + 10),
            )
            .unwrap();
            let mut a = p.initial_assignment();
            let mut b = q.initial_assignment();
            let source_ok = match p.apply(&mut a, &movement) {
                Ok(trial) => {
                    trial.commit();
                    true
                }
                Err(_) => false,
            };
            let target_ok = match q.apply(&mut b, &mapped) {
                Ok(trial) => {
                    trial.commit();
                    true
                }
                Err(_) => false,
            };
            assert_eq!(source_ok, target_ok);
            assert_eq!(b.lists(), list_mapping(&a));
            accepted += usize::from(source_ok);
        }
    }
    assert!(accepted > 30);
}
#[test]
fn lookup_is_memoized_by_identity_and_namespaces_are_independent() {
    let basic = Move::Composite(vec![
        change(1, Some(2)),
        Move::Composite(vec![]),
        change(1, None),
        change(0, Some(2)),
        Move::Swap { left: 1, right: 0 },
    ]);
    let lists: ListMove<u8> = ListMove::Composite(vec![
        ListMove::Assign {
            value: 2,
            destination: pos(1, 0),
        },
        ListMove::Unassign {
            entity: 1,
            start: 0,
            end: 1,
        },
        ListMove::Assign {
            value: 2,
            destination: pos(0, 0),
        },
    ]);
    let calls = RefCell::new(vec![]);
    let output = rebase_mixed_move(
        &MixedMove {
            basic: basic.clone(),
            lists: lists.clone(),
        },
        |v| {
            calls.borrow_mut().push((0, v));
            Ok::<_, ()>(v + 4)
        },
        |v| {
            calls.borrow_mut().push((1, usize::from(*v)));
            Ok(*v + 10)
        },
        |v| {
            calls.borrow_mut().push((2, v));
            Ok(v + 4)
        },
        |v| {
            calls.borrow_mut().push((3, usize::from(*v)));
            Ok(*v + 10)
        },
    )
    .unwrap();
    let mut counted = BTreeMap::new();
    for call in calls.into_inner() {
        *counted.entry(call).or_insert(0) += 1;
    }
    assert_eq!(
        counted,
        [
            ((0, 1), 1),
            ((0, 0), 1),
            ((1, 2), 1),
            ((2, 1), 1),
            ((2, 0), 1),
            ((3, 2), 1)
        ]
        .into_iter()
        .collect()
    );
    let roundtrip = rebase_mixed_move(
        &output,
        |v| Ok::<_, ()>(v - 4),
        |v| Ok(*v - 10),
        |v| Ok(v - 4),
        |v| Ok(*v - 10),
    )
    .unwrap();
    assert_eq!(roundtrip, MixedMove { basic, lists });
    let identity = Rc::new("same".to_string());
    let movement = Move::Composite(vec![change(0, Some(1)), change(1, Some(1))]);
    let Move::Composite(children) =
        rebase_basic_move(&movement, Ok::<_, ()>, |_| Ok(identity.clone())).unwrap()
    else {
        unreachable!()
    };
    for child in children {
        let Move::Change {
            value: Some(value), ..
        } = child
        else {
            unreachable!()
        };
        assert!(Rc::ptr_eq(&value, &identity));
    }
}
#[test]
fn collisions_lookup_errors_and_panics_do_not_modify_the_source() {
    let movement = Move::Composite(vec![change(0, Some(1)), change(1, Some(2))]);
    let before = movement.clone();
    assert_eq!(
        rebase_basic_move(&movement, |_| Ok::<_, ()>(0), |v| Ok(*v)),
        Err(MoveRebaseError::Collision(
            MoveRebaseNamespace::BasicVariable
        ))
    );
    assert_eq!(
        rebase_basic_move(&movement, Ok::<_, ()>, |_| Ok(0)),
        Err(MoveRebaseError::Collision(MoveRebaseNamespace::BasicValue))
    );
    assert_eq!(
        rebase_basic_move(
            &movement,
            |v| if v == 1 { Err("missing") } else { Ok(v) },
            |v| Ok(*v)
        ),
        Err(MoveRebaseError::Lookup("missing"))
    );
    let panic = std::panic::catch_unwind(|| {
        rebase_basic_move(&movement, Ok::<_, ()>, |_: &u8| -> Result<u8, ()> {
            panic!("lookup")
        })
    });
    assert!(panic.is_err());
    assert_eq!(movement, before);
    let list = ListMove::Composite(vec![
        ListMove::Assign {
            value: 1,
            destination: pos(0, 0),
        },
        ListMove::Assign {
            value: 2,
            destination: pos(1, 0),
        },
    ]);
    assert_eq!(
        rebase_list_move(&list, |_| Ok::<_, ()>(0), |v| Ok(*v)),
        Err(MoveRebaseError::Collision(MoveRebaseNamespace::ListEntity))
    );
    assert_eq!(
        rebase_list_move(&list, Ok::<_, ()>, |_| Ok(0)),
        Err(MoveRebaseError::Collision(MoveRebaseNamespace::ListValue))
    );
}
#[test]
fn mixed_rebased_move_executes_atomically_and_target_validation_still_applies() {
    let make = |pinned| {
        MixedProblem::new(
            FiniteProblem::new(vec![
                Variable::new(vec![10u16, 11])
                    .with_initial(10)
                    .with_pinned(pinned),
            ])
            .unwrap(),
            ListProblem::new(vec![10u16, 11, 12], vec![ListEntity::new(vec![10, 11])])
                .unwrap()
                .allowing_unassigned(),
        )
    };
    let movement = MixedMove {
        basic: change(0, Some(1)),
        lists: ListMove::Composite(vec![
            ListMove::Assign {
                value: 2u8,
                destination: pos(0, 1),
            },
            ListMove::Change {
                source: pos(0, 2),
                destination: pos(0, 0),
            },
        ]),
    };
    let mapped = rebase_mixed_move(
        &movement,
        Ok::<_, ()>,
        |v| Ok(u16::from(*v) + 10),
        Ok,
        |v| Ok(u16::from(*v) + 10),
    )
    .unwrap();
    let p = make(false);
    let mut a = p.initial_assignment();
    let before = a.clone();
    {
        let trial = p.apply(&mut a, &mapped).unwrap();
        assert_eq!(trial.basic().values(), &[Some(11)]);
        assert_eq!(trial.lists().lists(), &[vec![11, 10, 12]]);
    }
    assert_eq!(a, before);
    p.apply(&mut a, &mapped).unwrap().commit();
    assert_eq!(a.basic().values(), &[Some(11)]);
    assert_eq!(a.lists().lists(), &[vec![11, 10, 12]]);
    let pinned = make(true);
    let mut a = pinned.initial_assignment();
    let before = a.clone();
    assert!(pinned.apply(&mut a, &mapped).is_err());
    assert_eq!(a, before);
}
