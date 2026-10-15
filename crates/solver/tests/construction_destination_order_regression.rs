//! Original list construction encounter-order controls; no upstream method credit.
use timetuck::*;
fn objective(list: &[i32]) -> SimpleScore {
    SimpleScore::new(
        if list
            .iter()
            .position(|v| *v == 4)
            .is_some_and(|index| index == 1 || index == 2)
        {
            0
        } else {
            -1
        },
    )
}
fn problem() -> ListProblem<i32> {
    ListProblem::new(
        (0..5).collect(),
        vec![
            ListEntity::new(vec![3, 2, 0]),
            ListEntity::new(vec![1]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
}
#[derive(Default)]
struct ListCalc(SimpleScore);
impl IncrementalCalculator<ListAssignment<i32>> for ListCalc {
    type Score = SimpleScore;
    type Justification = ();
    type Change = Vec<usize>;
    fn reset(
        &mut self,
        a: &ListAssignment<i32>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.0 = objective(&a.lists()[0]);
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &ListAssignment<i32>,
        _: &Vec<usize>,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.reset(a, r)
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.0
    }
}
#[derive(Default)]
struct MixedCalc(SimpleScore);
impl IncrementalCalculator<MixedAssignment<i32, i32>> for MixedCalc {
    type Score = SimpleScore;
    type Justification = ();
    type Change = MixedCalculatorChange;
    fn reset(
        &mut self,
        a: &MixedAssignment<i32, i32>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.0 = objective(&a.lists().lists()[0]);
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &MixedAssignment<i32, i32>,
        _: &MixedCalculatorChange,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.reset(a, r)
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.0
    }
}
#[derive(Debug, PartialEq)]
struct Outcome {
    lists: Vec<Vec<i32>>,
    evaluations: u64,
    steps: u64,
    complete: bool,
    termination: ConstructionTermination,
}
fn six(
    p: &ListProblem<i32>,
    c: ConstructionConfig,
    order: MixedConstructionOrder,
    basic_missing: bool,
) -> Vec<Outcome> {
    let mut outcomes = Vec::new();
    macro_rules! list_result {
        ($result:expr) => {{
            let r = $result.unwrap();
            outcomes.push(Outcome {
                lists: r.assignment.lists().to_vec(),
                evaluations: r.evaluations,
                steps: r.completed_steps,
                complete: r.complete,
                termination: r.termination,
            });
        }};
    }
    let mut n = ListConstraintNetwork::new(p.entities().len(), p.values().len());
    n.add_constraint("tie", vec![ListScope::Entity(0)], |facts| {
        let ListFact::Entity(list) = facts[0] else {
            unreachable!()
        };
        objective(list)
    })
    .unwrap();
    list_result!(greedy_list_construction(p, &n, c));
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = ListStreamSources::new(&mut b);
    b.score("tie", s.entities.port(), |row| {
        if row.0 == 0 {
            objective(&row.1)
        } else {
            SimpleScore::ZERO
        }
    })
    .unwrap();
    list_result!(greedy_list_stream_construction(p, b.build(), s, c));
    list_result!(list_calculator_construction(
        p,
        p.initial_assignment(),
        ListCalc::default,
        c
    ));
    let variable = Variable::new(vec![10, 11]);
    let variable = if basic_missing {
        variable
    } else {
        variable.with_initial(10).with_pinned(true)
    };
    let mixed = MixedProblem::new(FiniteProblem::new(vec![variable]).unwrap(), p.clone());
    let c = MixedConstructionConfig {
        construction: c,
        order,
    };
    macro_rules! mixed_result {
        ($result:expr) => {{
            let r = $result.unwrap();
            if r.complete {
                assert_eq!(r.assignment.basic().values(), &[Some(10)]);
            }
            outcomes.push(Outcome {
                lists: r.assignment.lists().lists().to_vec(),
                evaluations: r.evaluations,
                steps: r.completed_steps,
                complete: r.complete,
                termination: r.termination,
            });
        }};
    }
    let mut n = MixedConstraintNetwork::new(1, p.entities().len(), p.values().len());
    n.add_constraint("tie", vec![MixedScope::ListEntity(0)], |facts| {
        let MixedFact::ListEntity(list) = facts[0] else {
            unreachable!()
        };
        objective(list)
    })
    .unwrap();
    mixed_result!(greedy_mixed_construction(&mixed, &n, c));
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = MixedStreamSources::new(&mut b);
    b.score("tie", s.lists.entities.port(), |row| {
        if row.0 == 0 {
            objective(&row.1)
        } else {
            SimpleScore::ZERO
        }
    })
    .unwrap();
    mixed_result!(greedy_mixed_stream_construction(&mixed, b.build(), s, c));
    mixed_result!(mixed_calculator_construction(
        &mixed,
        mixed.initial_assignment(),
        MixedCalc::default,
        c
    ));
    outcomes
}
fn config(
    order: ListConstructionOrder,
    placement: ConstructionPlacement,
) -> ConstructionConfig<'static> {
    ConstructionConfig {
        list_order: order,
        placement,
        ..Default::default()
    }
}
const ORDERS: [MixedConstructionOrder; 2] = [
    MixedConstructionOrder::BasicThenLists,
    MixedConstructionOrder::ListsThenBasic,
];
const PLACEMENTS: [ConstructionPlacement; 3] = [
    ConstructionPlacement::Sequential,
    ConstructionPlacement::Pooled,
    ConstructionPlacement::ValueQueue,
];
#[test]
fn explicit_registry_anchor_order_changes_only_first_tied_winner_in_all_six_backends() {
    let p = problem();
    let entities: Vec<_> = (0..4).collect();
    let initial = p.initial_assignment();
    let positions: Vec<_> =
        ListDestinationSelector::new(&p, &initial, &entities, p.values(), false)
            .unwrap()
            .for_values(&[4])
            .unwrap()
            .iter()
            .flatten()
            .map(|at| (at.entity, at.index))
            .collect();
    assert_eq!(
        positions,
        [
            (0, 0),
            (1, 0),
            (2, 0),
            (3, 0),
            (0, 3),
            (1, 1),
            (0, 2),
            (0, 1)
        ]
    );
    assert_eq!(
        ConstructionConfig::default().list_order,
        ListConstructionOrder::EntityThenIndex
    );
    for order in ORDERS {
        for placement in PLACEMENTS {
            for (list_order, index) in [
                (ListConstructionOrder::EntityThenIndex, 1),
                (ListConstructionOrder::EntityThenValue, 2),
            ] {
                let results = six(&p, config(list_order, placement), order, true);
                for (backend, r) in results.iter().enumerate() {
                    assert!(r.complete);
                    assert_eq!(r.termination, ConstructionTermination::Completed);
                    let mut expected = initial.lists().to_vec();
                    expected[0].insert(index, 4);
                    assert_eq!(r.lists, expected);
                    assert_eq!(objective(&r.lists[0]), SimpleScore::ZERO);
                    if placement != ConstructionPlacement::Pooled {
                        assert_eq!(
                            r.evaluations,
                            if backend < 3 {
                                9
                            } else if placement == ConstructionPlacement::ValueQueue {
                                10
                            } else {
                                11
                            }
                        );
                    }
                }
                assert!(results[..3].windows(2).all(|pair| pair[0] == pair[1]));
                assert!(results[3..].windows(2).all(|pair| pair[0] == pair[1]));
            }
        }
    }
}
#[test]
fn budgets_early_pick_discard_and_optional_no_change_keep_scan_contracts() {
    let p = problem();
    let initial = p.initial_assignment();
    for placement in PLACEMENTS {
        for order in ORDERS {
            for list_order in [
                ListConstructionOrder::EntityThenIndex,
                ListConstructionOrder::EntityThenValue,
            ] {
                for budget in 0..=9 {
                    for discard in [false, true] {
                        let c = ConstructionConfig {
                            max_evaluations: budget,
                            interruption: if discard {
                                ConstructionInterruption::DiscardIncompleteStep
                            } else {
                                ConstructionInterruption::KeepBestPrefix
                            },
                            ..config(list_order, placement)
                        };
                        let results = six(&p, c, order, false);
                        for r in &results {
                            assert!(r.evaluations <= budget);
                            if budget <= 1 || (discard && budget < 9) {
                                assert_eq!(r.lists, initial.lists());
                                assert_eq!(r.steps, 0);
                            }
                        }
                        assert!(results.windows(2).all(|pair| pair[0] == pair[1]));
                    }
                }
                let early = ConstructionConfig {
                    pick_early: ConstructionPickEarly::FirstNonDeterioratingScore,
                    ..config(list_order, placement)
                };
                for r in six(&p, early, order, false) {
                    assert_eq!(r.evaluations, 2);
                    assert_eq!(r.lists[0], vec![4, 3, 2, 0]);
                }
            }
        }
    }
    let optional = p.allowing_unassigned();
    let c = ConstructionConfig {
        optional: OptionalConstruction::EvaluateUnassigned,
        ..config(
            ListConstructionOrder::EntityThenValue,
            ConstructionPlacement::Sequential,
        )
    };
    for r in six(&optional, c, ORDERS[0], false) {
        assert_eq!(r.evaluations, 10);
        assert_eq!(r.lists[0], vec![3, 2, 4, 0]);
    }
}
#[test]
fn pinned_prefix_entity_and_ranges_remain_eligible_and_basic_construction_ignores_list_order() {
    let p = ListProblem::new(
        (0..5).collect(),
        vec![
            ListEntity::new(vec![3, 2, 0]).with_pinned_prefix(1),
            ListEntity::new(vec![1]).with_pinned(true),
            ListEntity::new(vec![]).with_value_range(vec![0, 1, 2, 3]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let initial = p.initial_assignment();
    let positions: Vec<_> =
        ListDestinationSelector::new(&p, &initial, &[0, 1, 2, 3], p.values(), false)
            .unwrap()
            .for_values(&[4])
            .unwrap()
            .iter()
            .flatten()
            .map(|at| (at.entity, at.index))
            .collect();
    assert_eq!(positions, [(0, 1), (3, 0), (0, 3), (0, 2)]);
    for placement in PLACEMENTS {
        for r in six(
            &p,
            config(ListConstructionOrder::EntityThenValue, placement),
            ORDERS[1],
            false,
        ) {
            assert_eq!(r.lists, [vec![3, 4, 2, 0], vec![1], vec![], vec![]]);
            assert!(r.complete);
        }
    }
    let basic = FiniteProblem::new(vec![Variable::new(vec![2, 1]); 2]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let old = greedy_construction(&basic, &n, ConstructionConfig::default()).unwrap();
    let new = greedy_construction(
        &basic,
        &n,
        ConstructionConfig {
            list_order: ListConstructionOrder::EntityThenValue,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(old, new);
}
