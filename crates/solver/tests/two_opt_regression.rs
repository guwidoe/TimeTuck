//! Original route-neighborhood regressions; no source-method coverage credit.
use std::collections::HashSet;
use timetuck::*;
#[test]
fn finite_route_sets_match_independent_successors_with_pins_ranges_and_empty_owners() {
    for cut in 0..=7 {
        let p = ListProblem::new(
            (0..8).collect(),
            vec![
                ListEntity::new((0..cut).collect()).with_pinned_prefix(usize::from(cut > 0)),
                ListEntity::new((cut..7).collect())
                    .with_value_range((0..8).filter(|&v| v != 2 || cut <= 2).collect()),
                ListEntity::new(vec![7]).with_pinned(true),
                ListEntity::new(vec![]),
            ],
        )
        .unwrap()
        .allowing_unassigned();
        let a = p.initial_assignment();
        for cross_entity in [false, true] {
            let mut expected = HashSet::new();
            for (e, row) in a.lists().iter().enumerate() {
                if p.entities()[e].is_pinned() {
                    continue;
                }
                for start in p.entities()[e].pinned_prefix()..row.len() {
                    for end in start + 2..=row.len() {
                        let mut next = a.lists().to_vec();
                        next[e][start..end].reverse();
                        expected.insert(next);
                    }
                    if !cross_entity {
                        continue;
                    }
                    for d in e + 1..a.lists().len() {
                        if p.entities()[d].is_pinned() {
                            continue;
                        }
                        for index in p.entities()[d].pinned_prefix()..a.lists()[d].len() {
                            let mut next = a.lists().to_vec();
                            next[e] = row[..start]
                                .iter()
                                .chain(&a.lists()[d][index..])
                                .copied()
                                .collect();
                            next[d] = a.lists()[d][..index]
                                .iter()
                                .chain(&row[start..])
                                .copied()
                                .collect();
                            if p.validate(&ListAssignment::new(next.clone())).is_ok() {
                                expected.insert(next);
                            }
                        }
                    }
                }
            }
            let moves: Vec<_> = list_two_opt_moves(&p, &a, cross_entity).unwrap().collect();
            assert_eq!(moves.len(), moves.iter().collect::<HashSet<_>>().len());
            assert_eq!(
                moves,
                list_moves(&p, &a, Neighborhood::TwoOpt { cross_entity })
                    .unwrap()
                    .collect::<Vec<_>>()
            );
            let actual: HashSet<_> = moves
                .iter()
                .map(|m| {
                    let mut copy = a.clone();
                    p.apply(&mut copy, m).unwrap().commit();
                    assert!(p.is_complete(&copy));
                    copy.lists().to_vec()
                })
                .collect();
            assert_eq!(actual, expected);
        }
        for reversing in [false, true] {
            let moves: Vec<_> = list_tail_swap_moves(&p, &a, reversing).unwrap().collect();
            for m in &moves {
                let ListMove::SubListSwap {
                    left,
                    right,
                    reverse,
                } = m
                else {
                    panic!("expected tails")
                };
                assert!(left.entity < right.entity);
                assert_eq!(left.end, a.list(left.entity).unwrap().len());
                assert_eq!(right.end, a.list(right.entity).unwrap().len());
                if *reverse {
                    assert!(reversing);
                    assert!(left.end - left.start > 1 || right.end - right.start > 1);
                }
                let mut copy = a.clone();
                p.apply(&mut copy, m).unwrap().commit();
                assert!(p.is_complete(&copy));
            }
            assert_eq!(moves.len(), moves.iter().collect::<HashSet<_>>().len());
            let forwards = moves
                .iter()
                .filter(|m| matches!(m, ListMove::SubListSwap { reverse: false, .. }))
                .count();
            // Each legal forward tail exchange has exactly one reversed partner
            // except when reversing is disabled or both tails have length one.
            let singles=moves.iter().filter(|m| matches!(m,ListMove::SubListSwap {left,right,reverse:false} if left.end-left.start==1 && right.end-right.start==1)).count();
            assert_eq!(
                moves.len(),
                if reversing {
                    2 * forwards - singles
                } else {
                    forwards
                }
            );
        }
    }
    let p = ListProblem::new(vec![0], vec![ListEntity::new(vec![0])]).unwrap();
    let bad = ListAssignment::new(vec![vec![0, 0]]);
    assert!(list_tail_swap_moves(&p, &bad, true).is_err());
    assert!(list_two_opt_moves(&p, &bad, true).is_err());
}
fn weighted(a: &ListAssignment<i32>) -> SimpleScore {
    let mut sum = 0;
    for (e, row) in a.lists().iter().enumerate() {
        for (i, &v) in row.iter().enumerate() {
            sum += i64::from(v) * (e as i64 + 1) * (i as i64 + 1)
                + i64::from(if i == 0 { 0 } else { row[i - 1] })
                - i64::from(*row.get(i + 1).unwrap_or(&0));
        }
    }
    SimpleScore::new(sum)
}
#[test]
fn scoped_and_stream_caches_match_full_score_through_two_opt_trials_and_commits() {
    let p = ListProblem::new(
        (0..6).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3]),
            ListEntity::new(vec![4, 5]),
        ],
    )
    .unwrap();
    let mut n = ListConstraintNetwork::new(2, 6);
    for i in 0..6 {
        n.add_constraint(format!("v{i}"), vec![ListScope::Value(i)], |facts| {
            let ListFact::Value { value, element } = &facts[0] else {
                unreachable!()
            };
            let r = element.as_ref().unwrap();
            SimpleScore::new(
                i64::from(**value) * (r.position.entity as i64 + 1) * (r.position.index as i64 + 1)
                    + i64::from(r.previous.copied().unwrap_or(0))
                    - i64::from(r.next.copied().unwrap_or(0)),
            )
        })
        .unwrap();
    }
    let mut b = StreamGraphBuilder::new();
    let s = ListStreamSources::new(&mut b);
    b.score("positions", s.values.port(), |r| {
        let pos = r.position.unwrap();
        SimpleScore::new(
            i64::from(r.value) * (pos.entity as i64 + 1) * (pos.index as i64 + 1)
                + i64::from(r.previous.unwrap_or(0))
                - i64::from(r.next.unwrap_or(0)),
        )
    })
    .unwrap();
    let mut scoped = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let mut stream =
        ListStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    for round in 0..120 {
        let moves: Vec<_> = list_two_opt_moves(&p, scoped.assignment(), true)
            .unwrap()
            .collect();
        let m = &moves[(round * 37 + 11) % moves.len()];
        let old = scoped.assignment().clone();
        {
            let a = scoped.apply(m).unwrap();
            let b = stream.apply(m).unwrap();
            assert_eq!(a.assignment(), b.assignment());
            assert_eq!(a.score(), weighted(a.assignment()));
            assert_eq!(a.score(), b.score());
        }
        assert_eq!(scoped.assignment(), &old);
        assert_eq!(stream.assignment(), &old);
        if round % 3 != 0 {
            scoped.apply(m).unwrap().commit();
            stream.apply(m).unwrap().commit();
        }
        scoped.assert_consistent().unwrap();
        stream.assert_consistent().unwrap();
    }
}

