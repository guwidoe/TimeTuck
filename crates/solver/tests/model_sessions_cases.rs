// Complete operational Community TEST adaptations; Apache-2.0 attribution recorded.
#[path = "fixtures/model_sessions_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::{Arc, atomic::Ordering};
use timetuck::*;
#[test]
fn two_failed_solves_leave_the_solver_manager_usable() {
    for stream in [false, true] {
        let audit = Arc::new(Audit::default());
        let session = session(
            binding(),
            DeliberateFailure(audit.clone()),
            stream,
            audit.clone(),
            |s, score| s.score = Some(score),
        );
        let mut manager = session.build_manager(1).unwrap();
        for id in [1, 2] {
            let original = Arc::new(problem(&[FAILING_VALUE], 2));
            let job = manager
                .job_builder(id)
                .with_problem(original.clone())
                .run()
                .unwrap();
            assert!(
                matches!(job.wait().as_ref(),Err(SolverJobError::Panicked(message)) if message.contains(FAILURE_MESSAGE))
            );
            assert_eq!(manager.status(id), SolverStatus::NotSolving);
            assert_eq!(original.entities.len(), 2);
            assert!(original.entities.iter().all(|e| e.value.is_none()));
            assert_eq!(audit.assigned_failures.load(Ordering::SeqCst), id as usize);
            assert_eq!(audit.opened.load(Ordering::SeqCst), id as usize);
            assert_eq!(audit.dropped.load(Ordering::SeqCst), id as usize);
        }
        let job = manager
            .job_builder(3)
            .with_problem(Arc::new(problem(&["v1", "v2"], 2)))
            .run()
            .unwrap();
        let output = job.wait();
        let solution = output.as_ref().as_ref().unwrap();
        assert!(solution.score.is_some());
        assert_eq!(solution.score, Some(SimpleScore::ZERO));
        assert!(solution.entities.iter().all(|e| e.value.is_some()));
        assert_eq!(manager.status(3), SolverStatus::NotSolving);
        assert_eq!(audit.opened.load(Ordering::SeqCst), 3);
        assert_eq!(audit.dropped.load(Ordering::SeqCst), 3);
        manager.shutdown().unwrap();
    }
}
#[test]
fn normal_solve() {
    for stream in [false, true] {
        let session = session(
            binding(),
            DistinctValues,
            stream,
            Arc::default(),
            |s, score| s.score = Some(score),
        );
        let mut manager = session.build_manager(1).unwrap();
        let output = manager
            .job_builder(1)
            .with_problem(Arc::new(problem(&["v1", "v2"], 2)))
            .run()
            .unwrap()
            .wait();
        let solution = output.as_ref().as_ref().unwrap();
        assert!(solution.score.unwrap().score() >= 0);
        assert_eq!(solution.score, Some(duplicate_score(solution)));
        assert_eq!(solution.entities.len(), 2);
        manager.shutdown().unwrap();
    }
}
#[test]
fn extended_solution_solve() {
    for stream in [false, true] {
        let session = session(
            extended_binding(),
            DistinctValues,
            stream,
            Arc::default(),
            |s, score| s.base.score = Some(score),
        );
        let mut manager = session.build_manager(1).unwrap();
        let input = Arc::new(ExtendedSolution {
            base: problem(&["v1", "v2"], 2),
            extra_data: "Extra Data".into(),
        });
        let output = manager
            .job_builder(1)
            .with_problem(input.clone())
            .run()
            .unwrap()
            .wait();
        let solution = output.as_ref().as_ref().unwrap();
        assert!(solution.base.score.unwrap().score() >= 0);
        assert_eq!(solution.extra_data, "Extra Data");
        assert_eq!(solution.base.score, Some(duplicate_score(&solution.base)));
        assert!(input.base.entities.iter().all(|e| e.value.is_none()));
        manager.shutdown().unwrap();
    }
}
#[test]
fn interface_entity_build_solver() {
    for stream in [false, true] {
        let session = session(
            interface_binding(),
            MinimizeIntegers,
            stream,
            Arc::default(),
            |s, score| s.score = Some(score),
        );
        let mut manager = session.build_manager(1).unwrap();
        let input = Arc::new(interface_problem());
        let output = manager
            .job_builder(1)
            .with_problem(input.clone())
            .run()
            .unwrap()
            .wait();
        let solution = output.as_ref().as_ref().unwrap();
        assert_eq!(solution.entities.len(), input.entities.len());
        for (original, solved) in input.entities.iter().zip(&solution.entities) {
            assert!(!std::ptr::eq::<dyn ValueEntity>(
                original.as_ref(),
                solved.as_ref()
            ));
            assert!(original.value().is_none());
        }
        assert_eq!(
            solution.score,
            Some(SimpleScore::new(
                -solution
                    .entities
                    .iter()
                    .map(|e| i64::from(e.value().unwrap()))
                    .sum::<i64>()
            ))
        );
        manager.shutdown().unwrap();
    }
}
#[test]
fn read_only_concrete_provider_class() {
    for stream in [false, true] {
        let bank = concrete_provider_bank(stream);
        let factory = bank
            .factory::<Arc<Solution>, Solution, ModelSessionError<String>, SimpleScore>("native")
            .unwrap();
        let mut manager = factory.build_manager(1).unwrap();
        let input = Arc::new(problem(
            &[
                "Generated Value 0",
                "Generated Value 1",
                "Generated Value 2",
            ],
            2,
        ));
        let output = manager
            .job_builder(1)
            .with_problem(input)
            .run()
            .unwrap()
            .wait();
        let solution = output.as_ref().as_ref().unwrap();
        assert_eq!(solution.entities.len(), 2);
        assert_eq!(solution.score, Some(duplicate_score(solution)));
        manager.shutdown().unwrap();
    }
}
