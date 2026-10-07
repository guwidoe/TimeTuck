//! Original live list/mixed restart coverage; no upstream method credit.
use std::{sync::mpsc, time::Duration};
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);
#[derive(Clone)]
struct Facts {
    ids: Vec<u64>,
    lists: ListProblem<i32>,
    basic_ids: Vec<u64>,
    basic: FiniteProblem<i32>,
}
#[derive(Clone, Debug)]
struct ResultSnapshot {
    lists: ListAssignmentSnapshot<u64, i32>,
    basic: AssignmentSnapshot<u64, i32>,
    list_start: ListAssignment<i32>,
    basic_start: Assignment<i32>,
    score: Option<SimpleScore>,
    complete: bool,
}
fn run(
    mode: usize,
    facts: &Facts,
    old: Option<&ResultSnapshot>,
    token: &CancellationToken,
) -> Result<ResultSnapshot, String> {
    let p = reconcile_list_problem(&facts.lists, &facts.ids, old.map(|s| &s.lists))
        .map_err(|e| e.to_string())?;
    let b = reconcile_problem(&facts.basic, &facts.basic_ids, old.map(|s| &s.basic))
        .map_err(|e| e.to_string())?;
    let list_start = p.initial_assignment();
    let basic_start = b.initial_assignment();
    let limits = SolverLimits {
        cancellation: Some(token),
        ..Default::default()
    };
    let (list_result, basic_result, score, complete) = if mode < 2 {
        let config = SolverConfig {
            limits,
            ..Default::default()
        };
        let result = if mode == 0 {
            let n =
                ListConstraintNetwork::<_, SimpleScore>::new(p.entities().len(), p.values().len());
            solve_list(&p, &n, config)
        } else {
            solve_list_stream(
                &p,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let source = ListStreamSources::<i32>::new(&mut b);
                    b.score("zero", source.values.port(), |_| SimpleScore::ZERO)?;
                    Ok((b.build(), source))
                },
                config,
            )
        }
        .map_err(|e| e.to_string())?;
        (
            result.assignment,
            basic_start.clone(),
            result.score,
            result.complete,
        )
    } else {
        let p = MixedProblem::new(b, p);
        let config = MixedSolverConfig {
            limits,
            ..Default::default()
        };
        let result = if mode == 2 {
            let n = MixedConstraintNetwork::<_, _, SimpleScore>::new(
                p.basic().variables().len(),
                p.lists().entities().len(),
                p.lists().values().len(),
            );
            solve_mixed(&p, &n, config)
        } else {
            solve_mixed_stream(
                &p,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let source = MixedStreamSources::<i32, i32>::new(&mut b);
                    b.score("zero", source.lists.values.port(), |_| SimpleScore::ZERO)?;
                    Ok((b.build(), source))
                },
                config,
            )
        }
        .map_err(|e| e.to_string())?;
        (
            result.assignment.lists().clone(),
            result.assignment.basic().clone(),
            result.score,
            result.complete,
        )
    };
    Ok(ResultSnapshot {
        lists: ListAssignmentSnapshot::new(facts.ids.clone(), list_result)
            .map_err(|e| e.to_string())?,
        basic: AssignmentSnapshot::new(facts.basic_ids.clone(), basic_result)
            .map_err(|e| e.to_string())?,
        list_start,
        basic_start,
        score,
        complete,
    })
}
fn initial() -> Facts {
    Facts {
        ids: vec![10, 20],
        lists: ListProblem::new(
            vec![0, 1, 2, 3],
            vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![2, 3])],
        )
        .unwrap(),
        basic_ids: vec![10, 20],
        basic: FiniteProblem::new(vec![
            Variable::new(vec![1, 2]).with_initial(1),
            Variable::new(vec![1, 2]).with_initial(2),
        ])
        .unwrap(),
    }
}
#[test]
fn daemon_restarts_list_and_mixed_solvers_with_identity_pins_and_range_changes() {
    for mode in 0..4 {
        let mut manager = LiveSolverManager::<Facts, ResultSnapshot, String>::new(1).unwrap();
        let (send, recv) = mpsc::channel();
        let job = manager
            .submit(
                1,
                initial(),
                LiveSolverConfig {
                    daemon: true,
                    ..Default::default()
                },
                move |p, old, token| run(mode, p, old, token),
                move |_, s| send.send(s.clone()).unwrap(),
            )
            .unwrap();
        let first = recv.recv_timeout(WAIT).unwrap();
        assert_eq!(first.lists.assignment().lists(), &[vec![0, 1], vec![2, 3]]);
        let receipt = job
            .add_problem_change(|p| {
                p.ids = vec![20, 30, 10];
                p.lists = ListProblem::new(
                    vec![0, 2, 3, 4],
                    vec![
                        ListEntity::new(vec![0])
                            .with_pinned_prefix(1)
                            .with_value_range(vec![0, 2]),
                        ListEntity::new(vec![4])
                            .with_pinned(true)
                            .with_value_range(vec![4]),
                        ListEntity::new(vec![]).with_value_range(vec![3]),
                    ],
                )
                .unwrap();
                p.basic_ids = vec![20, 10];
                p.basic = FiniteProblem::new(vec![
                    Variable::new(vec![1, 2]).with_initial(1).with_pinned(true),
                    Variable::new(vec![1, 2]),
                ])
                .unwrap();
                Ok(())
            })
            .unwrap();
        assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
        let second = recv.recv_timeout(WAIT).unwrap();
        assert_eq!(second.lists.ids(), &[20, 30, 10]);
        assert_eq!(second.list_start.lists(), &[vec![0, 2], vec![4], vec![]]);
        assert_eq!(
            second.lists.assignment().lists(),
            &[vec![0, 2], vec![4], vec![3]]
        );
        assert_eq!(second.basic_start.values(), &[Some(1), Some(1)]);
        assert_eq!(second.basic.assignment().values(), &[Some(1), Some(1)]);
        assert!(second.complete);
        assert_eq!(second.score, Some(SimpleScore::ZERO));
        let receipt = job
            .add_problem_change(|p| {
                p.ids = vec![10, 30];
                p.lists = ListProblem::new(
                    vec![3, 4, 5],
                    vec![
                        ListEntity::new(vec![]).with_value_range(vec![3, 5]),
                        ListEntity::new(vec![4])
                            .with_pinned(true)
                            .with_value_range(vec![4]),
                    ],
                )
                .unwrap();
                Ok(())
            })
            .unwrap();
        assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
        let third = recv.recv_timeout(WAIT).unwrap();
        assert_eq!(third.list_start.lists(), &[vec![3], vec![4]]);
        assert!(third.complete);
        assert_eq!(third.score, Some(SimpleScore::ZERO));
        assert_eq!(third.lists.assignment().lists()[1], vec![4]);
        let mut row = third.lists.assignment().lists()[0].clone();
        row.sort();
        assert_eq!(row, vec![3, 5]);
        assert_eq!(first.lists.assignment().lists(), &[vec![0, 1], vec![2, 3]]);
        assert_eq!(
            second.lists.assignment().lists(),
            &[vec![0, 2], vec![4], vec![3]]
        );
        job.cancel();
        let result = job.wait_timeout(WAIT).unwrap();
        assert_eq!(result.as_ref().as_ref().unwrap().revision, 2);
        assert!(job.callback_failures().is_empty());
        manager.shutdown().unwrap();
    }
}
