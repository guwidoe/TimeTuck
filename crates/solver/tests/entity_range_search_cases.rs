// Adapted from pinned Timefold Community DefaultSolverTest and list range fixtures.
// Apache-2.0; exact upstream methods and native substitutions in compat/provenance.json.
use timetuck::*;

#[test]
fn solve_list_var_entity_range_model_single_local_search() {
    run(1);
}
#[test]
fn solve_list_var_entity_range_model_multiple_local_search() {
    run(2);
}

fn run(phase_count: usize) {
    let p = ListProblem::from_entity_ranges(vec![
        ListEntity::new(vec![]).with_value_range(vec![1, 2]),
        ListEntity::new(vec![]).with_value_range(vec![2, 3]),
        ListEntity::new(vec![]).with_value_range(vec![4, 5]),
    ])
    .unwrap()
    .allowing_unassigned();
    let mut n = ListConstraintNetwork::new(3, 5);
    for i in 0..3 {
        n.add_constraint(
            format!("range owner {i}"),
            vec![ListScope::Entity(i)],
            |v| {
                let ListFact::Entity(l) = v[0] else { panic!() };
                SimpleScore::new(if l.len() >= 2 { 2 } else { 1 })
            },
        )
        .unwrap();
    }
    let neighborhoods = [
        Neighborhood::Change,
        Neighborhood::Change,
        Neighborhood::Swap,
        Neighborhood::Swap,
        Neighborhood::SubListChange(SubListChangeConfig::default()),
        Neighborhood::SubListSwap(SubListSwapConfig::default()),
        Neighborhood::RepartitionKOpt(ListKOptSampling::new(
            ListKOptConfig::new(2, 4).unwrap(),
            37,
            32,
        )),
    ];
    let children = neighborhoods.map(|n| (n, 1));
    // The final two variants are repair-only and the union of all eight children.
    for (variant, neighborhood) in neighborhoods
        .into_iter()
        .map(Some)
        .chain([None, None])
        .enumerate()
    {
        let mut phases = vec![SolverPhase::Construction(ConstructionConfig {
            optional: OptionalConstruction::NonWorsening,
            assert_scores: true,
            ..Default::default()
        })];
        for _ in 0..phase_count {
            phases.push(match neighborhood {
                None => SolverPhase::RuinRecreate(RuinRecreateConfig {
                    additional_neighborhoods: if variant == 8 { &children } else { &[] },
                    max_move_evaluations: Some(1000),
                    max_attempts: 1000,
                    max_evaluations: 10000,
                    minimum_ruined: 1,
                    maximum_ruined: 5,
                    seed: 19,
                    assert_scores: true,
                    ..Default::default()
                }),
                Some(neighborhood) => SolverPhase::LocalSearch(LocalSearchConfig {
                    neighborhood,
                    move_order: if variant == 1 || variant == 3 {
                        MoveOrder::Original
                    } else {
                        MoveOrder::ShuffledMoves {
                            seed: 19,
                            window: std::num::NonZeroUsize::new(32).unwrap(),
                        }
                    },
                    max_move_evaluations: Some(1000),
                    max_steps: 1000,
                    max_evaluations: 10001,
                    assert_scores: true,
                    ..Default::default()
                }),
            });
        }
        let mut events = 0;
        let result = solve_list_with_observer(
            &p,
            &n,
            SolverSequence {
                limits: Default::default(),
                phases,
            },
            |_| events += 1,
        )
        .unwrap();
        assert!(events > 0, "variant {variant}");
        assert!(result.solution().is_some());
        assert_eq!(result.phases.len(), phase_count + 1);
        for (i, list) in result.assignment.lists().iter().enumerate() {
            assert!(!list.is_empty(), "variant {variant}, entity {i}");
            assert!(
                list.iter()
                    .all(|v| p.entities()[i].value_range().unwrap().contains(v))
            );
        }
        p.validate(&result.assignment).unwrap();
        let expected = result
            .assignment
            .lists()
            .iter()
            .map(|l| if l.len() >= 2 { 2 } else { 1 })
            .sum();
        assert_eq!(result.score, Some(SimpleScore::new(expected)));
    }
}