#[test]
fn two_opt_reversals_escape_list_and_joint_mixed_plateaus_in_both_backends() {
    fn reward(route: &[i32]) -> SimpleScore {
        SimpleScore::new(i64::from(route == [0, 4, 3, 2, 1, 5]))
    }
    let p = ListProblem::new((0..6).collect(), vec![ListEntity::new((0..6).collect())]).unwrap();
    let mut n = ListConstraintNetwork::new(1, 6);
    n.add_constraint("target", vec![ListScope::Entity(0)], |facts| {
        let ListFact::Entity(row) = facts[0] else {
            unreachable!()
        };
        reward(row)
    })
    .unwrap();
    for neighborhood in [
        Neighborhood::ChangeAndSwap,
        Neighborhood::TwoOpt {
            cross_entity: false,
        },
        Neighborhood::TwoOpt { cross_entity: true },
    ] {
        let expected = SimpleScore::new(i64::from(neighborhood != Neighborhood::ChangeAndSwap));
        let config = LocalSearchConfig {
            neighborhood,
            max_evaluations: 100_000,
            max_steps: 2,
            selection: MoveSelection::BestAccepted,
            assert_scores: true,
            ..Default::default()
        };
        let scoped = list_local_search_from(&p, &n, p.initial_assignment(), config).unwrap();
        let mut b = StreamGraphBuilder::new();
        let s = ListStreamSources::new(&mut b);
        b.score("target", s.entities.port(), |(_, row)| reward(row))
            .unwrap();
        let stream =
            list_stream_local_search_from(&p, b.build(), s, p.initial_assignment(), config)
                .unwrap();
        assert_eq!(scoped.best.as_ref().unwrap().score, expected);
        assert_eq!(scoped.best, stream.best);
        assert_eq!(scoped.completed_steps, stream.completed_steps);
        let p = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
            p.clone(),
        );
        let mut n = MixedConstraintNetwork::new(1, 1, 6);
        n.add_constraint(
            "joint",
            vec![MixedScope::Basic(0), MixedScope::ListEntity(0)],
            |facts| {
                let (MixedFact::Basic(v), MixedFact::ListEntity(row)) = (&facts[0], &facts[1])
                else {
                    unreachable!()
                };
                if v.copied() == Some(1) {
                    reward(row)
                } else {
                    SimpleScore::new(0)
                }
            },
        )
        .unwrap();
        let config = MixedLocalSearchConfig {
            search: config,
            joint_moves: true,
        };
        let scoped = mixed_local_search_from(&p, &n, p.initial_assignment(), config).unwrap();
        let mut b = StreamGraphBuilder::new();
        let s = MixedStreamSources::new(&mut b);
        let joined = b
            .join_equal(
                s.basic.port(),
                s.lists.entities.port(),
                |(i, _)| *i,
                |(i, _)| *i,
            )
            .unwrap();
        b.score("joint", &joined, |((_, v), (_, row))| {
            if *v == Some(1) {
                reward(row)
            } else {
                SimpleScore::new(0)
            }
        })
        .unwrap();
        let stream =
            mixed_stream_local_search_from(&p, b.build(), s, p.initial_assignment(), config)
                .unwrap();
        assert_eq!(scoped.best.as_ref().unwrap().score, expected);
        assert_eq!(scoped.best, stream.best);
        assert_eq!(scoped.completed_steps, stream.completed_steps);
    }
}

#[test]
fn two_opt_keeps_basic_swaps_and_does_not_assign_missing_list_values() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(1),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("target", vec![0, 1], |v| {
        SimpleScore::new(i64::from(v == [Some(&1), Some(&0)]))
    })
    .unwrap();
    for cross_entity in [false, true] {
        let config = LocalSearchConfig {
            neighborhood: Neighborhood::TwoOpt { cross_entity },
            max_steps: 1,
            max_evaluations: 100,
            ..Default::default()
        };
        let result = local_search_from(&p, &n, p.initial_assignment(), config).unwrap();
        assert_eq!(result.best.unwrap().score, SimpleScore::new(1));
    }
    let p = ListProblem::new(
        vec![0, 1],
        vec![ListEntity::new(vec![0]), ListEntity::new(vec![])],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    assert_eq!(
        list_moves(&p, &a, Neighborhood::TwoOpt { cross_entity: true })
            .unwrap()
            .count(),
        0
    );
}
