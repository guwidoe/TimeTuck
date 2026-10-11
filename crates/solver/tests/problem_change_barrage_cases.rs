// ProblemChangeBarrageIT and named test fixtures adapted from pinned Timefold
// Community under Apache-2.0; see compat/provenance.json. No production source used.
use std::{
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::mpsc,
    time::{Duration, Instant},
};
use timetuck::*;

const WAIT: Duration = Duration::from_secs(10);
const SCHEDULE: [(u64, bool); 200] = include!("fixtures/problem_change_barrage_schedule.rs");
type Facts = Vec<(u64, Option<i32>)>;
type Solution = AssignmentSnapshot<u64, i32>;

// TestdataSolution.generateSolution(): five values, seven assigned entities.
fn initial() -> Facts {
    (0..7).map(|i| (i, Some((i % 5) as i32))).collect()
}
fn solve_revision(
    facts: &Facts,
    previous: Option<&Solution>,
    token: &CancellationToken,
    progress: &LiveSolverProgress<'_, Solution>,
) -> Result<Solution, String> {
    let ids: Vec<_> = facts.iter().map(|(id, _)| *id).collect();
    let base = FiniteProblem::new(
        facts
            .iter()
            .map(|(_, value)| {
                let variable = Variable::new((0..5).collect());
                value.map_or(variable.clone(), |value| variable.with_initial(value))
            })
            .collect(),
    )
    .map_err(|e| e.to_string())?;
    let problem = reconcile_problem(&base, &ids, previous).map_err(|e| e.to_string())?;
    let mut network = ConstraintNetwork::<i32, SimpleScore>::new(ids.len());
    // TestdataEasyScoreCalculator: every ordered pair of distinct entities
    // assigned to the same non-null value incurs one penalty.
    network
        .add_constraint("duplicate values", (0..ids.len()).collect(), |values| {
            let mut score = 0;
            for (left, value) in values.iter().enumerate() {
                if value.is_some() {
                    for (right, other) in values.iter().enumerate() {
                        if left != right && value == other {
                            score -= 1;
                        }
                    }
                }
            }
            SimpleScore::new(score)
        })
        .map_err(|e| e.to_string())?;
    // Explicit unlimited cooperative search replaces Java's implicit default
    // phases. Equal-score swaps keep search alive between changes, including at
    // a balanced allocation where no improving change exists.
    let plan = SolverSequence {
        limits: SolverLimits {
            max_evaluations: u64::MAX,
            cancellation: Some(token),
            ..Default::default()
        },
        phases: vec![
            SolverPhase::Construction(Default::default()),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_evaluations: u64::MAX,
                max_accepted_moves: u64::MAX,
                max_steps: u64::MAX,
                neighborhood: Neighborhood::ChangeAndSwap,
                acceptance: Acceptance::HillClimbing,
                ..Default::default()
            }),
        ],
    };
    let result = solve_with_events(&problem, &network, plan, |event| match event {
        SolverEvent::Best(update) => {
            let snapshot = AssignmentSnapshot::new(ids.clone(), update.assignment.clone()).unwrap();
            progress.publish_from(
                snapshot,
                SolverEventProducer {
                    phase: update.phase,
                    kind: update.kind,
                },
            );
        }
        SolverEvent::FirstInitialized {
            initialization,
            assignment,
            ..
        } => {
            let snapshot = AssignmentSnapshot::new(ids.clone(), assignment.clone()).unwrap();
            progress.publish_first_initialized(snapshot, initialization);
        }
    })
    .map_err(|e| e.to_string())?;
    AssignmentSnapshot::new(ids, result.assignment).map_err(|e| e.to_string())
}

struct RecordedReceipt {
    id: usize,
    receipt: ProblemChangeReceipt,
}
// Preserve the source helper's strict wait-until-done semantics. Returning a
// suffix of still-pending receipts is not a substitute for successful waits.
fn outstanding(receipts: &[RecordedReceipt]) -> Vec<usize> {
    receipts
        .iter()
        .filter_map(|record| {
            assert!(
                record.receipt.wait_timeout(WAIT).is_some(),
                "receipt {} did not complete in 10 seconds",
                record.id
            );
            record.receipt.try_result().is_none().then_some(record.id)
        })
        .collect()
}

#[test]
fn problem_change_barrage_intermediate_best_solution_consumer() {
    // Preserve @RepeatedTest(value = 10, failureThreshold = 1).
    for repetition in 0..10 {
        let mut receipts = Vec::with_capacity(200);
        let mut manager = LiveSolverManager::<Facts, Solution, String>::new(1).unwrap();
        let body = catch_unwind(AssertUnwindSafe(|| {
            let (initialized, initialization) = mpsc::channel();
            let job = manager
                .submit_with_events(
                    repetition,
                    initial(),
                    Default::default(),
                    solve_revision,
                    LiveSolverCallbacks::new(|_, _| {}).on_first_initialized(move |_, _, _| {
                        initialized.send(()).unwrap();
                    }),
                )
                .unwrap();
            initialization
                .recv_timeout(WAIT)
                .expect("first construction phase did not finish");
            let (applied, applications) = mpsc::channel();
            for (id, &(delay_nanos, add)) in SCHEDULE.iter().enumerate() {
                let start = Instant::now();
                while start.elapsed() < Duration::from_nanos(delay_nanos) {
                    std::hint::spin_loop();
                }
                let applied = applied.clone();
                let receipt = job
                    .add_problem_change(move |facts| {
                        if add {
                            // Source UUIDs identify new unassigned entities; monotonic
                            // native IDs provide the same uniqueness without RNG drift.
                            facts.push((7 + id as u64, None));
                        } else if facts.len() >= 2 {
                            facts.remove(0);
                        }
                        // The source fixture has no shadow variables. Reconciliation
                        // uses stable IDs on the next actual solve, after this batch.
                        applied.send(()).unwrap();
                        Ok(())
                    })
                    .unwrap();
                receipts.push(RecordedReceipt { id, receipt });
            }
            for _ in 0..200 {
                applications
                    .recv_timeout(WAIT)
                    .expect("an accepted edit was never applied");
            }
            let lost = outstanding(&receipts);
            if !lost.is_empty() {
                assert_eq!(*lost.iter().min().unwrap(), 200 - lost.len());
            }
        }));
        // Preserve try-with-resources plus finally: terminate/join and assert
        // every receipt terminal even when an earlier assertion/panic failed.
        manager.shutdown().unwrap();
        assert!(
            outstanding(&receipts).is_empty(),
            "all receipts must be terminal after shutdown"
        );
        if let Err(payload) = body {
            resume_unwind(payload);
        }
    }
}
