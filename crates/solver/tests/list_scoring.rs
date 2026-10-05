//! Original incremental-list scoring regressions; no upstream coverage credit.
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::{
    BendableScore, HardSoftScore, ListAssignment, ListConstraintNetwork, ListEntity, ListFact,
    ListMove, ListPosition, ListProblem, ListScope, ListScoreDirector, ListScoringError,
    SimpleScore,
};
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn entity<'a>(fact: &ListFact<'a, i32>) -> &'a [i32] {
    match fact {
        ListFact::Entity(list) => list,
        _ => panic!("entity scope"),
    }
}
fn ordinary_network(n: usize, m: usize) -> ListConstraintNetwork<'static, i32, HardSoftScore> {
    let mut net = ListConstraintNetwork::new(m, n);
    net.add_constraint("constant", vec![], |facts| {
        assert!(facts.is_empty());
        HardSoftScore::new(0, 13)
    })
    .unwrap();
    for e in 0..m {
        net.add_constraint(
            format!("route {e}"),
            vec![ListScope::Entity(e)],
            move |facts| {
                let route = entity(&facts[0]);
                let soft = route
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| (e as i64 + 1) * (i as i64 + 1) * (i64::from(v) + 1))
                    .sum();
                HardSoftScore::new(-(route.len().saturating_sub(2) as i64), soft)
            },
        )
        .unwrap();
    }
    for v in 0..n {
        net.add_constraint(format!("element {v}"), vec![ListScope::Value(v)], |facts| {
            let ListFact::Value { value, element } = &facts[0] else {
                panic!("value scope")
            };
            let score = element.as_ref().map_or(-7, |s| {
                100 * s.position.entity as i64
                    + 10 * s.position.index as i64
                    + i64::from(s.previous.copied().unwrap_or(-1))
                    + 3 * i64::from(s.next.copied().unwrap_or(-1))
            });
            assert!(**value >= 0);
            HardSoftScore::new(0, score)
        })
        .unwrap();
    }
    net
}
// Independent direct formula over lists; does not use shadow queries/network.
fn reference(a: &ListAssignment<i32>, n: usize, optional: bool) -> HardSoftScore {
    let mut soft = 13;
    let mut hard = 0;
    let mut assigned = 0;
    for (e, list) in a.lists().iter().enumerate() {
        hard -= list.len().saturating_sub(2) as i64;
        for (i, &v) in list.iter().enumerate() {
            assigned += 1;
            soft += (e as i64 + 1) * (i as i64 + 1) * (i64::from(v) + 1);
            soft += 100 * e as i64 + 10 * i as i64;
            soft += if i == 0 { -1 } else { i64::from(list[i - 1]) };
            soft += 3 * if i + 1 == list.len() {
                -1
            } else {
                i64::from(list[i + 1])
            };
        }
    }
    soft -= 7 * (n - assigned) as i64;
    HardSoftScore::new(hard, soft).with_structural(if optional {
        0
    } else {
        -((n - assigned) as i64)
    })
}
#[test]
fn seeded_composites_commit_and_reject_match_independent_full_scoring() {
    const N: usize = 7;
    const M: usize = 4;
    let p = ListProblem::new(
        (0..N as i32).collect(),
        (0..M).map(|_| ListEntity::new(vec![])).collect(),
    )
    .unwrap()
    .allowing_unassigned();
    let network = ordinary_network(N, M);
    let mut d = ListScoreDirector::new(&p, &network, p.initial_assignment()).unwrap();
    let mut rng = 20261004_u64;
    let mut pick = |max: usize| {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (rng >> 32) as usize % max
    };
    for step in 0..1200 {
        let original = d.assignment().clone();
        let score = d.score();
        let mut working = original.clone();
        let mut movements = vec![];
        for _ in 0..=pick(3) {
            let positions: Vec<_> = working
                .lists()
                .iter()
                .enumerate()
                .flat_map(|(e, l)| (0..l.len()).map(move |i| pos(e, i)))
                .collect();
            let missing: Vec<_> = (0..N as i32)
                .filter(|v| working.location(v).is_none())
                .collect();
            let movement = match pick(4) {
                0 if !missing.is_empty() => {
                    let e = pick(M);
                    ListMove::Assign {
                        value: missing[pick(missing.len())],
                        destination: pos(e, pick(working.lists()[e].len() + 1)),
                    }
                }
                1 if !positions.is_empty() => {
                    let source = positions[pick(positions.len())];
                    let e = pick(M);
                    let limit = working.lists()[e].len() + 1 - usize::from(e == source.entity);
                    ListMove::Change {
                        source,
                        destination: pos(e, pick(limit)),
                    }
                }
                2 if !positions.is_empty() => ListMove::Swap {
                    left: positions[pick(positions.len())],
                    right: positions[pick(positions.len())],
                },
                _ => {
                    let e = pick(M);
                    let start = pick(working.lists()[e].len() + 1);
                    let end = start + pick(working.lists()[e].len() - start + 1);
                    ListMove::Unassign {
                        entity: e,
                        start,
                        end,
                    }
                }
            };
            p.apply(&mut working, &movement).unwrap().commit();
            movements.push(movement);
        }
        let movement = ListMove::Composite(movements);
        let expected = reference(&working, N, true);
        let candidate = d.apply(&movement).unwrap();
        assert_eq!(candidate.assignment(), &working);
        assert_eq!(candidate.score(), expected);
        assert_eq!(candidate.score(), network.calculate(&p, &working).unwrap());
        if step % 3 != 0 {
            candidate.commit();
            assert_eq!(d.score(), expected);
        } else {
            drop(candidate);
            assert_eq!(d.assignment(), &original);
            assert_eq!(d.score(), score);
        }
        d.assert_consistent().unwrap();
    }
}
#[test]
fn dependency_scopes_invalidate_neighbors_and_deduplicate_repeated_scopes() {
    let p = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![
            ListEntity::new(vec![0, 1]),
            ListEntity::new(vec![2]),
            ListEntity::new(vec![3]),
        ],
    )
    .unwrap();
    let calls = RefCell::new(vec![0; 6]);
    let mut net = ListConstraintNetwork::new(3, 4);
    for (i, scope) in [
        vec![],
        vec![ListScope::Entity(0)],
        vec![ListScope::Value(0)],
        vec![ListScope::Value(1)],
        vec![ListScope::Entity(2), ListScope::Value(3)],
        vec![
            ListScope::Entity(0),
            ListScope::Value(0),
            ListScope::Entity(0),
        ],
    ]
    .into_iter()
    .enumerate()
    {
        let calls = &calls;
        net.add_constraint(format!("c{i}"), scope, move |facts| {
            calls.borrow_mut()[i] += 1;
            if i == 5 {
                assert_eq!(entity(&facts[0]), entity(&facts[2]));
            }
            SimpleScore::ZERO
        })
        .unwrap();
    }
    let mut d = ListScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
    assert_eq!(*calls.borrow(), vec![1; 6]);
    let candidate = d
        .apply(&ListMove::Change {
            source: pos(0, 0),
            destination: pos(1, 1),
        })
        .unwrap();
    assert_eq!(candidate.constraint_evaluations(), 4);
    assert_eq!(*calls.borrow(), vec![1, 2, 2, 2, 1, 2]);
    candidate.commit();
    assert_eq!(*calls.borrow(), vec![1, 2, 2, 2, 1, 2]);
    let before = calls.borrow().clone();
    let candidate = d.apply(&ListMove::Composite(vec![])).unwrap();
    assert_eq!(candidate.constraint_evaluations(), 0);
    drop(candidate);
    assert_eq!(*calls.borrow(), before);
}
#[test]
fn required_missing_values_and_optional_values_have_correct_structure() {
    let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![])]).unwrap();
    let net = ordinary_network(3, 1);
    let mut d = ListScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
    assert_eq!(d.score(), reference(d.assignment(), 3, false));
    assert_eq!(d.score().structural(), -3);
    for v in 0..3 {
        let candidate = d
            .apply(&ListMove::Assign {
                value: v,
                destination: pos(0, v as usize),
            })
            .unwrap();
        assert_eq!(
            candidate.score(),
            reference(candidate.assignment(), 3, false)
        );
        candidate.commit();
    }
    assert_eq!(d.score().structural(), 0);
    d.assert_consistent().unwrap();
    let candidate = d
        .apply(&ListMove::Unassign {
            entity: 0,
            start: 0,
            end: 2,
        })
        .unwrap();
    assert_eq!(candidate.score().structural(), -2);
    drop(candidate);
    assert_eq!(d.score().structural(), 0);
    let optional = p.allowing_unassigned();
    let d = ListScoreDirector::new(&optional, &net, optional.initial_assignment()).unwrap();
    assert_eq!(d.score().structural(), 0);
    assert_eq!(d.score(), reference(d.assignment(), 3, true));
}
#[test]
fn invalid_model_shapes_scopes_and_names_fail_before_registration() {
    let p = ListProblem::new(vec![0], vec![ListEntity::new(vec![])]).unwrap();
    let mut net = ListConstraintNetwork::<i32, SimpleScore>::new(1, 1);
    for scope in [
        ListScope::Entity(1),
        ListScope::Value(1),
        ListScope::Value(usize::MAX),
    ] {
        assert_eq!(
            net.add_constraint("name", vec![scope], |_| SimpleScore::ZERO),
            Err(ListScoringError::InvalidScope(scope))
        );
        assert_eq!(net.constraint_count(), 0);
    }
    net.add_constraint("name", vec![], |_| SimpleScore::ZERO)
        .unwrap();
    assert_eq!(
        net.add_constraint("name", vec![], |_| SimpleScore::ZERO),
        Err(ListScoringError::DuplicateName("name".into()))
    );
    assert_eq!(net.constraint_count(), 1);
    for (e, v) in [(0, 1), (1, 0), (2, 2)] {
        let wrong = ListConstraintNetwork::<i32, SimpleScore>::new(e, v);
        assert!(matches!(
            wrong.calculate(&p, &p.initial_assignment()),
            Err(ListScoringError::ModelShape { .. })
        ));
        assert!(matches!(
            ListScoreDirector::new(&p, &wrong, p.initial_assignment()),
            Err(ListScoringError::ModelShape { .. })
        ));
    }
    assert!(matches!(
        net.calculate(&p, &ListAssignment::new(vec![vec![0, 0]])),
        Err(ListScoringError::Model(_))
    ));
}
#[test]
fn scoring_errors_and_panics_leave_lists_and_all_caches_unchanged() {
    for should_panic in [false, true] {
        let p = ListProblem::new(
            vec![0, 1],
            vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![])],
        )
        .unwrap();
        let mut net = ListConstraintNetwork::new(2, 2);
        net.add_constraint("earlier contribution", vec![ListScope::Entity(0)], |f| {
            SimpleScore::new(entity(&f[0]).len() as i64)
        })
        .unwrap();
        net.add_constraint("failure", vec![ListScope::Value(0)], move |f| {
            let ListFact::Value { element, .. } = &f[0] else {
                unreachable!()
            };
            if element.as_ref().unwrap().position.entity == 1 {
                if should_panic {
                    panic!("callback fault");
                }
                return SimpleScore::ZERO.with_structural(1);
            }
            SimpleScore::new(10)
        })
        .unwrap();
        let mut d = ListScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
        let original = d.assignment().clone();
        let score = d.score();
        let cache: Vec<_> = d
            .constraint_scores()
            .map(|(n, s)| (n.to_owned(), s))
            .collect();
        let result = catch_unwind(AssertUnwindSafe(|| {
            d.apply(&ListMove::Change {
                source: pos(0, 0),
                destination: pos(1, 0),
            })
            .map(drop)
        }));
        if should_panic {
            assert!(result.is_err());
        } else {
            assert_eq!(
                result.unwrap(),
                Err(ListScoringError::StructuralContribution { constraint: 1 })
            );
        }
        assert_eq!(d.assignment(), &original);
        assert_eq!(d.score(), score);
        assert_eq!(
            d.constraint_scores()
                .map(|(n, s)| (n.to_owned(), s))
                .collect::<Vec<_>>(),
            cache
        );
        d.assert_consistent().unwrap();
        assert!(matches!(
            d.apply(&ListMove::Swap {
                left: pos(0, 0),
                right: pos(2, 0)
            }),
            Err(ListScoringError::Model(_))
        ));
        assert_eq!(d.assignment(), &original);
    }
}
#[test]
fn diagnostics_detect_cancelling_contribution_corruption() {
    let p = ListProblem::new(vec![0], vec![ListEntity::new(vec![0])]).unwrap();
    let offset = Cell::new(0);
    let mut net = ListConstraintNetwork::new(1, 1);
    net.add_constraint("positive", vec![], |_| SimpleScore::new(offset.get()))
        .unwrap();
    net.add_constraint("negative", vec![], |_| SimpleScore::new(-offset.get()))
        .unwrap();
    let d = ListScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
    offset.set(10);
    assert_eq!(net.calculate(&p, d.assignment()).unwrap(), d.score());
    assert_eq!(
        d.assert_consistent(),
        Err(ListScoringError::CorruptedConstraint { constraint: 0 })
    );
    offset.set(0);
    d.assert_consistent().unwrap();
}
#[test]
fn bendable_levels_and_wrapping_totals_survive_commit_and_undo() {
    type S = BendableScore<2, 2>;
    let p = ListProblem::new(vec![0], vec![ListEntity::new(vec![])]).unwrap();
    let mut net =
        ListConstraintNetwork::new(1, 1).with_arithmetic(timetuck::ScoringArithmetic::Wrapping);
    net.add_constraint("constant", vec![], |_| {
        S::new([i64::MAX, i64::MIN], [i64::MAX, i64::MIN])
    })
    .unwrap();
    net.add_constraint("element", vec![ListScope::Value(0)], |f| {
        let ListFact::Value { element, .. } = &f[0] else {
            unreachable!()
        };
        if element.is_some() {
            S::new([1, -1], [2, -2])
        } else {
            S::ZERO
        }
    })
    .unwrap();
    let mut d = ListScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
    let initial = d.score();
    {
        let c = d
            .apply(&ListMove::Assign {
                value: 0,
                destination: pos(0, 0),
            })
            .unwrap();
        assert_eq!(
            c.score(),
            S::new([i64::MIN, i64::MAX], [i64::MIN + 1, i64::MAX - 1])
        );
    }
    assert_eq!(d.score(), initial);
    d.apply(&ListMove::Assign {
        value: 0,
        destination: pos(0, 0),
    })
    .unwrap()
    .commit();
    d.assert_consistent().unwrap();
    d.apply(&ListMove::Unassign {
        entity: 0,
        start: 0,
        end: 1,
    })
    .unwrap()
    .commit();
    assert_eq!(d.score(), initial);
    d.assert_consistent().unwrap();
}
#[test]
fn pin_errors_and_composite_repeated_touches_preserve_cache_consistency() {
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![
            ListEntity::new(vec![0, 1]).with_pinned_prefix(1),
            ListEntity::new(vec![2]).with_pinned(true),
        ],
    )
    .unwrap();
    let net = ordinary_network(3, 2);
    let mut d = ListScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
    let original = d.assignment().clone();
    let initial = d.score();
    assert!(matches!(
        d.apply(&ListMove::Unassign {
            entity: 0,
            start: 0,
            end: 1
        }),
        Err(ListScoringError::Model(_))
    ));
    assert_eq!(d.score(), initial);
    assert_eq!(d.assignment(), &original);
    let c = d
        .apply(&ListMove::Composite(vec![
            ListMove::Unassign {
                entity: 0,
                start: 0,
                end: 1,
            },
            ListMove::Assign {
                value: 0,
                destination: pos(0, 0),
            },
        ]))
        .unwrap();
    assert_eq!(c.score(), initial);
    assert_eq!(c.constraint_evaluations(), 3);
    c.commit();
    d.assert_consistent().unwrap();
}

