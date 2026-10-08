// Adapted from pinned Timefold Community MoveDirectorTest and test domain fields.
// SPDX-License-Identifier: Apache-2.0
// Typed field bindings replace Java reflection; list positions are derived live.
use timetuck::*;
struct Solution;
struct Entity;
fn schema(list: bool, optional: bool) -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        if list {
            PlanningVariableDefinition::list::<&'static str>("valueList", optional)
        } else {
            PlanningVariableDefinition::basic::<&'static str>("value", optional)
        },
    ])])
    .unwrap()
}
fn binding(s: &PlanningSchema, list: bool, slots: Vec<usize>) -> SchemaVariableSlots<'_> {
    SchemaVariableSlots {
        variable: s
            .genuine_entity::<Entity>()
            .unwrap()
            .genuine_variable(if list { "valueList" } else { "value" })
            .unwrap(),
        slots,
    }
}
#[test]
fn read_basic_variable() {
    let s = schema(false, false);
    let p = FiniteProblem::new(vec![Variable::new(vec!["value"]).with_initial("value")]).unwrap();
    let field = binding(&s, false, vec![0]).bind_basic(&p).unwrap();
    assert_eq!(
        field.read(&p.initial_assignment(), 0).unwrap(),
        Some(&"value")
    );
}
#[test]
fn get_value_range_includes_null_for_unassigned_allowing_variable() {
    let s = schema(false, true);
    let p = FiniteProblem::new(
        (0..2)
            .map(|i| {
                Variable::new(vec!["Generated Value 0", "Generated Value 1"])
                    .allowing_unassigned()
                    .with_initial(if i == 0 {
                        "Generated Value 0"
                    } else {
                        "Generated Value 1"
                    })
            })
            .collect(),
    )
    .unwrap();
    let field = binding(&s, false, vec![0, 1]).bind_basic(&p).unwrap();
    assert!(field.range(0).unwrap().contains(&None));
    assert!(field.is_value_in_range(0, None).unwrap());
}
#[test]
fn read_list_variable() {
    let s = schema(true, false);
    let p = ListProblem::new(
        vec!["value1", "value2"],
        vec![ListEntity::new(vec!["value1", "value2"])],
    )
    .unwrap();
    let field = binding(&s, true, vec![0]).bind_list(&p).unwrap();
    let a = p.initial_assignment();
    assert_eq!(
        field
            .value_at(&a, FieldListPosition { row: 0, index: 0 })
            .unwrap(),
        &"value1"
    );
    assert_eq!(
        field.position_of(&a, &"value2").unwrap(),
        Some(FieldListPosition { row: 0, index: 1 })
    );
}
#[test]
fn get_first_unpinned_index_defaults_to_zero() {
    let s = schema(true, false);
    let p = ListProblem::new(
        vec!["value1", "value2"],
        vec![ListEntity::new(vec!["value1", "value2"])],
    )
    .unwrap();
    let field = binding(&s, true, vec![0]).bind_list(&p).unwrap();
    assert_eq!(
        field
            .first_unpinned_index(&p.initial_assignment(), 0)
            .unwrap(),
        0
    );
}
#[test]
fn get_first_unpinned_index_reads_planning_pin_to_index() {
    let s = schema(true, false);
    let p = ListProblem::new(
        vec!["value1", "value2", "value3"],
        vec![ListEntity::new(vec!["value1", "value2", "value3"]).with_pinned_prefix(2)],
    )
    .unwrap();
    let field = binding(&s, true, vec![0]).bind_list(&p).unwrap();
    assert_eq!(
        field
            .first_unpinned_index(&p.initial_assignment(), 0)
            .unwrap(),
        2
    );
}
