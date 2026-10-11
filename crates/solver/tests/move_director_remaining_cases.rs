// Adapted from pinned Community MoveDirectorTest. Apache-2.0; see provenance.
use std::{cell::RefCell, rc::Rc, sync::Arc};
use timetuck::*;
struct Solution;
struct Entity;
fn basic_schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<&'static str>("value", false),
    ])])
    .unwrap()
}
fn basic_slots(s: &PlanningSchema) -> SchemaVariableSlots<'_> {
    SchemaVariableSlots {
        variable: s
            .genuine_entity::<Entity>()
            .unwrap()
            .genuine_variable("value")
            .unwrap(),
        slots: vec![0],
    }
}
#[test]
fn two_arg_is_value_in_range_throws_for_entity_scoped_range() {
    let schema = basic_schema();
    let problem = FiniteProblem::new(vec![Variable::new(vec!["value"])]).unwrap();
    let binding = basic_slots(&schema).bind_basic(&problem).unwrap();
    let error = binding
        .is_value_in_solution_range(Some(&"value"))
        .unwrap_err();
    assert_eq!(error, FieldBindingError::EntityScopedRange);
    assert!(
        error
            .to_string()
            .contains("has a value range on an entity, not on the solution")
    );
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Event {
    before: bool,
    slots: Vec<usize>,
    values: Vec<Option<&'static str>>,
}
struct Calculator {
    events: Rc<RefCell<Vec<Event>>>,
}
impl IncrementalCalculator<Assignment<&'static str>> for Calculator {
    type Score = SimpleScore;
    type Justification = ();
    type Change = Vec<usize>;
    fn reset(
        &mut self,
        _: &Assignment<&'static str>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        Ok(())
    }
    fn before_change(
        &mut self,
        a: &Assignment<&'static str>,
        slots: &Vec<usize>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.events.borrow_mut().push(Event {
            before: true,
            slots: slots.clone(),
            values: a.values().to_vec(),
        });
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &Assignment<&'static str>,
        slots: &Vec<usize>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.events.borrow_mut().push(Event {
            before: false,
            slots: slots.clone(),
            values: a.values().to_vec(),
        });
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::ZERO
    }
}
#[test]
fn change_variable() {
    let schema = basic_schema();
    let problem = FiniteProblem::new(vec![
        Variable::new(vec!["value", "newValue"]).with_initial("value"),
    ])
    .unwrap();
    let field = basic_slots(&schema).bind_basic(&problem).unwrap();
    let events = Rc::new(RefCell::new(vec![]));
    let mut director = CalculatorScoreDirector::new(
        &problem,
        problem.initial_assignment(),
        || Calculator {
            events: events.clone(),
        },
        false,
    )
    .unwrap();
    let edit = director
        .apply(&field.change(0, Some("newValue")).unwrap())
        .unwrap();
    assert_eq!(field.read(edit.assignment(), 0).unwrap(), Some(&"newValue"));
    assert_eq!(
        *events.borrow(),
        vec![
            Event {
                before: true,
                slots: vec![0],
                values: vec![Some("value")]
            },
            Event {
                before: false,
                slots: vec![0],
                values: vec![Some("newValue")]
            }
        ]
    );
    events.borrow_mut().clear();
    drop(edit);
    assert_eq!(
        field.read(director.assignment(), 0).unwrap(),
        Some(&"value")
    );
    assert_eq!(
        *events.borrow(),
        vec![
            Event {
                before: true,
                slots: vec![0],
                values: vec![Some("newValue")]
            },
            Event {
                before: false,
                slots: vec![0],
                values: vec![Some("value")]
            }
        ]
    );
}
fn list(values: Vec<&'static str>) -> ListProblem<&'static str> {
    ListProblem::new(values.clone(), vec![ListEntity::new(values)]).unwrap()
}
#[test]
fn move_values_between_lists_throws_on_same_entity() {
    let p = list(vec!["value1", "value2"]);
    let mut editor = ListMoveBuilder::new(&p, &p.initial_assignment()).unwrap();
    let error = editor
        .relocate_between_lists(
            ListRange {
                entity: 0,
                start: 0,
                end: 1,
            },
            ListPosition {
                entity: 0,
                index: 0,
            },
            false,
        )
        .unwrap_err();
    assert_eq!(
        error,
        ListEditError::SameEntity {
            entity: 0,
            swap: false
        }
    );
    assert!(error.to_string().contains("must be different"));
}
#[test]
fn swap_values_between_lists_throws_on_same_entity() {
    let p = list(vec!["value1", "value2", "value3", "value4"]);
    let mut editor = ListMoveBuilder::new(&p, &p.initial_assignment()).unwrap();
    let error = editor
        .swap_between_lists(
            ListRange {
                entity: 0,
                start: 0,
                end: 1,
            },
            ListRange {
                entity: 0,
                start: 2,
                end: 3,
            },
            false,
        )
        .unwrap_err();
    assert_eq!(
        error,
        ListEditError::SameEntity {
            entity: 0,
            swap: true
        }
    );
    assert!(error.to_string().contains("must be different"));
    assert!(error.to_string().contains("swapValuesInList"));
}
#[test]
fn replace_value_in_list_throws_on_negative_index() {
    let p = list(vec!["value1", "value2"]);
    let mut editor = ListMoveBuilder::new(&p, &p.initial_assignment()).unwrap();
    assert_eq!(
        editor.replace_signed(0, -1, 1).unwrap_err(),
        ListEditError::SignedIndex {
            entity: 0,
            index: -1,
            len: 2
        }
    );
}
#[derive(Debug)]
struct Value {
    code: &'static str,
}
#[test]
fn look_up_working_object() {
    let expected = Arc::new(Value { code: "value" });
    let working = Arc::new(Value { code: "value" });
    let mut registry = WorkingObjectLookup::new(
        WorkingLookupStrategy::IdOrFail,
        WorkingLookupSchema::id("code", |v: &Value| Some(v.code)),
    );
    registry.add(working).unwrap();
    let actual = registry.lookup(&expected).unwrap();
    assert!(!Arc::ptr_eq(&actual, &expected));
    assert_eq!(actual.code, expected.code);
}
#[test]
fn undo_nested_phase_move() {
    let schema =
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
            PlanningVariableDefinition::list::<&'static str>("valueList", false),
        ])])
        .unwrap();
    let field = schema
        .genuine_entity::<Entity>()
        .unwrap()
        .genuine_variable("valueList")
        .unwrap();
    // Source mocks present inconsistent duplicate membership, while reporting
    // the pre/post windows [v1]/[v2] -> []/[v2,v1]. Use those coherent windows
    // with actual native validation and a real nested construction phase.
    let p = ListProblem::new(
        vec!["v1", "v2"],
        vec![ListEntity::new(vec!["v1"]), ListEntity::new(vec!["v2"])],
    )
    .unwrap();
    let mut network = ListConstraintNetwork::new(2, 2);
    network
        .add_constraint("v1 receiver", vec![ListScope::Value(0)], |facts| {
            let ListFact::Value { element, .. } = &facts[0] else {
                unreachable!()
            };
            SimpleScore::new(element.as_ref().map_or(0, |v| {
                v.position.entity as i64 * 100 + v.position.index as i64
            }))
        })
        .unwrap();
    let mut director = ListScoreDirector::new(&p, &network, p.initial_assignment()).unwrap();
    let recording = ListChangeRecorder::<usize, &'static str>::new(field).unwrap();
    let undo = recording.undo_handle();
    {
        let mut ephemeral = director.edit_session().unwrap();
        recording
            .before(0, 0, 1, &ephemeral.assignment().lists()[0])
            .unwrap();
        ephemeral
            .apply(&ListMove::Unassign {
                entity: 0,
                start: 0,
                end: 1,
            })
            .unwrap();
        recording
            .after(0, 0, 0, &ephemeral.assignment().lists()[0])
            .unwrap();
        let before_nested = ephemeral.assignment().clone();
        let result = ephemeral
            .run_solver(SolverSequence {
                limits: Default::default(),
                phases: vec![SolverPhase::Construction(ConstructionConfig {
                    max_steps: Some(1),
                    assert_scores: true,
                    ..Default::default()
                })],
            })
            .unwrap();
        assert!(result.complete);
        assert_eq!(ephemeral.assignment().lists(), &[vec![], vec!["v2", "v1"]]);
        recording
            .record_phase_transition(before_nested.lists(), ephemeral.assignment().lists())
            .unwrap();
        let actions = recording.recorded_before().unwrap();
        assert!(actions.iter().any(|a| a.entity == 0
            && a.from == 0
            && a.original_to == 1
            && a.old_values.len() == 1
            && a.old_values[0] == "v1"));
        assert!(actions.iter().any(|a| a.entity == 1
            && a.from == 0
            && a.original_to == 1
            && a.old_values.len() == 1
            && a.old_values[0] == "v2"));
        let mut lists = ephemeral
            .assignment()
            .lists()
            .iter()
            .cloned()
            .enumerate()
            .collect();
        undo.execute(&mut lists, |_, _| {}).unwrap();
        assert_eq!(lists[&0], vec!["v1"]);
        assert_eq!(lists[&1], vec!["v2"]);
    }
    assert_eq!(director.assignment(), &p.initial_assignment());
    director.assert_consistent().unwrap();
}
#[test]
fn test_solver_scope_nested_phase() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec!["v1", "v2"]).with_initial("v1"),
        Variable::new(vec!["v1", "v2"]).with_initial("v2"),
    ])
    .unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let mut main_scope = ScoringContextManager::new(
        d,
        EnvironmentMode::NoAssert,
        vec![],
        ContextMetrics::default(),
        |_: &mut ScoreDirector<'_, '_, &'static str, SimpleScore>, _: ContextSettings| {
            unreachable!("no mode transitions")
        },
    );
    main_scope.start().unwrap();
    let main = main_scope.handle();
    let called = std::cell::Cell::new(0);
    let used_main = std::cell::Cell::new(0);
    main_scope
        .change_director()
        .edit(|director| {
            let mut ephemeral = director.edit_session().unwrap();
            ephemeral
                .apply(&Move::Change {
                    variable: 0,
                    value: None,
                })
                .unwrap();
            let result = ephemeral
                .run_solver_in_context(
                    SolverSequence {
                        limits: Default::default(),
                        phases: vec![SolverPhase::Construction(Default::default())],
                    },
                    |nested| {
                        called.set(called.get() + 1);
                        if main.same_resource_as(nested) {
                            used_main.set(used_main.get() + 1);
                        }
                        assert!(nested.calculation_count() > 0);
                        assert!(
                            nested
                                .director()
                                .unwrap()
                                .director()
                                .assignment()
                                .values()
                                .iter()
                                .all(Option::is_some)
                        );
                    },
                )
                .unwrap();
            assert!(result.complete);
            Ok(())
        })
        .unwrap();
    assert_eq!(used_main.get(), 0);
    assert_eq!(called.get(), 1);
    assert_eq!(main.calculation_count(), 0);
    assert_eq!(
        main.director().unwrap().assignment(),
        &p.initial_assignment()
    );
    main_scope.finish().unwrap();
}
