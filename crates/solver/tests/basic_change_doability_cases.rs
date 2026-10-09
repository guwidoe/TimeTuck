// Adapted from pinned Community SelectorBasedChangeMoveTest and SelectorBasedSwapMoveTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[test]
fn change_is_doable_against_each_current_value() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec!["1", "2", "3"]).allowing_unassigned(),
    ])
    .unwrap();
    let movement = Move::Change {
        variable: 0,
        value: Some("2"),
    };
    for (value, expected) in [("1", true), ("2", false), ("3", true)] {
        assert_eq!(
            p.is_move_doable(&Assignment::new(vec![Some(value)]), &movement)
                .unwrap(),
            expected
        );
    }
}
#[test]
fn change_executes_even_when_current_value_already_matches() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec!["1", "2", "3"]).allowing_unassigned(),
    ])
    .unwrap();
    let movement = Move::Change {
        variable: 0,
        value: Some("2"),
    };
    for value in ["1", "2", "3"] {
        let mut a = Assignment::new(vec![Some(value)]);
        p.apply(&mut a, &movement).unwrap().commit();
        assert_eq!(a.values(), &[Some("2")]);
    }
}

#[test]
fn swap_doability_uses_each_entity_range() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2, 3]).allowing_unassigned(),
        Variable::new(vec![2, 3, 4, 5]).allowing_unassigned(),
        Variable::new(vec![4, 5]).allowing_unassigned(),
    ])
    .unwrap();
    let ab = Move::Swap { left: 0, right: 1 };
    for (a, b, expected) in [(2, 3, true), (3, 2, true), (3, 3, false)] {
        assert_eq!(
            p.is_move_doable(&Assignment::new(vec![Some(a), Some(b), None]), &ab)
                .unwrap(),
            expected
        );
    }
    let bc = Move::Swap { left: 1, right: 2 };
    for (b, c, expected) in [(4, 5, true), (5, 4, true), (5, 5, false)] {
        assert_eq!(
            p.is_move_doable(&Assignment::new(vec![Some(3), Some(b), Some(c)]), &bc)
                .unwrap(),
            expected
        );
    }
    assert!(
        !p.is_move_doable(
            &Assignment::new(vec![Some(3), Some(5), Some(5)]),
            &Move::Swap { left: 0, right: 0 }
        )
        .unwrap()
    );
}
