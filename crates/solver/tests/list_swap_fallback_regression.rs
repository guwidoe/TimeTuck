use std::num::NonZeroUsize;
use timetuck::*;
#[test]
fn seeded_attempts_yield_real_valid_moves_and_shared_fallbacks_without_mutation() {
    let problem = ListProblem::new(
        vec![1, 2],
        vec![ListEntity::new(vec![1]), ListEntity::new(vec![2])],
    )
    .unwrap();
    let before = problem.initial_assignment();
    let make = || {
        repeating_list_swap_attempts(
            &problem,
            &before,
            &[1, 1, 2],
            &[1, 2],
            NonZeroUsize::new(1).unwrap(),
            false,
            7,
        )
        .unwrap()
    };
    let mut candidates = 0;
    let mut fallbacks = 0;
    let actual: Vec<_> = make()
        .take(100)
        .map(|attempt| {
            match &attempt {
                ListSwapAttempt::Candidate(_) => candidates += 1,
                ListSwapAttempt::NoChange(marker) => {
                    fallbacks += 1;
                    assert!(std::ptr::eq(*marker, &LIST_SWAP_NO_CHANGE));
                }
            }
            let movement = attempt.into_movement();
            let mut working = before.clone();
            problem.apply(&mut working, &movement).unwrap().commit();
            problem.validate(&working).unwrap();
            (movement, working)
        })
        .collect();
    assert!(candidates > 0 && fallbacks > 0);
    assert_eq!(
        actual,
        make()
            .take(100)
            .map(|attempt| {
                let movement = attempt.into_movement();
                let mut working = before.clone();
                problem.apply(&mut working, &movement).unwrap().commit();
                (movement, working)
            })
            .collect::<Vec<_>>()
    );
    assert_eq!(before, problem.initial_assignment());
}
#[test]
fn empty_left_fuses_empty_right_repeats_and_cursor_guards_do_not_consume() {
    let problem = ListProblem::new(vec![1], vec![ListEntity::new(vec![1])]).unwrap();
    let before = problem.initial_assignment();
    let n = NonZeroUsize::new(2).unwrap();
    let mut empty =
        repeating_list_swap_attempts(&problem, &before, &[], &[1], n, false, 0).unwrap();
    assert!(!empty.has_next());
    assert!(empty.next().is_none());
    assert!(empty.next().is_none());
    let mut cursor =
        repeating_list_swap_attempts(&problem, &before, &[1], &[], n, false, 0).unwrap();
    assert!(cursor.try_remove().is_err());
    assert!(
        cursor
            .try_for_each_remaining(|_| panic!("must not consume"))
            .is_err()
    );
    for attempt in cursor.take(100) {
        assert!(
            matches!(attempt, ListSwapAttempt::NoChange(marker) if std::ptr::eq(marker, &LIST_SWAP_NO_CHANGE))
        );
    }
    assert!(repeating_list_swap_attempts(&problem, &before, &[9], &[1], n, false, 0).is_err());
}
