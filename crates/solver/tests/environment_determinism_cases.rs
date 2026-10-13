// Adapted from pinned Community EnvironmentModeTest.determinism and its
// initializer/different-values TEST helpers. Apache-2.0; see provenance.
#[path = "fixtures/assembled_factory_fixture.rs"]
mod fixture;
use fixture::*;
use std::{collections::BTreeSet, io, num::NonZeroUsize, rc::Rc};
use timetuck::*;
fn domain() -> Domain {
    let model = Rc::new(Model::new(vec![Variable::new(vec![0, 1, 2]); 4]).unwrap());
    let mut network = ConstraintNetwork::new(4);
    network
        .add_constraint("different values", vec![0, 1, 2, 3], |rows| {
            let assigned = rows.iter().flatten().count();
            let unique = rows
                .iter()
                .flatten()
                .copied()
                .collect::<BTreeSet<_>>()
                .len();
            SimpleScore::new(-((assigned - unique) as i64))
        })
        .unwrap();
    Domain {
        model,
        network,
        objective: Objective::Zero,
    }
}
fn factory(domain: &Domain, mode: EnvironmentMode) -> Factory<'_> {
    let mut config = configuration(domain, Some(mode), vec![None, None], 1000);
    config.phases = vec![
        SolverPhase::Custom(CustomPhaseConfig::default()),
        SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: u64::MAX,
            max_evaluations: u64::MAX,
            max_move_evaluations: Some(1000),
            move_order: MoveOrder::ShuffledMoves {
                seed: 0,
                window: NonZeroUsize::new(32).unwrap(),
            },
            selection: MoveSelection::FirstAccepted,
            acceptance: Acceptance::LateAcceptance {
                history_size: 32,
                hill_climbing: true,
            },
            ..Default::default()
        }),
    ];
    declaration(domain, false, Rc::new(Observations::default()))
        .assemble_solver_factory(
            config,
            Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
        )
        .unwrap()
}
fn solve(factory: &Factory<'_>, domain: &Domain, scores: &mut Vec<SimpleScore>) {
    factory
        .solve_assembled_with(domain.model.initial_assignment(), |resource, phase| {
            let Engine::Dense(director) = &mut resource.engine else {
                panic!()
            };
            let metrics = SolverMetrics::default();
            let mut lifecycle = |event| {
                if matches!(event, SolverLifecycleEvent::StepEnded { .. }) {
                    scores.push(
                        metrics.snapshot().active[0]
                            .last_step
                            .as_ref()
                            .unwrap()
                            .score,
                    );
                }
            };
            let mut initialize = |_,
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
            let result = director
                .run_solver_with_extensions(
                    SolverSequence {
                        phases: vec![phase.clone()],
                        ..Default::default()
                    },
                    SolverExtensions {
                        metrics: Some(&metrics),
                        lifecycle: Some(&mut lifecycle),
                        command: Some(&mut initialize),
                        ..Default::default()
                    },
                )
                .map_err(|e| io::Error::other(e.to_string()))?;
            if matches!(phase, SolverPhase::LocalSearch(_)) {
                assert_eq!(result.work.move_evaluations, 1000);
            }
            Ok(Work(result))
        })
        .unwrap();
}
#[test]
fn determinism() {
    use EnvironmentMode::*;
    let domain = domain();
    for mode in [
        TrackedFullAssert,
        FullAssert,
        NonIntrusiveFullAssert,
        StepAssert,
        PhaseAssert,
        NoAssert,
        NonReproducible,
    ] {
        let first = factory(&domain, mode);
        let second = factory(&domain, mode);
        let mut a = first.open_assembled_random();
        let mut b = second.open_assembled_random();
        for _ in 0..1000 {
            if mode.is_reproducible() {
                assert_eq!(a.next_factory_word(), b.next_factory_word());
            } else {
                assert_ne!(a.next_factory_word(), b.next_factory_word());
            }
        }
        let mut left = vec![];
        let mut right = vec![];
        for _ in 0..10 {
            solve(&first, &domain, &mut left);
            solve(&second, &domain, &mut right);
            if mode.is_reproducible() {
                assert_eq!(left, right);
            } else {
                assert_ne!(left, right);
            }
        }
    }
}
