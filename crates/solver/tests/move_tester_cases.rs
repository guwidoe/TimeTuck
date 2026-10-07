// Adapted from pinned Community MoveTesterTest and TestdataSolution.generateSolution.
// Apache-2.0; see compat/provenance.json.
use timetuck::*;
fn solution(values: i32, entities: usize) -> FiniteProblem<i32> {
    FiniteProblem::new(
        (0..entities)
            .map(|i| Variable::new((0..values).collect()).with_initial(i as i32 % values))
            .collect(),
    )
    .unwrap()
}
fn swap(draft: &mut MoveDraft<'_, FiniteProblem<i32>>) -> Result<(), ModelError> {
    let a = draft.assignment().values()[0];
    let b = draft.assignment().values()[1];
    draft.push(Move::Change {
        variable: 0,
        value: b,
    })?;
    draft.push(Move::Change {
        variable: 1,
        value: a,
    })
}
#[test]
fn build_succeeds() {
    let p = solution(2, 2);
    let runner = MoveTester::new(&p);
    assert!(std::ptr::eq(runner.problem(), &p));
}
#[test]
fn using_succeeds() {
    let p = solution(2, 3);
    let mut a = p.initial_assignment();
    let runner = MoveTester::new(&p);
    let context = runner.using(&mut a).unwrap();
    assert_eq!(context.assignment().values(), &[Some(0), Some(1), Some(0)]);
}
#[test]
fn execute_simple_move() {
    let p = solution(2, 2);
    let mut a = p.initial_assignment();
    let before = a.clone();
    MoveTester::new(&p)
        .using(&mut a)
        .unwrap()
        .execute(swap)
        .unwrap();
    assert_eq!(a.values()[0], before.values()[1]);
    assert_eq!(a.values()[1], before.values()[0]);
}
#[test]
fn temporary_swap_undo() {
    let p = solution(2, 2);
    let mut a = p.initial_assignment();
    let before = a.clone();
    MoveTester::new(&p)
        .using(&mut a)
        .unwrap()
        .execute_temporarily(swap, |view| {
            assert_eq!(view.assignment().values()[0], before.values()[1]);
            assert_eq!(view.assignment().values()[1], before.values()[0]);
        })
        .unwrap();
    assert_eq!(a, before);
}
#[test]
fn temporary_complex_rotation() {
    let p = solution(3, 3);
    let mut a = p.initial_assignment();
    let before = a.clone();
    MoveTester::new(&p)
        .using(&mut a)
        .unwrap()
        .execute_temporarily(
            |view| {
                let values = view.assignment().values().to_vec();
                for i in 0..values.len() {
                    view.push(Move::Change {
                        variable: i,
                        value: values[(i + 1) % values.len()],
                    })?;
                }
                Ok(())
            },
            |view| {
                for i in 0..before.values().len() {
                    assert_eq!(
                        view.assignment().values()[i],
                        before.values()[(i + 1) % before.values().len()]
                    );
                }
            },
        )
        .unwrap();
    assert_eq!(a, before);
}
