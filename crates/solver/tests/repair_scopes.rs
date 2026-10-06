//! Original repair-pool, component-family and shared-budget regressions.
use std::cell::Cell;
use timetuck::*;
fn problem() -> MixedProblem<i32, i32> {
    MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 2]).unwrap(),
        ListProblem::new(
            vec![10, 20],
            vec![ListEntity::new(vec![10, 20]), ListEntity::new(vec![])],
        )
        .unwrap(),
    )
}
fn network() -> MixedConstraintNetwork<'static, i32, i32, SimpleScore> {
    let mut n = MixedConstraintNetwork::new(2, 2, 2);
    for i in 0..2 {
        n.add_constraint(format!("basic{i}"), vec![MixedScope::Basic(i)], |v| {
            let MixedFact::Basic(b) = v[0] else { panic!() };
            SimpleScore::new(i64::from(b == Some(&1)))
        })
        .unwrap();
        n.add_constraint(format!("list{i}"), vec![MixedScope::ListValue(i)], |v| {
            let MixedFact::ListValue { element, .. } = v[0] else {
                panic!()
            };
            SimpleScore::new(i64::from(
                element.as_ref().is_some_and(|e| e.position.entity == 1),
            ))
        })
        .unwrap();
    }
    n
}
fn graph() -> Result<
    (
        StreamGraph<'static, SimpleScore>,
        MixedStreamSources<i32, i32>,
    ),
    GraphError,
