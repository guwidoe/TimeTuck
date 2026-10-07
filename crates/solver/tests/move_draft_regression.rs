use std::panic::{AssertUnwindSafe, catch_unwind};
use timetuck::*;
fn basic() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![
        Variable::new(vec![1, 2, 3]).with_initial(1),
        Variable::new(vec![1, 2, 3]).with_initial(2),
    ])
    .unwrap()
}
fn change(variable: usize, value: i32) -> Move<i32> {
    Move::Change {
        variable,
        value: Some(value),
    }
}
#[test]
fn failed_edits_and_panics_never_publish_partial_work() {
    let p = basic();
    let mut a = p.initial_assignment();
    let before = a.clone();
    let runner = MoveTester::new(&p);
    let mut context = runner.using(&mut a).unwrap();
    let result = context.execute(|d| {
        d.push(change(0, 3))?;
        d.push(change(1, 99))
    });
    assert!(matches!(
        result,
        Err(MoveTestError::Model(ModelError::ValueOutsideDomain {
            variable: 1
        }))
    ));
    assert_eq!(context.assignment(), &before);
    assert!(
        catch_unwind(AssertUnwindSafe(|| context.execute(|d| {
            d.push(change(0, 3))?;
            panic!("edit callback")
        })))
        .is_err()
    );
    assert_eq!(context.assignment(), &before);
    assert!(
        catch_unwind(AssertUnwindSafe(|| context.execute_temporarily(
            |d| d.push(change(0, 3)),
            |d| {
                d.push(change(1, 1)).unwrap();
                panic!("inspection callback")
            }
        )))
        .is_err()
    );
    assert_eq!(context.assignment(), &before);
    let result = context
        .execute_temporarily(
            |d| d.push(change(0, 3)),
            |d| {
                assert_eq!(d.assignment().values()[0], Some(3));
                d.push(change(1, 1)).unwrap();
                Err::<(), _>("inspection failure")
            },
        )
        .unwrap();
    assert_eq!(result, Err("inspection failure"));
    assert_eq!(context.assignment(), &before);
    let movement = context.execute(|d| d.push(change(0, 3))).unwrap();
    let mut replay = before;
    p.apply(&mut replay, &movement).unwrap().commit();
    assert_eq!(context.assignment(), &replay);
}
#[test]
fn cloned_branches_keep_prior_successes_and_foreign_drafts_are_rejected() {
    let p = basic();
    let a = p.initial_assignment();
    let mut draft = MoveDraft::new(&p, &a).unwrap();
    draft.push(change(0, 3)).unwrap();
    let mut branch = draft.clone();
    branch.push(change(1, 1)).unwrap();
    assert!(draft.push(change(1, 99)).is_err());
    assert_eq!(draft.steps().len(), 1);
    assert_eq!(draft.assignment().values(), &[Some(3), Some(2)]);
    let (expected, movement) = branch.into_parts();
    let mut replay = a.clone();
    p.apply(&mut replay, &movement).unwrap().commit();
    assert_eq!(replay, expected);
    let mut working = a.clone();
    let runner = MoveTester::new(&p);
    let mut context = runner.using(&mut working).unwrap();
    let foreign = MoveDraft::new(&p, &Assignment::new(vec![Some(3), Some(3)])).unwrap();
    assert!(matches!(
        context.execute(|d| {
            *d = foreign;
            Ok(())
        }),
        Err(MoveTestError::ReplacedDraft)
    ));
    assert_eq!(context.assignment(), &a);
    let foreign = MoveDraft::new(&p, &a).unwrap();
    assert_eq!(
        context.execute_temporarily(
            |_| Ok(()),
            |d| {
                *d = foreign;
            }
        ),
        Err(MoveTestError::ReplacedDraft)
    );
    context
        .execute(|d| {
            let checkpoint = d.clone();
            d.push(change(0, 3))?;
            *d = checkpoint;
            d.push(change(1, 1))
        })
        .unwrap();
    assert_eq!(context.assignment().values(), &[Some(1), Some(1)]);
}
#[test]
fn model_validation_and_pins_apply_to_drafts_and_contexts() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]).with_initial(1).with_pinned(true),
    ])
    .unwrap();
    assert!(MoveDraft::new(&p, &Assignment::new(vec![])).is_err());
    let mut invalid = Assignment::new(vec![Some(2)]);
    assert!(MoveTester::new(&p).using(&mut invalid).is_err());
    let a = p.initial_assignment();
    let mut d = MoveDraft::new(&p, &a).unwrap();
    assert_eq!(
        d.push(change(0, 2)),
        Err(ModelError::PinnedValueChanged { variable: 0 })
    );
    assert_eq!(d.assignment(), &a);
    assert!(d.steps().is_empty());
    let empty = d.finish();
    let mut replay = a.clone();
    p.apply(&mut replay, &empty).unwrap().commit();
    assert_eq!(replay, a);
}
#[test]
fn list_drafts_produce_moves_for_incremental_scoring_and_undo() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let mut d = MoveDraft::new(&p, &a).unwrap();
    d.push(ListMove::Assign {
        value: 3,
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    })
    .unwrap();
    d.push(ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 1,
        },
        destination: ListPosition {
            entity: 1,
            index: 1,
        },
    })
    .unwrap();
    let expected = d.assignment().clone();
    assert!(
        d.push(ListMove::Change {
            source: ListPosition {
                entity: 0,
                index: 0
            },
            destination: ListPosition {
                entity: 1,
                index: 0
            }
        })
        .is_err()
    );
    assert_eq!(d.assignment(), &expected);
    assert_eq!(d.steps().len(), 2);
    let movement = d.finish();
    let mut n = ListConstraintNetwork::new(2, 3);
    n.add_constraint("receiver", vec![ListScope::Entity(1)], |f| {
        let ListFact::Entity(row) = f[0] else {
            unreachable!()
        };
        SimpleScore::new(row.len() as i64)
    })
    .unwrap();
    let mut director = ListScoreDirector::new(&p, &n, a.clone()).unwrap();
    let before_score = director.score();
    {
        let trial = director.apply(&movement).unwrap();
        assert_eq!(trial.assignment(), &expected);
        assert_eq!(trial.score(), SimpleScore::new(2));
    }
    assert_eq!(director.assignment(), &a);
    assert_eq!(director.score(), before_score);
    director.apply(&movement).unwrap().commit();
    assert_eq!(director.assignment(), &expected);
    assert_eq!(director.score(), SimpleScore::new(2));
    let mut direct = a.clone();
    MoveTester::new(&p)
        .using(&mut direct)
        .unwrap()
        .execute(|draft| draft.push(movement))
        .unwrap();
    assert_eq!(direct, expected);
}
#[test]
fn mixed_interleaved_drafts_compose_and_temporary_execution_restores_both_parts() {
    let lists = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![])],
    )
    .unwrap();
    let p = MixedProblem::new(basic(), lists);
    let mut a = p.initial_assignment();
    let before = a.clone();
    let edit = |d: &mut MoveDraft<'_, MixedProblem<i32, i32>>| {
        d.push(MixedMove {
            basic: change(0, 3),
            lists: ListMove::Assign {
                value: 3,
                destination: ListPosition {
                    entity: 1,
                    index: 0,
                },
            },
        })?;
        d.push(MixedMove {
            basic: Move::Swap { left: 0, right: 1 },
            lists: ListMove::Change {
                source: ListPosition {
                    entity: 0,
                    index: 1,
                },
                destination: ListPosition {
                    entity: 1,
                    index: 1,
                },
            },
        })
    };
    let runner = MoveTester::new(&p);
    let mut context = runner.using(&mut a).unwrap();
    let expected = context
        .execute_temporarily(edit, |d| d.assignment().clone())
        .unwrap();
    assert_eq!(context.assignment(), &before);
    assert_eq!(expected.basic().values(), &[Some(2), Some(3)]);
    assert_eq!(expected.lists().lists(), &[vec![1], vec![3, 2]]);
    let movement = context.execute(edit).unwrap();
    assert_eq!(context.assignment(), &expected);
    let mut replay = before.clone();
    p.apply(&mut replay, &movement).unwrap().commit();
    assert_eq!(replay, expected);
    let mut d = MoveDraft::new(&p, &before).unwrap();
    assert!(
        d.push(MixedMove {
            basic: change(0, 3),
            lists: ListMove::Assign {
                value: 99,
                destination: ListPosition {
                    entity: 1,
                    index: 0
                }
            }
        })
        .is_err()
    );
    assert_eq!(d.assignment(), &before);
    assert!(d.steps().is_empty());
}
