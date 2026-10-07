// DefaultSolverTest.solveWithProblemChange and named test fixtures adapted from
// pinned Timefold Community under Apache-2.0; see compat/provenance.json.
use std::{sync::mpsc, time::Duration};
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);
#[derive(Clone)]
struct Facts {
    values: Vec<i32>,
    entities: Vec<(u64, Option<i32>, bool)>,
}
#[derive(Clone, Debug)]
struct Solution {
    values: Vec<i32>,
    assignments: AssignmentSnapshot<u64, i32>,
    start: Vec<Option<i32>>,
    score: Option<SimpleScore>,
    complete: bool,
}
fn run(
    mode: usize,
    facts: &Facts,
    previous: Option<&Solution>,
    token: &CancellationToken,
) -> Result<Solution, String> {
    let ids: Vec<_> = facts.entities.iter().map(|e| e.0).collect();
    let p = FiniteProblem::new(
        facts
            .entities
            .iter()
            .map(|(_, initial, pin)| {
                let v = Variable::new(facts.values.clone()).with_pinned(*pin);
                match initial {
                    Some(value) => v.with_initial(*value),
                    None => v,
                }
            })
            .collect(),
    )
    .map_err(|e| e.to_string())?;
    let p =
        reconcile_problem(&p, &ids, previous.map(|s| &s.assignments)).map_err(|e| e.to_string())?;
    let start = p.initial_assignment().values().to_vec();
    let config = SolverConfig {
        limits: SolverLimits {
            cancellation: Some(token),
            ..Default::default()
        },
        local_search: vec![LocalSearchConfig {
            max_steps: 10,
            ..Default::default()
        }],
        ..Default::default()
    };
    let result = if mode == 0 {
        let n = ConstraintNetwork::<_, SimpleScore>::new(ids.len());
        solve(&p, &n, config)
    } else {
        solve_stream(
            &p,
            || {
                let mut b = StreamGraphBuilder::new();
                let source = b.source::<usize, (usize, Option<i32>)>();
                b.score("zero", source.port(), |_| SimpleScore::ZERO)?;
                Ok((b.build(), source))
            },
            config,
        )
    }
    .map_err(|e| e.to_string())?;
    Ok(Solution {
        values: facts.values.clone(),
        assignments: AssignmentSnapshot::new(ids, result.assignment).map_err(|e| e.to_string())?,
        start,
        score: result.score,
        complete: result.complete,
    })
}
fn initial() -> Facts {
    Facts {
        values: vec![0, 1, 2, 3],
        entities: (0..4).map(|i| (i as u64, Some(i), false)).collect(),
    }
}
#[test]
fn solve_with_problem_change() {
    for mode in 0..2 {
        let mut manager = LiveSolverManager::<Facts, Solution, String>::new(1).unwrap();
        let (send, recv) = mpsc::channel();
        let job = manager
            .submit(
                1,
                initial(),
                LiveSolverConfig {
                    daemon: true,
                    ..Default::default()
                },
                move |facts, previous, token| run(mode, facts, previous, token),
                move |_, solution| {
                    if solution.values.len() == 5 {
                        send.send(solution.clone()).unwrap();
                    }
                },
            )
            .unwrap();
        // Submit immediately, allowing either pre-run batching or an interrupted run.
        let receipt = job
            .add_problem_change(|facts| {
                facts.values.push(4);
                Ok(())
            })
            .unwrap();
        assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
        let updated = recv.recv_timeout(WAIT).unwrap();
        assert_eq!(updated.values.len(), 5);
        assert!(updated.complete);
        assert_eq!(updated.score, Some(SimpleScore::ZERO));
        job.cancel();
        let final_result = job.wait_timeout(WAIT).unwrap();
        let final_result = final_result.as_ref().as_ref().unwrap();
        assert_eq!(final_result.revision, receipt.revision);
        assert_eq!(final_result.solution.values.len(), 5);
        assert!(job.callback_failures().is_empty());
        manager.shutdown().unwrap();
    }
}
// Original integration regression: no additional upstream-method credit.
#[test]
fn daemon_reconciles_entity_and_domain_changes_without_mutating_published_snapshots() {
    for mode in 0..2 {
        let mut manager = LiveSolverManager::<Facts, Solution, String>::new(1).unwrap();
        let (send, recv) = mpsc::channel();
        let job = manager
            .submit(
                2,
                initial(),
                LiveSolverConfig {
                    daemon: true,
                    ..Default::default()
                },
                move |facts, previous, token| run(mode, facts, previous, token),
                move |_, solution| send.send(solution.clone()).unwrap(),
            )
            .unwrap();
        let first = recv.recv_timeout(WAIT).unwrap();
        let receipt = job
            .add_problem_change(|facts| {
                facts.values = vec![0, 2, 4];
                facts.entities = vec![(3, Some(2), true), (0, None, false), (9, Some(4), false)];
                Ok(())
            })
            .unwrap();
        assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
        let second = recv.recv_timeout(WAIT).unwrap();
        assert_eq!(second.assignments.ids(), &[3, 0, 9]);
        assert_eq!(second.start, vec![Some(2), Some(0), Some(4)]);
        assert_eq!(second.assignments.assignment().values(), second.start);
        let receipt = job
            .add_problem_change(|facts| {
                facts.values = vec![9];
                for entity in &mut facts.entities {
                    entity.1 = None;
                    entity.2 = false;
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
        let third = recv.recv_timeout(WAIT).unwrap();
        assert_eq!(third.start, vec![None; 3]);
        assert_eq!(third.assignments.assignment().values(), &[Some(9); 3]);
        assert!(third.complete);
        assert_eq!(first.values, vec![0, 1, 2, 3]);
        assert_eq!(
            first.assignments.assignment().values(),
            &[Some(0), Some(1), Some(2), Some(3)]
        );
        assert_eq!(
            second.assignments.assignment().values(),
            &[Some(2), Some(0), Some(4)]
        );
        job.cancel();
        let final_result = job.wait_timeout(WAIT).unwrap();
        assert_eq!(final_result.as_ref().as_ref().unwrap().revision, 2);
        assert!(job.callback_failures().is_empty());
        manager.shutdown().unwrap();
    }
}
