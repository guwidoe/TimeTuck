//! Original solver integration tests; no upstream method credit.
use SelectionLifecycleEvent as E;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use timetuck::*;

#[derive(Default)]
struct Keys {
    values: Vec<usize>,
    events: Vec<(E, usize)>,
    captures: Vec<usize>,
}
impl Keys {
    fn new(values: Vec<usize>) -> Self {
        Self {
            values,
            ..Default::default()
        }
    }
}
impl SelectionSource<usize, usize> for Keys {
    fn event(&mut self, e: E, c: &usize) {
        self.events.push((e, *c));
    }
    fn size(&mut self, _: &usize) -> Option<usize> {
        Some(self.values.len())
    }
    fn open<'a>(&'a mut self, c: &'a usize) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        self.captures.push(*c);
        Box::new(self.values.iter().copied())
    }
}
struct Pool {
    values: Vec<i32>,
    events: Vec<(E, usize)>,
    opens: Vec<(usize, usize)>,
    declared: Option<usize>,
}
impl Pool {
    fn new(values: Vec<i32>) -> Self {
        Self {
            values,
            events: vec![],
            opens: vec![],
            declared: None,
        }
    }
}
impl PlacementSource<usize, usize, i32> for Pool {
    fn event(&mut self, e: E, c: &usize) {
        self.events.push((e, *c));
    }
    fn size(&mut self, _: &usize, _: &usize) -> Option<usize> {
        self.declared.or(Some(self.values.len()))
    }
    fn open<'a>(&'a mut self, c: &'a usize, key: &'a usize) -> Box<dyn Iterator<Item = i32> + 'a>
    where
        i32: 'a,
    {
        self.opens.push((*c, *key));
        Box::new(self.values.iter().copied())
    }
}

#[test]
fn shared_phases_rebuild_keys_and_generate_from_current_assignment() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 4]).unwrap();
    let mut n = ConstraintNetwork::new(4);
    n.add_constraint("sum", (0..4).collect(), |v| {
        SimpleScore::new(v.iter().filter_map(|v| v.copied()).map(i64::from).sum())
    })
    .unwrap();
    let calls = RefCell::new(vec![]);
    let queue = PlacementQueue::new(
        Keys::new(vec![1, 0]),
        vec![Pool::new(vec![1, 2]), Pool::new(vec![1, 2])],
        PlacementOrder::Once,
        2,
        2,
    )
    .unwrap();
    let mut provider = PlacementQueueProvider::new(
        queue,
        |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| a.values().iter().flatten().count(),
        |_: &FiniteProblem<i32>,
         a: &Assignment<i32>,
         step: u64,
         &key: &usize,
         lane: usize,
         &value: &i32| {
            calls
                .borrow_mut()
                .push((step, key, lane, a.values().to_vec()));
            let variable = key * 2 + lane;
            a.values()[variable].is_none().then_some(Move::Change {
                variable,
                value: Some(value),
            })
        },
    );
    for run in 1..=2 {
        let mut ext = SolverExtensions::default();
        ext.move_providers
            .bind_shared([0, 1], &mut provider)
            .unwrap();
        let r = solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: [Some(2), None]
                    .into_iter()
                    .map(|max_steps| {
                        SolverPhase::Construction(ConstructionConfig {
                            max_steps,
                            assert_scores: true,
                            ..Default::default()
                        })
                    })
                    .collect(),
                limits: SolverLimits {
                    assert_phase_scores: true,
                    ..Default::default()
                },
            },
            ext,
        )
        .unwrap();
        assert_eq!(r.assignment.values(), &[Some(2); 4]);
        assert_eq!(r.work.construction_steps, 4);
        assert_eq!(provider.queue().state(), SelectionCacheState::Idle);
        assert_eq!(provider.queue().key_source().captures, [0, 2].repeat(run));
        let expected = vec![
            (E::SolveStarted, 0),
            (E::PhaseStarted, 0),
            (E::StepStarted, 0),
            (E::StepEnded, 0),
            (E::StepStarted, 1),
            (E::StepEnded, 1),
            (E::PhaseEnded, 1),
            (E::PhaseStarted, 2),
            (E::StepStarted, 2),
            (E::StepEnded, 2),
            (E::StepStarted, 3),
            (E::StepEnded, 3),
            (E::PhaseEnded, 3),
            (E::SolveEnded, 3),
        ];
        assert_eq!(provider.queue().key_source().events, expected.repeat(run));
        for child in provider.queue().children() {
            assert_eq!(child.events, expected.repeat(run));
        }
    }
    assert!(
        calls
            .borrow()
            .iter()
            .any(|(_, k, lane, a)| *k == 0 && *lane == 1 && a[0] == Some(2))
    );
}

