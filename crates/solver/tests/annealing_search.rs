// Original regression tests. These do not add upstream coverage credit.
use std::cell::Cell;
use timetuck::{
    Acceptance, AcceptanceError, ConstraintNetwork, FiniteProblem, HardSoftScore,
    LocalSearchConfig, LocalSearchError, LocalSearchTermination, MoveOrder, Neighborhood,
    SimpleScore, SimulatedAnnealing, Variable, exhaustive_search, local_search,
};

fn simple(temperature: i64) -> SimulatedAnnealing<SimpleScore> {
    SimulatedAnnealing::new(SimpleScore::new(temperature), 10, 7).unwrap()
}

#[test]
fn exponential_threshold_and_cooling_boundaries() {
    let state = simple(20);
    let current = SimpleScore::new(-100);
    let candidate = SimpleScore::new(-110);
    for (gradient, expected) in [(0.0, (-0.5_f64).exp()), (0.5, (-1.0_f64).exp()), (1.0, 0.0)] {
        assert_eq!(
            state.probability_at(current, candidate, gradient),
            Ok(expected)
        );
        assert_eq!(
            state.accepts_at(current, candidate, gradient, expected),
            Ok(false)
        );
        if expected > 0.0 {
            assert_eq!(
                state.accepts_at(current, candidate, gradient, expected - 1e-10),
                Ok(true)
            );
        }
    }
    for gradient in [0.0, 0.5, 1.0] {
        assert_eq!(state.probability_at(current, current, gradient), Ok(1.0));
        assert_eq!(
            state.probability_at(current, SimpleScore::new(0), gradient),
            Ok(1.0)
        );
    }
}

#[test]
fn native_multilevel_energy_preserves_lexicographic_improvements() {
    let state = SimulatedAnnealing::new(HardSoftScore::new(2, 10), 10, 0).unwrap();
    let current = HardSoftScore::new(0, 0);
    assert_eq!(
        state.probability_at(current, HardSoftScore::new(-2, -20), 0.0),
        Ok((-3.0_f64).exp())
    );
    assert_eq!(
        state.probability_at(current, HardSoftScore::new(-2, 1000), 0.0),
        Ok((-1.0_f64).exp())
    );
    assert_eq!(
        state.probability_at(current, HardSoftScore::new(1, i64::MIN), 1.0),
        Ok(1.0)
    );
    let frozen_hard = SimulatedAnnealing::new(HardSoftScore::new(0, 10), 10, 0).unwrap();
    assert_eq!(
        frozen_hard.probability_at(current, HardSoftScore::new(-1, 1000), 0.0),
        Ok(0.0)
    );
    assert_eq!(
        frozen_hard.probability_at(current, HardSoftScore::new(0, -10), 0.0),
        Ok((-1.0_f64).exp())
    );
}

#[test]
fn integer_deltas_do_not_wrap_or_disappear_at_float_precision_limit() {
    let state = simple(1);
    for current in [i64::MAX, i64::MIN + 1, 1_i64 << 54] {
        assert_eq!(
            state.probability_at(
                SimpleScore::new(current),
                SimpleScore::new(current - 1),
                0.0
            ),
            Ok((-1.0_f64).exp())
        );
    }
    assert_eq!(
        state.probability_at(SimpleScore::new(i64::MAX), SimpleScore::new(i64::MIN), 0.0),
        Ok(0.0)
    );
    assert_eq!(
        state.probability_at(SimpleScore::new(i64::MIN), SimpleScore::new(i64::MAX), 0.0),
        Ok(1.0)
    );
}

