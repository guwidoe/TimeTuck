//! Original branch-order behavior; pinned exhaustive-search docs motivate the API.
use std::{cell::RefCell, collections::BTreeSet};
use timetuck::*;
fn exact(order: Option<BranchOrder>, completion: bool, budget: u64) -> ExactPhaseConfig<'static> {
    let c = BoundedPhaseConfig {
        order: order.map(Into::into),
        exploration: NodeExploration::InputOrderDepthFirst,
        ..Default::default()
    };
    ExactPhaseConfig {
        strategy: if completion {
            ExactSearchStrategy::CompleteBranchAndBound(c)
        } else {
            ExactSearchStrategy::BranchAndBound(c)
        },
        max_evaluations: budget,
        ..Default::default()
    }
}
fn plan(c: ExactPhaseConfig<'_>) -> SolverSequence<'_> {
    SolverSequence {
        phases: vec![SolverPhase::Exact(c)],
        ..Default::default()
    }
}
#[test]
fn full_permutations_validate_pins_and_zero_budgets_before_callbacks() {
    use BranchOrderDimension as D;
    let p = FiniteProblem::new(vec![
        Variable::new(vec![3, 7]).with_initial(3).with_pinned(true),
        Variable::new(vec![2, 4]),
    ])
    .unwrap();
    let cases = [
        (
            BranchOrder {
                basic_variables: Some(vec![0]),
                ..Default::default()
            },
            BranchOrderError::Length {
                dimension: D::BasicVariables,
                expected: 2,
                actual: 1,
            },
        ),
        (
            BranchOrder {
                basic_variables: Some(vec![0, 0]),
                ..Default::default()
            },
            BranchOrderError::Duplicate {
                dimension: D::BasicVariables,
                position: 1,
                value: 0,
            },
        ),
        (
            BranchOrder {
                basic_variables: Some(vec![2, 0]),
                ..Default::default()
            },
            BranchOrderError::OutOfRange {
                dimension: D::BasicVariables,
                position: 0,
                value: 2,
                length: 2,
            },
        ),
        (
            BranchOrder {
                basic_values: Some(vec![]),
                ..Default::default()
            },
            BranchOrderError::Length {
                dimension: D::BasicDomains,
                expected: 2,
                actual: 0,
            },
        ),
        (
            BranchOrder {
                basic_values: Some(vec![vec![1, 1], vec![0, 1]]),
                ..Default::default()
            },
            BranchOrderError::Duplicate {
                dimension: D::BasicValues { variable: 0 },
                position: 1,
                value: 1,
            },
        ),
        (
            BranchOrder {
                list_values: Some(vec![]),
                ..Default::default()
            },
            BranchOrderError::Unavailable {
                dimension: D::ListValues,
            },
        ),
    ];
    for (order, expected) in cases {
        let mut factory=|| -> Result<(StreamGraph<'static,SimpleScore>,PlanningStreamSource<i32>),GraphError> { panic!("invalid order ran factory") };
        let mut events = |_: SolverEvent<'_, Assignment<i32>, SimpleScore>| {
            panic!("invalid order emitted event")
        };
        let c = plan(exact(Some(order), true, 0));
        let error = solve_stream_with_extensions(
            &p,
            &mut factory,
            c,
            SolverExtensions {
                events: Some(&mut events),
                ..Default::default()
            },
        )
        .unwrap_err();
        let SolverError::InvalidBranchOrder { phase, error } = error else {
            panic!("wrong error: {error:?}")
        };
        assert_eq!(phase, 0);
        assert_eq!(error, expected);
    }
    let l = ListProblem::new(
        vec![1, 2],
        vec![
            ListEntity::new(vec![1]).with_pinned(true),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    for order in [
        BranchOrder {
            list_values: Some(vec![0, 0]),
            ..Default::default()
        },
        BranchOrder {
            list_entities: Some(vec![1]),
            ..Default::default()
        },
        BranchOrder {
            list_entities: Some(vec![1, 2]),
            ..Default::default()
        },
        BranchOrder {
            basic_variables: Some(vec![]),
            ..Default::default()
        },
    ] {
        let n = ListConstraintNetwork::<_, SimpleScore>::new(2, 2);
        assert!(matches!(
            solve_list(&l, &n, plan(exact(Some(order), true, 0))),
            Err(SolverError::InvalidBranchOrder { phase: 0, .. })
        ));
    }
}
#[test]
fn generation_trace_uses_model_indices_and_all_budget_prefixes_retain_first_tie() {
    let p = FiniteProblem::new(vec![Variable::new(vec![9_i64, 2]); 2]).unwrap();
    let expected = [
        vec![None, None],
        vec![None, Some(2)],
        vec![None, Some(9)],
        vec![Some(2), Some(2)],
        vec![Some(9), Some(2)],
        vec![Some(2), Some(9)],
        vec![Some(9), Some(9)],
    ];
    for budget in 0..=7 {
        let trace = RefCell::new(vec![]);
        let mut n = ConstraintNetwork::<_, SimpleScore>::new(2);
        n.add_constraint("misleading partial", vec![0, 1], |v| {
            let a = v.iter().map(|v| v.copied()).collect::<Vec<_>>();
            trace.borrow_mut().push(a);
            SimpleScore::new(if v.iter().any(Option::is_none) {
                v.iter().flatten().map(|v| **v).sum()
            } else {
                0
            })
        })
        .unwrap();
        let order = BranchOrder {
            basic_variables: Some(vec![1, 0]),
            basic_values: Some(vec![vec![1, 0], vec![1, 0]]),
            ..Default::default()
        };
        let r = solve(&p, &n, plan(exact(Some(order), false, budget))).unwrap();
        assert_eq!(*trace.borrow(), expected[..budget as usize]);
        assert_eq!(r.work.evaluations, budget);
        assert_eq!(r.score, (budget >= 4).then_some(SimpleScore::ZERO));
        if budget >= 4 {
            assert_eq!(r.assignment.values(), &[Some(2), Some(2)]);
        }
        assert_eq!(r.is_optimal(), budget == 7);
    }
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("ties", vec![0, 1], |_| SimpleScore::ZERO)
        .unwrap();
    let r = solve(&p, &n, plan(exact(None, false, 100))).unwrap();
    assert_eq!(
        r.assignment.values(),
        &[Some(9), Some(9)],
        "default order stays unchanged"
    );
}
#[test]
fn completion_filters_fixed_payloads_and_pins_without_renumbering_domain_entries() {
    #[derive(Clone, Debug)]
    struct V {
        id: i32,
        payload: i64,
    }
    impl PartialEq for V {
        fn eq(&self, o: &Self) -> bool {
            self.id == o.id
        }
    }
    impl Eq for V {}
    let v = |id, payload| V { id, payload };
    let p = FiniteProblem::new(vec![
        Variable::new(vec![v(0, 1), v(1, 2)]).with_initial(v(0, 100)),
        Variable::new(vec![v(2, 3), v(3, 4)])
            .with_initial(v(2, 200))
            .with_pinned(true),
        Variable::new(vec![v(4, 5), v(5, 6)]).allowing_unassigned(),
    ])
    .unwrap();
    let trace = RefCell::new(vec![]);
    let mut n = ConstraintNetwork::<V, SimpleScore>::new(3);
    n.add_constraint("payloads", vec![0, 1, 2], |x| {
        trace.borrow_mut().push(
            x.iter()
                .map(|v| v.map(|v| (v.id, v.payload)))
                .collect::<Vec<_>>(),
        );
        SimpleScore::new(x.iter().flatten().map(|v| v.payload).sum())
    })
    .unwrap();
    let order = BranchOrder {
        basic_variables: Some(vec![2, 1, 0]),
        basic_values: Some(vec![vec![1, 0]; 3]),
        ..Default::default()
    };
    let r = solve(&p, &n, plan(exact(Some(order), true, 100))).unwrap();
    assert_eq!(r.score, Some(SimpleScore::new(306)));
    assert!(!r.is_optimal());
    let t = trace.borrow();
    assert_eq!(t.len(), 4);
    assert!(
        t.iter()
            .all(|x| x[0] == Some((0, 100)) && x[1] == Some((2, 200)))
    );
    assert_eq!(
        t.iter().map(|x| x[2]).collect::<Vec<_>>(),
        vec![None, Some((5, 6)), Some((4, 5)), None]
    );
}
#[test]
fn ordered_list_leaves_equal_independent_assignment_set_with_pins_and_optional_values() {
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![
            ListEntity::new(vec![0]).with_pinned_prefix(1),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let mut expected = BTreeSet::new();
    // Independent insertion enumeration: every placement of 1 and 2 after frozen0,
    // in the second row, or omitted. This deliberately uses a different value order.
    for a in [None, Some((0, 1)), Some((1, 0))] {
        let mut rows = vec![vec![0], vec![]];
        if let Some((e, i)) = a {
            rows[e].insert(i, 1);
        }
        expected.insert(rows.clone());
        for e in 0..2 {
            for i in if e == 0 { 1 } else { 0 }..=rows[e].len() {
                let mut out = rows.clone();
                out[e].insert(i, 2);
                expected.insert(out);
            }
        }
    }
    let seen = RefCell::new(BTreeSet::new());
    let mut n = ListConstraintNetwork::new(2, 3);
    n.add_constraint(
        "record",
        vec![ListScope::Entity(0), ListScope::Entity(1)],
        |f| {
            let [ListFact::Entity(a), ListFact::Entity(b)] = f else {
                unreachable!()
            };
            seen.borrow_mut().insert(vec![a.to_vec(), b.to_vec()]);
            SimpleScore::ZERO
        },
    )
    .unwrap();
    let order = BranchOrder {
        list_values: Some(vec![2, 0, 1]),
        list_entities: Some(vec![1, 0]),
        ..Default::default()
    };
    let r = solve_list(&p, &n, plan(exact(Some(order), false, 1000))).unwrap();
    assert!(r.is_optimal());
    assert_eq!(*seen.borrow(), expected);
    assert_eq!(
        r.assignment.lists(),
        &[vec![0], vec![1, 2]],
        "last inserted1 uses position0 in prioritized entity1"
    );
}
#[test]
fn constrained_variable_priority_reduces_actual_pruning_work_with_same_incumbent() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]).with_initial(0); 8]).unwrap();
    let mut n = ConstraintNetwork::new(8);
    n.add_constraint("positive costs", (0..8).collect(), |v| {
        SimpleScore::new(
            -v.iter()
                .enumerate()
                .filter_map(|(i, v)| v.map(|x| 1 + if i == 7 { 100 * x } else { 0 }))
                .sum::<i64>(),
        )
    })
    .unwrap();
    let run = |ordered: bool| {
        let mut c = exact(
            ordered.then(|| BranchOrder {
                basic_variables: Some(vec![7, 0, 1, 2, 3, 4, 5, 6]),
                ..Default::default()
            }),
            false,
            10000,
        );
        let ExactSearchStrategy::BranchAndBound(ref mut b) = c.strategy else {
            unreachable!()
        };
        b.bound = PartialScoreBound::NonIncreasing;
        solve(
            &p,
            &n,
            SolverSequence {
                phases: vec![
                    SolverPhase::LocalSearch(LocalSearchConfig {
                        max_steps: 0,
                        ..Default::default()
                    }),
                    SolverPhase::Exact(c),
                ],
                ..Default::default()
            },
        )
        .unwrap()
    };
    let before = run(false);
    let after = run(true);
    assert_eq!(before.score, Some(SimpleScore::new(-8)));
    assert_eq!(after.score, before.score);
    assert_eq!(after.assignment, before.assignment);
    assert!(before.is_optimal() && after.is_optimal());
    assert_eq!(before.phases[1].work.evaluations, 511);
    assert_eq!(after.phases[1].work.evaluations, 257);
}

