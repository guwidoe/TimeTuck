//! Original sparse-trial regressions; no upstream source-method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;

fn basic_model(n: usize) -> FiniteProblem<i32> {
    FiniteProblem::new(
        (0..n)
            .map(|i| {
                let mut v = Variable::new(vec![0, 1]);
                if i.is_multiple_of(3) {
                    v = v.with_initial(0);
                }
                if i.is_multiple_of(5) {
                    v = v.allowing_unassigned();
                }
                v.with_pinned(i + 1 == n)
            })
            .collect(),
    )
    .unwrap()
}

fn basic_move(writes: usize, step: usize) -> Move<i32> {
    Move::Composite(
        (0..writes)
            .map(|i| {
                // Repeated writes straddle the small-journal and indexed paths. The
                // first and last null state frequently differ from intermediate writes.
                let variable = (i / 3 + step) % 100;
                Move::Change {
                    variable,
                    value: if (i + step).is_multiple_of(4) {
                        None
                    } else {
                        Some((i % 2) as i32)
                    },
                }
            })
            .chain([Move::Swap {
                left: step % 100,
                right: (step + 1) % 100,
            }])
            .collect(),
    )
}

#[test]
fn sparse_and_dense_basic_journals_match_fresh_counts_across_commits() {
    let problem = basic_model(1024);
    let mut network = ConstraintNetwork::new(1024);
    network
        .add_constraint("first", vec![0], |v| {
            SimpleScore::new(v[0].copied().unwrap_or(-1) as i64)
        })
        .unwrap();
    let mut director =
        ScoreDirector::new(&problem, &network, problem.initial_assignment()).unwrap();
    for writes in [0, 1, 6, 7, 8, 9, 40, 256] {
        for step in 0..12 {
            let before = director.assignment().clone();
            let old_score = director.score();
            let movement = basic_move(writes, step);
            let trial = director.apply(&movement).unwrap();
            let missing = problem
                .variables()
                .iter()
                .zip(trial.assignment().values())
                .filter(|(v, value)| !v.allows_unassigned() && value.is_none())
                .count();
            assert_eq!(trial.score().structural(), -(missing as i64));
            assert_eq!(
                trial.score(),
                network.calculate(&problem, trial.assignment()).unwrap()
            );
            if step.is_multiple_of(2) {
                trial.commit();
            } else {
                drop(trial);
                assert_eq!(director.assignment(), &before);
                assert_eq!(director.score(), old_score);
            }
            director.assert_consistent().unwrap();
        }
    }
    let before = director.assignment().clone();
    let old_score = director.score();
    let invalid = Move::Composite(vec![
        Move::Change {
            variable: 1,
            value: None,
        },
        Move::Change {
            variable: 1023,
            value: Some(1),
        },
    ]);
    assert!(director.apply(&invalid).is_err());
    assert_eq!(director.assignment(), &before);
    assert_eq!(director.score(), old_score);
}

fn list_model(optional: bool, rows: usize) -> ListProblem<i32> {
    let mut entities = vec![
        ListEntity::new(vec![0]).with_pinned_prefix(1),
        ListEntity::new(vec![1]),
        ListEntity::new(vec![5]).with_pinned(true),
    ];
    entities.extend((3..rows).map(|_| ListEntity::new(vec![])));
    let p = ListProblem::new_indexed((0..8).collect(), entities).unwrap();
    if optional { p.allowing_unassigned() } else { p }
}

fn list_move() -> ListMove<i32> {
    ListMove::Composite(vec![
        ListMove::Unassign {
            entity: 1,
            start: 0,
            end: 1,
        },
        ListMove::Assign {
            value: 2,
            destination: ListPosition {
                entity: 1,
                index: 0,
            },
        },
        ListMove::Assign {
            value: 1,
            destination: ListPosition {
                entity: 0,
                index: 1,
            },
        },
        ListMove::Assign {
            value: 3,
            destination: ListPosition {
                entity: 3,
                index: 0,
            },
        },
        ListMove::Unassign {
            entity: 3,
            start: 0,
            end: 1,
        },
    ])
}

