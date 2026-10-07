use timetuck::*;
#[test]
fn optional_and_required_slots_have_disjoint_changes_assignments_and_unassignments() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1])
            .with_initial(0)
            .allowing_unassigned(),
        Variable::new(vec![0, 1]).allowing_unassigned(),
        Variable::new(vec![0, 1])
            .with_initial(1)
            .with_pinned(true)
            .allowing_unassigned(),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let provider = DefaultNeighborhood::default();
    let moves: Vec<_> = provider.basic_moves(&p, &a, 0).unwrap().collect();
    assert_eq!(
        &moves[..5],
        &[
            Move::Change {
                variable: 0,
                value: Some(1)
            },
            Move::Change {
                variable: 1,
                value: Some(1)
            },
            Move::Change {
                variable: 2,
                value: Some(0)
            },
            Move::Change {
                variable: 2,
                value: Some(1)
            },
            Move::Change {
                variable: 1,
                value: None
            },
        ]
    );
    assert!(moves[5..].iter().all(|m| matches!(m, Move::Swap { .. })));
    for m in &moves {
        assert_eq!(moves.iter().filter(|v| *v == m).count(), 1);
        let mut copy = a.clone();
        p.apply(&mut copy, m).unwrap().commit();
        assert_eq!(copy.values()[3], Some(1));
        assert!(p.is_complete(&copy));
    }
    let bad = Assignment::new(vec![]);
    assert!(provider.basic_moves(&p, &bad, 0).is_err());
}
#[test]
fn list_default_drains_real_families_in_order_and_preserves_all_constraints() {
    let p = ListProblem::new(
        vec![0, 1, 2, 3, 4, 5, 6],
        vec![
            ListEntity::new(vec![0, 1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![3, 4]).with_value_range(vec![1, 2, 3, 4, 6]),
            ListEntity::new(vec![5]).with_pinned(true),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let provider = DefaultNeighborhood::default();
    let actual: Vec<_> = provider.list_moves(&p, &a, 0).unwrap().collect();
    assert!(matches!(actual.first(), Some(ListMove::Change { .. })));
    assert!(matches!(actual.last(), Some(ListMove::Unassign { .. })));
    assert!(
        actual
            .iter()
            .any(|m| matches!(m, ListMove::SubListChange { reverse: true, .. }))
    );
    assert!(
        actual
            .iter()
            .any(|m| matches!(m, ListMove::SubListSwap { reverse: true, .. }))
    );
    assert!(
        actual
            .iter()
            .any(|m| matches!(m, ListMove::Assign { value: 6, .. }))
    );
    assert!(
        actual
            .iter()
            .any(|m| matches!(m, ListMove::Unassign { .. }))
    );
    for m in actual {
        let mut next = a.clone();
        p.apply(&mut next, &m).unwrap().commit();
        assert_eq!(next.lists()[0][0], 0);
        assert_eq!(next.lists()[2], [5]);
        assert!(p.is_complete(&next));
    }
}
#[test]
fn dependency_cycles_are_filtered_before_the_solver_receives_a_move() {
    let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0, 1, 2])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0], vec![]])
        .unwrap();
    let a = p.initial_assignment();
    let provider = DefaultNeighborhood::default();
    let mut raw: Vec<_> = list_change_moves(&p, &a)
        .unwrap()
        .chain(list_swap_moves(&p, &a).unwrap())
        .chain(list_two_opt_moves(&p, &a, false).unwrap())
        .collect();
    let before = raw.len();
    raw.retain(|m| {
        let mut next = a.clone();
        p.apply(&mut next, m).is_ok()
    });
    assert!(raw.len() < before);
    let actual: Vec<_> = provider.list_moves(&p, &a, 0).unwrap().collect();
    assert_eq!(actual, raw);
    assert!(!actual.is_empty());
    for m in actual {
        let mut next = a.clone();
        p.apply(&mut next, &m).unwrap().commit();
        p.validate(&next).unwrap();
    }
}
#[test]
fn explicit_entity_rows_swap_atomically_and_pins_remove_whole_rows() {
    let p = FiniteProblem::new(
        (0..6)
            .map(|i| Variable::new(vec![0, 1, 2]).with_initial(i / 2))
            .collect(),
    )
    .unwrap();
    let provider = DefaultNeighborhood::default()
        .with_entity_rows(vec![vec![0, 1], vec![2, 3], vec![4, 5]])
        .unwrap();
    let a = p.initial_assignment();
    let moves: Vec<_> = provider.basic_moves(&p, &a, 0).unwrap().collect();
    let swaps: Vec<_> = moves
        .iter()
        .filter(|m| matches!(m, Move::Composite(_)))
        .collect();
    assert_eq!(swaps.len(), 3);
    for m in swaps {
        let mut next = a.clone();
        p.apply(&mut next, m).unwrap().commit();
        for row in next.values().chunks(2) {
            assert_eq!(row[0], row[1]);
        }
    }
    let pinned = FiniteProblem::new(
        (0..6)
            .map(|i| {
                Variable::new(vec![0, 1, 2])
                    .with_initial(i / 2)
                    .with_pinned(i == 0)
            })
            .collect(),
    )
    .unwrap();
    let a = pinned.initial_assignment();
    let swaps: Vec<_> = provider
        .basic_moves(&pinned, &a, 0)
        .unwrap()
        .filter(|m| matches!(m, Move::Composite(_)))
        .collect();
    assert_eq!(swaps.len(), 1);
    assert!(swaps[0].touched_variables().iter().all(|&i| i >= 2));
    let invalid = DefaultNeighborhood::default()
        .with_entity_rows(vec![vec![99]])
        .unwrap();
    assert!(invalid.basic_moves(&p, &p.initial_assignment(), 0).is_err());
    assert!(
        DefaultNeighborhood::default()
            .with_entity_rows(vec![vec![0, 1], vec![1, 2]])
            .is_err()
    );
}
#[test]
fn mixed_generation_uses_current_components_and_seeded_order_is_reproducible() {
    let basic = FiniteProblem::new(vec![
        Variable::new(vec![0, 1])
            .with_initial(0)
            .allowing_unassigned(),
    ])
    .unwrap();
    let lists = ListProblem::new(vec![10, 20, 30], vec![ListEntity::new(vec![10, 20])])
        .unwrap()
        .allowing_unassigned();
    let p = MixedProblem::new(basic, lists);
    let a = p.initial_assignment();
    let seq = DefaultNeighborhood::default();
    let first: Vec<_> = seq.mixed_moves(&p, &a, 0).unwrap().collect();
    assert_eq!(seq.mixed_families(&p).len(), 10);
    for m in &first {
        let mut copy = a.clone();
        p.apply(&mut copy, m).unwrap().commit();
        p.validate(&copy).unwrap();
    }
    let random = DefaultNeighborhood::new(UnionOrder::Random { seed: 19 });
    let r: Vec<_> = random.mixed_moves(&p, &a, 7).unwrap().collect();
    assert_eq!(
        r,
        random.mixed_moves(&p, &a, 7).unwrap().collect::<Vec<_>>()
    );
    assert_eq!(r.len(), first.len());
    let mut unseen = first.clone();
    for m in r {
        let i = unseen.iter().position(|x| *x == m).unwrap();
        unseen.remove(i);
    }
    assert!(unseen.is_empty());
    let mut next = a.clone();
    p.apply(
        &mut next,
        &MixedMove {
            basic: Move::Change {
                variable: 0,
                value: None,
            },
            lists: ListMove::Composite(vec![]),
        },
    )
    .unwrap()
    .commit();
    let new: Vec<_> = seq.mixed_moves(&p, &next, 1).unwrap().collect();
    assert_ne!(first, new);
    assert!(new.iter().any(|m| m.basic
        == Move::Change {
            variable: 0,
            value: Some(0)
        }));
}
#[test]
fn opening_is_lazy_and_pinned_or_empty_models_exhaust() {
    use std::{cell::Cell, rc::Rc};
    #[derive(Debug)]
    struct Value {
        n: i32,
        copies: Rc<Cell<usize>>,
    }
    impl Clone for Value {
        fn clone(&self) -> Self {
            self.copies.set(self.copies.get() + 1);
            Self {
                n: self.n,
                copies: self.copies.clone(),
            }
        }
    }
    impl PartialEq for Value {
        fn eq(&self, b: &Self) -> bool {
            self.n == b.n
        }
    }
    impl Eq for Value {}
    let copies = Rc::new(Cell::new(0));
    let v = |n| Value {
        n,
        copies: copies.clone(),
    };
    let p = FiniteProblem::new(vec![Variable::new(vec![v(0), v(1)]).with_initial(v(0))]).unwrap();
    let a = p.initial_assignment();
    copies.set(0);
    let provider = DefaultNeighborhood::default();
    let mut it = provider.basic_moves(&p, &a, 0).unwrap();
    assert_eq!(copies.get(), 0);
    assert!(it.next().is_some());
    assert_eq!(copies.get(), 1);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0).with_pinned(true),
    ])
    .unwrap();
    assert!(
        provider
            .basic_moves(&p, &p.initial_assignment(), 0)
            .unwrap()
            .next()
            .is_none()
    );
    let p = ListProblem::<i32>::new(vec![], vec![]).unwrap();
    assert!(
        provider
            .list_moves(&p, &p.initial_assignment(), 0)
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn default_provider_solves_basic_list_and_mixed_models_with_both_backends() {
    let basic = FiniteProblem::new(vec![Variable::new(vec![0, 1]).allowing_unassigned()]).unwrap();
    let lists =
        ListProblem::new(vec![0, 1, 2, 3], vec![ListEntity::new(vec![0, 1, 2, 3])]).unwrap();
    let reward = |row: &[i32]| SimpleScore::new(i64::from(row == [3, 2, 1, 0]));
    let mut bn = ConstraintNetwork::new(1);
    bn.add_constraint("value", vec![0], |v| {
        SimpleScore::new(i64::from(v[0].copied().unwrap_or(0)))
    })
    .unwrap();
    let mut ln = ListConstraintNetwork::new(1, 4);
    ln.add_constraint("reverse", vec![ListScope::Entity(0)], |v| {
        let ListFact::Entity(row) = v[0] else {
            unreachable!()
        };
        reward(row)
    })
    .unwrap();
    let config = LocalSearchConfig {
        max_steps: 2,
        selection: MoveSelection::BestAccepted,
        assert_scores: true,
        ..Default::default()
    };
    for stream in [false, true] {
        let mut provider = DefaultNeighborhood::default();
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("value", source.port(), |r| {
                SimpleScore::new(i64::from(r.1.unwrap_or(0)))
            })
            .unwrap();
            stream_local_search_from_with_provider(
                &basic,
                b.build(),
                source,
                basic.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            local_search_from_with_provider(
                &basic,
                &bn,
                basic.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        };
        assert_eq!(result.best.unwrap().score, SimpleScore::new(1));
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let sources = ListStreamSources::new(&mut b);
            b.score("reverse", sources.entities.port(), move |r| reward(&r.1))
                .unwrap();
            list_stream_local_search_from_with_provider(
                &lists,
                b.build(),
                sources,
                lists.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            list_local_search_from_with_provider(
                &lists,
                &ln,
                lists.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        };
        let best = result.best.unwrap();
        assert_eq!(best.score, SimpleScore::new(1));
        assert_eq!(best.assignment.lists(), &[vec![3, 2, 1, 0]]);
    }
    let p = MixedProblem::new(basic, lists);
    let mut n = MixedConstraintNetwork::new(1, 1, 4);
    n.add_constraint("value", vec![MixedScope::Basic(0)], |v| {
        let MixedFact::Basic(value) = v[0] else {
            unreachable!()
        };
        SimpleScore::new(i64::from(value.copied().unwrap_or(0)))
    })
    .unwrap();
    n.add_constraint("reverse", vec![MixedScope::ListEntity(0)], |v| {
        let MixedFact::ListEntity(row) = v[0] else {
            unreachable!()
        };
        reward(row)
    })
    .unwrap();
    for stream in [false, true] {
        let mut provider = DefaultNeighborhood::default();
        let config = MixedLocalSearchConfig {
            search: config,
            ..Default::default()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let sources = MixedStreamSources::new(&mut b);
            b.score("value", sources.basic.port(), |r| {
                SimpleScore::new(i64::from(r.1.unwrap_or(0)))
            })
            .unwrap();
            b.score("reverse", sources.lists.entities.port(), move |r| {
                reward(&r.1)
            })
            .unwrap();
            mixed_stream_local_search_from_with_provider(
                &p,
                b.build(),
                sources,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
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
        assert_eq!(best.score, SimpleScore::new(2));
        assert_eq!(best.assignment.basic().values(), &[Some(1)]);
        assert_eq!(best.assignment.lists().lists(), &[vec![3, 2, 1, 0]]);
    }
}