#[test]
fn optional_rejections_empty_pools_and_installations_bound_cycles() {
    for order in [PlacementOrder::Once, PlacementOrder::Cycle] {
        for values in [vec![-1, 0, -1], vec![-1, 0, 2]] {
            let p =
                FiniteProblem::new(vec![Variable::new(vec![-1, 2]).allowing_unassigned()]).unwrap();
            let mut n = ConstraintNetwork::new(1);
            n.add_constraint("value", vec![0], |v| {
                SimpleScore::new(i64::from(v[0].copied().unwrap_or(0)))
            })
            .unwrap();
            let opens = Cell::new(0);
            let queue = PlacementQueue::new(
                Keys::new(vec![0, 1, 2]),
                vec![PlacementFn(|_: &usize, key: &usize| {
                    opens.set(opens.get() + 1);
                    if values[*key] == 0 {
                        vec![]
                    } else {
                        vec![values[*key]]
                    }
                })],
                order,
                3,
                1,
            )
            .unwrap();
            let mut provider = PlacementQueueProvider::new(
                queue,
                |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| 0,
                |_: &FiniteProblem<i32>,
                 a: &Assignment<i32>,
                 _: u64,
                 _: &usize,
                 _: usize,
                 &value: &i32| {
                    a.values()[0].is_none().then_some(Move::Change {
                        variable: 0,
                        value: Some(value),
                    })
                },
            );
            let r = construction_from_with_provider(
                &p,
                &n,
                p.initial_assignment(),
                ProviderConstructionConfig {
                    optional: OptionalConstruction::NonWorsening,
                    ..Default::default()
                },
                &mut provider,
            )
            .unwrap();
            assert_eq!(
                r.assignment.values(),
                &[if values[2] == 2 { Some(2) } else { None }]
            );
            assert_eq!(r.completed_steps, 2);
            assert_eq!(opens.get(), 3);
            assert_eq!(provider.queue().state(), SelectionCacheState::Idle);
        }
    }
}

