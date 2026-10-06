//! Original queue/provider integration; no upstream method credit.
use std::{cell::Cell, rc::Rc};
use timetuck::*;

#[test]
fn queues_preserve_occurrences_retry_steps_and_bound_empty_rotations() {
    struct Key(usize); // Deliberately not Clone.
    for order in [PlacementQueueOrder::Once, PlacementQueueOrder::Cycle] {
        let calls = Cell::new(0);
        let mut q = QueuedMoveProvider::new(
            vec![Key(2), Key(0), Key(2), Key(1)],
            order,
            |_: &(), enabled: &bool, key: &Key| {
                calls.set(calls.get() + 1);
                if *enabled && key.0 != 0 {
                    vec![key.0]
                } else {
                    vec![]
                }
            },
        );
        assert_eq!(q.moves(&(), &true, 0).collect::<Vec<_>>(), vec![2]);
        assert_eq!(q.moves(&(), &true, 0).collect::<Vec<_>>(), vec![2]);
        assert_eq!(q.moves(&(), &true, 1).collect::<Vec<_>>(), vec![2]);
        assert_eq!(q.moves(&(), &true, 2).collect::<Vec<_>>(), vec![1]);
        assert_eq!(
            q.moves(&(), &true, 3).collect::<Vec<_>>(),
            if order == PlacementQueueOrder::Once {
                vec![]
            } else {
                vec![2]
            }
        );
        // A backwards counter begins at the first key again.
        assert_eq!(q.moves(&(), &true, 0).collect::<Vec<_>>(), vec![2]);
        q.reset();
        let before = calls.get();
        assert!(q.moves(&(), &false, 0).next().is_none());
        assert_eq!(calls.get() - before, 4);
        q.reset();
        assert_eq!(q.current_key().unwrap().0, 2);
        assert_eq!(q.moves(&(), &true, u64::MAX).collect::<Vec<_>>(), vec![2]);
        assert_eq!(q.moves(&(), &true, u64::MAX).collect::<Vec<_>>(), vec![2]);
    }
    let mut empty = QueuedMoveProvider::new(
        vec![],
        PlacementQueueOrder::Cycle,
        |_: &(), _: &(), _: &usize| -> Vec<usize> { panic!("empty queue") },
    );
    assert!(empty.moves(&(), &(), 0).next().is_none());
}
struct LazyFactory(Rc<Cell<usize>>);
impl PlacementMoveFactory<Vec<i32>, (), usize, i32> for LazyFactory {
    fn candidates<'a>(
        &mut self,
        p: &'a Vec<i32>,
        _: &'a (),
        &start: &usize,
    ) -> Box<dyn Iterator<Item = i32> + 'a>
    where
        i32: 'a,
    {
        let calls = self.0.clone();
        Box::new(
            p[start..]
                .iter()
                .copied()
                .inspect(move |_| calls.set(calls.get() + 1)),
        )
    }
}
#[test]
fn borrowing_factories_prefetch_only_one_candidate() {
    let calls = Rc::new(Cell::new(0));
    let mut q = QueuedMoveProvider::new(
        vec![0],
        PlacementQueueOrder::Once,
        LazyFactory(calls.clone()),
    );
    let p = (0..1_000_000).collect();
    {
        let mut moves = q.moves(&p, &(), 0);
        assert_eq!(calls.get(), 1);
        assert_eq!(moves.next(), Some(0));
        assert_eq!(calls.get(), 1);
        assert_eq!(moves.next(), Some(1));
        assert_eq!(calls.get(), 2);
    }
    assert_eq!(calls.get(), 2);
    assert_eq!(q.moves(&p, &(), 0).next(), Some(0));
    assert_eq!(calls.get(), 3);
}
fn basic_run<F: MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>>>(
    p: &FiniteProblem<i32>,
    a: Assignment<i32>,
    stream: bool,
    c: ProviderConstructionConfig,
    q: &mut F,
) -> ProviderConstructionResult<Assignment<i32>, SimpleScore> {
    if stream {
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, (usize, Option<i32>)>();
        b.score("sum", s.port(), |(_, v)| {
            SimpleScore::new(i64::from(v.unwrap_or(0)))
        })
        .unwrap();
        stream_construction_from_with_provider(p, b.build(), s, a, c, q).unwrap()
    } else {
        let mut n = ConstraintNetwork::new(p.variables().len());
        for i in 0..p.variables().len() {
            n.add_constraint(format!("v{i}"), vec![i], |v| {
                SimpleScore::new(i64::from(v[0].copied().unwrap_or(0)))
            })
            .unwrap();
        }
        construction_from_with_provider(p, &n, a, c, q).unwrap()
    }
}
#[test]
fn queued_construction_resumes_a_discarded_step_and_filters_current_assignments() {
    for stream in [false, true] {
        let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2]).unwrap();
        let calls = Cell::new(0);
        let mut q = QueuedMoveProvider::new(
            vec![1, 0],
            PlacementQueueOrder::Once,
            |p: &FiniteProblem<i32>, a: &Assignment<i32>, &key: &usize| {
                calls.set(calls.get() + 1);
                if a.values()[key].is_some() {
                    return vec![];
                }
                p.variables()[key]
                    .domain()
                    .iter()
                    .map(|v| Move::Change {
                        variable: key,
                        value: Some(*v),
                    })
                    .collect()
            },
        );
        let config = ProviderConstructionConfig {
            interruption: ConstructionInterruption::DiscardIncompleteStep,
            max_move_evaluations: Some(1),
            assert_scores: true,
            ..Default::default()
        };
        let r = basic_run(&p, p.initial_assignment(), stream, config, &mut q);
        assert_eq!(r.assignment.values(), &[None, None]);
        assert_eq!(r.completed_steps, 0);
        assert_eq!(q.current_key(), Some(&1));
        let r = basic_run(
            &p,
            r.assignment,
            stream,
            ProviderConstructionConfig {
                max_move_evaluations: None,
                ..config
            },
            &mut q,
        );
        assert_eq!(r.assignment.values(), &[Some(2), Some(2)]);
        assert_eq!(r.completed_steps, 2);
        assert_eq!(calls.get(), 3);
        // Reuse after a new solve requires reset; no source or candidate caches leak.
        q.reset();
        let r = basic_run(
            &p,
            p.initial_assignment(),
            stream,
            ProviderConstructionConfig {
                max_move_evaluations: None,
                max_steps: Some(1),
                ..config
            },
            &mut q,
        );
        assert_eq!(r.assignment.values(), &[None, Some(2)]);
    }
}
#[test]
fn cycling_values_fill_more_receivers_than_keys_and_zero_budget_never_opens_queue() {
    for stream in [false, true] {
        let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 3]).unwrap();
        let calls = Cell::new(0);
        let mut q = QueuedMoveProvider::new(
            vec![1, 2],
            PlacementQueueOrder::Cycle,
            |_: &FiniteProblem<i32>, a: &Assignment<i32>, v: &i32| {
                calls.set(calls.get() + 1);
                a.values()
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v.is_none())
                    .map(|(i, _)| Move::Change {
                        variable: i,
                        value: Some(*v),
                    })
                    .collect()
            },
        );
        for c in [
            ProviderConstructionConfig {
                max_evaluations: 0,
                ..Default::default()
            },
            ProviderConstructionConfig {
                max_move_evaluations: Some(0),
                ..Default::default()
            },
        ] {
            basic_run(&p, p.initial_assignment(), stream, c, &mut q);
            assert_eq!(calls.get(), 0);
        }
        let r = basic_run(
            &p,
            p.initial_assignment(),
            stream,
            ProviderConstructionConfig::default(),
            &mut q,
        );
        assert_eq!(r.assignment.values(), &[Some(1), Some(2), Some(1)]);
        assert_eq!(r.completed_steps, 3);
        assert_eq!(calls.get(), 3);
    }
}

