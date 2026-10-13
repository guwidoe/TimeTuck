//! Original real factory proposals, typed inference failures and solver controls.
use std::{collections::BTreeSet, num::NonZeroUsize};
use timetuck::*;
struct World;
struct First;
struct Second;
struct Basic;
fn schema(two: bool) -> PlanningSchema {
    let mut entities = vec![
        PlanningEntityDefinition::of::<First>(vec![PlanningVariableDefinition::list::<i32>(
            "first", false,
        )]),
        PlanningEntityDefinition::of::<Basic>(vec![
            PlanningVariableDefinition::basic::<i32>("basic", false),
            PlanningVariableDefinition::basic::<i64>("other", false),
        ]),
    ];
    if two {
        entities.push(PlanningEntityDefinition::of::<Second>(vec![
            PlanningVariableDefinition::list::<i32>("second", false),
        ]));
    }
    PlanningSchema::new::<World>(entities).unwrap()
}
fn source(s: &PlanningSchema, swap: bool, reverse: bool, two: bool) -> SchemaSubListSource {
    let mut bindings = vec![SchemaVariableSlots {
        variable: s
            .genuine_entity::<First>()
            .unwrap()
            .genuine_variable("first")
            .unwrap(),
        slots: vec![0, 1],
    }];
    if two {
        bindings.push(SchemaVariableSlots {
            variable: s
                .genuine_entity::<Second>()
                .unwrap()
                .genuine_variable("second")
                .unwrap(),
            slots: vec![2, 3],
        });
    }
    (if swap {
        SchemaSubListFactory::swap()
    } else {
        SchemaSubListFactory::change()
    })
    .with_reversing(reverse)
    .bind(
        s,
        &bindings,
        0,
        NonZeroUsize::new(512).unwrap(),
        NonZeroUsize::new(4096).unwrap(),
    )
    .unwrap()
}
#[test]
fn real_sources_repeat_rebuild_reverse_and_keep_multiple_fields_separate() {
    for two in [false, true] {
        let s = schema(two);
        let mut rows = vec![ListEntity::new(vec![3, 2, 1]), ListEntity::new(vec![0])];
        if two {
            rows.extend([ListEntity::new(vec![7, 6, 5]), ListEntity::new(vec![4])]);
        }
        let p = ListProblem::new((0..if two { 8 } else { 4 }).collect(), rows).unwrap();
        let a = p.initial_assignment();
        for swap in [false, true] {
            for reversing in [false, true] {
                let mut provider = source(&s, swap, reversing, two);
                let mut replay = source(&s, swap, reversing, two);
                let moves = provider
                    .try_proposals(&p, &a)
                    .unwrap()
                    .take(4096)
                    .collect::<Vec<_>>();
                assert_eq!(moves.len(), 4096);
                assert_eq!(
                    moves,
                    replay
                        .try_proposals(&p, &a)
                        .unwrap()
                        .take(4096)
                        .collect::<Vec<_>>()
                );
                let mut orientations = BTreeSet::new();
                let mut groups = BTreeSet::new();
                for movement in &moves {
                    let (rev, owner) = match movement {
                        ListMove::SubListChange {
                            reverse, entity, ..
                        } => (*reverse, *entity),
                        ListMove::SubListSwap { reverse, left, .. } => (*reverse, left.entity),
                        _ => panic!("wrong proposal"),
                    };
                    orientations.insert(rev);
                    groups.insert(owner / 2);
                    let touched = movement.touched_entities();
                    assert!(touched.iter().all(|e| *e / 2 == owner / 2));
                    let mut actual = a.clone();
                    {
                        let _undo = p.apply(&mut actual, movement).unwrap();
                    }
                    assert_eq!(actual, a);
                }
                assert!(orientations.contains(&false));
                assert_eq!(orientations.contains(&true), reversing);
                assert_eq!(groups.len(), if two { 2 } else { 1 });
                let mut changed = a.clone();
                p.apply(&mut changed, &moves[0]).unwrap().commit();
                for movement in provider.try_proposals(&p, &changed).unwrap().take(64) {
                    assert!(p.is_move_doable(&changed, &movement).unwrap());
                }
            }
        }
    }
}
#[test]
fn binding_inference_and_open_errors_happen_before_drawing() {
    let s = schema(false);
    let field = s
        .genuine_entity::<First>()
        .unwrap()
        .genuine_variable("first")
        .unwrap();
    let factory = SchemaSubListFactory::change();
    let n = NonZeroUsize::new(10).unwrap();
    assert!(matches!(
        factory.bind(&s, &[], 0, n, NonZeroUsize::new(4096).unwrap()),
        Err(SchemaMoveError::MissingBinding(_))
    ));
    let foreign = schema(false);
    assert!(matches!(
        factory.bind(
            &s,
            &[SchemaVariableSlots {
                variable: foreign
                    .genuine_entity::<First>()
                    .unwrap()
                    .genuine_variable("first")
                    .unwrap(),
                slots: vec![0]
            }],
            0,
            n,
            NonZeroUsize::new(4096).unwrap()
        ),
        Err(SchemaMoveError::ForeignSchema)
    ));
    let only_basic =
        PlanningSchema::new::<World>(vec![PlanningEntityDefinition::of::<Basic>(vec![
            PlanningVariableDefinition::basic::<i32>("value", false),
        ])])
        .unwrap();
    assert!(matches!(
        factory.bind(&only_basic, &[], 0, n, NonZeroUsize::new(4096).unwrap()),
        Err(SchemaMoveError::CannotDeduceListVariable)
    ));
    let mut explicit = SchemaSubListFactory::change();
    explicit.config.entity = Some(PlanningType::of::<Basic>());
    explicit.config.variables = vec!["basic".into()];
    assert!(matches!(
        explicit.bind(&s, &[], 0, n, NonZeroUsize::new(4096).unwrap()),
        Err(SchemaMoveError::ListVariableRequired(_))
    ));
    let mut out = factory
        .bind(
            &s,
            &[SchemaVariableSlots {
                variable: field,
                slots: vec![9],
            }],
            0,
            n,
            NonZeroUsize::new(4096).unwrap(),
        )
        .unwrap();
    let p = ListProblem::new(vec![1i32, 2], vec![ListEntity::new(vec![1, 2])]).unwrap();
    assert!(matches!(
        out.try_proposals(&p, &p.initial_assignment()),
        Err(SchemaMoveError::SlotOutOfBounds { .. })
    ));
    let mut typed = factory
        .bind(
            &s,
            &[SchemaVariableSlots {
                variable: field,
                slots: vec![0],
            }],
            0,
            n,
            NonZeroUsize::new(4096).unwrap(),
        )
        .unwrap();
    let wrong = ListProblem::new(vec![1i64, 2], vec![ListEntity::new(vec![1, 2])]).unwrap();
    assert!(matches!(
        typed.try_proposals(&wrong, &wrong.initial_assignment()),
        Err(SchemaMoveError::WrongValueType { .. })
    ));
    let empty = ListProblem::<i32>::new(vec![], vec![ListEntity::new(vec![])]).unwrap();
    assert_eq!(
        typed
            .try_proposals(&empty, &empty.initial_assignment())
            .unwrap()
            .next(),
        None
    );
}
#[test]
fn actual_factory_provider_improves_a_real_list_solver() {
    let s = schema(false);
    let p = ListProblem::new(
        (0..4).collect(),
        vec![ListEntity::new(vec![3, 2, 1]), ListEntity::new(vec![0])],
    )
    .unwrap();
    let mut network = ListConstraintNetwork::new(2, 4);
    network
        .add_constraint(
            "ascending",
            vec![ListScope::Entity(0), ListScope::Entity(1)],
            |facts| {
                let mut score = 0;
                for fact in facts {
                    let ListFact::Entity(row) = fact else {
                        panic!()
                    };
                    score += row.windows(2).filter(|w| w[0] < w[1]).count() as i64;
                }
                SimpleScore::new(score)
            },
        )
        .unwrap();
    for swap in [false, true] {
        let mut provider = source(&s, swap, true, false);
        let best = list_local_search_from_with_provider(
            &p,
            &network,
            p.initial_assignment(),
            LocalSearchConfig {
                max_steps: 5,
                assert_scores: true,
                ..Default::default()
            },
            &mut provider,
        )
        .unwrap()
        .best
        .unwrap();
        assert!(best.score > SimpleScore::new(0));
        p.validate(&best.assignment).unwrap();
    }
}
#[test]
fn large_opening_rejects_bounded_catalog_without_cloning_or_consuming_randomness() {
    use std::{
        hash::{Hash, Hasher},
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    #[derive(Debug)]
    struct Counted {
        id: usize,
        clones: Arc<AtomicUsize>,
    }
    impl Clone for Counted {
        fn clone(&self) -> Self {
            self.clones.fetch_add(1, Ordering::Relaxed);
            Self {
                id: self.id,
                clones: self.clones.clone(),
            }
        }
    }
    impl PartialEq for Counted {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl Eq for Counted {}
    impl Hash for Counted {
        fn hash<H: Hasher>(&self, h: &mut H) {
            self.id.hash(h);
        }
    }
    let clones = Arc::new(AtomicUsize::new(0));
    let values = (0..1024)
        .map(|id| Counted {
            id,
            clones: clones.clone(),
        })
        .collect::<Vec<_>>();
    let p = ListProblem::new_indexed(
        values.clone(),
        vec![ListEntity::new(values), ListEntity::new(vec![])],
    )
    .unwrap();
    let a = p.initial_assignment();
    let s = PlanningSchema::new::<World>(vec![PlanningEntityDefinition::of::<First>(vec![
        PlanningVariableDefinition::list::<Counted>("route", false),
    ])])
    .unwrap();
    let mut source = SchemaSubListFactory::change()
        .bind(
            &s,
            &[SchemaVariableSlots {
                variable: s
                    .genuine_entity::<First>()
                    .unwrap()
                    .genuine_variable("route")
                    .unwrap(),
                slots: vec![0, 1],
            }],
            47,
            NonZeroUsize::new(32).unwrap(),
            NonZeroUsize::new(16).unwrap(),
        )
        .unwrap();
    let mut before_failure = source.clone();
    clones.store(0, Ordering::Relaxed);
    let error = source.try_proposals(&p, &a).err().unwrap();
    assert!(
        error
            .to_string()
            .contains("catalog exceeds caller limit 16")
    );
    assert_eq!(clones.load(Ordering::Relaxed), 0);
    let tiny = ListProblem::new_indexed(
        vec![
            Counted {
                id: 0,
                clones: clones.clone(),
            },
            Counted {
                id: 1,
                clones: clones.clone(),
            },
        ],
        vec![
            ListEntity::new(vec![
                Counted {
                    id: 0,
                    clones: clones.clone(),
                },
                Counted {
                    id: 1,
                    clones: clones.clone(),
                },
            ]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let initial = tiny.initial_assignment();
    assert_eq!(
        source
            .try_proposals(&tiny, &initial)
            .unwrap()
            .take(16)
            .collect::<Vec<_>>(),
        before_failure
            .try_proposals(&tiny, &initial)
            .unwrap()
            .take(16)
            .collect::<Vec<_>>()
    );
}
#[test]
fn multi_field_factory_solves_both_mixed_backends_and_preserves_basic_union_values() {
    #[derive(Clone, Debug, Eq, PartialEq)]
    enum BasicValue {
        Int(i32),
        Long(i64),
    }
    let s = schema(true);
    let p = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![BasicValue::Int(7)]).with_initial(BasicValue::Int(7)),
            Variable::new(vec![BasicValue::Long(8)]).with_initial(BasicValue::Long(8)),
        ])
        .unwrap(),
        ListProblem::new(
            (0..8).collect(),
            vec![
                ListEntity::new(vec![3, 2, 1]),
                ListEntity::new(vec![0]),
                ListEntity::new(vec![7, 6, 5]),
                ListEntity::new(vec![4]),
            ],
        )
        .unwrap(),
    );
    let mut network = MixedConstraintNetwork::new(2, 4, 8);
    network
        .add_constraint(
            "ascending",
            (0..4).map(MixedScope::ListEntity).collect(),
            |facts| {
                SimpleScore::new(
                    facts
                        .iter()
                        .map(|f| {
                            let MixedFact::ListEntity(row) = f else {
                                panic!()
                            };
                            row.windows(2).filter(|w| w[0] < w[1]).count() as i64
                        })
                        .sum(),
                )
            },
        )
        .unwrap();
    let config = MixedLocalSearchConfig {
        search: LocalSearchConfig {
            max_steps: 5,
            assert_scores: true,
            ..Default::default()
        },
        ..Default::default()
    };
    for swap in [false, true] {
        let mut provider = source(&s, swap, true, true);
        let plain = mixed_local_search_from_with_provider(
            &p,
            &network,
            p.initial_assignment(),
            config.clone(),
            &mut provider,
        )
        .unwrap()
        .best
        .unwrap();
        assert!(plain.score > SimpleScore::new(0));
        assert_eq!(
            plain.assignment.basic().values(),
            [Some(BasicValue::Int(7)), Some(BasicValue::Long(8))]
        );
        p.validate(&plain.assignment).unwrap();
        let mut b = StreamGraphBuilder::new();
        let sources = MixedStreamSources::new(&mut b);
        b.score("ascending", sources.lists.entities.port(), |(_, row)| {
            SimpleScore::new(row.windows(2).filter(|w| w[0] < w[1]).count() as i64)
        })
        .unwrap();
        let mut provider = source(&s, swap, true, true);
        let stream = mixed_stream_local_search_from_with_provider(
            &p,
            b.build(),
            sources,
            p.initial_assignment(),
            config.clone(),
            &mut provider,
        )
        .unwrap()
        .best
        .unwrap();
        assert!(stream.score > SimpleScore::new(0));
        assert_eq!(stream.assignment.basic(), plain.assignment.basic());
        p.validate(&stream.assignment).unwrap();
    }
}
