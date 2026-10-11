//! Original retained scoped-cache regressions; no upstream-method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn normalize<A, S>(mut r: SolverResult<A, S>) -> SolverResult<A, S> {
    r.work.constraint_evaluations = 0;
    for p in &mut r.phases {
        p.work.constraint_evaluations = 0;
    }
    r
}
fn basic() -> FiniteProblem<i64> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 2]).unwrap()
}
fn lists() -> ListProblem<i64> {
    ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![]); 2]).unwrap()
}
fn bn(calls: &Cell<u128>) -> ConstraintNetwork<'_, i64, SimpleScore> {
    let mut n = ConstraintNetwork::new(2);
    for i in 0..2 {
        n.add_constraint(format!("value{i}"), vec![i], move |v| {
            calls.set(calls.get() + 1);
            SimpleScore::new(v[0].copied().unwrap_or(0))
        })
        .unwrap();
    }
    n.add_constraint("constant", vec![], |_| {
        calls.set(calls.get() + 1);
        SimpleScore::new(7)
    })
    .unwrap();
    n
}
fn cost(v: &[i64]) -> SimpleScore {
    SimpleScore::new(
        -v.iter()
            .enumerate()
            .map(|(i, v)| (i as i64 + 1) * v)
            .sum::<i64>(),
    )
}
fn ln(calls: &Cell<u128>) -> ListConstraintNetwork<'_, i64, SimpleScore> {
    let mut n = ListConstraintNetwork::new(2, 2);
    for i in 0..2 {
        n.add_constraint(format!("list{i}"), vec![ListScope::Entity(i)], move |f| {
            calls.set(calls.get() + 1);
            let ListFact::Entity(v) = f[0] else {
                unreachable!()
            };
            cost(v)
        })
        .unwrap();
    }
    for i in 0..2 {
        n.add_constraint(
            format!("neighbors{i}"),
            vec![ListScope::Value(i)],
            move |f| {
                calls.set(calls.get() + 1);
                let ListFact::Value { element, .. } = &f[0] else {
                    unreachable!()
                };
                SimpleScore::new(element.as_ref().map_or(0, |e| {
                    e.previous.copied().unwrap_or(0) - e.next.copied().unwrap_or(0)
                }))
            },
        )
        .unwrap();
    }
    n
}
fn mn(calls: &Cell<u128>) -> MixedConstraintNetwork<'_, i64, i64, SimpleScore> {
    let mut n = MixedConstraintNetwork::new(2, 2, 2);
    for i in 0..2 {
        n.add_constraint(format!("basic{i}"), vec![MixedScope::Basic(i)], move |f| {
            calls.set(calls.get() + 1);
            let MixedFact::Basic(v) = f[0] else {
                unreachable!()
            };
            SimpleScore::new(v.copied().unwrap_or(0))
        })
        .unwrap();
    }
    for i in 0..2 {
        n.add_constraint(
            format!("list{i}"),
            vec![MixedScope::ListEntity(i)],
            move |f| {
                calls.set(calls.get() + 1);
                let MixedFact::ListEntity(v) = f[0] else {
                    unreachable!()
                };
                cost(v)
            },
        )
        .unwrap();
    }
    n.add_constraint(
        "coupled",
        vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
        |f| {
            calls.set(calls.get() + 1);
            let (MixedFact::Basic(b), MixedFact::ListValue { element, .. }) = (&f[0], &f[1]) else {
                unreachable!()
            };
            SimpleScore::new(
                b.copied().unwrap_or(0)
                    * element.as_ref().map_or(0, |e| e.position.entity as i64 + 1),
            )
        },
    )
    .unwrap();
    n
}
fn mixed(c: SolverSequence<'_>) -> MixedSolverSequence<'_> {
    MixedSolverSequence {
        limits: c.limits,
        phases: c
            .phases
            .into_iter()
            .map(|p| match p {
                SolverPhase::Construction(construction) => {
                    MixedSolverPhase::Construction(MixedConstructionConfig {
                        construction,
                        ..Default::default()
                    })
                }
                SolverPhase::LocalSearch(search) => {
                    MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                        search,
                        joint_moves: true,
                    })
                }
                SolverPhase::RuinRecreate(search) => {
                    MixedSolverPhase::RuinRecreate(MixedRuinRecreateConfig {
                        search,
                        ..Default::default()
                    })
                }
                SolverPhase::Exact(c) => MixedSolverPhase::Exact(c),
                SolverPhase::Custom(c) => MixedSolverPhase::Custom(c),
            })
            .collect(),
    }
}
fn plans() -> Vec<SolverSequence<'static>> {
    let local = SolverPhase::LocalSearch(LocalSearchConfig {
        max_steps: 3,
        acceptance: Acceptance::HillClimbing,
        ..Default::default()
    });
    [
        local.clone(),
        SolverPhase::RuinRecreate(RuinRecreateConfig {
            minimum_ruined: 1,
            maximum_ruined: 1,
            max_attempts: 3,
            seed: 7,
            ..Default::default()
        }),
        SolverPhase::Exact(ExactPhaseConfig::default()),
        SolverPhase::Exact(ExactPhaseConfig {
            strategy: ExactSearchStrategy::Complete,
            ..Default::default()
        }),
        SolverPhase::Exact(ExactPhaseConfig {
            strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig::default()),
            ..Default::default()
        }),
    ]
    .into_iter()
    .flat_map(|last| {
        [0, 1, 2, 5, 30].map(|cap| SolverSequence {
            limits: SolverLimits {
                max_evaluations: cap,
                ..Default::default()
            },
            phases: vec![
                SolverPhase::Construction(Default::default()),
                local.clone(),
                last.clone(),
            ],
        })
    })
    .collect()
}
#[test]
fn plans_match_ordinary_backends_and_report_actual_callbacks() {
    for c in plans() {
        let p = basic();
        let calls = Cell::new(0);
        let n = bn(&calls);
        let expected = solve(&p, &n, c.clone()).unwrap();
        let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        let before = calls.get();
        let actual = d.run_solver(c.clone()).unwrap();
        assert_eq!(actual.work.constraint_evaluations, calls.get() - before);
        assert_eq!(d.assignment(), &actual.assignment);
        if let Some(s) = actual.score {
            assert_eq!(d.score(), s);
        }
        assert_eq!(normalize(actual), normalize(expected));
        d.assert_consistent().unwrap();
        let p = lists();
        let calls = Cell::new(0);
        let n = ln(&calls);
        let expected = solve_list(&p, &n, c.clone()).unwrap();
        let mut d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        let before = calls.get();
        let actual = d.run_solver(c.clone()).unwrap();
        assert_eq!(actual.work.constraint_evaluations, calls.get() - before);
        assert_eq!(d.assignment(), &actual.assignment);
        if let Some(s) = actual.score {
            assert_eq!(d.score(), s);
        }
        assert_eq!(normalize(actual), normalize(expected));
        d.assert_consistent().unwrap();
        let p = MixedProblem::new(basic(), lists());
        let calls = Cell::new(0);
        let n = mn(&calls);
        let expected = solve_mixed(&p, &n, mixed(c.clone())).unwrap();
        let mut d = MixedScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        let before = calls.get();
        let actual = d.run_solver(mixed(c)).unwrap();
        assert_eq!(actual.work.constraint_evaluations, calls.get() - before);
        assert_eq!(d.assignment(), &actual.assignment);
        if let Some(s) = actual.score {
            assert_eq!(d.score(), s);
        }
        assert_eq!(normalize(actual), normalize(expected));
        d.assert_consistent().unwrap();
    }
}
fn exact(strategy: ExactSearchStrategy) -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![SolverPhase::Exact(ExactPhaseConfig {
            strategy,
            ..Default::default()
        })],
        ..Default::default()
    }
}
#[test]
fn exact_enumeration_reuses_sparse_contributions_instead_of_rescoring_the_network() {
    const N: usize = 12;
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]); N]).unwrap();
    let calls = Cell::new(0);
    let mut n = ConstraintNetwork::new(N);
    for i in 0..N {
        n.add_constraint(i.to_string(), vec![i], |v| {
            calls.set(calls.get() + 1);
            SimpleScore::new(-v[0].copied().unwrap_or(0))
        })
        .unwrap();
    }
    n.add_weighted_constraint("disabled", vec![0], SimpleScore::ZERO, |_| {
        panic!("disabled")
    })
    .unwrap();
    let c = exact(ExactSearchStrategy::Enumerate);
    let expected = solve(&p, &n, c.clone()).unwrap();
    let full = calls.get();
    let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let before = calls.get();
    let actual = d.run_solver(c).unwrap();
    let incremental = calls.get() - before;
    assert_eq!(normalize(actual.clone()), normalize(expected));
    assert_eq!(actual.work.evaluations, 1 << N);
    assert_eq!(full, (N as u128) * (1 << N));
    assert_eq!(incremental, (1 << (N + 1)) - 2); // First full score + actual changed binary digits.
    assert_eq!(actual.work.constraint_evaluations, incremental);
    assert_eq!(d.score(), SimpleScore::ZERO);
}
#[derive(Clone, Debug)]
struct Value {
    id: i64,
    payload: i64,
}
impl PartialEq for Value {
    fn eq(&self, o: &Self) -> bool {
        self.id == o.id
    }
}
impl Eq for Value {}
fn v(id: i64, payload: i64) -> Value {
    Value { id, payload }
}
#[test]
fn identity_equal_initial_payloads_refresh_at_exact_entry_and_prior_best_restores() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![v(0, 1), v(1, 2)]).with_initial(v(0, 1)),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::<Value, SimpleScore>::new(1);
    n.add_constraint("payload", vec![0], |values| {
        SimpleScore::new(values[0].map_or(0, |v| v.payload))
    })
    .unwrap();
    for strategy in [
        ExactSearchStrategy::Enumerate,
        ExactSearchStrategy::Complete,
        ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig::default()),
    ] {
        let initial = Assignment::new(vec![Some(v(0, 100))]);
        let mut d = ScoreDirector::new(&p, &n, initial.clone()).unwrap();
        let r = d.run_solver(exact(strategy.clone())).unwrap();
        let expected = if strategy == ExactSearchStrategy::Complete {
            100
        } else {
            2
        };
        assert_eq!(r.score, Some(SimpleScore::new(expected)));
        assert_eq!(
            d.assignment().values()[0].as_ref().unwrap().payload,
            expected
        );
        d.assert_consistent().unwrap();
        let mut d = ScoreDirector::new(&p, &n, initial).unwrap();
        let mut c = exact(strategy);
        c.phases.insert(0, SolverPhase::Custom(Default::default()));
        let mut command = |_: usize,
                           c: &mut CustomPhaseContext<
            '_,
            Assignment<Value>,
            Move<Value>,
            SimpleScore,
            ScoringError,
        >| {
            // Same identity, new scoring data; this is a real scored move.
            c.execute(&Move::Change {
                variable: 0,
                value: Some(v(0, 200)),
            })?;
            Ok(())
        };
        let r = d
            .run_solver_with_extensions(
                c,
                SolverExtensions {
                    command: Some(&mut command),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(r.score, Some(SimpleScore::new(200)));
        assert_eq!(d.assignment().values()[0].as_ref().unwrap().payload, 200);
        d.assert_consistent().unwrap();
    }
}
#[test]
fn list_and_mixed_exact_refresh_identity_equal_payloads() {
    let p = ListProblem::new(vec![v(0, 1), v(1, 2)], vec![ListEntity::new(vec![])]).unwrap();
    let mut n = ListConstraintNetwork::<Value, SimpleScore>::new(1, 2);
    n.add_constraint("payload", vec![ListScope::Entity(0)], |f| {
        let ListFact::Entity(v) = f[0] else {
            unreachable!()
        };
        SimpleScore::new(v.iter().map(|v| v.payload).sum())
    })
    .unwrap();
    let initial = ListAssignment::new(vec![vec![v(0, 100), v(1, 200)]]);
    for strategy in [
        ExactSearchStrategy::Enumerate,
        ExactSearchStrategy::Complete,
    ] {
        let mut d = ListScoreDirector::new(&p, &n, initial.clone()).unwrap();
        let r = d.run_solver(exact(strategy.clone())).unwrap();
        assert_eq!(
            r.score,
            Some(SimpleScore::new(
                if strategy == ExactSearchStrategy::Complete {
                    300
                } else {
                    3
                }
            ))
        );
        assert_eq!(
            d.assignment().lists()[0]
                .iter()
                .map(|v| v.payload)
                .sum::<i64>(),
            r.score.unwrap().score()
        );
        d.assert_consistent().unwrap();
        let bp =
            FiniteProblem::new(vec![Variable::new(vec![v(0, 4)]).with_initial(v(0, 4))]).unwrap();
        let p = MixedProblem::new(bp, p.clone());
        let mut n = MixedConstraintNetwork::<Value, Value, SimpleScore>::new(1, 1, 2);
        n.add_constraint(
            "payload",
            vec![MixedScope::Basic(0), MixedScope::ListEntity(0)],
            |f| {
                let (MixedFact::Basic(b), MixedFact::ListEntity(l)) = (&f[0], &f[1]) else {
                    unreachable!()
                };
                SimpleScore::new(
                    b.map_or(0, |v| v.payload) + l.iter().map(|v| v.payload).sum::<i64>(),
                )
            },
        )
        .unwrap();
        let a = MixedAssignment::new(Assignment::new(vec![Some(v(0, 400))]), initial.clone());
        let mut d = MixedScoreDirector::new(&p, &n, a).unwrap();
        let r = d.run_solver(mixed(exact(strategy.clone()))).unwrap();
        assert_eq!(
            r.score,
            Some(SimpleScore::new(
                if strategy == ExactSearchStrategy::Complete {
                    700
                } else {
                    7
                }
            ))
        );
        d.assert_consistent().unwrap();
    }
}
#[test]
fn failed_exact_transitions_leave_the_last_committed_assignment_and_cache() {
    for panic in [false, true] {
        let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]).with_initial(0)]).unwrap();
        let mut n = ConstraintNetwork::new(1);
        n.add_constraint("constant", vec![], |_| SimpleScore::new(i64::MAX))
            .unwrap();
        n.add_constraint("variable", vec![0], |v| {
            let x = v[0].copied().unwrap_or(0);
            if panic && x == 1 {
                panic!("score callback");
            }
            SimpleScore::new(x)
        })
        .unwrap();
        let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        let r = catch_unwind(AssertUnwindSafe(|| {
            d.run_solver(exact(ExactSearchStrategy::Enumerate))
        }));
        if panic {
            assert!(r.is_err());
        } else {
            assert!(r.unwrap().is_err());
        }
        assert_eq!(d.assignment().values(), &[Some(0)]);
        assert_eq!(d.score(), SimpleScore::new(i64::MAX));
        d.assert_consistent().unwrap();
    }
}

