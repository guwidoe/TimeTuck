//! Original list-candidate checks against transactional execution and real solvers.
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn compare(p: &ListProblem<usize>, a: &ListAssignment<usize>, m: &ListMove<usize>) {
    let mut copy = a.clone();
    let expected = p.apply(&mut copy, m).map(|tx| tx.lists() != a.lists());
    assert_eq!(p.is_move_doable(a, m), expected, "{a:?}: {m:?}");
    assert_eq!(
        p.move_checker(a).and_then(|c| c.is_doable(m)),
        expected,
        "checker {m:?}"
    );
    assert_eq!(copy, *a);
}
fn random_move(draw: &mut impl FnMut(usize) -> usize) -> ListMove<usize> {
    let e = draw(4);
    let f = draw(4);
    let i = draw(5);
    let j = draw(5);
    match draw(7) {
        0 => ListMove::Assign {
            value: draw(10),
            destination: pos(e, i),
        },
        1 => ListMove::Change {
            source: pos(e, i),
            destination: pos(f, j),
        },
        2 => ListMove::Swap {
            left: pos(e, i),
            right: pos(f, j),
        },
        3 => ListMove::Unassign {
            entity: e,
            start: i,
            end: j,
        },
        4 => ListMove::SubListChange {
            entity: e,
            start: i,
            end: j,
            destination: pos(f, draw(5)),
            reverse: draw(2) == 0,
        },
        5 => ListMove::SubListSwap {
            left: ListRange {
                entity: e,
                start: i,
                end: j,
            },
            right: ListRange {
                entity: f,
                start: draw(5),
                end: draw(5),
            },
            reverse: draw(2) == 0,
        },
        _ => ListMove::Composite(vec![]),
    }
}
#[test]
fn every_move_family_and_nested_sequences_match_transactional_results_and_errors() {
    let mut seed = 1927_u64;
    let mut draw = |bound| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 32) as usize % bound
    };
    for mode in 0..4 {
        let mut entities = vec![
            ListEntity::new(vec![0, 1, 2]),
            ListEntity::new(vec![3, 4]),
            ListEntity::new(vec![5]),
        ];
        if mode == 1 {
            entities[0] = entities[0].clone().with_pinned_prefix(1);
            entities[1] = entities[1].clone().with_pinned(true);
        }
        if mode == 2 {
            for (entity, range) in
                entities
                    .iter_mut()
                    .zip([vec![0, 1, 2, 6], vec![3, 4, 6], vec![5, 7]])
            {
                *entity = entity
                    .clone()
                    .with_value_range(range)
                    .with_range_diagnostics("route", "visits", usize::to_string);
            }
        }
        let mut p = ListProblem::new((0..8).collect(), entities).unwrap();
        if mode == 3 {
            p = p
                .with_dependencies(vec![
                    vec![],
                    vec![0],
                    vec![1],
                    vec![],
                    vec![3],
                    vec![],
                    vec![],
                    vec![],
                ])
                .unwrap();
        }
        let mut a = p.initial_assignment();
        for _ in 0..500 {
            let first = random_move(&mut draw);
            compare(&p, &a, &first);
            let second = random_move(&mut draw);
            compare(
                &p,
                &a,
                &ListMove::Composite(vec![first.clone(), ListMove::Composite(vec![second])]),
            );
            if let Ok(tx) = p.apply(&mut a, &first) {
                tx.commit();
            }
        }
    }
}
#[test]
fn final_pin_range_checks_allow_restoration_but_preserve_operation_error_order() {
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![
            ListEntity::new(vec![0, 1])
                .with_pinned_prefix(1)
                .with_value_range(vec![0, 1]),
            ListEntity::new(vec![2]).with_value_range(vec![2]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let swap = ListMove::Swap {
        left: pos(0, 0),
        right: pos(1, 0),
    };
    assert!(p.is_move_doable(&a, &swap).is_err());
    let restore = ListMove::Composite(vec![swap.clone(), swap.clone()]);
    assert_eq!(p.is_move_doable(&a, &restore), Ok(false));
    let invalid = ListMove::Composite(vec![
        swap,
        ListMove::Change {
            source: pos(3, 0),
            destination: pos(0, 0),
        },
    ]);
    assert_eq!(
        p.is_move_doable(&a, &invalid),
        Err(ListError::EntityIndex {
            entity: 3,
            count: 2
        })
    );
    compare(&p, &ListAssignment::new(vec![]), &invalid);
    compare(
        &p,
        &ListAssignment::new(vec![vec![0, 0], vec![2]]),
        &invalid,
    );
    compare(
        &p,
        &a,
        &ListMove::Composite(vec![
            ListMove::Assign {
                value: 1,
                destination: pos(1, 0),
            },
            ListMove::Unassign {
                entity: 0,
                start: 1,
                end: 2,
            },
        ]),
    );
    let remove_restore = ListMove::Composite(vec![
        ListMove::Unassign {
            entity: 0,
            start: 0,
            end: 1,
        },
        ListMove::Assign {
            value: 0,
            destination: pos(0, 0),
        },
    ]);
    assert_eq!(p.is_move_doable(&a, &remove_restore), Ok(false));
}
#[test]
fn dependency_cycles_are_checked_globally_after_final_rows() {
    let p = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![2, 3])],
    )
    .unwrap()
    .with_dependencies(vec![vec![], vec![0], vec![1], vec![2]])
    .unwrap();
    let a = p.initial_assignment();
    let cycle = ListMove::Change {
        source: pos(1, 1),
        destination: pos(0, 0),
    };
    let error = p.is_move_doable(&a, &cycle).unwrap_err();
    assert_eq!(
        error,
        ListError::DependencyCycle {
            values: vec![0, 1, 2, 3]
        }
    );
    compare(&p, &a, &cycle);
    let restore = ListMove::Composite(vec![
        cycle,
        ListMove::Change {
            source: pos(0, 0),
            destination: pos(1, 1),
        },
    ]);
    assert_eq!(p.is_move_doable(&a, &restore), Ok(false));
    compare(&p, &a, &restore);
}
#[test]
fn checks_clone_no_planning_values_and_preserve_payload_sensitive_diagnostics() {
    use std::{cell::Cell, rc::Rc};
    #[derive(Debug)]
    struct Value {
        id: usize,
        payload: &'static str,
        clones: Rc<Cell<usize>>,
    }
    impl PartialEq for Value {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl Eq for Value {}
    impl Clone for Value {
        fn clone(&self) -> Self {
            self.clones.set(self.clones.get() + 1);
            Self {
                id: self.id,
                payload: self.payload,
                clones: Rc::clone(&self.clones),
            }
        }
    }
    let clones = Rc::new(Cell::new(0));
    let value = |id, payload| Value {
        id,
        payload,
        clones: Rc::clone(&clones),
    };
    let p = ListProblem::new(
        (0..128).map(|id| value(id, "registry")).collect(),
        vec![
            ListEntity::new((0..64).map(|id| value(id, "left")).collect()),
            ListEntity::new((64..127).map(|id| value(id, "right")).collect())
                .with_value_range((64..127).map(|id| value(id, "allowed")).collect())
                .with_range_diagnostics("route", "visits", |v| format!("{}:{}", v.id, v.payload)),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    clones.set(0);
    let checker = p.move_checker(&a).unwrap();
    let assignment = ListMove::Assign {
        value: value(127, "candidate"),
        destination: pos(1, 0),
    };
    let error = checker.is_doable(&assignment).unwrap_err();
    assert!(format!("{error:?}").contains("127:candidate"));
    for movement in [
        ListMove::Change {
            source: pos(0, 0),
            destination: pos(1, 0),
        },
        ListMove::Swap {
            left: pos(1, 0),
            right: pos(0, 0),
        },
    ] {
        let error = checker.is_doable(&movement).unwrap_err();
        assert!(format!("{error:?}").contains("0:left"));
    }

    let movement = ListMove::Composite(vec![
        ListMove::SubListChange {
            entity: 0,
            start: 0,
            end: 3,
            destination: pos(1, 2),
            reverse: true,
        },
        ListMove::SubListChange {
            entity: 1,
            start: 2,
            end: 5,
            destination: pos(0, 0),
            reverse: true,
        },
        ListMove::Swap {
            left: pos(0, 0),
            right: pos(0, 63),
        },
    ]);
    assert_eq!(checker.is_doable(&movement), Ok(true));
    assert_eq!(clones.get(), 0);
    assert_eq!(a.lists()[0][0].payload, "left");
}

#[test]
fn lazy_list_and_mixed_providers_filter_on_all_four_scoring_paths() {
    struct Provider {
        emitted: usize,
    }
    impl MoveProvider<ListProblem<i32>, ListAssignment<i32>, ListMove<i32>> for Provider {
        fn moves<'a>(
            &'a mut self,
            p: &'a ListProblem<i32>,
            a: &'a ListAssignment<i32>,
            _: u64,
        ) -> Box<dyn Iterator<Item = ListMove<i32>> + 'a>
        where
            ListMove<i32>: 'a,
        {
            let checker = p.move_checker(a).unwrap();
            let source = a.location(&7).unwrap();
            let candidates = [
                ListMove::Change {
                    source,
                    destination: source,
                },
                ListMove::Change {
                    source,
                    destination: pos(source.entity + 1, 0),
                },
                ListMove::Swap {
                    left: source,
                    right: pos(9, 0),
                },
            ];
            Box::new(candidates.into_iter().filter(move |m| {
                let keep = checker.is_doable(m) == Ok(true);
                self.emitted += usize::from(keep);
                keep
            }))
        }
    }
    impl MoveProvider<MixedProblem<i32, i32>, MixedAssignment<i32, i32>, MixedMove<i32, i32>>
        for Provider
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
                self.moves(p.lists(), a.lists(), step)
                    .map(|lists| MixedMove {
                        basic: Move::Composite(vec![]),
                        lists,
                    }),
            )
        }
    }
    let p = ListProblem::new(
        vec![7],
        vec![
            ListEntity::new(vec![7]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let mixed = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![9]).with_initial(9)]).unwrap(),
        p.clone(),
    );
    for stream in [false, true] {
        let mut provider = Provider { emitted: 0 };
        let config = LocalSearchConfig {
            selection: MoveSelection::BestAccepted,
            ..Default::default()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = ListStreamSources::new(&mut b);
            b.score("owner", source.values.port(), |v| {
                SimpleScore::new(v.position.unwrap().entity as i64)
            })
            .unwrap();
            list_stream_local_search_from_with_provider(
                &p,
                b.build(),
                source,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            let mut n = ListConstraintNetwork::new(3, 1);
            n.add_constraint("owner", vec![ListScope::Value(0)], |f| {
                let ListFact::Value { element, .. } = &f[0] else {
                    unreachable!()
                };
                SimpleScore::new(element.as_ref().unwrap().position.entity as i64)
            })
            .unwrap();
            list_local_search_from_with_provider(
                &p,
                &n,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        };
        assert_eq!(result.best.unwrap().score, SimpleScore::new(2));
        assert_eq!(result.evaluations, 3);
        assert_eq!(result.completed_steps, 2);
        assert_eq!(provider.emitted, 2);
        let mut provider = Provider { emitted: 0 };
        let config = MixedLocalSearchConfig {
            search: config,
            joint_moves: false,
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = MixedStreamSources::new(&mut b);
            b.score("owner", source.lists.values.port(), |v| {
                SimpleScore::new(v.position.unwrap().entity as i64)
            })
            .unwrap();
            mixed_stream_local_search_from_with_provider(
                &mixed,
                b.build(),
                source,
                mixed.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            let mut n = MixedConstraintNetwork::new(1, 3, 1);
            n.add_constraint("owner", vec![MixedScope::ListValue(0)], |f| {
                let MixedFact::ListValue { element, .. } = &f[0] else {
                    unreachable!()
                };
                SimpleScore::new(element.as_ref().unwrap().position.entity as i64)
            })
            .unwrap();
            mixed_local_search_from_with_provider(
                &mixed,
                &n,
                mixed.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        };
        assert_eq!(result.best.unwrap().score, SimpleScore::new(2));
        assert_eq!(result.evaluations, 3);
        assert_eq!(result.completed_steps, 2);
        assert_eq!(provider.emitted, 2);
    }
}

#[test]
fn unequal_span_swaps_and_deep_composites_keep_pre_move_coordinates() {
    let p = ListProblem::new(
        (0..9).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3, 4]),
            ListEntity::new(vec![5, 6, 7, 8]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let small = ListRange {
        entity: 0,
        start: 0,
        end: 1,
    };
    let large = ListRange {
        entity: 0,
        start: 2,
        end: 5,
    };
    let cross = ListRange {
        entity: 1,
        start: 1,
        end: 3,
    };
    for (left, right) in [
        (small, large),
        (large, small),
        (large, cross),
        (cross, large),
    ] {
        for reverse in [false, true] {
            let m = ListMove::SubListSwap {
                left,
                right,
                reverse,
            };
            assert_eq!(p.is_move_doable(&a, &m), Ok(true));
            compare(&p, &a, &m);
        }
    }
    let mut deep = ListMove::Change {
        source: pos(0, 0),
        destination: pos(0, 0),
    };
    for _ in 0..512 {
        deep = ListMove::Composite(vec![deep]);
    }
    assert_eq!(p.is_move_doable(&a, &deep), Ok(false));
    let unknown = ListMove::Composite(vec![
        ListMove::Assign {
            value: 99,
            destination: pos(0, 0),
        },
        ListMove::Unassign {
            entity: 0,
            start: 0,
            end: 1,
        },
    ]);
    assert_eq!(
        p.is_move_doable(&a, &unknown),
        Err(ListError::ValueOutsideDomain)
    );
    compare(&p, &a, &unknown);
}

#[test]
fn primitive_geometry_pins_and_range_error_order_match_transactions_exhaustively() {
    for mode in 0..7 {
        let mut entities = vec![
            ListEntity::new(vec![0, 1, 2]),
            ListEntity::new(vec![3, 4]),
            ListEntity::new(vec![]),
        ];
        if mode == 1 || mode == 6 {
            entities[0] = entities[0].clone().with_pinned(true);
        }
        if mode == 2 || mode == 5 {
            entities[1] = entities[1].clone().with_pinned(true);
        }
        if mode == 3 {
            for e in &mut entities[..2] {
                *e = e.clone().with_pinned_prefix(1);
            }
        }
        if mode >= 4 {
            entities[0] = entities[0]
                .clone()
                .with_value_range(vec![0, 1, 2])
                .with_range_diagnostics("row", "values", usize::to_string);
            entities[1] = entities[1]
                .clone()
                .with_value_range(vec![3, 4])
                .with_range_diagnostics("row", "values", usize::to_string);
        }
        let p = ListProblem::new((0..5).collect(), entities).unwrap();
        let a = p.initial_assignment();
        for e in 0..4 {
            for i in 0..4 {
                for f in 0..4 {
                    for j in 0..4 {
                        compare(
                            &p,
                            &a,
                            &ListMove::Swap {
                                left: pos(e, i),
                                right: pos(f, j),
                            },
                        );
                        compare(
                            &p,
                            &a,
                            &ListMove::Change {
                                source: pos(e, i),
                                destination: pos(f, j),
                            },
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn plain_primitive_checks_do_not_compare_unaffected_values() {
    use std::{cell::Cell, rc::Rc};
    #[derive(Clone, Debug)]
    struct Value(usize, Rc<Cell<usize>>);
    impl PartialEq for Value {
        fn eq(&self, other: &Self) -> bool {
            self.1.set(self.1.get() + 1);
            self.0 == other.0
        }
    }
    impl Eq for Value {}
    for n in [32, 256] {
        let calls = Rc::new(Cell::new(0));
        let v = |i| Value(i, calls.clone());
        let p = ListProblem::new(
            (0..n).map(v).collect(),
            vec![
                ListEntity::new((0..n / 2).map(v).collect()),
                ListEntity::new((n / 2..n).map(v).collect()),
            ],
        )
        .unwrap();
        let a = p.initial_assignment();
        let c = p.move_checker(&a).unwrap();
        calls.set(0);
        for _ in 0..100 {
            assert_eq!(
                c.is_doable(&ListMove::Swap {
                    left: pos(0, n / 2 - 1),
                    right: pos(1, n / 2 - 1)
                }),
                Ok(true)
            );
            assert_eq!(
                c.is_doable(&ListMove::Change {
                    source: pos(0, n / 2 - 1),
                    destination: pos(1, n / 2)
                }),
                Ok(true)
            );
            assert_eq!(
                c.is_doable(&ListMove::Swap {
                    left: pos(0, 0),
                    right: pos(0, 0)
                }),
                Ok(false)
            );
        }
        assert_eq!(
            c.is_doable(&ListMove::Unassign {
                entity: 0,
                start: 0,
                end: 0
            }),
            Ok(false)
        );
        assert_eq!(
            c.is_doable(&ListMove::Unassign {
                entity: 0,
                start: n / 2 - 2,
                end: n / 2
            }),
            Ok(true)
        );
        for reverse in [false, true] {
            for destination in [pos(0, 0), pos(0, n / 2 - 4), pos(1, 0)] {
                assert_eq!(
                    c.is_doable(&ListMove::SubListChange {
                        entity: 0,
                        start: 0,
                        end: 4,
                        destination,
                        reverse,
                    }),
                    Ok(reverse || destination != pos(0, 0))
                );
            }
            for entity in [0, 1] {
                assert_eq!(
                    c.is_doable(&ListMove::SubListSwap {
                        left: ListRange {
                            entity: 0,
                            start: 0,
                            end: 4
                        },
                        right: ListRange {
                            entity,
                            start: 5,
                            end: 8
                        },
                        reverse,
                    }),
                    Ok(true)
                );
            }
        }
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn assignment_and_removal_geometry_domains_and_pins_match_transactions() {
    for mode in 0..5 {
        let mut entities = vec![
            ListEntity::new(vec![0, 1, 2]),
            ListEntity::new(vec![3]),
            ListEntity::new(vec![]),
        ];
        if mode == 1 {
            entities[0] = entities[0].clone().with_pinned_prefix(1);
        }
        if mode == 2 {
            entities[0] = entities[0].clone().with_pinned(true);
            entities[2] = entities[2].clone().with_pinned(true);
        }
        if mode == 3 {
            entities[0] = entities[0].clone().with_value_range(vec![0, 1, 2, 4]);
            entities[1] = entities[1]
                .clone()
                .with_value_range(vec![3, 5])
                .with_range_diagnostics("row", "values", usize::to_string);
        }
        let mut p = ListProblem::new((0..6).collect(), entities).unwrap();
        if mode == 4 {
            p = p.with_dependencies(vec![vec![]; 6]).unwrap();
        }
        let a = p.initial_assignment();
        for e in 0..4 {
            for i in 0..5 {
                for value in 0..7 {
                    compare(
                        &p,
                        &a,
                        &ListMove::Assign {
                            value,
                            destination: pos(e, i),
                        },
                    );
                }
                for end in 0..5 {
                    compare(
                        &p,
                        &a,
                        &ListMove::Unassign {
                            entity: e,
                            start: i,
                            end,
                        },
                    );
                }
            }
        }
    }
}

#[test]
fn sublist_geometry_reversal_pins_and_arrival_diagnostics_match_transactions() {
    for mode in 0..6 {
        let mut entities = vec![
            ListEntity::new(vec![0, 1, 2]),
            ListEntity::new(vec![3, 4]),
            ListEntity::new(vec![]),
        ];
        if mode == 1 {
            entities[0] = entities[0].clone().with_pinned_prefix(1);
        }
        if mode == 2 || mode == 5 {
            entities[1] = entities[1].clone().with_pinned(true);
            entities[2] = entities[2].clone().with_pinned(true);
        }
        if mode >= 3 {
            entities[0] = entities[0].clone().with_value_range(vec![0, 1, 2, 4]);
            entities[1] = entities[1].clone().with_value_range(vec![1, 3, 4]);
            for e in &mut entities {
                *e = e
                    .clone()
                    .with_range_diagnostics("route", "visits", usize::to_string);
            }
        }
        let mut p = ListProblem::new((0..5).collect(), entities).unwrap();
        if mode == 4 {
            p = p.with_dependencies(vec![vec![]; 5]).unwrap();
        }
        let a = p.initial_assignment();
        let ranges: Vec<_> = (0..4)
            .flat_map(|entity| {
                [
                    (0, 0),
                    (0, 1),
                    (0, 2),
                    (0, 3),
                    (1, 2),
                    (1, 3),
                    (2, 3),
                    (2, 4),
                    (2, 1),
                ]
                .map(move |(start, end)| ListRange { entity, start, end })
            })
            .collect();
        for left in &ranges {
            for reverse in [false, true] {
                for entity in 0..4 {
                    for index in 0..5 {
                        compare(
                            &p,
                            &a,
                            &ListMove::SubListChange {
                                entity: left.entity,
                                start: left.start,
                                end: left.end,
                                destination: pos(entity, index),
                                reverse,
                            },
                        );
                    }
                }
                for right in &ranges {
                    compare(
                        &p,
                        &a,
                        &ListMove::SubListSwap {
                            left: *left,
                            right: *right,
                            reverse,
                        },
                    );
                }
            }
        }
    }
}
