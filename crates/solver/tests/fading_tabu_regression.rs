// Original regressions; no upstream-method credit.
use timetuck::*;
const ZERO: SimpleScore = SimpleScore::ZERO;
#[test]
fn chronological_reference_covers_refresh_duplicates_empty_steps_and_multiple_keys() {
    for hard in 0..5 {
        for fading in 0..6 {
            if hard + fading == 0 {
                continue;
            }
            let mut m = FadingTabuMemory::new(hard, fading, false, 77).unwrap();
            let mut last = [None; 9];
            for step in 0..180 {
                for a in 0..9 {
                    for b in 0..9 {
                        let age = [a, b]
                            .iter()
                            .filter_map(|&k| last[k].map(|s| step - s))
                            .min();
                        let expected = match age {
                            None => 1.0,
                            Some(age) if age <= hard => 0.0,
                            Some(age) if age >= hard + fading => 1.0,
                            Some(age) => (age - hard + 1) as f64 / (fading + 1) as f64,
                        };
                        assert_eq!(m.probability(&[a, b], ZERO, ZERO), expected);
                    }
                }
                let keys = if step % 7 == 0 {
                    vec![]
                } else {
                    vec![step % 9, (step * 5) % 9, step % 9]
                };
                m.step_ended(&keys);
                for k in keys {
                    last[k] = Some(step);
                }
            }
        }
    }
}
#[test]
fn zero_fading_matches_hard_memory_and_extreme_tenures_do_not_overflow() {
    for tenure in [1, 2, 7, usize::MAX] {
        let mut hard = TabuMemory::new(tenure, true).unwrap();
        let mut fade = FadingTabuMemory::new(tenure, 0, true, 5).unwrap();
        for step in 0..100 {
            for key in 0..7 {
                for score in [-1, 0, 1] {
                    let c = SimpleScore::new(score);
                    assert_eq!(
                        fade.accepts_with_draw(&[key], c, ZERO, || panic!("no fading"))
                            .unwrap(),
                        hard.accepts(&[key], c, ZERO)
                    );
                }
            }
            let keys = [step % 7];
            hard.step_ended(&keys);
            fade.step_ended(&keys);
        }
    }
    assert!(matches!(
        FadingTabuMemory::<usize>::new(usize::MAX, 1, false, 0),
        Err(AcceptanceError::TabuTenureOverflow)
    ));
    assert!(matches!(
        FadingTabuMemory::<usize>::new(0, 0, false, 0),
        Err(AcceptanceError::EmptyTabu)
    ));
    let mut huge = FadingTabuMemory::new(0, usize::MAX, false, 0).unwrap();
    huge.step_ended(&[1]);
    assert!(huge.probability(&[1], ZERO, ZERO) > 0.0);
}
#[test]
fn lazy_draw_boundaries_aspiration_and_invalid_draws_preserve_memory() {
    let mut m = FadingTabuMemory::new(2, 4, true, 123).unwrap();
    m.step_ended(&[Some(1), None]);
    assert!(
        m.accepts_with_draw(&[None], SimpleScore::new(1), ZERO, || panic!("aspiration"))
            .unwrap()
    );
    assert!(
        !m.accepts_with_draw(&[None], ZERO, ZERO, || panic!("hard"))
            .unwrap()
    );
    m.step_ended(&[]);
    m.step_ended(&[]);
    assert_eq!(m.probability(&[None], ZERO, ZERO), 0.4);
    assert!(m.accepts_with_draw(&[None], ZERO, ZERO, || 0.399).unwrap());
    assert!(!m.accepts_with_draw(&[None], ZERO, ZERO, || 0.4).unwrap());
    for draw in [-1.0, 1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            m.accepts_with_draw(&[None], ZERO, ZERO, || draw),
            Err(AcceptanceError::InvalidRandomDraw)
        );
        assert_eq!(m.probability(&[None], ZERO, ZERO), 0.4);
    }
    m.step_ended(&[Some(2)]);
    assert_eq!(m.probability(&[None, Some(2)], ZERO, ZERO), 0.0);
    assert_eq!(m.probability(&[None], ZERO, ZERO), 0.6);
}
#[test]
fn seeds_replay_and_certain_trials_do_not_advance_randomness() {
    let mut a = FadingTabuMemory::new(2, 4, true, 123).unwrap();
    a.step_ended(&[1]);
    let mut b = a.clone();
    for _ in 0..50 {
        assert!(!a.accepts(&[1], ZERO, ZERO));
        assert!(a.accepts(&[9], ZERO, ZERO));
        assert!(a.accepts(&[1], SimpleScore::new(1), ZERO));
    }
    for _ in 0..2 {
        a.step_ended(&[]);
        b.step_ended(&[]);
    }
    let draws: Vec<_> = (0..100)
        .map(|_| {
            let x = a.accepts(&[1], ZERO, ZERO);
            assert_eq!(x, b.accepts(&[1], ZERO, ZERO));
            x
        })
        .collect();
    assert!(draws.contains(&true) && draws.contains(&false));
}
#[test]
fn acceptor_tracks_global_best_and_fresh_sessions() {
    let config = Acceptance::FadingVariableTabu {
        hard_tenure: 2,
        fading_tenure: 4,
        aspiration: true,
        seed: 7,
    };
    let mut a = config.start(SimpleScore::new(10)).unwrap();
    a.step_ended(SimpleScore::new(5), &[0]);
    assert!(!a.accepts(SimpleScore::new(5), SimpleScore::new(10), &[0]));
    assert!(a.accepts(SimpleScore::new(5), SimpleScore::new(11), &[0]));
    a.step_ended(SimpleScore::new(11), &[0]);
    assert!(!a.accepts(SimpleScore::new(5), SimpleScore::new(11), &[0]));
    assert!(config.start(ZERO).unwrap().accepts(ZERO, ZERO, &[0]));
}
#[test]
fn hard_interval_changes_actual_search_and_invalid_config_precedes_scoring() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let calls = std::cell::Cell::new(0);
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        calls.set(calls.get() + 1);
        SimpleScore::new(*v[0].unwrap())
    })
    .unwrap();
    let config = LocalSearchConfig {
        acceptance: Acceptance::FadingVariableTabu {
            hard_tenure: 2,
            fading_tenure: 4,
            aspiration: false,
            seed: 11,
        },
        max_steps: 10,
        max_evaluations: 50,
        ..Default::default()
    };
    let result = local_search(&p, &n, config).unwrap();
    assert_eq!(result.completed_steps, 1);
    assert_eq!(result.termination, LocalSearchTermination::NoAcceptedMove);
    assert_eq!(result.best.unwrap().score, SimpleScore::new(1));
    calls.set(0);
    let invalid = LocalSearchConfig {
        acceptance: Acceptance::FadingVariableTabu {
            hard_tenure: usize::MAX,
            fading_tenure: 1,
            aspiration: false,
            seed: 0,
        },
        ..config
    };
    assert!(matches!(
        local_search(&p, &n, invalid),
        Err(LocalSearchError::Acceptance(
            AcceptanceError::TabuTenureOverflow
        ))
    ));
    assert_eq!(calls.get(), 0);
}
#[test]
fn list_and_mixed_search_forbid_selected_entities_and_keep_key_namespaces_separate() {
    let lists = ListProblem::new(
        vec![7],
        vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
    )
    .unwrap();
    let config = LocalSearchConfig {
        acceptance: Acceptance::FadingVariableTabu {
            hard_tenure: 10,
            fading_tenure: 4,
            aspiration: false,
            seed: 31,
        },
        max_steps: 8,
        max_evaluations: 100,
        assert_scores: true,
        ..Default::default()
    };
    let network = ListConstraintNetwork::<i32, SimpleScore>::new(2, 1);
    let a = list_local_search_from(&lists, &network, lists.initial_assignment(), config).unwrap();
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let sources = ListStreamSources::new(&mut builder);
    let b = list_stream_local_search_from(
        &lists,
        builder.build(),
        sources,
        lists.initial_assignment(),
        config,
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
    let a = mixed_local_search_from(&problem, &network, problem.initial_assignment(), settings)
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
