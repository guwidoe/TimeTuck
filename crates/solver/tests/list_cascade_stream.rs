//! Original scored cascades: independent schedule reference, undo and search.
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
type Matches = StreamConstraint<usize, (ListValueRow<i64>, DerivedValue<i64>)>;
fn problem() -> ListProblem<i64> {
    ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![3, 1]), ListEntity::new(vec![2])],
    )
    .unwrap()
}
fn arrival(c: ListCascadeStep<'_, i64, i64>, direction: ListCascadeDirection) -> Option<i64> {
    Some(
        (match direction {
            ListCascadeDirection::Previous => {
                assert!(c.next_shadow.is_none());
                c.previous_shadow
            }
            ListCascadeDirection::Next => {
                assert!(c.previous_shadow.is_none());
                c.next_shadow
            }
        })
        .copied()
        .unwrap_or(c.position.entity as i64 * 10)
            + *c.value,
    )
}
fn score(a: &ListAssignment<i64>, direction: ListCascadeDirection) -> SimpleScore {
    SimpleScore::new(
        -a.lists()
            .iter()
            .enumerate()
            .map(|(e, l)| {
                (0..l.len())
                    .map(|i| {
                        let segment = match direction {
                            ListCascadeDirection::Previous => &l[..=i],
                            ListCascadeDirection::Next => &l[i..],
                        };
                        e as i64 * 10 + segment.iter().sum::<i64>()
                    })
                    .sum::<i64>()
            })
            .sum::<i64>(),
    )
}

