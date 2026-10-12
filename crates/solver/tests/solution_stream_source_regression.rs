//! Original concrete allocation/graph controls; no source-method credit yet.
use std::sync::Arc;
use timetuck::*;
struct Root;
struct Entity {
    code: String,
    value: Option<i64>,
}
struct Child {
    base: Entity,
}
struct FactBase {
    code: String,
}
struct FactChild {
    base: FactBase,
}
impl Clone for FactChild {
    fn clone(&self) -> Self {
        panic!("must not clone payload")
    }
}
fn layout() -> Arc<ComposedPlanningSolution> {
    let entities = PlanningComposition::new(vec![
        PlanningComponent::registered::<Entity>(vec![PlanningVariableDefinition::basic::<i64>(
            "value", false,
        )]),
        PlanningComponent::registered::<Child>(vec![PlanningVariableDefinition::basic::<i64>(
            "value", false,
        )]),
    ])
    .unwrap();
    Arc::new(
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<Root>(vec![
            SolutionMemberDefinition::entities(
                "entities",
                vec![PlanningType::of::<Entity>(), PlanningType::of::<Child>()],
                false,
            ),
            SolutionMemberDefinition::value_range::<i64>("values", "range")
                .as_problem_fact_collection(),
            SolutionMemberDefinition::problem_fact_collection::<FactChild>("facts"),
            SolutionMemberDefinition::score::<SimpleScore>("score"),
        ])])
        .unwrap()
        .compile::<Root>(
            &entities,
            &[PlanningType::of::<Entity>(), PlanningType::of::<Child>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap(),
    )
}
fn facts(layout: Arc<ComposedPlanningSolution>) -> BoundSolutionFacts {
    let a = Arc::new(FactChild {
        base: FactBase {
            code: "same".into(),
        },
    });
    let b = Arc::new(FactChild {
        base: FactBase {
            code: "same".into(),
        },
    });
    let mut reads = SolutionFactAccessors::new(layout).unwrap();
    reads
        .collection("values", |_: &Root| vec![Arc::new(1i64), Arc::new(2)])
        .unwrap();
    reads
        .collection("facts", move |_| vec![a.clone(), b.clone(), a.clone()])
        .unwrap();
    reads.read(&Root).unwrap()
}
fn bound(
    layout: &ComposedPlanningSolution,
    facts: Option<BoundSolutionFacts>,
) -> BoundBasicSolution<i64> {
    let cat = Arc::new(Child {
        base: Entity {
            code: "Cat".into(),
            value: Some(1),
        },
    });
    let animal = Arc::new(Entity {
        code: "Animal".into(),
        value: Some(1),
    });
    let dog = Arc::new(Child {
        base: Entity {
            code: "Dog".into(),
            value: Some(1),
        },
    });
    let entities = vec![SolutionCollection {
        member: "entities".into(),
        entities: vec![
            SolutionEntityRecord::new(cat.clone(), vec![("value".into(), cat.base.value)]),
            SolutionEntityRecord::new(animal.clone(), vec![("value".into(), animal.value)]),
            SolutionEntityRecord::new(dog.clone(), vec![("value".into(), dog.base.value)]),
        ],
    }];
    let values = vec![SolutionRange {
        member: "values".into(),
        values: vec![1, 2],
    }];
    let fields = vec![
        SolutionFieldRange::new::<Entity>("value", vec!["range".into()]),
        SolutionFieldRange::new::<Child>("value", vec!["range".into()]),
    ];
    match facts {
        Some(f) => layout
            .bind_basic_with_facts(entities, values, fields, f)
            .unwrap(),
        None => layout.bind_basic(entities, values, fields).unwrap(),
    }
}
#[test]
fn projected_actual_entities_feed_base_and_child_constraints() {
    let layout = layout();
    let bound = bound(&layout, Some(facts(layout.clone())));
    let mut registry = SolutionStreamRegistry::new(layout);
    registry.project::<Child, Entity>(|c| &c.base).unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let base = registry.source::<Entity, _>(&mut b).unwrap();
    let child = registry.source::<Child, _>(&mut b).unwrap();
    let base_constraint = b
        .score("base", base.port(), |_| SimpleScore::new(-1))
        .unwrap();
    let child_constraint = b
        .score("child", child.port(), |_| SimpleScore::new(-1))
        .unwrap();
    let base_rows = base.bound(&bound).unwrap();
    let child_rows = child.bound(&bound).unwrap();
    assert_eq!(base_rows.len(), 3);
    assert_eq!(child_rows.len(), 2);
    for (key, row) in &child_rows {
        let parent = base_rows.iter().find(|(k, _)| k == key).unwrap();
        assert!(Arc::ptr_eq(row.allocation(), parent.1.allocation()));
        assert!(std::ptr::eq(&row.get().base, parent.1.get()));
    }
    drop(bound);
    drop(registry);
    let mut graph = b.build();
    let mut tx = graph.transaction();
    for (key, row) in base_rows {
        tx.apply(base.source(), TupleChange::put(key, row)).unwrap();
    }
    for (key, row) in child_rows {
        tx.apply(child.source(), TupleChange::put(key, row))
            .unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(graph.score(), SimpleScore::new(-5));
    let mut names = graph
        .constraint_matches(&base_constraint)
        .unwrap()
        .values()
        .map(|(r, _)| r.get().code.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(names, ["Animal", "Cat", "Dog"]);
    let mut names = graph
        .constraint_matches(&child_constraint)
        .unwrap()
        .values()
        .map(|(r, _)| r.get().base.code.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(names, ["Cat", "Dog"]);
    graph.assert_consistent().unwrap();
}
#[test]
fn transitive_fact_projection_preserves_alias_dedup_and_distinct_equal_allocations() {
    let layout = layout();
    let facts = facts(layout.clone());
    let mut registry = SolutionStreamRegistry::new(layout);
    registry
        .project::<FactChild, FactBase>(|f| &f.base)
        .unwrap();
    registry.project::<FactBase, String>(|f| &f.code).unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = registry.source::<String, _>(&mut b).unwrap();
    b.score("strings", source.port(), |r| {
        SimpleScore::new(r.get().len() as i64)
    })
    .unwrap();
    let rows = source.facts(&facts).unwrap();
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0].0, rows[1].0);
    assert_eq!(rows[0].1.get(), rows[1].1.get());
    assert!(!Arc::ptr_eq(rows[0].1.allocation(), rows[1].1.allocation()));
    let row_copy = rows[0].1.clone();
    assert!(Arc::ptr_eq(row_copy.allocation(), rows[0].1.allocation()));
    drop(facts);
    drop(registry);
    let mut graph = b.build();
    let mut tx = graph.transaction();
    for (key, row) in rows {
        tx.apply(source.source(), TupleChange::put(key, row))
            .unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(graph.score(), SimpleScore::new(8));
    graph.assert_consistent().unwrap();
}
#[test]
fn invalid_types_projections_and_foreign_snapshots_fail_before_graph_data_reads() {
    let layout = layout();
    let mut registry = SolutionStreamRegistry::new(layout.clone());
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let error = match registry.source::<i32, _>(&mut b) {
        Err(e) => e,
        Ok(_) => panic!("unregistered type"),
    };
    assert!(error.to_string().contains("i32"));
    assert!(error.to_string().contains("assignable from"));
    assert!(matches!(
        registry.project::<i32, i64>(|_| panic!()),
        Err(SolutionStreamError::UnknownProjectionOwner(_))
    ));
    registry.project::<Child, Entity>(|c| &c.base).unwrap();
    assert!(matches!(
        registry.project::<Child, Entity>(|c| &c.base),
        Err(SolutionStreamError::DuplicateProjection)
    ));
    assert!(matches!(
        registry.project::<Entity, Child>(|_| panic!()),
        Err(SolutionStreamError::ProjectionCycle)
    ));
    let source = registry.source::<Entity, _>(&mut b).unwrap();
    assert!(matches!(
        source.facts(&facts(self::layout())),
        Err(SolutionStreamError::DifferentDescriptor)
    ));
    assert!(matches!(
        source.bound(&bound(&layout, None)),
        Err(SolutionStreamError::MissingFactSnapshot)
    ));
}
