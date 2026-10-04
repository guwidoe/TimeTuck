//! Original independent construction references; no upstream method credit.
use std::cell::Cell;
use timetuck::*;
fn pooled() -> ConstructionConfig<'static> {
    ConstructionConfig {
        placement: ConstructionPlacement::Pooled,
        max_evaluations: u64::MAX,
        ..Default::default()
    }
}
#[test]
fn a_global_placement_changes_order_and_reuses_the_selected_score() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let calls = Cell::new(0);
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("coupled", vec![0, 1], |v| {
        calls.set(calls.get() + 1);
        let a = v[0].copied();
        let b = v[1].copied();
        SimpleScore::new(
            a.map_or(0, |x| if x == 0 { 5 } else { 0 })
                + b.map_or(0, |x| if x == 0 { 10 } else { 1 })
                - if a == Some(0) && b == Some(0) { 100 } else { 0 },
        )
    })
    .unwrap();
    let one = greedy_construction(
        &p,
        &n,
        ConstructionConfig {
            max_steps: Some(1),
            ..pooled()
        },
    )
    .unwrap();
    assert_eq!(one.assignment.values(), &[None, Some(0)]);
    assert_eq!(one.termination, ConstructionTermination::StepLimit);
    assert_eq!(one.evaluations, 5);
    assert_eq!(calls.get(), 5);
    calls.set(0);
    let all = greedy_construction(&p, &n, pooled()).unwrap();
    assert_eq!(all.assignment.values(), &[Some(1), Some(0)]);
    assert_eq!(all.score, Some(SimpleScore::new(10)));
    assert_eq!(
        (all.evaluations, all.completed_steps, calls.get()),
        (7, 2, 7)
    );
    let sequential = greedy_construction(&p, &n, ConstructionConfig::default()).unwrap();
    assert_eq!(sequential.assignment.values(), &[Some(0), Some(1)]);
    assert_eq!(sequential.score, Some(SimpleScore::new(6)));
    let early = greedy_construction(
        &p,
        &n,
        ConstructionConfig {
            pick_early: ConstructionPickEarly::FirstFeasibleScore,
            ..pooled()
        },
    )
    .unwrap();
    assert_eq!(early.evaluations, 3); // A global pool can still be deliberately cut short.
}
#[test]
fn generated_basic_pools_match_full_assignment_reference_at_every_budget() {
    let mut rng = 79_u64;
    let mut random = || {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        (rng >> 32) as i64
    };
    for case in 0..64 {
        let size = 1 + case % 4;
        let weights: Vec<Vec<i64>> = (0..size)
            .map(|i| (0..1 + (i + case) % 3).map(|_| random() % 19 - 9).collect())
            .collect();
        let mut initial = vec![None; size];
        if size > 2 && case % 2 == 0 {
            initial[1] = Some(0);
        }
        let p = FiniteProblem::new(
            weights
                .iter()
                .enumerate()
                .map(|(i, w)| {
                    let v = Variable::new((0..w.len()).collect());
                    if initial[i].is_some() {
                        v.with_initial(0).with_pinned(true)
                    } else {
                        v
                    }
                })
                .collect(),
        )
        .unwrap();
        let score = |v: &[Option<usize>]| {
            let hard = -((0..size)
                .flat_map(|i| (i + 1..size).map(move |j| (i, j)))
                .filter(|&(i, j)| v[i].is_some() && v[i] == v[j])
                .count() as i64);
            let soft = v
                .iter()
                .enumerate()
                .map(|(i, v)| v.map_or(0, |v| weights[i][v]))
                .sum();
            HardSoftScore::new(hard, soft)
                .with_structural(-(v.iter().filter(|v| v.is_none()).count() as i64))
        };
        let mut n = ConstraintNetwork::new(size);
        n.add_constraint("objective", (0..size).collect(), |v| {
            score(&v.iter().map(|v| v.copied()).collect::<Vec<_>>()).with_structural(0)
        })
        .unwrap();
        let max = 1 + (size * (size + 1) / 2 * 3) as u64;
        for budget in 0..=max {
            let mut state = initial.clone();
            let mut evaluations = u64::from(budget > 0);
            let mut steps = 0;
            let mut stop = if budget == 0 {
                ConstructionTermination::EvaluationLimit
            } else {
                ConstructionTermination::Completed
            };
            if budget > 0 {
                'search: loop {
                    let mut candidates = vec![];
                    for i in 0..size {
                        if state[i].is_none() {
                            for value in 0..weights[i].len() {
                                let mut candidate = state.clone();
                                candidate[i] = Some(value);
                                candidates.push(candidate);
                            }
                        }
                    }
                    if candidates.is_empty() {
                        break;
                    }
                    let mut best: Option<(HardSoftScore, Vec<Option<usize>>)> = None;
                    let mut cut = false;
                    let mut count = 0;
                    for candidate in candidates {
                        if evaluations == budget {
                            cut = true;
                            break;
                        }
                        evaluations += 1;
                        count += 1;
                        let s = score(&candidate);
                        if best.as_ref().is_none_or(|(old, _)| s > *old) {
                            best = Some((s, candidate));
                        }
                    }
                    steps += u64::from(count > 0);
                    if let Some((_, chosen)) = best {
                        state = chosen;
                    }
                    if cut {
                        stop = ConstructionTermination::EvaluationLimit;
                        break 'search;
                    }
                }
            }
            let r = greedy_construction(
                &p,
                &n,
                ConstructionConfig {
                    max_evaluations: budget,
                    assert_scores: true,
                    ..pooled()
                },
            )
            .unwrap();
            assert_eq!(r.assignment.values(), state, "case {case} budget {budget}");
            assert_eq!(
                (r.evaluations, r.completed_steps, r.termination),
                (evaluations, steps, stop)
            );
            assert_eq!(r.score, (budget > 0).then(|| score(&state)));
        }
    }
}
#[test]
fn optional_candidates_are_reconsidered_after_other_placements_and_then_stop() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0]).allowing_unassigned(),
        Variable::new(vec![0]).allowing_unassigned(),
        Variable::new(vec![0])
            .allowing_unassigned()
            .with_pinned(true),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("unlock", vec![0, 1], |v| {
        SimpleScore::new(
            if v[0].is_some() {
                if v[1].is_some() { 100 } else { -1 }
            } else {
                0
            } + if v[1].is_some() { 5 } else { 0 },
        )
    })
    .unwrap();
    let c = ConstructionConfig {
        optional: OptionalConstruction::NonWorsening,
        ..pooled()
    };
    let r = greedy_construction(&p, &n, c).unwrap();
    assert_eq!(r.assignment.values(), &[Some(0), Some(0), None]);
    assert_eq!(r.score, Some(SimpleScore::new(105)));
    assert_eq!((r.evaluations, r.completed_steps), (4, 2));
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("reject", vec![0, 1], |v| {
        SimpleScore::new(-(v.iter().flatten().count() as i64))
    })
    .unwrap();
    let r = greedy_construction(&p, &n, c).unwrap();
    assert_eq!(
        (r.evaluations, r.completed_steps, r.assigned_variables),
        (3, 1, 0)
    );
    assert_eq!(r.termination, ConstructionTermination::Completed);
    let r = greedy_construction(&p, &n, pooled()).unwrap();
    assert_eq!(r.evaluations, 1);
}
fn route_score(lists: &[Vec<i32>]) -> i64 {
    lists
        .iter()
        .enumerate()
        .map(|(e, l)| {
            let positions = l
                .iter()
                .enumerate()
                .map(|(i, &v)| i64::from(v) * (i as i64 + 1) * (e as i64 + 1))
                .sum::<i64>();
            let edges = l
                .windows(2)
                .map(|w| i64::from((w[0] - w[1]).abs()) * 3)
                .sum::<i64>();
            -positions - edges
        })
        .sum()
}
#[test]
fn list_pools_match_independent_insertions_with_prefix_pins_and_owner_ranges() {
    let p = ListProblem::new(
        vec![9, 1, 2, 3],
        vec![
            ListEntity::new(vec![9]).with_pinned_prefix(1),
            ListEntity::new(vec![]).with_value_range(vec![2, 3]),
        ],
    )
    .unwrap();
    let mut n = ListConstraintNetwork::new(2, 4);
    n.add_constraint(
        "route",
        vec![ListScope::Entity(0), ListScope::Entity(1)],
        |f| {
            let lists = f
                .iter()
                .map(|f| {
                    let ListFact::Entity(l) = f else { panic!() };
                    l.to_vec()
                })
                .collect::<Vec<_>>();
            SimpleScore::new(route_score(&lists))
        },
    )
    .unwrap();
    let mut expected = vec![vec![9], vec![]];
    for step in 1..=3 {
        let mut candidates = vec![];
        for v in [1, 2, 3] {
            if expected.iter().any(|l| l.contains(&v)) {
                continue;
            }
            for e in 0..2 {
                if e == 1 && v == 1 {
                    continue;
                }
                for i in usize::from(e == 0)..=expected[e].len() {
                    let mut c = expected.clone();
                    c[e].insert(i, v);
                    candidates.push(c);
                }
            }
        }
        let best_score = candidates.iter().map(|c| route_score(c)).max().unwrap();
        expected = candidates
            .into_iter()
            .find(|c| route_score(c) == best_score)
            .unwrap();
        let r = greedy_list_construction(
            &p,
            &n,
            ConstructionConfig {
                max_steps: Some(step),
                assert_scores: true,
                ..pooled()
            },
        )
        .unwrap();
        assert_eq!(r.assignment.lists(), expected);
        assert_eq!(r.assigned_values, step as usize);
        assert_eq!(r.completed_steps, step);
        assert_eq!(
            r.termination,
            if step == 3 {
                ConstructionTermination::Completed
            } else {
                ConstructionTermination::StepLimit
            }
        );
        p.validate(&r.assignment).unwrap();
    }
}
#[test]
fn mixed_pools_compare_both_components_and_use_component_order_only_for_ties() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap(),
        ListProblem::new(
            vec![7],
            vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
        )
        .unwrap(),
    );
    let mut n = MixedConstraintNetwork::new(1, 2, 1);
    n.add_constraint(
        "coupled",
        vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
        |f| {
            let MixedFact::Basic(a) = f[0] else { panic!() };
            let MixedFact::ListValue { ref element, .. } = f[1] else {
                panic!()
            };
            let a = a.copied();
            let b = element.as_ref().map(|e| e.position.entity);
            SimpleScore::new(
                a.map_or(0, |x| if x == 0 { 5 } else { 0 })
                    + b.map_or(0, |x| if x == 0 { 10 } else { 1 })
                    - if a == Some(0) && b == Some(0) { 100 } else { 0 },
            )
        },
    )
    .unwrap();
    for order in [
        MixedConstructionOrder::BasicThenLists,
        MixedConstructionOrder::ListsThenBasic,
    ] {
        let c = MixedConstructionConfig {
            order,
            construction: pooled(),
        };
        let one = greedy_mixed_construction(
            &p,
            &n,
            MixedConstructionConfig {
                construction: ConstructionConfig {
                    max_steps: Some(1),
                    ..pooled()
                },
                ..c
            },
        )
        .unwrap();
        assert_eq!(one.assignment.basic().values(), &[None]);
        assert_eq!(one.assignment.lists().lists(), &[vec![7], vec![]]);
        assert_eq!(
            (one.assigned_basic_variables, one.assigned_list_values),
            (0, 1)
        );
        let r = greedy_mixed_construction(&p, &n, c).unwrap();
        assert_eq!(r.score, Some(SimpleScore::new(10)));
        assert_eq!(r.assignment.basic().values(), &[Some(1)]);
        assert_eq!((r.evaluations, r.completed_steps), (7, 2));
        let empty = MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 2, 1);
        let tied = greedy_mixed_construction(
            &p,
            &empty,
            MixedConstructionConfig {
                construction: ConstructionConfig {
                    max_steps: Some(1),
                    ..pooled()
                },
                ..c
            },
        )
        .unwrap();
        assert_eq!(
            tied.assigned_basic_variables,
            usize::from(order == MixedConstructionOrder::BasicThenLists)
        );
        assert_eq!(
            tied.assigned_list_values,
            usize::from(order == MixedConstructionOrder::ListsThenBasic)
        );
    }
}
#[test]
fn pooled_owned_decimal_scores_preserve_differences_below_float_precision() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0]); 2]).unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("precise", vec![0, 1], |v| {
        let value = match (v[0], v[1]) {
            (None, None) => "0",
            (Some(_), None) => "9007199254740992.000000000000000001",
            (None, Some(_)) => "9007199254740992.000000000000000002",
            _ => "18014398509481984.000000000000000003",
        };
        value.parse::<SimpleDecimalScore>().unwrap()
    })
    .unwrap();
    let r = greedy_construction(
        &p,
        &n,
        ConstructionConfig {
            max_steps: Some(1),
            ..pooled()
        },
    )
    .unwrap();
    assert_eq!(r.assignment.values(), &[None, Some(0)]);
}