#[test]
fn repeated_openings_replay_one_pool_and_all_mapper_rejections_stop() {
    let queue = PlacementQueue::new(
        Keys::new(vec![0, 1, 2]),
        vec![Pool::new(vec![1, 2])],
        PlacementOrder::Cycle,
        3,
        2,
    )
    .unwrap();
    let seen = Cell::new(0);
    let mut provider = PlacementQueueProvider::new(
        queue,
        |_: &(), _: &(), _: u64| 0,
        |_: &(), _: &(), _: u64, key: &usize, _: usize, value: &i32| {
            seen.set(seen.get() + 1);
            (*key == 1).then_some(*value)
        },
    );
    assert!(matches!(
        provider.try_moves(&(), &(), 0),
        Err(PlacementProviderError::OutsideStep)
    ));
    MoveProvider::<(), (), i32>::event(&mut provider, MoveProviderEvent::PhaseStarted);
    MoveProvider::<(), (), i32>::event(&mut provider, MoveProviderEvent::StepStarted);
    assert_eq!(provider.try_moves(&(), &(), 0).unwrap().next(), Some(1));
    assert_eq!(
        provider.try_moves(&(), &(), 0).unwrap().collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(provider.queue().children()[0].opens.len(), 2);
    assert_eq!(seen.get(), 5); // two rejected key0 tokens, prefix then full replay
    assert!(MoveProvider::<(), (), i32>::construction_placement_completed(&mut provider, false));
    MoveProvider::<(), (), i32>::event(&mut provider, MoveProviderEvent::StepEnded);
    MoveProvider::<(), (), i32>::event(&mut provider, MoveProviderEvent::StepStarted);
    assert_eq!(provider.try_moves(&(), &(), 0).unwrap().count(), 0);
    assert_eq!(provider.try_moves(&(), &(), 0).unwrap().count(), 0);
    assert_eq!(provider.queue().children()[0].opens.len(), 3);
    MoveProvider::<(), (), i32>::event(&mut provider, MoveProviderEvent::StepEnded);
    MoveProvider::<(), (), i32>::event(&mut provider, MoveProviderEvent::PhaseEnded);
}

#[test]
fn cyclic_installation_revisits_a_previously_unavailable_key() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1]); 2]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let queue = PlacementQueue::new(
        Keys::new(vec![0, 1]),
        vec![Pool::new(vec![1])],
        PlacementOrder::Cycle,
        2,
        1,
    )
    .unwrap();
    let mut provider = PlacementQueueProvider::new(
        queue,
        |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| a.values().iter().flatten().count(),
        |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64, &key: &usize, _: usize, &v: &i32| {
            (a.values()[key].is_none() && (key == 1 || a.values()[1].is_some())).then_some(
                Move::Change {
                    variable: key,
                    value: Some(v),
                },
            )
        },
    );
    let r = construction_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        ProviderConstructionConfig::default(),
        &mut provider,
    )
    .unwrap();
    assert_eq!(r.assignment.values(), &[Some(1), Some(1)]);
    assert_eq!(r.completed_steps, 2);
    assert_eq!(
        provider.queue().children()[0].opens,
        [(0, 0), (0, 1), (1, 0)]
    );
}

#[test]
fn malformed_sources_are_errors_and_reset_recovers_after_mapper_panic() {
    let mut pool = Pool::new(vec![1]);
    pool.declared = Some(2);
    let queue =
        PlacementQueue::new(Keys::new(vec![0]), vec![pool], PlacementOrder::Once, 1, 2).unwrap();
    let panic_now = Cell::new(false);
    let mut provider = PlacementQueueProvider::new(
        queue,
        |_: &(), _: &(), _: u64| 0,
        |_: &(), _: &(), _: u64, _: &usize, _: usize, v: &i32| {
            assert!(!panic_now.get(), "mapper");
            Some(*v)
        },
    );
    for panic_case in [false, true, false] {
        MoveProvider::<(), (), i32>::event(&mut provider, MoveProviderEvent::PhaseStarted);
        MoveProvider::<(), (), i32>::event(&mut provider, MoveProviderEvent::StepStarted);
        panic_now.set(panic_case);
        if provider.queue().children()[0].declared.is_some() {
            assert!(matches!(
                provider.try_moves(&(), &(), 0),
                Err(PlacementProviderError::Queue(
                    PlacementError::SizeMismatch { .. }
                ))
            ));
            provider.children_mut()[0].declared = None;
        } else if panic_case {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    drop(provider.try_moves(&(), &(), 0));
                }))
                .is_err()
            );
        } else {
            assert_eq!(
                provider.try_moves(&(), &(), 0).unwrap().collect::<Vec<_>>(),
                [1]
            );
        }
        provider.reset().unwrap();
        assert_eq!(provider.queue().state(), SelectionCacheState::Idle);
    }
}