fn graph(
    p: &ListProblem<i64>,
    direction: ListCascadeDirection,
) -> (
    StreamGraph<'static, SimpleScore>,
    ListStreamSources<i64>,
    Matches,
) {
    let mut b = StreamGraphBuilder::new();
    let sources = ListStreamSources::new(&mut b);
    let cascade = list_cascade_stream_with_direction(&mut b, &sources, p, direction, move |c| {
        arrival(c, direction)
    })
    .unwrap();
    let matches = b
        .score("finish early", &cascade, |(_, v)| {
            SimpleScore::new(-v.value.unwrap_or(0))
        })
        .unwrap();
    (b.build(), sources, matches)
}
fn check(
    g: &StreamGraph<'_, SimpleScore>,
    m: &Matches,
    a: &ListAssignment<i64>,
    direction: ListCascadeDirection,
) {
    assert_eq!(g.score(), score(a, direction));
    let rows = g.constraint_matches(m).unwrap();
    assert_eq!(rows.len(), 3);
    for (k, v) in [1, 2, 3].iter().enumerate() {
        let (row, shadow) = &rows[&k].0;
        assert_eq!(&row.value, v);
        let element = a.element(v);
        assert_eq!(row.position, element.as_ref().map(|e| e.position));
        assert_eq!(
            row.previous,
            element.as_ref().and_then(|e| e.previous.copied())
        );
        assert_eq!(row.next, element.as_ref().and_then(|e| e.next.copied()));
        let expected = element.map(|e| {
            e.position.entity as i64 * 10
                + (match direction {
                    ListCascadeDirection::Previous => {
                        &a.lists()[e.position.entity][..=e.position.index]
                    }
                    ListCascadeDirection::Next => &a.lists()[e.position.entity][e.position.index..],
                })
                .iter()
                .sum::<i64>()
        });
        assert_eq!(shadow.value, expected);
        assert!(!shadow.inconsistent);
    }
}
fn replace(from: &ListAssignment<i64>, to: &ListAssignment<i64>) -> ListMove<i64> {
    let mut steps: Vec<_> = from
        .lists()
        .iter()
        .enumerate()
        .map(|(entity, l)| ListMove::Unassign {
            entity,
            start: 0,
            end: l.len(),
        })
        .collect();
    for (entity, l) in to.lists().iter().enumerate() {
        steps.extend(
            l.iter()
                .enumerate()
                .map(|(index, &value)| ListMove::Assign {
                    value,
                    destination: ListPosition { entity, index },
                }),
        );
    }
    ListMove::Composite(steps)
}
#[test]
fn all_partial_orders_commit_and_undo_against_independent_arrival_times() {
    for direction in [ListCascadeDirection::Previous, ListCascadeDirection::Next] {
        let p = problem().allowing_unassigned();
        let (g, s, m) = graph(&p, direction);
        let mut d = ListStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
        let orders = [
            [1, 2, 3],
            [1, 3, 2],
            [2, 1, 3],
            [2, 3, 1],
            [3, 1, 2],
            [3, 2, 1],
        ];
        for order in orders {
            for code in 0..27 {
                let mut placements = code;
                let mut lists = vec![vec![], vec![]];
                for value in order {
                    let owner = placements % 3;
                    placements /= 3;
                    if owner < 2 {
                        lists[owner].push(value);
                    }
                }
                let expected = ListAssignment::new(lists);
                let before = d.assignment().clone();
                let before_rows = d.graph().constraint_matches(&m).unwrap().clone();
                let movement = replace(&before, &expected);
                let t = d.apply(&movement).unwrap();
                check(t.graph(), &m, &expected, direction);
                if code % 3 == 0 {
                    t.commit();
                } else {
                    drop(t);
                    assert_eq!(d.assignment(), &before);
                    assert_eq!(d.graph().constraint_matches(&m).unwrap(), &before_rows);
                }
            }
        }
        d.assert_consistent().unwrap();
    }
}
#[test]
fn failures_and_nested_sessions_restore_shadows_without_supplier_replay() {
    for direction in [ListCascadeDirection::Previous, ListCascadeDirection::Next] {
        let p = problem();
        let calls = RefCell::new(vec![]);
        let supplier_panic = Cell::new(false);
        let score_panic = Cell::new(false);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = ListStreamSources::new(&mut b);
        let cascade = list_cascade_stream_with_direction(&mut b, &s, &p, direction, |c| {
            calls.borrow_mut().push(*c.value);
            assert!(!supplier_panic.get(), "supplier panic");
            arrival(c, direction)
        })
        .unwrap();
        let m = b
            .score("finish early", &cascade, |(_, v)| {
                assert!(!score_panic.get(), "score panic");
                SimpleScore::new(-v.value.unwrap_or(0))
            })
            .unwrap();
        let mut d = ListStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
        let movement = ListMove::Swap {
            left: ListPosition {
                entity: 0,
                index: 0,
            },
            right: ListPosition {
                entity: 0,
                index: 1,
            },
        };
        let original = d.graph().constraint_matches(&m).unwrap().clone();
        calls.borrow_mut().clear();
        {
            let t = d.apply(&movement).unwrap();
            check(t.graph(), &m, t.assignment(), direction);
            calls.borrow_mut().clear();
        }
        assert!(calls.borrow().is_empty(), "undo must not run suppliers");
        for trigger in [&supplier_panic, &score_panic] {
            trigger.set(true);
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _ = d.apply(&movement);
                }))
                .is_err()
            );
            trigger.set(false);
            assert_eq!(d.assignment(), &p.initial_assignment());
            assert_eq!(d.graph().constraint_matches(&m).unwrap(), &original);
        }
        calls.borrow_mut().clear();
        {
            let mut outer = d.edit_session().unwrap();
            outer.apply(&movement).unwrap();
            assert!(
                !calls.borrow().contains(&2),
                "untouched entity must not recalculate"
            );
            let outer_score = outer.score();
            {
                let mut inner = outer.nested().unwrap();
                inner.apply(&movement).unwrap();
            }
            assert_eq!(outer.score(), outer_score);
            {
                let mut inner = outer.nested().unwrap();
                inner.apply(&movement).unwrap();
                inner.commit();
            }
            assert_eq!(outer.score(), score(&p.initial_assignment(), direction));
            calls.borrow_mut().clear();
        }
        assert!(calls.borrow().is_empty());
        assert_eq!(d.graph().constraint_matches(&m).unwrap(), &original);
        // Reuse restored dependency indexes for a later committed move.
        d.apply(&movement).unwrap().commit();
        check(d.graph(), &m, d.assignment(), direction);
        d.assert_consistent().unwrap();
    }
}
#[test]
fn mixed_moves_update_basic_and_cascade_constraints_atomically() {
    for direction in [ListCascadeDirection::Previous, ListCascadeDirection::Next] {
        let p = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![0i64, 1]).with_initial(0)]).unwrap(),
            problem(),
        );
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = MixedStreamSources::new(&mut b);
        let cascade =
            list_cascade_stream_with_direction(&mut b, &s.lists, p.lists(), direction, move |c| {
                arrival(c, direction)
            })
            .unwrap();
        let m = b
            .score("finish early", &cascade, |(_, v)| {
                SimpleScore::new(-v.value.unwrap_or(0))
            })
            .unwrap();
        b.score("basic", s.basic.port(), |(_, v)| {
            SimpleScore::new(v.unwrap_or(0) * 100)
        })
        .unwrap();
        let mut d =
            MixedStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
        let old = d.graph().constraint_matches(&m).unwrap().clone();
        let movement = MixedMove {
            basic: Move::Change {
                variable: 0,
                value: Some(1),
            },
            lists: ListMove::Swap {
                left: ListPosition {
                    entity: 0,
                    index: 0,
                },
                right: ListPosition {
                    entity: 1,
                    index: 0,
                },
            },
        };
        {
            let t = d.apply(&movement).unwrap();
            assert_eq!(
                t.score(),
                score(t.assignment().lists(), direction) + SimpleScore::new(100)
            );
            assert_ne!(t.graph().constraint_matches(&m).unwrap(), &old);
        }
        assert_eq!(d.assignment(), &p.initial_assignment());
        assert_eq!(d.graph().constraint_matches(&m).unwrap(), &old);
        d.assert_consistent().unwrap();
    }
}
#[test]
fn local_search_uses_cascades_for_candidate_selection_and_saved_best() {
    for direction in [ListCascadeDirection::Previous, ListCascadeDirection::Next] {
        let p = problem();
        let mut n = ListConstraintNetwork::new(2, 3);
        n.add_constraint(
            "reference",
            vec![ListScope::Entity(0), ListScope::Entity(1)],
            |facts| {
                let lists = facts
                    .iter()
                    .map(|f| {
                        let ListFact::Entity(l) = f else { panic!() };
                        l.to_vec()
                    })
                    .collect();
                score(&ListAssignment::new(lists), direction)
            },
        )
        .unwrap();
        for evaluations in [1, 7, 50] {
            let config = LocalSearchConfig {
                max_steps: 20,
                max_evaluations: evaluations,
                assert_scores: true,
                ..Default::default()
            };
            let expected =
                list_local_search_from(&p, &n, p.initial_assignment(), config.clone()).unwrap();
            let (g, s, _) = graph(&p, direction);
            let actual =
                list_stream_local_search_from(&p, g, s, p.initial_assignment(), config).unwrap();
            let actual = actual.best.unwrap();
            let expected = expected.best.unwrap();
            assert_eq!(actual.assignment, expected.assignment);
            assert_eq!(actual.score, expected.score);
            assert_eq!(actual.score, score(&actual.assignment, direction));
        }
    }
}
