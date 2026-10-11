//! Original explicit-repair contract checks; no upstream method credit.
use std::panic::{AssertUnwindSafe, catch_unwind};
use timetuck::*;

#[test]
fn preparation_rejects_invalid_selection_without_mutation() {
    let p = ListProblem::new(
        vec![3, 1, 4, 0, 2],
        vec![
            ListEntity::new(vec![0, 1, 2, 3]).with_pinned_prefix(1),
            ListEntity::new(vec![4]).with_pinned(true),
        ],
    )
    .unwrap();
    let before = p.initial_assignment();
    for (selected, expected) in [
        (vec![], ListRuinPlanError::EmptySelection),
        (
            vec![5],
            ListRuinPlanError::UnknownValue { value: 5, count: 5 },
        ),
        (vec![1, 1], ListRuinPlanError::DuplicateValue { value: 1 }),
        (vec![3], ListRuinPlanError::PinnedValue { value: 3 }),
        (vec![2], ListRuinPlanError::PinnedValue { value: 2 }),
    ] {
        assert_eq!(
            ListRuinPlan::new(&p, &before, &selected).unwrap_err(),
            expected
        );
        assert_eq!(before, p.initial_assignment());
    }
    let plan = ListRuinPlan::new(&p, &before, &[0, 1]).unwrap();
    assert_eq!(
        plan.repair_problem().initial_assignment().lists(),
        &[vec![0, 2], vec![4]]
    );
    let partial = ListAssignment::new(vec![vec![0, 1, 2], vec![4]]);
    assert_eq!(
        ListRuinPlan::new(&p, &partial, &[1]).unwrap_err(),
        ListRuinPlanError::IncompleteInitialAssignment
    );
}

#[test]
fn finish_rejects_incomplete_or_changed_unselected_values_and_can_retry() {
    let p = ListProblem::new_indexed(
        (0..6).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3]).with_pinned_prefix(1),
            ListEntity::new(vec![4, 5]),
        ],
    )
    .unwrap();
    let before = p.initial_assignment();
    let plan = ListRuinPlan::new(&p, &before, &[1, 3]).unwrap();
    assert_eq!(
        plan.finish(&plan.repair_problem().initial_assignment())
            .unwrap_err(),
        ListRuinPlanError::IncompleteReconstruction
    );
    for rows in [
        vec![vec![0, 1, 2, 3], vec![5, 4]],
        vec![vec![0, 1, 3], vec![2, 4, 5]],
    ] {
        assert!(matches!(
            plan.finish(&ListAssignment::new(rows)),
            Err(ListRuinPlanError::UnselectedValuesChanged { .. })
        ));
    }
    assert!(matches!(
        plan.finish(&ListAssignment::new(vec![vec![2, 1, 0, 3], vec![4, 5]])),
        Err(ListRuinPlanError::Model(ListError::PinChanged { .. }))
    ));
    let rebuilt = ListAssignment::new(vec![vec![0, 2], vec![3, 4, 1, 5]]);
    let movement = plan.finish(&rebuilt).unwrap();
    let mut applied = before.clone();
    p.apply(&mut applied, &movement).unwrap().commit();
    assert_eq!(applied, rebuilt);
    assert_eq!(before, p.initial_assignment());
}

