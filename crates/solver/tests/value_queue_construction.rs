//! Original value-first construction checks; no adapted upstream methods.
use timetuck::*;
fn config() -> ConstructionConfig<'static> {
    ConstructionConfig {
        placement: ConstructionPlacement::ValueQueue,
        max_evaluations: u64::MAX,
        ..Default::default()
    }
}
#[test]
fn queue_rotates_values_and_selects_the_best_target_with_stable_ties() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]); 3]).unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("target", vec![0, 1, 2], |v| {
        SimpleScore::new(
            v[0].map_or(0, |v| 3 - *v) + v[1].map_or(0, |v| 10 - *v) + v[2].map_or(0, |v| 3 - *v),
        )
    })
    .unwrap();
    let one = greedy_construction(
        &p,
        &n,
        ConstructionConfig {
            max_steps: Some(1),
            ..config()
        },
    )
    .unwrap();
    assert_eq!(one.assignment.values(), &[None, Some(0), None]);
    assert_eq!(
        (one.evaluations, one.completed_steps, one.termination),
        (4, 1, ConstructionTermination::StepLimit)
    );
    let all = greedy_construction(&p, &n, config()).unwrap();
    assert_eq!(all.assignment.values(), &[Some(1), Some(0), Some(0)]);
    assert_eq!((all.evaluations, all.completed_steps), (7, 3));
    assert_eq!(all.score, Some(SimpleScore::new(15)));
    // Each call restarts the queue; supplied values are preserved.
    let resumed = greedy_construction_from(&p, &n, one.assignment, config()).unwrap();
    assert_eq!(resumed.assignment.values(), &[Some(0), Some(0), Some(1)]);
}
#[test]
fn optional_rejections_are_retried_after_progress_but_a_dry_rotation_stops() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0]).allowing_unassigned(),
        Variable::new(vec![1]).allowing_unassigned(),
        Variable::new(vec![2])
            .allowing_unassigned()
            .with_pinned(true),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("unlock", vec![0, 1], |v| {
        SimpleScore::new(if v[1].is_some() {
            5 + if v[0].is_some() { 10 } else { 0 }
        } else if v[0].is_some() {
            -1
        } else {
            0
        })
    })
    .unwrap();
    let c = ConstructionConfig {
        optional: OptionalConstruction::NonWorsening,
        ..config()
    };
    let r = greedy_construction(&p, &n, c).unwrap();
    assert_eq!(r.assignment.values(), &[Some(0), Some(1), None]);
    assert_eq!(
        (r.evaluations, r.completed_steps, r.assigned_variables),
        (4, 3, 2)
    );
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("reject", vec![0, 1], |v| {
        SimpleScore::new(-(v.iter().flatten().count() as i64))
    })
    .unwrap();
    let r = greedy_construction(&p, &n, c).unwrap();
    assert_eq!(
        (r.evaluations, r.completed_steps, r.assigned_variables),
        (3, 2, 0)
    );
    assert_eq!(r.termination, ConstructionTermination::Completed);
    assert_eq!(
        greedy_construction(&p, &n, config()).unwrap().evaluations,
        1
    );
}
#[derive(Debug, PartialEq)]
struct ResultView {
    state: Vec<Option<usize>>,
    score: Option<HardSoftScore>,
    evaluations: u64,
    steps: u64,
    reason: ConstructionTermination,
}
fn objective(values: &[Option<usize>], weights: &[[i64; 3]]) -> HardSoftScore {
    let hard = -(values
        .iter()
        .enumerate()
        .flat_map(|(i, a)| values[i + 1..].iter().map(move |b| (a, b)))
        .filter(|(a, b)| a.is_some() && a == b)
        .count() as i64);
    let soft = values
        .iter()
        .enumerate()
        .map(|(i, v)| v.map_or(0, |v| weights[i][v]))
        .sum();
    HardSoftScore::new(hard, soft)
}
fn run(
    p: &FiniteProblem<usize>,
    weights: &[[i64; 3]],
    c: ConstructionConfig,
    mode: usize,
) -> ResultView {
    macro_rules! view {
        ($r:expr, $state:expr) => {{
            let r = $r.unwrap();
            ResultView {
                state: $state(&r.assignment),
                score: r.score,
                evaluations: r.evaluations,
                steps: r.completed_steps,
                reason: r.termination,
            }
        }};
    }
    if mode == 0 {
        let mut n = ConstraintNetwork::new(p.variables().len());
        n.add_constraint("objective", (0..p.variables().len()).collect(), |v| {
            objective(&v.iter().map(|v| v.copied()).collect::<Vec<_>>(), weights)
        })
        .unwrap();
        return view!(greedy_construction(p, &n, c), |a: &Assignment<usize>| a
            .values()
            .to_vec());
    }
    if mode == 2 {
        let m = MixedProblem::new(
            p.clone(),
            ListProblem::<usize>::new(vec![], vec![]).unwrap(),
        );
        let mut n = MixedConstraintNetwork::new(p.variables().len(), 0, 0);
        n.add_constraint(
            "objective",
            (0..p.variables().len()).map(MixedScope::Basic).collect(),
            |v| {
                objective(
                    &v.iter()
                        .map(|f| {
                            let MixedFact::Basic(v) = f else { panic!() };
                            v.copied()
                        })
                        .collect::<Vec<_>>(),
                    weights,
                )
            },
        )
        .unwrap();
        return view!(
            greedy_mixed_construction(
                &m,
                &n,
                MixedConstructionConfig {
                    construction: c,
                    ..Default::default()
                }
            ),
            |a: &MixedAssignment<usize, usize>| a.basic().values().to_vec()
        );
    }
    let mut b = StreamGraphBuilder::new();
    let sources = MixedStreamSources::<usize, usize>::new(&mut b);
    b.score("weights", sources.basic.port(), |(i, v)| {
        HardSoftScore::new(0, v.map_or(0, |v| weights[*i][v]))
    })
    .unwrap();
    let j = b
        .join_equal(sources.basic.port(), sources.basic.port(), |_| (), |_| ())
        .unwrap();
    b.score("conflicts", &j, |((i, a), (k, z))| {
        HardSoftScore::new(-i64::from(i < k && a.is_some() && a == z), 0)
    })
    .unwrap();
    if mode == 1 {
        return view!(
            greedy_stream_construction(p, b.build(), sources.basic, c),
            |a: &Assignment<usize>| a.values().to_vec()
        );
    }
    let m = MixedProblem::new(
        p.clone(),
        ListProblem::<usize>::new(vec![], vec![]).unwrap(),
    );
    view!(
        greedy_mixed_stream_construction(
            &m,
            b.build(),
            sources,
            MixedConstructionConfig {
                construction: c,
                ..Default::default()
            }
        ),
        |a: &MixedAssignment<usize, usize>| a.basic().values().to_vec()
    )
}
// Independent full-assignment reference: materialize target trials for each value,
// advance the value cursor and exhaust a complete rotation before stopping.
fn reference(p: &FiniteProblem<usize>, weights: &[[i64; 3]], budget: u64) -> ResultView {
    let mut state = p.initial_assignment().values().to_vec();
    let score = |state: &[Option<usize>]| {
        objective(state, weights).with_structural(
            -(p.variables()
                .iter()
                .zip(state)
                .filter(|(v, x)| !v.allows_unassigned() && x.is_none())
                .count() as i64),
        )
    };
    let mut r = ResultView {
        state: vec![],
        score: None,
        evaluations: 0,
        steps: 0,
        reason: ConstructionTermination::Completed,
    };
    if budget == 0 {
        r.reason = ConstructionTermination::EvaluationLimit;
        r.state = state;
        return r;
    }
    r.evaluations = 1;
    let mut values = vec![];
    for v in p.variables().iter().flat_map(|v| v.domain()) {
        if !values.contains(v) {
            values.push(*v);
        }
    }
    let mut index = 0;
    let mut unsuccessful = 0;
    while unsuccessful < values.len() {
        let v = values[index];
        index = (index + 1) % values.len();
        unsuccessful += 1;
        let mut trials = vec![];
        for (i, variable) in p.variables().iter().enumerate() {
            if !variable.is_pinned() && state[i].is_none() && variable.domain().contains(&v) {
                let mut trial = state.clone();
                trial[i] = Some(v);
                trials.push((i, trial));
            }
        }
        let old = score(&state);
        let mut best = None;
        let mut scanned = 0;
        let mut cut = false;
        for (i, trial) in trials {
            if r.evaluations == budget {
                cut = true;
                break;
            }
            r.evaluations += 1;
            scanned += 1;
            let candidate = score(&trial);
            if (!p.variables()[i].allows_unassigned() || candidate >= old)
                && best.as_ref().is_none_or(|(s, _)| candidate > *s)
            {
                best = Some((candidate, trial));
            }
        }
        r.steps += u64::from(scanned > 0);
        if let Some((_, trial)) = best {
            state = trial;
            unsuccessful = 0;
        }
        if cut {
            r.reason = ConstructionTermination::EvaluationLimit;
            break;
        }
    }
    r.score = Some(score(&state));
    r.state = state;
    r
}
#[test]
fn heterogeneous_queues_match_independent_reference_in_four_paths_at_every_budget() {
    let mut seed = 619_u64;
    let mut random = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        seed >> 32
    };
    for case in 0..32 {
        let size = 1 + case % 4;
        let mut variables = vec![];
        let mut weights = vec![];
        for i in 0..size {
            let mut domain = (0..3).filter(|_| random() % 3 != 0).collect::<Vec<_>>();
            let rotation = i % domain.len().max(1);
            domain.rotate_left(rotation);
            let optional = domain.is_empty() || random() % 2 == 0;
            let mut v = Variable::new(domain.clone());
            if optional {
                v = v.allowing_unassigned();
            }
            if random() % 4 == 0 && !domain.is_empty() {
                v = v.with_initial(domain[0]);
            }
            if random() % 4 == 0 && (optional || v.initial().is_some()) {
                v = v.with_pinned(true);
            }
            variables.push(v);
            weights.push([
                random() as i64 % 13 - 6,
                random() as i64 % 13 - 6,
                random() as i64 % 13 - 6,
            ]);
        }
        let p = FiniteProblem::new(variables).unwrap();
        for budget in 0..=24 {
            let expected = reference(&p, &weights, budget);
            for mode in 0..4 {
                let actual = run(
                    &p,
                    &weights,
                    ConstructionConfig {
                        max_evaluations: budget,
                        optional: OptionalConstruction::NonWorsening,
                        assert_scores: true,
                        ..config()
                    },
                    mode,
                );
                assert_eq!(actual, expected, "case {case} mode {mode} budget {budget}");
            }
        }
    }
}
#[test]
fn empty_queue_and_unreachable_optional_values_do_not_consume_steps() {
    for variables in [
        vec![],
        vec![Variable::new(vec![]).allowing_unassigned()],
        vec![
            Variable::new(vec![0])
                .allowing_unassigned()
                .with_pinned(true),
        ],
    ] {
        let p = FiniteProblem::new(variables).unwrap();
        for mode in 0..4 {
            let r = run(
                &p,
                &vec![[0; 3]; p.variables().len()],
                ConstructionConfig {
                    max_steps: Some(0),
                    optional: OptionalConstruction::NonWorsening,
                    ..config()
                },
                mode,
            );
            assert_eq!(
                (r.evaluations, r.steps, r.reason),
                (1, 0, ConstructionTermination::Completed)
            );
        }
    }
}
#[test]
fn list_queue_preserves_sequential_insertion_and_mixed_component_order() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap(),
        ListProblem::new(vec![7, 8], vec![ListEntity::new(vec![])]).unwrap(),
    );
    let n = MixedConstraintNetwork::<_, _, SimpleScore>::new(2, 1, 2);
    for order in [
        MixedConstructionOrder::BasicThenLists,
        MixedConstructionOrder::ListsThenBasic,
    ] {
        let c = MixedConstructionConfig {
            construction: ConstructionConfig {
                max_steps: Some(2),
                ..config()
            },
            order,
        };
        let plain = greedy_mixed_construction(&p, &n, c).unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = MixedStreamSources::new(&mut b);
        let stream = greedy_mixed_stream_construction(&p, b.build(), s, c).unwrap();
        assert_eq!(plain, stream);
        assert_eq!(
            (plain.assigned_basic_variables, plain.assigned_list_values),
            if order == MixedConstructionOrder::BasicThenLists {
                (2, 0)
            } else {
                (0, 2)
            }
        );
        if order == MixedConstructionOrder::BasicThenLists {
            assert_eq!(plain.assignment.basic().values(), &[Some(0), Some(1)]);
        } else {
            assert_eq!(plain.assignment.lists().lists(), &[vec![8, 7]]);
        }
    }
    let n = ListConstraintNetwork::<_, SimpleScore>::new(1, 2);
    let queued = greedy_list_construction(p.lists(), &n, config()).unwrap();
    let sequential = greedy_list_construction(
        p.lists(),
        &n,
        ConstructionConfig {
            placement: ConstructionPlacement::Sequential,
            ..config()
        },
    )
    .unwrap();
    assert_eq!(queued, sequential);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = ListStreamSources::new(&mut b);
    assert_eq!(
        queued,
        greedy_list_stream_construction(p.lists(), b.build(), s, config()).unwrap()
    );
}

