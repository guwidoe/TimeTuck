//! Original transactional and nullable list aggregate regressions; no method credit.
#[path = "../examples/list_element_support/mod.rs"]
mod support;
use timetuck::*;
#[test]
fn ordered_chains_restore_all_values_and_empty_owner_fallback() {
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![2])],
    )
    .unwrap()
    .allowing_unassigned();
    let (g, s, c, v) = support::ordered(vec![10, 30], vec![2, 7, 3]);
    let mut d = ListStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    let before = d.graph().constraint_matches(&v).unwrap().clone();
    assert_eq!(before[&0].0.start, Some(10));
    assert_eq!(before[&1].0.start, Some(12));
    assert_eq!(before[&1].0.end, Some(19));
    assert_eq!(d.score(), SimpleScore::new(-52));
    {
        let t = d
            .apply(&ListMove::Composite(vec![
                ListMove::Unassign {
                    entity: 0,
                    start: 0,
                    end: 2,
                },
                ListMove::Assign {
                    value: 1,
                    destination: ListPosition {
                        entity: 1,
                        index: 0,
                    },
                },
            ]))
            .unwrap();
        let times = t.graph().constraint_matches(&v).unwrap();
        assert_eq!(
            times[&0].0,
            support::Times {
                owner: None,
                previous: None,
                start: None,
                end: None
            }
        );
        assert_eq!(times[&1].0.end, Some(37));
        assert_eq!(times[&2].0.start, Some(37));
        assert_eq!(times[&2].0.previous, Some(1));
        assert_eq!(t.graph().constraint_matches(&c).unwrap()[&0].0.1, 10);
        assert_eq!(t.score(), SimpleScore::new(-150));
    }
    assert_eq!(d.graph().constraint_matches(&v).unwrap(), &before);
    assert_eq!(d.score(), SimpleScore::new(-52));
    d.assert_consistent().unwrap();
}
#[test]
fn null_member_poison_is_distinct_from_empty_and_survives_transfer_undo() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![2, 5]).allowing_unassigned(),
            Variable::new(vec![3]).allowing_unassigned().with_initial(3),
        ])
        .unwrap(),
        ListProblem::new(
            vec![0, 1],
            vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![])],
        )
        .unwrap(),
    );
    let (g, s, c, v) = support::mixed();
    let mut d = MixedStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    let totals = d.graph().constraint_matches(&c).unwrap().clone();
    assert_eq!(totals[&0].0.1, None);
    assert_eq!(totals[&1].0.1, Some(0));
    {
        let t = d
            .apply(&MixedMove {
                basic: Move::Change {
                    variable: 0,
                    value: Some(5),
                },
                lists: ListMove::Change {
                    source: ListPosition {
                        entity: 0,
                        index: 1,
                    },
                    destination: ListPosition {
                        entity: 1,
                        index: 0,
                    },
                },
            })
            .unwrap();
        let totals = t.graph().constraint_matches(&c).unwrap();
        assert_eq!(totals[&0].0.1, Some(6));
        assert_eq!(totals[&1].0.1, Some(4));
        assert_eq!(t.graph().constraint_matches(&v).unwrap()[&0].0.1, Some(6));
    }
    assert_eq!(d.graph().constraint_matches(&c).unwrap(), &totals);
    d.apply(&MixedMove {
        basic: Move::Composite(vec![]),
        lists: ListMove::Change {
            source: ListPosition {
                entity: 0,
                index: 0,
            },
            destination: ListPosition {
                entity: 1,
                index: 0,
            },
        },
    })
    .unwrap()
    .commit();
    let totals = d.graph().constraint_matches(&c).unwrap().clone();
    assert_eq!(totals[&0].0.1, Some(4));
    assert_eq!(totals[&1].0.1, None);
    d.assert_consistent().unwrap();
}
#[test]
fn bounded_optional_list_search_agrees_with_independent_full_score() {
    let p = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![ListEntity::new(vec![2, 0]), ListEntity::new(vec![])],
    )
    .unwrap()
    .allowing_unassigned();
    let starts = [4, 11];
    let durations = [3, 7, 2, 5];
    let mut n = ListConstraintNetwork::new(2, 4);
    n.add_constraint(
        "independent",
        vec![ListScope::Entity(0), ListScope::Entity(1)],
        |facts| {
            let mut missing = 4;
            let mut total = 0_i64;
            for (e, f) in facts.iter().enumerate() {
                let ListFact::Entity(list) = f else {
                    unreachable!()
                };
                missing -= list.len();
                total += i64::from(starts[e])
                    + list.iter().map(|&v| i64::from(durations[v])).sum::<i64>();
            }
            SimpleScore::new(-total - 100 * missing as i64)
        },
    )
    .unwrap();
    for cap in [1, 7, 30, 100] {
        let config = SolverConfig {
            limits: SolverLimits {
                max_move_evaluations: Some(cap),
                ..Default::default()
            },
            construction: ConstructionConfig {
                assert_scores: true,
                ..Default::default()
            },
            local_search: vec![LocalSearchConfig {
                assert_scores: true,
                ..Default::default()
            }],
        };
        let expected = solve_list(&p, &n, config.clone()).unwrap();
        let actual = solve_list_stream(
            &p,
            || {
                let (g, s, _, _) = support::ordered(starts.to_vec(), durations.to_vec());
                Ok((g, s))
            },
            config,
        )
        .unwrap();
        assert_eq!(actual.assignment, expected.assignment);
        assert_eq!(actual.score, expected.score);
        assert_eq!(actual.work.move_evaluations, expected.work.move_evaluations);
    }
}
