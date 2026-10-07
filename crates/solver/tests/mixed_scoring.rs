//! Original mixed scoring regressions; no upstream coverage credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
const VALUES: [i32; 4] = [11, 23, 47, 89];
fn basic(fact: &MixedFact<'_, i64, i32>) -> i64 {
    let MixedFact::Basic(value) = fact else {
        panic!("basic fact")
    };
    value.copied().unwrap_or(-3)
}
fn list<'a>(fact: &MixedFact<'a, i64, i32>) -> &'a [i32] {
    let MixedFact::ListEntity(list) = fact else {
        panic!("list fact")
    };
    list
}
fn problem(optional: bool) -> MixedProblem<i64, i32> {
    let mut lists = ListProblem::new(
        VALUES.to_vec(),
        vec![
            ListEntity::new(vec![11]).with_pinned_prefix(1),
            ListEntity::new(vec![]),
            ListEntity::new(vec![89]).with_pinned(true),
        ],
    )
    .unwrap();
    if optional {
        lists = lists.allowing_unassigned();
    }
    MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![0, 1, 2]),
            Variable::new(vec![0, 1, 2]).allowing_unassigned(),
            Variable::new(vec![2]).with_initial(2).with_pinned(true),
        ])
        .unwrap(),
        lists,
    )
}
fn network() -> MixedConstraintNetwork<'static, i64, i32, HardSoftScore> {
    let mut network = MixedConstraintNetwork::new(3, 3, 4);
    network
        .add_constraint("constant", vec![], |_| HardSoftScore::new(0, 7))
        .unwrap();
    for e in 0..3 {
        network
            .add_constraint(
                format!("route {e}"),
                vec![MixedScope::Basic(e), MixedScope::ListEntity(e)],
                |facts| {
                    let values = list(&facts[1]);
                    let soft = values
                        .iter()
                        .enumerate()
                        .map(|(i, &v)| i64::from(v) * (basic(&facts[0]) + 1) * (i as i64 + 1))
                        .sum();
                    HardSoftScore::new(-(values.len().saturating_sub(2) as i64), soft)
                },
            )
            .unwrap();
    }
    for i in 0..4 {
        network
            .add_constraint(
                format!("value {i}"),
                vec![
                    MixedScope::Basic(0),
                    MixedScope::Basic(1),
                    MixedScope::Basic(2),
                    MixedScope::ListValue(i),
                ],
                |facts| {
                    let MixedFact::ListValue { value, element } = &facts[3] else {
                        panic!("value fact")
                    };
                    assert!(VALUES.contains(value));
                    HardSoftScore::new(
                        0,
                        element.as_ref().map_or(-17, |view| {
                            100 * basic(&facts[view.position.entity])
                                + 30 * view.position.entity as i64
                                + 7 * view.position.index as i64
                                + i64::from(view.previous.copied().unwrap_or(0))
                                - i64::from(view.next.copied().unwrap_or(0))
                        }),
                    )
                },
            )
            .unwrap();
    }
    network
}
// Independent direct formula: no network callbacks or derived element queries.
fn reference(state: &MixedAssignment<i64, i32>, optional: bool) -> HardSoftScore {
    let mut soft = 7;
    let mut hard = 0;
    let mut assigned = 0;
    for (e, route) in state.lists().lists().iter().enumerate() {
        let b = state.basic().values()[e].unwrap_or(-3);
        hard -= route.len().saturating_sub(2) as i64;
        for (i, &value) in route.iter().enumerate() {
            assigned += 1;
            soft += i64::from(value) * (b + 1) * (i as i64 + 1)
                + 100 * b
                + 30 * e as i64
                + 7 * i as i64;
            if i > 0 {
                soft += i64::from(route[i - 1]);
            }
            if i + 1 < route.len() {
                soft -= i64::from(route[i + 1]);
            }
        }
    }
    soft -= 17 * (4 - assigned);
    let missing_basic = i64::from(state.basic().values()[0].is_none());
    HardSoftScore::new(hard, soft)
        .with_structural(-missing_basic - if optional { 0 } else { 4 - assigned })
}
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn no_list() -> ListMove<i32> {
    ListMove::Composite(vec![])
}
fn no_basic() -> Move<i64> {
    Move::Composite(vec![])
}

