//! Original owned-score integration checks; no upstream method credit.
#[path = "support/owned_score.rs"]
mod support;
use support::{OwnedScore, fail_clone_at};
use timetuck::*;

type RawScore = (i64, Vec<i64>);
type Phase = (usize, PhaseTermination, SolverWork, Option<RawScore>);
#[derive(Debug, PartialEq)]
struct Outcome {
    values: Vec<Option<usize>>,
    score: Option<(i64, Vec<i64>)>,
    complete: bool,
    work: SolverWork,
    phases: Vec<Phase>,
    reason: SolverTermination,
}
#[derive(Debug, PartialEq)]
enum Failure {
    Acceptance(usize),
    Token(usize),
    Phase(usize),
}
fn run<S: AdditiveScore + 'static>(
    mode: usize,
    initialized: bool,
    c: SolverConfig<'_, S>,
    make: impl Fn(i64) -> S + Copy,
    order: MixedConstructionOrder,
    raw: impl Fn(&S) -> RawScore + Copy,
) -> Result<Outcome, Failure> {
    let score = |v: &[Option<usize>]| make(v.iter().flatten().map(|&v| v as i64).sum());
    macro_rules! outcome {
        ($r:expr,$p:expr,$values:expr) => {{
            match $r {
                Ok(r) => {
                    $p.validate(&r.assignment).unwrap();
                    assert_eq!(r.complete, $p.is_complete(&r.assignment));
                    let values: Vec<Option<usize>> = $values(&r.assignment);
                    assert_eq!(r.solution().is_some(), r.complete && r.score.is_some());
                    if let Some(s) = &r.score {
                        assert_eq!(
                            *s,
                            make(values.iter().flatten().map(|&v| v as i64).sum()).with_structural(
                                -(values.iter().filter(|v| v.is_none()).count() as i64)
                            )
                        );
                    }
                    Ok(Outcome {
                        values,
                        score: r.score.as_ref().map(raw),
                        complete: r.complete,
                        work: r.work,
                        phases: r
                            .phases
                            .into_iter()
                            .map(|p| (p.index, p.termination, p.work, p.score.as_ref().map(raw)))
                            .collect(),
                        reason: r.termination,
                    })
                }
                Err(SolverError::Repair { .. }) => panic!("fixture has no repair phases"),
                Err(SolverError::PinnedUnassignedVariable { .. }) => {
                    panic!("fixture uses permissive pin policy")
                }
                Err(SolverError::EmptyPlan) => panic!("legacy plans include construction"),
                Err(SolverError::InitialScoring { .. }) => {
                    panic!("fixture models have movable entities")
                }
                Err(SolverError::InvalidMoveProviderPhase { .. }) => {
                    panic!("fixture has no provider bindings")
                }
                Err(SolverError::InvalidPhaseTermination { .. } | SolverError::DuplicatePhaseTermination { .. }) => {
                    panic!("fixture has no phase termination bindings")
                }
                Err(SolverError::InvalidScoreTrend { .. }) => panic!("fixture has no trend"),
                Err(
                    SolverError::PhaseScoreMismatch { .. }
                    | SolverError::PhaseScoreCheckFailed { .. },
                ) => {
                    panic!("fixture has no phase score checker")
                }
                Err(SolverError::InvalidOptimisticBound { .. }) => {
                    panic!("fixture has no bounded phases")
                }
                Err(SolverError::IncompleteLocalSearch { .. }) => {
                    panic!("fixture returns partials")
                }
                Err(SolverError::InvalidAcceptance { phase, .. }) => {
                    Err(Failure::Acceptance(phase))
                }
                Err(SolverError::ConflictingCancellation { phase }) => Err(Failure::Token(phase)),
                Err(SolverError::Phase { phase, .. }) => Err(Failure::Phase(phase)),
                Err(SolverError::Custom { .. } | SolverError::UnboundCustomPhase { .. }) => {
                    panic!("fixture has no custom phases")
                }
            }
        }};
    }
    let v = Variable::new(vec![0usize, 1, 2]);
    let basic = FiniteProblem::new(vec![if initialized { v.with_initial(0) } else { v }]).unwrap();
    if mode < 2 {
        let values = |a: &Assignment<usize>| a.values().to_vec();
        if mode == 0 {
            let mut n = ConstraintNetwork::new(1);
            n.add_constraint("score", vec![0], |v| score(&[v[0].copied()]))
                .unwrap();
            return outcome!(solve(&basic, &n, c), basic, values);
        }
        return outcome!(
            solve_stream(
                &basic,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let s = b.source::<usize, (usize, Option<usize>)>();
                    b.score("score", s.port(), |(_, v)| score(&[*v]))?;
                    Ok((b.build(), s))
                },
                c
            ),
            basic,
            values
        );
    }
    let lists = ListProblem::new(
        vec![7usize],
        vec![
            ListEntity::new(if initialized { vec![7] } else { vec![] }),
            ListEntity::new(vec![]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    if mode < 4 {
        let values = |a: &ListAssignment<usize>| vec![a.location(&7).map(|p| p.entity)];
        if mode == 2 {
            let mut n = ListConstraintNetwork::new(3, 1);
            n.add_constraint("score", vec![ListScope::Value(0)], |v| {
                let ListFact::Value { element, .. } = &v[0] else {
                    panic!()
                };
                score(&[element.as_ref().map(|v| v.position.entity)])
            })
            .unwrap();
            return outcome!(solve_list(&lists, &n, c), lists, values);
        }
        return outcome!(
            solve_list_stream(
                &lists,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let s = ListStreamSources::<usize>::new(&mut b);
                    b.score("score", s.values.port(), |v| {
                        score(&[v.position.map(|p| p.entity)])
                    })?;
                    Ok((b.build(), s))
                },
                c
            ),
            lists,
            values
        );
    }
    let p = MixedProblem::new(basic, lists);
    let values = |a: &MixedAssignment<usize, usize>| {
        vec![
            a.basic().values()[0],
            a.lists().location(&7).map(|p| p.entity),
        ]
    };
    let c = MixedSolverConfig {
        limits: c.limits,
        construction: MixedConstructionConfig {
            construction: c.construction,
            order,
        },
        local_search: c
            .local_search
            .into_iter()
            .map(|search| MixedLocalSearchConfig {
                search,
                joint_moves: true,
            })
            .collect(),
    };
    if mode == 4 {
        let mut n = MixedConstraintNetwork::new(1, 3, 1);
        n.add_constraint(
            "score",
            vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
            |v| {
                let (MixedFact::Basic(a), MixedFact::ListValue { element, .. }) = (&v[0], &v[1])
                else {
                    panic!()
                };
                score(&[a.copied(), element.as_ref().map(|v| v.position.entity)])
            },
        )
        .unwrap();
        return outcome!(solve_mixed(&p, &n, c), p, values);
    }
    outcome!(
        solve_mixed_stream(
            &p,
            || {
                let mut b = StreamGraphBuilder::new();
                let s = MixedStreamSources::<usize, usize>::new(&mut b);
                let j = b.join_equal(s.basic.port(), s.lists.values.port(), |_| (), |_| ())?;
                b.score("score", &j, |((_, a), v)| {
                    score(&[*a, v.position.map(|p| p.entity)])
                })?;
                Ok((b.build(), s))
            },
            c
        ),
        p,
        values
    )
}

fn raw<S: Score<Level = i64>>(score: &S) -> (i64, Vec<i64>) {
    (score.structural(), score.ordinary_levels().collect())
}
fn policies<S: AdditiveScore>(make: impl Fn(i64, i64) -> S) -> Vec<Acceptance<S>> {
    vec![
        Acceptance::DiversifiedLateAcceptance { history_size: 3 },
        Acceptance::SizedVariableTabu {
            size: TabuSize::ratio(0.5).unwrap(),
            aspiration: true,
        },
        Acceptance::StrictImprovement,
        Acceptance::HillClimbing,
        Acceptance::LateAcceptance {
            history_size: 3,
            hill_climbing: true,
        },
        Acceptance::VariableTabu {
            tenure: 2,
            aspiration: true,
        },
        Acceptance::ValueTabu {
            tenure: 2,
            aspiration: true,
        },
        Acceptance::FadingValueTabu {
            hard_tenure: 1,
            fading_tenure: 4,
            aspiration: true,
            seed: 77,
        },
        Acceptance::MoveTabu {
            tenure: 2,
            aspiration: true,
        },
        Acceptance::FadingMoveTabu {
            hard_tenure: 1,
            fading_tenure: 4,
            aspiration: true,
            seed: 77,
        },
        Acceptance::FadingVariableTabu {
            hard_tenure: 1,
            fading_tenure: 4,
            aspiration: true,
            seed: 77,
        },
        Acceptance::SimulatedAnnealing {
            starting_temperature: make(2, 3),
            cooling_steps: 6,
            seed: 77,
        },
    ]
}
#[test]
fn every_solver_backend_preserves_owned_scores_through_phases_and_budgets() {
    owned_policy_sequences(false);
}
#[test]
fn every_solver_backend_preserves_owned_scores_with_composite_policies() {
    owned_policy_sequences(true);
}
fn owned_policy_sequences(combined: bool) {
    let extra_owned = [
        Acceptance::LateAcceptance {
            history_size: 2,
            hill_climbing: true,
        },
        Acceptance::VariableTabu {
            tenure: 1,
            aspiration: true,
        },
    ];
    let extra_plain = [
        Acceptance::LateAcceptance {
            history_size: 2,
            hill_climbing: true,
        },
        Acceptance::VariableTabu {
            tenure: 1,
            aspiration: true,
        },
    ];
    for mode in 0..6 {
        for initialized in [false, true] {
            for order in [
                MixedConstructionOrder::BasicThenLists,
                MixedConstructionOrder::ListsThenBasic,
            ] {
                for budget in [0, 1, 2, 5, 12, 40] {
                    for (owned, plain) in policies(OwnedScore::new)
                        .into_iter()
                        .zip(policies(HardSoftScore::new))
                    {
                        let owned_plan = SolverConfig {
                            limits: SolverLimits {
                                max_evaluations: budget,
                                best_score_limit: Some(OwnedScore::new(0, 5)),
                                ..Default::default()
                            },
                            local_search: vec![
                                LocalSearchConfig {
                                    acceptance: owned,
                                    additional_acceptance: if combined {
                                        &extra_owned
                                    } else {
                                        &[]
                                    },
                                    max_steps: 4,
                                    assert_scores: true,
                                    ..Default::default()
                                };
                                2
                            ],
                            ..Default::default()
                        };
                        let plain_plan = SolverConfig {
                            limits: SolverLimits {
                                max_evaluations: budget,
                                best_score_limit: Some(HardSoftScore::new(0, 5)),
                                ..Default::default()
                            },
                            local_search: vec![
                                LocalSearchConfig {
                                    acceptance: plain,
                                    additional_acceptance: if combined {
                                        &extra_plain
                                    } else {
                                        &[]
                                    },
                                    max_steps: 4,
                                    assert_scores: true,
                                    ..Default::default()
                                };
                                2
                            ],
                            ..Default::default()
                        };
                        let expected = run(
                            mode,
                            initialized,
                            plain_plan,
                            |n| HardSoftScore::new(0, n),
                            order,
                            raw,
                        );
                        let actual = run(
                            mode,
                            initialized,
                            owned_plan,
                            |n| OwnedScore::new(0, n),
                            order,
                            raw,
                        );
                        assert_eq!(
                            actual, expected,
                            "mode {mode}, initialized {initialized}, budget {budget}"
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn owned_hard_history_annealing_and_progress_keep_independent_score_values() {
    let mut late = LateAcceptance::new(3, false, OwnedScore::new(-1, 9)).unwrap();
    late.step_ended(OwnedScore::new(0, -4));
    assert!(late.history().iter().all(|s| s == &OwnedScore::new(0, -4)));
    late.step_ended(OwnedScore::new(0, -3));
    assert_eq!(late.history()[1], OwnedScore::new(0, -3));
    assert_eq!(late.history()[0], OwnedScore::new(0, -4));
    let a = SimulatedAnnealing::new(OwnedScore::new(2, 3), 8, 44).unwrap();
    let b = SimulatedAnnealing::new(HardSoftScore::new(2, 3), 8, 44).unwrap();
    for h in -3..=3 {
        for s in -3..=3 {
            assert_eq!(
                a.probability_at(OwnedScore::new(0, 0), OwnedScore::new(h, s), 0.25)
                    .unwrap(),
                b.probability_at(HardSoftScore::ZERO, HardSoftScore::new(h, s), 0.25)
                    .unwrap()
            );
        }
    }
    let p = ScoreTargetProgress::new(OwnedScore::new(-4, -10), OwnedScore::new(0, 0), vec![0.7])
        .unwrap();
    let q = ScoreTargetProgress::new(HardSoftScore::new(-4, -10), HardSoftScore::ZERO, vec![0.7])
        .unwrap();
    let f = FeasibilityProgress::new(OwnedScore::new(-4, -10), vec![]).unwrap();
    for h in -5..=1 {
        for s in -12..=1 {
            assert_eq!(
                p.progress(OwnedScore::new(h, s)),
                q.progress(HardSoftScore::new(h, s))
            );
            assert_eq!(f.is_reached(OwnedScore::new(h, s)), h >= 0);
        }
    }
}
#[test]
fn owned_score_clone_failure_restores_graph_without_cloning_during_unwind() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let mut failures = 0;
    let mut completed = false;
    for at in 1..200 {
        let mut b = StreamGraphBuilder::<OwnedScore>::new();
        let source = b.source::<usize, i64>();
        let terminal = b
            .score("score", source.port(), |v| OwnedScore::new(0, *v))
            .unwrap();
        let mut g = b.build();
        let mut t = g.transaction();
        t.apply(&source, TupleChange::put(0, 3)).unwrap();
        t.commit().unwrap();
        fail_clone_at(at);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut t = g.transaction();
            t.apply(&source, TupleChange::put(0, 7)).unwrap();
            t.apply(&source, TupleChange::put(1, 9)).unwrap();
            drop(t);
        }));
        fail_clone_at(0);
        assert_eq!(g.score(), OwnedScore::new(0, 3));
        assert_eq!(g.constraint_matches(&terminal).unwrap().len(), 1);
        g.assert_consistent().unwrap();
        if result.is_ok() {
            completed = true;
            break;
        }
        failures += 1;
    }
    assert!(completed && failures > 5);
}

#[test]
fn dropping_a_successful_owned_score_trial_never_clones_its_saved_total() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let mut b = StreamGraphBuilder::<OwnedScore>::new();
    let s = b.source::<usize, i64>();
    b.score("s", s.port(), |v| OwnedScore::new(0, *v)).unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 2)).unwrap();
    t.commit().unwrap();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 5)).unwrap();
    fail_clone_at(1);
    let result = catch_unwind(AssertUnwindSafe(|| drop(t)));
    fail_clone_at(0);
    assert!(result.is_ok());
    assert_eq!(g.score(), OwnedScore::new(0, 2));
    g.assert_consistent().unwrap();
}
#[test]
fn owned_exact_scores_retain_the_same_prefix_optima_for_all_models() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0i64, 1, 2]); 2]).unwrap(),
        ListProblem::new(
            vec![1i64, 2],
            vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
        )
        .unwrap()
        .allowing_unassigned(),
    );
    let basic = |a: &Assignment<i64>| a.values().iter().flatten().sum::<i64>();
    let lists = |a: &ListAssignment<i64>| {
        a.lists()
            .iter()
            .enumerate()
            .map(|(i, l)| (i as i64 + 1) * l.iter().sum::<i64>())
            .sum::<i64>()
    };
    let owned = |v: i64| OwnedScore::new(v % 3 - 1, v).with_structural(if v == 2 { -1 } else { 0 });
    let plain =
        |v: i64| HardSoftScore::new(v % 3 - 1, v).with_structural(if v == 2 { -1 } else { 0 });
    for cap in 0..=130 {
        let a = exhaustive_search(p.basic(), |v| owned(basic(v)), cap);
        let b = exhaustive_search(p.basic(), |v| plain(basic(v)), cap);
        assert_eq!(
            (a.evaluations, a.termination, a.is_optimal()),
            (b.evaluations, b.termination, b.is_optimal())
        );
        assert_eq!(
            a.best.map(|b| (b.assignment, raw(&b.score))),
            b.best.map(|b| (b.assignment, raw(&b.score)))
        );
        let a = exhaustive_list_search(p.lists(), |v| owned(lists(v)), cap);
        let b = exhaustive_list_search(p.lists(), |v| plain(lists(v)), cap);
        assert_eq!(
            (a.evaluations, a.termination, a.is_optimal()),
            (b.evaluations, b.termination, b.is_optimal())
        );
        assert_eq!(
            a.best.map(|b| (b.assignment, raw(&b.score))),
            b.best.map(|b| (b.assignment, raw(&b.score)))
        );
        let a = exhaustive_mixed_search(&p, |v| owned(basic(v.basic()) + lists(v.lists())), cap);
        let b = exhaustive_mixed_search(&p, |v| plain(basic(v.basic()) + lists(v.lists())), cap);
        assert_eq!(
            (a.evaluations, a.termination, a.is_optimal()),
            (b.evaluations, b.termination, b.is_optimal())
        );
        assert_eq!(
            a.best.map(|b| (b.assignment, raw(&b.score))),
            b.best.map(|b| (b.assignment, raw(&b.score)))
        );
    }
}
#[test]
fn owned_score_director_clones_fail_before_installing_caches_or_moves() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let p = FiniteProblem::new(vec![Variable::new(vec![0i64, 1, 2]).with_initial(0); 2]).unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("sum", vec![0, 1], |v| {
        OwnedScore::new(0, v.iter().flatten().map(|v| **v).sum())
    })
    .unwrap();
    let mut failures = 0;
    let mut completed = false;
    for at in 1..100 {
        let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        fail_clone_at(at);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let t = d
                .apply(&Move::Change {
                    variable: 0,
                    value: Some(2),
                })
                .unwrap();
            assert_eq!(t.score(), OwnedScore::new(0, 2));
            drop(t);
        }));
        fail_clone_at(0);
        assert_eq!(d.score(), OwnedScore::ZERO);
        assert_eq!(d.assignment(), &p.initial_assignment());
        d.assert_consistent().unwrap();
        if result.is_ok() {
            completed = true;
            break;
        }
        failures += 1;
    }
    assert!(completed && failures > 1);
    let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    d.apply(&Move::Change {
        variable: 0,
        value: Some(2),
    })
    .unwrap()
    .commit();
    assert_eq!(d.score(), OwnedScore::new(0, 2));
    d.assert_consistent().unwrap();
}

