//! Original real-solver variable-event bridge controls; no source-method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
use timetuck::*;
struct Root;
struct Info {
    name: &'static str,
    offset: i64,
}
struct Entity {
    info: Info,
}
struct Tax {
    info: Info,
}
fn data() -> (Arc<ComposedPlanningSolution>, BoundBasicSolution<i32>) {
    let entity_schema =
        PlanningComposition::new(vec![PlanningComponent::registered::<Entity>(vec![
            PlanningVariableDefinition::basic::<i32>("x", false),
            PlanningVariableDefinition::basic::<i32>("y", false),
        ])])
        .unwrap();
    let layout = Arc::new(
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<Root>(vec![
            SolutionMemberDefinition::entity_collection::<Entity>("entities"),
            SolutionMemberDefinition::value_range::<i32>("values", "range"),
            SolutionMemberDefinition::problem_fact_collection::<Tax>("taxes"),
            SolutionMemberDefinition::score::<SimpleScore>("score"),
        ])])
        .unwrap()
        .compile::<Root>(
            &entity_schema,
            &[PlanningType::of::<Entity>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap(),
    );
    let mut facts = SolutionFactAccessors::new(layout.clone()).unwrap();
    let tax = Arc::new(Tax {
        info: Info {
            name: "tax",
            offset: 3,
        },
    });
    facts
        .collection("taxes", move |_: &Root| vec![tax.clone()])
        .unwrap();
    let entities = ["a", "b"]
        .into_iter()
        .map(|name| {
            SolutionEntityRecord::new(
                Arc::new(Entity {
                    info: Info { name, offset: 0 },
                }),
                vec![("x".into(), None), ("y".into(), None)],
            )
        })
        .collect();
    let bound = layout
        .bind_basic_with_facts(
            vec![SolutionCollection {
                member: "entities".into(),
                entities,
            }],
            vec![SolutionRange {
                member: "values".into(),
                values: vec![1, 2],
            }],
            vec![
                SolutionFieldRange::new::<Entity>("x", vec!["range".into()]),
                SolutionFieldRange::new::<Entity>("y", vec!["range".into()]),
            ],
            facts.read(&Root).unwrap(),
        )
        .unwrap();
    (layout, bound)
}
fn registry(layout: Arc<ComposedPlanningSolution>) -> SolutionStreamRegistry {
    let mut registry = SolutionStreamRegistry::new(layout);
    registry.project::<Entity, Info>(|e| &e.info).unwrap();
    registry.project::<Tax, Info>(|t| &t.info).unwrap();
    registry
}
fn entity_score(x: Option<i32>, y: Option<i32>) -> i64 {
    if x.is_none() || y.is_none() {
        0
    } else if x == Some(1) && y == Some(2) {
        5
    } else {
        1
    }
}
#[test]
fn automatic_multi_field_feed_drives_real_solve_and_matches_dense_optimum() {
    let (layout, bound) = data();
    let registry = registry(layout);
    let mut dense = ConstraintNetwork::<i32, SimpleScore>::new(4);
    for (i, slots) in [[0, 1], [2, 3]].into_iter().enumerate() {
        dense
            .add_constraint(format!("entity {i}"), slots.to_vec(), |v| {
                SimpleScore::new(entity_score(v[0].copied(), v[1].copied()))
            })
            .unwrap();
    }
    dense
        .add_constraint("tax", vec![], |_| SimpleScore::new(-3))
        .unwrap();
    let expected = solve(bound.problem(), &dense, SolverPlan::default()).unwrap();
    let result = solve_stream(
        bound.problem(),
        || {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let model = registry
                .working_basic_model::<Info, i32, _>(&mut b, &bound, true)
                .unwrap();
            b.score("objects", model.port(), |row| {
                if row.fields().count() == 0 {
                    SimpleScore::new(-row.object().offset)
                } else {
                    SimpleScore::new(entity_score(
                        *row.field("x").unwrap(),
                        *row.field("y").unwrap(),
                    ))
                }
            })?;
            let mut graph = b.build();
            model.initialize_constants(&mut graph)?;
            Ok((graph, model.planning_source().clone()))
        },
        SolverPlan::default(),
    )
    .unwrap();
    assert!(result.complete);
    assert_eq!(result.score, Some(SimpleScore::new(7)));
    assert_eq!(result.score, expected.score);
    assert_eq!(
        result.assignment.values(),
        &[Some(1), Some(2), Some(1), Some(2)]
    );
}
#[test]
fn grouped_fields_route_only_changed_entity_and_undo_all_snapshots() {
    let (layout, bound) = data();
    let registry = registry(layout);
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let model = registry
        .working_basic_model::<Info, i32, _>(&mut b, &bound, false)
        .unwrap();
    let constraint = b
        .score("objects", model.port(), |row| {
            calls.set(calls.get() + 1);
            if row.fields().count() == 0 {
                SimpleScore::new(-row.object().offset)
            } else {
                SimpleScore::new(i64::from(
                    row.field("x").unwrap().unwrap_or(0) + row.field("y").unwrap().unwrap_or(0),
                ))
            }
        })
        .unwrap();
    let mut graph = b.build();
    model.initialize_constants(&mut graph).unwrap();
    assert_eq!(calls.get(), 1);
    let mut director = StreamScoreDirector::new(
        bound.problem(),
        graph,
        model.planning_source().clone(),
        bound.problem().initial_assignment(),
    )
    .unwrap();
    assert_eq!(calls.get(), 3);
    let old = director
        .graph()
        .explain_constraint_default(&constraint, |row| {
            vec![(
                row.object().name,
                row.fields().map(|(_, v)| *v).collect::<Vec<_>>(),
            )]
        })
        .unwrap();
    let before = calls.get();
    {
        let guard = director
            .apply(&Move::Change {
                variable: 1,
                value: Some(2),
            })
            .unwrap();
        assert_eq!(guard.score(), SimpleScore::new(-1).with_structural(-3));
    }
    assert_eq!(calls.get(), before + 1);
    assert_eq!(director.assignment().values(), &[None, None, None, None]);
    for row in old
        .matches
        .values()
        .filter(|m| m.justification.facts[0].0 != "tax")
    {
        assert_eq!(row.justification.facts[0].1, vec![None, None]);
    }
    director
        .apply(&Move::Composite(vec![
            Move::Change {
                variable: 0,
                value: Some(1),
            },
            Move::Change {
                variable: 1,
                value: Some(2),
            },
        ]))
        .unwrap()
        .commit();
    let matches = director.graph().constraint_matches(&constraint).unwrap();
    assert_eq!(matches.len(), 3);
    let a = matches
        .values()
        .find(|(row, _)| row.object().name == "a")
        .unwrap();
    assert_eq!(a.0.field("x"), Some(&Some(1)));
    assert_eq!(a.0.field("y"), Some(&Some(2)));
    assert!(Arc::ptr_eq(a.0.allocation(), bound.entities()[0].object()));
    director.assert_consistent().unwrap();
}
#[test]
fn initialized_filter_never_exposes_incomplete_multi_field_groups() {
    let (layout, bound) = data();
    let registry = registry(layout);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let model = registry
        .working_basic_model::<Info, i32, _>(&mut b, &bound, true)
        .unwrap();
    let constraint = b
        .score("complete", model.port(), |row| {
            if row.fields().count() == 0 {
                SimpleScore::ZERO
            } else {
                assert!(row.field("x").unwrap().is_some());
                assert!(row.field("y").unwrap().is_some());
                SimpleScore::new(1)
            }
        })
        .unwrap();
    let mut graph = b.build();
    model.initialize_constants(&mut graph).unwrap();
    let mut director = StreamScoreDirector::new(
        bound.problem(),
        graph,
        model.planning_source().clone(),
        bound.problem().initial_assignment(),
    )
    .unwrap();
    assert_eq!(
        director
            .graph()
            .constraint_matches(&constraint)
            .unwrap()
            .len(),
        1
    );
    director
        .apply(&Move::Change {
            variable: 0,
            value: Some(1),
        })
        .unwrap()
        .commit();
    assert_eq!(
        director
            .graph()
            .constraint_matches(&constraint)
            .unwrap()
            .len(),
        1
    );
    director
        .apply(&Move::Change {
            variable: 1,
            value: Some(2),
        })
        .unwrap()
        .commit();
    assert_eq!(
        director
            .graph()
            .constraint_matches(&constraint)
            .unwrap()
            .len(),
        2
    );
    director
        .apply(&Move::Change {
            variable: 0,
            value: None,
        })
        .unwrap()
        .commit();
    assert_eq!(
        director
            .graph()
            .constraint_matches(&constraint)
            .unwrap()
            .len(),
        1
    );
    director.assert_consistent().unwrap();
}
#[test]
fn failed_solver_move_rolls_back_current_fields_and_allocation_identity() {
    let (layout, bound) = data();
    let registry = registry(layout);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let model = registry
        .working_basic_model::<Info, i32, _>(&mut b, &bound, false)
        .unwrap();
    let constraint = b
        .score("throw", model.port(), |row| {
            assert!(
                row.field("x") != Some(&Some(2)),
                "controlled scoring failure"
            );
            SimpleScore::ZERO
        })
        .unwrap();
    let mut graph = b.build();
    model.initialize_constants(&mut graph).unwrap();
    let mut director = StreamScoreDirector::new(
        bound.problem(),
        graph,
        model.planning_source().clone(),
        bound.problem().initial_assignment(),
    )
    .unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| director
            .apply(&Move::Change {
                variable: 0,
                value: Some(2)
            })
            .unwrap()
            .commit()))
        .is_err()
    );
    assert_eq!(director.assignment().values(), &[None, None, None, None]);
    let matches = director.graph().constraint_matches(&constraint).unwrap();
    assert_eq!(matches.len(), 3);
    let a = matches
        .values()
        .find(|(row, _)| row.object().name == "a")
        .unwrap();
    assert_eq!(a.0.field("x"), Some(&None));
    assert!(Arc::ptr_eq(a.0.allocation(), bound.entities()[0].object()));
    director.assert_consistent().unwrap();
}

