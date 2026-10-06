//! Original operational tests for typed source paths and dependency evaluation.
use std::sync::Arc;
use timetuck::*;
struct Schedule;
struct Node {
    id: usize,
    previous: Option<Arc<Node>>,
    group: Vec<Arc<Node>>,
}
fn node(id: usize, previous: Option<Arc<Node>>) -> Arc<Node> {
    Arc::new(Node {
        id,
        previous,
        group: vec![],
    })
}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Schedule>(vec![PlanningEntityDefinition::of::<Node>(vec![
        PlanningVariableDefinition::shadow::<Node>("previous"),
        PlanningVariableDefinition::shadow::<i64>("finish"),
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
        |_: &Node| -> Option<&i64> { panic!("terminal value getters must never run") },
    )
    .unwrap();
    a.many("group", SourceMemberRole::Fact, |n: &Node| {
        n.group.iter().map(Arc::as_ref).collect()
    })
    .unwrap();
    a
}
#[test]
fn compiled_paths_drive_incremental_evaluation_edge_changes_cycles_and_snapshot_restore() {
    let s = schema();
    let a = access(&s);
    let p = a.compile::<Node>("finish", "previous.finish").unwrap();
    let n0 = node(0, None);
    let n1 = node(1, Some(n0.clone()));
    let n2 = node(2, Some(n1.clone()));
    let dependencies = |n: &Node| {
        p.dependency_keys(n, |v, e| {
            (v.name() == "finish").then(|| e.downcast_ref::<Node>().unwrap().id)
        })
        .unwrap()
    };
    let mut topology = DependencyTopology::new(0..3).unwrap();
    let mut edges = vec![];
    for n in [&n0, &n1, &n2] {
        edges.extend(dependencies(n).into_iter().map(|dep| (n.id, dep)));
    }
    assert_eq!(edges, vec![(1, 0), (2, 1)]);
    topology.update(&[], &edges).unwrap();
    let mut cache = DependencyCache::new(topology, |_, inputs| {
        Some(inputs.first().map_or(1i64, |(_, v)| v.value.unwrap() + 1))
    });
    assert_eq!(cache.values()[&2].value, Some(3));
    let saved = cache.clone();
    let moved = node(2, Some(n0.clone()));
    let old = dependencies(&n2);
    let next = dependencies(&moved);
    let remove = old
        .iter()
        .filter(|k| !next.contains(k))
        .map(|&k| (2, k))
        .collect::<Vec<_>>();
    let add = next
        .iter()
        .filter(|k| !old.contains(k))
        .map(|&k| (2, k))
        .collect::<Vec<_>>();
    let mut calls = vec![];
    cache
        .update(&remove, &add, &[2], |key, inputs| {
            calls.push(*key);
            Some(inputs.first().map_or(1, |(_, v)| v.value.unwrap() + 1))
        })
        .unwrap();
    assert_eq!(calls, [2]);
    assert_eq!(cache.values()[&2].value, Some(2));
    assert_eq!(saved.values()[&2].value, Some(3));
    let cyclic1 = node(1, Some(n2.clone()));
    assert_eq!(dependencies(&cyclic1), [2]);
    cache
        .update(&[(1, 0), (2, 0)], &[(1, 2), (2, 1)], &[], |_, _| {
            panic!("cyclic suppliers must not run")
        })
        .unwrap();
    assert!(cache.values()[&1].inconsistent);
    assert!(cache.values()[&2].inconsistent);
    assert!(!cache.values()[&0].inconsistent);
    cache = saved;
    assert_eq!(cache.values()[&2].value, Some(3));
    assert!(!cache.values()[&2].inconsistent);
}
#[test]
fn traversal_handles_nulls_duplicate_groups_wrong_types_and_distinct_snapshots() {
    let s = schema();
    let a = access(&s);
    let direct = a.compile::<Node>("finish", "finish").unwrap();
    let chain = a.compile::<Node>("finish", "previous.finish").unwrap();
    let group = a.compile::<Node>("finish", "group[].finish").unwrap();
    let leaf = node(0, None);
    assert_eq!(direct.value_entities(leaf.as_ref()).unwrap().len(), 1);
    assert!(chain.value_entities(leaf.as_ref()).unwrap().is_empty());
    assert!(group.value_entities(leaf.as_ref()).unwrap().is_empty());
    let root = Node {
        id: 1,
        previous: Some(leaf.clone()),
        group: vec![leaf.clone(), leaf.clone()],
    };
    let values = group.value_entities(&root).unwrap();
    assert_eq!(values.len(), 2);
    for v in values {
        assert!(std::ptr::eq(
            v.downcast_ref::<Node>().unwrap(),
            leaf.as_ref()
        ));
    }
    assert_eq!(
        group
            .dependency_keys(&root, |_, e| Some(e.downcast_ref::<Node>().unwrap().id))
            .unwrap(),
        [0]
    );
    assert!(direct.value_entities(&0usize).is_err());
    assert!(
        chain
            .references()
            .next()
            .unwrap()
            .target_entities_from_variable(&0usize)
            .is_err()
    );
    let other = node(3, None);
    let root2 = Node {
        id: 1,
        previous: Some(other.clone()),
        group: vec![],
    };
    assert_eq!(
        chain.value_entities(&root).unwrap()[0]
            .downcast_ref::<Node>()
            .unwrap()
            .id,
        0
    );
    assert_eq!(
        chain.value_entities(&root2).unwrap()[0]
            .downcast_ref::<Node>()
            .unwrap()
            .id,
        3
    );
}
#[test]
fn registrations_fail_before_mutation_and_paths_reject_invalid_structure() {
    let s = schema();
    let mut a = SourcePathRegistry::new(&s);
    assert!(
        a.one("finish", SourceMemberRole::Declarative, |n: &Node| Some(
            &n.id
        ))
        .is_err()
    );
    assert!(
        a.many("previous", SourceMemberRole::Variable, |n: &Node| n
            .group
            .iter()
            .map(Arc::as_ref)
            .collect())
            .is_err()
    );
    assert!(
        a.one("finish", SourceMemberRole::Fact, |n: &Node| Some(&n.id))
            .is_err()
    );
    assert!(
        a.one("unknown", SourceMemberRole::Variable, |n: &Node| Some(
            &n.id
        ))
        .is_err()
    );
    assert!(
        a.one("bad.name", SourceMemberRole::Fact, |n: &Node| Some(&n.id))
            .is_err()
    );
    a.one("previous", SourceMemberRole::Previous, |n: &Node| {
        n.previous.as_deref()
    })
    .unwrap();
    assert!(
        a.one("previous", SourceMemberRole::Previous, |n: &Node| n
            .previous
            .as_deref())
            .is_err()
    );
    assert!(a.compile::<Node>("finish", "previous").is_ok());
    assert!(a.is_variable::<Node>("finish"));
    assert!(!a.is_variable::<Node>("group"));
    assert!(!a.is_variable::<usize>("finish"));
    for path in [
        "",
        "previous[]",
        "previous.",
        ".previous",
        "previous..previous",
        "previous[0]",
    ] {
        assert!(a.compile::<Node>("finish", path).is_err(), "{path}");
    }
    assert!(a.compile::<Node>("missing", "previous").is_err());
    let a = access(&s);
    assert!(
        a.compile::<Node>("finish", "previous.group[].finish")
            .is_err()
    );
}
