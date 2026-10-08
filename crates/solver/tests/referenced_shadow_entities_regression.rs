//! Original transitive traversal, identity and manager atomicity checks.
use std::{any::Any, cell::Cell, sync::Arc};
use timetuck::*;
struct Solution;
struct Node {
    id: usize,
    group: Vec<Arc<Node>>,
    others: Vec<Other>,
}
struct Other {
    id: usize,
}
fn node(id: usize, group: Vec<Arc<Node>>) -> Arc<Node> {
    Arc::new(Node {
        id,
        group,
        others: vec![],
    })
}
fn key(e: &dyn Any) -> Option<usize> {
    e.downcast_ref::<Node>()
        .map(|n| n.id)
        .or_else(|| e.downcast_ref::<Other>().map(|n| n.id))
}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Node>(vec![
            PlanningVariableDefinition::shadow::<i64>("result"),
            PlanningVariableDefinition::shadow::<i64>("alternate"),
            PlanningVariableDefinition::shadow::<usize>("index"),
        ]),
        PlanningEntityDefinition::of::<Other>(vec![PlanningVariableDefinition::shadow::<i64>(
            "result",
        )]),
    ])
    .unwrap()
}
fn registry(schema: &PlanningSchema) -> SourcePathRegistry<'_> {
    let mut r = SourcePathRegistry::new(schema);
    r.many("group", SourceMemberRole::Fact, |n: &Node| {
        n.group.iter().map(Arc::as_ref).collect()
    })
    .unwrap();
    r.many("others", SourceMemberRole::Fact, |n: &Node| {
        n.others.iter().collect()
    })
    .unwrap();
    for field in ["result", "alternate"] {
        r.one(
            field,
            SourceMemberRole::Declarative,
            |_: &Node| -> Option<&i64> { panic!("terminal value getter") },
        )
        .unwrap();
    }
    r.one(
        "index",
        SourceMemberRole::Variable,
        |_: &Node| -> Option<&usize> { panic!("builtin terminal getter") },
    )
    .unwrap();
    r.one(
        "result",
        SourceMemberRole::Declarative,
        |_: &Other| -> Option<&i64> { panic!("other terminal getter") },
    )
    .unwrap();
    r
}
#[test]
fn report_walks_missing_entities_deduplicates_fields_and_distinguishes_types() {
    let s = schema();
    let r = registry(&s);
    let p = r.compile::<Node>("result", "group[].result").unwrap();
    let q = r.compile::<Node>("alternate", "group[].result").unwrap();
    let a = r.compile::<Node>("result", "group[].alternate").unwrap();
    let b = r.compile::<Node>("result", "group[].index").unwrap();
    let o = r.compile::<Node>("result", "others[].result").unwrap();
    let paths = [&p, &p, &q, &a, &b, &o];
    let leaf = node(3, vec![]);
    let middle = node(2, vec![leaf.clone()]);
    let root = Node {
        id: 1,
        group: vec![middle.clone(), middle.clone()],
        others: vec![Other { id: 2 }],
    };
    // Other(2) is present; Node(2) is missing. Identity includes concrete type.
    let given: [&dyn Any; 3] = [&root, &root, &root.others[0]];
    let report = missing_referenced_shadow_entities(&paths, &given, key).unwrap();
    assert_eq!(
        report
            .iter()
            .map(|e| (
                e.target.entity,
                e.target.variable.as_str(),
                e.source.entity,
                e.source.variable.as_str()
            ))
            .collect::<Vec<_>>(),
        [
            (1, "result", 2, "result"),
            (1, "alternate", 2, "result"),
            (1, "result", 2, "alternate"),
            (2, "result", 3, "result"),
            (2, "alternate", 3, "result"),
            (2, "result", 3, "alternate"),
        ]
    );
    assert!(
        report
            .iter()
            .all(|e| e.source.entity_type == PlanningType::of::<Node>())
    );
    assert!(
        missing_referenced_shadow_entities(
            &paths,
            &[&root, middle.as_ref(), leaf.as_ref(), &root.others[0]],
            key
        )
        .unwrap()
        .is_empty()
    );
    assert!(
        missing_referenced_shadow_entities(&paths, &[], key)
            .unwrap()
            .is_empty()
    );
    drop(root);
    drop(middle);
    drop(leaf);
    assert_eq!(report[3].source.entity, 3);
}
#[test]
fn invalid_identity_and_mixed_schemas_fail_instead_of_returning_partial_reports() {
    let s = schema();
    let r = registry(&s);
    let p = r.compile::<Node>("result", "group[].result").unwrap();
    let leaf = node(1, vec![]);
    let duplicate = node(1, vec![]);
    assert!(matches!(
        missing_referenced_shadow_entities(&[&p], &[leaf.as_ref(), duplicate.as_ref()], key),
        Err(ShadowSessionError::Metadata(_))
    ));
    let root = node(0, vec![leaf.clone(), duplicate]);
    assert!(matches!(
        missing_referenced_shadow_entities(&[&p], &[root.as_ref()], key),
        Err(ShadowSessionError::Metadata(_))
    ));
    assert!(matches!(
        missing_referenced_shadow_entities(&[&p], &[root.as_ref()], |e| key(e)
            .filter(|&id| id != 1)),
        Err(ShadowSessionError::UnregisteredSource)
    ));
    assert!(matches!(
        missing_referenced_shadow_entities(&[&p], &[&true], key),
        Err(ShadowSessionError::UnregisteredSource)
    ));
    let other_schema = schema();
    let other_registry = registry(&other_schema);
    let other_path = other_registry
        .compile::<Node>("result", "group[].result")
        .unwrap();
    assert!(matches!(
        missing_referenced_shadow_entities(&[&p, &other_path], &[leaf.as_ref()], key),
        Err(ShadowSessionError::Metadata(_))
    ));
}
#[test]
fn manager_preflight_preserves_uninitialized_and_ready_state_then_recovers() {
    let s = schema();
    let r = registry(&s);
    let p = r.compile::<Node>("result", "group[].result").unwrap();
    let calls = Cell::new(0);
    let mut b = ShadowSessionBuilder::<usize, Vec<Arc<Node>>>::new(&s);
    let h = b
        .input(
            s.entity::<Node>().unwrap().variable("result").unwrap(),
            0,
            |_| {
                calls.set(calls.get() + 1);
                Some(9i64)
            },
        )
        .unwrap();
    let project_node = h.clone();
    let mut m = ShadowSolutionManager::new(
        b.build(),
        move |_, s| {
            Ok::<_, ConsistentShadowUpdateError<usize>>(
                *s.read(&project_node).unwrap().value.unwrap(),
            )
        },
        |_, _| Ok(()),
        |_, v| Ok(*v),
    )
    .with_consistency_policy(ShadowConsistencyPolicy::AllowInconsistent)
    .with_preflight(|nodes| {
        let given = nodes
            .iter()
            .map(|n| n.as_ref() as &dyn Any)
            .collect::<Vec<_>>();
        validate_referenced_shadow_entities(&[&p], &given, key)
    });
    let invalid = node(0, vec![node(1, vec![node(2, vec![])])]);
    let mut solution = ManagedSolution::new(vec![invalid.clone()], -1i64);
    assert!(
        matches!(m.update(&mut solution),Err(ShadowSolutionError::Adapter(ConsistentShadowUpdateError::MissingSources(refs))) if refs.len()==2)
    );
    assert_eq!(calls.get(), 0);
    assert!(matches!(
        m.session().read(&h),
        Err(ShadowSessionError::NotCurrent)
    ));
    assert_eq!((solution.shadows, solution.score), (-1, None));
    solution.assignment = vec![node(0, vec![])];
    assert_eq!(m.update(&mut solution).unwrap(), Some(&9));
    let previous_calls = calls.get();
    solution.assignment = vec![invalid];
    for policy in [
        SolutionUpdatePolicy::Everything,
        SolutionUpdatePolicy::ShadowsOnly,
        SolutionUpdatePolicy::ScoreOnly,
    ] {
        assert!(matches!(
            m.update_with_policy(&mut solution, policy),
            Err(ShadowSolutionError::Adapter(
                ConsistentShadowUpdateError::MissingSources(_)
            ))
        ));
        assert_eq!((solution.shadows, solution.score), (9, Some(9)));
        assert_eq!(m.session().read(&h).unwrap().value, Some(&9));
        assert_eq!(calls.get(), previous_calls);
    }
    solution.assignment = vec![node(0, vec![])];
    assert_eq!(m.update(&mut solution).unwrap(), Some(&9));
}
