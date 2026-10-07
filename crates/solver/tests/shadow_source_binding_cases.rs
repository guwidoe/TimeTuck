// Adapted from MixedModelTest and TestdataMixed* fixtures. Apache-2.0.
// See compat/provenance.json. Typed paths and native mixed moves replace reflection.
use std::{any::Any, sync::Arc};
use timetuck::*;
struct Solution;
struct Entity;
#[derive(Clone)]
struct Value {
    id: usize,
    delay: i32,
    previous: Option<Arc<Value>>,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
fn world(assignment: &MixedAssignment<i32, usize>) -> Vec<Value> {
    let mut rows = (0..2)
        .map(|id| Value {
            id,
            delay: assignment.basic().values()[id].unwrap(),
            previous: None,
        })
        .collect::<Vec<_>>();
    for list in assignment.lists().lists() {
        for pair in list.windows(2) {
            rows[pair[1]].previous = Some(Arc::new(rows[pair[0]].clone()));
        }
    }
    rows
}
#[test]
fn changing_variable_of_parent_changes_dependent_variable_of_children() {
    let schema = PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Entity>(vec![PlanningVariableDefinition::list::<Value>(
            "valueList",
            false,
        )]),
        PlanningEntityDefinition::of::<Value>(vec![
            PlanningVariableDefinition::basic::<i32>("delay", false),
            PlanningVariableDefinition::shadow::<Entity>("entity"),
            PlanningVariableDefinition::shadow::<Value>("previous"),
            PlanningVariableDefinition::shadow::<i32>("previousDelay"),
        ]),
    ])
    .unwrap();
    let mut access = SourcePathRegistry::new(&schema);
    access
        .one("previous", SourceMemberRole::Previous, |v: &Value| {
            v.previous.as_deref()
        })
        .unwrap();
    access
        .one("delay", SourceMemberRole::Variable, |v: &Value| {
            Some(&v.delay)
        })
        .unwrap();
    let direct = access
        .compile::<Value>("previousDelay", "previous")
        .unwrap();
    let path = access
        .compile::<Value>("previousDelay", "previous.delay")
        .unwrap();
    let value = schema.entity::<Value>().unwrap();
    let delay = value.variable("delay").unwrap();
    let mut b = ShadowSessionBuilder::<usize, Vec<Value>>::new(&schema);
    let mut targets = vec![];
    for id in 0..2 {
        b.input(delay, id, move |w| Some(w[id].delay)).unwrap();
        b.input(value.variable("previous").unwrap(), id, move |w| {
            w[id].previous.as_deref().cloned()
        })
        .unwrap();
        let output = b
            .derive(value.variable("previousDelay").unwrap(), id, move |_, i| {
                i.for_field::<i32>(delay)
                    .unwrap()
                    .first()
                    .copied()
                    .flatten()
                    .copied()
            })
            .unwrap();
        b.bind_source_paths(
            &output,
            vec![&direct, &path],
            move |w| &w[id],
            |v: &dyn Any| v.downcast_ref::<Value>().map(|v| v.id),
        )
        .unwrap();
        targets.push(output);
    }
    let mut session = b.build();
    let problem = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![1, 2]).with_initial(1),
            Variable::new(vec![1, 2]).with_initial(2),
        ])
        .unwrap(),
        ListProblem::new(vec![0usize, 1], vec![ListEntity::new(vec![0, 1])]).unwrap(),
    );
    let mut assignment = problem.initial_assignment();
    let initial = world(&assignment);
    session.update(&initial).unwrap();
    assert_eq!(session.read(&targets[1]).unwrap().value, Some(&1));
    let saved = session.checkpoint().unwrap();
    let trial = problem
        .apply(
            &mut assignment,
            &MixedMove {
                basic: Move::Change {
                    variable: 0,
                    value: Some(2),
                },
                lists: ListMove::Composite(vec![]),
            },
        )
        .unwrap();
    let changed = world(&trial);
    session.update(&changed).unwrap();
    assert_eq!(changed[0].delay, 2);
    assert_eq!(session.read(&targets[1]).unwrap().value, Some(&2));
    drop(trial);
    session.restore(&saved, &world(&assignment)).unwrap();
    assert_eq!(session.read(&targets[1]).unwrap().value, Some(&1));
}