#[test]
fn queued_groups_initialize_mixed_components_as_atomic_candidates() {
    for stream in [false, true] {
        let p = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2]).unwrap(),
            ListProblem::new(vec![10, 20], vec![ListEntity::new(vec![]); 2]).unwrap(),
        );
        let mut q = QueuedMoveProvider::new(
            vec![(1, 20), (0, 10)],
            PlacementQueueOrder::Once,
            |_: &MixedProblem<i32, i32>,
             a: &MixedAssignment<i32, i32>,
             &(variable, value): &(usize, i32)| {
                if a.basic().values()[variable].is_some() {
                    return vec![];
                }
                (1..=2)
                    .map(|v| MixedMove {
                        basic: Move::Change {
                            variable,
                            value: Some(v),
                        },
                        lists: ListMove::Assign {
                            value,
                            destination: ListPosition {
                                entity: variable,
                                index: 0,
                            },
                        },
                    })
                    .collect()
            },
        );
        let c = ProviderConstructionConfig {
            assert_scores: true,
            ..Default::default()
        };
        let r = if stream {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let s = MixedStreamSources::new(&mut b);
            b.score("basic", s.basic.port(), |(_, v): &(usize, Option<i32>)| {
                SimpleScore::new(i64::from(v.unwrap_or(0)))
            })
            .unwrap();
            mixed_stream_construction_from_with_provider(
                &p,
                b.build(),
                s,
                p.initial_assignment(),
                c,
                &mut q,
            )
            .unwrap()
        } else {
            let mut n = MixedConstraintNetwork::new(2, 2, 2);
            for i in 0..2 {
                n.add_constraint(format!("v{i}"), vec![MixedScope::Basic(i)], |f| {
                    let MixedFact::Basic(v) = &f[0] else { panic!() };
                    SimpleScore::new(i64::from(v.copied().unwrap_or(0)))
                })
                .unwrap();
            }
            mixed_construction_from_with_provider(&p, &n, p.initial_assignment(), c, &mut q)
                .unwrap()
        };
        assert!(r.complete);
        assert_eq!(r.assignment.basic().values(), &[Some(2), Some(2)]);
        assert_eq!(r.assignment.lists().lists(), &[vec![10], vec![20]]);
        assert_eq!(r.completed_steps, 2);
        assert_eq!(r.score, Some(SimpleScore::new(4)));
    }
}

