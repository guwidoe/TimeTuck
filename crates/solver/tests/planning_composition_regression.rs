// Original regressions. Reused test-domain setup is attributed in the helper.
mod planning_composition_support;
use planning_composition_support::*;
use timetuck::*;
struct Root;
struct Shared;
struct Second;
struct Unused;
struct World;
fn t<T: 'static>() -> PlanningType {
    PlanningType::of::<T>()
}
fn basic(name: &str) -> PlanningVariableDefinition {
    PlanningVariableDefinition::basic::<i32>(name, false)
}
#[test]
fn general_composition_runs_deep_and_mixed_models_in_both_score_backends() {
    for (shape, expected) in [
        (Shape::DeepBothOwned, 4),
        (Shape::DeepBothProjected, 4),
        (Shape::Mixed, 6),
    ] {
        let f = generate::<30>(shape);
        assert!(!f.actual_class_fixture);
        let c = compile::<30>(&f, PlanningCompositionPolicy::default()).unwrap();
        assert_eq!(
            c.schema()
                .genuine_entity::<Child<30>>()
                .unwrap()
                .genuine_variables()
                .count(),
            f.fields.len()
        );
        for result in solve_fixture::<30>(&f, &c, 100) {
            assert_eq!(result.score, Some(SimpleScore::new(expected)));
            assert!(result.assignment.values().iter().all(Option::is_some));
        }
    }
}
#[test]
fn diamond_shared_ancestor_contributes_once_and_depth_uses_longest_path() {
    let graph = PlanningComposition::new(vec![
        PlanningComponent::registered::<Shared>(vec![basic("shared")]),
        PlanningComponent::registered::<Second>(vec![basic("second")])
            .with_parent::<Shared>(PlanningParentKind::Owned),
        PlanningComponent::registered::<Root>(vec![basic("root")])
            .with_parent::<Shared>(PlanningParentKind::Owned)
            .with_parent::<Second>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Unused>(vec![basic("invalid but unreachable")]),
    ])
    .unwrap();
    let c = graph
        .compile::<World>(&[t::<Root>()], PlanningCompositionPolicy::default())
        .unwrap();
    assert_eq!(
        c.schema()
            .genuine_entity::<Root>()
            .unwrap()
            .variables()
            .map(|v| v.name())
            .collect::<Vec<_>>(),
        ["shared", "second", "root"]
    );
    assert_eq!(c.field_origin(t::<Root>(), "shared"), Some(t::<Shared>()));
    assert_eq!(c.field_origin(t::<Root>(), "second"), Some(t::<Second>()));
    assert_eq!(c.field_origin(t::<Root>(), "unknown"), None);
    assert_eq!(c.field_origin(t::<Unused>(), "shared"), None);
    let policy = PlanningCompositionPolicy {
        maximum_depth: std::num::NonZeroUsize::new(2),
        ..Default::default()
    };
    assert_eq!(
        graph.compile::<World>(&[t::<Root>()], policy).unwrap_err(),
        PlanningCompositionError::DepthLimit {
            entity: t::<Root>(),
            actual: 3,
            maximum: 2
        }
    );
    assert_eq!(
        graph
            .compile::<World>(&[t::<Unused>()], Default::default())
            .unwrap_err(),
        PlanningCompositionError::UnregisteredVariables(t::<Unused>())
    );
}
#[test]
fn graph_errors_identify_actual_duplicates_missing_links_and_cycles() {
    assert_eq!(
        PlanningComposition::new(vec![PlanningComponent::registered::<Root>(vec![]); 2])
            .unwrap_err(),
        PlanningCompositionError::DuplicateComponent(t::<Root>())
    );
    let missing = PlanningComposition::new(vec![
        PlanningComponent::registered::<Root>(vec![])
            .with_parent::<Shared>(PlanningParentKind::Owned),
    ])
    .unwrap();
    assert_eq!(
        missing
            .compile::<World>(&[t::<Root>()], Default::default())
            .unwrap_err(),
        PlanningCompositionError::UnknownParent {
            component: t::<Root>(),
            parent: t::<Shared>()
        }
    );
    let cyclic = PlanningComposition::new(vec![
        PlanningComponent::registered::<Root>(vec![])
            .with_parent::<Shared>(PlanningParentKind::Owned),
        PlanningComponent::registered::<Shared>(vec![])
            .with_parent::<Root>(PlanningParentKind::Projection),
    ])
    .unwrap();
    assert_eq!(
        cyclic
            .compile::<World>(&[t::<Root>()], Default::default())
            .unwrap_err(),
        PlanningCompositionError::Cycle {
            component: t::<Shared>(),
            parent: t::<Root>()
        }
    );
    let duplicate = PlanningComposition::new(vec![
        PlanningComponent::registered::<Root>(vec![])
            .with_parent::<Shared>(PlanningParentKind::Owned)
            .with_parent::<Shared>(PlanningParentKind::Projection),
        PlanningComponent::registered::<Shared>(vec![]),
    ])
    .unwrap();
    assert_eq!(
        duplicate
            .compile::<World>(&[t::<Root>()], Default::default())
            .unwrap_err(),
        PlanningCompositionError::DuplicateParent {
            component: t::<Root>(),
            parent: t::<Shared>()
        }
    );
    assert!(matches!(
        missing.compile::<World>(&[t::<Unused>()], Default::default()),
        Err(PlanningCompositionError::UnknownComponent(_))
    ));
}
#[test]
fn field_origins_detect_conflicting_parent_names_and_preserve_native_kinds() {
    let valid = PlanningComposition::new(vec![
        PlanningComponent::registered::<Shared>(vec![
            PlanningVariableDefinition::list::<String>("list", true),
            PlanningVariableDefinition::shadow::<u64>("shadow"),
        ]),
        PlanningComponent::registered::<Root>(vec![basic("value")])
            .with_parent::<Shared>(PlanningParentKind::Projection),
    ])
    .unwrap()
    .compile::<World>(&[t::<Root>()], Default::default())
    .unwrap();
    let entity = valid.schema().genuine_entity::<Root>().unwrap();
    assert_eq!(
        entity.variable("list").unwrap().kind(),
        PlanningVariableKind::List {
            allows_unassigned: true
        }
    );
    assert_eq!(entity.variable("shadow").unwrap().value_type(), t::<u64>());
    assert_eq!(entity.shadow_variables().count(), 1);
    assert_eq!(
        valid.field_origin(t::<Root>(), "shadow"),
        Some(t::<Shared>())
    );
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2])]).unwrap();
    let slot = SchemaVariableSlots {
        variable: entity.genuine_variable("value").unwrap(),
        slots: vec![0],
    };
    let bound = slot.bind_basic(&p).unwrap();
    let mut assignment = p.initial_assignment();
    p.apply(&mut assignment, &bound.change(0, Some(2)).unwrap())
        .unwrap()
        .commit();
    assert_eq!(bound.read(&assignment, 0).unwrap(), Some(&2));
    let schema = valid.into_schema();
    assert_eq!(schema.solution_type(), t::<World>());
    let conflicting = PlanningComposition::new(vec![
        PlanningComponent::registered::<Shared>(vec![basic("same")]),
        PlanningComponent::registered::<Second>(vec![PlanningVariableDefinition::shadow::<bool>(
            "same",
        )]),
        PlanningComponent::registered::<Root>(vec![])
            .with_parent::<Shared>(PlanningParentKind::Owned)
            .with_parent::<Second>(PlanningParentKind::Projection),
    ])
    .unwrap();
    assert_eq!(
        conflicting
            .compile::<World>(&[t::<Root>()], Default::default())
            .unwrap_err(),
        PlanningCompositionError::RedefinedVariable {
            entity: t::<Root>(),
            variable: "same".into(),
            original: t::<Shared>(),
            redefining: t::<Second>()
        }
    );
}
#[test]
fn independent_roots_keep_requested_order_and_invalid_local_fields_are_rejected() {
    let graph = PlanningComposition::new(vec![
        PlanningComponent::registered::<Root>(vec![basic("r")]),
        PlanningComponent::registered::<Second>(vec![basic("s")]),
    ])
    .unwrap();
    let c = graph
        .compile::<World>(&[t::<Second>(), t::<Root>()], Default::default())
        .unwrap();
    assert_eq!(
        c.schema()
            .entities()
            .map(|e| e.entity_type())
            .collect::<Vec<_>>(),
        [t::<Second>(), t::<Root>()]
    );
    assert_eq!(
        graph
            .compile::<World>(&[t::<Root>(), t::<Root>()], Default::default())
            .unwrap_err(),
        PlanningCompositionError::DuplicateRoot(t::<Root>())
    );
    assert_eq!(
        graph
            .compile::<World>(&[], Default::default())
            .unwrap()
            .schema()
            .entities()
            .count(),
        0
    );
    for (fields, error) in [
        (
            vec![basic("")],
            PlanningSchemaError::EmptyVariableName(t::<Root>()),
        ),
        (
            vec![basic("r"), basic("r")],
            PlanningSchemaError::DuplicateVariable {
                entity: t::<Root>(),
                variable: "r".into(),
            },
        ),
    ] {
        let invalid =
            PlanningComposition::new(vec![PlanningComponent::registered::<Root>(fields)]).unwrap();
        assert_eq!(
            invalid
                .compile::<World>(&[t::<Root>()], Default::default())
                .unwrap_err(),
            PlanningCompositionError::Schema(error)
        );
    }
}
#[test]
fn independent_policy_switches_validate_real_parent_structure() {
    let graph = PlanningComposition::new(vec![
        PlanningComponent::registered::<Shared>(vec![basic("left")]),
        PlanningComponent::registered::<Second>(vec![basic("right")]),
        PlanningComponent::registered::<Root>(vec![])
            .with_parent::<Shared>(PlanningParentKind::Owned)
            .with_parent::<Second>(PlanningParentKind::Projection),
    ])
    .unwrap();
    let single = PlanningCompositionPolicy {
        allow_multiple_parents: false,
        ..Default::default()
    };
    assert_eq!(
        graph.compile::<World>(&[t::<Root>()], single).unwrap_err(),
        PlanningCompositionError::MultipleParents(t::<Root>())
    );
    let unmixed = PlanningCompositionPolicy {
        allow_mixed_parent_kinds: false,
        ..Default::default()
    };
    assert_eq!(
        graph.compile::<World>(&[t::<Root>()], unmixed).unwrap_err(),
        PlanningCompositionError::MixedParents(t::<Root>())
    );
    assert_eq!(
        graph
            .compile::<World>(&[t::<Root>()], Default::default())
            .unwrap()
            .schema()
            .genuine_entity::<Root>()
            .unwrap()
            .genuine_variables()
            .count(),
        2
    );
}

