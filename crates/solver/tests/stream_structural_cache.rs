//! Original structural cache/delta regressions; no upstream method credit.
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

fn basic_structure(p: &FiniteProblem<i32>, a: &Assignment<i32>) -> i64 {
    -(p.variables()
        .iter()
        .zip(a.values())
        .filter(|(v, x)| !v.allows_unassigned() && x.is_none())
        .count() as i64)
}
fn list_structure(p: &ListProblem<i32>, a: &ListAssignment<i32>) -> i64 {
    if p.allows_unassigned() {
        0
    } else {
        -(p.values().len() as i64 - a.lists().iter().map(|r| r.len() as i64).sum::<i64>())
    }
}
fn mixed_structure(p: &MixedProblem<i32, i32>, a: &MixedAssignment<i32, i32>) -> i64 {
    basic_structure(p.basic(), a.basic()) + list_structure(p.lists(), a.lists())
}
#[test]
fn basic_repeated_writes_dense_fallback_replacement_and_nested_sessions() {
    let p = basic_model(1024);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    b.score("value", source.port(), |(_, v)| {
        SimpleScore::new(i64::from(v.unwrap_or(0)))
    })
    .unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    for writes in [0, 1, 6, 7, 8, 9, 40, 256] {
        for step in 0..12 {
            let before = d.assignment().clone();
            let old = d.score();
            let trial = d.apply(&basic_move(writes, step)).unwrap();
            assert_eq!(
                trial.score().structural(),
                basic_structure(&p, trial.assignment())
            );
            if step.is_multiple_of(2) {
                trial.commit();
            } else {
                drop(trial);
                assert_eq!(d.assignment(), &before);
                assert_eq!(d.score(), old);
            }
            assert_eq!(d.score().structural(), basic_structure(&p, d.assignment()));
            d.assert_consistent().unwrap();
        }
    }
    let before = d.assignment().clone();
    let old = d.score();
    {
        let mut session = d.edit_session().unwrap();
        session.replace_assignment(p.initial_assignment()).unwrap();
        session.apply(&basic_move(9, 3)).unwrap();
        session.assert_consistent().unwrap();
        {
            let mut nested = session.nested().unwrap();
            nested.apply(&basic_move(40, 8)).unwrap();
            nested.commit();
        }
    }
    assert_eq!(d.assignment(), &before);
    assert_eq!(d.score(), old);
    d.replace_assignment(p.initial_assignment())
        .unwrap()
        .commit();
    assert_eq!(d.score().structural(), basic_structure(&p, d.assignment()));
    d.assert_consistent().unwrap();
}
#[test]
fn list_final_cardinality_drop_commit_dense_edits_and_replacement() {
    for optional in [false, true] {
        for rows in [4, 1024] {
            let p = list_model(optional, rows);
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let source = ListStreamSources::<i32>::new(&mut b);
            b.score("row", source.entities.port(), |(_, r)| {
                SimpleScore::new(r.len() as i64)
            })
            .unwrap();
            let mut d = ListStreamScoreDirector::new(&p, b.build(), source, p.initial_assignment())
                .unwrap();
            let before = d.assignment().clone();
            let old = d.score();
            let mut dense: Vec<_> = before
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
            let trial = d.apply(&ListMove::Composite(dense)).unwrap();
            assert_eq!(
                trial.score().structural(),
                list_structure(&p, trial.assignment())
            );
            drop(trial);
            assert_eq!(d.score(), old);
            for commit in [false, true] {
                let trial = d.apply(&list_move()).unwrap();
                assert_eq!(
                    trial.score().structural(),
                    list_structure(&p, trial.assignment())
                );
                if commit {
                    trial.commit();
                } else {
                    drop(trial);
                    assert_eq!(d.assignment(), &before);
                    assert_eq!(d.score(), old);
                }
                d.assert_consistent().unwrap();
            }
            let current = d.assignment().clone();
            let score = d.score();
            {
                let mut session = d.edit_session().unwrap();
                session.replace_assignment(before.clone()).unwrap();
                session.assert_consistent().unwrap();
            }
            assert_eq!(d.assignment(), &current);
            assert_eq!(d.score(), score);
            d.replace_assignment(before.clone()).unwrap().commit();
            assert_eq!(d.assignment(), &before);
            assert_eq!(d.score(), old);
            d.assert_consistent().unwrap();
        }
    }
}
#[test]
fn mixed_delta_callback_failure_panic_and_replacement_preserve_cache() {
    for optional in [false, true] {
        let p = MixedProblem::new(basic_model(1024), list_model(optional, 1024));
        let mode = Cell::new(0);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let sources = MixedStreamSources::<i32, i32>::new(&mut b);
        b.score("value", sources.basic.port(), |(_, v)| {
            assert_ne!(mode.get(), 1, "injected callback panic");
            SimpleScore::new(i64::from(v.unwrap_or(0))).with_structural(i64::from(mode.get() == 2))
        })
        .unwrap();
        let mut d =
            MixedStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
        let before = d.assignment().clone();
        let old = d.score();
        for writes in [1, 9, 40, 512] {
            let movement = MixedMove {
                basic: basic_move(writes, 0),
                lists: list_move(),
            };
            let trial = d.apply(&movement).unwrap();
            assert_eq!(
                trial.score().structural(),
                mixed_structure(&p, trial.assignment())
            );
            drop(trial);
            assert_eq!(d.assignment(), &before);
            assert_eq!(d.score(), old);
        }
        let movement = MixedMove {
            basic: basic_move(9, 0),
            lists: list_move(),
        };
        mode.set(1);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _ = d.apply(&movement);
            }))
            .is_err()
        );
        mode.set(2);
        assert!(d.apply(&movement).is_err());
        mode.set(0);
        assert_eq!(d.assignment(), &before);
        assert_eq!(d.score(), old);
        d.apply(&movement).unwrap().commit();
        assert_eq!(d.score().structural(), mixed_structure(&p, d.assignment()));
        let next = d.assignment().clone();
        let score = d.score();
        {
            let mut session = d.edit_session().unwrap();
            session.replace_assignment(before.clone()).unwrap();
            session.assert_consistent().unwrap();
        }
        assert_eq!(d.assignment(), &next);
        assert_eq!(d.score(), score);
        drop(d.replace_assignment(before.clone()).unwrap());
        assert_eq!(d.assignment(), &next);
        assert_eq!(d.score(), score);
        d.replace_assignment(before).unwrap().commit();
        assert_eq!(d.score(), old);
        d.assert_consistent().unwrap();
    }
}