#[test]
fn coupled_mixed_scores_change_queue_targets_after_list_construction() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]); 2]).unwrap(),
        ListProblem::new(
            vec![7_i64],
            vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
        )
        .unwrap(),
    );
    let objective = |a: Option<i64>, z: Option<i64>, owner: Option<usize>| {
        SimpleScore::new(
            a.map_or(0, |v| if owner.is_some() { 10 - v } else { 3 - v })
                + z.map_or(0, |v| if owner.is_some() { 3 - v } else { 10 - v })
                + owner.map_or(0, |e| 20 + 2 * e as i64),
        )
    };
    let mut n = MixedConstraintNetwork::new(2, 2, 1);
    n.add_constraint(
        "coupled",
        vec![
            MixedScope::Basic(0),
            MixedScope::Basic(1),
            MixedScope::ListValue(0),
        ],
        |f| {
            let (MixedFact::Basic(a), MixedFact::Basic(z), MixedFact::ListValue { element, .. }) =
                (&f[0], &f[1], &f[2])
            else {
                panic!()
            };
            objective(
                a.copied(),
                z.copied(),
                element.as_ref().map(|e| e.position.entity),
            )
        },
    )
    .unwrap();
    for order in [
        MixedConstructionOrder::BasicThenLists,
        MixedConstructionOrder::ListsThenBasic,
    ] {
        let c = MixedConstructionConfig {
            construction: ConstructionConfig {
                assert_scores: true,
                ..config()
            },
            order,
        };
        let plain = greedy_mixed_construction(&p, &n, c).unwrap();
        let mut b = StreamGraphBuilder::new();
        let s = MixedStreamSources::new(&mut b);
        let a = b.filter(s.basic.port(), |(i, _)| *i == 0).unwrap();
        let z = b.filter(s.basic.port(), |(i, _)| *i == 1).unwrap();
        let j = b.join_equal(&a, &z, |_| (), |_| ()).unwrap();
        let j = b
            .join_equal(&j, s.lists.values.port(), |_| (), |_| ())
            .unwrap();
        b.score("coupled", &j, |(((_, a), (_, z)), v)| {
            objective(*a, *z, v.position.map(|p| p.entity))
        })
        .unwrap();
        let stream = greedy_mixed_stream_construction(&p, b.build(), s, c).unwrap();
        assert_eq!(plain, stream);
        assert_eq!(
            plain.assignment.basic().values(),
            if order == MixedConstructionOrder::BasicThenLists {
                &[Some(1), Some(0)]
            } else {
                &[Some(0), Some(1)]
            }
        );
        assert_eq!(
            plain.assignment.lists().location(&7),
            Some(ListPosition {
                entity: 1,
                index: 0
            })
        );
        assert_eq!(plain.score, Some(SimpleScore::new(34)));
        assert!(plain.complete);
    }
}
#[test]
fn exact_decimal_target_comparison_and_early_pick_are_distinct() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0]); 2]).unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("precision", vec![0, 1], |v| {
        (if v[1].is_some() {
            "9007199254740992.000000000000000002"
        } else if v[0].is_some() {
            "9007199254740992.000000000000000001"
        } else {
            "0"
        })
        .parse::<SimpleDecimalScore>()
        .unwrap()
    })
    .unwrap();
    for early in [false, true] {
        let r = greedy_construction(
            &p,
            &n,
            ConstructionConfig {
                max_steps: Some(1),
                pick_early: if early {
                    ConstructionPickEarly::FirstFeasibleScore
                } else {
                    ConstructionPickEarly::Never
                },
                ..config()
            },
        )
        .unwrap();
        assert_eq!(
            r.assignment.values(),
            if early {
                &[Some(0), None]
            } else {
                &[None, Some(0)]
            }
        );
        assert_eq!(r.evaluations, if early { 2 } else { 3 });
    }
}