#[test]
fn empty_optional_pools_finish_without_consuming_a_step_in_all_backends() {
    let c = ConstructionConfig {
        max_steps: Some(0),
        optional: OptionalConstruction::NonWorsening,
        ..pooled()
    };
    let basic =
        FiniteProblem::new(vec![Variable::<i32>::new(vec![]).allowing_unassigned()]).unwrap();
    let list = ListProblem::new(vec![7], vec![ListEntity::new(vec![]).with_pinned(true)])
        .unwrap()
        .allowing_unassigned();
    macro_rules! check {
        ($result:expr) => {{
            let r = $result.unwrap();
            assert_eq!(r.termination, ConstructionTermination::Completed);
            assert_eq!((r.evaluations, r.completed_steps), (1, 0));
            assert!(r.complete);
        }};
    }
    check!(greedy_construction(
        &basic,
        &ConstraintNetwork::<_, SimpleScore>::new(1),
        c
    ));
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (usize, Option<i32>)>();
    check!(greedy_stream_construction(&basic, b.build(), s, c));
    check!(greedy_list_construction(
        &list,
        &ListConstraintNetwork::<_, SimpleScore>::new(1, 1),
        c
    ));
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = ListStreamSources::new(&mut b);
    check!(greedy_list_stream_construction(&list, b.build(), s, c));
    let mixed = MixedProblem::new(basic, list);
    let c = MixedConstructionConfig {
        construction: c,
        ..Default::default()
    };
    check!(greedy_mixed_construction(
        &mixed,
        &MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 1, 1),
        c
    ));
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = MixedStreamSources::new(&mut b);
    check!(greedy_mixed_stream_construction(&mixed, b.build(), s, c));
}