#[test]
fn empty_variable_models_still_execute_real_constant_constraints() {
    struct FactRoot;
    let entities = PlanningComposition::new(vec![]).unwrap();
    let layout = Arc::new(
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<FactRoot>(
            vec![
                SolutionMemberDefinition::problem_fact_collection::<Tax>("taxes"),
                SolutionMemberDefinition::score::<SimpleScore>("score"),
            ],
        )])
        .unwrap()
        .compile::<FactRoot>(
            &entities,
            &[],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap(),
    );
    let tax = Arc::new(Tax {
        info: Info {
            name: "tax",
            offset: 11,
        },
    });
    let mut reads = SolutionFactAccessors::new(layout.clone()).unwrap();
    reads
        .collection("taxes", move |_: &FactRoot| vec![tax.clone()])
        .unwrap();
    let bound = layout
        .bind_basic_with_facts::<i32>(vec![], vec![], vec![], reads.read(&FactRoot).unwrap())
        .unwrap();
    let mut registry = SolutionStreamRegistry::new(layout);
    registry.project::<Tax, Info>(|t| &t.info).unwrap();
    let result = solve_stream(
        bound.problem(),
        || {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let model = registry
                .working_basic_model::<Info, i32, _>(&mut b, &bound, true)
                .unwrap();
            b.score("constant", model.port(), |row| {
                SimpleScore::new(-row.object().offset)
            })?;
            let mut graph = b.build();
            model.initialize_constants(&mut graph)?;
            assert!(graph.source_rows(model.planning_source())?.is_empty());
            Ok((graph, model.planning_source().clone()))
        },
        SolverPlan::default(),
    )
    .unwrap();
    assert!(result.complete);
    assert_eq!(result.score, Some(SimpleScore::new(-11)));
    assert!(result.assignment.values().is_empty());
}

