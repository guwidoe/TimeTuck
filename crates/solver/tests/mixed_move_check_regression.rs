//! Original joint candidate validation regressions; no upstream method credit.
use std::{cell::Cell, rc::Rc};
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn problem() -> MixedProblem<i32, i32> {
    MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![0, 1, 2]).with_initial(0),
            Variable::new(vec![1, 2]).with_initial(1).with_pinned(true),
        ])
        .unwrap(),
        ListProblem::new(
            vec![7, 8, 9],
            vec![
                ListEntity::new(vec![7, 8]).with_pinned_prefix(1),
                ListEntity::new(vec![9]),
                ListEntity::new(vec![]).with_value_range(vec![8]),
            ],
        )
        .unwrap(),
    )
}
fn compare(p: &MixedProblem<i32, i32>, a: &MixedAssignment<i32, i32>, m: &MixedMove<i32, i32>) {
    let mut copy = a.clone();
    let expected = p.apply(&mut copy, m).map(|tx| &*tx != a);
    assert_eq!(p.is_move_doable(a, m), expected, "{m:?}");
    assert_eq!(
        p.move_checker(a).and_then(|c| c.is_doable(m)),
        expected,
        "checker {m:?}"
    );
    assert_eq!(copy, *a);
}
#[test]
fn joint_proposals_match_transactions_including_restoration_and_partial_states() {
    let p = problem();
    let a = p.initial_assignment();
    let basics = vec![
        Move::Composite(vec![]),
        Move::Change {
            variable: 0,
            value: Some(0),
        },
        Move::Change {
            variable: 0,
            value: Some(2),
        },
        Move::Change {
            variable: 0,
            value: None,
        },
        Move::Change {
            variable: 0,
            value: Some(99),
        },
        Move::Change {
            variable: 1,
            value: Some(2),
        },
        Move::Change {
            variable: 9,
            value: Some(0),
        },
        Move::Swap { left: 0, right: 1 },
        Move::Composite(vec![
            Move::Change {
                variable: 1,
                value: Some(2),
            },
            Move::Change {
                variable: 1,
                value: Some(1),
            },
        ]),
        Move::Composite(vec![
            Move::Change {
                variable: 0,
                value: Some(2),
            },
            Move::Change {
                variable: 0,
                value: Some(0),
            },
        ]),
    ];
    let lists = vec![
        ListMove::Composite(vec![]),
        ListMove::Change {
            source: pos(0, 1),
            destination: pos(0, 1),
        },
        ListMove::Change {
            source: pos(0, 1),
            destination: pos(2, 0),
        },
        ListMove::Change {
            source: pos(0, 0),
            destination: pos(1, 1),
        },
        ListMove::Change {
            source: pos(1, 0),
            destination: pos(2, 0),
        },
        ListMove::Unassign {
            entity: 0,
            start: 1,
            end: 2,
        },
        ListMove::Assign {
            value: 8,
            destination: pos(2, 0),
        },
        ListMove::Swap {
            left: pos(0, 1),
            right: pos(1, 0),
        },
        ListMove::Swap {
            left: pos(9, 0),
            right: pos(0, 1),
        },
        ListMove::SubListChange {
            entity: 0,
            start: 0,
            end: 2,
            destination: pos(0, 0),
            reverse: true,
        },
        ListMove::SubListSwap {
            left: ListRange {
                entity: 0,
                start: 1,
                end: 2,
            },
            right: ListRange {
                entity: 1,
                start: 0,
                end: 1,
            },
            reverse: true,
        },
        ListMove::Composite(vec![
            ListMove::Change {
                source: pos(0, 0),
                destination: pos(1, 1),
            },
            ListMove::Change {
                source: pos(1, 1),
                destination: pos(0, 0),
            },
        ]),
    ];
    for partial in [false, true] {
        let a = if partial {
            MixedAssignment::new(
                Assignment::new(vec![None, Some(1)]),
                ListAssignment::new(vec![vec![7], vec![9], vec![]]),
            )
        } else {
            a.clone()
        };
        for basic in &basics {
            for lists in &lists {
                compare(
                    &p,
                    &a,
                    &MixedMove {
                        basic: basic.clone(),
                        lists: lists.clone(),
                    },
                );
            }
        }
    }
}
#[test]
fn all_sources_are_checked_before_moves_and_changed_basic_does_not_skip_list_errors() {
    let p = problem();
    let a = p.initial_assignment();
    let bad_basic = Move::Change {
        variable: 9,
        value: Some(0),
    };
    let bad_list = ListMove::Change {
        source: pos(9, 0),
        destination: pos(0, 0),
    };
    let bad = MixedMove {
        basic: bad_basic.clone(),
        lists: bad_list.clone(),
    };
    assert!(matches!(
        p.is_move_doable(&a, &bad),
        Err(MixedError::Basic(_))
    ));
    let changed = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(2),
        },
        lists: bad_list,
    };
    assert!(matches!(
        p.is_move_doable(&a, &changed),
        Err(MixedError::Lists(_))
    ));
    for basic in [a.basic().clone(), Assignment::new(vec![])] {
        let source = MixedAssignment::new(basic, ListAssignment::new(vec![]));
        let validation = p.validate(&source).unwrap_err();
        assert_eq!(p.is_move_doable(&source, &bad), Err(validation));
        compare(&p, &source, &bad);
    }
}
#[derive(Debug)]
struct Counted {
    value: i32,
    clones: Rc<Cell<usize>>,
}
impl Clone for Counted {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            value: self.value,
            clones: self.clones.clone(),
        }
    }
}
impl PartialEq for Counted {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for Counted {}
#[test]
fn repeated_joint_checks_clone_no_planning_values() {
    let clones = Rc::new(Cell::new(0));
    let v = |value| Counted {
        value,
        clones: clones.clone(),
    };
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![v(0), v(1)]).with_initial(v(0))]).unwrap(),
        ListProblem::new(
            vec![v(7), v(8)],
            vec![ListEntity::new(vec![v(7), v(8)]), ListEntity::new(vec![])],
        )
        .unwrap(),
    );
    let a = p.initial_assignment();
    let movement = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(v(1)),
        },
        lists: ListMove::Change {
            source: pos(0, 1),
            destination: pos(1, 0),
        },
    };
    clones.set(0);
    let checker = p.move_checker(&a).unwrap();
    for _ in 0..100 {
        assert_eq!(checker.is_doable(&movement), Ok(true));
    }
    assert_eq!(clones.get(), 0);
    assert_eq!(a.basic().values()[0].as_ref().unwrap().value, 0);
    assert_eq!(a.lists().lists()[0].len(), 2);
}
#[test]
fn lazy_joint_filter_runs_in_both_scoring_backends_and_selection_modes() {
    struct Provider {
        emitted: usize,
    }
    impl MoveProvider<MixedProblem<i32, i32>, MixedAssignment<i32, i32>, MixedMove<i32, i32>>
        for Provider
    {
        fn moves<'a>(
            &'a mut self,
            p: &'a MixedProblem<i32, i32>,
            a: &'a MixedAssignment<i32, i32>,
            _: u64,
        ) -> Box<dyn Iterator<Item = MixedMove<i32, i32>> + 'a>
        where
            MixedMove<i32, i32>: 'a,
        {
            let checker = p.move_checker(a).unwrap();
            let source = a.lists().location(&7).unwrap();
            let candidates = [
                MixedMove {
                    basic: Move::Composite(vec![]),
                    lists: ListMove::Composite(vec![]),
                },
                MixedMove {
                    basic: Move::Change {
                        variable: 0,
                        value: Some(1),
                    },
                    lists: ListMove::Change {
                        source,
                        destination: pos(9, 0),
                    },
                },
                MixedMove {
                    basic: Move::Change {
                        variable: 0,
                        value: Some(source.entity as i32 + 1),
                    },
                    lists: ListMove::Change {
                        source,
                        destination: pos(source.entity + 1, 0),
                    },
                },
            ];
            Box::new(candidates.into_iter().filter(move |m| {
                let keep = checker.is_doable(m) == Ok(true);
                self.emitted += usize::from(keep);
                keep
            }))
        }
    }
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap(),
        ListProblem::new(
            vec![7],
            vec![
                ListEntity::new(vec![7]),
                ListEntity::new(vec![]),
                ListEntity::new(vec![]),
            ],
        )
        .unwrap(),
    );
    for stream in [false, true] {
        for selection in [MoveSelection::FirstAccepted, MoveSelection::BestAccepted] {
            let mut provider = Provider { emitted: 0 };
            let config = MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    selection,
                    ..Default::default()
                },
                joint_moves: true,
            };
            let result = if stream {
                let mut b = StreamGraphBuilder::new();
                let s = MixedStreamSources::new(&mut b);
                b.score("basic", s.basic.port(), |(_, v)| {
                    SimpleScore::new(i64::from(v.unwrap()))
                })
                .unwrap();
                b.score("owner", s.lists.values.port(), |r| {
                    SimpleScore::new(r.position.unwrap().entity as i64)
                })
                .unwrap();
                mixed_stream_local_search_from_with_provider(
                    &p,
                    b.build(),
                    s,
                    p.initial_assignment(),
                    config,
                    &mut provider,
                )
                .unwrap()
            } else {
                let mut n = MixedConstraintNetwork::new(1, 3, 1);
                n.add_constraint(
                    "joint",
                    vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
                    |f| {
                        let (MixedFact::Basic(v), MixedFact::ListValue { element, .. }) =
                            (&f[0], &f[1])
                        else {
                            unreachable!()
                        };
                        SimpleScore::new(
                            i64::from(*v.unwrap())
                                + element.as_ref().unwrap().position.entity as i64,
                        )
                    },
                )
                .unwrap();
                mixed_local_search_from_with_provider(
                    &p,
                    &n,
                    p.initial_assignment(),
                    config,
                    &mut provider,
                )
                .unwrap()
            };
            assert_eq!(result.best.unwrap().score, SimpleScore::new(4));
            assert_eq!(result.evaluations, 3);
            assert_eq!(result.completed_steps, 2);
            assert_eq!(provider.emitted, 2);
        }
    }
}
