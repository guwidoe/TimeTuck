//! Original actual branch replay, checked insertion and retry/panic controls.
use std::panic::{AssertUnwindSafe, catch_unwind};
use timetuck::*;
fn record(
    p: &FiniteProblem<i32>,
    a: &Assignment<i32>,
    value: i32,
) -> RecordedMove<Assignment<i32>, Move<i32>> {
    let n = ConstraintNetwork::<i32, SimpleScore>::new(1);
    let mut d = ScoreDirector::new(p, &n, a.clone()).unwrap();
    TemporaryMoves::new(&mut d)
        .recorded(
            &Move::Change {
                variable: 0,
                value: Some(value),
            },
            TemporaryMoveOptions::default(),
            |_, _| Ok::<_, ()>(()),
        )
        .unwrap()
        .1
}
#[test]
fn failures_preserve_completed_branch_progress_for_retry_and_observer_panics() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2, 3]).with_initial(0)]).unwrap();
    let root = p.initial_assignment();
    let mut tree = BranchRestorer::new(root.clone());
    let a = tree.add_child(0, record(&p, &root, 1)).unwrap();
    let b = tree
        .add_child(a, record(&p, tree.assignment_at(a).unwrap(), 2))
        .unwrap();
    let c = tree.add_child(0, record(&p, &root, 3)).unwrap();
    let mut actual = root.clone();
    tree.restore(
        b,
        BranchRestoreMode::CommonAncestor,
        &mut actual,
        |a, m| {
            p.apply(a, m).unwrap().commit();
            Ok::<_, &str>(())
        },
        |_, _| {},
    )
    .unwrap();
    let mut executions = 0;
    let mut events = vec![];
    let error = tree
        .restore(
            c,
            BranchRestoreMode::CommonAncestor,
            &mut actual,
            |a, m| {
                executions += 1;
                if executions == 2 {
                    return Err("blocked");
                }
                p.apply(a, m).unwrap().commit();
                Ok(())
            },
            |i, d| events.push((i, d)),
        )
        .unwrap_err();
    assert_eq!(error, BranchRestoreError::Execute("blocked"));
    assert_eq!(tree.current_node(), a);
    assert_eq!(actual.values(), &[Some(1)]);
    assert_eq!(events, vec![(b, ReplayDirection::Undo)]);
    tree.restore(
        c,
        BranchRestoreMode::CommonAncestor,
        &mut actual,
        |a, m| {
            p.apply(a, m).unwrap().commit();
            Ok::<_, ()>(())
        },
        |_, _| {},
    )
    .unwrap();
    assert_eq!(actual.values(), &[Some(3)]);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            tree.restore(
                0,
                BranchRestoreMode::CommonAncestor,
                &mut actual,
                |a, m| {
                    p.apply(a, m).unwrap().commit();
                    Ok::<_, ()>(())
                },
                |_, _| panic!("observer"),
            )
            .unwrap();
        }))
        .is_err()
    );
    assert_eq!(tree.current_node(), 0);
    assert_eq!(actual, root);
    let mut calls = 0;
    tree.restore(
        0,
        BranchRestoreMode::CommonAncestor,
        &mut actual,
        |_, _| {
            calls += 1;
            Ok::<_, ()>(())
        },
        |_, _| panic!(),
    )
    .unwrap();
    assert_eq!(calls, 0);
}
#[test]
fn invalid_parent_target_preimage_and_execution_result_are_checked() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let root = p.initial_assignment();
    let mut tree = BranchRestorer::new(root.clone());
    let a = tree.add_child(0, record(&p, &root, 1)).unwrap();
    assert_eq!(
        tree.add_child(a, record(&p, &root, 2)).unwrap_err(),
        BranchRestoreError::ParentState
    );
    assert_eq!(
        tree.add_child(99, record(&p, &root, 2)).unwrap_err(),
        BranchRestoreError::UnknownNode { node: 99 }
    );
    let mut actual = root.clone();
    assert_eq!(
        tree.restore(
            99,
            BranchRestoreMode::CommonAncestor,
            &mut actual,
            |_, _| Ok::<_, ()>(()),
            |_, _| {}
        )
        .unwrap_err(),
        BranchRestoreError::UnknownNode { node: 99 }
    );
    actual = Assignment::new(vec![Some(2)]);
    assert_eq!(
        tree.restore(
            a,
            BranchRestoreMode::CommonAncestor,
            &mut actual,
            |_, _| Ok::<_, ()>(()),
            |_, _| {}
        )
        .unwrap_err(),
        BranchRestoreError::ParentState
    );
    actual = root.clone();
    assert_eq!(
        tree.restore(
            a,
            BranchRestoreMode::CommonAncestor,
            &mut actual,
            |_, _| Ok::<_, ()>(()),
            |_, _| {}
        )
        .unwrap_err(),
        BranchRestoreError::UnexpectedResult
    );
    assert_eq!(tree.current_node(), 0);
    assert_eq!(actual, root);
}
