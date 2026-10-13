// Adapted from Community RecordedUndoMoveTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
struct Solution;
#[test]
fn execute_replays_actions_in_reverse_recording_order() {
    let schema = PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<usize>(
        ["action1", "action2", "action3"]
            .into_iter()
            .map(|name| PlanningVariableDefinition::basic::<usize>(name, false))
            .collect(),
    )])
    .unwrap();
    let mut registry = VariableStateRegistry::new(&schema);
    let mut actions = vec![];
    for (index, name) in ["action1", "action2", "action3"].into_iter().enumerate() {
        let field = schema.entity::<usize>().unwrap().variable(name).unwrap();
        registry
            .reset_basic(field, [(index, Some(100usize))])
            .unwrap();
        actions.push(BasicRestoreAction::new(field, index, Some(index)).unwrap());
    }
    let mut undo = RecordedBasicUndo::new(actions);
    let mut call_order = vec![];
    undo.execute(&mut registry, |action, registry| {
        assert_eq!(
            registry
                .basic::<usize, usize>(action.variable)
                .unwrap()
                .snapshot()[&action.entity],
            action.value
        );
        call_order.push(action.variable.name().to_owned());
    })
    .unwrap();
    assert_eq!(call_order, ["action3", "action2", "action1"]);
}