> {
    let mut g = StreamGraphBuilder::new();
    let sources = MixedStreamSources::new(&mut g);
    g.score("basic", sources.basic.port(), |(_, v)| {
        SimpleScore::new(i64::from(*v == Some(1)))
    })?;
    g.score("list", sources.lists.values.port(), |v| {
        SimpleScore::new(i64::from(v.position.is_some_and(|at| at.entity == 1)))
    })?;
    Ok((g.build(), sources))
}
fn config(indices: &[usize]) -> MixedRuinRecreateConfig<'_> {
    MixedRuinRecreateConfig {
        search: RuinRecreateConfig {
            ruin_candidates: Some(indices),
            minimum_ruined: usize::MAX,
            maximum_ruined: usize::MAX,
            max_attempts: 1,
            assert_scores: true,
            ..Default::default()
        },
        ..Default::default()
    }
}
#[test]
fn all_mixed_subsets_preserve_unselected_identities_and_match_both_backends() {
    let p = problem();
    let n = network();
    for mask in 0..16 {
        let indices = (0..4).filter(|i| mask & (1 << i) != 0).collect::<Vec<_>>();
        let a = ruin_recreate_mixed_search_from(&p, &n, p.initial_assignment(), config(&indices))
            .unwrap();
        let b = ruin_recreate_mixed_stream_search_from(
            &p,
            graph,
            p.initial_assignment(),
            config(&indices),
        )
        .unwrap();
        assert_eq!(a, b);
        let best = a.best.unwrap();
        for i in 0..2 {
            assert_eq!(
                best.assignment.basic().values()[i],
                Some(i32::from(mask & (1 << i) != 0))
            );
        }
        for (i, value) in [10, 20].iter().enumerate() {
            assert_eq!(
                best.assignment.lists().location(value).unwrap().entity,
                usize::from(mask & (1 << (i + 2)) != 0)
            );
        }
        assert_eq!(best.score, SimpleScore::new(indices.len() as i64));
        let mut reversed = indices.clone();
        reversed.reverse();
        assert_eq!(
            b,
            ruin_recreate_mixed_search_from(&p, &n, p.initial_assignment(), config(&reversed))
                .unwrap()
        );
        for budget in 0..8 {
            let mut c = config(&indices);
            c.search.max_move_evaluations = Some(budget);
            let result =
                ruin_recreate_mixed_search_from(&p, &n, p.initial_assignment(), c).unwrap();
            assert!(result.repair_move_evaluations <= budget);
            p.validate(&result.best.unwrap().assignment).unwrap();
        }
    }
    assert_eq!(p.initial_assignment(), problem().initial_assignment());
}
#[test]
fn weighted_basic_and_list_pools_never_ruin_across_scopes_in_one_attempt() {
    let p = problem();
    let calls = Cell::new(0);
    let mut n = MixedConstraintNetwork::new(2, 2, 2);
    n.add_constraint(
        "all",
        vec![
            MixedScope::Basic(0),
            MixedScope::Basic(1),
            MixedScope::ListEntity(0),
            MixedScope::ListEntity(1),
        ],
        |f| {
            calls.set(calls.get() + 1);
            let MixedFact::Basic(first) = f[0] else {
                panic!()
            };
            let MixedFact::Basic(second) = f[1] else {
                panic!()
            };
            let MixedFact::ListEntity(a) = f[2] else {
                panic!()
            };
            let MixedFact::ListEntity(b) = f[3] else {
                panic!()
            };
            assert_eq!(second, Some(&0));
            assert!(
                first.is_some() || a.len() + b.len() == 2,
                "one repair must not cross basic/list pools"
            );
            SimpleScore::new(i64::from(first == Some(&1)) + b.len() as i64)
        },
    )
    .unwrap();
    let pools = [
        RepairPool {
            candidates: &[0],
            weight: 1,
        },
        RepairPool {
            candidates: &[2, 3],
            weight: 1,
        },
        RepairPool {
            candidates: &[usize::MAX],
            weight: 0,
        },
    ];
    let families = [
        (
            RepairNeighborhood::Mixed(MixedNeighborhood {
                pillar_schema: None,
                basic: Some(Neighborhood::Change),
                lists: None,
                basic_variables: Some(&[0]),
                joint_moves: false,
            }),
            1,
        ),
        (
            RepairNeighborhood::Mixed(MixedNeighborhood {
                basic: None,
                lists: Some(Neighborhood::Change),
                ..Default::default()
            }),
            1,
        ),
    ];
    for seed in 0..16 {
        let c = MixedRuinRecreateConfig {
            search: RuinRecreateConfig {
                repair_pools: Some(&pools),
                additional_neighborhoods: &families,
                seed,
                max_attempts: 32,
                minimum_ruined: usize::MAX,
                maximum_ruined: usize::MAX,
                max_move_evaluations: Some(100),
                assert_scores: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let result =
            ruin_recreate_mixed_search_from(&p, &n, p.initial_assignment(), c.clone()).unwrap();
        assert!(result.attempts > 0 && result.fine_move_attempts > 0);
        assert!(result.repair_move_evaluations + result.fine_move_evaluations <= 100);
        assert_eq!(result.best.as_ref().unwrap().score, SimpleScore::new(3));
        assert_eq!(
            result,
            ruin_recreate_mixed_search_from(&p, &n, p.initial_assignment(), c).unwrap()
        );
    }
    assert!(calls.get() > 0);
}
#[test]
fn invalid_scope_or_component_is_rejected_before_solver_scoring() {
    let p = problem();
    let called = Cell::new(0);
    let mut n = MixedConstraintNetwork::new(2, 2, 2);
    n.add_constraint("called", vec![MixedScope::Basic(0)], |_| {
        called.set(called.get() + 1);
        SimpleScore::new(0)
    })
    .unwrap();
    for ids in [&[4][..], &[0, 0][..]] {
        let c = config(ids);
        assert!(
            ruin_recreate_mixed_search_from(&p, &n, p.initial_assignment(), c.clone()).is_err()
        );
        let plan = MixedSolverSequence {
            phases: vec![
                MixedSolverPhase::Construction(Default::default()),
                MixedSolverPhase::RuinRecreate(c),
            ],
            ..Default::default()
        };
        assert!(solve_mixed(&p, &n, plan.clone()).is_err());
        assert!(
            solve_mixed_stream(
                &p,
                || {
                    called.set(called.get() + 1);
                    graph()
                },
                plan
            )
            .is_err()
        );
    }
    assert_eq!(called.get(), 0);
    let extra = [(RepairNeighborhood::Mixed(MixedNeighborhood::default()), 1)];
    let bn = ConstraintNetwork::<_, SimpleScore>::new(2);
    let ln = ListConstraintNetwork::<_, SimpleScore>::new(2, 2);
    let c = RuinRecreateConfig {
        additional_neighborhoods: &extra,
        ..Default::default()
    };
    assert!(matches!(
        ruin_recreate_search_from(p.basic(), &bn, p.basic().initial_assignment(), c.clone()),
        Err(RuinRecreateError::MixedFamilyRequiresMixedModel)
    ));
    assert!(matches!(
        ruin_recreate_list_search_from(p.lists(), &ln, p.lists().initial_assignment(), c),
        Err(RuinRecreateError::MixedFamilyRequiresMixedModel)
    ));
}
#[test]
fn explicit_plans_keep_scopes_and_global_budgets_across_phase_handoffs() {
    let p = problem();
    let n = network();
    let first = config(&[0]);
    let second = config(&[2, 3]);
    for budget in 0..16 {
        let plan = MixedSolverSequence {
            limits: SolverLimits {
                max_move_evaluations: Some(budget),
                ..Default::default()
            },
            phases: vec![
                MixedSolverPhase::Construction(Default::default()),
                MixedSolverPhase::RuinRecreate(first.clone()),
                MixedSolverPhase::RuinRecreate(second.clone()),
            ],
        };
        let a = solve_mixed(&p, &n, plan.clone()).unwrap();
        let b = solve_mixed_stream(&p, graph, plan).unwrap();
        assert_eq!(a.assignment, b.assignment);
        assert_eq!(a.score, b.score);
        assert_eq!(a.work.move_evaluations, b.work.move_evaluations);
        assert!(a.work.move_evaluations <= budget);
        assert_eq!(a.assignment.basic().values()[1], Some(0));
    }
}

#[test]
fn standalone_scopes_use_variable_or_registry_indices_in_both_backends() {
    let b = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 2]).unwrap();
    let mut bn = ConstraintNetwork::new(2);
    for i in 0..2 {
        bn.add_constraint(format!("basic{i}"), vec![i], |v| {
            SimpleScore::new(i64::from(v[0] == Some(&1)))
        })
        .unwrap();
    }
    let c = RuinRecreateConfig {
        ruin_candidates: Some(&[1]),
        minimum_ruined: usize::MAX,
        maximum_ruined: usize::MAX,
        max_attempts: 1,
        assert_scores: true,
        ..Default::default()
    };
    let a = ruin_recreate_search_from(&b, &bn, b.initial_assignment(), c.clone()).unwrap();
    let s = ruin_recreate_stream_search_from(
        &b,
        || {
            let mut g = StreamGraphBuilder::new();
            let s = g.source::<usize, (usize, Option<i32>)>();
            g.score("basic", s.port(), |(_, v)| {
                SimpleScore::new(i64::from(*v == Some(1)))
            })?;
            Ok((g.build(), s))
        },
        b.initial_assignment(),
        c.clone(),
    )
    .unwrap();
    assert_eq!(a, s);
    assert_eq!(a.best.unwrap().assignment.values(), &[Some(0), Some(1)]);
    let l = ListProblem::new(
        vec![10, 20],
        vec![ListEntity::new(vec![10, 20]), ListEntity::new(vec![])],
    )
    .unwrap();
    let mut ln = ListConstraintNetwork::new(2, 2);
    for i in 0..2 {
        ln.add_constraint(format!("list{i}"), vec![ListScope::Value(i)], |v| {
            let ListFact::Value { element, .. } = v[0] else {
                panic!()
            };
            SimpleScore::new(i64::from(
                element.as_ref().is_some_and(|e| e.position.entity == 1),
            ))
        })
        .unwrap();
    }
    let a = ruin_recreate_list_search_from(&l, &ln, l.initial_assignment(), c.clone()).unwrap();
    let s = ruin_recreate_list_stream_search_from(
        &l,
        || {
            let mut g = StreamGraphBuilder::new();
            let s = ListStreamSources::<i32>::new(&mut g);
            g.score("list", s.values.port(), |v| {
                SimpleScore::new(i64::from(v.position.is_some_and(|p| p.entity == 1)))
            })?;
            Ok((g.build(), s))
        },
        l.initial_assignment(),
        c,
    )
    .unwrap();
    assert_eq!(a, s);
    assert_eq!(a.best.unwrap().assignment.lists(), &[vec![10], vec![20]]);
}
#[test]
fn empty_disabled_and_intersected_pools_preserve_eligibility() {
    let p = problem();
    let n = network();
    let empty = [];
    let disabled = [RepairPool {
        candidates: &[usize::MAX],
        weight: 0,
    }];
    for pools in [&empty[..], &disabled[..]] {
        let c = MixedRuinRecreateConfig {
            search: RuinRecreateConfig {
                repair_pools: Some(pools),
                ..Default::default()
            },
            ..Default::default()
        };
        let r = ruin_recreate_mixed_search_from(&p, &n, p.initial_assignment(), c).unwrap();
        assert_eq!(r.termination, RuinRecreateTermination::NoAvailableMoves);
        assert_eq!(r.attempts, 0);
    }
    let pools = [RepairPool {
        candidates: &[0, 1, 2, 3],
        weight: 1,
    }];
    let mut c = config(&[2]);
    c.search.repair_pools = Some(&pools);
    let r = ruin_recreate_mixed_search_from(&p, &n, p.initial_assignment(), c).unwrap();
    let a = r.best.unwrap().assignment;
    assert_eq!(a.basic(), p.initial_assignment().basic());
    assert_eq!(a.lists().lists(), &[vec![20], vec![10]]);
    for bad in [&[0, 0][..], &[4][..]] {
        let pools = [RepairPool {
            candidates: bad,
            weight: 1,
        }];
        let c = MixedRuinRecreateConfig {
            search: RuinRecreateConfig {
                repair_pools: Some(&pools),
                max_evaluations: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(ruin_recreate_mixed_search_from(&p, &n, p.initial_assignment(), c).is_err());
    }
}

#[test]
fn maximum_pool_weights_use_wide_totals_without_merging_repair_scopes() {
    let p = problem();
    let n = network();
    let pools = [
        RepairPool {
            candidates: &[0],
            weight: u32::MAX,
        },
        RepairPool {
            candidates: &[2, 3],
            weight: u32::MAX,
        },
    ];
    let mut seen = [false; 2];
    for seed in 0..32 {
        let c = MixedRuinRecreateConfig {
            search: RuinRecreateConfig {
                repair_pools: Some(&pools),
                move_weights: RepairMoveWeights {
                    change: 0,
                    swap: 0,
                    repair: u32::MAX,
                },
                max_attempts: 1,
                minimum_ruined: usize::MAX,
                maximum_ruined: usize::MAX,
                seed,
                ..Default::default()
            },
            ..Default::default()
        };
        let r = ruin_recreate_mixed_search_from(&p, &n, p.initial_assignment(), c).unwrap();
        assert_eq!(r.attempts, 1);
        let best = r.best.unwrap();
        if best.assignment.basic().values()[0] == Some(1) {
            assert_eq!(best.assignment.lists(), p.initial_assignment().lists());
            seen[0] = true;
        } else {
            assert_eq!(best.assignment.basic(), p.initial_assignment().basic());
            let mut assigned = best.assignment.lists().lists()[1].clone();
            assigned.sort();
            assert_eq!(assigned, vec![10, 20]);
            p.validate(&best.assignment).unwrap();
            seen[1] = true;
        }
    }
    assert_eq!(seen, [true, true]);
}