#[test]
fn rejected_optional_placements_advance_and_an_unproductive_cycle_stops() {
    for stream in [false, true] {
        for order in [PlacementQueueOrder::Once, PlacementQueueOrder::Cycle] {
            for optional in [
                OptionalConstruction::NonWorsening,
                OptionalConstruction::EvaluateUnassigned,
            ] {
                let p = FiniteProblem::new(vec![Variable::new(vec![-1, 2]).allowing_unassigned()])
                    .unwrap();
                let mut q = QueuedMoveProvider::new(
                    vec![-1, 2],
                    order,
                    |_: &FiniteProblem<i32>, a: &Assignment<i32>, v: &i32| {
                        if a.values()[0].is_some() {
                            vec![]
                        } else {
                            vec![Move::Change {
                                variable: 0,
                                value: Some(*v),
                            }]
                        }
                    },
                );
                let c = ProviderConstructionConfig {
                    optional,
                    assert_scores: true,
                    ..Default::default()
                };
                let r = basic_run(&p, p.initial_assignment(), stream, c, &mut q);
                assert_eq!(r.assignment.values(), &[Some(2)]);
                assert_eq!(r.completed_steps, 2);
                // An empty middle key contributes to rotation exhaustion, so the
                // rejected first value must not be rescored before termination.
                let calls = Cell::new(0);
                let mut q = QueuedMoveProvider::new(
                    vec![-1, 0, -1],
                    order,
                    |_: &FiniteProblem<i32>, _: &Assignment<i32>, v: &i32| {
                        calls.set(calls.get() + 1);
                        if *v == 0 {
                            vec![]
                        } else {
                            vec![Move::Change {
                                variable: 0,
                                value: Some(*v),
                            }]
                        }
                    },
                );
                let r = basic_run(&p, p.initial_assignment(), stream, c, &mut q);
                assert_eq!(r.assignment.values(), &[None]);
                assert_eq!(r.completed_steps, 2);
                assert_eq!(r.termination, ProviderConstructionTermination::NoCandidates);
                assert_eq!(calls.get(), 3);
            }
        }
    }
}

#[test]
fn completion_notifications_exclude_atomic_discard_but_include_rejection() {
    struct Notified<'n> {
        notices: &'n std::cell::RefCell<Vec<bool>>,
    }
    impl MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>> for Notified<'_> {
        fn construction_placement_completed(&mut self, installed: bool) -> bool {
            self.notices.borrow_mut().push(installed);
            false
        }
        fn moves<'a>(
            &'a mut self,
            p: &'a FiniteProblem<i32>,
            _: &'a Assignment<i32>,
            _: u64,
        ) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
        where
            Move<i32>: 'a,
        {
            Box::new(p.variables()[0].domain().iter().map(|v| Move::Change {
                variable: 0,
                value: Some(*v),
            }))
        }
    }
    for stream in [false, true] {
        for atomic in [false, true] {
            let p = FiniteProblem::new(vec![Variable::new(vec![1, 2])]).unwrap();
            let notices = std::cell::RefCell::new(vec![]);
            let mut q = Notified { notices: &notices };
            let c = ProviderConstructionConfig {
                max_move_evaluations: Some(1),
                interruption: if atomic {
                    ConstructionInterruption::DiscardIncompleteStep
                } else {
                    ConstructionInterruption::KeepBestPrefix
                },
                ..Default::default()
            };
            basic_run(&p, p.initial_assignment(), stream, c, &mut q);
            assert_eq!(*notices.borrow(), if atomic { vec![] } else { vec![true] });
        }
        let p =
            FiniteProblem::new(vec![Variable::new(vec![-1, -2]).allowing_unassigned()]).unwrap();
        let notices = std::cell::RefCell::new(vec![]);
        let mut q = Notified { notices: &notices };
        let r = basic_run(
            &p,
            p.initial_assignment(),
            stream,
            ProviderConstructionConfig {
                optional: OptionalConstruction::NonWorsening,
                ..Default::default()
            },
            &mut q,
        );
        assert_eq!(r.completed_steps, 1);
        assert_eq!(*notices.borrow(), vec![false]);
    }
}
