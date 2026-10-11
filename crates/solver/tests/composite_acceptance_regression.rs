// Original state/composition regressions; no upstream-method credit.
use timetuck::*;
const ZERO: SimpleScore = SimpleScore::ZERO;
#[test]
fn empty_single_large_and_nested_conjunctions_preserve_boolean_identity() {
    for size in [0, 1, 3, 257] {
        let policies = vec![Acceptance::HillClimbing; size];
        let mut a = Acceptor::from_policies(&policies, ZERO).unwrap();
        assert!(a.accepts(ZERO, ZERO, &[]));
        assert_eq!(a.accepts(ZERO, SimpleScore::new(-1), &[]), size == 0);
    }
    let mut a = Acceptor::All(vec![
        Acceptor::All(vec![Acceptor::HillClimbing]),
        Acceptor::StrictImprovement,
    ]);
    assert!(!a.accepts(ZERO, ZERO, &[]));
    assert!(a.accepts(ZERO, SimpleScore::new(1), &[]));
}
#[test]
fn short_circuit_skips_later_trials_but_completed_steps_reach_every_child() {
    let policies = [
        Acceptance::StrictImprovement,
        Acceptance::StepCountingHillClimbing {
            period: 1,
            mode: StepCountingMode::SelectedMove,
        },
        Acceptance::LateAcceptance {
            history_size: 2,
            hill_climbing: false,
        },
        Acceptance::VariableTabu {
            tenure: 2,
            aspiration: false,
        },
    ];
    let mut a = Acceptor::from_policies(&policies, ZERO).unwrap();
    assert!(!a.accepts(ZERO, ZERO, &[4]));
    let Acceptor::All(children) = &a else {
        unreachable!()
    };
    let Acceptor::StepCountingHillClimbing(counter) = &children[1] else {
        unreachable!()
    };
    assert_eq!(counter.count(), 0);
    // A configured rejected fallback still completes a step.
    a.step_ended(SimpleScore::new(-1), &[4]);
    let Acceptor::All(children) = &mut a else {
        unreachable!()
    };
    let Acceptor::LateAcceptance(history) = &children[2] else {
        unreachable!()
    };
    assert_eq!(history.history(), &[SimpleScore::new(-1), ZERO]);
    assert!(!children[3].accepts(SimpleScore::new(-1), ZERO, &[4]));
    assert!(children[3].accepts(SimpleScore::new(-1), ZERO, &[5]));
    assert!(!a.accepts(SimpleScore::new(-1), ZERO, &[4]));
    let Acceptor::All(children) = &a else {
        unreachable!()
    };
    let Acceptor::StepCountingHillClimbing(counter) = &children[1] else {
        unreachable!()
    };
    assert_eq!(counter.count(), 1);
    a.step_ended(SimpleScore::new(-2), &[]);
    let Acceptor::All(children) = &a else {
        unreachable!()
    };
    let Acceptor::StepCountingHillClimbing(counter) = &children[1] else {
        unreachable!()
    };
    assert_eq!(counter.count(), 0);
    assert_eq!(counter.threshold(), SimpleScore::new(-2));
}
#[test]
fn all_policies_validate_before_constructing_history_and_sessions_are_fresh() {
    let policies = [
        Acceptance::LateAcceptance {
            history_size: usize::MAX,
            hill_climbing: false,
        },
        Acceptance::VariableTabu {
            tenure: 0,
            aspiration: false,
        },
    ];
    assert!(matches!(
        Acceptor::from_policies(&policies, ZERO),
        Err(AcceptanceError::EmptyTabu)
    ));
    let policies = [
        Acceptance::HillClimbing,
        Acceptance::VariableTabu {
            tenure: 1,
            aspiration: false,
        },
    ];
    let mut a = Acceptor::from_policies(&policies, ZERO).unwrap();
    a.step_ended(ZERO, &[0]);
    assert!(!a.accepts(ZERO, ZERO, &[0]));
    assert!(
        Acceptor::from_policies(&policies, ZERO)
            .unwrap()
            .accepts(ZERO, ZERO, &[0])
    );
}
#[test]
fn skipped_random_policy_keeps_its_seed_and_child_aspiration_cannot_override_other_rejections() {
    let policies = [
        Acceptance::StrictImprovement,
        Acceptance::SimulatedAnnealing {
            starting_temperature: SimpleScore::new(10),
            cooling_steps: 10,
            seed: 27,
        },
    ];
    let mut a = Acceptor::from_policies(&policies, ZERO).unwrap();
    let mut b = policies[1].start(ZERO).unwrap();
    for _ in 0..100 {
        assert!(!a.accepts(ZERO, SimpleScore::new(-1), &[]));
    }
    let Acceptor::All(children) = &mut a else {
        unreachable!()
    };
    for _ in 0..100 {
        assert_eq!(
            children[1].accepts(ZERO, SimpleScore::new(-1), &[]),
            b.accepts(ZERO, SimpleScore::new(-1), &[])
        );
    }
    let policies = [
        Acceptance::VariableTabu {
            tenure: 2,
            aspiration: true,
        },
        Acceptance::StrictImprovement,
    ];
    let mut a = Acceptor::from_policies(&policies, ZERO).unwrap();
    a.step_ended(SimpleScore::new(-2), &[0]);
    assert!(!a.accepts(SimpleScore::new(3), SimpleScore::new(2), &[0]));
}
#[test]
fn extra_tabu_tracks_basic_keys_and_bad_extra_config_precedes_scoring() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let calls = std::cell::Cell::new(0);
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        calls.set(calls.get() + 1);
        SimpleScore::new(*v[0].unwrap())
    })
    .unwrap();
    let extra = [Acceptance::VariableTabu {
        tenure: 2,
        aspiration: false,
    }];
    let c = LocalSearchConfig {
        acceptance: Acceptance::LateAcceptance {
            history_size: 3,
            hill_climbing: true,
        },
        additional_acceptance: &extra,
        max_evaluations: 50,
        max_steps: 8,
        ..Default::default()
    };
    let a = local_search(&p, &n, c.clone()).unwrap();
    assert_eq!(a.completed_steps, 1);
    assert_eq!(a.termination, LocalSearchTermination::NoAcceptedMove);
    let b = local_search(
        &p,
        &n,
        LocalSearchConfig {
            additional_acceptance: &[],
            ..c.clone()
        },
    )
    .unwrap();
    assert!(b.completed_steps > 1);
    let invalid = [Acceptance::VariableTabu {
        tenure: 0,
        aspiration: false,
    }];
    calls.set(0);
    let c = LocalSearchConfig {
        additional_acceptance: &invalid,
        max_evaluations: 0,
        ..c.clone()
    };
    assert!(matches!(
        local_search(&p, &n, c.clone()),
        Err(LocalSearchError::Acceptance(AcceptanceError::EmptyTabu))
    ));
    assert_eq!(calls.get(), 0);
    let plan = SolverConfig {
        local_search: vec![LocalSearchConfig::default(), c],
        ..Default::default()
    };
    assert!(matches!(
        solve(&p, &n, plan),
        Err(SolverError::InvalidAcceptance {
            phase: 2,
            error: AcceptanceError::EmptyTabu
        })
    ));
    assert_eq!(calls.get(), 0);
}

