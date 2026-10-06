//! Original structural-dependency regressions, no upstream method credit.
use std::cell::Cell;
use timetuck::*;
fn problem() -> ListProblem<usize> {
    ListProblem::new(
        vec![0, 1, 2, 3],
        vec![ListEntity::new(vec![1, 0, 2]), ListEntity::new(vec![3])],
    )
    .unwrap()
    .with_dependencies(vec![vec![], vec![0], vec![1], vec![]])
    .unwrap()
}
#[test]
fn preparation_clears_cycles_and_downstream_values_but_preserves_pins_and_metadata() {
    let p = problem();
    let original = p.initial_assignment();
    assert_eq!(p.inconsistent_values(&original).unwrap(), vec![0, 1, 2]);
    assert_eq!(
        p.prepare_assignment(&original).unwrap().lists(),
        &[vec![], vec![3]]
    );
    assert_eq!(p.initial_assignment(), original);
    for full in [false, true] {
        let e = if full {
            ListEntity::new(vec![1, 0, 2]).with_pinned(true)
        } else {
            ListEntity::new(vec![1, 0, 2]).with_pinned_prefix(1)
        };
        let p = ListProblem::new(vec![0, 1, 2], vec![e])
            .unwrap()
            .with_dependencies(vec![vec![], vec![0], vec![1]])
            .unwrap();
        assert_eq!(
            p.prepare_assignment(&p.initial_assignment()),
            Err(ListError::PinnedDependencyCycle {
                value: if full { 0 } else { 1 },
                position: ListPosition {
                    entity: 0,
                    index: usize::from(full)
                }
            })
        );
    }
    // A pinned downstream value is also protected, even when its list has no cycle.
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![
            ListEntity::new(vec![1, 0]),
            ListEntity::new(vec![2]).with_pinned(true),
        ],
    )
    .unwrap()
    .with_dependencies(vec![vec![], vec![0], vec![1]])
    .unwrap();
    assert_eq!(
        p.prepare_assignment(&p.initial_assignment()),
        Err(ListError::PinnedDependencyCycle {
            value: 2,
            position: ListPosition {
                entity: 1,
                index: 0
            }
        })
    );
}
#[test]
fn dependency_metadata_and_missing_inputs_are_distinct_from_dynamic_cycles() {
    let base = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![1])]).unwrap();
    assert!(matches!(
        base.clone().with_dependencies(vec![]),
        Err(ListError::DependencyCount { .. })
    ));
    assert!(matches!(
        base.clone().with_dependencies(vec![vec![2], vec![]]),
        Err(ListError::Dependency(
            DependencyError::UnknownDependency { .. }
        ))
    ));
    assert!(matches!(
        base.clone().with_dependencies(vec![vec![1], vec![0]]),
        Err(ListError::Dependency(DependencyError::FixedCycle(_)))
    ));
    let p = base.with_dependencies(vec![vec![], vec![0]]).unwrap();
    assert_eq!(
        p.prepare_assignment(&p.initial_assignment()).unwrap(),
        p.initial_assignment()
    );
    let reset = p
        .with_initial_assignment(&ListAssignment::new(vec![vec![0, 1]]))
        .unwrap();
    assert!(matches!(
        reset.validate(&ListAssignment::new(vec![vec![1, 0]])),
        Err(ListError::DependencyCycle { .. })
    ));
    assert!(matches!(
        p.inconsistent_values(&ListAssignment::new(vec![vec![9]])),
        Err(ListError::ValueOutsideDomain)
    ));
}
#[test]
fn rejected_composites_roll_back_before_scoring_in_both_engines() {
    let p = problem();
    let a = p.prepare_assignment(&p.initial_assignment()).unwrap();
    let m = ListMove::Composite(vec![
        ListMove::Assign {
            value: 1,
            destination: ListPosition {
                entity: 0,
                index: 0,
            },
        },
        ListMove::Assign {
            value: 0,
            destination: ListPosition {
                entity: 0,
                index: 1,
            },
        },
    ]);
    let calls = Cell::new(0);
    let mut n = ListConstraintNetwork::new(2, 4);
    n.add_constraint("calls", vec![ListScope::Entity(0)], |_| {
        calls.set(calls.get() + 1);
        SimpleScore::ZERO
    })
    .unwrap();
    let mut d = ListScoreDirector::new(&p, &n, a.clone()).unwrap();
    let before = calls.get();
    assert!(matches!(
        d.apply(&m),
        Err(ListScoringError::Model(ListError::DependencyCycle { .. }))
    ));
    assert_eq!(calls.get(), before);
    assert_eq!(d.assignment(), &a);
    d.assert_consistent().unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = ListStreamSources::new(&mut b);
    b.score("calls", s.entities.port(), |_| {
        calls.set(calls.get() + 1);
        SimpleScore::ZERO
    })
    .unwrap();
    let mut d = ListStreamScoreDirector::new(&p, b.build(), s, a.clone()).unwrap();
    let before = calls.get();
    assert!(matches!(
        d.apply(&m),
        Err(ListStreamScoringError::Model(
            ListError::DependencyCycle { .. }
        ))
    ));
    assert_eq!(calls.get(), before);
    assert_eq!(d.assignment(), &a);
    d.assert_consistent().unwrap();
}

