//! Original tests for independently scoped, composable pillar neighborhoods.
use timetuck::*;

fn policy() -> PillarMoveProvider {
    PillarMoveProvider {
        swaps: false,
        ..Default::default()
    }
}
fn basic() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 4]).unwrap()
}
fn scopes() -> (ScopedPillarMoveProvider, ScopedPillarMoveProvider) {
    (
        policy().with_schema(PillarSchema::new(vec![vec![0], vec![2]]).unwrap()),
        policy().with_schema(PillarSchema::new(vec![vec![1], vec![3]]).unwrap()),
    )
}

#[test]
fn scope_overrides_problem_schema_and_regroups_without_cloning_values() {
    use std::{cell::Cell, rc::Rc};
    #[derive(Debug)]
    struct Value(i32, Rc<Cell<usize>>);
    impl PartialEq for Value {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl Eq for Value {}
    impl Clone for Value {
        fn clone(&self) -> Self {
            self.1.set(self.1.get() + 1);
            Self(self.0, self.1.clone())
        }
    }
    let clones = Rc::new(Cell::new(0));
    let p = FiniteProblem::new(
        (0..4)
            .map(|_| {
                Variable::new(vec![Value(0, clones.clone()), Value(1, clones.clone())])
                    .with_initial(Value(0, clones.clone()))
            })
            .collect(),
    )
    .unwrap()
    .with_pillar_schema(PillarSchema::new(vec![vec![1], vec![3]]).unwrap())
    .unwrap();
    let provider = scopes().0;
    let mut a = p.initial_assignment();
    clones.set(0);
    let cursor = provider.select(&p, &a).unwrap();
    assert_eq!(clones.get(), 0);
    let candidates: Vec<_> = cursor.collect();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].touched_variables(), vec![0, 2]);
    p.apply(
        &mut a,
        &Move::Change {
            variable: 2,
            value: Some(Value(1, clones.clone())),
        },
    )
    .unwrap()
    .commit();
    let candidates: Vec<_> = provider.select(&p, &a).unwrap().collect();
    assert_eq!(candidates.len(), 2);
    assert_eq!(
        provider
            .schema
            .groups(&p, &a)
            .unwrap()
            .iter()
            .map(Pillar::variables)
            .collect::<Vec<_>>(),
        vec![vec![0], vec![2]]
    );
    assert_eq!(p.pillar_schema().unwrap().rows(), &[vec![1], vec![3]]);
    assert!(provider.select(&p, &Assignment::new(vec![])).is_err());
    let bad = policy().with_schema(PillarSchema::new(vec![vec![4]]).unwrap());
    assert!(matches!(
        bad.select(&p, &a),
        Err(ModelError::Move(MoveError {
            variable: 4,
            variable_count: 4
        }))
    ));
}

#[test]
fn field_bindings_preserve_columns_and_reject_unrelated_or_aliased_fields() {
    struct Solution;
    struct Task;
    struct Other;
    fn schema() -> PlanningSchema {
        PlanningSchema::new::<Solution>(vec![
            PlanningEntityDefinition::of::<Task>(vec![
                PlanningVariableDefinition::basic::<i32>("x", false),
                PlanningVariableDefinition::basic::<i32>("y", false),
                PlanningVariableDefinition::list::<i32>("items", false),
            ]),
            PlanningEntityDefinition::of::<Other>(vec![PlanningVariableDefinition::basic::<i32>(
                "x", false,
            )]),
        ])
        .unwrap()
    }
    let s = schema();
    let field = |name, slots| SchemaVariableSlots {
        variable: s
            .genuine_entity::<Task>()
            .unwrap()
            .genuine_variable(name)
            .unwrap(),
        slots,
    };
    let rows =
        PillarSchema::from_fields(&[field("y", vec![3, 1]), field("x", vec![2, 0])]).unwrap();
    assert_eq!(rows.rows(), &[vec![1, 0], vec![3, 2]]);
    assert!(matches!(
        PillarSchema::from_fields(&[]),
        Err(SchemaMoveError::NoMatchingVariables)
    ));
    assert!(matches!(
        PillarSchema::from_fields(&[field("items", vec![0])]),
        Err(SchemaMoveError::WrongModel)
    ));
    assert!(matches!(
        PillarSchema::from_fields(&[field("x", vec![0]), field("x", vec![1])]),
        Err(SchemaMoveError::DuplicateBinding(_))
    ));
    assert!(matches!(
        PillarSchema::from_fields(&[field("x", vec![0]), field("y", vec![1, 2])]),
        Err(SchemaMoveError::BindingLength(_))
    ));
    assert!(matches!(
        PillarSchema::from_fields(&[field("x", vec![0]), field("y", vec![0])]),
        Err(SchemaMoveError::AliasedSlot { slot: 0, .. })
    ));
    let other = SchemaVariableSlots {
        variable: s
            .genuine_entity::<Other>()
            .unwrap()
            .genuine_variable("x")
            .unwrap(),
        slots: vec![1],
    };
    assert!(matches!(
        PillarSchema::from_fields(&[field("x", vec![0]), other]),
        Err(SchemaMoveError::WrongModel)
    ));
    let foreign = schema();
    let other = SchemaVariableSlots {
        variable: foreign
            .genuine_entity::<Task>()
            .unwrap()
            .genuine_variable("y")
            .unwrap(),
        slots: vec![1],
    };
    assert!(matches!(
        PillarSchema::from_fields(&[field("x", vec![0]), other]),
        Err(SchemaMoveError::ForeignSchema)
    ));
}