#[test]
fn seeded_joint_composites_match_independent_formula_after_commit_and_undo() {
    let mut seed = 20261104_u64;
    let mut pick = |max: usize| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((seed >> 32) as usize) % max
    };
    for optional in [false, true] {
        let problem = problem(optional);
        let network = network();
        let mut director =
            MixedScoreDirector::new(&problem, &network, problem.initial_assignment()).unwrap();
        for step in 0..600 {
            let original = director.assignment().clone();
            let old_score = director.score();
            let old_parts = director
                .constraint_scores()
                .map(|(name, s)| (name.to_owned(), s))
                .collect::<Vec<_>>();
            let variable = pick(2);
            let value = match pick(4) {
                3 => None,
                n => Some(n as i64),
            };
            let mut movement = MixedMove {
                basic: Move::Composite(vec![
                    Move::Change {
                        variable,
                        value: Some(2),
                    },
                    Move::Change { variable, value },
                ]),
                lists: no_list(),
            };
            // List-only rounds must prove value/neighbor invalidation without
            // basic dependencies causing every value constraint to rerun.
            if step % 3 == 0 {
                movement.basic = no_basic();
            }
            let mut working = original.clone();
            problem.apply(&mut working, &movement).unwrap().commit();
            let mut list_steps = vec![];
            for _ in 0..1 + step % 3 {
                let mut moves = list_moves(
                    problem.lists(),
                    working.lists(),
                    Neighborhood::ChangeAndSwap,
                )
                .unwrap()
                .collect::<Vec<_>>();
                // Partial required states are legal even though complete-search
                // neighborhoods intentionally do not unassign required values.
                for e in 0..2 {
                    let start = problem.lists().entities()[e].pinned_prefix();
                    if start < working.lists().lists()[e].len() {
                        moves.push(ListMove::Unassign {
                            entity: e,
                            start,
                            end: working.lists().lists()[e].len(),
                        });
                    }
                }
                moves.push(no_list());
                let next = moves.swap_remove(pick(moves.len()));
                problem
                    .apply(
                        &mut working,
                        &MixedMove {
                            basic: no_basic(),
                            lists: next.clone(),
                        },
                    )
                    .unwrap()
                    .commit();
                list_steps.push(next);
            }
            movement.lists = ListMove::Composite(list_steps);
            let trial = director.apply(&movement).unwrap();
            assert_eq!(trial.assignment(), &working);
            assert_eq!(trial.score(), reference(&working, optional));
            assert_eq!(
                trial.score(),
                network.calculate(&problem, &working).unwrap()
            );
            if pick(3) != 0 {
                trial.commit();
            } else {
                drop(trial);
                assert_eq!(director.assignment(), &original);
                assert_eq!(director.score(), old_score);
                assert_eq!(
                    director
                        .constraint_scores()
                        .map(|(n, s)| (n.to_owned(), s))
                        .collect::<Vec<_>>(),
                    old_parts
                );
            }
            assert_eq!(director.score(), reference(director.assignment(), optional));
            director.assert_consistent().unwrap();
        }
    }
}

#[test]
fn scopes_union_once_preserve_order_and_leave_unrelated_callbacks_cached() {
    let problem = problem(false);
    let calls: Vec<_> = (0..5).map(|_| Cell::new(0)).collect();
    let mut network = MixedConstraintNetwork::new(3, 3, 4);
    let scopes = vec![
        vec![
            MixedScope::Basic(0),
            MixedScope::ListEntity(0),
            MixedScope::Basic(0),
            MixedScope::ListValue(0),
        ],
        vec![MixedScope::Basic(1)],
        vec![MixedScope::ListEntity(2)],
        vec![MixedScope::ListValue(1)],
        vec![],
    ];
    for (i, scope) in scopes.into_iter().enumerate() {
        let calls = &calls;
        network
            .add_constraint(format!("c{i}"), scope, move |facts| {
                calls[i].set(calls[i].get() + 1);
                if i == 0 {
                    assert_eq!(basic(&facts[0]), basic(&facts[2]));
                    assert_eq!(list(&facts[1])[0], 11);
                    let MixedFact::ListValue { value, element } = &facts[3] else {
                        panic!()
                    };
                    assert_eq!(**value, 11);
                    assert_eq!(element.as_ref().unwrap().position, pos(0, 0));
                }
                SimpleScore::new(i as i64)
            })
            .unwrap();
    }
    let mut director =
        MixedScoreDirector::new(&problem, &network, problem.initial_assignment()).unwrap();
    for c in &calls {
        c.set(0);
    }
    let trial = director
        .apply(&MixedMove {
            basic: Move::Change {
                variable: 0,
                value: Some(1),
            },
            lists: ListMove::Assign {
                value: 23,
                destination: pos(0, 1),
            },
        })
        .unwrap();
    assert_eq!(trial.constraint_evaluations(), 2);
    assert_eq!(
        calls.iter().map(Cell::get).collect::<Vec<_>>(),
        vec![1, 0, 0, 1, 0]
    );
    trial.commit();
    assert_eq!(
        calls.iter().map(Cell::get).collect::<Vec<_>>(),
        vec![1, 0, 0, 1, 0]
    );
    let trial = director
        .apply(&MixedMove {
            basic: no_basic(),
            lists: no_list(),
        })
        .unwrap();
    assert_eq!(trial.constraint_evaluations(), 0);
    drop(trial);
    let trial = director
        .apply(&MixedMove {
            basic: Move::Change {
                variable: 0,
                value: Some(1),
            },
            lists: no_list(),
        })
        .unwrap();
    assert_eq!(trial.constraint_evaluations(), 1); // no-op write is a dependency touch
}