struct Layer<const N: usize>;
#[test]
fn long_lineage_compiles_iteratively_and_keeps_the_declaring_origin() {
    let components = vec![
        PlanningComponent::registered::<Layer<0>>(vec![basic("value")]),
        PlanningComponent::plain::<Layer<1>>(vec![])
            .with_parent::<Layer<0>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<2>>(vec![])
            .with_parent::<Layer<1>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<3>>(vec![])
            .with_parent::<Layer<2>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<4>>(vec![])
            .with_parent::<Layer<3>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<5>>(vec![])
            .with_parent::<Layer<4>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<6>>(vec![])
            .with_parent::<Layer<5>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<7>>(vec![])
            .with_parent::<Layer<6>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<8>>(vec![])
            .with_parent::<Layer<7>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<9>>(vec![])
            .with_parent::<Layer<8>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<10>>(vec![])
            .with_parent::<Layer<9>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<11>>(vec![])
            .with_parent::<Layer<10>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<12>>(vec![])
            .with_parent::<Layer<11>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<13>>(vec![])
            .with_parent::<Layer<12>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<14>>(vec![])
            .with_parent::<Layer<13>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<15>>(vec![])
            .with_parent::<Layer<14>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<16>>(vec![])
            .with_parent::<Layer<15>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<17>>(vec![])
            .with_parent::<Layer<16>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<18>>(vec![])
            .with_parent::<Layer<17>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<19>>(vec![])
            .with_parent::<Layer<18>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<20>>(vec![])
            .with_parent::<Layer<19>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<21>>(vec![])
            .with_parent::<Layer<20>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<22>>(vec![])
            .with_parent::<Layer<21>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<23>>(vec![])
            .with_parent::<Layer<22>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<24>>(vec![])
            .with_parent::<Layer<23>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<25>>(vec![])
            .with_parent::<Layer<24>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<26>>(vec![])
            .with_parent::<Layer<25>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<27>>(vec![])
            .with_parent::<Layer<26>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<28>>(vec![])
            .with_parent::<Layer<27>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<29>>(vec![])
            .with_parent::<Layer<28>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<30>>(vec![])
            .with_parent::<Layer<29>>(PlanningParentKind::Owned),
        PlanningComponent::plain::<Layer<31>>(vec![])
            .with_parent::<Layer<30>>(PlanningParentKind::Owned),
    ];
    let graph = PlanningComposition::new(components).unwrap();
    let c = graph
        .compile::<World>(&[t::<Layer<31>>()], Default::default())
        .unwrap();
    assert_eq!(
        c.schema()
            .genuine_entity::<Layer<31>>()
            .unwrap()
            .genuine_variables()
            .count(),
        1
    );
    assert_eq!(
        c.field_origin(t::<Layer<31>>(), "value"),
        Some(t::<Layer<0>>())
    );
    let policy = PlanningCompositionPolicy {
        maximum_depth: std::num::NonZeroUsize::new(31),
        ..Default::default()
    };
    assert_eq!(
        graph
            .compile::<World>(&[t::<Layer<31>>()], policy)
            .unwrap_err(),
        PlanningCompositionError::DepthLimit {
            entity: t::<Layer<31>>(),
            actual: 32,
            maximum: 31
        }
    );
}