#[test]
fn scoped_subpillars_preserve_pins_ranges_and_optional_values() {
    for pin in [false, true] {
        let p = FiniteProblem::new(vec![
            Variable::new(vec![0, 1])
                .allowing_unassigned()
                .with_initial(0),
            Variable::new(vec![0, 1])
                .allowing_unassigned()
                .with_initial(0),
            Variable::new(vec![0])
                .allowing_unassigned()
                .with_initial(0)
                .with_pinned(pin),
            Variable::new(vec![0, 1])
                .allowing_unassigned()
                .with_initial(1),
            Variable::new(vec![0, 1]).allowing_unassigned(),
        ])
        .unwrap();
        let a = p.initial_assignment();
        let provider = PillarMoveProvider {
            left: PillarSelectionPolicy::subsets(1, 2).unwrap(),
            right: PillarSelectionPolicy::whole(),
            ..Default::default()
        }
        .with_schema(PillarSchema::new(vec![vec![0], vec![2], vec![3], vec![4]]).unwrap());
        let mut expected = std::collections::BTreeSet::new();
        // Independent single-column subset/swap oracle, including duplicate successors.
        let groups = if pin {
            vec![vec![0], vec![3], vec![4]]
        } else {
            vec![vec![0, 2], vec![3], vec![4]]
        };
        for group in &groups {
            for mask in 1usize..(1 << group.len()) {
                let left: Vec<_> = group
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| mask & (1 << i) != 0)
                    .map(|(_, v)| *v)
                    .collect();
                for value in [None, Some(0), Some(1)] {
                    if value == a.values()[left[0]]
                        || left.iter().any(|&i| {
                            value.is_some_and(|v| !p.variables()[i].domain().contains(&v))
                        })
                    {
                        continue;
                    }
                    let mut values = a.values().to_vec();
                    for &i in &left {
                        values[i] = value;
                    }
                    expected.insert(values);
                }
                for right in &groups {
                    if right == group {
                        continue;
                    }
                    let lv = a.values()[left[0]];
                    let rv = a.values()[right[0]];
                    if left
                        .iter()
                        .any(|&i| rv.is_some_and(|v| !p.variables()[i].domain().contains(&v)))
                        || right
                            .iter()
                            .any(|&i| lv.is_some_and(|v| !p.variables()[i].domain().contains(&v)))
                    {
                        continue;
                    }
                    let mut values = a.values().to_vec();
                    for &i in &left {
                        values[i] = rv;
                    }
                    for &i in right {
                        values[i] = lv;
                    }
                    expected.insert(values);
                }
            }
        }
        let actual: std::collections::BTreeSet<_> = provider
            .select(&p, &a)
            .unwrap()
            .map(|m| {
                assert!(!m.touched_variables().contains(&1));
                let mut next = a.clone();
                p.apply(&mut next, &m).unwrap().commit();
                next.values().to_vec()
            })
            .collect();
        assert_eq!(actual, expected);
    }
}

#[test]
fn independent_scopes_solve_basic_and_mixed_with_both_scoring_backends() {
    for stream in [false, true] {
        let p = basic();
        let (mut left, mut right) = scopes();
        let mut provider = ProviderUnion::new(
            vec![
                (&mut left as &mut dyn MoveProvider<_, _, _>, 1),
                (&mut right, 1),
            ],
            UnionOrder::Sequential,
        )
        .unwrap();
        let config = LocalSearchConfig {
            max_steps: 2,
            assert_scores: true,
            ..Default::default()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("ones", source.port(), |(_, v)| {
                SimpleScore::new(v.unwrap_or(0) as i64)
            })
            .unwrap();
            stream_local_search_from_with_provider(
                &p,
                b.build(),
                source,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            let mut n = ConstraintNetwork::new(4);
            for i in 0..4 {
                n.add_constraint(format!("v{i}"), vec![i], |v| {
                    SimpleScore::new(v[0].copied().unwrap_or(0) as i64)
                })
                .unwrap();
            }
            local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut provider)
                .unwrap()
        };
        assert_eq!(result.completed_steps, 2);
        assert_eq!(result.best.unwrap().assignment.values(), &[Some(1); 4]);
        let p = MixedProblem::new(
            basic(),
            ListProblem::new(vec![10], vec![ListEntity::new(vec![10])]).unwrap(),
        );
        let (mut left, mut right) = scopes();
        let mut provider = ProviderUnion::new(
            vec![
                (&mut left as &mut dyn MoveProvider<_, _, _>, 1),
                (&mut right, 1),
            ],
            UnionOrder::Sequential,
        )
        .unwrap();
        let config = MixedLocalSearchConfig {
            search: config,
            joint_moves: true,
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = MixedStreamSources::new(&mut b);
            b.score(
                "ones",
                source.basic.port(),
                |(_, v): &(usize, Option<i32>)| SimpleScore::new(v.unwrap_or(0) as i64),
            )
            .unwrap();
            mixed_stream_local_search_from_with_provider(
                &p,
                b.build(),
                source,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            let mut n = MixedConstraintNetwork::new(4, 1, 1);
            for i in 0..4 {
                n.add_constraint(format!("v{i}"), vec![MixedScope::Basic(i)], |v| {
                    let MixedFact::Basic(v) = v[0] else {
                        unreachable!()
                    };
                    SimpleScore::new(v.copied().unwrap_or(0) as i64)
                })
                .unwrap();
            }
            mixed_local_search_from_with_provider(
                &p,
                &n,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        };
        let best = result.best.unwrap();
        assert_eq!(result.completed_steps, 2);
        assert_eq!(best.assignment.basic().values(), &[Some(1); 4]);
        assert_eq!(best.assignment.lists(), p.initial_assignment().lists());
    }
}