#[test]
fn error_and_panic_after_earlier_contribution_restore_assignment_and_cache() {
    let problem = problem(false);
    for panic in [false, true] {
        let mut network = MixedConstraintNetwork::new(3, 3, 4);
        network
            .add_constraint("first", vec![MixedScope::Basic(0)], |f| {
                SimpleScore::new(basic(&f[0]))
            })
            .unwrap();
        network
            .add_constraint("later", vec![MixedScope::ListEntity(1)], |f| {
                if !list(&f[0]).is_empty() {
                    if panic {
                        panic!("callback failure")
                    }
                    return SimpleScore::ZERO.with_structural(1);
                }
                SimpleScore::ZERO
            })
            .unwrap();
        let mut director =
            MixedScoreDirector::new(&problem, &network, problem.initial_assignment()).unwrap();
        let original = director.assignment().clone();
        let score = director.score();
        let parts = director
            .constraint_scores()
            .map(|(_, s)| s)
            .collect::<Vec<_>>();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            director
                .apply(&MixedMove {
                    basic: Move::Change {
                        variable: 0,
                        value: Some(1),
                    },
                    lists: ListMove::Assign {
                        value: 23,
                        destination: pos(1, 0),
                    },
                })
                .map(|trial| trial.commit())
        }));
        if panic {
            assert!(outcome.is_err())
        } else {
            assert_eq!(
                outcome.unwrap(),
                Err(MixedScoringError::StructuralContribution { constraint: 1 })
            );
        }
        assert_eq!(director.assignment(), &original);
        assert_eq!(director.score(), score);
        assert_eq!(
            director
                .constraint_scores()
                .map(|(_, s)| s)
                .collect::<Vec<_>>(),
            parts
        );
        director.assert_consistent().unwrap();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _trial = director
                    .apply(&MixedMove {
                        basic: Move::Change {
                            variable: 0,
                            value: Some(1),
                        },
                        lists: no_list(),
                    })
                    .unwrap();
                panic!("caller rejects by unwinding");
            }))
            .is_err()
        );
        assert_eq!(director.assignment(), &original);
        assert_eq!(director.score(), score);
        director.assert_consistent().unwrap();
    }
}

#[test]
fn invalid_scope_shape_assignment_and_move_fail_before_callbacks() {
    let problem = problem(false);
    let calls = Cell::new(0);
    let mut network = MixedConstraintNetwork::<i64, i32, SimpleScore>::new(3, 3, 4);
    for scope in [
        MixedScope::Basic(3),
        MixedScope::ListEntity(3),
        MixedScope::ListValue(4),
    ] {
        assert_eq!(
            network.add_constraint("bad", vec![MixedScope::Basic(0), scope], |_| panic!()),
            Err(MixedScoringError::InvalidScope(scope))
        );
    }
    assert_eq!(network.constraint_count(), 0);
    network
        .add_constraint("valid", vec![MixedScope::Basic(0)], |_| {
            calls.set(calls.get() + 1);
            SimpleScore::ZERO
        })
        .unwrap();
    assert_eq!(
        network.add_constraint("valid", vec![], |_| panic!()),
        Err(MixedScoringError::DuplicateName("valid".into()))
    );
    for dimensions in [[2, 3, 4], [3, 2, 4], [3, 3, 3]] {
        let mut wrong = MixedConstraintNetwork::<i64, i32, SimpleScore>::new(
            dimensions[0],
            dimensions[1],
            dimensions[2],
        );
        wrong.add_constraint("never", vec![], |_| panic!()).unwrap();
        assert!(matches!(
            wrong.calculate(&problem, &problem.initial_assignment()),
            Err(MixedScoringError::ModelShape { .. })
        ));
    }
    let invalid = MixedAssignment::new(
        Assignment::new(vec![]),
        problem.lists().initial_assignment(),
    );
    assert!(matches!(
        network.calculate(&problem, &invalid),
        Err(MixedScoringError::Model(_))
    ));
    assert_eq!(calls.get(), 0);
    let mut director =
        MixedScoreDirector::new(&problem, &network, problem.initial_assignment()).unwrap();
    calls.set(0);
    assert!(matches!(
        director.apply(&MixedMove {
            basic: Move::Change {
                variable: 0,
                value: Some(1)
            },
            lists: ListMove::Unassign {
                entity: 2,
                start: 0,
                end: 1
            }
        }),
        Err(MixedScoringError::Model(_))
    ));
    assert_eq!(calls.get(), 0);
    let trial = director
        .apply(&MixedMove {
            basic: Move::Change {
                variable: 0,
                value: Some(1),
            },
            lists: no_list(),
        })
        .unwrap();
    assert_eq!(trial.constraint_evaluations(), 1); // failed registration added no phantom dependency
    assert_eq!(calls.get(), 1);
}

