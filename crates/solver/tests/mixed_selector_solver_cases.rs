// Adapted from pinned Timefold Community DefaultSolverTest and test fixtures.
// Apache-2.0; exact source identities and native substitutions: compat/provenance.json.
use timetuck::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BasicValue {
    id: usize,
    strength: usize,
}

fn run(multi_entity: bool) {
    let entities = if multi_entity { 3 } else { 2 };
    let values = (0..2)
        .map(|id| BasicValue {
            id,
            strength: if multi_entity { 20 - id } else { 2 - id },
        })
        .collect::<Vec<_>>();
    let mut ascending = values.clone();
    ascending.sort_by_key(|v| v.strength);
    let p = MixedProblem::new(
        FiniteProblem::new(
            (0..entities)
                .flat_map(|_| {
                    [
                        Variable::new(if multi_entity {
                            ascending.clone()
                        } else {
                            values.clone()
                        }),
                        Variable::new(values.clone()),
                    ]
                })
                .collect(),
        )
        .unwrap(),
        ListProblem::new(vec![0usize, 1], vec![ListEntity::new(vec![]); entities]).unwrap(),
    );
    let mut n = MixedConstraintNetwork::<_, _, SimpleScore>::new(2 * entities, entities, 2);
    for slot in 0..2 * entities {
        n.add_constraint(
            format!("basic {slot}"),
            vec![MixedScope::Basic(slot)],
            |f| {
                let MixedFact::Basic(v) = f[0] else { panic!() };
                SimpleScore::new(i64::from(v.is_some()))
            },
        )
        .unwrap();
    }
    for owner in 0..entities {
        n.add_constraint(
            format!("list {owner}"),
            vec![MixedScope::ListEntity(owner)],
            |f| {
                let MixedFact::ListEntity(v) = f[0] else {
                    panic!()
                };
                SimpleScore::new(if v.len() == 1 { 2 } else { 1 })
            },
        )
        .unwrap();
    }
    let first_slots = (0..entities).map(|i| 2 * i).collect::<Vec<_>>();
    let list_ids = (2 * entities..2 * entities + 2).collect::<Vec<_>>();
    let column = PillarSchema::new(first_slots.iter().map(|i| vec![*i]).collect()).unwrap();
    let rows = PillarSchema::new((0..entities).map(|i| vec![2 * i, 2 * i + 1]).collect()).unwrap();
    let singleton = SubPillarSelection::combinations(1, 1).unwrap();
    let subsets = SubPillarSelection::combinations(1, usize::MAX).unwrap();
    let row_swap = |selection| MixedNeighborhood {
        basic: Some(Neighborhood::SubPillar(SubPillarConfig {
            selection,
            changes: PillarChangeMode::Disabled,
            swap_selection: Some(selection),
        })),
        lists: None,
        pillar_schema: Some(&rows),
        ..Default::default()
    };
    let mut fine = vec![
        MixedNeighborhood {
            basic: Some(Neighborhood::Change),
            lists: None,
            ..Default::default()
        },
        row_swap(singleton),
        MixedNeighborhood {
            basic: Some(Neighborhood::SubPillar(SubPillarConfig {
                selection: subsets,
                changes: PillarChangeMode::All,
                swap_selection: None,
            })),
            lists: None,
            basic_variables: Some(&first_slots),
            pillar_schema: Some(&column),
            ..Default::default()
        },
        row_swap(subsets),
    ];
    fine.extend(
        [
            Neighborhood::Change,
            Neighborhood::Swap,
            Neighborhood::SubListChange(SubListChangeConfig::default()),
            Neighborhood::SubListSwap(SubListSwapConfig::default()),
            Neighborhood::RepartitionKOpt(ListKOptSampling::new(
                ListKOptConfig::new(2, 4).unwrap(),
                37,
                32,
            )),
        ]
        .map(|lists| MixedNeighborhood {
            basic: None,
            lists: Some(lists),
            ..Default::default()
        }),
    );
    let pools = [
        RepairPool {
            candidates: &first_slots,
            weight: 1,
        },
        RepairPool {
            candidates: &list_ids,
            weight: 1,
        },
    ];
    let order = if multi_entity {
        MixedConstructionOrder::ListsThenBasic
    } else {
        MixedConstructionOrder::BasicThenLists
    };
    // Source order: four basic fine families, basic repair, five list fine families,
    // list repair, then their eleven-child union. Independent families share one
    // native bounded search; source PRNG/selector traces are not reproduced.
    for variant in 0..12 {
        let families = match variant {
            0..=3 => vec![(RepairNeighborhood::Mixed(fine[variant].clone()), 1)],
            5..=9 => vec![(RepairNeighborhood::Mixed(fine[variant - 1].clone()), 1)],
            11 => fine
                .iter()
                .map(|f| (RepairNeighborhood::Mixed(f.clone()), 1))
                .collect(),
            _ => vec![],
        };
        let selected_pools = match variant {
            4 => &pools[..1],
            10 => &pools[1..],
            11 => &pools[..],
            _ => &pools[..0],
        };
        let plan = MixedSolverSequence {
            phases: vec![
                MixedSolverPhase::Construction(MixedConstructionConfig {
                    order,
                    ..Default::default()
                }),
                MixedSolverPhase::RuinRecreate(MixedRuinRecreateConfig {
                    search: RuinRecreateConfig {
                        additional_neighborhoods: &families,
                        repair_pools: Some(selected_pools),
                        move_weights: RepairMoveWeights {
                            change: 0,
                            swap: 0,
                            repair: 1,
                        },
                        seed: 20261006,
                        max_attempts: 40,
                        max_move_evaluations: Some(40),
                        assert_scores: true,
                        ..Default::default()
                    },
                    order,
                }),
            ],
            ..Default::default()
        };
        let mut events = 0;
        let r = solve_mixed_with_observer(&p, &n, plan, |_| events += 1).unwrap();
        assert!(events > 0, "variant {variant}");
        assert!(r.complete && r.solution().is_some(), "variant {variant}");
        assert_eq!(r.phases.len(), 2);
        assert!(r.phases[1].work.move_evaluations <= 40);
        p.validate(&r.assignment).unwrap();
        for pair in r.assignment.basic().values().chunks_exact(2) {
            assert!(pair[0].is_some() && pair[1].is_some(), "variant {variant}");
            if multi_entity {
                assert_ne!(pair[0], pair[1], "variant {variant}");
            }
        }
        assert_eq!(
            r.assignment
                .lists()
                .lists()
                .iter()
                .filter(|l| l.is_empty())
                .count(),
            usize::from(multi_entity),
            "variant {variant}"
        );
        let expected = 2 * entities
            + r.assignment
                .lists()
                .lists()
                .iter()
                .map(|l| if l.len() == 1 { 2 } else { 1 })
                .sum::<usize>();
        assert_eq!(
            r.score,
            Some(SimpleScore::new(expected as i64)),
            "variant {variant}"
        );
    }
}
#[test]
fn solve_move_config_mixed_model() {
    run(false);
}
#[test]
fn solve_multi_entity_move_config_mixed_model() {
    run(true);
}
