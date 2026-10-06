//! Original candidate-inspection regressions across list and mixed backends.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn problem(pin: usize) -> ListProblem<usize> {
    ListProblem::new(
        vec![0, 1, 2],
        vec![ListEntity::new(vec![0, 1, 2]).with_pinned_prefix(pin)],
    )
    .unwrap()
    .with_dependencies(vec![vec![], vec![0], vec![1]])
    .unwrap()
}
fn swap() -> ListMove<usize> {
    ListMove::Swap {
        left: ListPosition {
            entity: 0,
            index: 0,
        },
        right: ListPosition {
            entity: 0,
            index: 1,
        },
    }
}
fn valid() -> ListMove<usize> {
    ListMove::Unassign {
        entity: 0,
        start: 2,
        end: 3,
    }
}
fn invalid() -> ListMove<usize> {
    ListMove::Unassign {
        entity: 0,
        start: 9,
        end: 10,
    }
}
macro_rules! exercise {
    ($d:ident,$calls:ident,$flawed:expr,$valid:expr,$invalid:expr,$expected:expr) => {{
        let initial = $d.assignment().clone();
        let old_score = $d.score();
        let flawed = $flawed;
        let before = $calls.get();
        let preview = $d.inspect_move(&flawed).unwrap();
        assert_eq!(preview.assignment(), &$expected);
        assert_eq!(
            preview.inconsistent_values(),
            &[0, 1, 2],
            "include downstream value"
        );
        assert_eq!($calls.get(), before);
        for failure in [
            TrialFailurePolicy::Rollback,
            TrialFailurePolicy::KeepApplied,
        ] {
            let options = TemporaryMoveOptions {
                failure,
                restored_score: RestoredScorePolicy::Cached,
            };
            let result = TemporaryMoves::new(&mut $d).process_inspected(
                &flawed,
                options,
                |_, _| -> Result<(), &str> { panic!("valid callback") },
                |p| {
                    assert_eq!(p.assignment(), &$expected);
                    Err("flawed callback error")
                },
            );
            assert!(matches!(
                result,
                Err(TemporaryMoveError::FlawedProcessor("flawed callback error"))
            ));
            let result = catch_unwind(AssertUnwindSafe(|| {
                TemporaryMoves::new(&mut $d).process_inspected(
                    &flawed,
                    options,
                    |_, _| -> Result<(), &str> { panic!("valid callback") },
                    |_| -> Result<(), &str> { panic!("flawed callback panic") },
                )
            }));
            assert!(result.is_err());
            assert_eq!($d.assignment(), &initial);
            assert_eq!($d.score(), old_score);
            assert_eq!($calls.get(), before);
        }
        let result = TemporaryMoves::new(&mut $d).process_inspected(
            &$invalid,
            TemporaryMoveOptions::default(),
            |_, _| -> Result<(), ()> { panic!("invalid reached valid callback") },
            |_| -> Result<(), ()> { panic!("invalid reached flawed callback") },
        );
        assert!(matches!(result, Err(TemporaryMoveError::Apply(_))));
        assert_eq!($calls.get(), before);
        TemporaryMoves::new(&mut $d)
            .process_inspected(
                &flawed,
                TemporaryMoveOptions {
                    restored_score: RestoredScorePolicy::CheckFresh,
                    ..Default::default()
                },
                |_, _| -> Result<(), ()> { panic!("valid callback") },
                |_| Ok(()),
            )
            .unwrap();
        assert!(
            $calls.get() > before,
            "fresh mode must run scoring on restored state"
        );
        let before = $calls.get();
        TemporaryMoves::new(&mut $d)
            .process_inspected(
                &$valid,
                TemporaryMoveOptions::default(),
                |a, s| {
                    assert_ne!(a, &initial);
                    assert_ne!(s, old_score);
                    Ok::<_, ()>(())
                },
                |_| -> Result<(), ()> { panic!("sound candidate reached flawed callback") },
            )
            .unwrap();
        assert!($calls.get() > before);
        assert_eq!($d.assignment(), &initial);
        assert_eq!($d.score(), old_score);
        $d.assert_consistent().unwrap();
    }};
}
#[test]
fn list_inspection_routes_candidates_without_touching_scoring_cache() {
    let p = problem(0);
    let calls = Cell::new(0);
    let score = |list: &[usize]| {
        calls.set(calls.get() + 1);
        SimpleScore::new(list.len() as i64)
    };
    let mut n = ListConstraintNetwork::new(1, 3);
    n.add_constraint("count", vec![ListScope::Entity(0)], |facts| {
        let ListFact::Entity(list) = facts[0] else {
            panic!()
        };
        score(list)
    })
    .unwrap();
    let expected = ListAssignment::new(vec![vec![1, 0, 2]]);
    let mut d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    exercise!(d, calls, swap(), valid(), invalid(), expected);
    let mut b = StreamGraphBuilder::new();
    let sources = ListStreamSources::<usize>::new(&mut b);
    b.score("count", sources.entities.port(), |(_, list)| score(list))
        .unwrap();
    let mut d =
        ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    exercise!(d, calls, swap(), valid(), invalid(), expected);
}
#[test]
fn mixed_inspection_includes_basic_changes_but_never_installs_flawed_state() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
        problem(0),
    );
    let calls = Cell::new(0);
    let score = |list: &[usize]| {
        calls.set(calls.get() + 1);
        SimpleScore::new(list.len() as i64)
    };
    let mut n = MixedConstraintNetwork::new(1, 1, 3);
    n.add_constraint("count", vec![MixedScope::ListEntity(0)], |facts| {
        let MixedFact::ListEntity(list) = facts[0] else {
            panic!()
        };
        score(list)
    })
    .unwrap();
    let movement = |lists| MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(1),
        },
        lists,
    };
    let invalid_basic = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(99),
        },
        lists: swap(),
    };
    let expected = MixedAssignment::new(
        Assignment::new(vec![Some(1)]),
        ListAssignment::new(vec![vec![1, 0, 2]]),
    );
    let mut d = MixedScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    exercise!(
        d,
        calls,
        movement(swap()),
        movement(valid()),
        invalid_basic.clone(),
        expected
    );
    let mut b = StreamGraphBuilder::new();
    let sources = MixedStreamSources::<usize, usize>::new(&mut b);
    b.score("count", sources.lists.entities.port(), |(_, list)| {
        score(list)
    })
    .unwrap();
    let mut d =
        MixedStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    exercise!(
        d,
        calls,
        movement(swap()),
        movement(valid()),
        invalid_basic,
        expected
    );
}
#[test]
fn inspection_keeps_pins_and_coordinates_strict_and_requires_a_valid_start() {
    let p = problem(1);
    let initial = p.initial_assignment();
    assert!(inspect_list_move(&p, &initial, &swap()).is_err());
    assert!(inspect_list_move(&p, &initial, &invalid()).is_err());
    assert_eq!(initial, p.initial_assignment());
    let p = problem(0);
    let cyclic = ListAssignment::new(vec![vec![1, 0, 2]]);
    assert!(inspect_list_move(&p, &cyclic, &swap()).is_err());
    let duplicate = ListMove::Assign {
        value: 0,
        destination: ListPosition {
            entity: 0,
            index: 3,
        },
    };
    assert!(inspect_list_move(&p, &p.initial_assignment(), &duplicate).is_err());
}
