//! Original component-provider integration; no upstream method credit.
use std::{cell::Cell, cmp::Ordering};
use timetuck::*;
const ORDERS: [MixedConstructionOrder; 2] = [
    MixedConstructionOrder::BasicThenLists,
    MixedConstructionOrder::ListsThenBasic,
];
fn same(_: usize, _: usize) -> Ordering {
    Ordering::Equal
}
fn same_value(_: &i32, _: &i32) -> Ordering {
    Ordering::Equal
}
fn run(
    p: &MixedProblem<i32, i32>,
    a: MixedAssignment<i32, i32>,
    stream: bool,
    config: ProviderConstructionConfig,
    provider: &mut dyn MoveProvider<MixedProblem<i32, i32>, MixedAssignment<i32, i32>, MixedMove<i32, i32>>,
) -> ProviderConstructionResult<MixedAssignment<i32, i32>, SimpleScore> {
    if stream {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let sources = MixedStreamSources::new(&mut b);
        mixed_stream_construction_from_with_provider(p, b.build(), sources, a, config, provider)
            .unwrap()
    } else {
        let n = MixedConstraintNetwork::<_, _, SimpleScore>::new(
            p.basic().variables().len(),
            p.lists().entities().len(),
            p.lists().values().len(),
        );
        mixed_construction_from_with_provider(p, &n, a, config, provider).unwrap()
    }
}
#[test]
fn joint_objective_selects_component_moves_on_both_solver_backends() {
    for stream in [false, true] {
        for order in ORDERS {
            let p = MixedProblem::new(
                FiniteProblem::new(vec![Variable::new(vec![1usize, 2])]).unwrap(),
                ListProblem::new(vec!['X'], vec![ListEntity::new(vec![]); 2]).unwrap(),
            );
            let basic = SortedBasicConstruction::new(
                p.basic(),
                ConstructionPlacement::Sequential,
                OptionalConstruction::Preserve,
                |a, b| a.cmp(&b),
                |a, b| a.cmp(b),
            );
            let lists = SortedListConstruction::new(
                p.lists(),
                ConstructionPlacement::ValueQueue,
                OptionalConstruction::Preserve,
                |a, b| b.cmp(&a),
                |a, b| a.cmp(b),
            );
            let mut provider = MixedConstructionProvider::new(&p, basic, lists, order);
            let phase = MixedSolverSequence {
                phases: vec![MixedSolverPhase::Construction(MixedConstructionConfig {
                    construction: ConstructionConfig {
                        assert_scores: true,
                        ..Default::default()
                    },
                    order,
                })],
                ..Default::default()
            };
            let r = if stream {
                let mut ext = SolverExtensions::default();
                ext.move_providers.bind(0, &mut provider);
                solve_mixed_stream_with_extensions(
                    &p,
                    || {
                        let mut b = StreamGraphBuilder::new();
                        let sources = MixedStreamSources::new(&mut b);
                        let joined = b.join_equal(
                            sources.basic.port(),
                            sources.lists.entities.port(),
                            |_| 0,
                            |_| 0,
                        )?;
                        b.score("match", &joined, |((_, v), (owner, list))| {
                            SimpleScore::new(if *v == Some(owner + 1) && !list.is_empty() {
                                10
                            } else {
                                0
                            })
                        })?;
                        Ok((b.build(), sources))
                    },
                    phase,
                    ext,
                )
                .unwrap()
            } else {
                let mut ext = SolverExtensions::default();
                ext.move_providers.bind(0, &mut provider);
                let mut n = MixedConstraintNetwork::new(1, 2, 1);
                for owner in 0..2 {
                    n.add_constraint(
                        format!("match{owner}"),
                        vec![MixedScope::Basic(0), MixedScope::ListEntity(owner)],
                        move |facts| {
                            let MixedFact::Basic(v) = facts[0] else {
                                panic!()
                            };
                            let MixedFact::ListEntity(list) = &facts[1] else {
                                panic!()
                            };
                            SimpleScore::new(if v.copied() == Some(owner + 1) && !list.is_empty() {
                                10
                            } else {
                                0
                            })
                        },
                    )
                    .unwrap();
                }
                solve_mixed_with_extensions(&p, &n, phase, ext).unwrap()
            };
            let selected = if order == MixedConstructionOrder::BasicThenLists {
                1
            } else {
                2
            };
            assert_eq!(r.assignment.basic().values(), &[Some(selected)]);
            assert_eq!(r.assignment.lists().list(selected - 1).unwrap(), &['X']);
            assert_eq!(r.score, Some(SimpleScore::new(10)));
            assert_eq!(provider.component_steps(), [1, 1]);
        }
    }
}
#[test]
fn dynamic_child_counters_survive_partial_solves_and_reset_together() {
    for stream in [false, true] {
        for order in ORDERS {
            let p = MixedProblem::new(
                FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 3]).unwrap(),
                ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![]); 3]).unwrap(),
            );
            let basic_calls = Cell::new(0);
            let list_calls = Cell::new(0);
            let basic = SortedBasicConstruction::new(
                p.basic(),
                ConstructionPlacement::Sequential,
                OptionalConstruction::Preserve,
                same,
                same_value,
            )
            .with_step_sorting(|_: &FiniteProblem<i32>, a: &Assignment<i32>| {
                let count = a.values().iter().flatten().count();
                basic_calls.set(basic_calls.get() + 1);
                let preferred = 2 - count as i32;
                (same, move |a: &i32, b: &i32| {
                    (a != &preferred).cmp(&(b != &preferred))
                })
            });
            let lists = SortedListConstruction::new(
                p.lists(),
                ConstructionPlacement::ValueQueue,
                OptionalConstruction::Preserve,
                same,
                same_value,
            )
            .with_step_sorting(|_: &ListProblem<i32>, a: &ListAssignment<i32>| {
                let count = a.lists().iter().map(Vec::len).sum::<usize>();
                list_calls.set(list_calls.get() + 1);
                let owner = [2, 0, 1][count];
                (
                    move |a: usize, b: usize| (a != owner).cmp(&(b != owner)),
                    same_value,
                )
            });
            let mut provider = MixedConstructionProvider::new(&p, basic, lists, order);
            let cfg = ProviderConstructionConfig {
                assert_scores: true,
                ..Default::default()
            };
            // Atomic interruption opens only the first child and retries its cached step.
            let discarded = run(
                &p,
                p.initial_assignment(),
                stream,
                ProviderConstructionConfig {
                    max_move_evaluations: Some(1),
                    interruption: ConstructionInterruption::DiscardIncompleteStep,
                    ..cfg
                },
                &mut provider,
            );
            assert_eq!(discarded.assignment, p.initial_assignment());
            assert_eq!(provider.component_steps(), [0, 0]);
            assert_eq!(basic_calls.get() + list_calls.get(), 1);
            let first = run(
                &p,
                discarded.assignment,
                stream,
                ProviderConstructionConfig {
                    max_steps: Some(2),
                    ..cfg
                },
                &mut provider,
            );
            assert_eq!(
                provider.component_steps(),
                if order == MixedConstructionOrder::BasicThenLists {
                    [2, 0]
                } else {
                    [0, 2]
                }
            );
            // The outer solve's counter restarts at zero; component counters do not.
            let full = run(&p, first.assignment, stream, cfg, &mut provider);
            assert!(full.complete);
            assert_eq!(
                full.assignment.basic().values(),
                &[Some(2), Some(1), Some(0)]
            );
            assert_eq!(
                full.assignment.lists().lists(),
                &[vec![1], vec![2], vec![0]]
            );
            assert_eq!(provider.component_steps(), [3, 3]);
            assert_eq!((basic_calls.get(), list_calls.get()), (3, 3));
            provider.reset_with(|b, l| {
                b.reset();
                l.reset();
            });
            let fresh = run(&p, p.initial_assignment(), stream, cfg, &mut provider);
            assert_eq!(fresh.assignment, full.assignment);
            assert_eq!((basic_calls.get(), list_calls.get()), (6, 6));
        }
    }
}
#[test]
fn optional_rejection_of_one_component_does_not_stop_the_other() {
    for stream in [false, true] {
        for order in ORDERS {
            for optional in [
                OptionalConstruction::NonWorsening,
                OptionalConstruction::EvaluateUnassigned,
            ] {
                let p = MixedProblem::new(
                    FiniteProblem::new(vec![Variable::new(vec![-1]).allowing_unassigned()])
                        .unwrap(),
                    ListProblem::new(vec![10], vec![ListEntity::new(vec![])]).unwrap(),
                );
                let basic = SortedBasicConstruction::new(
                    p.basic(),
                    ConstructionPlacement::Pooled,
                    optional,
                    same,
                    same_value,
                );
                let lists = SortedListConstruction::new(
                    p.lists(),
                    ConstructionPlacement::ValueQueue,
                    optional,
                    same,
                    same_value,
                );
                let mut provider = MixedConstructionProvider::new(&p, basic, lists, order);
                let cfg = ProviderConstructionConfig {
                    optional,
                    assert_scores: true,
                    ..Default::default()
                };
                let r = if stream {
                    let mut b = StreamGraphBuilder::new();
                    let sources = MixedStreamSources::new(&mut b);
                    b.score(
                        "basic",
                        sources.basic.port(),
                        |(_, v): &(usize, Option<i32>)| SimpleScore::new(i64::from(v.unwrap_or(0))),
                    )
                    .unwrap();
                    mixed_stream_construction_from_with_provider(
                        &p,
                        b.build(),
                        sources,
                        p.initial_assignment(),
                        cfg,
                        &mut provider,
                    )
                    .unwrap()
                } else {
                    let mut n = MixedConstraintNetwork::new(1, 1, 1);
                    n.add_constraint("basic", vec![MixedScope::Basic(0)], |facts| {
                        let MixedFact::Basic(v) = facts[0] else {
                            panic!()
                        };
                        SimpleScore::new(i64::from(v.copied().unwrap_or(0)))
                    })
                    .unwrap();
                    mixed_construction_from_with_provider(
                        &p,
                        &n,
                        p.initial_assignment(),
                        cfg,
                        &mut provider,
                    )
                    .unwrap()
                };
                assert!(r.complete);
                assert_eq!(r.assignment.basic().values(), &[None]);
                assert_eq!(r.assignment.lists().lists(), &[vec![10]]);
                assert_eq!(provider.component_steps(), [1, 1]);
                assert_eq!(r.score, Some(SimpleScore::ZERO));
            }
        }
    }
}
struct CountingBasic<'c>(&'c Cell<usize>);
impl MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>> for CountingBasic<'_> {
    fn moves<'a>(
        &'a mut self,
        p: &'a FiniteProblem<i32>,
        _: &'a Assignment<i32>,
        _: u64,
    ) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
    where
        Move<i32>: 'a,
    {
        let calls = self.0;
        Box::new(p.variables()[0].domain().iter().map(move |v| {
            calls.set(calls.get() + 1);
            Move::Change {
                variable: 0,
                value: Some(*v),
            }
        }))
    }
}
#[test]
fn lazy_prefetch_zero_budget_and_foreign_problem_rejection() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new((0..1000).collect())]).unwrap(),
        ListProblem::<i32>::new(vec![], vec![]).unwrap(),
    );
    let calls = Cell::new(0);
    let lists = |_: &ListProblem<i32>, _: &ListAssignment<i32>, _: u64| -> Vec<ListMove<i32>> {
        panic!("fully assigned component must stay unopened")
    };
    let mut provider = MixedConstructionProvider::new(
        &p,
        CountingBasic(&calls),
        lists,
        MixedConstructionOrder::ListsThenBasic,
    );
    let a = p.initial_assignment();
    run(
        &p,
        a.clone(),
        false,
        ProviderConstructionConfig {
            max_evaluations: 0,
            ..Default::default()
        },
        &mut provider,
    );
    assert_eq!(calls.get(), 0);
    {
        let mut cursor = provider.moves(&p, &a, 0);
        assert_eq!(calls.get(), 1);
        assert_eq!(
            cursor.next(),
            Some(MixedMove {
                basic: Move::Change {
                    variable: 0,
                    value: Some(0)
                },
                lists: ListMove::Composite(vec![])
            })
        );
        assert_eq!(calls.get(), 1);
        cursor.next();
        assert_eq!(calls.get(), 2);
    }
    assert_eq!(provider.component_steps(), [0, 0]);
    assert!(provider.moves(&p, &a, 0).next().is_some());
    assert_eq!(calls.get(), 3);
    let foreign = p.clone();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| provider
            .moves(&foreign, &a, 0)
            .next()))
        .is_err()
    );
    assert_eq!(calls.get(), 3);
}
#[test]
fn complete_basic_and_list_pins_ranges_and_prefixes_remain_enforced() {
    for stream in [false, true] {
        for order in ORDERS {
            let p = MixedProblem::new(
                FiniteProblem::new(vec![
                    Variable::new(vec![1, 2]).with_initial(2).with_pinned(true),
                ])
                .unwrap(),
                ListProblem::from_entity_ranges(vec![
                    ListEntity::new(vec![10])
                        .with_pinned_prefix(1)
                        .with_value_range(vec![10]),
                    ListEntity::new(vec![]).with_value_range(vec![20]),
                ])
                .unwrap(),
            );
            let basic = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| -> Vec<Move<i32>> {
                panic!("already initialized basic component")
            };
            let lists = SortedListConstruction::new(
                p.lists(),
                ConstructionPlacement::ValueQueue,
                OptionalConstruction::Preserve,
                same,
                same_value,
            );
            let mut provider = MixedConstructionProvider::new(&p, basic, lists, order);
            let r = run(
                &p,
                p.initial_assignment(),
                stream,
                ProviderConstructionConfig {
                    assert_scores: true,
                    ..Default::default()
                },
                &mut provider,
            );
            assert!(r.complete);
            assert_eq!(r.assignment.basic().values(), &[Some(2)]);
            assert_eq!(r.assignment.lists().lists(), &[vec![10], vec![20]]);
            assert_eq!(provider.component_steps(), [0, 1]);
        }
    }
}
