// Adapted pinned Community DefaultPhaseCommandContextTest, Apache-2.0.
// Native scored structural state/independent full recomputation replace mocks.
use std::cell::Cell;
use timetuck::*;
fn run(rescored: bool, flawed: bool) {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2]).with_initial(0),
        Variable::new(vec![0, 1, 2]).with_initial(0),
    ])
    .unwrap();
    let before = p.initial_assignment();
    let calculations = Cell::new(0);
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("zero", vec![0], |_| {
        calculations.set(calculations.get() + 1);
        SimpleScore::ZERO
    })
    .unwrap();
    let mut valid_calls = 0;
    let mut flawed_calls = 0;
    let result = custom_phase(&p, &n, before.clone(), Default::default(), |c| {
        let old_score = c.score();
        calculations.set(0); // The source verification concerns the temporary operation.
        let movement = Move::Change {
            variable: 0,
            value: if flawed { None } else { Some(1) },
        };
        let mut valid = |a: &Assignment<i32>| {
            valid_calls += 1;
            assert_eq!(a.values()[0], Some(1));
            "success"
        };
        let mut invalid = |a: &Assignment<i32>| {
            flawed_calls += 1;
            assert_eq!(a.values()[0], None);
            "error"
        };
        let value = if rescored {
            c.evaluate_structured_with_score(
                &movement,
                |a| n.calculate(&p, a).map(|s| (s, 1)),
                |a, s| {
                    assert_eq!(s, SimpleScore::ZERO);
                    valid(a)
                },
                |a, s| {
                    assert_eq!(s, SimpleScore::ZERO.with_structural(-1));
                    invalid(a)
                },
            )?
        } else {
            c.evaluate_structured(&movement, valid, invalid)?
        };
        assert_eq!(value, Some(if flawed { "error" } else { "success" }));
        assert_eq!(valid_calls, usize::from(!flawed));
        assert_eq!(flawed_calls, usize::from(flawed));
        assert_eq!(c.work().move_evaluations, 1); // One real applied move, no replay.
        assert_eq!(calculations.get(), if rescored { 2 } else { 1 });
        assert_eq!(c.assignment(), &before);
        assert_eq!(c.score(), old_score);
        Ok(())
    })
    .unwrap();
    assert_eq!(result.assignment, before);
    assert_eq!(result.score, Some(SimpleScore::ZERO));
    assert_eq!(result.work.committed_moves, 0);
    assert_eq!(result.work.evaluations, if rescored { 3 } else { 2 });
}
#[test]
fn temporary_move_does_not_flaw_solution() {
    run(false, false)
}
#[test]
fn temporary_move_flaws_solution() {
    run(false, true)
}
#[test]
fn temporary_scored_move_does_not_flaw_solution() {
    run(true, false)
}
#[test]
fn temporary_scored_move_flaws_solution() {
    run(true, true)
}
