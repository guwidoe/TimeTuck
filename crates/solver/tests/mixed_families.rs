//! Original scoped mixed-neighborhood and end-to-end provider regressions.
use timetuck::*;
fn problem() -> MixedProblem<i32, i32> {
    MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).allowing_unassigned(); 2]).unwrap(),
        ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![]); 2])
            .unwrap()
            .allowing_unassigned(),
    )
}
#[test]
fn configured_components_and_joint_products_match_independent_component_moves() {
    let p = problem();
    let lists = [
        vec![vec![], vec![]],
        vec![vec![0], vec![]],
        vec![vec![1], vec![]],
        vec![vec![], vec![0]],
        vec![vec![], vec![1]],
        vec![vec![0, 1], vec![]],
        vec![vec![1, 0], vec![]],
        vec![vec![], vec![0, 1]],
        vec![vec![], vec![1, 0]],
        vec![vec![0], vec![1]],
        vec![vec![1], vec![0]],
    ];
    let mut checked = 0;
    for first in [None, Some(0), Some(1)] {
        for second in [None, Some(0), Some(1)] {
            for list in &lists {
                let a = MixedAssignment::new(
                    Assignment::new(vec![first, second]),
                    ListAssignment::new(list.clone()),
                );
                for selected in [&[][..], &[0][..], &[1][..], &[1, 0][..]] {
                    for b in [None, Some(Neighborhood::Change), Some(Neighborhood::Swap)] {
                        for l in [None, Some(Neighborhood::Change), Some(Neighborhood::Swap)] {
                            for joint in [false, true] {
                                let mut basics = Vec::new();
                                match b {
                                    Some(Neighborhood::Change) => {
                                        for &i in selected {
                                            for v in [Some(0), Some(1), None] {
                                                if a.basic().values()[i] != v {
                                                    basics.push(Move::Change {
                                                        variable: i,
                                                        value: v,
                                                    });
                                                }
                                            }
                                        }
                                    }
                                    Some(Neighborhood::Swap) => {
                                        for &i in selected {
                                            for &j in selected {
                                                if i != j
                                                    && a.basic().values()[i]
                                                        != a.basic().values()[j]
                                                {
                                                    basics.push(Move::Swap { left: i, right: j });
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                                let lists = l
                                    .map(|n| {
                                        list_moves(p.lists(), a.lists(), n)
                                            .unwrap()
                                            .collect::<Vec<_>>()
                                    })
                                    .unwrap_or_default();
                                let empty_b = Move::Composite(vec![]);
                                let empty_l = ListMove::Composite(vec![]);
                                let mut expected: Vec<_> = basics
                                    .iter()
                                    .map(|b| MixedMove {
                                        basic: b.clone(),
                                        lists: empty_l.clone(),
                                    })
                                    .chain(lists.iter().map(|l| MixedMove {
                                        basic: empty_b.clone(),
                                        lists: l.clone(),
                                    }))
                                    .collect();
                                if joint {
                                    for b in &basics {
                                        for l in &lists {
                                            expected.push(MixedMove {
                                                basic: b.clone(),
                                                lists: l.clone(),
                                            });
                                        }
                                    }
                                }
                                let c = MixedNeighborhood {
                                    pillar_schema: None,
                                    basic: b,
                                    lists: l,
                                    basic_variables: Some(selected),
                                    joint_moves: joint,
                                };
                                let actual = mixed_moves_configured(&p, &a, c)
                                    .unwrap()
                                    .collect::<Vec<_>>();
                                assert_eq!(actual, expected);
                                for movement in actual {
                                    let mut next = a.clone();
                                    p.apply(&mut next, &movement).unwrap().commit();
                                    p.validate(&next).unwrap();
                                }
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(checked, 7128);
}
#[test]
fn scopes_preserve_pins_and_other_basic_slots_for_rich_families() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![0, 1]).with_initial(0),
            Variable::new(vec![0, 1]).with_initial(0),
            Variable::new(vec![0, 1]).with_initial(1).with_pinned(true),
        ])
        .unwrap(),
        ListProblem::new(
            (0..8).collect(),
            vec![
                ListEntity::new((0..6).collect()).with_pinned_prefix(1),
                ListEntity::new(vec![6, 7]).with_pinned(true),
            ],
        )
        .unwrap(),
    );
    let a = p.initial_assignment();
    for basic in [
        Neighborhood::Change,
        Neighborhood::SubPillar(SubPillarConfig::new(
            SubPillarSelection::combinations(1, 2).unwrap(),
        )),
    ] {
        for lists in [
            Neighborhood::Swap,
            Neighborhood::SubListChange(SubListChangeConfig::default()),
            Neighborhood::KOpt(ListKOptConfig::new(2, 3).unwrap()),
        ] {
            let c = MixedNeighborhood {
                pillar_schema: None,
                basic: Some(basic),
                lists: Some(lists),
                basic_variables: Some(&[0]),
                joint_moves: true,
            };
            let moves = mixed_moves_configured(&p, &a, c)
                .unwrap()
                .collect::<Vec<_>>();
            assert!(!moves.is_empty());
            for m in moves {
                assert!(m.basic.touched_variables().iter().all(|i| *i == 0));
                let mut next = a.clone();
                p.apply(&mut next, &m).unwrap().commit();
                assert_eq!(&next.basic().values()[1..], &a.basic().values()[1..]);
                assert_eq!(next.lists().lists()[0][0], 0);
                assert_eq!(next.lists().lists()[1], vec![6, 7]);
            }
        }
    }
    assert_eq!(p.initial_assignment(), a);
}
#[test]
fn disabled_children_skip_configuration_validation_and_union_retains_occurrences() {
    let p = problem();
    let a = p.initial_assignment();
    let bad = MixedNeighborhood {
        basic_variables: Some(&[9]),
        ..Default::default()
    };
    assert!(matches!(
        mixed_moves_configured(&p, &a, bad),
        Err(MixedNeighborhoodError::UnknownBasicVariable {
            variable: 9,
            count: 2
        })
    ));
    assert!(matches!(
        mixed_moves_configured(
            &p,
            &a,
            MixedNeighborhood {
                basic_variables: Some(&[0, 0]),
                ..bad
            }
        ),
        Err(MixedNeighborhoodError::DuplicateBasicVariable { variable: 0 })
    ));
    let disabled = MixedNeighborhood {
        basic: None,
        lists: None,
        ..bad
    };
    assert_eq!(mixed_moves_configured(&p, &a, disabled).unwrap().count(), 0);
    let basic = MixedNeighborhood {
        lists: None,
        basic_variables: Some(&[1]),
        ..Default::default()
    };
    let list = MixedNeighborhood {
        basic: None,
        ..Default::default()
    };
    let children = [(bad, 0), (basic, 1), (list, 2), (basic, 1)];
    let expected = children
        .iter()
        .filter(|(_, w)| *w > 0)
        .flat_map(|(c, _)| mixed_moves_configured(&p, &a, *c).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        mixed_moves_configured_union(&p, &a, &children, UnionOrder::Sequential, 0)
            .unwrap()
            .collect::<Vec<_>>(),
        expected
    );
    for step in 0..20 {
        let run = || {
            mixed_moves_configured_union(&p, &a, &children, UnionOrder::Random { seed: 45 }, step)
                .unwrap()
                .collect::<Vec<_>>()
        };
        let actual = run();
        assert_eq!(actual, run());
        let mut remaining = expected.clone();
        for m in actual {
            let i = remaining.iter().position(|e| e == &m).unwrap();
            remaining.remove(i);
        }
        assert!(remaining.is_empty());
    }
}
struct Provider<'a> {
    children: &'a [(MixedNeighborhood<'a>, u32)],
}
impl MoveProvider<MixedProblem<i32, i32>, MixedAssignment<i32, i32>, MixedMove<i32, i32>>
    for Provider<'_>
{
    fn moves<'a>(
        &'a mut self,
        p: &'a MixedProblem<i32, i32>,
        a: &'a MixedAssignment<i32, i32>,
        step: u64,
    ) -> Box<dyn Iterator<Item = MixedMove<i32, i32>> + 'a>
    where
        MixedMove<i32, i32>: 'a,
    {
        Box::new(
            mixed_moves_configured_union(p, a, self.children, UnionOrder::Sequential, step)
                .unwrap(),
        )
    }
}
fn objective(b: Option<&i32>, l: &[i32]) -> SimpleScore {
    SimpleScore::new(if b == Some(&1) && l == [1, 0] {
        10
    } else if b == Some(&0) && l == [0, 1] {
        0
    } else {
        -1
    })
}
#[test]
fn independently_configured_joint_moves_escape_coupled_trap_in_both_backends() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 2]).unwrap(),
        ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])]).unwrap(),
    );
    let mut n = MixedConstraintNetwork::new(2, 1, 2);
    n.add_constraint(
        "coupled",
        vec![MixedScope::Basic(0), MixedScope::ListEntity(0)],
        |v| {
            let MixedFact::Basic(b) = v[0] else { panic!() };
            let MixedFact::ListEntity(l) = v[1] else {
                panic!()
            };
            objective(b, l)
        },
    )
    .unwrap();
    for joint in [false, true] {
        let children = [(
            MixedNeighborhood {
                pillar_schema: None,
                basic: Some(Neighborhood::Change),
                lists: Some(Neighborhood::Swap),
                basic_variables: Some(&[0]),
                joint_moves: joint,
            },
            1,
        )];
        let mut provider = Provider {
            children: &children,
        };
        let config = MixedLocalSearchConfig {
            search: LocalSearchConfig {
                max_steps: 3,
                assert_scores: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let r = mixed_local_search_from_with_provider(
            &p,
            &n,
            p.initial_assignment(),
            config,
            &mut provider,
        )
        .unwrap();
        let graph = || {
            let mut g = StreamGraphBuilder::new();
            let sources = MixedStreamSources::new(&mut g);
            let b = g.filter(sources.basic.port(), |(i, _)| *i == 0)?;
            let j = g.join_equal(&b, sources.lists.entities.port(), |_| (), |_| ())?;
            g.score("coupled", &j, |((_, b), (_, l))| objective(b.as_ref(), l))?;
            Ok((g.build(), sources))
        };
        let (g, sources) = graph().unwrap();
        let s = mixed_stream_local_search_from_with_provider(
            &p,
            g,
            sources,
            p.initial_assignment(),
            config,
            &mut provider,
        )
        .unwrap();
        let plan = MixedSolverSequence {
            limits: SolverLimits {
                max_move_evaluations: Some(20),
                ..Default::default()
            },
            phases: vec![
                MixedSolverPhase::Construction(Default::default()),
                MixedSolverPhase::LocalSearch(config),
            ],
        };
        let mut extensions = MixedSolverExtensions::default();
        extensions.move_providers.bind(1, &mut provider);
        let complete = solve_mixed_with_extensions(&p, &n, plan.clone(), extensions).unwrap();
        let mut extensions = MixedSolverExtensions::default();
        extensions.move_providers.bind(1, &mut provider);
        let streamed = solve_mixed_stream_with_extensions(&p, graph, plan, extensions).unwrap();
        assert_eq!(complete.assignment, streamed.assignment);
        assert_eq!(complete.score, streamed.score);
        assert_eq!(complete.assignment, r.best.as_ref().unwrap().assignment);
        assert_eq!(complete.phases.len(), 2);
        assert!(complete.work.move_evaluations <= 20 && streamed.work.move_evaluations <= 20);
        assert_eq!(r.best, s.best);
        assert_eq!(r.evaluations, s.evaluations);
        assert_eq!(r.completed_steps, s.completed_steps);
        let best = r.best.unwrap();
        assert_eq!(best.score, SimpleScore::new(if joint { 10 } else { 0 }));
        assert_eq!(best.assignment.basic().values()[1], Some(0));
    }
}
