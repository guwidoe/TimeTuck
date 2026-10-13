use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn finite_sessions_publish_restore_and_discard_failed_callbacks() {
    for count in 2..=5 {
        let p = FiniteProblem::new(
            (0..count)
                .map(|i| Variable::new(vec![0, 1, 2]).with_initial(i % 3))
                .collect(),
        )
        .unwrap();
        let mut a = p.initial_assignment();
        let before = a.clone();
        let calls = Cell::new(0);
        let runner = CheckedMoveTester::new(&p);
        assert!(std::ptr::eq(runner.problem(), &p));
        let mut context = runner.using(Some(&mut a)).unwrap();
        let edit = |d: &mut MoveDraft<'_, FiniteProblem<i32>>| {
            calls.set(calls.get() + 1);
            let values = d.assignment().values().to_vec();
            for i in 0..values.len() {
                d.push(Move::Change {
                    variable: i,
                    value: values[(i + 1) % values.len()],
                })?;
            }
            Ok(())
        };
        let seen = context
            .execute_temporarily(
                Some(edit),
                Some(|d: &mut MoveDraft<'_, FiniteProblem<i32>>| {
                    calls.set(calls.get() + 1);
                    assert_ne!(d.assignment(), &before);
                    d.assignment().clone()
                }),
            )
            .unwrap();
        assert_eq!(context.assignment(), &before);
        assert_eq!(calls.get(), 2);
        let replay = context.execute(Some(edit)).unwrap();
        assert_eq!(context.assignment(), &seen);
        let mut independent = before.clone();
        {
            let applied = p.apply(&mut independent, &replay).unwrap();
            assert_eq!(&*applied, &seen);
        }
        assert_eq!(independent, before);
        p.apply(&mut independent, &replay).unwrap().commit();
        assert_eq!(independent, seen);
        let saved = context.assignment().clone();
        let failure = context.execute(Some(|d: &mut MoveDraft<'_, FiniteProblem<i32>>| {
            d.push(Move::Change {
                variable: 0,
                value: Some(2),
            })?;
            d.push(Move::Change {
                variable: 1,
                value: Some(99),
            })
        }));
        assert!(matches!(
            failure,
            Err(CheckedMoveTestError::Execution(MoveTestError::Model(_)))
        ));
        assert_eq!(context.assignment(), &saved);
        let panic = catch_unwind(AssertUnwindSafe(|| {
            context.execute_temporarily(
                Some(edit),
                Some(|_: &mut MoveDraft<'_, FiniteProblem<i32>>| panic!("inspection failed")),
            )
        }));
        assert!(panic.is_err());
        assert_eq!(context.assignment(), &saved);
    }
}
#[test]
fn missing_inputs_are_checked_before_edits_and_inspection() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let mut a = p.initial_assignment();
    let mut context = CheckedMoveTester::new(&p).using(Some(&mut a)).unwrap();
    let called = Cell::new(0);
    let edit = |d: &mut MoveDraft<'_, FiniteProblem<i32>>| {
        called.set(called.get() + 1);
        d.push(Move::Change {
            variable: 0,
            value: Some(1),
        })
    };
    type Inspect = fn(&mut MoveDraft<'_, FiniteProblem<i32>>);
    assert_eq!(
        context.execute_temporarily(Some(edit), None::<Inspect>),
        Err(CheckedMoveTestError::MissingArgument("callback"))
    );
    assert_eq!(called.get(), 0);
    assert_eq!(context.assignment().values(), &[Some(0)]);
    type Edit = fn(&mut MoveDraft<'_, FiniteProblem<i32>>) -> Result<(), ModelError>;
    assert_eq!(
        context.execute_temporarily(None::<Edit>, None::<Inspect>),
        Err(CheckedMoveTestError::MissingArgument("move"))
    );
    let mut invalid = Assignment::new(vec![Some(99)]);
    assert!(matches!(
        CheckedMoveTester::new(&p).using(Some(&mut invalid)),
        Err(CheckedMoveTestError::Model(_))
    ));
}
#[test]
fn native_list_and_mixed_models_use_real_drafts() {
    let lists = ListProblem::new(
        vec![10, 20, 30],
        vec![ListEntity::new(vec![10, 20]), ListEntity::new(vec![30])],
    )
    .unwrap();
    let mut a = lists.initial_assignment();
    let before = a.clone();
    let mut c = CheckedMoveTester::new(&lists).using(Some(&mut a)).unwrap();
    c.execute_temporarily(
        Some(|d: &mut MoveDraft<'_, ListProblem<i32>>| {
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
        }),
        Some(|d: &mut MoveDraft<'_, ListProblem<i32>>| {
            assert_eq!(d.assignment().lists(), [vec![10], vec![30, 20]]);
            lists.validate(d.assignment()).unwrap();
        }),
    )
    .unwrap();
    assert_eq!(c.assignment(), &before);
    let mixed = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(1)]).unwrap(),
        lists,
    );
    let mut a = mixed.initial_assignment();
    let before = a.clone();
    let mut c = CheckedMoveTester::new(&mixed).using(Some(&mut a)).unwrap();
    let movement = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(2),
        },
        lists: ListMove::Swap {
            left: ListPosition {
                entity: 0,
                index: 0,
            },
            right: ListPosition {
                entity: 1,
                index: 0,
            },
        },
    };
    c.execute_temporarily(
        Some(|d: &mut MoveDraft<'_, MixedProblem<i32, i32>>| d.push(movement.clone())),
        Some(|d: &mut MoveDraft<'_, MixedProblem<i32, i32>>| {
            mixed.validate(d.assignment()).unwrap();
            assert_ne!(d.assignment(), &before);
        }),
    )
    .unwrap();
    assert_eq!(c.assignment(), &before);
    let replay = c
        .execute(Some(|d: &mut MoveDraft<'_, MixedProblem<i32, i32>>| {
            d.push(movement)
        }))
        .unwrap();
    let mut independent = before.clone();
    {
        let applied = mixed.apply(&mut independent, &replay).unwrap();
        assert_eq!(&*applied, c.assignment());
    }
    assert_eq!(independent, before);
    mixed.apply(&mut independent, &replay).unwrap().commit();
    assert_eq!(c.assignment(), &independent);
}