#[test]
fn invalid_inputs_and_structural_states_cannot_create_probability() {
    let state = simple(1);
    for gradient in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        assert_eq!(
            state.probability_at(SimpleScore::ZERO, SimpleScore::ZERO, gradient),
            Err(AcceptanceError::InvalidGradient)
        );
    }
    for draw in [f64::NAN, f64::INFINITY, -0.1, 1.0] {
        assert_eq!(
            state.accepts_at(SimpleScore::ZERO, SimpleScore::ZERO, 0.0, draw),
            Err(AcceptanceError::InvalidRandomDraw)
        );
    }
    for structural in [-1, 1] {
        let flawed = SimpleScore::ZERO.with_structural(structural);
        assert!(matches!(
            SimulatedAnnealing::new(flawed, 1, 0),
            Err(AcceptanceError::InvalidTemperature)
        ));
        assert_eq!(
            state.probability_at(SimpleScore::ZERO, flawed, 0.0),
            Ok(0.0)
        );
        assert_eq!(
            state.probability_at(flawed, SimpleScore::ZERO, 0.0),
            Ok(0.0)
        );
    }
    assert!(matches!(
        SimulatedAnnealing::new(SimpleScore::new(1), 0, 0),
        Err(AcceptanceError::ZeroCoolingSteps)
    ));
}

#[test]
fn trials_consume_only_probabilistic_draws_and_completed_steps_cool() {
    let mut state = simple(10);
    let mut control = state.clone();
    for _ in 0..200 {
        assert!(state.accepts(SimpleScore::ZERO, SimpleScore::ZERO));
        assert!(state.accepts(SimpleScore::ZERO, SimpleScore::new(1)));
        assert!(!state.accepts(SimpleScore::ZERO, SimpleScore::ZERO.with_structural(-1)));
        assert_eq!(
            state.accepts(SimpleScore::ZERO, SimpleScore::new(-5)),
            control.accepts(SimpleScore::ZERO, SimpleScore::new(-5))
        );
    }
    assert_eq!(state.gradient(), 0.0);
    for i in 1..=12 {
        state.step_ended();
        assert_eq!(state.gradient(), f64::from(i.min(10)) / 10.0);
    }
    for _ in 0..100 {
        assert!(!state.accepts(SimpleScore::ZERO, SimpleScore::new(-1)));
    }
    let mut frozen = simple(0);
    assert!(!frozen.accepts(SimpleScore::ZERO, SimpleScore::new(-1)));
    assert!(frozen.accepts(SimpleScore::ZERO, SimpleScore::ZERO));
}

#[test]
fn seeded_acceptance_frequency_matches_probability() {
    let mut state = simple(10);
    let mut accepted = 0;
    for _ in 0..100_000 {
        accepted += usize::from(state.accepts(SimpleScore::ZERO, SimpleScore::new(-10)));
    }
    // Wide deterministic distribution check; exp(-1) is about 0.368.
    assert!((35_500..38_000).contains(&accepted), "accepted {accepted}");
}

#[test]
fn seeded_search_crosses_a_valley_and_retains_an_independently_proven_best() {
    let model = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let mut network = ConstraintNetwork::new(2);
    network
        .add_constraint("valley", vec![0, 1], |v| {
            SimpleScore::new(match (*v[0].unwrap(), *v[1].unwrap()) {
                (0, 0) => 0,
                (1, 1) => 5,
                _ => -1,
            })
        })
        .unwrap();
    let hill = local_search(&model, &network, LocalSearchConfig::default()).unwrap();
    assert_eq!(hill.best.unwrap().score, SimpleScore::ZERO);
    let exact = exhaustive_search(
        &model,
        |a| {
            SimpleScore::new(match (a.values()[0], a.values()[1]) {
                (Some(0), Some(0)) => 0,
                (Some(1), Some(1)) => 5,
                _ => -1,
            })
        },
        100,
    );
    assert!(exact.is_optimal());
    let optimum = exact.best.unwrap().score;
    assert_eq!(optimum, SimpleScore::new(5));
    for seed in 0..32 {
        let config = LocalSearchConfig {
            acceptance: Acceptance::SimulatedAnnealing {
                starting_temperature: SimpleScore::new(10),
                cooling_steps: 100,
                seed,
            },
            move_order: MoveOrder::ShuffledVariables { seed: seed + 100 },
            neighborhood: Neighborhood::Change,
            max_evaluations: 201,
            max_steps: 100,
            assert_scores: true,
            ..LocalSearchConfig::default()
        };
        let a = local_search(&model, &network, config.clone()).unwrap();
        let b = local_search(&model, &network, config).unwrap();
        assert_eq!(a.best.as_ref().unwrap().score, optimum, "seed {seed}");
        assert_eq!(
            a.best.as_ref().unwrap().assignment,
            b.best.as_ref().unwrap().assignment
        );
        assert_eq!(a.evaluations, b.evaluations);
        assert_eq!(a.completed_steps, b.completed_steps);
        assert_eq!(a.termination, b.termination);
        assert!(a.evaluations <= 201 && a.completed_steps <= 100);
        assert_eq!(a.constraint_evaluations, u128::from(a.evaluations));
    }
}

