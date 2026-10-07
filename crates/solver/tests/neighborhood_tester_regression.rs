use std::{
    cell::Cell,
    convert::Infallible,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[test]
fn infinite_provider_is_bounded_without_an_extra_draw() {
    let calls = Rc::new(Cell::new(0));
    let tester = NeighborhoodTester::new((), |_| {
        let counter = calls.clone();
        Ok::<_, Infallible>(Box::new(
            (0..).inspect(move |_| counter.set(counter.get() + 1)),
        ))
    });
    assert_eq!(
        tester.within(3).produces_all_of([3]),
        Err(NeighborhoodAssertionError::Missing {
            moves: vec![3],
            inspected: 3
        })
    );
    assert_eq!(calls.get(), 3);
    tester.within(4).produces_all_of([0, 0, 2]).unwrap();
    assert_eq!(calls.get(), 6); // All-of stops immediately after the last match.
    tester.within(3).produces_none_of([3]).unwrap();
    assert_eq!(calls.get(), 9);
    assert_eq!(
        tester.within(4).produces_none_of([3]),
        Err(NeighborhoodAssertionError::Unexpected {
            movement: 3,
            inspected: 4
        })
    );
    assert_eq!(calls.get(), 13);
    assert_eq!(
        tester.within(0).produces_all_of([0]),
        Err(NeighborhoodAssertionError::Missing {
            moves: vec![0],
            inspected: 0
        })
    );
    tester.within(0).produces_none_of([0]).unwrap();
    assert_eq!(calls.get(), 13);
}
#[test]
fn provider_errors_and_empty_requests_do_not_get_misreported_as_absence() {
    let calls = Cell::new(0);
    let tester = NeighborhoodTester::<_, u8, &str>::new((), |_| {
        calls.set(calls.get() + 1);
        Err("broken")
    });
    tester.produces_all_of([]).unwrap();
    tester.produces_none_of([]).unwrap();
    assert_eq!(calls.get(), 0);
    assert_eq!(
        tester.produces_all_of([1]),
        Err(NeighborhoodAssertionError::Provider("broken"))
    );
    assert_eq!(
        tester.produces_none_of([1]),
        Err(NeighborhoodAssertionError::Provider("broken"))
    );
    assert_eq!(calls.get(), 2);
}
#[test]
fn derived_contexts_share_score_director_updates_and_rollback_on_panic() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let mut network = ConstraintNetwork::new(1);
    network
        .add_constraint("value", vec![0], |v| {
            SimpleScore::new(i64::from(*v[0].unwrap()))
        })
        .unwrap();
    let director = ScoreDirector::new(&p, &network, p.initial_assignment()).unwrap();
    let tester = NeighborhoodTester::new(director, |d| {
        Ok::<_, ModelError>(Box::new(change_moves(&p, d.assignment())?))
    });
    let derived = tester.within(2);
    let first = Move::Change {
        variable: 0,
        value: Some(1),
    };
    let reverse = Move::Change {
        variable: 0,
        value: Some(0),
    };
    derived.produces_all_of([first.clone()]).unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| derived.update(|d| {
            let applied = d.apply(&first).unwrap();
            assert_eq!(applied.score(), SimpleScore::new(1));
            panic!("trial failed")
        })))
        .is_err()
    );
    tester.inspect(|d| {
        assert_eq!(d.assignment().values(), &[Some(0)]);
        assert_eq!(d.score(), SimpleScore::new(0));
    });
    tester.produces_all_of([first.clone()]).unwrap();
    derived.update(|d| d.apply(&first).unwrap().commit());
    tester.inspect(|d| assert_eq!(d.score(), SimpleScore::new(1)));
    tester.produces_none_of([first]).unwrap();
    tester.produces_all_of([reverse]).unwrap();
    tester.update(|d| d.assert_consistent().unwrap());
}
