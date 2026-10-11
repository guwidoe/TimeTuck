// Original validation, rounding and all-backend population-context checks.
use SimpleScore as S;
use timetuck::*;
#[test]
fn boundaries_signed_zero_and_invalid_ratios() {
    for ratio in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01, 1.01] {
        assert_eq!(
            TabuSize::ratio(ratio),
            Err(AcceptanceError::InvalidTabuRatio)
        );
    }
    assert_eq!(TabuSize::ratio(-0.0), TabuSize::ratio(0.0));
    for ratio in [0.0, f64::MIN_POSITIVE, 0.5, 1.0] {
        for population in [0, 1, 2] {
            assert_eq!(TabuSize::ratio(ratio).unwrap().resolve(population), 1);
        }
    }
    assert_eq!(
        TabuSize::ratio(1.0).unwrap().resolve(usize::MAX),
        usize::MAX - 1
    );
    assert_eq!(TabuSize::ratio(0.0).unwrap().resolve(usize::MAX), 1);
    for n in [0, 1, 10, usize::MAX] {
        assert_eq!(TabuSize::fixed(17).unwrap().resolve(n), 17);
    }
}
#[test]
fn binary_exact_ratios_match_integer_rounding_and_population_caps() {
    for denominator in [2usize, 4, 8, 16, 64, 1024] {
        for numerator in 0..=denominator {
            let size = TabuSize::ratio(numerator as f64 / denominator as f64).unwrap();
            for n in [0usize, 1, 2, 3, 7, 100, 999, 10001] {
                let expected = ((n * numerator * 2 + denominator) / (denominator * 2))
                    .max(1)
                    .min(n.saturating_sub(1).max(1));
                assert_eq!(size.resolve(n), expected, "{n} {numerator}/{denominator}");
            }
        }
    }
}
#[test]
fn standalone_context_is_explicit_and_fixed_memory_retains_step_semantics() {
    let p = Acceptance::<S>::SizedVariableTabu {
        size: TabuSize::ratio(0.5).unwrap(),
        aspiration: false,
    };
    assert!(matches!(
        p.start(S::ZERO),
        Err(AcceptanceError::PopulationContextRequired)
    ));
    assert!(matches!(
        p.start_with_moves::<i32>(S::ZERO),
        Err(AcceptanceError::PopulationContextRequired)
    ));
    assert!(matches!(
        p.start_with_values::<i32, i32>(S::ZERO),
        Err(AcceptanceError::PopulationContextRequired)
    ));
    let mut a = p.for_population(6).start(S::ZERO).unwrap();
    a.step_ended(S::ZERO, &[1, 2]);
    for _ in 0..2 {
        assert!(!a.accepts(S::ZERO, S::new(1), &[1]));
        a.step_ended(S::ZERO, &[]);
    }
    assert!(!a.accepts(S::ZERO, S::ZERO, &[2]));
    a.step_ended(S::ZERO, &[]);
    assert!(a.accepts(S::ZERO, S::ZERO, &[1, 2]));
    let mut b = p.for_population(2).start(S::ZERO).unwrap();
    b.step_ended(S::ZERO, &[1]);
    b.step_ended(S::ZERO, &[]);
    assert!(b.accepts(S::ZERO, S::ZERO, &[1]));
    assert_eq!(
        Acceptance::<S>::HillClimbing.for_population(99),
        Acceptance::HillClimbing
    );
}

