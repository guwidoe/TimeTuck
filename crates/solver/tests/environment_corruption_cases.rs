// Adapted from pinned Community EnvironmentModeTest.corruptedConstraints and
// its corrupted different-values calculator/initializer. Apache-2.0; provenance.
#[path = "fixtures/assembled_factory_fixture.rs"]
mod fixture;
use fixture::*;
use std::{cell::Cell, collections::BTreeSet, io, num::NonZeroUsize, rc::Rc};
use timetuck::*;
#[test]
fn corrupted_constraints() {
    use EnvironmentMode::*;
    for mode in [
        TrackedFullAssert,
        FullAssert,
        NonIntrusiveFullAssert,
        StepAssert,
        PhaseAssert,
        NoAssert,
        NonReproducible,
    ] {
        let model = Rc::new(Model::new(vec![Variable::new(vec![0, 1, 2]); 4]).unwrap());
        let calls = Rc::new(Cell::new(0_i64));
        let count = calls.clone();
        let mut network = ConstraintNetwork::new(4);
        network
            .add_constraint(
                "corrupted different values",
                vec![0, 1, 2, 3],
                move |rows| {
                    let assigned = rows.iter().flatten().count();
                    let unique = rows
                        .iter()
                        .flatten()
                        .copied()
                        .collect::<BTreeSet<_>>()
                        .len();
                    count.set(count.get() + 1);
                    SimpleScore::new(-((assigned - unique) as i64) - count.get())
                },
            )
            .unwrap();
        let domain = Domain {
            model,
            network,
            objective: Objective::Zero,
        };
        let mut c = configuration(&domain, Some(mode), vec![None, None], 1000);
        c.phases = vec![
            SolverPhase::Custom(CustomPhaseConfig::default()),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: u64::MAX,
                max_evaluations: u64::MAX,
                max_move_evaluations: Some(1000),
                move_order: MoveOrder::ShuffledMoves {
                    seed: 0,
                    window: NonZeroUsize::new(32).unwrap(),
                },
                acceptance: Acceptance::LateAcceptance {
                    history_size: 32,
                    hill_climbing: true,
                },
                ..Default::default()
            }),
        ];
        let f = declaration(&domain, false, Rc::new(Observations::default()))
            .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
            .unwrap();
        let result = f.solve_assembled_with(domain.model.initial_assignment(), |r, phase| {
            let Engine::Dense(d) = &mut r.engine else {
                panic!()
            };
            let mut command = |_,
                               context: &mut CustomPhaseContext<
                '_,
                Assignment<i64>,
                Move<i64>,
                SimpleScore,
                ScoringError,
            >| {
                for variable in 0..4 {
                    context.execute(&Move::Change {
                        variable,
                        value: Some(0),
                    })?;
                }
                Ok(())
            };
            d.run_solver_with_extensions(
                SolverSequence {
                    phases: vec![phase.clone()],
                    ..Default::default()
                },
                SolverExtensions {
                    command: Some(&mut command),
                    ..Default::default()
                },
            )
            .map(Work)
            .map_err(|e| io::Error::other(e.to_string()))
        });
        match mode {
            NoAssert | NonReproducible => {
                assert!(result.is_ok());
            }
            PhaseAssert => {
                let e = result.err().unwrap();
                let text = match &e.primary {
                    FactoryExecutionError::Build(error) => error.to_string(),
                    FactoryExecutionError::Context(error) => error.to_string(),
                };
                assert!(matches!(
                    e.primary,
                    FactoryExecutionError::Context(ScoringContextError::Scoring(
                        FactoryAssemblyError::PhaseAssertion { phase: 0, .. }
                    ))
                ));
                assert!(text.contains("score corruption"));
            }
            _ => {
                let e = result.err().unwrap();
                let text = match &e.primary {
                    FactoryExecutionError::Build(error) => error.to_string(),
                    FactoryExecutionError::Context(error) => error.to_string(),
                };
                let FactoryExecutionError::Context(ScoringContextError::Scoring(
                    FactoryAssemblyError::ScoreAssertion {
                        phase,
                        mode: actual,
                        error,
                    },
                )) = e.primary
                else {
                    panic!("wrong diagnostic: {text}");
                };
                assert_eq!(phase, 0);
                assert_eq!(actual, mode);
                assert!(error.to_string().contains("working score"));
                assert!(error.to_string().contains("recomputed score"));
                // A named native constraint provides real detailed evidence, in
                // place of Java STEP_ASSERT's unavailable-analysis fallback.
                assert!(error.to_string().contains("corrupted different values"));
            }
        }
    }
}
