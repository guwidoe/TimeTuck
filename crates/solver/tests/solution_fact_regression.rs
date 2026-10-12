//! Original fact-reading and real optimization controls; no source-method credit.
#[path = "fixtures/solution_fact_fixture.rs"]
mod fixture;
use fixture::*;
use std::{cell::Cell, rc::Rc, sync::Arc};
use timetuck::*;
fn value(code: &str) -> Arc<Value> {
    Arc::new(Value { code: code.into() })
}
fn data() -> Properties {
    Properties {
        code: "s1".into(),
        value_list: vec![value("v1")],
        other_problem_fact_list: vec![],
        extra_object: None,
        entity_list: vec![],
        score: None,
    }
}
fn readers(layout: Arc<ComposedPlanningSolution>) -> SolutionFactAccessors<'static, Properties> {
    let mut r = SolutionFactAccessors::new(layout).unwrap();
    r.collection("valueList", |s: &Properties| s.value_list.clone())
        .unwrap();
    r.heterogeneous("otherProblemFactList", |s| {
        s.other_problem_fact_list.clone()
    })
    .unwrap();
    r.singleton("extraObject", |s| {
        s.extra_object
            .as_ref()
            .and_then(|r| r.object().clone().downcast::<Opaque>().ok())
    })
    .unwrap();
    r
}
struct Opaque {
    amount: i64,
}
impl Clone for Opaque {
    fn clone(&self) -> Self {
        panic!("fact payload must not be cloned")
    }
}
impl PartialEq for Opaque {
    fn eq(&self, _: &Self) -> bool {
        panic!("fact identity must not use payload equality")
    }
}
impl Eq for Opaque {}
impl std::hash::Hash for Opaque {
    fn hash<H: std::hash::Hasher>(&self, _: &mut H) {
        panic!("fact identity must not hash the payload")
    }
}
#[test]
fn real_allocations_keep_aliases_equal_values_and_snapshot_lifetimes() {
    let layout = Arc::new(properties());
    let a = Arc::new(Opaque { amount: 3 });
    let b = Arc::new(Opaque { amount: 3 });
    let mut s = data();
    s.value_list.push(s.value_list[0].clone());
    s.value_list.push(value("v1"));
    s.other_problem_fact_list = vec![
        SolutionFactRecord::new(a.clone()),
        SolutionFactRecord::new(b.clone()),
        SolutionFactRecord::new(a.clone()),
    ];
    s.extra_object = Some(SolutionFactRecord::new(a.clone()));
    let registry = readers(layout);
    let view = registry.read(&s).unwrap();
    assert_eq!(view.collection("valueList"), Some([0, 0, 1].as_slice()));
    assert_eq!(
        view.collection("otherProblemFactList"),
        Some([2, 3, 2].as_slice())
    );
    assert_eq!(view.collection("extraObject"), Some([2].as_slice()));
    assert_eq!(view.records().len(), 4);
    assert_eq!(view.facts_of::<Opaque>().count(), 2);
    assert!(std::ptr::eq(
        view.records()[2].object_ref::<Opaque>().unwrap(),
        a.as_ref()
    ));
    s.value_list.clear();
    s.other_problem_fact_list.clear();
    s.extra_object = None;
    let next = registry.read(&s).unwrap();
    assert!(next.records().is_empty());
    drop(registry);
    drop(s);
    drop(a);
    drop(b);
    assert_eq!(view.facts_of::<Opaque>().map(|p| p.amount).sum::<i64>(), 6);
    assert_eq!(view.facts_of::<Value>().count(), 2);
}
#[test]
fn computed_collections_are_invoked_and_each_read_owns_its_membership() {
    let mut m = standard();
    m.push(SolutionMemberDefinition::problem_fact_collection::<String>(
        "createProblemFacts",
    ));
    let layout = Arc::new(compile::<ReadMethod>(m, &[t::<Entity>()]).unwrap());
    let mut registry = SolutionFactAccessors::new(layout).unwrap();
    let calls = Rc::new(Cell::new(0));
    let counter = calls.clone();
    registry
        .collection("valueList", |s: &ReadMethod| s.value_list.clone())
        .unwrap();
    registry
        .collection("createProblemFacts", move |s| {
            counter.set(counter.get() + 1);
            s.create_problem_facts()
        })
        .unwrap();
    let mut s = ReadMethod {
        code: "s".into(),
        value_list: vec![value("v1")],
        entity_list: vec![],
        score: None,
    };
    let a = registry.read(&s).unwrap();
    s.value_list.push(value("v2"));
    let b = registry.read(&s).unwrap();
    assert_eq!(calls.get(), 2);
    assert_eq!(
        a.facts_of::<String>()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    assert_eq!(a.facts_of::<Value>().count(), 1);
    assert_eq!(b.facts_of::<Value>().count(), 2);
    assert!(!Arc::ptr_eq(
        a.records()[1].object(),
        b.records()[2].object()
    ));
    assert_eq!(a.collection("valueList"), Some([0].as_slice()));
}
#[test]
fn invalid_registration_and_missing_reads_do_not_invoke_callbacks_or_replace_readers() {
    let layout = Arc::new(properties());
    assert!(matches!(
        SolutionFactAccessors::<ReadMethod>::new(layout.clone()),
        Err(SolutionFactError::SolutionType { .. })
    ));
    let mut r = SolutionFactAccessors::new(layout).unwrap();
    let calls = Rc::new(Cell::new(0));
    let c = calls.clone();
    r.collection("valueList", move |s: &Properties| {
        c.set(c.get() + 1);
        s.value_list.clone()
    })
    .unwrap();
    assert!(matches!(
        r.collection::<String, _>("valueList", |_| vec![Arc::new("wrong".to_owned())]),
        Err(SolutionFactError::DuplicateAccessor(_))
    ));
    assert!(matches!(
        r.collection::<String, _>("score", |_| vec![]),
        Err(SolutionFactError::WrongRole(_))
    ));
    assert!(matches!(
        r.read(&data()),
        Err(SolutionFactError::MissingAccessor(_))
    ));
    assert_eq!(calls.get(), 0);
    r.heterogeneous("otherProblemFactList", |s| {
        s.other_problem_fact_list.clone()
    })
    .unwrap();
    r.singleton("extraObject", |s| {
        s.extra_object
            .as_ref()
            .and_then(|r| r.object().clone().downcast::<Opaque>().ok())
    })
    .unwrap();
    let s = data();
    let previous = r.read(&s).unwrap();
    assert_eq!(calls.get(), 1);
    let mut bad = data();
    bad.other_problem_fact_list = vec![SolutionFactRecord::new(Arc::new(Entity {
        code: "e".into(),
        value: None,
    }))];
    assert!(matches!(
        r.read(&bad),
        Err(SolutionFactError::EntityAsFact { .. })
    ));
    assert_eq!(
        previous.records()[0].object_ref::<Value>().unwrap().code,
        "v1"
    );
    let mut fresh = SolutionFactAccessors::new(Arc::new(properties())).unwrap();
    assert!(matches!(
        fresh.collection::<String, _>("valueList", |_: &Properties| vec![]),
        Err(SolutionFactError::WrongType { .. })
    ));
    fresh
        .collection("valueList", |s| s.value_list.clone())
        .unwrap();
}
struct NativeEntity {
    value: Option<i64>,
}
struct Weight {
    target: i64,
}
struct NativeSolution {
    values: Vec<Arc<i64>>,
    weights: Vec<Arc<Weight>>,
    entity: Arc<NativeEntity>,
}
fn native_layout() -> Arc<ComposedPlanningSolution> {
    let entities =
        PlanningComposition::new(vec![PlanningComponent::registered::<NativeEntity>(vec![
            PlanningVariableDefinition::basic::<i64>("value", false),
        ])])
        .unwrap();
    Arc::new(
        SolutionComposition::new(vec![
            PlanningSolutionComponent::registered::<NativeSolution>(vec![
                SolutionMemberDefinition::value_range::<i64>("values", "range")
                    .as_problem_fact_collection(),
                SolutionMemberDefinition::problem_fact_collection::<Weight>("weights"),
                SolutionMemberDefinition::entity::<NativeEntity>("entity"),
                SolutionMemberDefinition::score::<SimpleScore>("score"),
            ]),
        ])
        .unwrap()
        .compile::<NativeSolution>(
            &entities,
            &[t::<NativeEntity>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap(),
    )
}
fn bind(
    layout: &ComposedPlanningSolution,
    s: &NativeSolution,
    facts: BoundSolutionFacts,
) -> Result<BoundBasicSolution<i64>, SolutionBindingError> {
    layout.bind_basic_with_facts(
        vec![SolutionCollection {
            member: "entity".into(),
            entities: vec![SolutionEntityRecord::new(
                s.entity.clone(),
                vec![("value".into(), s.entity.value)],
            )],
        }],
        vec![SolutionRange {
            member: "values".into(),
            values: s.values.iter().map(|v| **v).collect(),
        }],
        vec![SolutionFieldRange::new::<NativeEntity>(
            "value",
            vec!["range".into()],
        )],
        facts,
    )
}
#[test]
fn bound_fact_payloads_change_actual_dense_and_stream_optima() {
    let layout = native_layout();
    let mut reads = SolutionFactAccessors::new(layout.clone()).unwrap();
    reads
        .collection("values", |s: &NativeSolution| s.values.clone())
        .unwrap();
    reads.collection("weights", |s| s.weights.clone()).unwrap();
    let mut s = NativeSolution {
        values: (0..3).map(Arc::new).collect(),
        weights: vec![Arc::new(Weight { target: 1 })],
        entity: Arc::new(NativeEntity { value: None }),
    };
    let first = bind(&layout, &s, reads.read(&s).unwrap()).unwrap();
    s.weights = vec![Arc::new(Weight { target: 2 })];
    s.entity = Arc::new(NativeEntity { value: Some(1) });
    let second = bind(&layout, &s, reads.read(&s).unwrap()).unwrap();
    for (bound, target) in [(&first, 1), (&second, 2)] {
        let facts = bound.facts().unwrap();
        let mut network = ConstraintNetwork::<_, SimpleScore>::new(1);
        network
            .add_constraint("distance", vec![0], move |values| {
                SimpleScore::new(
                    -facts
                        .facts_of::<Weight>()
                        .map(|w| (values[0].copied().unwrap_or(0) - w.target).abs())
                        .sum::<i64>(),
                )
            })
            .unwrap();
        let dense = solve(bound.problem(), &network, SolverPlan::default()).unwrap();
        let stream = solve_stream(
            bound.problem(),
            || {
                let mut b = StreamGraphBuilder::new();
                let source = b.source::<usize, (usize, Option<i64>)>();
                b.score("distance", source.port(), move |(_, v)| {
                    SimpleScore::new(
                        -facts
                            .facts_of::<Weight>()
                            .map(|w| (v.unwrap_or(0) - w.target).abs())
                            .sum::<i64>(),
                    )
                })?;
                Ok((b.build(), source))
            },
            SolverPlan::default(),
        )
        .unwrap();
        for result in [dense, stream] {
            assert_eq!(result.assignment.values(), &[Some(target)]);
            assert_eq!(result.score, Some(SimpleScore::ZERO));
        }
    }
    assert_eq!(
        first
            .facts()
            .unwrap()
            .facts_of::<Weight>()
            .next()
            .unwrap()
            .target,
        1
    );
    assert!(matches!(
        bind(&native_layout(), &s, reads.read(&s).unwrap()),
        Err(SolutionBindingError::DifferentFactDescriptor)
    ));
    drop(s);
    drop(reads);
    assert_eq!(
        second
            .facts()
            .unwrap()
            .facts_of::<Weight>()
            .next()
            .unwrap()
            .target,
        2
    );
}

#[test]
fn heterogeneous_singletons_reject_multiple_objects_before_snapshot_publication() {
    let mut s = data();
    s.extra_object = Some(SolutionFactRecord::new(Arc::new(Opaque { amount: 4 })));
    let mut r = SolutionFactAccessors::new(Arc::new(properties())).unwrap();
    r.collection("valueList", |s: &Properties| s.value_list.clone())
        .unwrap();
    r.heterogeneous("otherProblemFactList", |s| {
        s.other_problem_fact_list.clone()
    })
    .unwrap();
    r.heterogeneous("extraObject", |s| {
        s.extra_object
            .iter()
            .flat_map(|v| [v.clone(), v.clone()])
            .collect()
    })
    .unwrap();
    assert!(
        matches!(r.read(&s),Err(SolutionFactError::SingletonCardinality(name)) if name=="extraObject")
    );
    let good = readers(Arc::new(properties())).read(&s).unwrap();
    assert_eq!(good.facts_of::<Opaque>().count(), 1);
    assert_eq!(good.facts_of::<Opaque>().next().unwrap().amount, 4);
}
