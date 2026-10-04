// Adapted from pinned Timefold Community And/OrCompositeTerminationTest.
// Apache-2.0; provenance and substitutions: compat/provenance.json.
use timetuck::{TerminationExpression as Expr, TerminationState};

#[derive(Clone, Copy)]
enum Scope {
    Solver,
    Phase,
}
fn decisions(all: bool, scope: Scope, applicable: bool, expected: [bool; 4]) {
    let children = [Expr::Condition(0), Expr::Condition(1)];
    let expression = if all {
        Expr::All(&children)
    } else {
        Expr::Any(&children)
    };
    for ((a, b), wanted) in [(false, false), (true, false), (false, true), (true, true)]
        .into_iter()
        .zip(expected)
    {
        let values = [a, b];
        let state = expression
            .evaluate(|&i| {
                if matches!(scope, Scope::Phase) && i == 0 && !applicable {
                    None
                } else {
                    Some(TerminationState {
                        reached: values[i],
                        progress: None,
                    })
                }
            })
            .unwrap();
        assert_eq!(state.reached, wanted);
    }
}
fn gradients(all: bool, scope: Scope, applicable: bool, expected: [f64; 6]) {
    let children = [Expr::Condition(0), Expr::Condition(1)];
    let expression = if all {
        Expr::All(&children)
    } else {
        Expr::Any(&children)
    };
    for ((a, b), wanted) in [
        (0.0, 0.0),
        (0.5, 0.0),
        (0.0, 0.5),
        (-1.0, -1.0),
        (0.5, -1.0),
        (-1.0, 0.5),
    ]
    .into_iter()
    .zip(expected)
    {
        let values = [a, b];
        let state = expression
            .evaluate(|&i| {
                if matches!(scope, Scope::Phase) && i == 0 && !applicable {
                    None
                } else {
                    Some(TerminationState {
                        reached: false,
                        progress: Some(values[i]),
                    })
                }
            })
            .unwrap();
        assert_eq!(state.progress, Some(wanted));
    }
}
#[test]
fn and_solve_termination() {
    decisions(true, Scope::Solver, true, [false, false, false, true]);
}
#[test]
fn and_phase_termination() {
    decisions(true, Scope::Phase, true, [false, false, false, true]);
}
#[test]
fn and_phase_termination_inapplicable() {
    decisions(true, Scope::Phase, false, [false, false, true, true]);
}
#[test]
fn and_solver_gradient() {
    gradients(true, Scope::Solver, true, [0.0, 0.0, 0.0, 1.0, 0.5, 0.5]);
}
#[test]
fn and_phase_gradient() {
    gradients(true, Scope::Phase, true, [0.0, 0.0, 0.0, 1.0, 0.5, 0.5]);
}
#[test]
fn and_phase_gradient_inapplicable() {
    gradients(true, Scope::Phase, false, [0.0, 0.0, 0.5, 1.0, 1.0, 0.5]);
}
#[test]
fn or_solve_termination() {
    decisions(false, Scope::Solver, true, [false, true, true, true]);
}
#[test]
fn or_phase_termination() {
    decisions(false, Scope::Phase, true, [false, true, true, true]);
}
#[test]
fn or_phase_termination_inapplicable() {
    decisions(false, Scope::Phase, false, [false, false, true, true]);
}
#[test]
fn or_solver_gradient() {
    gradients(false, Scope::Solver, true, [0.0, 0.5, 0.5, 0.0, 0.5, 0.5]);
}
#[test]
fn or_phase_gradient() {
    gradients(false, Scope::Phase, true, [0.0, 0.5, 0.5, 0.0, 0.5, 0.5]);
}
#[test]
fn or_phase_gradient_inapplicable() {
    gradients(false, Scope::Phase, false, [0.0, 0.0, 0.5, 0.0, 0.0, 0.5]);
}
