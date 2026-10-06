// Adapted from pinned Timefold Community DefaultSolverTest and test fixtures.
// Apache-2.0; source identities, fixture notices and substitutions: compat/provenance.json.
use timetuck::*;

fn basic_variants<V: Eq + Clone>(
    p: &FiniteProblem<V>,
    families: &[BasicNeighborhood<'_>],
    repair: bool,
    target: bool,
) {
    let n = ConstraintNetwork::<_, SimpleScore>::new(p.variables().len());
    let fine = families
        .iter()
        .map(|c| (RepairNeighborhood::Basic(*c), 1))
        .collect::<Vec<_>>();
    let variants = fine.len() + usize::from(repair) + 1;
    for variant in 0..variants {
        let union = variant == variants - 1;
        let children = if union {
            &fine[..]
        } else if variant < fine.len() {
            &fine[variant..variant + 1]
        } else {
            &fine[..0]
        };
        let repair_weight = u32::from(repair && (union || variant == fine.len()));
        let plan = SolverSequence {
            limits: SolverLimits {
                best_score_limit: target.then_some(SimpleScore::new(0)),
                ..Default::default()
            },
            phases: vec![
                SolverPhase::Construction(ConstructionConfig {
                    assert_scores: true,
                    ..Default::default()
                }),
                SolverPhase::RuinRecreate(RuinRecreateConfig {
                    additional_neighborhoods: children,
                    move_weights: RepairMoveWeights {
                        change: 0,
                        swap: 0,
                        repair: repair_weight,
                    },
                    seed: 20261007,
                    max_attempts: 40,
                    max_move_evaluations: Some(40),
                    assert_scores: true,
                    ..Default::default()
                }),
            ],
        };
        let mut events = 0;
        let r = solve_with_observer(p, &n, plan, |_| events += 1).unwrap();
        assert!(events > 0, "variant {variant}");
        assert!(r.complete && r.solution().is_some(), "variant {variant}");
        assert!(
            r.phases
                .iter()
                .skip(1)
                .all(|phase| phase.work.move_evaluations <= 40)
        );
        p.validate(&r.assignment).unwrap();
        assert_eq!(r.score, Some(SimpleScore::new(0)));
        if target {
            // Source best-score limit0 can end after construction; do not turn
            // this fixture into a claim that every search selector executed.
            assert_eq!(r.phases.len(), 1);
        } else {
            assert_eq!(r.phases.len(), 2);
        }
    }
}
fn pillar<'a>(
    schema: Option<&'a PillarSchema>,
    variables: Option<&'a [usize]>,
    swaps: bool,
    singleton: bool,
) -> BasicNeighborhood<'a> {
    let selection =
        SubPillarSelection::combinations(1, if singleton { 1 } else { usize::MAX }).unwrap();
    BasicNeighborhood {
        value_orders: &[],

        neighborhood: Neighborhood::SubPillar(SubPillarConfig {
            selection,
            changes: if swaps {
                PillarChangeMode::Disabled
            } else {
                PillarChangeMode::All
            },
            swap_selection: swaps.then_some(selection),
        }),
        variables,
        pillar_schema: schema,
    }
}
#[test]
fn solve_single_var_move_config() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let families = [
        BasicNeighborhood {
            neighborhood: Neighborhood::Change,
            ..Default::default()
        },
        BasicNeighborhood {
            neighborhood: Neighborhood::Swap,
            ..Default::default()
        },
        pillar(None, None, false, false),
        pillar(None, None, true, false),
    ];
    basic_variants(&p, &families, true, false);
}
#[test]
fn solve_list_var_move_config() {
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![]); 2]).unwrap();
    let n = ListConstraintNetwork::<_, SimpleScore>::new(2, 2);
    let fine = [
        Neighborhood::Change,
        Neighborhood::Swap,
        Neighborhood::SubListChange(SubListChangeConfig::default()),
        Neighborhood::SubListSwap(SubListSwapConfig::default()),
    ]
    .map(|n| (RepairNeighborhood::Standard(n), 1));
    for variant in 0..6 {
        let children = if variant == 5 {
            &fine[..]
        } else if variant < 4 {
            &fine[variant..variant + 1]
        } else {
            &fine[..0]
        };
        let mut events = 0;
        let r = solve_list_with_observer(
            &p,
            &n,
            SolverSequence {
                phases: vec![
                    SolverPhase::Construction(ConstructionConfig {
                        assert_scores: true,
                        ..Default::default()
                    }),
                    SolverPhase::RuinRecreate(RuinRecreateConfig {
                        additional_neighborhoods: children,
                        move_weights: RepairMoveWeights {
                            change: 0,
                            swap: 0,
                            repair: u32::from(variant >= 4),
                        },
                        seed: 20261007,
                        max_attempts: 40,
                        max_move_evaluations: Some(40),
                        assert_scores: true,
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            },
            |_| events += 1,
        )
        .unwrap();
        assert!(
            events > 0 && r.complete && r.solution().is_some(),
            "variant {variant}"
        );
        assert_eq!(r.phases.len(), 2);
        assert!(r.phases[1].work.move_evaluations <= 40);
        p.validate(&r.assignment).unwrap();
        assert_eq!(r.score, Some(SimpleScore::new(0)));
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MultiValue {
    Value(usize),
    Other(usize),
}
#[test]
fn solve_multi_var_move_config() {
    // Each entity has two required shared-range fields and an optional third
    // field with a distinct value type. All six fields start unassigned.
    let variables = (0..2)
        .flat_map(|_| {
            [
                Variable::new(vec![MultiValue::Value(0), MultiValue::Value(1)]),
                Variable::new(vec![MultiValue::Value(0), MultiValue::Value(1)]),
                Variable::new(vec![MultiValue::Other(0), MultiValue::Other(1)])
                    .allowing_unassigned(),
            ]
        })
        .collect();
    let p = FiniteProblem::new(variables).unwrap();
    let rows = PillarSchema::new(vec![vec![0, 1, 2], vec![3, 4, 5]]).unwrap();
    let primary = PillarSchema::new(vec![vec![0], vec![3]]).unwrap();
    let families = [
        BasicNeighborhood {
            neighborhood: Neighborhood::Change,
            ..Default::default()
        },
        pillar(Some(&rows), None, true, true),
        pillar(Some(&primary), Some(&[0, 3]), false, false),
        pillar(Some(&rows), None, true, false),
    ];
    basic_variants(&p, &families, false, false);
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EntityValue {
    Value(usize),
    Lead(usize),
}
#[test]
fn solve_multi_entity_move_config() {
    // Native sequential construction initializes lead slots before herd slots,
    // preserving the source's two ordered construction phases' component order.
    let mut vars = vec![Variable::new(vec![EntityValue::Value(0), EntityValue::Value(1)]); 2];
    vars.extend(vec![
        Variable::new(vec![
            EntityValue::Lead(0),
            EntityValue::Lead(1)
        ]);
        2
    ]);
    let p = FiniteProblem::new(vars).unwrap();
    let families = [
        BasicNeighborhood {
            neighborhood: Neighborhood::Change,
            ..Default::default()
        },
        BasicNeighborhood {
            neighborhood: Neighborhood::Swap,
            ..Default::default()
        },
        pillar(None, Some(&[0, 1]), false, false),
        pillar(None, Some(&[0, 1]), true, false),
    ];
    basic_variants(&p, &families, false, true);
}