#[test]
fn list_cardinality_uses_final_touched_rows_and_preserves_optional_semantics() {
    for optional in [false, true] {
        let problem = list_model(optional, 1024);
        let network = ListConstraintNetwork::<i32, SimpleScore>::new(1024, 8);
        let mut director =
            ListScoreDirector::new(&problem, &network, problem.initial_assignment()).unwrap();
        let original = director.assignment().clone();
        let score = director.score();
        let mut dense: Vec<_> = original
            .lists()
            .iter()
            .enumerate()
            .map(|(entity, row)| ListMove::Unassign {
                entity,
                start: row.len(),
                end: row.len(),
            })
            .collect();
        dense.push(ListMove::Assign {
            value: 2,
            destination: ListPosition {
                entity: 1,
                index: 1,
            },
        });
        let dense = ListMove::Composite(dense);
        let trial = director.apply(&dense).unwrap();
        assert_eq!(
            trial.score(),
            network.calculate(&problem, trial.assignment()).unwrap()
        );
        assert_eq!(trial.score().structural(), if optional { 0 } else { -4 });
        drop(trial);
        assert_eq!(director.assignment(), &original);
        assert_eq!(director.score(), score);
        for commit in [false, true] {
            let trial = director.apply(&list_move()).unwrap();
            assert_eq!(trial.score().structural(), if optional { 0 } else { -4 });
            assert_eq!(
                trial.score(),
                network.calculate(&problem, trial.assignment()).unwrap()
            );
            assert_eq!(trial.constraint_evaluations(), 0);
            if commit {
                trial.commit();
            } else {
                drop(trial);
                assert_eq!(director.assignment(), &original);
                assert_eq!(director.score(), score);
            }
            director.assert_consistent().unwrap();
        }
        let before = director.assignment().clone();
        let score = director.score();
        let invalid = ListMove::Composite(vec![
            ListMove::Assign {
                value: 4,
                destination: ListPosition {
                    entity: 3,
                    index: 0,
                },
            },
            ListMove::Unassign {
                entity: 2,
                start: 0,
                end: 1,
            },
        ]);
        assert!(director.apply(&invalid).is_err());
        assert_eq!(director.assignment(), &before);
        assert_eq!(director.score(), score);
    }
}

#[test]
fn joint_deltas_survive_failed_callbacks_and_component_validation() {
    for optional in [false, true] {
        let problem = MixedProblem::new(basic_model(1024), list_model(optional, 1024));
        let failure = Cell::new(0);
        let calls = Cell::new(0);
        let mut network = MixedConstraintNetwork::new(1024, 1024, 8);
        network
            .add_constraint(
                "joint",
                vec![MixedScope::Basic(0), MixedScope::ListEntity(1)],
                |_| {
                    calls.set(calls.get() + 1);
                    assert!(failure.get() != 1, "injected callback failure");
                    SimpleScore::new(17).with_structural(i64::from(failure.get() == 2))
                },
            )
            .unwrap();
        let mut director =
            MixedScoreDirector::new(&problem, &network, problem.initial_assignment()).unwrap();
        let original = director.assignment().clone();
        let original_score = director.score();
        for writes in [1, 9, 40, 512] {
            let movement = MixedMove {
                basic: basic_move(writes, 0),
                lists: list_move(),
            };
            let before_calls = calls.get();
            let trial = director.apply(&movement).unwrap();
            assert_eq!(calls.get(), before_calls + 1);
            assert_eq!(
                trial.score(),
                network.calculate(&problem, trial.assignment()).unwrap()
            );
            drop(trial);
            assert_eq!(director.assignment(), &original);
            assert_eq!(director.score(), original_score);
        }
        let movement = MixedMove {
            basic: basic_move(9, 0),
            lists: list_move(),
        };
        failure.set(1);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _ = director.apply(&movement);
            }))
            .is_err()
        );
        failure.set(2);
        assert!(director.apply(&movement).is_err());
        failure.set(0);
        assert_eq!(director.assignment(), &original);
        assert_eq!(director.score(), original_score);
        let invalid = MixedMove {
            basic: basic_move(9, 0),
            lists: ListMove::Assign {
                value: 99,
                destination: ListPosition {
                    entity: 3,
                    index: 0,
                },
            },
        };
        assert!(director.apply(&invalid).is_err());
        assert_eq!(director.assignment(), &original);
        assert_eq!(director.score(), original_score);
        director.apply(&movement).unwrap().commit();
        director.assert_consistent().unwrap();
        assert_eq!(
            director.score(),
            network.calculate(&problem, director.assignment()).unwrap()
        );
    }
}