#[test]
fn binding_rejects_foreign_descriptor_and_frame_output_tracks_replaced_values() {
    let (layout, bound) = data();
    let registry = registry(layout);
    let (_, foreign) = data();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    assert!(matches!(
        registry.working_basic_model::<Info, i32, _>(&mut b, &foreign, true),
        Err(WorkingBasicUpdateError::Source(
            SolutionStreamError::DifferentDescriptor
        ))
    ));
    let model = registry
        .working_basic_model::<Info, i32, _>(&mut b, &bound, false)
        .unwrap();
    let constraint = b
        .score("sum", model.port(), |row| {
            SimpleScore::new(row.fields().filter_map(|(_, v)| v.map(i64::from)).sum())
        })
        .unwrap();
    let mut graph = b.build();
    model.initialize_constants(&mut graph).unwrap();
    let mut director = StreamScoreDirector::new(
        bound.problem(),
        graph,
        model.planning_source().clone(),
        bound.problem().initial_assignment(),
    )
    .unwrap();
    let before = director
        .graph()
        .explain_constraint_default(&constraint, |row| {
            vec![(
                row.object().name,
                row.fields().map(|(_, v)| *v).collect::<Vec<_>>(),
            )]
        })
        .unwrap();
    director
        .apply(&Move::Composite(vec![
            Move::Change {
                variable: 0,
                value: Some(2),
            },
            Move::Change {
                variable: 1,
                value: Some(1),
            },
        ]))
        .unwrap()
        .commit();
    let current = director.graph().constraint_matches(&constraint).unwrap();
    let a = current
        .values()
        .find(|(row, _)| row.object().name == "a")
        .unwrap();
    assert_eq!(a.0.field("x"), Some(&Some(2)));
    assert_eq!(a.0.field("y"), Some(&Some(1)));
    assert_eq!(a.1, SimpleScore::new(3));
    let old = before
        .matches
        .values()
        .find(|m| m.justification.facts[0].0 == "a")
        .unwrap();
    assert_eq!(old.justification.facts[0].1, vec![None, None]);
    director.assert_consistent().unwrap();
}
