// Adapted from SolutionTrackerTest and TestdataSolution/Entity/Value/Object.
// Apache-2.0; see compat/provenance.json.
use std::collections::BTreeMap;
use timetuck::*;
struct TestdataSolution;
struct TestdataEntity;
type World = BTreeMap<String, Option<String>>;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<TestdataSolution>(vec![PlanningEntityDefinition::of::<TestdataEntity>(
        vec![PlanningVariableDefinition::basic::<String>("value", false)],
    )])
    .unwrap()
}
fn generated() -> World {
    (0..3)
        .map(|i| {
            (
                format!("Generated Entity {i}"),
                Some(format!("Generated Value {i}")),
            )
        })
        .collect()
}
fn tracker(
    s: &PlanningSchema,
) -> (
    SolutionTracker<'_, '_, String, World>,
    VariableEventTracker<'_, String>,
) {
    let mut t = SolutionTracker::new(s);
    let e = t
        .scalar(
            s.entity::<TestdataEntity>()
                .unwrap()
                .variable("value")
                .unwrap(),
            |w: &World| w.clone(),
            String::clone,
        )
        .unwrap();
    (t, e)
}
fn changed(w: &mut World) {
    w.insert("Generated Entity 0".into(), None);
    w.insert(
        "Generated Entity 1".into(),
        Some("Generated Value 0".into()),
    );
}
fn violation(entity: usize, actual: &str, expected: &str) -> String {
    format!(
        "Actual value ({actual}) of variable value on TestdataEntity entity (Generated Entity {entity}) differs from expected ({expected})"
    )
}
#[test]
fn format_list() {
    assert_eq!(format_tracking_list(&[] as &[&str]), "");
    assert_eq!(
        format_tracking_list(&["item 1", "item 2", "item 3"]),
        "  - item 1\n  - item 2\n  - item 3\n"
    );
    assert_eq!(
        format_tracking_list(&["item 1", "item 2", "item 3", "4", "5"]),
        "  - item 1\n  - item 2\n  - item 3\n  - 4\n  - 5\n"
    );
    assert_eq!(
        format_tracking_list(&["item 1", "item 2", "item 3", "4", "5", "6", "7", "8"]),
        "  - item 1\n  - item 2\n  - item 3\n  - 4\n  - 5\n  ...(3 more)\n"
    );
}
#[test]
fn variable_changed_violations() {
    let s = schema();
    let (t, _) = tracker(&s);
    let mut w = generated();
    let before = t.snapshot(&w);
    changed(&mut w);
    assert_eq!(
        before
            .differences(&t.snapshot(&w))
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        [
            violation(0, "null", "Generated Value 0"),
            violation(1, "Generated Value 0", "Generated Value 1")
        ]
    );
}
#[test]
fn variable_changed_violations_no_changes() {
    let s = schema();
    let (t, _) = tracker(&s);
    let w = generated();
    assert!(
        t.snapshot(&w)
            .differences(&t.snapshot(&w))
            .unwrap()
            .is_empty()
    );
}
fn corruption(forward: bool, undo: bool) {
    let s = schema();
    let (mut t, events) = tracker(&s);
    let mut w = generated();
    t.before_move(&w);
    for (entity, value) in [
        ("Generated Entity 0", None),
        ("Generated Entity 1", Some("Generated Value 0")),
    ] {
        if forward {
            events.before(entity.into());
        }
        w.insert(entity.into(), value.map(String::from));
        if forward {
            events.after(entity.into());
        }
    }
    t.after_move(&w).unwrap();
    if undo {
        events.before("Generated Entity 1".into());
    }
    w.insert(
        "Generated Entity 1".into(),
        Some("Generated Value 1".into()),
    );
    if undo {
        events.after("Generated Entity 1".into());
    }
    t.after_undo(&w).unwrap();
    w.insert("Generated Entity 1".into(), None);
    t.rebuilt_undo(&w).unwrap();
    t.rebuilt_before(&w).unwrap();
    let report = t.report().unwrap();
    assert_eq!(
        report
            .before_vs_undo
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        [violation(0, "null", "Generated Value 0")]
    );
    assert_eq!(
        report
            .rebuilt_vs_before
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        [
            violation(0, "Generated Value 0", "null"),
            violation(1, "Generated Value 1", "null")
        ]
    );
    assert_eq!(
        report
            .rebuilt_vs_undo
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        [violation(1, "Generated Value 1", "null")]
    );
    let message = report.to_string();
    for section in [
        "Variables that are different between before and undo:",
        "Variables that are different between from scratch and before",
        "Variables that are different between from scratch and undo",
    ] {
        assert!(message.contains(section));
    }
    for (good, header, entities, items) in [
        (
            forward,
            "Missing shadow variable update events for actual move",
            vec![0, 1],
            &report.forward_events,
        ),
        (
            undo,
            "Missing shadow variable update events for undo move",
            vec![1],
            &report.undo_events,
        ),
    ] {
        assert_eq!(message.contains(header), !good);
        let expected = if good {
            vec![]
        } else {
            entities.into_iter().flat_map(|i|[format!("Entity (Generated Entity {i}) is missing a beforeVariableChanged call for variable (value)."),format!("Entity (Generated Entity {i}) is missing a afterVariableChanged call for variable (value).")]).collect()
        };
        assert_eq!(
            items.iter().map(ToString::to_string).collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(t.report().unwrap(), report, "reports do not consume events");
}
#[test]
fn score_corruption_message() {
    corruption(false, false);
}
#[test]
fn score_corruption_good_shadow_variables() {
    corruption(true, true);
}
#[test]
fn score_corruption_good_forward_shadow_variables() {
    corruption(true, false);
}
#[test]
fn score_corruption_good_undo_shadow_variables() {
    corruption(false, true);
}
