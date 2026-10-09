//! Original assignment-policy integration across schema-bound solver providers.
use timetuck::*;
struct Solution;
struct Entity;
fn schema(basic: bool, list: bool) -> PlanningSchema {
    let mut fields = vec![];
    if basic {
        fields.push(PlanningVariableDefinition::basic::<i32>("x", true));
    }
    if list {
        fields.push(PlanningVariableDefinition::list::<i32>("items", true));
    }
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(fields)]).unwrap()
}
fn provider(
    s: &PlanningSchema,
    kind: SchemaMoveKind,
    crossing: Option<bool>,
    slots: Vec<usize>,
) -> SchemaMoveProvider {
    let mut c = SchemaMoveConfig::new(kind);
    c.crossing_unassigned = crossing;
    let bindings: Vec<_> = s
        .genuine_entity::<Entity>()
        .unwrap()
        .genuine_variables()
        .map(|variable| SchemaVariableSlots {
            variable,
            slots: slots.clone(),
        })
        .collect();
    c.resolve(s).unwrap().bind(&bindings).unwrap()
}
#[test]
fn basic_policies_preserve_selected_slots_pins_and_assignment_status() {
    let s = schema(true, false);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).allowing_unassigned(),
        Variable::new(vec![9]).with_initial(9),
        Variable::new(vec![0, 1])
            .allowing_unassigned()
            .with_initial(1),
        Variable::new(vec![0, 1])
            .allowing_unassigned()
            .with_pinned(true),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let change = |variable, value| Move::Change { variable, value };
    for (kind, crossing, expected) in [
        (
            SchemaMoveKind::Assign,
            None,
            vec![change(0, Some(0)), change(0, Some(1))],
        ),
        (SchemaMoveKind::Unassign, None, vec![change(2, None)]),
        (
            SchemaMoveKind::Change,
            Some(false),
            vec![change(2, Some(0))],
        ),
        (
            SchemaMoveKind::Change,
            None,
            vec![
                change(0, Some(0)),
                change(0, Some(1)),
                change(2, Some(0)),
                change(2, None),
            ],
        ),
    ] {
        let q = provider(&s, kind, crossing, vec![0, 2, 3]);
        let moves: Vec<_> = q.try_basic_moves(&p, &a, 0).unwrap().collect();
        assert_eq!(moves, expected);
        for movement in moves {
            let mut copy = a.clone();
            {
                let _guard = p.apply(&mut copy, &movement).unwrap();
            }
            assert_eq!(copy, a);
        }
    }
    let required =
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
            PlanningVariableDefinition::basic::<i32>("x", false),
        ])])
        .unwrap();
    for kind in [SchemaMoveKind::Assign, SchemaMoveKind::Unassign] {
        assert!(matches!(
            SchemaMoveConfig::new(kind).resolve(&required),
            Err(SchemaMoveError::TransitionRequiresOptional(_))
        ));
    }
    assert!(matches!(
        SchemaMoveConfig::new(SchemaMoveKind::Swap)
            .with_crossing_unassigned(false)
            .resolve(&s),
        Err(SchemaMoveError::CrossingPolicyRequiresChange)
    ));
}
#[test]
fn list_policies_preserve_pinned_prefixes_and_only_cross_when_enabled() {
    let s = schema(false, true);
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1]).with_pinned_prefix(1),
            ListEntity::new(vec![2]),
            ListEntity::new(vec![]).with_pinned(true),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let pos = |entity, index| ListPosition { entity, index };
    for (kind, crossing, expected) in [
        (
            SchemaMoveKind::Assign,
            None,
            vec![
                ListMove::Assign {
                    value: 3,
                    destination: pos(0, 1),
                },
                ListMove::Assign {
                    value: 3,
                    destination: pos(1, 0),
                },
                ListMove::Assign {
                    value: 3,
                    destination: pos(1, 1),
                },
            ],
        ),
        (
            SchemaMoveKind::Unassign,
            None,
            vec![ListMove::Unassign {
                entity: 1,
                start: 0,
                end: 1,
            }],
        ),
        (
            SchemaMoveKind::ListChange,
            Some(false),
            vec![ListMove::Change {
                source: pos(1, 0),
                destination: pos(0, 1),
            }],
        ),
    ] {
        let q = provider(&s, kind, crossing, vec![0, 1, 2]);
        let moves: Vec<_> = q.try_list_moves(&p, &a, 0).unwrap().collect();
        assert_eq!(moves, expected);
        for movement in moves {
            let mut copy = a.clone();
            {
                let _guard = p.apply(&mut copy, &movement).unwrap();
            }
            assert_eq!(copy, a);
        }
    }
    let q = provider(&s, SchemaMoveKind::ListChange, None, vec![0, 1, 2]);
    assert_eq!(q.try_list_moves(&p, &a, 0).unwrap().count(), 5);
}
#[test]
fn assign_and_unassign_search_agree_across_all_six_model_backend_paths() {
    for kind in [SchemaMoveKind::Assign, SchemaMoveKind::Unassign] {
        let sign = if kind == SchemaMoveKind::Assign {
            1
        } else {
            -1
        };
        let p = FiniteProblem::new(vec![
            Variable::new(vec![1i32, 2]).allowing_unassigned(),
            Variable::new(vec![1, 2])
                .allowing_unassigned()
                .with_initial(1),
        ])
        .unwrap();
        let lists = ListProblem::new(
            vec![1i32, 2],
            vec![ListEntity::new(vec![1]), ListEntity::new(vec![])],
        )
        .unwrap()
        .allowing_unassigned();
        let config = LocalSearchConfig {
            max_steps: 10,
            assert_scores: true,
            ..Default::default()
        };
        let s = schema(true, false);
        let mut q = provider(&s, kind, None, vec![0, 1]);
        let mut net = ConstraintNetwork::new(2);
        net.add_constraint("assigned", vec![0, 1], move |v| {
            SimpleScore::new(sign * v.iter().filter(|v| v.is_some()).count() as i64)
        })
        .unwrap();
        let full =
            local_search_from_with_provider(&p, &net, p.initial_assignment(), config, &mut q)
                .unwrap()
                .best
                .unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<i32>)>();
        b.score("assigned", source.port(), move |r| {
            SimpleScore::new(sign * i64::from(r.1.is_some()))
        })
        .unwrap();
        let stream = stream_local_search_from_with_provider(
            &p,
            b.build(),
            source,
            p.initial_assignment(),
            config,
            &mut q,
        )
        .unwrap()
        .best
        .unwrap();
        assert_eq!(full, stream);
        assert_eq!(full.score, SimpleScore::new(if sign > 0 { 2 } else { 0 }));
        let s = schema(false, true);
        let mut q = provider(&s, kind, None, vec![0, 1]);
        let mut net = ListConstraintNetwork::new(2, 2);
        net.add_constraint(
            "assigned",
            vec![ListScope::Entity(0), ListScope::Entity(1)],
            move |facts| {
                SimpleScore::new(
                    sign * facts
                        .iter()
                        .map(|v| match v {
                            ListFact::Entity(v) => v.len() as i64,
                            _ => 0,
                        })
                        .sum::<i64>(),
                )
            },
        )
        .unwrap();
        let full = list_local_search_from_with_provider(
            &lists,
            &net,
            lists.initial_assignment(),
            config,
            &mut q,
        )
        .unwrap()
        .best
        .unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = ListStreamSources::new(&mut b);
        b.score("assigned", source.entities.port(), move |(_, v)| {
            SimpleScore::new(sign * v.len() as i64)
        })
        .unwrap();
        let stream = list_stream_local_search_from_with_provider(
            &lists,
            b.build(),
            source,
            lists.initial_assignment(),
            config,
            &mut q,
        )
        .unwrap()
        .best
        .unwrap();
        assert_eq!(full, stream);
        assert_eq!(full.score, SimpleScore::new(if sign > 0 { 2 } else { 0 }));
        let p = MixedProblem::new(p, lists);
        let s = schema(true, true);
        let mut q = provider(&s, kind, None, vec![0, 1]);
        let mut net = MixedConstraintNetwork::new(2, 2, 2);
        net.add_constraint(
            "assigned",
            vec![
                MixedScope::Basic(0),
                MixedScope::Basic(1),
                MixedScope::ListEntity(0),
                MixedScope::ListEntity(1),
            ],
            move |facts| {
                SimpleScore::new(
                    sign * facts
                        .iter()
                        .map(|v| match v {
                            MixedFact::Basic(v) => i64::from(v.is_some()),
                            MixedFact::ListEntity(v) => v.len() as i64,
                            _ => 0,
                        })
                        .sum::<i64>(),
                )
            },
        )
        .unwrap();
        let mixed_config = MixedLocalSearchConfig {
            search: config,
            ..Default::default()
        };
        let full = mixed_local_search_from_with_provider(
            &p,
            &net,
            p.initial_assignment(),
            mixed_config,
            &mut q,
        )
        .unwrap()
        .best
        .unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = MixedStreamSources::new(&mut b);
        b.score("basic", source.basic.port(), move |(_, v)| {
            SimpleScore::new(sign * i64::from(v.is_some()))
        })
        .unwrap();
        b.score("list", source.lists.entities.port(), move |(_, v)| {
            SimpleScore::new(sign * v.len() as i64)
        })
        .unwrap();
        let stream = mixed_stream_local_search_from_with_provider(
            &p,
            b.build(),
            source,
            p.initial_assignment(),
            mixed_config,
            &mut q,
        )
        .unwrap()
        .best
        .unwrap();
        assert_eq!(full, stream);
        assert_eq!(full.score, SimpleScore::new(if sign > 0 { 4 } else { 0 }));
    }
}
