//! Original permanent-source validation versus recoverable runtime cycles.
use std::{any::Any, cell::Cell, sync::Arc};
use timetuck::*;
struct Solution;
#[derive(Clone)]
struct Value {
    id: usize,
    previous: Option<Arc<Value>>,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Value>(vec![
        PlanningVariableDefinition::shadow::<i64>("a"),
        PlanningVariableDefinition::shadow::<i64>("b"),
        PlanningVariableDefinition::shadow::<i64>("dependent"),
        PlanningVariableDefinition::shadow::<Value>("previous"),
    ])])
    .unwrap()
}
fn access(s: &PlanningSchema) -> SourcePathRegistry<'_> {
    let mut a = SourcePathRegistry::new(s);
    for field in ["a", "b", "dependent"] {
        a.one(
            field,
            SourceMemberRole::Declarative,
            |_: &Value| -> Option<&i64> { panic!("terminal getter") },
        )
        .unwrap();
    }
    a.one("previous", SourceMemberRole::Previous, |v: &Value| {
        v.previous.as_deref()
    })
    .unwrap();
    a
}
fn key(v: &dyn Any) -> Option<usize> {
    v.downcast_ref::<Value>().map(|v| v.id)
}
#[test]
fn fixed_cycles_report_only_cycle_members_and_validation_invokes_no_callbacks() {
    let s = schema();
    let access = access(&s);
    let e = s.entity::<Value>().unwrap();
    let ab = access.compile::<Value>("a", "b").unwrap();
    let ba = access.compile::<Value>("b", "a").unwrap();
    let downstream = access.compile::<Value>("dependent", "a").unwrap();
    let count = &Cell::new(0);
    let mut builder = ShadowSessionBuilder::<usize, Value>::new(&s);
    for (field, path) in [("dependent", &downstream), ("a", &ab), ("b", &ba)] {
        let node = builder
            .derive(e.variable(field).unwrap(), 0, move |_, _| {
                count.set(count.get() + 1);
                Some(0i64)
            })
            .unwrap();
        builder
            .bind_source_paths(&node, vec![path], |w| w, key)
            .unwrap();
    }
    assert!(
        matches!(builder.validate_fixed_dependencies(),Err(ShadowSessionError::FixedDependencyCycle(names)) if names==["a","b"])
    );
    assert!(
        matches!(builder.try_build(),Err(ShadowSessionError::FixedDependencyCycle(names)) if names==["a","b"])
    );
    assert_eq!(count.get(), 0);
    let own = access.compile::<Value>("a", "a").unwrap();
    let mut builder = ShadowSessionBuilder::<usize, Value>::new(&s);
    let node = builder
        .derive(e.variable("a").unwrap(), 0, |_, _| Some(0i64))
        .unwrap();
    builder
        .bind_source_paths(&node, vec![&own], |w| w, key)
        .unwrap();
    assert!(
        matches!(builder.try_build(),Err(ShadowSessionError::FixedDependencyCycle(names)) if names==["a"])
    );
    let mut missing = ShadowSessionBuilder::<usize, Value>::new(&s);
    let node = missing
        .derive(e.variable("a").unwrap(), 0, |_, _| Some(0i64))
        .unwrap();
    missing
        .bind_source_paths(&node, vec![&ab], |w| w, key)
        .unwrap();
    assert!(matches!(
        missing.try_build(),
        Err(ShadowSessionError::UnregisteredSource)
    ));
}
#[test]
fn strict_stream_binding_allows_dynamic_cycles_and_recovers_after_undo() {
    let s = schema();
    let access = access(&s);
    let e = s.entity::<Value>().unwrap();
    let path = access.compile::<Value>("a", "previous.a").unwrap();
    let field = e.variable("a").unwrap();
    let count = &Cell::new(0);
    let mut builder = ShadowSessionBuilder::<usize, Vec<Value>>::new(&s);
    let mut nodes = vec![];
    for id in 0..2 {
        builder
            .input(e.variable("previous").unwrap(), id, move |w| {
                w[id].previous.as_deref().cloned()
            })
            .unwrap();
        let n = builder
            .derive(field, id, move |_, i| {
                count.set(count.get() + 1);
                Some(
                    i.for_field::<i64>(field)
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
        builder
            .bind_source_paths(&n, vec![&path], move |w| &w[id], key)
            .unwrap();
        nodes.push(n);
    }
    builder.validate_fixed_dependencies().unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Vec<Value>>();
    let derived = b
        .map_shadows(source.port(), builder, move |_, session| {
            nodes
                .iter()
                .map(|n| {
                    let v = session.read(n)?;
                    Ok((v.value.copied(), v.inconsistent))
                })
                .collect::<Result<Vec<_>, ShadowSessionError>>()
        })
        .unwrap();
    let matches = b
        .score("consistent", &derived, |values| {
            SimpleScore::new(
                values
                    .iter()
                    .map(|(v, bad)| if *bad { -100 } else { v.unwrap_or(0) })
                    .sum(),
            )
        })
        .unwrap();
    let mut graph = b.build();
    let head = Value {
        id: 0,
        previous: None,
    };
    let tail = Value {
        id: 1,
        previous: Some(Arc::new(head.clone())),
    };
    let normal = vec![head.clone(), tail.clone()];
    let cycle = vec![
        Value {
            id: 0,
            previous: Some(Arc::new(tail.clone())),
        },
        tail,
    ];
    {
        let mut t = graph.transaction();
        t.apply(&source, TupleChange::put(0, normal.clone()))
            .unwrap();
        t.commit().unwrap();
    }
    assert_eq!(graph.score(), SimpleScore::new(3));
    let before = count.get();
    {
        let mut t = graph.transaction();
        t.apply(&source, TupleChange::put(0, cycle.clone()))
            .unwrap();
        assert_eq!(t.score().unwrap(), SimpleScore::new(-200));
    }
    assert_eq!(count.get(), before);
    assert_eq!(graph.score(), SimpleScore::new(3));
    assert_eq!(
        graph.constraint_matches(&matches).unwrap()[&0].0,
        vec![(Some(1), false), (Some(2), false)]
    );
    {
        let mut t = graph.transaction();
        t.apply(&source, TupleChange::put(0, cycle)).unwrap();
        t.commit().unwrap();
    }
    {
        let mut t = graph.transaction();
        t.apply(&source, TupleChange::put(0, normal)).unwrap();
        t.commit().unwrap();
    }
    assert_eq!(graph.score(), SimpleScore::new(3));
    graph.assert_consistent().unwrap();
}
