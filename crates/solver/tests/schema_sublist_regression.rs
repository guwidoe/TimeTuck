//! Native schema-based list selection and solver integration; no upstream credit.
use timetuck::*;
struct Solution;
struct Basic;
struct Lists;
fn schema(optional: bool) -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Basic>(vec![PlanningVariableDefinition::basic::<i32>(
            "tasks", false,
        )]),
        PlanningEntityDefinition::of::<Lists>(vec![PlanningVariableDefinition::list::<i32>(
            "tasks", optional,
        )]),
    ])
    .unwrap()
}
fn provider(s: &PlanningSchema, kind: SchemaMoveKind, slots: Vec<usize>) -> SchemaMoveProvider {
    SchemaMoveConfig::new(kind)
        .resolve(s)
        .unwrap()
        .bind(&[SchemaVariableSlots {
            variable: s
                .genuine_entity::<Lists>()
                .unwrap()
                .genuine_variable("tasks")
                .unwrap(),
            slots,
        }])
        .unwrap()
}
#[test]
fn list_only_deduction_skips_basic_entities_even_with_same_named_fields() {
    let s = schema(false);
    for kind in [
        SchemaMoveKind::ListChange,
        SchemaMoveKind::ListSwap,
        SchemaMoveKind::SubListChange(SubListChangeConfig::default()),
        SchemaMoveKind::SubListSwap(SubListSwapConfig::default()),
    ] {
        let mut c = SchemaMoveConfig::new(kind);
        for explicit_name in [false, true] {
            if explicit_name {
                c.variables = vec!["tasks".into()];
            }
            let plan = c.resolve(&s).unwrap();
            assert_eq!(plan.branches().len(), 1);
            assert_eq!(
                plan.branches()[0].variables[0],
                s.genuine_entity::<Lists>()
                    .unwrap()
                    .genuine_variable("tasks")
                    .unwrap()
            );
        }
        c.entity = Some(PlanningType::of::<Basic>());
        assert!(matches!(
            c.resolve(&s),
            Err(SchemaMoveError::ListVariableRequired(_))
        ));
    }
}
#[test]
fn span_settings_preserve_candidate_sequences_and_optional_unassignment() {
    for optional in [false, true] {
        let s = schema(optional);
        let p = ListProblem::new(
            (0..6).collect(),
            vec![
                ListEntity::new(vec![0, 1, 2, 3]),
                ListEntity::new(vec![4, 5]),
            ],
        )
        .unwrap();
        let p = if optional { p.allowing_unassigned() } else { p };
        let a = p.initial_assignment();
        for reverse in [false, true] {
            let change = SubListChangeConfig::new(2, 3)
                .unwrap()
                .with_reversing(reverse);
            let swap = SubListSwapConfig::asymmetric(1, 2, 2, 3)
                .unwrap()
                .with_reversing(reverse);
            for (kind, expected) in [
                (
                    SchemaMoveKind::SubListChange(change),
                    list_sublist_change_moves_with_config(&p, &a, change)
                        .unwrap()
                        .collect::<Vec<_>>(),
                ),
                (
                    SchemaMoveKind::SubListSwap(swap),
                    list_sublist_swap_moves_with_config(&p, &a, swap)
                        .unwrap()
                        .collect::<Vec<_>>(),
                ),
            ] {
                let q = provider(&s, kind, vec![0, 1]);
                let moves = q.try_list_moves(&p, &a, 0).unwrap().collect::<Vec<_>>();
                assert!(!moves.is_empty());
                assert_eq!(moves, expected);
                let mut unassignments = 0;
                for m in &moves {
                    match m {
                        ListMove::SubListChange {
                            start,
                            end,
                            reverse: r,
                            ..
                        } => {
                            assert!((2..=3).contains(&(end - start)));
                            assert!(reverse || !r);
                        }
                        ListMove::SubListSwap { reverse: r, .. } => assert!(reverse || !r),
                        ListMove::Unassign { start, end, .. } => {
                            assert!(optional);
                            assert!((2..=3).contains(&(end - start)));
                            unassignments += 1;
                        }
                        _ => panic!("unexpected family"),
                    }
                    let mut state = a.clone();
                    {
                        let _undo = p.apply(&mut state, m).unwrap();
                    }
                    assert_eq!(state, a);
                }
                if matches!(kind, SchemaMoveKind::SubListChange(_)) {
                    assert_eq!(unassignments > 0, optional);
                }
            }
        }
        let c = SubListChangeConfig::default().with_unassignment(true);
        let q = provider(&s, SchemaMoveKind::SubListChange(c), vec![0, 1]);
        assert_eq!(q.try_list_moves(&p, &a, 0).is_ok(), optional);
        let q = provider(
            &s,
            SchemaMoveKind::SubListChange(c.with_unassignment(false)),
            vec![0, 1],
        );
        assert!(
            q.try_list_moves(&p, &a, 0)
                .unwrap()
                .all(|m| !matches!(m, ListMove::Unassign { .. }))
        );
    }
}
#[test]
fn scoped_spans_preserve_pins_and_refresh_after_commit() {
    let s = schema(false);
    let p = ListProblem::new(
        (0..8).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![3, 4]),
            ListEntity::new(vec![5, 6, 7]),
        ],
    )
    .unwrap();
    for kind in [
        SchemaMoveKind::SubListChange(SubListChangeConfig::new(2, 2).unwrap()),
        SchemaMoveKind::SubListSwap(SubListSwapConfig::new(2, 2).unwrap()),
    ] {
        let q = provider(&s, kind, vec![2, 0]);
        let mut a = p.initial_assignment();
        for step in 0..2 {
            let moves = q.try_list_moves(&p, &a, step).unwrap().collect::<Vec<_>>();
            assert!(!moves.is_empty());
            for m in &moves {
                assert!(!m.touched_entities().contains(&1));
                let mut copy = a.clone();
                p.apply(&mut copy, m).unwrap().commit();
                assert_eq!(copy.lists()[0][0], 0);
                assert_eq!(copy.lists()[1], vec![3, 4]);
                p.validate(&copy).unwrap();
            }
            p.apply(&mut a, &moves[0]).unwrap().commit();
        }
    }
}
fn score(values: &[i32]) -> SimpleScore {
    SimpleScore::new(if values == [2, 3, 0, 1] { 10 } else { 0 })
}
#[test]
fn schema_sublist_moves_solve_list_and_mixed_models_on_both_backends() {
    let s = schema(false);
    for kind in [
        SchemaMoveKind::SubListChange(
            SubListChangeConfig::new(2, 2)
                .unwrap()
                .with_reversing(false),
        ),
        SchemaMoveKind::SubListSwap(SubListSwapConfig::new(2, 2).unwrap().with_reversing(false)),
    ] {
        let mut q = provider(&s, kind, vec![0]);
        let p =
            ListProblem::new((0..4).collect(), vec![ListEntity::new(vec![0, 1, 2, 3])]).unwrap();
        let config = LocalSearchConfig {
            max_steps: 1,
            assert_scores: true,
            ..Default::default()
        };
        let mut n = ListConstraintNetwork::new(1, 4);
        n.add_constraint("order", vec![ListScope::Entity(0)], |facts| {
            let ListFact::Entity(v) = facts[0] else {
                panic!()
            };
            score(v)
        })
        .unwrap();
        let plain =
            list_local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut q)
                .unwrap()
                .best
                .unwrap();
        let mut b = StreamGraphBuilder::new();
        let source = ListStreamSources::new(&mut b);
        b.score("order", source.entities.port(), |(_, v)| score(v))
            .unwrap();
        let stream = list_stream_local_search_from_with_provider(
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
        assert_eq!(plain.score, SimpleScore::new(10));
        assert_eq!(stream.score, plain.score);
        assert_eq!(stream.assignment, plain.assignment);
        let p = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![9]).with_initial(9)]).unwrap(),
            p,
        );
        let config = MixedLocalSearchConfig {
            search: config,
            ..Default::default()
        };
        let mut n = MixedConstraintNetwork::new(1, 1, 4);
        n.add_constraint("order", vec![MixedScope::ListEntity(0)], |facts| {
            let MixedFact::ListEntity(v) = facts[0] else {
                panic!()
            };
            score(v)
        })
        .unwrap();
        let plain =
            mixed_local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut q)
                .unwrap()
                .best
                .unwrap();
        let mut b = StreamGraphBuilder::new();
        let source = MixedStreamSources::new(&mut b);
        b.score("order", source.lists.entities.port(), |(_, v)| score(v))
            .unwrap();
        let stream = mixed_stream_local_search_from_with_provider(
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
        assert_eq!(plain.score, SimpleScore::new(10));
        assert_eq!(stream.score, plain.score);
        assert_eq!(stream.assignment, plain.assignment);
        assert_eq!(plain.assignment.basic().values(), &[Some(9)]);
    }
}
