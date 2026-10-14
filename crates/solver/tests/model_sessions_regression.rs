#[path = "fixtures/model_sessions_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::{Arc, Condvar, Mutex, atomic::Ordering};
use timetuck::*;
#[test]
fn both_backends_independently_score_live_ranges_without_mutating_inputs() {
    for stream in [false, true] {
        let audit = Arc::new(Audit::default());
        let session = session(
            binding(),
            DistinctValues,
            stream,
            audit.clone(),
            |s, score| s.score = Some(score),
        );
        let mut manager = session.build_manager(2).unwrap();
        let forced = Arc::new(problem(&["only"], 3));
        let free = Arc::new(problem(&["a", "b", "c"], 3));
        let first = manager
            .job_builder(10)
            .with_problem(forced.clone())
            .run()
            .unwrap();
        let second = manager
            .job_builder(11)
            .with_problem(free.clone())
            .run()
            .unwrap();
        for (job, expected) in [(first, -6), (second, 0)] {
            let output = job.wait();
            let solution = output.as_ref().as_ref().unwrap();
            assert_eq!(solution.score, Some(SimpleScore::new(expected)));
            assert_eq!(solution.score, Some(duplicate_score(solution)));
        }
        assert!(
            forced
                .entities
                .iter()
                .chain(&free.entities)
                .all(|e| e.value.is_none())
        );
        assert!(forced.score.is_none() && free.score.is_none());
        assert_eq!(audit.opened.load(Ordering::SeqCst), 2);
        assert_eq!(audit.dropped.load(Ordering::SeqCst), 2);
        manager.shutdown().unwrap();
    }
}
#[test]
fn bindings_access_live_trait_values_and_replaced_integer_ranges() {
    for stream in [false, true] {
        let session = session(
            interface_binding(),
            MinimizeIntegers,
            stream,
            Arc::default(),
            |s, score| s.score = Some(score),
        );
        let mut manager = session.build_manager(1).unwrap();
        let mut original = interface_problem();
        original.values = vec![7, 9];
        let original = Arc::new(original);
        let output = manager
            .job_builder(2)
            .with_problem(original.clone())
            .run()
            .unwrap()
            .wait();
        let solution = output.as_ref().as_ref().unwrap();
        assert_eq!(solution.score, Some(SimpleScore::new(-14)));
        assert!(solution.entities.iter().all(|e| e.value() == Some(7)));
        assert!(original.entities.iter().all(|e| e.value().is_none()));
        manager.shutdown().unwrap();
    }
}
struct Gate {
    state: Mutex<(bool, bool)>,
    changed: Condvar,
}
impl Gate {
    fn enter(&self) {
        let mut state = self.state.lock().unwrap();
        if !state.0 {
            state.0 = true;
            self.changed.notify_all();
            while !state.1 {
                state = self.changed.wait(state).unwrap();
            }
        }
    }
    fn await_entry(&self) {
        let (state, timeout) = self
            .changed
            .wait_timeout_while(
                self.state.lock().unwrap(),
                std::time::Duration::from_secs(10),
                |state| !state.0,
            )
            .unwrap();
        assert!(state.0 && !timeout.timed_out());
    }
    fn release(&self) {
        self.state.lock().unwrap().1 = true;
        self.changed.notify_all();
    }
}
struct WaitingDistinct(Arc<Gate>);
impl Constraint<String> for WaitingDistinct {
    fn dense(&self, rows: &[Option<&String>]) -> SimpleScore {
        if rows.iter().any(|v| v.is_some()) {
            self.0.enter();
        }
        DistinctValues.dense(rows)
    }
    fn stream<'a>(
        &'a self,
        g: &mut StreamGraphBuilder<'a, SimpleScore>,
        s: &StreamSource<usize, (usize, Option<String>)>,
    ) -> Result<(), GraphError> {
        let gated = g.filter(s.port(), move |(_, v)| {
            if v.is_some() {
                self.0.enter();
            }
            true
        })?;
        let join = g.join_equal(&gated, &gated, |(_, v)| v.clone(), |(_, v)| v.clone())?;
        let pairs = g.filter(&join, |((i, a), (j, _))| i != j && a.is_some())?;
        g.score("Don't assign 2 entities the same value.", &pairs, |_| {
            SimpleScore::new(-1)
        })?;
        Ok(())
    }
}
#[test]
fn queued_and_running_cancellation_use_real_model_jobs_and_manager_recovers() {
    for stream in [false, true] {
        let gate = Arc::new(Gate {
            state: Mutex::new((false, false)),
            changed: Condvar::new(),
        });
        let audit = Arc::new(Audit::default());
        let session = session(
            binding(),
            WaitingDistinct(gate.clone()),
            stream,
            audit.clone(),
            |s, score| s.score = Some(score),
        );
        let mut manager = session.build_manager(1).unwrap();
        let original = Arc::new(problem(&["a", "b"], 2));
        let active = manager
            .job_builder(1)
            .with_problem(original.clone())
            .run()
            .unwrap();
        gate.await_entry();
        let queued = manager
            .job_builder(2)
            .with_problem(original.clone())
            .run()
            .unwrap();
        assert!(queued.cancel());
        assert!(matches!(
            queued.wait().as_ref(),
            Err(SolverJobError::CancelledBeforeStart)
        ));
        assert!(active.cancel());
        gate.release();
        let output = active.wait();
        let solution = output.as_ref().as_ref().unwrap();
        assert_eq!(solution.score, Some(duplicate_score(solution)));
        assert!(original.entities.iter().all(|e| e.value.is_none()));
        for id in [1, 2] {
            assert_eq!(manager.status(id), SolverStatus::NotSolving);
        }
        assert_eq!(audit.opened.load(Ordering::SeqCst), 1);
        assert_eq!(audit.dropped.load(Ordering::SeqCst), 1);
        let next = manager
            .job_builder(1)
            .with_problem(original.clone())
            .run()
            .unwrap()
            .wait();
        let next = next.as_ref().as_ref().unwrap();
        assert_eq!(next.score, Some(SimpleScore::ZERO));
        assert_eq!(next.score, Some(duplicate_score(next)));
        assert_eq!(audit.opened.load(Ordering::SeqCst), 2);
        assert_eq!(audit.dropped.load(Ordering::SeqCst), 2);
        manager.shutdown().unwrap();
    }
}