#[test]
fn invalid_search_configuration_fails_before_scoring_even_at_zero_budget() {
    let calls = Cell::new(0);
    let model = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let mut network = ConstraintNetwork::new(1);
    network
        .add_constraint("count", vec![0], |_| {
            calls.set(calls.get() + 1);
            SimpleScore::ZERO
        })
        .unwrap();
    for (temperature, steps, error) in [
        (-1, 1, AcceptanceError::InvalidTemperature),
        (1, 0, AcceptanceError::ZeroCoolingSteps),
    ] {
        let result = local_search(
            &model,
            &network,
            LocalSearchConfig {
                acceptance: Acceptance::SimulatedAnnealing {
                    starting_temperature: SimpleScore::new(temperature),
                    cooling_steps: steps,
                    seed: 0,
                },
                max_evaluations: 0,
                ..LocalSearchConfig::default()
            },
        );
        assert!(matches!(result, Err(LocalSearchError::Acceptance(actual)) if actual == error));
    }
    assert_eq!(calls.get(), 0);
}

#[test]
fn selected_worsening_step_preserves_best_and_stops_at_step_limit() {
    let model = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let mut network = ConstraintNetwork::new(1);
    network
        .add_constraint("descending", vec![0], |v| {
            SimpleScore::new(-i64::from(*v[0].unwrap()))
        })
        .unwrap();
    let result = local_search(
        &model,
        &network,
        LocalSearchConfig {
            acceptance: Acceptance::SimulatedAnnealing {
                starting_temperature: SimpleScore::new(i64::MAX),
                cooling_steps: 1,
                seed: 7,
            },
            max_steps: 1,
            assert_scores: true,
            ..LocalSearchConfig::default()
        },
    )
    .unwrap();
    assert_eq!(result.termination, LocalSearchTermination::StepLimit);
    assert_eq!(result.accepted_moves, 1);
    assert_eq!(result.completed_steps, 1);
    assert_eq!(result.evaluations, 2);
    assert_eq!(result.best.unwrap().score, SimpleScore::ZERO);
}

#[test]
fn search_cooling_and_rejected_fallback_have_distinct_acceptance_counts() {
    let model = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let mut network = ConstraintNetwork::new(1);
    network
        .add_constraint("descending", vec![0], |v| {
            SimpleScore::new(-i64::from(*v[0].unwrap()))
        })
        .unwrap();
    let config = LocalSearchConfig {
        acceptance: Acceptance::SimulatedAnnealing {
            starting_temperature: SimpleScore::new(i64::MAX),
            cooling_steps: 1,
            seed: 7,
        },
        max_steps: 5,
        assert_scores: true,
        ..LocalSearchConfig::default()
    };
    let stopped = local_search(&model, &network, config.clone()).unwrap();
    assert_eq!(stopped.termination, LocalSearchTermination::NoAcceptedMove);
    assert_eq!(stopped.completed_steps, 2);
    assert_eq!(stopped.accepted_moves, 2);
    assert_eq!(stopped.evaluations, 4);
    let fallback = local_search(
        &model,
        &network,
        LocalSearchConfig {
            rejected_moves: timetuck::RejectedMovePolicy::BestScore,
            ..config
        },
    )
    .unwrap();
    assert_eq!(fallback.termination, LocalSearchTermination::StepLimit);
    assert_eq!(fallback.completed_steps, 5);
    assert_eq!(fallback.accepted_moves, 3);
    assert_eq!(fallback.evaluations, 6);
    assert_eq!(fallback.best.unwrap().score, SimpleScore::ZERO);
}
