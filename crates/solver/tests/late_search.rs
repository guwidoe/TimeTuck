// Original late-acceptance and reproducible-search regressions.
use std::{cell::RefCell, cmp::Ordering};
use timetuck::{
    Acceptance, AcceptanceError, ConstraintNetwork, FiniteProblem, HardMediumSoftScore,
    HardSoftScore, LateAcceptance, LocalSearchConfig, LocalSearchError, LocalSearchTermination,
    MoveOrder, Neighborhood, Score, SimpleScore, Variable, local_search,
};

fn landscape(values: &[Option<&i32>]) -> SimpleScore {
    SimpleScore::new(match *values {
        [Some(&0), Some(&0)] => 0,
        [Some(&1), Some(&0)] => 2,
        [Some(&1), Some(&1)] => 1,
        [Some(&0), Some(&1)] => 3,
        _ => panic!("unexpected assignment"),
    })
}
fn config() -> LocalSearchConfig<'static> {
    LocalSearchConfig {
        max_evaluations: 100,
        max_accepted_moves: 20,
        neighborhood: Neighborhood::Change,
        move_order: MoveOrder::ShuffledVariables { seed: 0 },
        assert_scores: true,
        ..LocalSearchConfig::default()
    }
}

#[test]
fn seeded_late_acceptance_crosses_a_valley_that_stops_hill_climbing() {
    let model = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let mut network = ConstraintNetwork::new(2);
    network
        .add_constraint("landscape", vec![0, 1], landscape)
        .unwrap();
    let strict = local_search(&model, &network, config()).unwrap();
    assert_eq!(strict.best.unwrap().score, SimpleScore::new(2));
    let cfg = LocalSearchConfig {
        acceptance: Acceptance::LateAcceptance {
            history_size: 3,
            hill_climbing: true,
        },
        ..config()
    };
    let late = local_search(&model, &network, cfg).unwrap();
    assert_eq!(late.best.as_ref().unwrap().score, SimpleScore::new(3));
    assert_eq!(
        late.best.as_ref().unwrap().assignment.values(),
        &[Some(0), Some(1)]
    );
    assert_eq!(late.accepted_moves, 3);
    assert_eq!(late.termination, LocalSearchTermination::NoAcceptedMove);
    assert_eq!(late, local_search(&model, &network, cfg).unwrap());
}
#[test]
fn returned_best_survives_a_worsening_final_step() {
    let observed = RefCell::new(Vec::new());
    let model = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let mut network = ConstraintNetwork::new(2);
    network
        .add_constraint("landscape", vec![0, 1], |v| {
            observed.borrow_mut().push(landscape(v));
            landscape(v)
        })
        .unwrap();
    let result = local_search(
        &model,
        &network,
        LocalSearchConfig {
            max_accepted_moves: 4,
            acceptance: Acceptance::LateAcceptance {
                history_size: 5,
                hill_climbing: true,
            },
            assert_scores: false,
            ..config()
        },
    )
    .unwrap();
    assert_eq!(
        result.termination,
        LocalSearchTermination::AcceptedMoveLimit
    );
    assert_eq!(result.best.unwrap().score, SimpleScore::new(3));
    assert_eq!(*observed.borrow().last().unwrap(), SimpleScore::ZERO);
}
#[test]
fn trials_do_not_advance_history_and_size_one_uses_completed_score() {
    let mut state = LateAcceptance::new(1, false, SimpleScore::new(0)).unwrap();
    for _ in 0..100 {
        assert!(state.accepts(SimpleScore::new(8), SimpleScore::new(1)));
        assert!(!state.accepts(SimpleScore::new(8), SimpleScore::new(-1)));
    }
    assert_eq!(state.threshold(), SimpleScore::ZERO);
    state.step_ended(SimpleScore::new(8));
    assert_eq!(state.threshold(), SimpleScore::new(8));
    assert!(!state.accepts(SimpleScore::new(0), SimpleScore::new(7)));
}
#[test]
fn medium_resets_history_but_is_not_a_feasibility_level() {
    let initial = HardMediumSoftScore::new(-1, 0, 0);
    let mut state = LateAcceptance::new(3, false, initial).unwrap();
    state.step_ended(HardMediumSoftScore::new(-1, 2, 0));
    assert_eq!(state.history(), &[HardMediumSoftScore::new(-1, 2, 0); 3]);
    let hard = HardMediumSoftScore::new(0, -100, -100);
    state.step_ended(hard);
    assert_eq!(state.history(), &[hard; 3]);
    assert_eq!(
        SimpleScore::new(20).hard_cmp(&SimpleScore::new(-20)),
        Ordering::Equal
    );
    assert_eq!(
        HardSoftScore::new(0, 99)
            .with_structural(-1)
            .hard_cmp(&HardSoftScore::new(0, -99)),
        Ordering::Equal
    );
}
#[test]
fn invalid_history_is_rejected_before_scoring_even_with_zero_budget() {
    let model = FiniteProblem::<i32>::new(vec![]).unwrap();
    let mut network = ConstraintNetwork::new(0);
    network
        .add_constraint("must not run", vec![], |_| -> SimpleScore { panic!() })
        .unwrap();
    let result = local_search(
        &model,
        &network,
        LocalSearchConfig {
            acceptance: Acceptance::LateAcceptance {
                history_size: 0,
                hill_climbing: false,
            },
            max_evaluations: 0,
            ..config()
        },
    );
    assert!(matches!(
        result,
        Err(LocalSearchError::Acceptance(AcceptanceError::EmptyHistory))
    ));
    assert!(matches!(
        LateAcceptance::new(usize::MAX, false, SimpleScore::ZERO),
        Err(AcceptanceError::HistoryAllocation)
    ));
}
#[test]
fn seeded_runs_respect_limits_and_best_quality_across_many_orders() {
    let model = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let mut network = ConstraintNetwork::new(2);
    network
        .add_constraint("landscape", vec![0, 1], landscape)
        .unwrap();
    for seed in 0..64 {
        let cfg = LocalSearchConfig {
            max_evaluations: 17,
            max_accepted_moves: 11,
            move_order: MoveOrder::ShuffledVariables { seed },
            acceptance: Acceptance::LateAcceptance {
                history_size: 4,
                hill_climbing: false,
            },
            ..config()
        };
        let result = local_search(&model, &network, cfg).unwrap();
        assert!(result.evaluations <= 17);
        assert!(result.accepted_moves <= 11);
        let best = result.best.as_ref().unwrap();
        assert!(model.is_complete(&best.assignment));
        assert_eq!(
            best.score,
            network.calculate(&model, &best.assignment).unwrap()
        );
        assert!(best.score >= SimpleScore::ZERO && best.score <= SimpleScore::new(3));
        assert_eq!(result, local_search(&model, &network, cfg).unwrap());
    }
}