#[test]
fn solve_basic_var_entity_range_model_single_local_search() {
    run_basic(1);
}
#[test]
fn solve_basic_var_entity_range_model_multiple_local_search() {
    run_basic(2);
}
fn run_basic(phase_count: usize) {
    let p = FiniteProblem::new(
        [vec![1, 2, 3], vec![1, 2, 5], vec![4, 5]]
            .into_iter()
            .map(|r| Variable::new(r).allowing_unassigned())
            .collect(),
    )
    .unwrap();
    let mut n = ConstraintNetwork::new(3);
    for i in 0..3 {
        n.add_constraint(format!("entity{i}"), vec![i], |v| {
            SimpleScore::new(if v[0].is_some() { 1 } else { -1 })
        })
        .unwrap();
    }
    let selection = SubPillarSelection::combinations(1, usize::MAX).unwrap();
    let neighborhoods = [
        Neighborhood::Change,
        Neighborhood::Swap,
        Neighborhood::Swap,
        Neighborhood::SubPillar(SubPillarConfig {
            selection,
            changes: PillarChangeMode::All,
            swap_selection: None,
        }),
        Neighborhood::SubPillar(SubPillarConfig {
            selection,
            changes: PillarChangeMode::Disabled,
            swap_selection: Some(selection),
        }),
    ];
    let children = neighborhoods.map(|n| (n, 1));
    for (variant, neighborhood) in neighborhoods
        .into_iter()
        .map(Some)
        .chain([None, None])
        .enumerate()
    {
        let mut phases = vec![SolverPhase::Construction(ConstructionConfig {
            optional: OptionalConstruction::NonWorsening,
            assert_scores: true,
            ..Default::default()
        })];
        for _ in 0..phase_count {
            phases.push(match neighborhood {
                Some(neighborhood) => SolverPhase::LocalSearch(LocalSearchConfig {
                    neighborhood,
                    move_order: if variant == 2 {
                        MoveOrder::Original
                    } else {
                        MoveOrder::ShuffledMoves {
                            seed: 29,
                            window: std::num::NonZeroUsize::new(32).unwrap(),
                        }
                    },
                    max_move_evaluations: Some(1000),
                    max_steps: 1000,
                    max_evaluations: 10001,
                    assert_scores: true,
                    ..Default::default()
                }),
                None => SolverPhase::RuinRecreate(RuinRecreateConfig {
                    additional_neighborhoods: if variant == 6 { &children } else { &[] },
                    max_move_evaluations: Some(1000),
                    max_attempts: 1000,
                    max_evaluations: 10000,
                    minimum_ruined: 1,
                    maximum_ruined: 3,
                    seed: 29,
                    assert_scores: true,
                    ..Default::default()
                }),
            });
        }
        let mut events = 0;
        let result = solve_with_observer(
            &p,
            &n,
            SolverSequence {
                limits: Default::default(),
                phases,
            },
            |_| events += 1,
        )
        .unwrap();
        assert!(events > 0);
        assert!(result.solution().is_some());
        assert_eq!(result.phases.len(), phase_count + 1);
        for (i, value) in result.assignment.values().iter().enumerate() {
            assert!(
                value.is_none_or(|v| p.variables()[i].domain().contains(&v)),
                "variant {variant}, entity {i}"
            );
        }
        assert!(![Some(4), Some(5)].contains(&result.assignment.values()[0]));
        assert!(![Some(3), Some(4)].contains(&result.assignment.values()[1]));
        assert!(![Some(1), Some(2), Some(3)].contains(&result.assignment.values()[2]));
        p.validate(&result.assignment).unwrap();
        let expected = result
            .assignment
            .values()
            .iter()
            .map(|v| if v.is_some() { 1 } else { -1 })
            .sum();
        assert_eq!(result.score, Some(SimpleScore::new(expected)));
    }
}