#[test]
fn every_solver_backend_accepts_fractional_and_extreme_magnitude_score_levels() {
    use timetuck::HardSoftDecimalScore as DecimalScore;
    for scale in [-1000, 3, 1000] {
        let make = |h: i64, s: i64| {
            DecimalScore::new(Decimal::new(h.into(), scale), Decimal::new(s.into(), scale))
        };
        let normalize = |s: &DecimalScore| {
            (
                s.structural(),
                s.ordinary_levels()
                    .map(|v| {
                        let difference = v.scale() - scale;
                        let ten = BigInt::from(10).pow(difference.unsigned_abs());
                        let value = if difference >= 0 {
                            v.coefficient() / ten
                        } else {
                            v.coefficient() * ten
                        };
                        value.to_string().parse::<i64>().unwrap()
                    })
                    .collect(),
            )
        };
        for mode in 0..6 {
            for initialized in [false, true] {
                for budget in [0, 1, 2, 5, 12, 40] {
                    for order in [
                        MixedConstructionOrder::BasicThenLists,
                        MixedConstructionOrder::ListsThenBasic,
                    ] {
                        for (decimal, integer) in
                            policies(make).into_iter().zip(policies(HardSoftScore::new))
                        {
                            let plan = |acceptance, goal| SolverConfig {
                                limits: SolverLimits {
                                    max_evaluations: budget,
                                    best_score_limit: Some(goal),
                                    ..Default::default()
                                },
                                local_search: vec![
                                    LocalSearchConfig {
                                        acceptance,
                                        max_steps: 4,
                                        assert_scores: true,
                                        ..Default::default()
                                    };
                                    2
                                ],
                                ..Default::default()
                            };
                            let decimal_plan = SolverConfig {
                                limits: SolverLimits {
                                    max_evaluations: budget,
                                    best_score_limit: Some(make(0, 5)),
                                    ..Default::default()
                                },
                                local_search: vec![
                                    LocalSearchConfig {
                                        acceptance: decimal,
                                        max_steps: 4,
                                        assert_scores: true,
                                        ..Default::default()
                                    };
                                    2
                                ],
                                ..Default::default()
                            };
                            assert_eq!(
                                run(
                                    mode,
                                    initialized,
                                    decimal_plan,
                                    |n| make(0, n),
                                    order,
                                    normalize
                                ),
                                run(
                                    mode,
                                    initialized,
                                    plan(integer, HardSoftScore::new(0, 5)),
                                    |n| HardSoftScore::new(0, n),
                                    order,
                                    raw
                                ),
                                "mode {mode}, initialized {initialized}, budget {budget}, scale {scale}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn every_solver_backend_accepts_bendable_decimal_levels() {
    type DecimalScore = BendableDecimalScore<2, 2>;
    type IntegerScore = BendableScore<2, 2>;
    let make_integer = |h, s| IntegerScore::new([h, h], [s, s]);
    for scale in [-1000, 3, 1000] {
        let make = |h: i64, s: i64| {
            DecimalScore::new(
                std::array::from_fn(|_| Decimal::new(h.into(), scale)),
                std::array::from_fn(|_| Decimal::new(s.into(), scale)),
            )
        };
        let normalize = |s: &DecimalScore| {
            (
                s.structural(),
                s.ordinary_levels()
                    .map(|v| {
                        let difference = v.scale() - scale;
                        let ten = BigInt::from(10).pow(difference.unsigned_abs());
                        let value = if difference >= 0 {
                            v.coefficient() / ten
                        } else {
                            v.coefficient() * ten
                        };
                        value.to_string().parse::<i64>().unwrap()
                    })
                    .collect(),
            )
        };
        for mode in 0..6 {
            for initialized in [false, true] {
                for budget in [0, 1, 2, 5, 12, 40] {
                    for order in [
                        MixedConstructionOrder::BasicThenLists,
                        MixedConstructionOrder::ListsThenBasic,
                    ] {
                        for (decimal, integer) in
                            policies(make).into_iter().zip(policies(make_integer))
                        {
                            let plan = |acceptance, goal| SolverConfig {
                                limits: SolverLimits {
                                    max_evaluations: budget,
                                    best_score_limit: Some(goal),
                                    ..Default::default()
                                },
                                local_search: vec![
                                    LocalSearchConfig {
                                        acceptance,
                                        max_steps: 4,
                                        assert_scores: true,
                                        ..Default::default()
                                    };
                                    2
                                ],
                                ..Default::default()
                            };
                            let decimal_plan = SolverConfig {
                                limits: SolverLimits {
                                    max_evaluations: budget,
                                    best_score_limit: Some(make(0, 5)),
                                    ..Default::default()
                                },
                                local_search: vec![
                                    LocalSearchConfig {
                                        acceptance: decimal,
                                        max_steps: 4,
                                        assert_scores: true,
                                        ..Default::default()
                                    };
                                    2
                                ],
                                ..Default::default()
                            };
                            assert_eq!(
                                run(
                                    mode,
                                    initialized,
                                    decimal_plan,
                                    |n| make(0, n),
                                    order,
                                    normalize
                                ),
                                run(
                                    mode,
                                    initialized,
                                    plan(integer, make_integer(0, 5)),
                                    |n| make_integer(0, n),
                                    order,
                                    raw
                                ),
                                "mode {mode}, initialized {initialized}, budget {budget}, scale {scale}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn owned_scores_support_every_forager_policy_in_all_six_backends() {
    for ranking in [
        FinalistRanking::HighestScore,
        FinalistRanking::StrategicOscillationLastStep,
        FinalistRanking::StrategicOscillationBest,
    ] {
        for mode in 0..6 {
            for selection in [
                MoveSelection::FirstAccepted,
                MoveSelection::BestAccepted,
                MoveSelection::FirstBestScoreImproving,
                MoveSelection::FirstLastStepScoreImproving,
            ] {
                for limit in [None, 1.try_into().ok(), 2.try_into().ok()] {
                    for ties in [TieBreaking::First, TieBreaking::Random { seed: 17 }] {
                        for budget in [0, 1, 3, 20] {
                            let owned = SolverConfig {
                                limits: SolverLimits {
                                    max_evaluations: budget,
                                    ..Default::default()
                                },
                                local_search: vec![
                                    LocalSearchConfig {
                                        selection,
                                        ranking,
                                        accepted_candidate_limit: limit,
                                        tie_breaking: ties,
                                        max_steps: 3,
                                        assert_scores: true,
                                        ..Default::default()
                                    };
                                    2
                                ],
                                ..Default::default()
                            };
                            let plain = SolverConfig {
                                limits: SolverLimits {
                                    max_evaluations: budget,
                                    ..Default::default()
                                },
                                local_search: vec![
                                    LocalSearchConfig {
                                        selection,
                                        ranking,
                                        accepted_candidate_limit: limit,
                                        tie_breaking: ties,
                                        max_steps: 3,
                                        assert_scores: true,
                                        ..Default::default()
                                    };
                                    2
                                ],
                                ..Default::default()
                            };
                            assert_eq!(
                                run(
                                    mode,
                                    true,
                                    owned,
                                    |v| OwnedScore::new(0, v),
                                    MixedConstructionOrder::BasicThenLists,
                                    raw
                                ),
                                run(
                                    mode,
                                    true,
                                    plain,
                                    |v| HardSoftScore::new(0, v),
                                    MixedConstructionOrder::BasicThenLists,
                                    raw
                                )
                            );
                        }
                    }
                }
            }
        }
    }
}
