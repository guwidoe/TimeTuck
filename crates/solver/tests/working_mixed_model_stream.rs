//! Original combined typed binding and actual mixed solver event controls.
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
#[derive(Debug, Eq, PartialEq)]
struct Value {
    id: i64,
}
struct Inputs {
    layout: Arc<ComposedPlanningSolution>,
    collections: Vec<SolutionMixedCollection<i32, Arc<Value>>>,
    basic: Vec<SolutionRange<i32>>,
    lists: Vec<SolutionRange<Arc<Value>>>,
    fields: Vec<SolutionFieldRange>,
    facts: BoundSolutionFacts,
}
impl Inputs {
    fn bind(self) -> Result<BoundMixedSolution<i32, Arc<Value>>, MixedSolutionBindingError> {
        self.layout.bind_mixed_with_facts(
            self.collections,
            self.basic,
            self.lists,
            self.fields,
            self.facts,
        )
    }
}
fn input(optional: bool, empty: bool, pinned: bool) -> Inputs {
    let entities = PlanningComposition::new(vec![
        PlanningComponent::registered::<Owner>(vec![
            PlanningVariableDefinition::basic::<i32>("x", false),
            PlanningVariableDefinition::basic::<i32>("y", false),
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
            SolutionMemberDefinition::entities("head", vec![PlanningType::of::<Owner>()], true),
            SolutionMemberDefinition::entity_collection::<Value>("valueObjects"),
            SolutionMemberDefinition::value_range::<i32>("basicValues", "basicRange"),
            SolutionMemberDefinition::value_range::<Arc<Value>>("listValues", "listRange"),
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
        (1..=3).map(|id| Arc::new(Value { id })).collect()
    };
    let owners: Vec<_> = if empty {
        vec![]
    } else {
        [2, 1]
            .into_iter()
            .map(|bias| {
                Arc::new(Owner {
                    info: Info { bias },
                })
            })
            .collect()
    };
    let record = |owner: Arc<Owner>, reverse: bool| {
        let items = if owner.info.bias == 2 {
            vec![values[1].clone()]
        } else {
            vec![]
        };
        let fields = if reverse {
            vec![("y".into(), Some(1)), ("x".into(), Some(1))]
        } else {
            vec![("x".into(), Some(1)), ("y".into(), Some(1))]
        };
        let is_pinned = pinned && owner.info.bias == 2;
        let r = SolutionMixedEntityRecord::new(owner, fields, vec![("items".into(), items)]);
        if is_pinned {
            r.with_basic_pin("x").with_list_pin_policy(ListPinPolicy {
                fully_pinned: false,
                prefix: 1,
            })
        } else {
            r
        }
    };
    let tax = Arc::new(Tax {
        info: Info { bias: 3 },
    });
    let mut reads = SolutionFactAccessors::new(layout.clone()).unwrap();
    reads
        .collection("taxes", move |_: &Root| vec![tax.clone()])
        .unwrap();
    Inputs {
        layout: layout.clone(),
        collections: vec![
            SolutionMixedCollection {
                member: "owners".into(),
                entities: owners.iter().map(|o| record(o.clone(), false)).collect(),
            },
            SolutionMixedCollection {
                member: "head".into(),
                entities: owners
                    .first()
                    .map(|o| vec![record(o.clone(), true)])
                    .unwrap_or_default(),
            },
            SolutionMixedCollection {
                member: "valueObjects".into(),
                entities: values
                    .iter()
                    .map(|v| SolutionMixedEntityRecord::new(v.clone(), vec![], vec![]))
                    .collect(),
            },
        ],
        basic: vec![SolutionRange {
            member: "basicValues".into(),
            values: vec![1, 2],
        }],
        lists: vec![SolutionRange {
            member: "listValues".into(),
            values,
        }],
        fields: vec![
            SolutionFieldRange::new::<Owner>("items", vec!["listRange".into()]),
            SolutionFieldRange::new::<Owner>("y", vec!["basicRange".into()]),
            SolutionFieldRange::new::<Owner>("x", vec!["basicRange".into()]),
        ],
        facts: reads.read(&Root).unwrap(),
    }
}
fn data(
    optional: bool,
    empty: bool,
) -> (SolutionStreamRegistry, BoundMixedSolution<i32, Arc<Value>>) {
    let input = input(optional, empty, false);
    let r = registry(input.layout.clone());
    (r, input.bind().unwrap())
}
fn registry(layout: Arc<ComposedPlanningSolution>) -> SolutionStreamRegistry {
    let mut r = SolutionStreamRegistry::new(layout);
    r.project::<Owner, Info>(|o| &o.info).unwrap();
    r.project::<Tax, Info>(|t| &t.info).unwrap();
    r
}
fn score(bias: i64, x: Option<i32>, y: Option<i32>, list: &[Arc<Value>]) -> i64 {
    let factor = i64::from(x.unwrap_or(0) + y.unwrap_or(0));
    factor
        + factor
            * bias
            * list
                .iter()
                .enumerate()
                .map(|(i, v)| (i as i64 + 1) * v.id)
                .sum::<i64>()
}
#[test]
fn combined_objects_drive_real_mixed_exact_solve_and_match_dense_optimum() {
    let (r, bound) = data(false, false);
    let plan = MixedSolverSequence {
        phases: vec![MixedSolverPhase::Exact(ExactPhaseConfig {
            max_evaluations: 1000,
            ..Default::default()
        })],
        ..Default::default()
    };
    let mut n = MixedConstraintNetwork::new(4, 2, 3);
    for (i, bias) in [(0, 2), (1, 1)] {
        n.add_constraint(
            format!("owner{i}"),
            vec![
                MixedScope::Basic(2 * i),
                MixedScope::Basic(2 * i + 1),
                MixedScope::ListEntity(i),
            ],
            move |f| {
                let (MixedFact::Basic(x), MixedFact::Basic(y), MixedFact::ListEntity(list)) =
                    (&f[0], &f[1], &f[2])
                else {
                    unreachable!()
                };
                SimpleScore::new(score(bias, x.copied(), y.copied(), list))
            },
        )
        .unwrap();
    }
    n.add_constraint("tax", vec![], |_| SimpleScore::new(-3))
        .unwrap();
    for i in 0..3 {
        n.add_constraint(format!("value{i}"), vec![MixedScope::ListValue(i)], |f| {
            let MixedFact::ListValue { element, .. } = &f[0] else {
                unreachable!()
            };
            SimpleScore::new(-i64::from(element.is_some()))
        })
        .unwrap();
    }
    let expected = solve_mixed(bound.problem(), &n, plan.clone()).unwrap();
    let result = solve_mixed_stream(
        bound.problem(),
        || {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let model = r
                .working_mixed_model::<Info, _, _, _>(&mut b, &bound, true)
                .unwrap();
            let values = r
                .working_mixed_values::<Value, Value, _, _>(&mut b, &bound, model.sources(), true)
                .unwrap();
            b.score("owners and tax", model.port(), |row| {
                SimpleScore::new(row.list("items").map_or(-row.object().bias, |list| {
                    score(
                        row.object().bias,
                        *row.field("x").unwrap(),
                        *row.field("y").unwrap(),
                        list,
                    )
                }))
            })?;
            b.score("assigned values", &values, |_| SimpleScore::new(-1))?;
            let mut g = b.build();
            model.initialize_constants(&mut g)?;
            Ok((g, model.sources().clone()))
        },
        plan,
    )
    .unwrap();
    assert!(result.is_optimal());
    assert_eq!(result.score, Some(SimpleScore::new(114)));
    assert_eq!(result.score, expected.score);
    assert_eq!(result.assignment.basic().values(), &[Some(2); 4]);
    assert_eq!(
        result.assignment.lists().lists()[0]
            .iter()
            .map(|v| v.id)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
}
#[test]
fn both_component_events_form_one_current_tuple_and_rollback_old_explanations() {
    let (r, bound) = data(false, false);
    assert_eq!(bound.collection("head"), Some([0].as_slice()));
    assert_eq!(bound.entities().len(), 5);
    let calls = Cell::new(0);
    let unrelated = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let model = r
        .working_mixed_model::<Info, _, _, _>(&mut b, &bound, false)
        .unwrap();
    let key = b
        .score("objects", model.port(), |row| {
            calls.set(calls.get() + 1);
            if row.object().bias != 2 {
                unrelated.set(unrelated.get() + 1);
            }
            SimpleScore::new(row.list("items").map_or(-row.object().bias, |list| {
                score(
                    row.object().bias,
                    *row.field("x").unwrap(),
                    *row.field("y").unwrap(),
                    list,
                )
            }))
        })
        .unwrap();
    let mut g = b.build();
    model.initialize_constants(&mut g).unwrap();
    let mut d = MixedStreamScoreDirector::new(
        bound.problem(),
        g,
        model.sources().clone(),
        bound.problem().initial_assignment(),
    )
    .unwrap();
    assert_eq!(calls.get(), 3);
    let old = d
        .graph()
        .constraint_matches(&key)
        .unwrap()
        .values()
        .find(|(row, _)| row.object().bias == 2)
        .unwrap()
        .0
        .clone();
    let movement = MixedMove {
        basic: Move::Composite(vec![
            Move::Change {
                variable: 0,
                value: Some(2),
            },
            Move::Change {
                variable: 1,
                value: Some(2),
            },
        ]),
        lists: ListMove::Assign {
            value: bound.problem().lists().values()[0].clone(),
            destination: ListPosition {
                entity: 0,
                index: 0,
            },
        },
    };
    let initial = d.assignment().clone();
    let initial_score = d.score();
    {
        let tx = d.apply(&movement).unwrap();
        let matches = tx.graph().constraint_matches(&key).unwrap();
        let current = &matches
            .values()
            .find(|(row, _)| row.object().bias == 2)
            .unwrap()
            .0;
        assert_eq!(*current.field("x").unwrap(), Some(2));
        assert_eq!(*current.field("y").unwrap(), Some(2));
        assert_eq!(
            current
                .list("items")
                .unwrap()
                .iter()
                .map(|v| v.id)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(Arc::ptr_eq(old.allocation(), current.allocation()));
        assert_eq!(calls.get(), 6);
        assert_eq!(unrelated.get(), 2);
    }
    assert_eq!(d.assignment(), &initial);
    assert_eq!(d.score(), initial_score);
    assert_eq!(*old.field("x").unwrap(), Some(1));
    assert_eq!(old.list("items").unwrap()[0].id, 2);
    d.assert_consistent().unwrap();
    d.apply(&movement).unwrap().commit();
    d.assert_consistent().unwrap();
    let snapshot = d
        .graph()
        .constraint_matches(&key)
        .unwrap()
        .values()
        .find(|(row, _)| row.object().bias == 2)
        .unwrap()
        .0
        .clone();
    let basic_only = MixedMove {
        basic: Move::Change {
            variable: 1,
            value: Some(1),
        },
        lists: ListMove::Composite(vec![]),
    };
    let unrelated_before = unrelated.get();
    {
        let tx = d.apply(&basic_only).unwrap();
        let matches = tx.graph().constraint_matches(&key).unwrap();
        let current = &matches
            .values()
            .find(|(row, _)| row.object().bias == 2)
            .unwrap()
            .0;
        assert_eq!(
            snapshot.list("items").unwrap().as_ptr(),
            current.list("items").unwrap().as_ptr()
        );
    }
    assert_eq!(unrelated.get(), unrelated_before);
    d.assert_consistent().unwrap();
}
#[test]
fn basic_and_list_pins_survive_binding_and_joint_invalid_moves_are_atomic() {
    let input = input(false, false, true);
    let bound = input.bind().unwrap();
    assert!(bound.problem().basic().variables()[0].is_pinned());
    assert_eq!(bound.problem().lists().entities()[0].pinned_prefix(), 1);
    let mut a = bound.problem().initial_assignment();
    let initial = a.clone();
    let m = MixedMove {
        basic: Move::Change {
            variable: 1,
            value: Some(2),
        },
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
    };
    assert!(bound.problem().apply(&mut a, &m).is_err());
    assert_eq!(a, initial);
    let m = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(2),
        },
        lists: ListMove::Composite(vec![]),
    };
    assert!(bound.problem().apply(&mut a, &m).is_err());
    assert_eq!(a, initial);
}
#[test]
fn callback_panic_restores_both_components_and_all_current_graph_rows() {
    let (r, bound) = data(false, false);
    let fail = Cell::new(false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let model = r
        .working_mixed_model::<Info, _, _, _>(&mut b, &bound, false)
        .unwrap();
    b.score("objects", model.port(), |row| {
        assert!(
            !(fail.get() && row.field("x").is_some_and(|v| *v == Some(2))),
            "controlled failure"
        );
        SimpleScore::new(row.list("items").map_or(-3, |l| l.len() as i64))
    })
    .unwrap();
    let mut g = b.build();
    model.initialize_constants(&mut g).unwrap();
    let mut d = MixedStreamScoreDirector::new(
        bound.problem(),
        g,
        model.sources().clone(),
        bound.problem().initial_assignment(),
    )
    .unwrap();
    let initial = d.assignment().clone();
    let initial_score = d.score();
    let m = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(2),
        },
        lists: ListMove::Assign {
            value: bound.problem().lists().values()[0].clone(),
            destination: ListPosition {
                entity: 1,
                index: 0,
            },
        },
    };
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = d.apply(&m);
        }))
        .is_err()
    );
    fail.set(false);
    assert_eq!(d.assignment(), &initial);
    assert_eq!(d.score(), initial_score);
    d.assert_consistent().unwrap();
}
#[test]
fn assigned_value_and_initialized_entity_filters_follow_native_changes() {
    let (r, bound) = data(true, false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let model = r
        .working_mixed_model::<Info, _, _, _>(&mut b, &bound, true)
        .unwrap();
    let values = r
        .working_mixed_values::<Value, Value, _, _>(&mut b, &bound, model.sources(), true)
        .unwrap();
    let owners = b
        .score("objects", model.port(), |_| SimpleScore::new(1))
        .unwrap();
    let assigned = b.score("values", &values, |_| SimpleScore::new(1)).unwrap();
    let mut g = b.build();
    model.initialize_constants(&mut g).unwrap();
    let mut d = MixedStreamScoreDirector::new(
        bound.problem(),
        g,
        model.sources().clone(),
        bound.problem().initial_assignment(),
    )
    .unwrap();
    let m = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: None,
        },
        lists: ListMove::Unassign {
            entity: 0,
            start: 0,
            end: 1,
        },
    };
    d.apply(&m).unwrap().commit();
    assert_eq!(d.graph().constraint_matches(&owners).unwrap().len(), 2);
    assert!(d.graph().constraint_matches(&assigned).unwrap().is_empty());
    d.assert_consistent().unwrap();
}
#[test]
fn binding_rejects_conflicting_aliases_missing_reads_and_foreign_facts() {
    let mut alias = input(false, false, false);
    let object = alias.collections[0].entities[0]
        .object()
        .clone()
        .downcast::<Owner>()
        .unwrap();
    let original_values = alias.lists[0].values.clone();
    alias.collections[1].entities = vec![SolutionMixedEntityRecord::new(
        object,
        vec![("x".into(), Some(2)), ("y".into(), Some(1))],
        vec![("items".into(), vec![original_values[1].clone()])],
    )];
    assert!(matches!(
        alias.bind(),
        Err(MixedSolutionBindingError::Binding(
            SolutionBindingError::ConflictingEntityRecord(_)
        ))
    ));
    let mut a = input(false, false, false);
    a.collections[1].entities.clear();
    a.collections[1]
        .entities
        .push(SolutionMixedEntityRecord::new(
            Arc::new(Owner {
                info: Info { bias: 4 },
            }),
            vec![("x".into(), Some(1))],
            vec![("items".into(), vec![])],
        ));
    assert!(matches!(
        a.bind(),
        Err(MixedSolutionBindingError::Binding(
            SolutionBindingError::InvalidFields(_)
        ))
    ));
    let mut a = input(false, false, false);
    a.basic.clear();
    assert!(matches!(
        a.bind(),
        Err(MixedSolutionBindingError::Binding(
            SolutionBindingError::MissingBinding(_)
        ))
    ));
    let mut a = input(false, false, false);
    a.facts = input(false, false, false).facts;
    assert!(matches!(
        a.bind(),
        Err(MixedSolutionBindingError::Binding(
            SolutionBindingError::DifferentFactDescriptor
        ))
    ));
}
#[test]
fn empty_mixed_model_still_scores_actual_facts_and_foreign_streams_are_rejected() {
    let (r, bound) = data(false, true);
    let result = solve_mixed_stream(
        bound.problem(),
        || {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let model = r
                .working_mixed_model::<Info, _, _, _>(&mut b, &bound, true)
                .unwrap();
            b.score("tax", model.port(), |row| {
                SimpleScore::new(-row.object().bias)
            })?;
            let mut g = b.build();
            model.initialize_constants(&mut g)?;
            Ok((g, model.sources().clone()))
        },
        MixedSolverConfig::default(),
    )
    .unwrap();
    assert!(result.complete);
    assert_eq!(result.score, Some(SimpleScore::new(-3)));
    let (_, other) = data(false, true);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    assert!(matches!(
        r.working_mixed_model::<Info, _, _, _>(&mut b, &other, false),
        Err(WorkingMixedUpdateError::Source(
            SolutionStreamError::DifferentDescriptor
        ))
    ));
    let mut foreign = StreamGraphBuilder::<SimpleScore>::new();
    let sources = MixedStreamSources::new(&mut foreign);
    assert!(matches!(
        r.working_mixed_values::<Value, Value, _, _>(&mut b, &bound, &sources, false),
        Err(WorkingMixedUpdateError::Graph(_))
    ));
}
