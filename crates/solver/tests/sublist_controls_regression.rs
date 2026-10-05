//! Original state-space and end-to-end regressions, independent of upstream credit.
use std::collections::BTreeSet;
use timetuck::*;
fn ranges(a: &ListAssignment<i32>) -> Vec<ListRange> {
    a.lists()
        .iter()
        .enumerate()
        .flat_map(|(entity, l)| {
            (0..l.len()).flat_map(move |start| {
                (start + 1..=l.len()).map(move |end| ListRange { entity, start, end })
            })
        })
        .collect()
}
fn models() -> Vec<ListProblem<i32>> {
    [false, true]
        .into_iter()
        .flat_map(|pin| {
            [false, true].into_iter().map(move |restricted| {
                let first = ListEntity::new(vec![0, 1, 2]).with_pinned_prefix(usize::from(pin));
                let second = ListEntity::new(vec![3, 4, 5]);
                let second = if restricted {
                    second.with_value_range(vec![1, 2, 3, 4, 5])
                } else {
                    second
                };
                ListProblem::new(
                    (0..6).collect(),
                    vec![first, second, ListEntity::new(vec![])],
                )
                .unwrap()
            })
        })
        .collect()
}
#[test]
fn swaps_match_independent_state_space_and_rollback_with_asymmetric_bounds() {
    for p in models() {
        let a = p.initial_assignment();
        for (lo, hi, ro, rhi) in [
            (1, 1, 3, 3),
            (3, 3, 1, 1),
            (1, 3, 2, 3),
            (1, 1, 1, 1),
            (1, usize::MAX, 1, usize::MAX),
        ] {
            for reverse in [false, true] {
                let c = SubListSwapConfig::asymmetric(lo, hi, ro, rhi)
                    .unwrap()
                    .with_reversing(reverse);
                let mut expected = BTreeSet::new();
                for l in ranges(&a) {
                    for r in ranges(&a) {
                        if !(lo..=hi).contains(&(l.end - l.start))
                            || !(ro..=rhi).contains(&(r.end - r.start))
                            || (l.entity == r.entity && l.start < r.end && r.start < l.end)
                        {
                            continue;
                        }
                        for rev in [false, true] {
                            if rev && !reverse {
                                continue;
                            }
                            let mut result = a.lists().to_vec();
                            let mut x = result[l.entity][l.start..l.end].to_vec();
                            let mut y = result[r.entity][r.start..r.end].to_vec();
                            if rev {
                                x.reverse();
                                y.reverse();
                            }
                            if l.entity == r.entity {
                                let (earlier, later, insert_early, insert_late) =
                                    if l.start < r.start {
                                        (l, r, &y, &x)
                                    } else {
                                        (r, l, &x, &y)
                                    };
                                result[l.entity] = [
                                    &a.lists()[l.entity][..earlier.start],
                                    insert_early,
                                    &a.lists()[l.entity][earlier.end..later.start],
                                    insert_late,
                                    &a.lists()[l.entity][later.end..],
                                ]
                                .concat();
                            } else {
                                result[l.entity] = [
                                    &a.lists()[l.entity][..l.start],
                                    &y,
                                    &a.lists()[l.entity][l.end..],
                                ]
                                .concat();
                                result[r.entity] = [
                                    &a.lists()[r.entity][..r.start],
                                    &x,
                                    &a.lists()[r.entity][r.end..],
                                ]
                                .concat();
                            }
                            if p.validate(&ListAssignment::new(result.clone())).is_ok() {
                                expected.insert(result);
                            }
                        }
                    }
                }
                let mut actual = BTreeSet::new();
                let mut identities = BTreeSet::new();
                let mut scratch = a.clone();
                for m in list_sublist_swap_moves_with_config(&p, &a, c).unwrap() {
                    let ListMove::SubListSwap {
                        left,
                        right,
                        reverse,
                    } = m
                    else {
                        panic!()
                    };
                    let mut pair = [
                        (left.entity, left.start, left.end),
                        (right.entity, right.start, right.end),
                    ];
                    pair.sort();
                    assert!(
                        identities.insert((pair, reverse)),
                        "duplicate symmetric move"
                    );
                    {
                        let tx = p.apply(&mut scratch, &m).unwrap();
                        actual.insert(tx.lists().to_vec());
                    }
                    assert_eq!(scratch, a);
                }
                assert_eq!(
                    actual, expected,
                    "{lo}..{hi} x {ro}..{rhi}, reverse={reverse}"
                );
            }
        }
    }
}
#[test]
fn changes_and_unassignments_match_independent_state_space() {
    for p in models() {
        let p = p.allowing_unassigned();
        let a = p.initial_assignment();
        for minimum in [2, 3, 8] {
            for reverse in [false, true] {
                for unassign in [false, true] {
                    let c = SubListChangeConfig::new(minimum, usize::MAX)
                        .unwrap()
                        .with_reversing(reverse)
                        .with_unassignment(unassign);
                    let mut expected = BTreeSet::new();
                    let mut removals = BTreeSet::new();
                    for r in ranges(&a) {
                        if r.end - r.start < minimum {
                            continue;
                        }
                        let mut removed = a.lists().to_vec();
                        let taken: Vec<_> = removed[r.entity].drain(r.start..r.end).collect();
                        if p.validate(&ListAssignment::new(removed.clone())).is_ok() {
                            removals.insert(removed.clone());
                            if unassign {
                                expected.insert(removed.clone());
                            }
                        }
                        for e in 0..removed.len() {
                            for i in 0..=removed[e].len() {
                                for rev in [false, true] {
                                    if rev && !reverse {
                                        continue;
                                    }
                                    let mut next = removed.clone();
                                    let mut values = taken.clone();
                                    if rev {
                                        values.reverse();
                                    }
                                    next[e].splice(i..i, values);
                                    if next != a.lists()
                                        && p.validate(&ListAssignment::new(next.clone())).is_ok()
                                    {
                                        expected.insert(next);
                                    }
                                }
                            }
                        }
                    }
                    let mut actual = BTreeSet::new();
                    let mut scratch = a.clone();
                    for m in list_sublist_change_moves_with_config(&p, &a, c).unwrap() {
                        let tx = p.apply(&mut scratch, &m).unwrap();
                        actual.insert(tx.lists().to_vec());
                    }
                    assert_eq!(scratch, a);
                    assert_eq!(actual, expected);
                    let mut actual_removals = BTreeSet::new();
                    for m in list_sublist_unassign_moves(
                        &p,
                        &a,
                        SubListSize::new(minimum, usize::MAX).unwrap(),
                    )
                    .unwrap()
                    {
                        let tx = p.apply(&mut scratch, &m).unwrap();
                        actual_removals.insert(tx.lists().to_vec());
                    }
                    assert_eq!(actual_removals, removals);
                }
            }
        }
    }
}
#[test]
fn configured_families_work_in_list_and_mixed_scoped_and_stream_search() {
    for neighborhood in [
        Neighborhood::SubListChange(SubListChangeConfig::new(2, 2).unwrap()),
        Neighborhood::SubListSwap(SubListSwapConfig::new(1, 1).unwrap()),
        Neighborhood::SubListUnassign(SubListSize::new(2, 2).unwrap()),
    ] {
        let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])])
            .unwrap()
            .allowing_unassigned();
        let nscore = |v: &[i32]| SimpleScore::new(if v == [1, 0] || v.is_empty() { 10 } else { 0 });
        let config = LocalSearchConfig {
            neighborhood,
            max_steps: 1,
            assert_scores: true,
            ..Default::default()
        };
        let mut n = ListConstraintNetwork::new(1, 2);
        n.add_constraint("order", vec![ListScope::Entity(0)], |v| {
            let ListFact::Entity(values) = &v[0] else {
                panic!()
            };
            nscore(values)
        })
        .unwrap();
        let r = list_local_search_from(&p, &n, p.initial_assignment(), config).unwrap();
        assert_eq!(r.best.unwrap().score, SimpleScore::new(10));
        let mut b = StreamGraphBuilder::new();
        let sources = ListStreamSources::<i32>::new(&mut b);
        b.score("order", sources.entities.port(), |e| nscore(&e.1))
            .unwrap();
        let r =
            list_stream_local_search_from(&p, b.build(), sources, p.initial_assignment(), config)
                .unwrap();
        assert_eq!(r.best.unwrap().score, SimpleScore::new(10));
        let mixed = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
            p,
        );
        let mut n = MixedConstraintNetwork::new(1, 1, 2);
        n.add_constraint("order", vec![MixedScope::ListEntity(0)], |v| {
            let MixedFact::ListEntity(values) = &v[0] else {
                panic!()
            };
            nscore(values)
        })
        .unwrap();
        let c = MixedLocalSearchConfig {
            search: config,
            ..Default::default()
        };
        let r = mixed_local_search_from(&mixed, &n, mixed.initial_assignment(), c).unwrap();
        assert_eq!(r.best.unwrap().score, SimpleScore::new(10));
        let mut b = StreamGraphBuilder::new();
        let sources = MixedStreamSources::<i32, i32>::new(&mut b);
        b.score("order", sources.lists.entities.port(), |e| nscore(&e.1))
            .unwrap();
        let r = mixed_stream_local_search_from(
            &mixed,
            b.build(),
            sources,
            mixed.initial_assignment(),
            c,
        )
        .unwrap();
        assert_eq!(r.best.unwrap().score, SimpleScore::new(10));
    }
}
#[test]
fn invalid_policies_fail_before_factories_or_scoring_even_with_zero_budget() {
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])]).unwrap();
    let mixed = MixedProblem::new(FiniteProblem::<i32>::new(vec![]).unwrap(), p.clone());
    let mut n = ListConstraintNetwork::<_, SimpleScore>::new(1, 2);
    n.add_constraint("never", vec![], |_| panic!("invalid policy scored"))
        .unwrap();
    let mut mn = MixedConstraintNetwork::<_, _, SimpleScore>::new(0, 1, 2);
    mn.add_constraint("never", vec![], |_| panic!("invalid policy scored"))
        .unwrap();
    for neighborhood in [
        Neighborhood::SubListChange(SubListChangeConfig::default().with_unassignment(true)),
        Neighborhood::SubListUnassign(SubListSize::new(2, 3).unwrap()),
        Neighborhood::SubListUnassign(SubListSize::new(1, 3).unwrap()),
    ] {
        let c = LocalSearchConfig {
            neighborhood,
            max_evaluations: 0,
            ..Default::default()
        };
        let plan = SolverSequence {
            phases: vec![SolverPhase::LocalSearch(c)],
            ..Default::default()
        };
        assert!(matches!(
            solve_list(&p, &n, plan.clone()),
            Err(SolverError::Phase { phase: 0, .. })
        ));
        assert!(matches!(
            solve_list_stream(&p, || panic!("invalid policy built graph"), plan),
            Err(SolverError::Phase { phase: 0, .. })
        ));
        let plan = MixedSolverSequence {
            phases: vec![MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                search: c,
                ..Default::default()
            })],
            ..Default::default()
        };
        assert!(matches!(
            solve_mixed(&mixed, &mn, plan.clone()),
            Err(SolverError::Phase { phase: 0, .. })
        ));
        assert!(matches!(
            solve_mixed_stream(&mixed, || panic!("invalid policy built graph"), plan),
            Err(SolverError::Phase { phase: 0, .. })
        ));
    }
}