#[test]
fn optional_repair_controls_missing_values_and_preserves_ranges() {
    let p = ListProblem::new_indexed(
        (0..6).collect(),
        vec![
            ListEntity::new(vec![0, 1]).with_value_range(vec![0, 1, 2]),
            ListEntity::new(vec![3]).with_value_range(vec![1, 3, 4]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let before = p.initial_assignment();
    let plan = ListRuinPlan::new(&p, &before, &[1, 4, 5]).unwrap();
    assert!(
        plan.finish(&ListAssignment::new(vec![vec![0], vec![3, 4]]))
            .is_ok()
    );
    for rows in [
        vec![vec![0, 2], vec![3]],
        vec![vec![0, 4], vec![3]],
        vec![vec![0], vec![3, 5]],
    ] {
        assert!(matches!(
            plan.finish(&ListAssignment::new(rows)),
            Err(ListRuinPlanError::Model(
                ListError::ValueOutsideEntityRange { .. }
            ))
        ));
    }
    assert!(matches!(
        plan.finish(&ListAssignment::new(vec![vec![0], vec![]])),
        Err(ListRuinPlanError::UnselectedValuesChanged { entity: 1 })
    ));
    let no_change = ListRuinPlan::new(&p, &before, &[5])
        .unwrap()
        .finish(&before)
        .unwrap();
    assert!(!p.is_move_doable(&before, &no_change).unwrap());
}

#[test]
fn interrupted_required_construction_can_resume_but_cannot_finish_partial() {
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![ListEntity::new(vec![0, 1, 2]), ListEntity::new(vec![])],
    )
    .unwrap();
    let before = p.initial_assignment();
    let plan = ListRuinPlan::new(&p, &before, &[0, 1, 2]).unwrap();
    let network = ListConstraintNetwork::<_, SimpleScore>::new(2, 3);
    let partial = greedy_list_construction(
        plan.repair_problem(),
        &network,
        ConstructionConfig {
            max_evaluations: 1,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!partial.complete);
    assert_eq!(
        plan.finish(&partial.assignment).unwrap_err(),
        ListRuinPlanError::IncompleteReconstruction
    );
    let complete = greedy_list_construction_from(
        plan.repair_problem(),
        &network,
        partial.assignment,
        ConstructionConfig::default(),
    )
    .unwrap();
    assert!(complete.complete);
    assert!(plan.finish(&complete.assignment).is_ok());
}

#[derive(Clone, Debug)]
struct Payload(u8, &'static str);
impl PartialEq for Payload {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for Payload {}

#[test]
fn unselected_payloads_and_temporary_error_panic_rollback_are_exact() {
    let p = ListProblem::new(
        vec![
            Payload(0, "registry"),
            Payload(1, "registry"),
            Payload(2, "registry"),
        ],
        vec![
            ListEntity::new(vec![
                Payload(0, "pin"),
                Payload(1, "selected"),
                Payload(2, "kept"),
            ])
            .with_pinned_prefix(1),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let before = p.initial_assignment();
    let rebuilt = ListAssignment::new(vec![
        vec![Payload(0, "discard"), Payload(2, "discard")],
        vec![Payload(1, "rebuilt")],
    ]);
    let movement = ListRuinPlan::new(&p, &before, &[1])
        .unwrap()
        .finish(&rebuilt)
        .unwrap();
    let mut n = ListConstraintNetwork::<Payload, SimpleScore>::new(2, 3);
    n.add_constraint(
        "payload",
        vec![ListScope::Entity(0), ListScope::Entity(1)],
        |facts| {
            SimpleScore::new(
                facts
                    .iter()
                    .map(|fact| {
                        let ListFact::Entity(row) = fact else {
                            unreachable!()
                        };
                        row.iter().map(|v| v.1.len() as i64).sum::<i64>()
                    })
                    .sum(),
            )
        },
    )
    .unwrap();
    let mut d = ListScoreDirector::new(&p, &n, before.clone()).unwrap();
    let original_score = d.score();
    let result =
        TemporaryMoves::new(&mut d).process(&movement, TemporaryMoveOptions::default(), |a, _| {
            assert_eq!(
                a.lists()[0].iter().map(|v| v.1).collect::<Vec<_>>(),
                vec!["pin", "kept"]
            );
            assert_eq!(a.lists()[1][0].1, "rebuilt");
            Err::<(), _>("reject")
        });
    assert!(matches!(
        result,
        Err(TemporaryMoveError::Postprocessor("reject"))
    ));
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            TemporaryMoves::new(&mut d)
                .process(
                    &movement,
                    TemporaryMoveOptions::default(),
                    |_, _| -> Result<(), ()> { panic!("reject") },
                )
                .unwrap();
        }))
        .is_err()
    );
    assert_eq!(
        d.assignment().lists()[0]
            .iter()
            .map(|v| v.1)
            .collect::<Vec<_>>(),
        vec!["pin", "selected", "kept"]
    );
    assert!(d.assignment().lists()[1].is_empty());
    assert_eq!(d.score(), original_score);
    d.assert_consistent().unwrap();
}

#[test]
fn required_reconstructions_match_all_small_ordered_partitions() {
    // Exhaust every complete two-list permutation independently of move assembly.
    for indexed in [false, true] {
        let entities = vec![ListEntity::new(vec![0, 1, 2]), ListEntity::new(vec![3])];
        let p = if indexed {
            ListProblem::new_indexed(vec![3, 1, 0, 2], entities)
        } else {
            ListProblem::new(vec![3, 1, 0, 2], entities)
        }
        .unwrap();
        let before = p.initial_assignment();
        for mask in 1_u8..16 {
            let selected: Vec<_> = p
                .values()
                .iter()
                .enumerate()
                .filter_map(|(i, v)| (mask & (1 << v) != 0).then_some(i))
                .collect();
            let plan = ListRuinPlan::new(&p, &before, &selected).unwrap();
            for a in 0..4 {
                for b in 0..4 {
                    for c in 0..4 {
                        for d in 0..4 {
                            let order = [a, b, c, d];
                            if (0..4).any(|i| order[..i].contains(&order[i])) {
                                continue;
                            }
                            for split in 0..=4 {
                                let candidate = ListAssignment::new(vec![
                                    order[..split].to_vec(),
                                    order[split..].to_vec(),
                                ]);
                                let expected = before.lists().iter().zip(candidate.lists()).all(
                                    |(left, right)| {
                                        left.iter()
                                            .filter(|v| mask & (1 << **v) == 0)
                                            .eq(right.iter().filter(|v| mask & (1 << **v) == 0))
                                    },
                                );
                                let result = plan.finish(&candidate);
                                assert_eq!(
                                    result.is_ok(),
                                    expected,
                                    "mask={mask}, candidate={candidate:?}"
                                );
                                if let Ok(movement) = result {
                                    let mut applied = before.clone();
                                    {
                                        let trial = p.apply(&mut applied, &movement).unwrap();
                                        assert_eq!(*trial, candidate);
                                    }
                                    assert_eq!(applied, before);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn repair_and_finish_preserve_fixed_dependency_validation() {
    let p = ListProblem::new_indexed(vec![0, 1, 2], vec![ListEntity::new(vec![0, 1, 2])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0], vec![1]])
        .unwrap();
    let before = p.initial_assignment();
    let plan = ListRuinPlan::new(&p, &before, &[0, 1, 2]).unwrap();
    let cyclic = ListAssignment::new(vec![vec![2, 1, 0]]);
    assert!(matches!(
        plan.repair_problem().validate(&cyclic),
        Err(ListError::DependencyCycle { .. })
    ));
    assert!(matches!(
        plan.finish(&cyclic),
        Err(ListRuinPlanError::Model(ListError::DependencyCycle { .. }))
    ));
    assert!(plan.finish(&before).is_ok());
}

#[test]
fn rebuilt_candidate_runs_through_both_custom_provider_search_backends() {
    let p = ListProblem::new_indexed(
        (0..4).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3]).with_pinned_prefix(1),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let mut n = ListConstraintNetwork::<i32, SimpleScore>::new(2, 4);
    n.add_constraint("receiver", vec![ListScope::Entity(1)], |facts| {
        let ListFact::Entity(row) = facts[0] else {
            unreachable!()
        };
        SimpleScore::new(row.len() as i64)
    })
    .unwrap();
    let mut provider = |model: &ListProblem<i32>, assignment: &ListAssignment<i32>, _: u64| {
        let plan = ListRuinPlan::new(model, assignment, &[1, 2, 3]).unwrap();
        let rebuilt = greedy_list_construction(
            plan.repair_problem(),
            &n,
            ConstructionConfig {
                max_evaluations: 100,
                assert_scores: true,
                ..Default::default()
            },
        )
        .unwrap();
        vec![plan.finish(&rebuilt.assignment).unwrap()]
    };
    let config = LocalSearchConfig {
        max_steps: 1,
        max_evaluations: 10,
        assert_scores: true,
        ..Default::default()
    };
    let scoped = list_local_search_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        config.clone(),
        &mut provider,
    )
    .unwrap();
    let mut builder = StreamGraphBuilder::new();
    let sources = ListStreamSources::new(&mut builder);
    builder
        .score("receiver", sources.entities.port(), |(entity, row)| {
            SimpleScore::new(if *entity == 1 { row.len() as i64 } else { 0 })
        })
        .unwrap();
    let stream = list_stream_local_search_from_with_provider(
        &p,
        builder.build(),
        sources,
        p.initial_assignment(),
        config,
        &mut provider,
    )
    .unwrap();
    assert_eq!(scoped.best, stream.best);
    assert_eq!(scoped.best.as_ref().unwrap().score, SimpleScore::new(3));
    assert_eq!(
        scoped.best.as_ref().unwrap().assignment.list(0),
        Some([0].as_slice())
    );
    assert_eq!(scoped.evaluations, stream.evaluations);
    assert_eq!(p.initial_assignment().lists(), &[vec![0, 1, 2, 3], vec![]]);
}