#[test]
fn mixed_scopes_preserve_order_repetitions_and_non_dense_value_identity() {
    let p = ListProblem::new(
        vec![7, 2, 9],
        vec![ListEntity::new(vec![7]), ListEntity::new(vec![9])],
    )
    .unwrap();
    let mut net = ListConstraintNetwork::new(2, 3);
    net.add_constraint(
        "mixed",
        vec![
            ListScope::Value(2),
            ListScope::Entity(1),
            ListScope::Value(0),
            ListScope::Value(2),
        ],
        |f| {
            let ListFact::Value {
                value: a,
                element: sa,
            } = &f[0]
            else {
                panic!("first value")
            };
            let ListFact::Value { value: b, .. } = &f[2] else {
                panic!("second value")
            };
            let ListFact::Value {
                value: c,
                element: sc,
            } = &f[3]
            else {
                panic!("repeat value")
            };
            assert_eq!((**a, **b, **c), (9, 7, 9));
            assert_eq!(sa, sc);
            let next = sa.as_ref().unwrap().next.copied().unwrap_or(0);
            SimpleScore::new(i64::from(**a + **b + **c + next) + entity(&f[1]).len() as i64)
        },
    )
    .unwrap();
    let mut d = ListScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
    assert_eq!(d.score(), SimpleScore::new(26).with_structural(-1));
    let c = d
        .apply(&ListMove::Assign {
            value: 2,
            destination: pos(1, 1),
        })
        .unwrap();
    assert_eq!(c.constraint_evaluations(), 1);
    assert_eq!(c.score(), SimpleScore::new(29));
    c.commit();
    d.assert_consistent().unwrap();
}
