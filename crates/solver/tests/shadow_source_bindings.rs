//! Original live compiled-path integration and transactional graph validation.
use std::{any::Any, cell::Cell, sync::Arc};
use timetuck::*;
struct Solution;
#[derive(Clone, Debug)]
struct Node {
    id: usize,
    previous: Option<Arc<Node>>,
    group: Vec<Arc<Node>>,
}
impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Node {}
fn node(id: usize, previous: Option<Arc<Node>>) -> Arc<Node> {
    Arc::new(Node {
        id,
        previous,
        group: vec![],
    })
}
fn key(value: &dyn Any) -> Option<usize> {
    value.downcast_ref::<Node>().map(|n| n.id)
}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Node>(vec![
        PlanningVariableDefinition::shadow::<Node>("previous"),
        PlanningVariableDefinition::shadow::<i64>("finish"),
        PlanningVariableDefinition::shadow::<i64>("other"),
    ])])
    .unwrap()
}
fn access(s: &PlanningSchema) -> SourcePathRegistry<'_> {
    let mut a = SourcePathRegistry::new(s);
    a.one("previous", SourceMemberRole::Previous, |n: &Node| {
        n.previous.as_deref()
    })
    .unwrap();
    a.one(
        "finish",
        SourceMemberRole::Declarative,
        |_: &Node| -> Option<&i64> { panic!("terminal getter") },
    )
    .unwrap();
    a.many("group", SourceMemberRole::Fact, |n: &Node| {
        n.group.iter().map(Arc::as_ref).collect()
    })
    .unwrap();
    a
}
#[test]
fn paths_rewire_latest_snapshots_detect_cycles_and_restore_without_suppliers() {
    let s = schema();
    let a = access(&s);
    let path = a.compile::<Node>("finish", "previous.finish").unwrap();
    let finish = s.entity::<Node>().unwrap().variable("finish").unwrap();
    let previous = s.entity::<Node>().unwrap().variable("previous").unwrap();
    let calls = &Cell::new(0);
    let mut b = ShadowSessionBuilder::<usize, Vec<Arc<Node>>>::new(&s);
    let mut outputs = vec![];
    for id in 0..3 {
        b.input(previous, id, move |w| w[id].previous.as_deref().cloned())
            .unwrap();
        let out = b
            .derive(finish, id, move |_, inputs| {
                calls.set(calls.get() + 1);
                Some(
                    inputs
                        .for_field::<i64>(finish)
                        .unwrap()
                        .first()
                        .copied()
                        .flatten()
                        .copied()
                        .unwrap_or(0)
                        + 1,
                )
            })
            .unwrap();
        b.bind_source_paths(&out, vec![&path], move |w| w[id].as_ref(), key)
            .unwrap();
        outputs.push(out);
    }
    let mut session = b.build();
    let n0 = node(0, None);
    let n1 = node(1, Some(n0.clone()));
    let n2 = node(2, Some(n1.clone()));
    let original = vec![n0.clone(), n1.clone(), n2.clone()];
    session.update(&original).unwrap();
    assert_eq!(calls.get(), 3);
    assert_eq!(session.read(&outputs[2]).unwrap().value, Some(&3));
    let saved = session.checkpoint().unwrap();
    let moved = vec![n0.clone(), n1.clone(), node(2, Some(n0.clone()))];
    session.update(&moved).unwrap();
    assert_eq!(session.read(&outputs[2]).unwrap().value, Some(&2));
    assert_eq!(calls.get(), 4);
    assert!(session.restore(&saved, &moved).is_err());
    let count = calls.get();
    session.restore(&saved, &original).unwrap();
    assert_eq!(calls.get(), count);
    assert_eq!(session.read(&outputs[2]).unwrap().value, Some(&3));
    let cyclic = vec![n0.clone(), node(1, Some(n2.clone())), n2.clone()];
    session.update(&cyclic).unwrap();
    assert!(session.read(&outputs[1]).unwrap().inconsistent);
    assert!(session.read(&outputs[2]).unwrap().inconsistent);
    assert_eq!(calls.get(), count);
    session.update(&original).unwrap();
    assert_eq!(session.read(&outputs[2]).unwrap().value, Some(&3));
    assert!(!session.read(&outputs[2]).unwrap().inconsistent);
    let null = vec![n0, n1, node(2, None)];
    session.update(&null).unwrap();
    assert_eq!(session.read(&outputs[2]).unwrap().value, Some(&1));
}
#[test]
fn fact_group_order_and_multiplicity_invalidate_even_when_dependency_set_is_unchanged() {
    let s = schema();
    let a = access(&s);
    let path = a.compile::<Node>("finish", "group[].finish").unwrap();
    let finish = s.entity::<Node>().unwrap().variable("finish").unwrap();
    let count = &Cell::new(0);
    let mut b = ShadowSessionBuilder::<usize, Vec<Arc<Node>>>::new(&s);
    for id in 0..2 {
        b.derive(finish, id, move |_, _| Some(id as i64 + 1))
            .unwrap();
    }
    let group = b
        .derive(finish, 2, move |w, i| {
            count.set(count.get() + 1);
            Some(
                i.for_field::<i64>(finish)
                    .unwrap()
                    .into_iter()
                    .flatten()
                    .sum::<i64>()
                    + w[2].group.len() as i64
                    + 10 * w[2].group.first().map_or(0, |n| n.id as i64),
            )
        })
        .unwrap();
    b.bind_source_paths(&group, vec![&path], |w| w[2].as_ref(), key)
        .unwrap();
    let mut session = b.build();
    let n0 = node(0, None);
    let n1 = node(1, None);
    let world = |group| {
        vec![
            n0.clone(),
            n1.clone(),
            Arc::new(Node {
                id: 2,
                previous: None,
                group,
            }),
        ]
    };
    let initial = world(vec![n0.clone(), n1.clone()]);
    session.update(&initial).unwrap();
    assert_eq!(session.read(&group).unwrap().value, Some(&5));
    let saved = session.checkpoint().unwrap();
    let repeated = world(vec![n0.clone(), n1.clone(), n1.clone()]);
    session.update(&repeated).unwrap();
    assert_eq!(session.read(&group).unwrap().value, Some(&6));
    assert_eq!(count.get(), 2);
    assert!(session.restore(&saved, &repeated).is_err());
    let reordered = world(vec![n1.clone(), n0.clone()]);
    session.update(&reordered).unwrap();
    assert_eq!(session.read(&group).unwrap().value, Some(&15));
    assert_eq!(count.get(), 3);
    session.update(&reordered).unwrap();
    assert_eq!(count.get(), 3);
    session.update(&world(vec![])).unwrap();
    assert_eq!(session.read(&group).unwrap().value, Some(&0));
    let before = count.get();
    session.restore(&saved, &initial).unwrap();
    assert_eq!(count.get(), before);
    assert_eq!(session.read(&group).unwrap().value, Some(&5));
}
#[test]
fn missing_nodes_wrong_roots_and_conflicting_bindings_fail_before_publication() {
    let s = schema();
    let a = access(&s);
    let path = a.compile::<Node>("finish", "previous.finish").unwrap();
    let wrong = a.compile::<Node>("other", "previous.finish").unwrap();
    let finish = s.entity::<Node>().unwrap().variable("finish").unwrap();
    let previous = s.entity::<Node>().unwrap().variable("previous").unwrap();
    let calls = &Cell::new(0);
    let mut b = ShadowSessionBuilder::<usize, Vec<Arc<Node>>>::new(&s);
    let input = b
        .input(previous, 0, |w| w[0].previous.as_deref().cloned())
        .unwrap();
    let out = b
        .derive(finish, 0, move |_, _| {
            calls.set(calls.get() + 1);
            Some(1i64)
        })
        .unwrap();
    assert!(
        b.bind_source_paths(&input, vec![&path], |w| w[0].as_ref(), key)
            .is_err()
    );
    assert!(
        b.bind_source_paths(&out, vec![&wrong], |w| w[0].as_ref(), key)
            .is_err()
    );
    b.bind_source_paths(&out, vec![&path], |w| w[0].as_ref(), key)
        .unwrap();
    assert!(b.set_dependencies(&out, &[input.erase()]).is_err());
    let mut session = b.build();
    session.update(&vec![node(0, None)]).unwrap();
    let before = calls.get();
    assert!(matches!(
        session.update(&vec![node(0, Some(node(99, None)))]),
        Err(ShadowSessionError::UnregisteredSource)
    ));
    assert!(session.update(&vec![node(9, None)]).is_err());
    assert_eq!(calls.get(), before);
    assert_eq!(session.read(&out).unwrap().value, Some(&1));
    assert!(session.set_dependencies(&out, &[]).is_err());
    assert!(session.update(&vec![node(0, None)]).unwrap().is_empty());
}