#[test]
fn additional_tabu_tracks_list_entities_and_mixed_key_namespaces() {
    let lists = ListProblem::new(
        vec![7],
        vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
    )
    .unwrap();
    let policies = [Acceptance::VariableTabu {
        tenure: 10,
        aspiration: false,
    }];
    let config = LocalSearchConfig {
        acceptance: Acceptance::HillClimbing,
        additional_acceptance: &policies,
        max_steps: 8,
        max_evaluations: 100,
        assert_scores: true,
        ..Default::default()
    };
    let network = ListConstraintNetwork::<i32, SimpleScore>::new(2, 1);
    let a = list_local_search_from(&lists, &network, lists.initial_assignment(), config.clone())
        .unwrap();
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let sources = ListStreamSources::new(&mut builder);
    let b = list_stream_local_search_from(
        &lists,
        builder.build(),
        sources,
        lists.initial_assignment(),
        config.clone(),
    )
    .unwrap();
    assert_eq!(a.completed_steps, 1);
    assert_eq!(a.termination, LocalSearchTermination::NoAcceptedMove);
    assert_eq!(b.completed_steps, a.completed_steps);
    assert_eq!(b.best, a.best);
    assert_eq!(b.termination, a.termination);
    let problem = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
        lists,
    );
    let settings = MixedLocalSearchConfig {
        search: config,
        joint_moves: false,
    };
    let network = MixedConstraintNetwork::<i32, i32, SimpleScore>::new(1, 2, 1);
    let a = mixed_local_search_from(
        &problem,
        &network,
        problem.initial_assignment(),
        settings.clone(),
    )
    .unwrap();
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let sources = MixedStreamSources::new(&mut builder);
    let b = mixed_stream_local_search_from(
        &problem,
        builder.build(),
        sources,
        problem.initial_assignment(),
        settings,
    )
    .unwrap();
    // Basic variable 0 must not also forbid list entity 0; the list relocation
    // then touches both owners, leaving no non-tabu candidate.
    assert_eq!(a.completed_steps, 2);
    assert_eq!(a.termination, LocalSearchTermination::NoAcceptedMove);
    assert_eq!(b.completed_steps, a.completed_steps);
    assert_eq!(b.best, a.best);
    assert_eq!(b.termination, a.termination);
}
