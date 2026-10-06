//! Original integration checks for weighted repair neighborhoods.
use std::cell::Cell;
use timetuck::*;
fn score(v: i64) -> SimpleScore {
    SimpleScore::new(v)
}
fn config<'a>(children: &'a [(RepairNeighborhood<'a>, u32)]) -> RuinRecreateConfig<'a> {
    RuinRecreateConfig {
        additional_neighborhoods: children,
        move_weights: RepairMoveWeights {
            change: 0,
            swap: 0,
            repair: 0,
        },
        max_attempts: 32,
        seed: 53,
        assert_scores: true,
        ..Default::default()
    }
}
#[test]
fn extra_swap_family_preserves_legacy_results_across_all_six_backends() {
    let b = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(1),
    ])
    .unwrap();
    let l = ListProblem::new(
        vec![0, 1],
        vec![ListEntity::new(vec![0]), ListEntity::new(vec![1])],
    )
    .unwrap();
    let m = MixedProblem::new(b.clone(), l.clone());
    let mut bn = ConstraintNetwork::new(2);
    for i in 0..2 {
        bn.add_constraint(format!("target{i}"), vec![i], move |v| {
            score(i64::from(v[0] == Some(&(1 - i))))
        })
        .unwrap();
    }
    let mut ln = ListConstraintNetwork::new(2, 2);
    for i in 0..2 {
        ln.add_constraint(format!("target{i}"), vec![ListScope::Entity(i)], move |v| {
            let ListFact::Entity(l) = v[0] else { panic!() };
            score(i64::from(l == [1 - i]))
        })
        .unwrap();
    }
    let mut mn = MixedConstraintNetwork::new(2, 2, 2);
    for i in 0..2 {
        mn.add_constraint(format!("basic{i}"), vec![MixedScope::Basic(i)], move |v| {
            let MixedFact::Basic(b) = v[0] else { panic!() };
            score(i64::from(b == Some(&(1 - i))))
        })
        .unwrap();
        mn.add_constraint(
            format!("list{i}"),
            vec![MixedScope::ListEntity(i)],
            move |v| {
                let MixedFact::ListEntity(l) = v[0] else {
                    panic!()
                };
                score(i64::from(l == [1 - i]))
            },
        )
        .unwrap();
    }
    let children = [(Neighborhood::Swap.into(), 1)];
    let extra = config(&children);
    let old = RuinRecreateConfig {
        additional_neighborhoods: &[],
        move_weights: RepairMoveWeights {
            change: 0,
            swap: 1,
            repair: 0,
        },
        ..extra.clone()
    };
    let bg = || {
        let mut g = StreamGraphBuilder::new();
        let s = g.source::<usize, (usize, Option<usize>)>();
        g.score("target", s.port(), |(i, v)| {
            score(i64::from(*v == Some(1 - i)))
        })?;
        Ok((g.build(), s))
    };
    let lg = || {
        let mut g = StreamGraphBuilder::new();
        let s = ListStreamSources::new(&mut g);
        g.score(
            "target",
            s.entities.port(),
            |(i, l): &(usize, Vec<usize>)| score(i64::from(l == &[1 - i])),
        )?;
        Ok((g.build(), s))
    };
    let mg = || {
        let mut g = StreamGraphBuilder::new();
        let s = MixedStreamSources::new(&mut g);
        g.score(
            "basic",
            s.basic.port(),
            |(i, v): &(usize, Option<usize>)| score(i64::from(*v == Some(1 - i))),
        )?;
        g.score(
            "list",
            s.lists.entities.port(),
            |(i, l): &(usize, Vec<usize>)| score(i64::from(l == &[1 - i])),
        )?;
        Ok((g.build(), s))
    };
    let br = ruin_recreate_search_from(&b, &bn, b.initial_assignment(), extra.clone()).unwrap();
    assert_eq!(
        br,
        ruin_recreate_search_from(&b, &bn, b.initial_assignment(), old.clone()).unwrap()
    );
    assert_eq!(
        br,
        ruin_recreate_stream_search_from(&b, bg, b.initial_assignment(), extra.clone()).unwrap()
    );
    let lr =
        ruin_recreate_list_search_from(&l, &ln, l.initial_assignment(), extra.clone()).unwrap();
    assert_eq!(
        lr,
        ruin_recreate_list_search_from(&l, &ln, l.initial_assignment(), old.clone()).unwrap()
    );
    assert_eq!(
        lr,
        ruin_recreate_list_stream_search_from(&l, lg, l.initial_assignment(), extra.clone())
            .unwrap()
    );
    let mc = |search| MixedRuinRecreateConfig {
        search,
        ..Default::default()
    };
    let mr = ruin_recreate_mixed_search_from(&m, &mn, m.initial_assignment(), mc(extra.clone()))
        .unwrap();
    assert_eq!(
        mr,
        ruin_recreate_mixed_search_from(&m, &mn, m.initial_assignment(), mc(old)).unwrap()
    );
    assert_eq!(
        mr,
        ruin_recreate_mixed_stream_search_from(&m, mg, m.initial_assignment(), mc(extra)).unwrap()
    );
    assert_eq!(br.best.unwrap().score, score(2));
    assert_eq!(lr.best.unwrap().score, score(2));
    assert_eq!(mr.best.unwrap().score, score(4));
}
#[test]
fn rich_list_families_produce_legal_successors_and_share_repair_budgets() {
    let p = ListProblem::from_entity_ranges(vec![
        ListEntity::new(vec![0, 1, 2, 3, 4, 5])
            .with_pinned_prefix(1)
            .with_value_range((0..8).collect()),
        ListEntity::new(vec![6, 7]).with_value_range((1..8).collect()),
    ])
    .unwrap();
    let initial = p.initial_assignment();
    let mut n = ListConstraintNetwork::new(2, 8);
    n.add_constraint(
        "changed",
        vec![ListScope::Entity(0), ListScope::Entity(1)],
        |v| {
            let ListFact::Entity(a) = v[0] else { panic!() };
            let ListFact::Entity(b) = v[1] else { panic!() };
            score(i64::from(a != [0, 1, 2, 3, 4, 5] || b != [6, 7]))
        },
    )
    .unwrap();
    for family in [
        Neighborhood::SubListChange(SubListChangeConfig::default()),
        Neighborhood::SubListSwap(SubListSwapConfig::default()),
        Neighborhood::KOpt(ListKOptConfig::new(2, 3).unwrap()),
        Neighborhood::SampledKOpt(ListKOptSampling::new(
            ListKOptConfig::new(2, 3).unwrap(),
            17,
            32,
        )),
        Neighborhood::RepartitionKOpt(ListKOptSampling::new(
            ListKOptConfig::new(2, 3).unwrap(),
            17,
            32,
        )),
    ] {
        let successors = list_moves(&p, &initial, family)
            .unwrap()
            .map(|m| {
                let mut a = initial.clone();
                p.apply(&mut a, &m).unwrap().commit();
                a
            })
            .collect::<Vec<_>>();
        assert!(!successors.is_empty(), "{family:?}");
        let children = [(family.into(), u32::MAX)];
        let c = RuinRecreateConfig {
            max_attempts: 1,
            ..config(&children)
        };
        let r = ruin_recreate_list_search_from(&p, &n, initial.clone(), c.clone()).unwrap();
        assert_eq!(
            r,
            ruin_recreate_list_search_from(&p, &n, initial.clone(), c).unwrap()
        );
        assert_eq!(r.fine_move_attempts, 1);
        assert_eq!(r.fine_move_evaluations, 1);
        assert_eq!(r.evaluations, 2);
        let best = r.best.unwrap();
        assert_eq!(best.score, score(1));
        assert!(successors.contains(&best.assignment));
        p.validate(&best.assignment).unwrap();
    }
    let children = [
        (
            Neighborhood::SubListSwap(SubListSwapConfig::default()).into(),
            3,
        ),
        (Neighborhood::Swap.into(), 0),
    ];
    for budget in 0..24 {
        let c = RuinRecreateConfig {
            move_weights: RepairMoveWeights {
                change: 1,
                swap: 1,
                repair: 2,
            },
            minimum_ruined: 2,
            maximum_ruined: 3,
            max_move_evaluations: Some(budget),
            ..config(&children)
        };
        let r = ruin_recreate_list_search_from(&p, &n, initial.clone(), c.clone()).unwrap();
        assert_eq!(
            r,
            ruin_recreate_list_search_from(&p, &n, initial.clone(), c).unwrap()
        );
        assert!(r.repair_move_evaluations + r.fine_move_evaluations <= budget);
        p.validate(&r.best.unwrap().assignment).unwrap();
        if budget == 23 {
            assert!(r.attempts > 0 && r.fine_move_attempts > 0);
        }
    }
    assert_eq!(p.initial_assignment(), initial);
}
#[test]
fn disabled_invalid_and_empty_families_obey_validation_and_fallback_rules() {
    let p = ListProblem::new(vec![0], vec![ListEntity::new(vec![0]).with_pinned(true)]).unwrap();
    let calls = Cell::new(0);
    let mut n = ListConstraintNetwork::new(1, 1);
    n.add_constraint("count", vec![ListScope::Entity(0)], |_| {
        calls.set(calls.get() + 1);
        score(0)
    })
    .unwrap();
    let invalid =
        Neighborhood::SubListChange(SubListChangeConfig::default().with_unassignment(true));
    let enabled = [(invalid.into(), 1)];
    let c = config(&enabled);
    assert!(ruin_recreate_list_search_from(&p, &n, p.initial_assignment(), c.clone()).is_err());
    assert!(
        solve_list(
            &p,
            &n,
            SolverSequence {
                limits: Default::default(),
                phases: vec![
                    SolverPhase::Construction(Default::default()),
                    SolverPhase::RuinRecreate(c)
                ]
            }
        )
        .is_err()
    );
    assert_eq!(calls.get(), 0);
    let disabled = [(invalid.into(), 0), (Neighborhood::Swap.into(), 1)];
    let r =
        ruin_recreate_list_search_from(&p, &n, p.initial_assignment(), config(&disabled)).unwrap();
    assert_eq!(r.termination, RuinRecreateTermination::NoAvailableMoves);
    assert_eq!(r.fine_move_attempts, 0);
    let b = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let mut bn = ConstraintNetwork::new(1);
    bn.add_constraint("value", vec![0], |v| score(i64::from(v[0] == Some(&1))))
        .unwrap();
    let children = [
        (Neighborhood::Swap.into(), u32::MAX),
        (Neighborhood::Change.into(), 1),
    ];
    let r = ruin_recreate_search_from(
        &b,
        &bn,
        b.initial_assignment(),
        RuinRecreateConfig {
            max_attempts: 1,
            ..config(&children)
        },
    )
    .unwrap();
    assert_eq!(r.best.unwrap().score, score(1));
    assert_eq!(r.fine_move_attempts, 1);
}
