// Adapted from Timefold Community AffectedEntitiesUpdaterTest.java,
// createNodeToEntityNodes. Apache-2.0; see compat/provenance.json.
// Native registered fields replace GraphNode mocks; entity/node identities and
// the complete shuffled traversal and membership assertions are preserved.
use timetuck::*;
struct Solution;
struct Entity;
#[test]
fn create_node_to_entity_nodes() {
    let names = ["a1", "a2", "b1", "b2", "b3", "c1"];
    let schema = PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(
        names
            .iter()
            .map(|&name| PlanningVariableDefinition::shadow::<i64>(name))
            .collect(),
    )])
    .unwrap();
    let entity = schema.entity::<Entity>().unwrap();
    let mut builder = ShadowSessionBuilder::<&str, ()>::new(&schema);
    let mut nodes = Vec::new();
    for (name, owner) in names.iter().zip(["a", "a", "b", "b", "b", "c"]) {
        nodes.push(
            builder
                .derive(entity.variable(name).unwrap(), owner, |_, _| Some(0i64))
                .unwrap(),
        );
    }
    let session = builder.build();
    let traversal = [0, 2, 1, 5, 4, 3].map(|i| nodes[i].erase());
    let result = session.affected_entities(&traversal).unwrap();
    let ids: Vec<Vec<_>> = result
        .iter()
        .map(|group| {
            group
                .iter()
                .map(|node| {
                    names
                        .iter()
                        .position(|name| *name == session.variable(node).unwrap().name())
                        .unwrap()
                })
                .collect()
        })
        .collect();
    assert_eq!(ids[0], [0, 1]);
    assert_eq!(ids[1], [2, 3, 4]);
    assert_eq!(ids[2], [5]);
}
