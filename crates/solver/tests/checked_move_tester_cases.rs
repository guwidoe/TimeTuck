// Adapted pinned Community MoveTesterTest; Apache-2.0, see compat/provenance.json.
use timetuck::*;
type Edit = fn(&mut MoveDraft<'_, FiniteProblem<i32>>) -> Result<(), ModelError>;
type Inspect = fn(&mut MoveDraft<'_, FiniteProblem<i32>>);
fn solution() -> FiniteProblem<i32> {
    FiniteProblem::new(
        (0..2)
            .map(|i| Variable::new(vec![0, 1]).with_initial(i))
            .collect(),
    )
    .unwrap()
}
fn dummy(_: &mut MoveDraft<'_, FiniteProblem<i32>>) -> Result<(), ModelError> {
    Ok(())
}
#[test]
fn using_with_null_solution() {
    let p = solution();
    let runner = CheckedMoveTester::new(&p);
    let error = match runner.using(None) {
        Err(error) => error,
        Ok(_) => panic!("absent solution accepted"),
    };
    assert!(error.to_string().contains("solution"));
    assert_eq!(error, CheckedMoveTestError::MissingArgument("solution"));
}
#[test]
fn execute_with_null_move() {
    let p = solution();
    let mut a = p.initial_assignment();
    let before = a.clone();
    let mut context = CheckedMoveTester::new(&p).using(Some(&mut a)).unwrap();
    let error = context.execute(None::<Edit>).unwrap_err();
    assert!(error.to_string().contains("move"));
    assert_eq!(context.assignment(), &before);
}
#[test]
fn execute_temporarily_with_null_move() {
    let p = solution();
    let mut a = p.initial_assignment();
    let before = a.clone();
    let mut context = CheckedMoveTester::new(&p).using(Some(&mut a)).unwrap();
    let mut called = false;
    let error = context
        .execute_temporarily(
            None::<Edit>,
            Some(|_: &mut MoveDraft<'_, FiniteProblem<i32>>| called = true),
        )
        .unwrap_err();
    assert!(error.to_string().contains("move"));
    assert!(!called);
    assert_eq!(context.assignment(), &before);
}
#[test]
fn execute_temporarily_with_null_callback() {
    let p = solution();
    let mut a = p.initial_assignment();
    let before = a.clone();
    let mut context = CheckedMoveTester::new(&p).using(Some(&mut a)).unwrap();
    let error = context
        .execute_temporarily(Some(dummy as Edit), None::<Inspect>)
        .unwrap_err();
    assert!(error.to_string().contains("callback"));
    assert_eq!(context.assignment(), &before);
}
