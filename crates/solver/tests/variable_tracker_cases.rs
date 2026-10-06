// Adapted from BasicVariableTrackerTest and ListVariableTrackerTest.
// Apache-2.0; see compat/provenance.json. Structured violations use stable entity keys.
use timetuck::*;
struct Solution;
struct Entity;
fn check(list: bool, before: bool, after: bool, reset: bool) {
    let s = PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        if list {
            PlanningVariableDefinition::list::<i32>("valueList", false)
        } else {
            PlanningVariableDefinition::basic::<i32>("value", false)
        },
    ])])
    .unwrap();
    let v = s.entity::<Entity>().unwrap().variables().next().unwrap();
    let mut r = VariableStateRegistry::new(&s);
    let t = r.tracker::<&str, i32>(v).unwrap();
    t.before("a");
    t.after("a");
    if before {
        t.before("b");
    }
    if after {
        t.after("b");
    }
    let mut expected = vec![];
    if !before {
        expected.push(VariableEventViolation {
            entity: "b",
            missing: MissingVariableEvent::Before,
        });
    }
    if !after {
        expected.push(VariableEventViolation {
            entity: "b",
            missing: MissingVariableEvent::After,
        });
    }
    assert_eq!(t.audit([(v, "a"), (v, "b")]), expected);
    if reset {
        assert_eq!(
            t.audit([(v, "a"), (v, "b")]),
            vec![
                VariableEventViolation {
                    entity: "a",
                    missing: MissingVariableEvent::Before
                },
                VariableEventViolation {
                    entity: "a",
                    missing: MissingVariableEvent::After
                },
                VariableEventViolation {
                    entity: "b",
                    missing: MissingVariableEvent::Before
                },
                VariableEventViolation {
                    entity: "b",
                    missing: MissingVariableEvent::After
                }
            ]
        );
    }
}
#[test]
fn basic_missing_before() {
    check(false, false, true, false);
}
#[test]
fn basic_missing_after() {
    check(false, true, false, false);
}
#[test]
fn basic_missing_both() {
    check(false, false, false, false);
}
#[test]
fn basic_complete() {
    check(false, true, true, false);
}
#[test]
fn basic_reset() {
    check(false, true, true, true);
}
#[test]
fn list_missing_before() {
    check(true, false, true, false);
}
#[test]
fn list_missing_after() {
    check(true, true, false, false);
}
#[test]
fn list_missing_both() {
    check(true, false, false, false);
}
#[test]
fn list_complete() {
    check(true, true, true, false);
}
#[test]
fn list_reset() {
    check(true, true, true, true);
}
