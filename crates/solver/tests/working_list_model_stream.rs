//! Original typed list event integration controls; no upstream-method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
use timetuck::*;
struct Root;
struct Info {
    bias: i64,
}
struct Owner {
    info: Info,
}
struct Tax {
    info: Info,
}
#[derive(Debug)]
struct Value {
    id: i64,
    stale_index: usize,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
fn data(
    optional: bool,
    empty: bool,
) -> (Arc<ComposedPlanningSolution>, BoundListSolution<Arc<Value>>) {
    let entities = PlanningComposition::new(vec![
        PlanningComponent::registered::<Owner>(vec![
            PlanningVariableDefinition::list::<Arc<Value>>("items", optional),
        ]),
        PlanningComponent::registered::<Value>(vec![PlanningVariableDefinition::shadow::<usize>(
            "index",
        )]),
    ])
    .unwrap();
    let layout = Arc::new(
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<Root>(vec![
            SolutionMemberDefinition::entity_collection::<Owner>("owners"),
            SolutionMemberDefinition::entity_collection::<Value>("valueObjects"),
            SolutionMemberDefinition::value_range::<Arc<Value>>("values", "range"),
            SolutionMemberDefinition::problem_fact_collection::<Tax>("taxes"),
            SolutionMemberDefinition::score::<SimpleScore>("score"),
        ])])
        .unwrap()
        .compile::<Root>(
            &entities,
            &[PlanningType::of::<Owner>(), PlanningType::of::<Value>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap(),
    );
    let values: Vec<_> = if empty {
        vec![]
    } else {
        (1..=3)
            .map(|id| {
                Arc::new(Value {
                    id,
                    stale_index: 999,
                })
            })
            .collect()
    };
    let owners = if empty {
        vec![]
    } else {
        vec![
            SolutionListEntityRecord::new(
                Arc::new(Owner {
                    info: Info { bias: 2 },
                }),
                vec![("items".into(), vec![values[1].clone()])],
            ),
            SolutionListEntityRecord::new(
                Arc::new(Owner {
                    info: Info { bias: 1 },
                }),
                vec![("items".into(), vec![])],
            ),
        ]
    };
    let tax = Arc::new(Tax {
        info: Info { bias: 3 },
    });
    let mut accessors = SolutionFactAccessors::new(layout.clone()).unwrap();
    accessors
        .collection("taxes", move |_: &Root| vec![tax.clone()])
        .unwrap();
    let bound = layout
        .bind_list_with_facts(
            vec![
                SolutionListCollection {
                    member: "owners".into(),
                    entities: owners,
                },
                SolutionListCollection {
                    member: "valueObjects".into(),
                    entities: values
                        .iter()
                        .map(|v| SolutionListEntityRecord::new(v.clone(), vec![]))
                        .collect(),
                },
            ],
            vec![SolutionRange {
                member: "values".into(),
                values,
            }],
            vec![SolutionFieldRange::new::<Owner>(
                "items",
                vec!["range".into()],
            )],
            accessors.read(&Root).unwrap(),
        )
        .unwrap();
    (layout, bound)
}
fn registry(layout: Arc<ComposedPlanningSolution>) -> SolutionStreamRegistry {
    let mut r = SolutionStreamRegistry::new(layout);
    r.project::<Owner, Info>(|o| &o.info).unwrap();
    r.project::<Tax, Info>(|t| &t.info).unwrap();
    r
}
fn ordered_score(bias: i64, values: &[Arc<Value>]) -> i64 {
    bias * values
        .iter()
        .enumerate()
        .map(|(i, v)| (i as i64 + 1) * v.id)
        .sum::<i64>()
}
#[test]
fn actual_typed_list_and_value_events_drive_exact_solve_matching_dense_optimum() {
    let (layout, bound) = data(false, false);
    let r = registry(layout);
    let plan = SolverSequence {
        phases: vec![SolverPhase::Exact(ExactPhaseConfig {
            max_evaluations: 1000,
            ..Default::default()
        })],
        ..Default::default()
    };
    let mut network = ListConstraintNetwork::new(2, 3);
    for (index, bias) in [(0, 2), (1, 1)] {
        network
            .add_constraint(
                format!("owner {index}"),
                vec![ListScope::Entity(index)],
                move |facts| {
                    let ListFact::Entity(list) = &facts[0] else {
                        unreachable!()
                    };
                    SimpleScore::new(ordered_score(bias, list))
                },
            )
            .unwrap();
    }
    network
        .add_constraint("tax", vec![], |_| SimpleScore::new(-3))
        .unwrap();
    for index in 0..3 {
        network
            .add_constraint(
                format!("value {index}"),
                vec![ListScope::Value(index)],
                |facts| {
                    let ListFact::Value { element, .. } = &facts[0] else {
                        unreachable!()
                    };
                    SimpleScore::new(-i64::from(element.is_some()))
                },
            )
            .unwrap();
    }
    let expected = solve_list(bound.problem(), &network, plan.clone()).unwrap();
    let result = solve_list_stream(
        bound.problem(),
        || {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let model = r.working_list_model::<Info, _, _>(&mut b, &bound).unwrap();
            let values = r
                .working_list_values::<Value, Value, _>(&mut b, &bound, model.sources(), true)
                .unwrap();
            b.score("owners and tax", model.port(), |row| {
                SimpleScore::new(row.list("items").map_or(-row.object().bias, |list| {
                    ordered_score(row.object().bias, list)
                }))
            })?;
            b.score("assigned values", &values, |_| SimpleScore::new(-1))?;
            let mut graph = b.build();
            model.initialize_constants(&mut graph)?;
            Ok((graph, model.sources().clone()))
        },
        plan,
    )
    .unwrap();
    assert!(result.is_optimal());
    assert_eq!(result.score, Some(SimpleScore::new(22)));
    assert_eq!(result.score, expected.score);
    assert_eq!(
        result.assignment.lists()[0]
            .iter()
            .map(|v| v.id)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert!(result.assignment.lists()[1].is_empty());
}
#[test]
fn current_lists_and_neighbors_route_only_touched_rows_and_undo_owned_snapshots() {
    let (layout, bound) = data(false, false);
    let r = registry(layout);
    let calls = Cell::new(0);
    let value_calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let model = r.working_list_model::<Info, _, _>(&mut b, &bound).unwrap();
    let values = r
        .working_list_values::<Value, Value, _>(&mut b, &bound, model.sources(), false)
        .unwrap();
    let owners = b
        .score("owners", model.port(), |row| {
            calls.set(calls.get() + 1);
            SimpleScore::new(row.list("items").map_or(-row.object().bias, |list| {
                ordered_score(row.object().bias, list)
            }))
        })
        .unwrap();
    let positions = b
        .score("positions", &values, |row| {
            value_calls.set(value_calls.get() + 1);
            SimpleScore::new(row.state().position.map_or(0, |p| p.index as i64))
        })
        .unwrap();
    let mut graph = b.build();
    model.initialize_constants(&mut graph).unwrap();
    let mut director = ListStreamScoreDirector::new(
        bound.problem(),
        graph,
        model.sources().clone(),
        bound.problem().initial_assignment(),
    )
    .unwrap();
    assert_eq!(calls.get(), 3);
    assert_eq!(value_calls.get(), 3);
    let old = director
        .graph()
        .constraint_matches(&owners)
        .unwrap()
        .values()
        .find(|(row, _)| row.list("items").is_some_and(|list| !list.is_empty()))
        .unwrap()
        .0
        .clone();
    let old_value = director
        .graph()
        .constraint_matches(&positions)
        .unwrap()
        .values()
        .find(|(row, _)| row.object().id == 2)
        .unwrap()
        .0
        .clone();
    assert_eq!(old_value.object().stale_index, 999);
    assert_eq!(
        old_value.state().position,
        Some(ListPosition {
            entity: 0,
            index: 0
        })
    );
    let movement = ListMove::Assign {
        value: bound.problem().values()[0].clone(),
        destination: ListPosition {
            entity: 0,
            index: 0,
        },
    };
    let initial = director.assignment().clone();
    let initial_score = director.score();
    {
        let scored = director.apply(&movement).unwrap();
        assert_eq!(calls.get(), 4);
        assert_eq!(value_calls.get(), 5);
        let current = scored.graph().constraint_matches(&positions).unwrap();
        let current_two = &current
            .values()
            .find(|(row, _)| row.object().id == 2)
            .unwrap()
            .0;
        assert_eq!(current_two.state().position.unwrap().index, 1);
        assert_eq!(current_two.state().previous.as_ref().unwrap().id, 1);
        assert!(Arc::ptr_eq(
            current_two.allocation(),
            old_value.allocation()
        ));
    }
    assert_eq!(director.assignment(), &initial);
    assert_eq!(director.score(), initial_score);
    assert_eq!(
        old.list("items")
            .unwrap()
            .iter()
            .map(|v| v.id)
            .collect::<Vec<_>>(),
        vec![2]
    );
    assert_eq!(old_value.state().position.unwrap().index, 0);
    assert!(old_value.state().previous.is_none());
    director.apply(&movement).unwrap().commit();
    director.assert_consistent().unwrap();
}
#[test]
fn optional_assignment_filter_and_equal_value_handles_keep_canonical_objects() {
    let (layout, bound) = data(true, false);
    let r = registry(layout);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let model = r.working_list_model::<Info, _, _>(&mut b, &bound).unwrap();
    let values = r
        .working_list_values::<Value, Value, _>(&mut b, &bound, model.sources(), true)
        .unwrap();
    let key = b
        .score("assigned", &values, |_| SimpleScore::new(1))
        .unwrap();
    let mut graph = b.build();
    model.initialize_constants(&mut graph).unwrap();
    let mut director = ListStreamScoreDirector::new(
        bound.problem(),
        graph,
        model.sources().clone(),
        bound.problem().initial_assignment(),
    )
    .unwrap();
    assert_eq!(director.score(), SimpleScore::new(1));
    let foreign_equal = Arc::new(Value {
        id: 1,
        stale_index: 100,
    });
    director
        .apply(&ListMove::Assign {
            value: foreign_equal,
            destination: ListPosition {
                entity: 1,
                index: 0,
            },
        })
        .unwrap()
        .commit();
    assert_eq!(director.score(), SimpleScore::new(2));
    let matches = director.graph().constraint_matches(&key).unwrap();
    let row = &matches
        .values()
        .find(|(row, _)| row.object().id == 1)
        .unwrap()
        .0;
    assert_eq!(row.object().stale_index, 999);
    assert!(std::ptr::eq(
        row.object(),
        bound.problem().values()[0].as_ref()
    ));
    director
        .apply(&ListMove::Unassign {
            entity: 1,
            start: 0,
            end: 1,
        })
        .unwrap()
        .commit();
    assert_eq!(director.score(), SimpleScore::new(1));
    director.assert_consistent().unwrap();
}
#[test]
fn relocation_and_callback_panic_restore_actual_lists_positions_and_allocations() {
    let (layout, bound) = data(false, false);
    let r = registry(layout);
    let fail = Cell::new(false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let model = r.working_list_model::<Info, _, _>(&mut b, &bound).unwrap();
    let values = r
        .working_list_values::<Value, Value, _>(&mut b, &bound, model.sources(), false)
        .unwrap();
    let key = b
        .score("value", &values, |row| {
            assert!(
                !(fail.get() && row.state().position.is_some_and(|p| p.entity == 1)),
                "controlled failure"
            );
            SimpleScore::new(row.state().position.map_or(0, |p| p.entity as i64))
        })
        .unwrap();
    let mut graph = b.build();
    model.initialize_constants(&mut graph).unwrap();
    let mut director = ListStreamScoreDirector::new(
        bound.problem(),
        graph,
        model.sources().clone(),
        bound.problem().initial_assignment(),
    )
    .unwrap();
    let movement = ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 0,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    };
    let initial = director.assignment().clone();
    let snapshot = director
        .graph()
        .constraint_matches(&key)
        .unwrap()
        .values()
        .find(|(r, _)| r.object().id == 2)
        .unwrap()
        .0
        .clone();
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = director.apply(&movement);
        }))
        .is_err()
    );
    fail.set(false);
    assert_eq!(director.assignment(), &initial);
    assert_eq!(director.score(), SimpleScore::ZERO.with_structural(-2));
    director.assert_consistent().unwrap();
    director.apply(&movement).unwrap().commit();
    let matches = director.graph().constraint_matches(&key).unwrap();
    let current = &matches
        .values()
        .find(|(r, _)| r.object().id == 2)
        .unwrap()
        .0;
    assert_eq!(current.state().position.unwrap().entity, 1);
    assert!(Arc::ptr_eq(snapshot.allocation(), current.allocation()));
    assert_eq!(snapshot.state().position.unwrap().entity, 0);
    director.assert_consistent().unwrap();
}
#[test]
fn foreign_descriptors_queries_and_graph_sources_are_rejected() {
    let (layout, bound) = data(false, false);
    let (_, foreign) = data(false, false);
    let r = registry(layout);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    assert!(matches!(
        r.working_list_model::<Info, _, _>(&mut b, &foreign),
        Err(WorkingListUpdateError::Source(
            SolutionStreamError::DifferentDescriptor
        ))
    ));
    let mut other = StreamGraphBuilder::<SimpleScore>::new();
    let sources = ListStreamSources::new(&mut other);
    assert!(matches!(
        r.working_list_values::<Value, Value, _>(&mut b, &bound, &sources, false),
        Err(WorkingListUpdateError::Graph(_))
    ));
    assert!(matches!(
        r.working_list_model::<String, _, _>(&mut b, &bound),
        Err(WorkingListUpdateError::Source(
            SolutionStreamError::UnknownType { .. }
        ))
    ));
}
#[test]
fn zero_list_model_executes_nonzero_constant_constraints() {
    let (layout, bound) = data(false, true);
    let r = registry(layout);
    let result = solve_list_stream(
        bound.problem(),
        || {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let model = r.working_list_model::<Info, _, _>(&mut b, &bound).unwrap();
            b.score("tax", model.port(), |row| {
                SimpleScore::new(-row.object().bias)
            })?;
            let mut graph = b.build();
            model.initialize_constants(&mut graph)?;
            Ok((graph, model.sources().clone()))
        },
        SolverConfig::default(),
    )
    .unwrap();
    assert!(result.complete);
    assert_eq!(result.score, Some(SimpleScore::new(-3)));
    assert!(result.assignment.lists().is_empty());
}