#[test]
fn unentered_scans_are_lazy_and_nonclone_tokens_release_at_phase_end() {
    struct Token(Rc<Cell<usize>>);
    impl Drop for Token {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = Rc::new(Cell::new(0));
    let queue = PlacementQueue::new(
        Keys::new(vec![0]),
        vec![PlacementFn(|_: &usize, _: &usize| {
            vec![Token(drops.clone()), Token(drops.clone())]
        })],
        PlacementOrder::Once,
        1,
        2,
    )
    .unwrap();
    let mut provider = PlacementQueueProvider::new(
        queue,
        |_: &(), _: &(), _: u64| 0,
        |_: &(), _: &(), _: u64, _: &usize, _: usize, _: &Token| Some(1),
    );
    MoveProvider::<(), (), i32>::event(&mut provider, MoveProviderEvent::SolveStarted);
    MoveProvider::<(), (), i32>::event(&mut provider, MoveProviderEvent::PhaseStarted);
    MoveProvider::<(), (), i32>::event(&mut provider, MoveProviderEvent::PhaseEnded);
    assert!(provider.queue().key_source().events.is_empty());
    MoveProvider::<(), (), i32>::event(&mut provider, MoveProviderEvent::PhaseStarted);
    MoveProvider::<(), (), i32>::event(&mut provider, MoveProviderEvent::StepStarted);
    assert_eq!(provider.try_moves(&(), &(), 0).unwrap().next(), Some(1));
    assert_eq!(drops.get(), 0);
    MoveProvider::<(), (), i32>::event(&mut provider, MoveProviderEvent::StepEnded);
    MoveProvider::<(), (), i32>::event(&mut provider, MoveProviderEvent::PhaseEnded);
    assert_eq!(drops.get(), 2);
    MoveProvider::<(), (), i32>::event(&mut provider, MoveProviderEvent::SolveEnded);
    assert_eq!(provider.queue().state(), SelectionCacheState::Idle);
}

#[test]
fn list_and_mixed_construction_share_the_same_queue_adapter() {
    let l = ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![])]).unwrap();
    let ln = ListConstraintNetwork::<_, SimpleScore>::new(1, 2);
    let queue = PlacementQueue::new(
        Keys::new(vec![2, 1]),
        vec![Pool::new(vec![0])],
        PlacementOrder::Once,
        2,
        1,
    )
    .unwrap();
    let mut provider = PlacementQueueProvider::new(
        queue,
        |_: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| a.lists()[0].len(),
        |_: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64, key: &usize, _: usize, _: &i32| {
            Some(ListMove::Assign {
                value: *key as i32,
                destination: ListPosition {
                    entity: 0,
                    index: a.lists()[0].len(),
                },
            })
        },
    );
    let r = list_construction_from_with_provider(
        &l,
        &ln,
        l.initial_assignment(),
        ProviderConstructionConfig::default(),
        &mut provider,
    )
    .unwrap();
    assert_eq!(r.assignment.lists(), &[vec![2, 1]]);
    let m = MixedProblem::new(FiniteProblem::new(vec![Variable::new(vec![3])]).unwrap(), l);
    let mn = MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 1, 2);
    let queue = PlacementQueue::new(
        Keys::new(vec![0, 2, 1]),
        vec![Pool::new(vec![0])],
        PlacementOrder::Once,
        3,
        1,
    )
    .unwrap();
    let mut provider = PlacementQueueProvider::new(
        queue,
        |_: &MixedProblem<i32, i32>, a: &MixedAssignment<i32, i32>, _: u64| {
            a.lists().lists()[0].len()
        },
        |_: &MixedProblem<i32, i32>,
         a: &MixedAssignment<i32, i32>,
         _: u64,
         key: &usize,
         _: usize,
         _: &i32| {
            Some(if *key == 0 {
                MixedMove {
                    basic: Move::Change {
                        variable: 0,
                        value: Some(3),
                    },
                    lists: ListMove::Composite(vec![]),
                }
            } else {
                MixedMove {
                    basic: Move::Composite(vec![]),
                    lists: ListMove::Assign {
                        value: *key as i32,
                        destination: ListPosition {
                            entity: 0,
                            index: a.lists().lists()[0].len(),
                        },
                    },
                }
            })
        },
    );
    let r = mixed_construction_from_with_provider(
        &m,
        &mn,
        m.initial_assignment(),
        ProviderConstructionConfig::default(),
        &mut provider,
    )
    .unwrap();
    assert_eq!(r.assignment.basic().values(), &[Some(3)]);
    assert_eq!(r.assignment.lists().lists(), &[vec![2, 1]]);
}