#[test]
fn exact_and_bounded_search_never_score_cyclic_orders() {
    let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0], vec![1]])
        .unwrap();
    for stream in [false, true] {
        for strategy in [
            ExactSearchStrategy::Enumerate,
            ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig::default()),
        ] {
            let plan = SolverSequence {
                phases: vec![SolverPhase::Exact(ExactPhaseConfig {
                    strategy,
                    ..Default::default()
                })],
                ..Default::default()
            };
            let r = if stream {
                solve_list_stream(
                    &p,
                    || {
                        let mut b = StreamGraphBuilder::<SimpleScore>::new();
                        let s = ListStreamSources::new(&mut b);
                        b.score("order", s.entities.port(), |(_, list)| {
                            assert!(list.windows(2).all(|w| w[0] < w[1]));
                            SimpleScore::ZERO
                        })?;
                        Ok((b.build(), s))
                    },
                    plan,
                )
                .unwrap()
            } else {
                let mut n = ListConstraintNetwork::new(1, 3);
                n.add_constraint("order", vec![ListScope::Entity(0)], |f| {
                    let ListFact::Entity(list) = f[0] else {
                        panic!()
                    };
                    assert!(list.windows(2).all(|w| w[0] < w[1]));
                    SimpleScore::ZERO
                })
                .unwrap();
                solve_list(&p, &n, plan).unwrap()
            };
            assert!(r.is_optimal());
            assert_eq!(r.assignment.lists(), &[vec![0, 1, 2]]);
        }
    }
}
#[test]
fn mixed_startup_and_trial_selection_use_the_list_dependency_policy() {
    let lists = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![1, 0])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0]])
        .unwrap();
    let p = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![7]).with_initial(7).with_pinned(true),
        ])
        .unwrap(),
        lists,
    );
    let initial = p.initial_assignment();
    for stream in [false, true] {
        let plan = MixedSolverSequence {
            phases: vec![
                MixedSolverPhase::Construction(MixedConstructionConfig::default()),
                MixedSolverPhase::LocalSearch(MixedLocalSearchConfig::default()),
            ],
            ..Default::default()
        };
        let r = if stream {
            solve_mixed_stream(
                &p,
                || {
                    let mut b = StreamGraphBuilder::<SimpleScore>::new();
                    let s = MixedStreamSources::new(&mut b);
                    b.score("order", s.lists.entities.port(), |(_, list)| {
                        assert!(list.windows(2).all(|w| w[0] < w[1]));
                        SimpleScore::ZERO
                    })?;
                    Ok((b.build(), s))
                },
                plan,
            )
            .unwrap()
        } else {
            let mut n = MixedConstraintNetwork::new(1, 1, 2);
            n.add_constraint("order", vec![MixedScope::ListEntity(0)], |f| {
                let MixedFact::ListEntity(list) = f[0] else {
                    panic!()
                };
                assert!(list.windows(2).all(|w| w[0] < w[1]));
                SimpleScore::ZERO
            })
            .unwrap();
            solve_mixed(&p, &n, plan).unwrap()
        };
        assert!(r.complete);
        assert_eq!(r.assignment.basic().values(), &[Some(7)]);
        assert_eq!(r.assignment.lists().lists(), &[vec![0, 1]]);
        assert_eq!(p.initial_assignment(), initial);
    }
}

#[test]
fn repaired_baseline_does_not_hide_later_improvements_or_exact_solution_events() {
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![1, 0])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0]])
        .unwrap();
    for stream in [false, true] {
        for phase in [
            SolverPhase::Construction(ConstructionConfig::default()),
            SolverPhase::Exact(ExactPhaseConfig::default()),
            SolverPhase::Exact(ExactPhaseConfig {
                strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig::default()),
                ..Default::default()
            }),
        ] {
            let plan = SolverSequence {
                phases: vec![phase],
                ..Default::default()
            };
            let mut best = vec![];
            let observer = |event: SolverEvent<'_, ListAssignment<usize>, SimpleScore>| {
                if let SolverEvent::Best(update) = event {
                    best.push(update.assignment.clone());
                }
            };
            let result = if stream {
                solve_list_stream_with_events(
                    &p,
                    || {
                        let mut b = StreamGraphBuilder::new();
                        let s = ListStreamSources::new(&mut b);
                        Ok((b.build(), s))
                    },
                    plan,
                    observer,
                )
                .unwrap()
            } else {
                solve_list_with_events(&p, &ListConstraintNetwork::new(1, 2), plan, observer)
                    .unwrap()
            };
            assert!(result.complete);
            assert!(!best.is_empty());
            assert!(best.iter().all(|a| !a.lists()[0].is_empty()));
            assert_eq!(best.last(), Some(&result.assignment));
        }
    }
}
