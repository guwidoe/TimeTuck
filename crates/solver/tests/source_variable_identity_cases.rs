// Adapted from pinned Community RootVariableSourceTest,
// BasicVariableTrackerTest and ListVariableTrackerTest (Apache-2.0).
// See compat/provenance.json. Canonical Rust schema fields replace Java
// descriptors; native entity keys replace fixture object references.
use timetuck::*;

// Fixture projections retain the planning-type and field identities used by
// these assertions. Getters, inheritance and unobserved fixture state are not
// reproduced: Entity.value is a variable, while Entity.code and Fact.code are facts.
struct Solution;
struct Entity;
struct Fact;
struct LavishEntity;
struct ListEntity;
struct HistoryListEntity;

fn basic_schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i32>("value", false),
    ])])
    .unwrap()
}

#[test]
fn is_variable_is_true_for_variable_on_entity() {
    let schema = basic_schema();
    let source = SourcePathRegistry::new(&schema);
    assert!(source.is_variable::<Entity>("value"));
}

#[test]
fn is_variable_is_false_for_fact_on_entity() {
    let schema = basic_schema();
    let source = SourcePathRegistry::new(&schema);
    assert!(!source.is_variable::<Entity>("code"));
}

#[test]
fn is_variable_is_false_for_fact_class() {
    let schema = basic_schema();
    let source = SourcePathRegistry::new(&schema);
    assert!(!source.is_variable::<Fact>("code"));
}

#[test]
fn basic_tracker_does_not_include_missing_events_for_other_variables() {
    let schema = basic_schema();
    let other_schema =
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<LavishEntity>(vec![
            PlanningVariableDefinition::basic::<i32>("value", false),
        ])])
        .unwrap();
    let variable = schema
        .entity::<Entity>()
        .unwrap()
        .variable("value")
        .unwrap();
    let other = other_schema
        .entity::<LavishEntity>()
        .unwrap()
        .variable("value")
        .unwrap();
    let tracker = VariableEventTracker::new(variable);
    tracker.before("a");
    tracker.after("a");
    assert!(tracker.audit([(variable, "a"), (other, "b")]).is_empty());
}

#[test]
fn list_tracker_does_not_include_missing_events_for_other_variables() {
    let schema =
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<ListEntity>(vec![
            PlanningVariableDefinition::list::<i32>("valueList", false),
        ])])
        .unwrap();
    let other_schema =
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<HistoryListEntity>(
            vec![PlanningVariableDefinition::list::<i32>("valueList", false)],
        )])
        .unwrap();
    let variable = schema
        .entity::<ListEntity>()
        .unwrap()
        .variable("valueList")
        .unwrap();
    let other = other_schema
        .entity::<HistoryListEntity>()
        .unwrap()
        .variable("valueList")
        .unwrap();
    let tracker = VariableEventTracker::new(variable);
    // Source calls both list notifications with [0, 1). The native tracker records
    // their presence per entity/field; the source assertion does not inspect ranges.
    tracker.before("a");
    tracker.after("a");
    assert!(tracker.audit([(variable, "a"), (other, "b")]).is_empty());
}

// Original regression; no upstream method credit. Equal field names, equal
// entity keys and equal type names must not merge distinct schema/field identity.
#[test]
fn same_named_fields_across_types_and_schemas_do_not_share_event_windows() {
    for list in [false, true] {
        let field = |name| {
            if list {
                PlanningVariableDefinition::list::<i32>(name, false)
            } else {
                PlanningVariableDefinition::basic::<i32>(name, false)
            }
        };
        let schema = PlanningSchema::new::<Solution>(vec![
            PlanningEntityDefinition::of::<Entity>(vec![field("value"), field("backup")]),
            PlanningEntityDefinition::of::<LavishEntity>(vec![field("value")]),
        ])
        .unwrap();
        let foreign = PlanningSchema::new::<Solution>(vec![
            PlanningEntityDefinition::of::<Entity>(vec![field("value")]),
        ])
        .unwrap();
        let owner = schema.entity::<Entity>().unwrap();
        let variable = owner.variable("value").unwrap();
        let backup = owner.variable("backup").unwrap();
        let other_type = schema
            .entity::<LavishEntity>()
            .unwrap()
            .variable("value")
            .unwrap();
        let other_schema = foreign
            .entity::<Entity>()
            .unwrap()
            .variable("value")
            .unwrap();
        let tracker = VariableEventTracker::new(variable);
        let other_tracker = VariableEventTracker::new(other_schema);
        tracker.before(1);
        tracker.after(1);
        assert!(
            tracker
                .audit([
                    (other_schema, 1),
                    (other_type, 1),
                    (backup, 1),
                    (variable, 1)
                ])
                .is_empty()
        );
        let missing = vec![
            VariableEventViolation {
                entity: 1,
                missing: MissingVariableEvent::Before,
            },
            VariableEventViolation {
                entity: 1,
                missing: MissingVariableEvent::After,
            },
        ];
        assert_eq!(other_tracker.audit([(other_schema, 1)]), missing);
        assert_eq!(
            tracker.audit([(variable, 1)]),
            missing,
            "audit clears only its own completed event window"
        );
        let sources = SourcePathRegistry::new(&schema);
        assert!(sources.is_variable::<Entity>("backup"));
        assert!(sources.is_variable::<LavishEntity>("value"));
        assert!(!sources.is_variable::<LavishEntity>("backup"));
        assert!(!sources.is_variable::<Fact>("value"));
    }
}