#[test]
fn rejected_repairs_restore_unselected_identity_equal_payloads() {
    // Canonical reconstruction loses 99 points. Every repair is rejected, and
    // subsequent attempts may ruin a different variable while restoring this one.
    let p = FiniteProblem::new(vec![
        Variable::new(vec![v(0, 1)]).with_initial(v(0, 100)),
        Variable::new(vec![v(1, 1)]).with_initial(v(1, 100)),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::<Value, SimpleScore>::new(2);
    for i in 0..2 {
        n.add_constraint(format!("payload{i}"), vec![i], |values| {
            SimpleScore::new(values[0].map_or(0, |v| v.payload))
        })
        .unwrap();
    }
    for seed in 0..8 {
        let config = SolverSequence {
            phases: vec![SolverPhase::RuinRecreate(RuinRecreateConfig {
                minimum_ruined: 1,
                maximum_ruined: 1,
                max_attempts: 8,
                seed,
                assert_scores: true,
                ..Default::default()
            })],
            ..Default::default()
        };
        let expected = solve(&p, &n, config.clone()).unwrap();
        let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        let actual = d.run_solver(config).unwrap();
        assert_eq!(normalize(actual), normalize(expected));
        assert_eq!(d.score(), SimpleScore::new(200));
        assert!(
            d.assignment()
                .values()
                .iter()
                .all(|v| v.as_ref().unwrap().payload == 100)
        );
        d.assert_consistent().unwrap();
    }
}

#[test]
fn dense_indexed_scopes_keep_pinned_neighbor_contributions_consistent() {
    // Two movable values leave a small exhaustive space despite the dense scope.
    let p = ListProblem::new_indexed(
        (0..129).collect::<Vec<i64>>(),
        vec![
            ListEntity::new((0..127).collect()).with_pinned_prefix(127),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let calls = Cell::new(0u128);
    let mut n = ListConstraintNetwork::new(2, 129);
    for i in 0..129 {
        n.add_constraint(format!("position{i}"), vec![ListScope::Value(i)], |facts| {
            calls.set(calls.get() + 1);
            let ListFact::Value { element, .. } = &facts[0] else {
                unreachable!()
            };
            SimpleScore::new(element.as_ref().map_or(0, |e| {
                e.previous.copied().unwrap_or(-1) - e.next.copied().unwrap_or(-1)
            }))
        })
        .unwrap();
    }
    for cap in [1, 2, 3, 8, 30] {
        let mut config = exact(ExactSearchStrategy::Enumerate);
        config.limits.max_evaluations = cap;
        let expected = solve_list(&p, &n, config.clone()).unwrap();
        let mut d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        let before = calls.get();
        let actual = d.run_solver(config).unwrap();
        assert_eq!(actual.work.constraint_evaluations, calls.get() - before);
        assert_eq!(normalize(actual), normalize(expected));
        d.assert_consistent().unwrap();
    }
}