#[test]
fn reordered_dependency_insertions_obey_ranges_and_completion_anchors() {
    let p = ListProblem::new(
        vec![1i64, 2, 3, 4],
        vec![
            ListEntity::new(vec![4]).with_pinned(true),
            ListEntity::new(vec![1, 3])
                .with_pinned_prefix(1)
                .with_value_range(vec![1, 2, 3]),
        ],
    )
    .unwrap()
    .with_dependencies(vec![vec![], vec![0], vec![1], vec![]])
    .unwrap();
    let n = ListConstraintNetwork::<_, SimpleScore>::new(2, 4);
    for complete in [false, true] {
        let order = BranchOrder {
            list_values: Some(vec![3, 2, 1, 0]),
            list_entities: Some(vec![1, 0]),
            ..Default::default()
        };
        let mut d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        let r = d
            .run_solver(plan(exact(Some(order), complete, 100)))
            .unwrap();
        assert_eq!(r.assignment.lists(), &[vec![4], vec![1, 2, 3]]);
        assert_eq!(d.assignment(), &r.assignment);
        d.assert_consistent().unwrap();
        assert_eq!(r.is_optimal(), !complete);
    }
}
#[test]
fn completion_and_unrestricted_prefixes_restore_retained_state_on_cancellation() {
    use std::cell::Cell;
    let p = FiniteProblem::new(vec![
        Variable::new(vec![3_i64, 7]).with_initial(3),
        Variable::new(vec![2, 5]),
        Variable::new(vec![1, 4]),
    ])
    .unwrap();
    for completion in [false, true] {
        for cutoff in 0..=15 {
            let token = CancellationToken::new();
            let active = Cell::new(false);
            let calls = Cell::new(0);
            let mut n = ConstraintNetwork::new(3);
            n.add_constraint("sum", vec![0, 1, 2], |v| {
                if active.get() {
                    calls.set(calls.get() + 1);
                    if calls.get() == cutoff {
                        token.cancel();
                    }
                }
                SimpleScore::new(v.iter().flatten().map(|v| **v).sum())
            })
            .unwrap();
            let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
            if cutoff == 0 {
                token.cancel();
            }
            active.set(true);
            let order = BranchOrder {
                basic_variables: Some(vec![2, 0, 1]),
                basic_values: Some(vec![vec![1, 0]; 3]),
                ..Default::default()
            };
            let mut c = exact(Some(order), completion, 100);
            c.cancellation = Some(&token);
            let r = d.run_solver(plan(c)).unwrap();
            active.set(false);
            assert!(calls.get() <= cutoff || cutoff > if completion { 7 } else { 15 });
            assert_eq!(d.assignment(), &r.assignment);
            d.assert_consistent().unwrap();
            if r.complete {
                let sum = r.assignment.values().iter().flatten().sum();
                assert_eq!(r.score, Some(SimpleScore::new(sum)));
            }
            if completion {
                assert_eq!(r.assignment.values()[0], Some(3));
            }
            assert_eq!(r.work.evaluations, calls.get());
        }
    }
}

