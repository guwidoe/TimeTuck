//! Original native schema-to-provider integration; no upstream credit.
use timetuck::*;
struct Solution;
struct Row;
struct Tasks;
fn basic_schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Row>(vec![
        PlanningVariableDefinition::basic::<i32>("x", false),
        PlanningVariableDefinition::basic::<i32>("y", false),
        PlanningVariableDefinition::shadow::<i32>("derived"),
    ])])
    .unwrap()
}
fn slots<'a>(s: &'a PlanningSchema, name: &str, slots: Vec<usize>) -> SchemaVariableSlots<'a> {
    SchemaVariableSlots {
        variable: s
            .genuine_entity::<Row>()
            .unwrap()
            .genuine_variable(name)
            .unwrap(),
        slots,
    }
}
#[test]
fn selected_columns_swap_atomically_and_change_solve_with_both_scoring_backends() {
    let s = basic_schema();
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![10]).with_initial(10).with_pinned(true),
        Variable::new(vec![0, 1]).with_initial(1),
        Variable::new(vec![20]).with_initial(20).with_pinned(true),
    ])
    .unwrap();
    let mut c = SchemaMoveConfig::new(SchemaMoveKind::Swap);
    c.variables = vec!["x".into()];
    let mut q = c
        .resolve(&s)
        .unwrap()
        .bind(&[slots(&s, "x", vec![0, 2])])
        .unwrap();
    let a = p.initial_assignment();
    let moves: Vec<_> = q.try_basic_moves(&p, &a, 0).unwrap().collect();
    assert_eq!(
        moves,
        vec![Move::Composite(vec![Move::Swap { left: 0, right: 2 }])]
    );
    let mut changed = a.clone();
    p.apply(&mut changed, &moves[0]).unwrap().commit();
    assert_eq!(changed.values(), &[Some(1), Some(10), Some(0), Some(20)]);
    c.kind = SchemaMoveKind::Change;
    q = c
        .resolve(&s)
        .unwrap()
        .bind(&[slots(&s, "x", vec![0, 2])])
        .unwrap();
    assert_eq!(
        q.try_basic_moves(&p, &a, 0).unwrap().collect::<Vec<_>>(),
        change_moves(&p, &a)
            .unwrap()
            .filter(|m| m.touched_variables().iter().all(|i| [0, 2].contains(i)))
            .collect::<Vec<_>>()
    );
    let config = LocalSearchConfig {
        max_steps: 4,
        assert_scores: true,
        ..Default::default()
    };
    let mut n = ConstraintNetwork::new(4);
    n.add_constraint("x", vec![0, 2], |v| {
        SimpleScore::new(v.iter().map(|v| i64::from(v.copied().unwrap())).sum())
    })
    .unwrap();
    let plain = local_search_from_with_provider(&p, &n, a.clone(), config, &mut q).unwrap();
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    b.score("x", source.port(), |(id, v)| {
        SimpleScore::new(if [0, 2].contains(id) {
            i64::from(v.unwrap())
        } else {
            0
        })
    })
    .unwrap();
    let stream =
        stream_local_search_from_with_provider(&p, b.build(), source, a, config, &mut q).unwrap();
    let plain = plain.best.unwrap();
    let stream = stream.best.unwrap();
    assert_eq!(plain.score, SimpleScore::new(2));
    assert_eq!(plain.score, stream.score);
    assert_eq!(plain.assignment, stream.assignment);
    assert_eq!(
        plain.assignment.values(),
        &[Some(1), Some(10), Some(1), Some(20)]
    );
}
#[test]
fn all_columns_and_entity_types_resolve_without_cross_entity_swaps() {
    struct Other;
    let s = PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Row>(vec![
            PlanningVariableDefinition::basic::<i32>("x", false),
            PlanningVariableDefinition::basic::<i32>("y", false),
        ]),
        PlanningEntityDefinition::of::<Other>(vec![PlanningVariableDefinition::basic::<i32>(
            "x", false,
        )]),
    ])
    .unwrap();
    let plan = SchemaMoveConfig::new(SchemaMoveKind::Swap)
        .resolve(&s)
        .unwrap();
    assert_eq!(plan.branches().len(), 2);
    let q = plan
        .bind(&[
            slots(&s, "x", vec![0, 2]),
            slots(&s, "y", vec![1, 3]),
            SchemaVariableSlots {
                variable: s
                    .genuine_entity::<Other>()
                    .unwrap()
                    .genuine_variable("x")
                    .unwrap(),
                slots: vec![4, 5],
            },
        ])
        .unwrap();
    let p = FiniteProblem::new(
        [0, 1, 1, 0, 0, 1]
            .into_iter()
            .map(|x| Variable::new(vec![0, 1]).with_initial(x))
            .collect(),
    )
    .unwrap();
    let a = p.initial_assignment();
    let moves: Vec<_> = q.try_basic_moves(&p, &a, 0).unwrap().collect();
    assert_eq!(
        moves,
        vec![
            Move::Composite(vec![
                Move::Swap { left: 0, right: 2 },
                Move::Swap { left: 1, right: 3 }
            ]),
            Move::Composite(vec![Move::Swap { left: 4, right: 5 }])
        ]
    );
    for m in moves {
        let mut copy = a.clone();
        {
            let _guard = p.apply(&mut copy, &m).unwrap();
        }
        assert_eq!(copy, a);
    }
}
#[test]
fn bindings_reject_aliases_foreign_metadata_types_bounds_and_optional_policy() {
    let s = basic_schema();
    let other = basic_schema();
    let plan = SchemaMoveConfig::new(SchemaMoveKind::Change)
        .resolve(&s)
        .unwrap();
    assert!(matches!(
        plan.bind(&[slots(&other, "x", vec![0])]),
        Err(SchemaMoveError::ForeignSchema)
    ));
    assert!(matches!(
        plan.bind(&[slots(&s, "x", vec![0]), slots(&s, "y", vec![0])]),
        Err(SchemaMoveError::AliasedSlot { .. })
    ));
    assert!(matches!(
        plan.bind(&[slots(&s, "x", vec![0]), slots(&s, "y", vec![1, 2])]),
        Err(SchemaMoveError::BindingLength(_))
    ));
    assert!(matches!(
        plan.bind(&[slots(&s, "x", vec![0])]),
        Err(SchemaMoveError::MissingBinding(_))
    ));
    assert!(matches!(
        plan.bind(&[slots(&s, "x", vec![0]), slots(&s, "x", vec![1])]),
        Err(SchemaMoveError::DuplicateBinding(_))
    ));
    let mut c = SchemaMoveConfig::new(SchemaMoveKind::Change);
    c.variables = vec!["derived".into()];
    assert!(c.resolve(&s).is_err());
    c.variables = vec!["x".into(), "x".into()];
    assert!(matches!(
        c.resolve(&s),
        Err(SchemaMoveError::DuplicateName(_))
    ));
    c.variables = vec!["x".into()];
    let plan = c.resolve(&s).unwrap();
    let q = plan.bind(&[slots(&s, "x", vec![3])]).unwrap();
    let p = FiniteProblem::new(vec![Variable::new(vec![0i32]).with_initial(0)]).unwrap();
    assert!(matches!(
        q.try_basic_moves(&p, &p.initial_assignment(), 0),
        Err(SchemaMoveError::SlotOutOfBounds { .. })
    ));
    let q = plan.bind(&[slots(&s, "x", vec![0])]).unwrap();
    let p = FiniteProblem::new(vec![Variable::new(vec![0i64]).with_initial(0)]).unwrap();
    assert!(matches!(
        q.try_basic_moves(&p, &p.initial_assignment(), 0),
        Err(SchemaMoveError::WrongValueType { .. })
    ));
    let p = FiniteProblem::new(vec![Variable::new(vec![0i32]).allowing_unassigned()]).unwrap();
    assert!(matches!(
        q.try_basic_moves(&p, &p.initial_assignment(), 0),
        Err(SchemaMoveError::UnassignedPolicy(_))
    ));
}
#[test]
fn required_list_construction_and_selected_entity_partitions_remain_native() {
    let s = PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Tasks>(vec![
        PlanningVariableDefinition::list::<i32>("tasks", false),
    ])])
    .unwrap();
    let plan = SchemaMoveConfig::new(SchemaMoveKind::Change)
        .resolve(&s)
        .unwrap();
    let field = s
        .genuine_entity::<Tasks>()
        .unwrap()
        .genuine_variable("tasks")
        .unwrap();
    let mut q = plan
        .bind(&[SchemaVariableSlots {
            variable: field,
            slots: vec![0, 1],
        }])
        .unwrap();
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
    )
    .unwrap();
    let mut answers = Vec::new();
    for stream in [false, true] {
        let config = ProviderConstructionConfig {
            max_steps: Some(4),
            ..Default::default()
        };
        let r = if stream {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let source = ListStreamSources::new(&mut b);
            list_stream_construction_from_with_provider(
                &p,
                b.build(),
                source,
                p.initial_assignment(),
                config,
                &mut q,
            )
            .unwrap()
        } else {
            let n = ListConstraintNetwork::<_, SimpleScore>::new(2, 3);
            list_construction_from_with_provider(&p, &n, p.initial_assignment(), config, &mut q)
                .unwrap()
        };
        assert!(r.complete);
        p.validate(&r.assignment).unwrap();
        answers.push(r.assignment);
    }
    assert_eq!(answers[0], answers[1]);
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1]).with_pinned_prefix(1),
            ListEntity::new(vec![2]),
            ListEntity::new(vec![3]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    for kind in [SchemaMoveKind::Change, SchemaMoveKind::Swap] {
        let plan = SchemaMoveConfig::new(kind).resolve(&s).unwrap();
        let q = plan
            .bind(&[SchemaVariableSlots {
                variable: field,
                slots: vec![0, 1],
            }])
            .unwrap();
        for m in q.try_list_moves(&p, &a, 0).unwrap() {
            assert!(m.touched_entities().iter().all(|e| *e < 2));
            let mut copy = a.clone();
            p.apply(&mut copy, &m).unwrap().commit();
            assert_eq!(copy.lists()[0].first(), Some(&1));
            assert_eq!(copy.lists()[2], vec![3]);
        }
    }
}
#[test]
fn mixed_provider_optimizes_basic_and_list_state_on_both_backends() {
    let s = PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Row>(vec![PlanningVariableDefinition::basic::<i32>(
            "x", false,
        )]),
        PlanningEntityDefinition::of::<Tasks>(vec![PlanningVariableDefinition::list::<i32>(
            "tasks", false,
        )]),
    ])
    .unwrap();
    let mut q = SchemaMoveConfig::new(SchemaMoveKind::Change)
        .resolve(&s)
        .unwrap()
        .bind(&[
            slots(&s, "x", vec![0]),
            SchemaVariableSlots {
                variable: s
                    .genuine_entity::<Tasks>()
                    .unwrap()
                    .genuine_variable("tasks")
                    .unwrap(),
                slots: vec![0, 1],
            },
        ])
        .unwrap();
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
        ListProblem::new(
            vec![1, 2],
            vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![])],
        )
        .unwrap(),
    );
    let config = MixedLocalSearchConfig {
        search: LocalSearchConfig {
            max_steps: 6,
            assert_scores: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut n = MixedConstraintNetwork::new(1, 2, 2);
    n.add_constraint("x", vec![MixedScope::Basic(0)], |r| {
        let MixedFact::Basic(v) = r[0] else {
            unreachable!()
        };
        SimpleScore::new(i64::from(v.copied().unwrap()))
    })
    .unwrap();
    for i in 0..2 {
        n.add_constraint(format!("load{i}"), vec![MixedScope::ListEntity(i)], |r| {
            let MixedFact::ListEntity(v) = r[0] else {
                unreachable!()
            };
            SimpleScore::new(-(v.len() as i64).pow(2))
        })
        .unwrap();
    }
    let plain =
        mixed_local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut q)
            .unwrap();
    let mut b = StreamGraphBuilder::new();
    let sources = MixedStreamSources::new(&mut b);
    b.score("x", sources.basic.port(), |(_, v)| {
        SimpleScore::new(i64::from(v.unwrap()))
    })
    .unwrap();
    b.score("load", sources.lists.entities.port(), |(_, v)| {
        SimpleScore::new(-(v.len() as i64).pow(2))
    })
    .unwrap();
    let stream = mixed_stream_local_search_from_with_provider(
        &p,
        b.build(),
        sources,
        p.initial_assignment(),
        config,
        &mut q,
    )
    .unwrap();
    let plain = plain.best.unwrap();
    let stream = stream.best.unwrap();
    assert_eq!(plain.score, SimpleScore::new(-1));
    assert_eq!(stream.score, plain.score);
    assert_eq!(stream.assignment, plain.assignment);
    assert_eq!(plain.assignment.basic().values(), &[Some(1)]);
    assert!(
        plain
            .assignment
            .lists()
            .lists()
            .iter()
            .all(|l| l.len() == 1)
    );
}
#[test]
fn seeded_child_interleaving_caps_and_assignment_refresh_are_repeatable() {
    let s = basic_schema();
    let mut c = SchemaMoveConfig::new(SchemaMoveKind::Change);
    c.order = UnionOrder::Random { seed: 37 };
    c.selected_count_limit = Some(2);
    let mut q = c
        .resolve(&s)
        .unwrap()
        .bind(&[slots(&s, "x", vec![0, 2]), slots(&s, "y", vec![1, 3])])
        .unwrap();
    let p = FiniteProblem::new(
        (0..4)
            .map(|_| Variable::new(vec![0, 1, 2]).with_initial(0))
            .collect(),
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let first: Vec<_> = q.try_basic_moves(&p, &a, 4).unwrap().collect();
    assert_eq!(first.len(), 2);
    assert_eq!(
        q.try_basic_moves(&p, &a, 4).unwrap().collect::<Vec<_>>(),
        first
    );
    p.apply(&mut a, &first[0]).unwrap().commit();
    let next: Vec<_> = q.moves(&p, &a, 5).collect();
    assert_eq!(next.len(), 2);
    for m in next {
        let mut copy = a.clone();
        p.apply(&mut copy, &m).unwrap().commit();
        assert_ne!(copy, a);
    }
    c.selected_count_limit = Some(0);
    let q = c
        .resolve(&s)
        .unwrap()
        .bind(&[slots(&s, "x", vec![0, 2]), slots(&s, "y", vec![1, 3])])
        .unwrap();
    assert_eq!(q.try_basic_moves(&p, &a, 0).unwrap().count(), 0);
}
