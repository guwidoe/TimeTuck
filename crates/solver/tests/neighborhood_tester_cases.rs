// Adapted from Timefold Community NeighborhoodTesterTest and Testdata fixtures.
// Apache-2.0; exact pinned source identities are in compat/provenance.json.
use timetuck::*;
type State = (FiniteProblem<u8>, Assignment<u8>);
fn context(values: Vec<Option<u8>>) -> NeighborhoodTester<'static, State, Move<u8>, ModelError> {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    NeighborhoodTester::new((problem, Assignment::new(values)), |(p, a)| {
        Ok(Box::new(change_moves(p, a)?))
    })
}
fn change(variable: usize, value: u8) -> Move<u8> {
    Move::Change {
        variable,
        value: Some(value),
    }
}
fn four() -> Vec<Move<u8>> {
    vec![change(0, 0), change(0, 1), change(1, 0), change(1, 1)]
}
#[test]
fn temporary_move_execution_does_not_affect_move_iterator() {
    let tester = context(vec![None, None]);
    tester.produces_all_of(four()).unwrap();
    tester.update(|(p, a)| {
        let temporary = p.apply(a, &change(0, 0)).unwrap();
        assert_eq!(temporary.values()[0], Some(0));
    });
    tester.inspect(|(_, a)| assert_eq!(a.values()[0], None));
    tester.produces_all_of(four()).unwrap();
}
#[test]
fn new_iterator_after_move_execution() {
    let tester = context(vec![None, None]);
    tester.produces_all_of(four()).unwrap();
    tester.update(|(p, a)| p.apply(a, &change(0, 0)).unwrap().commit());
    tester.inspect(|(_, a)| assert_eq!(a.values()[0], Some(0)));
    tester
        .produces_all_of([change(0, 1), change(1, 0), change(1, 1)])
        .unwrap();
    tester.produces_none_of([change(0, 0)]).unwrap();
}
#[test]
fn within_overrides_the_iteration_limit_without_mutating_the_original_context() {
    let tester = context(vec![None, Some(1)]);
    let narrowed = tester.within(5);
    narrowed.produces_all_of([change(0, 0)]).unwrap();
    tester.produces_all_of([change(0, 0)]).unwrap();
    assert_eq!(tester.iteration_limit(), 10_000);
    assert_eq!(narrowed.iteration_limit(), 5);
}
#[test]
fn produces_all_of_fails_when_the_move_is_never_produced() {
    let tester = context(vec![Some(0), Some(1)]);
    assert!(matches!(
        tester.within(50).produces_all_of([change(0, 0)]),
        Err(NeighborhoodAssertionError::Missing { .. })
    ));
}
#[test]
fn produces_none_of_fails_when_the_move_is_produced() {
    let tester = context(vec![None, Some(1)]);
    assert!(matches!(
        tester.produces_none_of([change(0, 0)]),
        Err(NeighborhoodAssertionError::Unexpected { .. })
    ));
}