#[test]
fn source_entity_eligibility_waits_for_every_inherited_and_local_genuine_field() {
    let fixture = generate::<40>(Shape::BothOwned);
    let composed = compile::<40>(&fixture, PlanningCompositionPolicy::single_level()).unwrap();
    let problem = problem::<40>(&fixture, &composed);
    let network = dense_network(&fixture);
    let mut dense = ScoreDirector::new(&problem, &network, problem.initial_assignment()).unwrap();
    let (graph, source) = stream_graph(&fixture).unwrap();
    let mut streamed =
        StreamScoreDirector::new(&problem, graph, source, problem.initial_assignment()).unwrap();
    let entity = composed.schema().genuine_entity::<Child<40>>().unwrap();
    let fields = fixture
        .fields
        .iter()
        .enumerate()
        .map(|(field, name)| {
            SchemaVariableSlots {
                variable: entity.genuine_variable(name).unwrap(),
                slots: vec![field * 2, field * 2 + 1],
            }
            .bind_basic(&problem)
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(dense.score().with_structural(0), SimpleScore::ZERO);
    assert_eq!(dense.score(), streamed.score());
    for (field, row, value, expected) in [
        (0, 0, Some(0), 0),
        (1, 0, Some(0), 1),
        (0, 1, Some(1), 1),
        (1, 1, Some(1), 2),
        (1, 0, None, 1),
    ] {
        let value = value.map(|v| fixture.ranges[field][v].clone());
        let movement = fields[field].change(row, value).unwrap();
        dense.apply(&movement).unwrap().commit();
        streamed.apply(&movement).unwrap().commit();
        assert_eq!(dense.score().with_structural(0), SimpleScore::new(expected));
        assert_eq!(dense.score(), streamed.score());
        assert_eq!(
            network.calculate(&problem, dense.assignment()).unwrap(),
            dense.score()
        );
    }
}