#[test]
fn structure_combines_required_components_and_optional_unassigned_values() {
    for optional in [false, true] {
        let problem = problem(optional);
        let network = MixedConstraintNetwork::<i64, i32, SimpleScore>::new(3, 3, 4);
        let mut director =
            MixedScoreDirector::new(&problem, &network, problem.initial_assignment()).unwrap();
        assert_eq!(
            director.score().structural(),
            if optional { -1 } else { -3 }
        );
        let trial = director
            .apply(&MixedMove {
                basic: Move::Change {
                    variable: 0,
                    value: Some(0),
                },
                lists: ListMove::Composite(vec![
                    ListMove::Assign {
                        value: 23,
                        destination: pos(1, 0),
                    },
                    ListMove::Assign {
                        value: 47,
                        destination: pos(1, 1),
                    },
                ]),
            })
            .unwrap();
        assert_eq!(trial.constraint_evaluations(), 0);
        assert_eq!(trial.score(), SimpleScore::ZERO);
        trial.commit();
        assert!(problem.is_complete(director.assignment()));
        director.assert_consistent().unwrap();
    }
}

#[test]
fn diagnostics_detect_cancelling_corruption_and_bendable_wrapping_is_consistent() {
    let problem = problem(false);
    let changed = Cell::new(0);
    let mut network = MixedConstraintNetwork::new(3, 3, 4);
    for sign in [1, -1] {
        let changed = &changed;
        network
            .add_constraint(format!("sign {sign}"), vec![], move |_| {
                SimpleScore::new(sign * changed.get())
            })
            .unwrap();
    }
    let director =
        MixedScoreDirector::new(&problem, &network, problem.initial_assignment()).unwrap();
    changed.set(7); // Deliberately violate callback contract to test diagnostics.
    assert_eq!(
        network.calculate(&problem, director.assignment()).unwrap(),
        director.score()
    );
    let Err(MixedScoringError::ScoreCorruption(report)) = director.assert_consistent() else {
        panic!("expected corruption diagnostic")
    };
    assert_eq!(
        report
            .constraints
            .iter()
            .map(|c| c.constraint)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    changed.set(0);
    director.assert_consistent().unwrap();
    let mut network =
        MixedConstraintNetwork::new(3, 3, 4).with_arithmetic(timetuck::ScoringArithmetic::Wrapping);
    network
        .add_constraint("edge", vec![], |_| {
            BendableScore::new([i64::MAX], [i64::MIN, 9])
        })
        .unwrap();
    network
        .add_constraint(
            "joint",
            vec![MixedScope::Basic(0), MixedScope::ListEntity(1)],
            |f| {
                let value = basic(&f[0]) + list(&f[1]).len() as i64;
                BendableScore::new([value], [-value, 11])
            },
        )
        .unwrap();
    let mut director =
        MixedScoreDirector::new(&problem, &network, problem.initial_assignment()).unwrap();
    let trial = director
        .apply(&MixedMove {
            basic: Move::Change {
                variable: 0,
                value: Some(2),
            },
            lists: ListMove::Assign {
                value: 23,
                destination: pos(1, 0),
            },
        })
        .unwrap();
    assert_eq!(trial.score().hard_levels(), [i64::MIN + 2]);
    assert_eq!(trial.score().soft_levels(), [i64::MAX - 2, 20]);
    assert_eq!(trial.score().structural(), -1);
    assert_eq!(
        trial.score(),
        network.calculate(&problem, trial.assignment()).unwrap()
    );
    trial.commit();
    director.assert_consistent().unwrap();
}
