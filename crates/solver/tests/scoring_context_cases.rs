// Adapted from Community SolverContextManagerTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use EnvironmentMode as M;
use std::{cell::Cell, fmt, rc::Rc};
use timetuck::*;
fn graph() -> (
    StreamGraph<'static, SimpleScore>,
    PlanningStreamSource<usize>,
) {
    let mut builder = StreamGraphBuilder::new();
    let source = builder.source::<usize, (usize, Option<usize>)>();
    // DummyConstraintProvider declares no scoring constraints.
    (builder.build(), source)
}
macro_rules! check {
    ($phases:expr,$metrics:expr, |$manager:ident| $body:block) => {{
        // TestdataSolution.generateSolution(3,3): shared three-value range,
        // three initialized entities, with values assigned by index modulo 3.
        let problem = FiniteProblem::new(
            (0..3)
                .map(|i| Variable::new(vec![0, 1, 2]).with_initial(i))
                .collect(),
        )
        .unwrap();
        let network = ConstraintNetwork::<_, SimpleScore>::new(3);
        let director =
            ScoreDirector::new(&problem, &network, problem.initial_assignment()).unwrap();
        {
            let mut $manager = director.into_context(M::PhaseAssert, $phases.to_vec(), $metrics);
            $body
        }
        let (fresh_graph, source) = graph();
        let director =
            StreamScoreDirector::new(&problem, fresh_graph, source, problem.initial_assignment())
                .unwrap();
        {
            let mut $manager =
                director.into_context(M::PhaseAssert, $phases.to_vec(), $metrics, |_| Ok(graph()));
            $body
        }
    }};
}
#[test]
fn same_mode_keeps_the_score_and_change_directors() {
    check!([M::PhaseAssert], ContextMetrics::default(), |manager| {
        let score = manager.handle();
        let changes = manager.change_director();
        manager.start().unwrap();
        manager.start_phase(0).unwrap();
        assert!(manager.handle().same_resource(&score));
        assert!(manager.change_director().same_resource(&changes));
        assert!(score.director().is_some());
    });
}
#[test]
fn another_mode_replaces_both_directors_and_transfers_working_state() {
    check!([M::FullAssert], ContextMetrics::default(), |manager| {
        let score = manager.handle();
        let changes = manager.change_director();
        let model = score.director().unwrap().working_model() as *const _;
        let assignment = score.director().unwrap().assignment().values().as_ptr();
        manager.start().unwrap();
        manager.start_phase(0).unwrap();
        let next = manager.handle();
        assert!(!next.same_resource(&score));
        assert!(!manager.change_director().same_resource(&changes));
        assert_eq!(next.settings().mode, M::FullAssert);
        // The native working model is borrowed at the same address; its mutable
        // assignment Vec is moved into the new resource without cloning it.
        assert_eq!(next.director().unwrap().working_model() as *const _, model);
        assert_eq!(
            next.director().unwrap().assignment().values().as_ptr(),
            assignment
        );
        assert_eq!(
            next.director().unwrap().assignment().values(),
            &[Some(0), Some(1), Some(2)]
        );
    });
}
#[test]
fn swap_keeps_matching_needed_by_metrics() {
    check!(
        [M::PhaseAssert, M::NoAssert],
        ContextMetrics {
            best_constraint_totals: true,
            ..Default::default()
        },
        |manager| {
            manager.start().unwrap();
            manager.start_phase(1).unwrap();
            assert_eq!(manager.handle().settings().mode, M::NoAssert);
            assert_eq!(
                manager.handle().settings().matching,
                ContextMatchPolicy::Enabled
            );
            assert_eq!(manager.constraint_totals(), Some(vec![]));
        }
    );
}
#[test]
fn swap_without_match_metrics_leaves_matching_off() {
    check!(
        [M::PhaseAssert, M::NoAssert],
        ContextMetrics {
            best_score: true,
            ..Default::default()
        },
        |manager| {
            manager.start().unwrap();
            manager.start_phase(1).unwrap();
            assert_eq!(
                manager.handle().settings().matching,
                ContextMatchPolicy::Disabled
            );
            assert!(manager.constraint_totals().is_none());
        }
    );
}
#[test]
fn replaced_director_is_released() {
    check!([M::FullAssert], ContextMetrics::default(), |manager| {
        let previous = manager.handle();
        manager.start().unwrap();
        manager.start_phase(0).unwrap();
        assert!(previous.director().is_none());
    });
}
#[test]
fn calculation_count_carries_over_to_the_new_director() {
    check!([M::FullAssert], ContextMetrics::default(), |manager| {
        manager.calculate_score().unwrap();
        manager.calculate_score().unwrap();
        let before = manager.handle().calculation_count();
        assert!(before > 0);
        manager.start().unwrap();
        manager.start_phase(0).unwrap();
        assert_eq!(manager.handle().calculation_count(), before);
    });
}
#[test]
fn every_mode_change_gets_a_new_director() {
    check!(
        [M::PhaseAssert, M::FullAssert, M::PhaseAssert],
        ContextMetrics::default(),
        |manager| {
            let original = manager.handle();
            manager.start().unwrap();
            manager.start_phase(0).unwrap();
            assert!(manager.handle().same_resource(&original));
            manager.start_phase(1).unwrap();
            let full = manager.handle();
            assert!(!full.same_resource(&original));
            manager.start_phase(2).unwrap();
            let restored = manager.handle();
            assert!(!restored.same_resource(&full));
            assert!(!restored.same_resource(&original));
            assert_eq!(restored.settings().mode, M::PhaseAssert);
        }
    );
}
#[test]
fn solving_error_releases_the_director_in_use() {
    check!([M::FullAssert], ContextMetrics::default(), |manager| {
        manager.start().unwrap();
        manager.start_phase(0).unwrap();
        let used = manager.handle();
        let mut failure = ContextFailure::new("Boom");
        manager.solving_error(&mut failure);
        assert!(used.director().is_none());
        assert_eq!(failure.primary, "Boom");
    });
}
#[derive(Debug)]
struct ReleaseFailure {
    message: &'static str,
    identity: Rc<()>,
}
impl fmt::Display for ReleaseFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}
impl std::error::Error for ReleaseFailure {}
struct FailingResource {
    error: Option<ReleaseFailure>,
    dropped: Rc<Cell<usize>>,
}
impl ContextScoreResource for FailingResource {
    type Score = SimpleScore;
    type Error = ReleaseFailure;
    fn context_score(&self) -> SimpleScore {
        SimpleScore::new(0)
    }
    fn check_context(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn context_totals(&self) -> Vec<(String, SimpleScore)> {
        vec![]
    }
    fn release_context(&mut self) -> Result<(), Self::Error> {
        Err(self.error.take().unwrap())
    }
}
impl Drop for FailingResource {
    fn drop(&mut self) {
        self.dropped.set(self.dropped.get() + 1);
    }
}
#[test]
fn failed_release_is_suppressed_on_the_original_failure() {
    // Source intentionally mocks a director whose close throws.
    let identity = Rc::new(());
    let dropped = Rc::new(Cell::new(0));
    let resource = FailingResource {
        error: Some(ReleaseFailure {
            message: "Releasing blew up",
            identity: identity.clone(),
        }),
        dropped: dropped.clone(),
    };
    let mut manager = ScoringContextManager::new(
        resource,
        M::PhaseAssert,
        vec![M::PhaseAssert],
        ContextMetrics::default(),
        (),
    );
    let primary = Box::new("The real failure");
    let address = &*primary as *const _;
    let mut failure = ContextFailure::new(primary);
    manager.solving_error(&mut failure);
    assert_eq!(&*failure.primary as *const _, address);
    assert_eq!(*failure.primary, "The real failure");
    assert_eq!(failure.suppressed.len(), 1);
    assert_eq!(failure.suppressed[0].to_string(), "Releasing blew up");
    assert!(Rc::ptr_eq(&failure.suppressed[0].identity, &identity));
    assert_eq!(dropped.get(), 1);
    assert!(manager.handle().director().is_none());
    manager.solving_error(&mut failure);
    assert_eq!(failure.suppressed.len(), 1);
}
#[test]
fn solving_error_before_start_does_not_replace_the_failure() {
    check!([M::PhaseAssert], ContextMetrics::default(), |manager| {
        let mut failure = ContextFailure::new("Boom");
        manager.solving_error(&mut failure);
        assert_eq!(failure.primary, "Boom");
        assert!(failure.suppressed.is_empty());
        assert!(manager.handle().director().is_none());
    });
}