#[test]
fn reordered_dependency_tree_matches_independently_enumerated_valid_leaves() {
    // Fixed dependencies 1->0, 2->1 mean a row containing 2 before1 creates
    // a cycle. Entity0 is frozen3; entity1 has frozen prefix0. Enumerate all
    // destinations/positions for1 then2 independently of the reversed tree.
    let mut expected = BTreeSet::new();
    for first in 1..=2 {
        let mut rows = vec![vec![3_i64], vec![0], vec![]];
        rows[first].push(1);
        for second in 1..=2 {
            for index in usize::from(second == 1)..=rows[second].len() {
                let mut out = rows.clone();
                out[second].insert(index, 2);
                if out.iter().all(|row| !matches!((row.iter().position(|v|*v==2),row.iter().position(|v|*v==1)),(Some(a),Some(b)) if a<b)) {expected.insert(out);}
            }
        }
    }
    assert_eq!(expected.len(), 4);
    let p = ListProblem::new(
        vec![0_i64, 1, 2, 3],
        vec![
            ListEntity::new(vec![3]).with_pinned(true),
            ListEntity::new(vec![0])
                .with_pinned_prefix(1)
                .with_value_range(vec![0, 1, 2]),
            ListEntity::new(vec![]).with_value_range(vec![1, 2]),
        ],
    )
    .unwrap()
    .with_dependencies(vec![vec![], vec![0], vec![1], vec![]])
    .unwrap();
    for completion in [false, true] {
        let seen = RefCell::new(BTreeSet::new());
        let mut n = ListConstraintNetwork::new(3, 4);
        n.add_constraint(
            "leaf set",
            vec![
                ListScope::Entity(0),
                ListScope::Entity(1),
                ListScope::Entity(2),
            ],
            |f| {
                let rows = f
                    .iter()
                    .map(|f| {
                        let ListFact::Entity(row) = f else {
                            unreachable!()
                        };
                        row.to_vec()
                    })
                    .collect::<Vec<_>>();
                if rows.iter().map(Vec::len).sum::<usize>() == 4 {
                    seen.borrow_mut().insert(rows);
                }
                SimpleScore::ZERO
            },
        )
        .unwrap();
        let order = BranchOrder {
            list_values: Some(vec![2, 3, 1, 0]),
            list_entities: Some(vec![2, 0, 1]),
            ..Default::default()
        };
        let r = solve_list(&p, &n, plan(exact(Some(order), completion, 100))).unwrap();
        assert_eq!(*seen.borrow(), expected);
        assert!(expected.contains(r.assignment.lists()));
        assert_eq!(r.is_optimal(), !completion);
    }
}
