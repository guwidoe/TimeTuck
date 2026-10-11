//! Original metadata-to-repair integration regressions; no upstream credit.
#[path = "fixtures/schema_ruin_fixture.rs"]
mod fixture;
use std::{io, rc::Rc};
use timetuck::*;
#[test]
fn assembled_mixed_repairs_select_one_basic_column_or_the_list_registry() {
    for (field, expected, candidates) in [
        ("secondBasicValue", 7, vec![2, 3]),
        ("valueList", 20, vec![4, 5]),
    ] {
        let d = fixture::Domain::new(false, true);
        let mut c = d.configuration(Some(field), 0);
        if let SolverPhase::RuinRecreate(search) = &mut c.phases[0] {
            search.minimum_ruined = 2;
            search.maximum_ruined = 2;
        }
        let f = d
            .declaration()
            .assemble_solver_factory(c, fixture::shape())
            .unwrap();
        let selector = f.assembled_ruin_selector(0).unwrap();
        assert_eq!(
            selector.ruin_entity_type(),
            PlanningType::of::<fixture::TestdataMixedEntity>()
        );
        assert_eq!(selector.ruin_variable_name(), field);
        assert_eq!(
            selector.mixed_ruin_candidates(&d.model).unwrap(),
            candidates
        );
        let initial = d.initialized();
        for _ in 0..2 {
            let result = f
                .solve_assembled_with_schema_repairs(
                    initial.clone(),
                    |resource, phase, selector| {
                        let ids = selector
                            .unwrap()
                            .mixed_ruin_candidates(&resource.domain.model)
                            .map_err(io::Error::other)?;
                        let SolverPhase::RuinRecreate(search) = phase else {
                            unreachable!()
                        };
                        let mut search = search.clone();
                        search.ruin_candidates = Some(&ids);
                        resource
                            .director
                            .run_solver(MixedSolverSequence {
                                limits: Default::default(),
                                phases: vec![MixedSolverPhase::RuinRecreate(
                                    MixedRuinRecreateConfig {
                                        search,
                                        order: Default::default(),
                                    },
                                )],
                            })
                            .map(fixture::Work)
                            .map_err(|e| io::Error::other(e.to_string()))
                    },
                )
                .unwrap();
            assert_eq!(result.score, SimpleScore::new(expected));
            assert_eq!(result.phases.len(), 1);
            assert_eq!(
                result.logical_calculations,
                u128::from(result.phases[0].0.work.evaluations)
            );
            assert!(result.logical_calculations > 0);
            if field == "secondBasicValue" {
                assert_eq!(
                    &result.working.basic().values()[..2],
                    &initial.basic().values()[..2]
                );
                assert_eq!(result.working.lists(), initial.lists());
            } else {
                assert_eq!(result.working.basic(), initial.basic());
                assert_eq!(result.working.lists().list(1).unwrap().len(), 2);
            }
        }
        assert_eq!(d.preparations.get(), 1);
        assert!(
            d.model
                .initial_assignment()
                .basic()
                .values()
                .iter()
                .all(Option::is_none)
        );
    }
}
struct Model;
struct Row;
fn schema(kind: PlanningVariableKind) -> PlanningSchema {
    let field = match kind {
        PlanningVariableKind::Basic { allows_unassigned } => {
            PlanningVariableDefinition::basic::<i32>("x", allows_unassigned)
        }
        PlanningVariableKind::List { allows_unassigned } => {
            PlanningVariableDefinition::list::<i32>("x", allows_unassigned)
        }
        PlanningVariableKind::Shadow => PlanningVariableDefinition::shadow::<i32>("x"),
    };
    PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Row>(vec![field])]).unwrap()
}
fn binding(slots: Vec<usize>) -> SchemaRuinBinding {
    SchemaRuinBinding {
        entity: PlanningType::of::<Row>(),
        variable: "x".into(),
        slots,
    }
}
#[test]
fn basic_field_pool_runs_dense_and_stream_repair_without_touching_other_columns() {
    let s = schema(PlanningVariableKind::Basic {
        allows_unassigned: false,
    });
    let q = SchemaRuinConfig::default()
        .bind_schema_ruin(&s, &[binding(vec![0, 2])])
        .unwrap();
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![9]).with_initial(9).with_pinned(true),
        Variable::new(vec![0, 1]).with_initial(0),
    ])
    .unwrap();
    let pool = q.basic_ruin_candidates(&p).unwrap();
    let config = RuinRecreateConfig {
        minimum_ruined: 2,
        maximum_ruined: 2,
        max_attempts: 3,
        ruin_candidates: Some(&pool),
        assert_scores: true,
        ..Default::default()
    };
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("x", vec![0, 2], |values| {
        SimpleScore::new(
            values
                .iter()
                .map(|v| i64::from(v.copied().unwrap_or(0)))
                .sum(),
        )
    })
    .unwrap();
    let dense = ruin_recreate_search_from(&p, &n, p.initial_assignment(), config.clone())
        .unwrap()
        .best
        .unwrap();
    let stream = ruin_recreate_stream_search_from(
        &p,
        || {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("x", source.port(), |(id, v)| {
                SimpleScore::new(if *id == 1 {
                    0
                } else {
                    i64::from(v.unwrap_or(0))
                })
            })?;
            Ok((b.build(), source))
        },
        p.initial_assignment(),
        config,
    )
    .unwrap()
    .best
    .unwrap();
    assert_eq!(dense.score, SimpleScore::new(2));
    assert_eq!(dense.assignment.values(), &[Some(1), Some(9), Some(1)]);
    assert_eq!(dense.assignment, stream.assignment);
    assert_eq!(dense.score, stream.score);
}
#[test]
fn list_field_pool_runs_dense_and_stream_repair_and_retains_registry_order() {
    let s = schema(PlanningVariableKind::List {
        allows_unassigned: false,
    });
    let q = SchemaRuinConfig::default()
        .bind_schema_ruin(&s, &[binding(vec![1, 0])])
        .unwrap();
    let p = ListProblem::new(
        vec![10, 20],
        vec![ListEntity::new(vec![10, 20]), ListEntity::new(vec![])],
    )
    .unwrap();
    let pool = q.list_ruin_candidates(&p).unwrap();
    assert_eq!(pool, vec![0, 1]);
    let config = RuinRecreateConfig {
        minimum_ruined: 2,
        maximum_ruined: 2,
        max_attempts: 3,
        ruin_candidates: Some(&pool),
        assert_scores: true,
        ..Default::default()
    };
    let mut n = ListConstraintNetwork::new(2, 2);
    n.add_constraint("destination", vec![ListScope::Entity(1)], |facts| {
        let ListFact::Entity(v) = &facts[0] else {
            unreachable!()
        };
        SimpleScore::new(v.len() as i64)
    })
    .unwrap();
    let dense = ruin_recreate_list_search_from(&p, &n, p.initial_assignment(), config.clone())
        .unwrap()
        .best
        .unwrap();
    let stream = ruin_recreate_list_stream_search_from(
        &p,
        || {
            let mut b = StreamGraphBuilder::new();
            let sources = ListStreamSources::<i32>::new(&mut b);
            b.score("destination", sources.entities.port(), |(id, v)| {
                SimpleScore::new(if *id == 1 { v.len() as i64 } else { 0 })
            })?;
            Ok((b.build(), sources))
        },
        p.initial_assignment(),
        config,
    )
    .unwrap()
    .best
    .unwrap();
    assert_eq!(dense.score, SimpleScore::new(2));
    assert_eq!(dense.assignment.list(1).unwrap().len(), 2);
    assert_eq!(dense.assignment, stream.assignment);
    assert_eq!(dense.score, stream.score);
}
#[test]
fn schema_and_binding_errors_precede_preparation_and_remain_typed() {
    let d = fixture::Domain::new(false, false);
    for (phase, non_repair, duplicate) in [(99, false, false), (0, true, false), (0, false, true)] {
        let mut c = d.configuration(Some("basicValue"), phase);
        if non_repair {
            c.phases[0] = SolverPhase::Construction(Default::default());
        }
        if duplicate {
            c.schema_ruin_phases.push(c.schema_ruin_phases[0].clone());
        }
        let error = d
            .declaration()
            .assemble_solver_factory(c, fixture::shape())
            .err()
            .unwrap();
        assert!(
            matches!(
                error,
                FactoryAssemblyError::SchemaRuin {
                    error: SchemaRuinError::PhaseIndex { .. },
                    ..
                }
            ) || matches!(
                error,
                FactoryAssemblyError::SchemaRuin {
                    error: SchemaRuinError::NotRuinPhase { .. },
                    ..
                }
            ) || matches!(
                error,
                FactoryAssemblyError::SchemaRuin {
                    error: SchemaRuinError::DuplicatePhase { .. },
                    ..
                }
            )
        );
    }
    let error = d
        .declaration()
        .assemble_solver_factory(
            d.configuration(Some("declarativeShadowVariableValue"), 0),
            fixture::shape(),
        )
        .err()
        .unwrap();
    assert!(matches!(
        error,
        FactoryAssemblyError::SchemaRuin {
            error: SchemaRuinError::ShadowVariable { .. },
            ..
        }
    ));
    assert_eq!(d.preparations.get(), 0);
    let s = schema(PlanningVariableKind::Basic {
        allows_unassigned: false,
    });
    let q = SchemaRuinConfig::default();
    assert!(matches!(
        q.bind_schema_ruin(&s, &[]),
        Err(SchemaRuinError::MissingBinding { .. })
    ));
    assert!(matches!(
        q.bind_schema_ruin(&s, &[binding(vec![0]), binding(vec![0])]),
        Err(SchemaRuinError::DuplicateBinding { .. })
    ));
    assert!(matches!(
        q.bind_schema_ruin(&s, &[binding(vec![0, 0])]),
        Err(SchemaRuinError::DuplicateSlot { slot: 0 })
    ));
    let only_shadows = schema(PlanningVariableKind::Shadow);
    assert!(matches!(
        q.resolve_schema_ruin(&only_shadows),
        Err(SchemaRuinError::NoGenuineEntities)
    ));
    let explicit = SchemaRuinConfig {
        entity: Some(PlanningType::of::<Row>()),
        variable: Some("x".into()),
    };
    assert!(matches!(
        explicit.resolve_schema_ruin(&only_shadows),
        Err(SchemaRuinError::NoVariables { .. })
    ));
    let shared = Rc::new(s);
    assert_eq!(
        q.resolve_schema_ruin(&shared).unwrap(),
        q.resolve_schema_ruin(&shared).unwrap()
    );
}
#[test]
fn runtime_pool_validation_rejects_wrong_types_slots_policy_and_partial_list_field() {
    let basic = schema(PlanningVariableKind::Basic {
        allows_unassigned: false,
    });
    let c = SchemaRuinConfig::default();
    let q = c.bind_schema_ruin(&basic, &[binding(vec![2])]).unwrap();
    assert!(matches!(
        q.basic_ruin_candidates(&FiniteProblem::new(vec![Variable::new(vec![0])]).unwrap()),
        Err(SchemaRuinError::SlotOutOfBounds { slot: 2, count: 1 })
    ));
    assert!(matches!(
        q.basic_ruin_candidates(&FiniteProblem::new(vec![Variable::new(vec!["zero"])]).unwrap()),
        Err(SchemaRuinError::WrongValueType { .. })
    ));
    let q = c.bind_schema_ruin(&basic, &[binding(vec![0])]).unwrap();
    assert!(matches!(
        q.basic_ruin_candidates(
            &FiniteProblem::new(vec![Variable::new(vec![0]).allowing_unassigned()]).unwrap()
        ),
        Err(SchemaRuinError::UnassignedPolicy { .. })
    ));
    let lists = schema(PlanningVariableKind::List {
        allows_unassigned: false,
    });
    let q = c.bind_schema_ruin(&lists, &[binding(vec![0])]).unwrap();
    let p = ListProblem::new(
        vec![0],
        vec![ListEntity::new(vec![0]), ListEntity::new(vec![])],
    )
    .unwrap();
    assert!(matches!(
        q.list_ruin_candidates(&p),
        Err(SchemaRuinError::PartialListScope)
    ));
    assert!(matches!(
        q.basic_ruin_candidates(&FiniteProblem::new(vec![Variable::new(vec![0])]).unwrap()),
        Err(SchemaRuinError::WrongModel)
    ));
    let mut other = SchemaRuinConfig {
        entity: Some(PlanningType::of::<
            fixture::TestdataMixedMultiEntityFirstEntity,
        >()),
        variable: None,
    };
    let d = fixture::Domain::new(true, false);
    let q = other
        .bind_schema_ruin(
            &d.schema,
            &[SchemaRuinBinding {
                entity: other.entity.unwrap(),
                variable: "valueList".into(),
                slots: vec![0, 1],
            }],
        )
        .unwrap();
    assert_eq!(q.mixed_ruin_candidates(&d.model).unwrap(), vec![4, 5]);
    other.entity = Some(PlanningType::of::<Model>());
    assert!(matches!(
        other.resolve_schema_ruin(&d.schema),
        Err(SchemaRuinError::Schema(PlanningSchemaError::UnknownEntity(
            _
        )))
    ));
}
