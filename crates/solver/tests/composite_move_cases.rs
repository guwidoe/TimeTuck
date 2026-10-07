// Adapted from pinned Community builtin CompositeMoveTest. Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
// Adapted TestdataSolution.generateSolution: preserve cyclic initial values.
fn model(values: usize, entities: usize) -> FiniteProblem<usize> {
    FiniteProblem::new(
        (0..entities)
            .map(|i| Variable::new((0..values).collect()).with_initial(i % values))
            .collect(),
    )
    .unwrap()
}
fn change(variable: usize, value: usize) -> Move<usize> {
    Move::Change {
        variable,
        value: Some(value),
    }
}
fn permutation(values: &[usize]) -> Move<usize> {
    Move::Composite(
        values
            .iter()
            .enumerate()
            .map(|(i, &v)| change(i, v))
            .collect(),
    )
}
fn nested() -> Move<usize> {
    Move::Composite(vec![
        Move::Composite(vec![change(0, 3), change(1, 2)]),
        Move::Composite(vec![change(2, 1), change(3, 0)]),
    ])
}
#[test]
fn composite_move_executes_all_sub_moves() {
    let p = model(3, 3);
    let mut a = p.initial_assignment();
    MoveTester::new(&p)
        .using(&mut a)
        .unwrap()
        .execute(|d| d.push(permutation(&[2, 0, 1])))
        .unwrap();
    assert_eq!(a.values(), &[Some(2), Some(0), Some(1)]);
}
#[test]
fn composite_move_with_swap() {
    let p = model(3, 3);
    let mut a = p.initial_assignment();
    let before = a.clone();
    let m = Move::Composite(vec![Move::Swap { left: 0, right: 1 }, change(2, 2)]);
    MoveTester::new(&p)
        .using(&mut a)
        .unwrap()
        .execute(|d| d.push(m))
        .unwrap();
    assert_eq!(
        a.values(),
        &[before.values()[1], before.values()[0], Some(2)]
    );
}
#[test]
fn composite_move_with_single_sub_move() {
    let p = model(2, 1);
    let mut a = p.initial_assignment();
    MoveTester::new(&p)
        .using(&mut a)
        .unwrap()
        .execute(|d| d.push(Move::Composite(vec![change(0, 1)])))
        .unwrap();
    assert_eq!(a.values(), &[Some(1)]);
}
#[test]
fn nested_composite_moves() {
    let p = model(4, 4);
    let mut a = p.initial_assignment();
    MoveTester::new(&p)
        .using(&mut a)
        .unwrap()
        .execute(|d| d.push(nested()))
        .unwrap();
    assert_eq!(a.values(), &[Some(3), Some(2), Some(1), Some(0)]);
}
#[test]
fn composite_move_executes_temporarily_with_undo() {
    let p = model(3, 3);
    let mut a = p.initial_assignment();
    let before = a.clone();
    MoveTester::new(&p)
        .using(&mut a)
        .unwrap()
        .execute_temporarily(
            |d| d.push(permutation(&[2, 0, 1])),
            |d| assert_eq!(d.assignment().values(), &[Some(2), Some(0), Some(1)]),
        )
        .unwrap();
    assert_eq!(a, before);
}
#[test]
fn nested_composite_move_temporary() {
    let p = model(4, 4);
    let mut a = p.initial_assignment();
    let before = a.clone();
    MoveTester::new(&p)
        .using(&mut a)
        .unwrap()
        .execute_temporarily(
            |d| d.push(nested()),
            |d| {
                assert_eq!(
                    d.assignment().values(),
                    &[Some(3), Some(2), Some(1), Some(0)]
                )
            },
        )
        .unwrap();
    assert_eq!(a, before);
}
