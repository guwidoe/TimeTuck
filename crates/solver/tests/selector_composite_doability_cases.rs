// Adapted from Community SelectorBasedCompositeMoveTest.isMoveDoable,
// Apache-2.0. See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;

#[test]
fn is_move_doable() {
    // Real primitive changes replace the TEST dummy true/false children. Both
    // children are queried against the same opening assignment, without edits.
    let problem = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(0),
    ])
    .unwrap();
    let assignment = problem.initial_assignment();
    let checker = problem.move_checker(&assignment).unwrap();
    for (first, second, expected) in [(1, 1, true), (1, 0, true), (0, 1, true), (0, 0, false)] {
        let movement = compose_basic_moves([
            Move::Change {
                variable: 0,
                value: Some(first),
            },
            Move::Change {
                variable: 1,
                value: Some(second),
            },
        ]);
        assert_eq!(
            movement
                .try_selector_is_doable(|child| checker.is_doable(child))
                .unwrap(),
            expected
        );
    }
    assert_eq!(assignment.values(), &[Some(0), Some(0)]);
}
