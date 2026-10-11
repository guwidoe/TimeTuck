// Adapted from Community SolverFactoryTest and its TEST-only helpers.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/assembled_factory_fixture.rs"]
mod fixture;
use fixture::*;
use std::{cell::Cell, rc::Rc, time::Duration};
use timetuck::*;

#[test]
fn create() {
    let d = Domain::new(2, 3, Objective::Zero);
    let f = prepare(&d, false, None, vec![]);
    let mut solver = f
        .assembled_solver_builder()
        .build_assembled_run(d.model.initial_assignment())
        .unwrap();
    assert!(solver.inspect_assembled_resource(|_| ()).is_some());
    // A positive runtime control beyond the source's non-null builder assertion.
    assert!(
        solver
            .run_next_assembled_phase(|r, p| r.phase(p))
            .unwrap()
            .is_some()
    );
    assert_eq!(
        solver.finish_assembled_state().unwrap().1,
        SimpleScore::ZERO
    );
}
struct Override {
    reads: Cell<usize>,
}
impl FactoryRunOverride for Override {
    fn factory_run_limits(&self) -> FactoryRunLimits {
        self.reads.set(self.reads.get() + 1);
        FactoryRunLimits {
            max_time: Some(Duration::from_secs(60)),
            ..Default::default()
        }
    }
}
#[test]
fn create_and_override_settings() {
    let d = Domain::new(2, 3, Objective::Zero);
    let f = prepare(&d, false, None, vec![]);
    let settings = Override {
        reads: Cell::new(0),
    };
    let solver = f
        .assembled_solver_builder()
        .with_factory_override(&settings)
        .build_assembled_run(d.model.initial_assignment())
        .unwrap();
    assert!(solver.inspect_assembled_resource(|_| ()).is_some());
    assert!(settings.reads.get() >= 1);
    assert_eq!(
        solver.assembled_run_limits().max_time,
        Some(Duration::from_secs(60))
    );
    assert_eq!(
        f.assembled_solver_builder().factory_run_limits(),
        FactoryRunLimits::default()
    );
}
#[test]
fn get_score_director_factory() {
    // Complete source fixture: three unassigned entities and two shared values;
    // the TEST dummy calculator returns zero, including for uninitialized input.
    let d = Domain::new(2, 3, Objective::Zero);
    let f = prepare(&d, false, None, vec![]);
    let bank = f.assembled_scoring_factory();
    let director = bank
        .score_factory_builder(Assignment::new(vec![None; 3]))
        .instantiate_score_director()
        .unwrap();
    assert_eq!(
        director.factory_resource().context_score(),
        SimpleScore::ZERO.with_structural(-3)
    );
}
fn unbounded_local() -> SolverPhase<'static, SimpleScore> {
    SolverPhase::LocalSearch(LocalSearchConfig {
        max_steps: u64::MAX,
        max_accepted_moves: u64::MAX,
        max_evaluations: u64::MAX,
        ..Default::default()
    })
}
#[test]
fn local_search_after_unterminated_local_search() {
    let d = Domain::new(2, 3, Objective::Zero);
    let mut c = configuration(&d, None, vec![], 10);
    c.phases = vec![
        SolverPhase::Construction(Default::default()),
        unbounded_local(),
        unbounded_local(),
    ];
    let error = declaration(&d, false, Rc::new(Observations::default()))
        .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
        .err()
        .unwrap();
    assert!(error.to_string().contains("unreachable phase"));
    assert!(matches!(
        error,
        FactoryAssemblyError::UnreachablePhase {
            phase: 2,
            predecessor: 1
        }
    ));
}
