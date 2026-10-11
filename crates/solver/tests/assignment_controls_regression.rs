//! Original policy, schema and end-to-end checks; no upstream method credit.
use std::collections::{BTreeMap, BTreeSet};
use timetuck::*;
#[test]
fn policies_match_independent_successor_sets_and_rollback() {
    for code in 0..27 {
        for optional in [false, true] {
            for pinned in [0, 1, 4] {
                let mut k = code;
                let mut initial = vec![];
                let vars = (0..3)
                    .map(|i| {
                        let x = match k % 3 {
                            0 => None,
                            1 => Some(0),
                            _ => Some(1),
                        };
                        k /= 3;
                        initial.push(x);
                        let mut v = Variable::new(vec![0, 1]);
                        if optional {
                            v = v.allowing_unassigned();
                        }
                        if let Some(x) = x {
                            v = v.with_initial(x);
                        }
                        v.with_pinned(pinned & (1 << i) != 0)
                    })
                    .collect();
                let Ok(p) = FiniteProblem::new(vars) else {
                    continue;
                };
                let a = p.initial_assignment();
                let mut groups: BTreeMap<Option<i32>, Vec<usize>> = BTreeMap::new();
                for (i, &v) in initial.iter().enumerate() {
                    if pinned & (1 << i) == 0 {
                        groups.entry(v).or_default().push(i);
                    }
                }
                for mode in 0..4 {
                    if !optional && mode != 2 {
                        continue;
                    }
                    for cross in [None, Some(false), Some(true)] {
                        if !optional && cross == Some(true) {
                            continue;
                        }
                        let mut c = PillarChangeConfig::default();
                        if let Some(x) = cross {
                            c = c.with_crossing_null(x);
                        }
                        let mut expected = BTreeSet::new();
                        if mode < 2 {
                            for (i, &v) in initial.iter().enumerate() {
                                if pinned & (1 << i) != 0 {
                                    continue;
                                }
                                for to in [None, Some(0), Some(1)] {
                                    if (mode == 0 && v.is_none() && to.is_some())
                                        || (mode == 1 && v.is_some() && to.is_none())
                                    {
                                        let mut next = initial.clone();
                                        next[i] = to;
                                        expected.insert(next);
                                    }
                                }
                            }
                        } else {
                            for (from, ids) in &groups {
                                if mode == 2 && ids.len() < 2 {
                                    continue;
                                }
                                for to in [None, Some(0), Some(1)] {
                                    if to == *from
                                        || (mode == 3 && (from.is_none() || to.is_some()))
                                        || (mode == 2
                                            && ((!optional || cross == Some(false))
                                                && (from.is_none() || to.is_none())))
                                    {
                                        continue;
                                    }
                                    let mut next = initial.clone();
                                    for &i in ids {
                                        next[i] = to;
                                    }
                                    expected.insert(next);
                                }
                            }
                        }
                        let moves: Box<dyn Iterator<Item = Move<i32>>> = match mode {
                            0 => Box::new(assign_moves(&p, &a).unwrap()),
                            1 => Box::new(unassign_moves(&p, &a).unwrap()),
                            2 => Box::new(pillar_change_moves(&p, &a, c).unwrap()),
                            _ => Box::new(pillar_unassign_moves(&p, &a).unwrap()),
                        };
                        let mut actual = BTreeSet::new();
                        let mut scratch = a.clone();
                        for m in moves {
                            {
                                let tx = p.apply(&mut scratch, &m).unwrap();
                                assert!(actual.insert(tx.values().to_vec()), "duplicate successor");
                            }
                            assert_eq!(scratch, a);
                        }
                        assert_eq!(
                            actual, expected,
                            "state={initial:?}, optional={optional}, pins={pinned}, mode={mode}, cross={cross:?}"
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn schema_groups_keep_columns_separate_and_use_member_count() {
    let vars = [0, 1, 0, 1, 1, 0]
        .map(|x| {
            Variable::new(vec![0, 1])
                .allowing_unassigned()
                .with_initial(x)
        })
        .to_vec();
    let p = FiniteProblem::new(vars.clone())
        .unwrap()
        .with_pillar_schema(PillarSchema::new(vec![vec![0, 1], vec![2, 3], vec![4, 5]]).unwrap())
        .unwrap();
    let a = p.initial_assignment();
    let c = PillarChangeConfig::default().with_crossing_null(false);
    let moves: Vec<_> = pillar_change_moves(&p, &a, c).unwrap().collect();
    assert_eq!(moves.len(), 2);
    let touched: BTreeSet<_> = moves.iter().map(Move::touched_variables).collect();
    assert_eq!(touched, BTreeSet::from([vec![0, 2], vec![1, 3]]));
    let removals: Vec<_> = pillar_unassign_moves(&p, &a).unwrap().collect();
    assert_eq!(removals.len(), 4);
    let mut scratch = a.clone();
    for m in removals {
        let ids = m.touched_variables();
        {
            let tx = p.apply(&mut scratch, &m).unwrap();
            for &i in &ids {
                assert_eq!(tx.values()[i], None);
            }
        }
        assert_eq!(scratch, a);
    }
    let mut vars = vars;
    vars[0] = vars[0].clone().with_pinned(true);
    let pinned = FiniteProblem::new(vars)
        .unwrap()
        .with_pillar_schema(p.pillar_schema().unwrap().clone())
        .unwrap();
    assert_eq!(
        pillar_change_moves(&pinned, &pinned.initial_assignment(), c)
            .unwrap()
            .count(),
        0
    );
    assert!(
        pillar_unassign_moves(&pinned, &pinned.initial_assignment())
            .unwrap()
            .all(|m| !m.touched_variables().iter().any(|i| [0, 1].contains(i)))
    );
}
#[test]
fn policies_solve_through_basic_and_mixed_scoped_and_stream_backends() {
    for (mode, neighborhood) in [
        Neighborhood::Assign,
        Neighborhood::Unassign,
        Neighborhood::PillarChange(PillarChangeConfig::default()),
        Neighborhood::PillarUnassign,
    ]
    .into_iter()
    .enumerate()
    {
        let target = if mode % 2 == 0 { Some(1) } else { None };
        let mut var = Variable::new(vec![0, 1]).allowing_unassigned();
        if mode != 0 {
            var = var.with_initial(if mode == 2 { 0 } else { 1 });
        }
        let p = FiniteProblem::new(vec![var; 2]).unwrap();
        let mut n = ConstraintNetwork::new(2);
        n.add_constraint("target", vec![0, 1], |v| {
            SimpleScore::new(v.iter().filter(|x| x.copied() == target).count() as i64)
        })
        .unwrap();
        let config = LocalSearchConfig {
            neighborhood,
            max_steps: 2,
            assert_scores: true,
            ..Default::default()
        };
        assert_eq!(
            local_search_from(&p, &n, p.initial_assignment(), config.clone())
                .unwrap()
                .best
                .unwrap()
                .score,
            SimpleScore::new(2)
        );
        let mut b = StreamGraphBuilder::new();
        let source: PlanningStreamSource<i32> = b.source();
        b.score("target", source.port(), |(_, v)| {
            SimpleScore::new(i64::from(*v == target))
        })
        .unwrap();
        assert_eq!(
            stream_local_search_from(
                &p,
                b.build(),
                source,
                p.initial_assignment(),
                config.clone()
            )
            .unwrap()
            .best
            .unwrap()
            .score,
            SimpleScore::new(2)
        );
        let p = MixedProblem::new(
            p,
            ListProblem::new(
                vec![7],
                vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
            )
            .unwrap(),
        );
        let mut n = MixedConstraintNetwork::new(2, 2, 1);
        n.add_constraint(
            "target",
            vec![MixedScope::Basic(0), MixedScope::Basic(1)],
            |v| {
                SimpleScore::new(
                    v.iter()
                        .filter(|f| matches!(f,MixedFact::Basic(x) if x.copied()==target))
                        .count() as i64,
                )
            },
        )
        .unwrap();
        for joint_moves in [false, true] {
            let c = MixedLocalSearchConfig {
                search: config.clone(),
                joint_moves,
            };
            assert_eq!(
                mixed_local_search_from(&p, &n, p.initial_assignment(), c.clone())
                    .unwrap()
                    .best
                    .unwrap()
                    .score,
                SimpleScore::new(2)
            );
            let mut b = StreamGraphBuilder::new();
            let sources = MixedStreamSources::<i32, i32>::new(&mut b);
            b.score("target", sources.basic.port(), |(_, v)| {
                SimpleScore::new(i64::from(*v == target))
            })
            .unwrap();
            assert_eq!(
                mixed_stream_local_search_from(&p, b.build(), sources, p.initial_assignment(), c)
                    .unwrap()
                    .best
                    .unwrap()
                    .score,
                SimpleScore::new(2)
            );
        }
    }
}
#[test]
fn invalid_optional_policies_fail_before_scoring_and_factories() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::<_, SimpleScore>::new(1);
    n.add_constraint("never", vec![0], |_| panic!("invalid policy scored"))
        .unwrap();
    let mixed = MixedProblem::new(p.clone(), ListProblem::<i32>::new(vec![], vec![]).unwrap());
    let mut mn = MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 0, 0);
    mn.add_constraint("never", vec![MixedScope::Basic(0)], |_| {
        panic!("invalid policy scored")
    })
    .unwrap();
    for neighborhood in [
        Neighborhood::Assign,
        Neighborhood::Unassign,
        Neighborhood::PillarUnassign,
        Neighborhood::PillarChange(PillarChangeConfig::default().with_crossing_null(true)),
    ] {
        let c = LocalSearchConfig {
            neighborhood,
            max_evaluations: 0,
            ..Default::default()
        };
        let plan = SolverSequence {
            phases: vec![SolverPhase::LocalSearch(c.clone())],
            ..Default::default()
        };
        assert!(matches!(
            solve(&p, &n, plan.clone()),
            Err(SolverError::Phase { phase: 0, .. })
        ));
        assert!(matches!(
            solve_stream(&p, || panic!("invalid policy built graph"), plan),
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
