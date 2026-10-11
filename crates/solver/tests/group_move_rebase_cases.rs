// Adapted from Timefold Community at8b815bc6b5f3eb81feedc94dea0885a93b267762.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
// Native pillars produce atomic composites; rebasing maps explicit variable IDs
// and shared value identities instead of Java score-director object lookup.
use std::rc::Rc;
use timetuck::*;

struct World {
    values: [Rc<String>; 3],
    entities: [Rc<String>; 4],
    assignment: Assignment<Rc<String>>,
}
fn world(destination: bool) -> World {
    let values = ["v1", "v2", "v3"].map(|v| Rc::new(v.to_owned()));
    let identities = ["e1", "e2", "e3", "e4"].map(|v| Rc::new(v.to_owned()));
    let order = if destination {
        [1, 3, 0, 2]
    } else {
        [0, 1, 2, 3]
    };
    let original = [
        Some(values[0].clone()),
        None,
        Some(values[0].clone()),
        Some(values[2].clone()),
    ];
    World {
        values,
        entities: order.map(|i| identities[i].clone()),
        assignment: Assignment::new(order.map(|i| original[i].clone()).to_vec()),
    }
}
fn owner(i: usize) -> usize {
    [2, 0, 3, 1][i]
}
fn mapped(m: &Move<Rc<String>>, source: &World, target: &World) -> Move<Rc<String>> {
    rebase_basic_move(
        m,
        |i| Ok::<_, ()>(owner(i)),
        |v| {
            let i = source
                .values
                .iter()
                .position(|candidate| Rc::ptr_eq(candidate, v))
                .unwrap();
            Ok(target.values[i].clone())
        },
    )
    .unwrap()
}
fn writes(m: &Move<Rc<String>>) -> Vec<(usize, Option<&Rc<String>>)> {
    let Move::Composite(children) = m else {
        panic!("expected atomic pillar composite")
    };
    children
        .iter()
        .map(|child| {
            let Move::Change { variable, value } = child else {
                panic!("expected pillar member write")
            };
            (*variable, value.as_ref())
        })
        .collect()
}
fn same_members(actual: &[usize], expected: &[usize], target: &World) {
    assert_eq!(actual.len(), expected.len());
    for &member in expected {
        assert_eq!(
            actual
                .iter()
                .filter(|&&i| Rc::ptr_eq(&target.entities[i], &target.entities[member]))
                .count(),
            1
        );
    }
}
fn same_value(actual: Option<&Rc<String>>, expected: Option<&Rc<String>>) {
    match (actual, expected) {
        (Some(a), Some(b)) => assert!(Rc::ptr_eq(a, b)),
        (None, None) => {}
        _ => panic!("incorrect null identity"),
    }
}
// Additional native assertion: execution commutes with identity mapping and a
// dropped destination transaction restores every original pointer, not just Eq.
fn execution_and_undo(
    m: &Move<Rc<String>>,
    rebased: &Move<Rc<String>>,
    source: &World,
    target: &World,
) {
    let mut original = source.assignment.clone();
    let mut destination = target.assignment.clone();
    {
        let a = original.apply(m).unwrap();
        let b = destination.apply(rebased).unwrap();
        for i in 0..4 {
            let expected = a.values()[i].as_ref().map(|v| {
                let n = source.values.iter().position(|x| Rc::ptr_eq(x, v)).unwrap();
                &target.values[n]
            });
            same_value(b.values()[owner(i)].as_ref(), expected);
        }
    }
    for i in 0..4 {
        same_value(
            original.values()[i].as_ref(),
            source.assignment.values()[i].as_ref(),
        );
        same_value(
            destination.values()[i].as_ref(),
            target.assignment.values()[i].as_ref(),
        );
    }
}
#[test]
fn pillar_change_rebase() {
    let source = world(false);
    let target = world(true);
    // All four source helper invocations, including null and singleton pillars.
    for (members, value) in [
        (vec![0, 2], None),
        (vec![0, 2], Some(2)),
        (vec![1], Some(0)),
        (vec![0], Some(1)),
    ] {
        let movement = Pillar::new(members.clone())
            .unwrap()
            .change_move(
                &source.assignment,
                &[value.map(|i| source.values[i].clone())],
            )
            .unwrap();
        let rebased = mapped(&movement, &source, &target);
        let actual = writes(&rebased);
        same_members(
            &actual.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
            &members.iter().map(|&i| owner(i)).collect::<Vec<_>>(),
            &target,
        );
        for (_, v) in actual {
            same_value(v, value.map(|i| &target.values[i]));
        }
        execution_and_undo(&movement, &rebased, &source, &target);
    }
}
#[test]
fn pillar_swap_rebase() {
    let source = world(false);
    let target = world(true);
    for (left, right) in [(vec![0, 2], vec![1]), (vec![3], vec![0, 2])] {
        let movement = Pillar::new(left.clone())
            .unwrap()
            .swap_move(&Pillar::new(right.clone()).unwrap(), &source.assignment)
            .unwrap();
        let rebased = mapped(&movement, &source, &target);
        let actual = writes(&rebased);
        assert_eq!(actual.len(), left.len() + right.len());
        // Native composite retains the left writes followed by right writes.
        same_members(
            &actual[..left.len()]
                .iter()
                .map(|(i, _)| *i)
                .collect::<Vec<_>>(),
            &left.iter().map(|&i| owner(i)).collect::<Vec<_>>(),
            &target,
        );
        same_members(
            &actual[left.len()..]
                .iter()
                .map(|(i, _)| *i)
                .collect::<Vec<_>>(),
            &right.iter().map(|&i| owner(i)).collect::<Vec<_>>(),
            &target,
        );
        execution_and_undo(&movement, &rebased, &source, &target);
    }
}
#[test]
fn two_opt_rebase() {
    // The source checks metadata only: its owner has an empty list, while both
    // values are registered in the lookup world. Do not silently populate it.
    let source_values = [Rc::new("1".to_owned()), Rc::new("2".to_owned())];
    let destination_values = [Rc::new("1".to_owned()), Rc::new("2".to_owned())];
    let source_owner = Rc::new("e1".to_owned());
    let destination_owner = Rc::new("e1".to_owned());
    let source_list: Vec<Rc<String>> = vec![];
    let destination_list: Vec<Rc<String>> = vec![];
    assert!(!Rc::ptr_eq(&source_owner, &destination_owner));
    assert!(source_list.is_empty() && destination_list.is_empty());
    let movement: ListMove<Rc<String>> = ListMove::SubListChange {
        entity: 0,
        start: 0,
        end: 1,
        destination: ListPosition {
            entity: 0,
            index: 0,
        },
        reverse: true,
    };
    let rebased = rebase_list_move(
        &movement,
        |entity| {
            assert_eq!(entity, 0);
            Ok::<_, ()>(17)
        },
        |v| {
            let i = source_values.iter().position(|x| Rc::ptr_eq(x, v)).unwrap();
            Ok(destination_values[i].clone())
        },
    )
    .unwrap();
    let ListMove::SubListChange {
        entity, start, end, ..
    } = rebased
    else {
        panic!("expected two-opt reversal")
    };
    let lookup = [(17, destination_owner.clone())];
    assert!(Rc::ptr_eq(
        &lookup.iter().find(|(i, _)| *i == entity).unwrap().1,
        &destination_owner
    ));
    assert_eq!(start, 0);
    assert_eq!(end, 1);
}