fn basic() -> FiniteProblem<i32> {
    FiniteProblem::new(
        (0..5)
            .map(|i| {
                Variable::new(vec![0, 1, 2])
                    .with_initial(0)
                    .with_pinned(i < 2)
            })
            .collect(),
    )
    .unwrap()
}
fn lists() -> ListProblem<i32> {
    ListProblem::new(
        vec![0, 1, 2],
        vec![
            ListEntity::new(vec![]).with_pinned(true),
            ListEntity::new(vec![]).with_pinned(true),
            ListEntity::new(vec![0]),
            ListEntity::new(vec![1]),
            ListEntity::new(vec![2]),
        ],
    )
    .unwrap()
}
fn run(policies: [Acceptance<S>; 3], extra: bool) -> Vec<(u64, u64, u64, LocalSearchTermination)> {
    let p = basic();
    let l = lists();
    let m = MixedProblem::new(p.clone(), l.clone());
    let mut results = vec![];
    macro_rules! save {
        ($r:expr) => {{
            let r = $r.unwrap();
            assert_eq!(r.best.unwrap().score, S::ZERO);
            results.push((
                r.completed_steps,
                r.accepted_moves,
                r.evaluations,
                r.termination,
            ));
        }};
    }
    for (kind, policy) in policies.into_iter().enumerate() {
        let extras = [policy];
        let c = LocalSearchConfig {
            acceptance: if extra {
                Acceptance::HillClimbing
            } else {
                policy
            },
            additional_acceptance: if extra { &extras } else { &[] },
            max_steps: 20,
            max_evaluations: 10000,
            assert_scores: true,
            ..Default::default()
        };
        match kind {
            0 => {
                let n = ConstraintNetwork::<i32, S>::new(5);
                save!(local_search(&p, &n, c.clone()));
                let mut b = StreamGraphBuilder::<S>::new();
                let s = b.source::<usize, (usize, Option<i32>)>();
                save!(stream_local_search(&p, b.build(), s, c));
            }
            1 => {
                let n = ListConstraintNetwork::<i32, S>::new(5, 3);
                save!(list_local_search_from(
                    &l,
                    &n,
                    l.initial_assignment(),
                    c.clone()
                ));
                let mut b = StreamGraphBuilder::<S>::new();
                let s = ListStreamSources::new(&mut b);
                save!(list_stream_local_search_from(
                    &l,
                    b.build(),
                    s,
                    l.initial_assignment(),
                    c
                ));
            }
            _ => {
                let n = MixedConstraintNetwork::<i32, i32, S>::new(5, 5, 3);
                let c = MixedLocalSearchConfig {
                    search: c,
                    joint_moves: true,
                };
                save!(mixed_local_search_from(
                    &m,
                    &n,
                    m.initial_assignment(),
                    c.clone()
                ));
                let mut b = StreamGraphBuilder::<S>::new();
                let s = MixedStreamSources::new(&mut b);
                save!(mixed_stream_local_search_from(
                    &m,
                    b.build(),
                    s,
                    m.initial_assignment(),
                    c
                ));
            }
        }
    }
    results
}
#[test]
fn all_six_backends_resolve_primary_and_additional_policies_with_pins() {
    for (ratio, tenures) in [(0.1, [1, 1, 1]), (0.5, [3, 3, 5]), (0.9, [4, 4, 9])] {
        let sized = Acceptance::SizedVariableTabu {
            size: TabuSize::ratio(ratio).unwrap(),
            aspiration: false,
        };
        let fixed = tenures.map(|tenure| Acceptance::VariableTabu {
            tenure,
            aspiration: false,
        });
        for extra in [false, true] {
            let actual = run([sized; 3], extra);
            assert_eq!(actual, run(fixed, extra));
            for pair in actual.chunks_exact(2) {
                assert_eq!(pair[0], pair[1]);
            }
            if ratio == 0.9 {
                assert_eq!(actual[0].0, 3); // Five variables, including two pinned.
            }
        }
    }
    let sized = Acceptance::SizedVariableTabu {
        size: TabuSize::fixed(2).unwrap(),
        aspiration: true,
    };
    assert_eq!(
        run([sized; 3], false),
        run(
            [Acceptance::VariableTabu {
                tenure: 2,
                aspiration: true
            }; 3],
            false
        )
    );
}
#[test]
fn empty_models_and_new_population_each_search_do_not_leak_size() {
    let p = FiniteProblem::<i32>::new(vec![]).unwrap();
    let n = ConstraintNetwork::<i32, S>::new(0);
    let c = LocalSearchConfig {
        acceptance: Acceptance::SizedVariableTabu {
            size: TabuSize::ratio(1.0).unwrap(),
            aspiration: false,
        },
        ..Default::default()
    };
    assert_eq!(local_search(&p, &n, c.clone()).unwrap().completed_steps, 0);
    let mut n = ConstraintNetwork::<i32, S>::new(5);
    n.add_constraint("zero", vec![], |_| S::ZERO).unwrap();
    assert_eq!(
        local_search(&basic(), &n, c.clone())
            .unwrap()
            .completed_steps,
        3
    );
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<i32, S>::new(1);
    assert_eq!(local_search(&p, &n, c).unwrap().completed_steps, 1);
}
