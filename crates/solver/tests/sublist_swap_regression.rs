//! Original span-swap regressions; no upstream method coverage credit.
use std::{
    collections::HashSet,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn swap(l: (usize, usize, usize), r: (usize, usize, usize), reverse: bool) -> ListMove<i32> {
    ListMove::SubListSwap {
        left: ListRange {
            entity: l.0,
            start: l.1,
            end: l.2,
        },
        right: ListRange {
            entity: r.0,
            start: r.1,
            end: r.2,
        },
        reverse,
    }
}
#[test]
fn invalid_ranges_pins_and_owner_ranges_roll_back_composites_and_panics() {
    let p = ListProblem::new(
        (0..9).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3, 4]).with_pinned_prefix(1),
            ListEntity::new(vec![5, 6, 7]).with_value_range(vec![1, 2, 5, 6, 7]),
            ListEntity::new(vec![8]).with_pinned(true),
        ],
    )
    .unwrap();
    let initial = p.initial_assignment();
    let valid = swap((0, 1, 3), (1, 0, 1), true);
    for bad in [
        swap((0, 0, 0), (1, 0, 1), false),
        swap((0, 3, 2), (1, 0, 1), false),
        swap((0, 0, 99), (1, 0, 1), false),
        swap((9, 0, 1), (0, 1, 2), false),
        swap((0, 1, 3), (0, 2, 3), false),
        swap((0, 1, 3), (0, 1, 3), true),
        swap((0, 0, 1), (0, 1, 3), false),
        swap((0, 2, 4), (1, 0, 1), true),
        swap((0, 1, 2), (2, 0, 1), false),
    ] {
        let mut a = initial.clone();
        assert!(
            p.apply(&mut a, &ListMove::Composite(vec![valid.clone(), bad]))
                .is_err()
        );
        assert_eq!(a, initial);
    }
    let mut a = initial.clone();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _trial = p.apply(&mut a, &valid).unwrap();
            panic!("caller failure");
        }))
        .is_err()
    );
    assert_eq!(a, initial);
}

// Independent reference labels each value with its replacement block, then
// reconstructs the whole state. This does not depend on mutation/index shifting.
fn reference(rows: &[Vec<i32>], left: ListRange, right: ListRange, reverse: bool) -> Vec<Vec<i32>> {
    let mut l = rows[left.entity][left.start..left.end].to_vec();
    let mut r = rows[right.entity][right.start..right.end].to_vec();
    if reverse {
        l.reverse();
        r.reverse();
    }
    rows.iter()
        .enumerate()
        .map(|(e, row)| {
            let mut next = vec![];
            for (i, v) in row.iter().enumerate() {
                if e == left.entity && i == left.start {
                    next.extend_from_slice(&r);
                }
                if e == right.entity && i == right.start {
                    next.extend_from_slice(&l);
                }
                if !(e == left.entity && (left.start..left.end).contains(&i)
                    || e == right.entity && (right.start..right.end).contains(&i))
                {
                    next.push(*v);
                }
            }
            next
        })
        .collect()
}
#[test]
fn all_small_disjoint_spans_match_reference_in_both_orders_and_neighborhoods() {
    for cut in 0..=7 {
        let rows = vec![(0..cut).collect(), (cut..7).collect()];
        let p = ListProblem::new(
            (0..7).collect(),
            rows.iter().cloned().map(ListEntity::new).collect(),
        )
        .unwrap();
        let initial = p.initial_assignment();
        let mut ranges = vec![];
        for (entity, row) in rows.iter().enumerate() {
            for start in 0..row.len() {
                for end in start + 1..=row.len() {
                    ranges.push(ListRange { entity, start, end });
                }
            }
        }
        for limit in [0, 1, 2, 3, usize::MAX] {
            let mut expected = HashSet::new();
            for &left in &ranges {
                for &right in &ranges {
                    if left.entity == right.entity
                        && left.start < right.end
                        && right.start < left.end
                    {
                        continue;
                    }
                    for reverse in [false, true] {
                        let movement = ListMove::SubListSwap {
                            left,
                            right,
                            reverse,
                        };
                        let want = reference(&rows, left, right, reverse);
                        let mut a = initial.clone();
                        {
                            let trial = p.apply(&mut a, &movement).unwrap();
                            assert_eq!(trial.lists(), want);
                        }
                        assert_eq!(a, initial);
                        if left.end - left.start <= limit
                            && right.end - right.start <= limit
                            && (left.end - left.start > 1 || right.end - right.start > 1)
                        {
                            expected.insert(want);
                        }
                    }
                }
            }
            let generated: Vec<_> = list_sublist_swap_moves(&p, &initial, limit)
                .unwrap()
                .collect();
            assert_eq!(
                generated.len(),
                generated.iter().collect::<HashSet<_>>().len()
            );
            let actual: HashSet<_> = generated
                .iter()
                .map(|m| {
                    let mut a = initial.clone();
                    p.apply(&mut a, m).unwrap().commit();
                    a.lists().to_vec()
                })
                .collect();
            assert_eq!(actual, expected);
        }
    }
}
#[test]
fn lazy_provider_preserves_pins_and_bidirectional_ranges() {
    let p = ListProblem::new(
        (0..9).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3])
                .with_pinned_prefix(1)
                .with_value_range(vec![0, 1, 2, 3, 4, 5]),
            ListEntity::new(vec![4, 5, 6, 7]).with_value_range(vec![1, 2, 4, 5, 6, 7]),
            ListEntity::new(vec![8]).with_pinned(true),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    assert!(
        list_sublist_swap_moves(&p, &a, usize::MAX)
            .unwrap()
            .next()
            .is_some()
    );
    let moves: Vec<_> = list_sublist_swap_moves(&p, &a, usize::MAX)
        .unwrap()
        .collect();
    assert!(moves.contains(&swap((0, 1, 3), (1, 0, 2), true)));
    assert!(!moves.contains(&swap((0, 1, 4), (1, 0, 2), true)));
    assert!(!moves.contains(&swap((0, 1, 3), (1, 1, 3), true)));
    for m in moves {
        let mut copy = a.clone();
        p.apply(&mut copy, &m).unwrap().commit();
        assert!(p.is_complete(&copy));
    }
}
fn weighted(a: &ListAssignment<i32>) -> SimpleScore {
    let mut sum = 0;
    for (e, row) in a.lists().iter().enumerate() {
        for (i, &v) in row.iter().enumerate() {
            sum += i64::from(v) * (e as i64 + 1) * (i as i64 + 1)
                + i64::from(if i == 0 { 0 } else { row[i - 1] })
                - i64::from(*row.get(i + 1).unwrap_or(&0));
        }
    }
    SimpleScore::new(sum)
}
#[test]
fn scoped_and_stream_caches_match_full_score_through_span_swap_trials_and_commits() {
    let p = ListProblem::new(
        (0..6).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3]),
            ListEntity::new(vec![4, 5]),
        ],
    )
    .unwrap();
    let mut n = ListConstraintNetwork::new(2, 6);
    for i in 0..6 {
        n.add_constraint(format!("v{i}"), vec![ListScope::Value(i)], |facts| {
            let ListFact::Value { value, element } = &facts[0] else {
                unreachable!()
            };
            let r = element.as_ref().unwrap();
            SimpleScore::new(
                i64::from(**value) * (r.position.entity as i64 + 1) * (r.position.index as i64 + 1)
                    + i64::from(r.previous.copied().unwrap_or(0))
                    - i64::from(r.next.copied().unwrap_or(0)),
            )
        })
        .unwrap();
    }
    let mut b = StreamGraphBuilder::new();
    let s = ListStreamSources::new(&mut b);
    b.score("positions", s.values.port(), |r| {
        let pos = r.position.unwrap();
        SimpleScore::new(
            i64::from(r.value) * (pos.entity as i64 + 1) * (pos.index as i64 + 1)
                + i64::from(r.previous.unwrap_or(0))
                - i64::from(r.next.unwrap_or(0)),
        )
    })
    .unwrap();
    let mut scoped = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let mut stream =
        ListStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    for round in 0..120 {
        let moves: Vec<_> = list_sublist_swap_moves(&p, scoped.assignment(), 4)
            .unwrap()
            .collect();
        let m = &moves[(round * 37 + 11) % moves.len()];
        let old = scoped.assignment().clone();
        {
            let a = scoped.apply(m).unwrap();
            let b = stream.apply(m).unwrap();
            assert_eq!(a.assignment(), b.assignment());
            assert_eq!(a.score(), weighted(a.assignment()));
            assert_eq!(a.score(), b.score());
        }
        assert_eq!(scoped.assignment(), &old);
        assert_eq!(stream.assignment(), &old);
        if round % 3 != 0 {
            scoped.apply(m).unwrap().commit();
            stream.apply(m).unwrap().commit();
        }
        scoped.assert_consistent().unwrap();
        stream.assert_consistent().unwrap();
    }
}

#[test]
fn opt_in_span_swaps_escape_list_and_joint_mixed_plateaus_in_both_backends() {
    fn reward(route: &[i32]) -> SimpleScore {
        SimpleScore::new(i64::from(route == [0, 5, 6, 3, 4, 1, 2, 7]))
    }
    let p = ListProblem::new((0..8).collect(), vec![ListEntity::new((0..8).collect())]).unwrap();
    for m in list_sublist_change_moves(&p, &p.initial_assignment(), 2).unwrap() {
        let mut a = p.initial_assignment();
        p.apply(&mut a, &m).unwrap().commit();
        assert_eq!(reward(a.list(0).unwrap()), SimpleScore::new(0));
    }
    let mut n = ListConstraintNetwork::new(1, 8);
    n.add_constraint("target", vec![ListScope::Entity(0)], |facts| {
        let ListFact::Entity(row) = facts[0] else {
            unreachable!()
        };
        reward(row)
    })
    .unwrap();
    for neighborhood in [
        Neighborhood::ChangeAndSwap,
        Neighborhood::ChangeSwapAndSubList { max_length: 2 },
    ] {
        let expected = SimpleScore::new(i64::from(neighborhood != Neighborhood::ChangeAndSwap));
        let config = LocalSearchConfig {
            neighborhood,
            max_evaluations: 100_000,
            max_steps: 2,
            selection: MoveSelection::BestAccepted,
            assert_scores: true,
            ..Default::default()
        };
        let scoped =
            list_local_search_from(&p, &n, p.initial_assignment(), config.clone()).unwrap();
        let mut b = StreamGraphBuilder::new();
        let s = ListStreamSources::new(&mut b);
        b.score("target", s.entities.port(), |(_, row)| reward(row))
            .unwrap();
        let stream =
            list_stream_local_search_from(&p, b.build(), s, p.initial_assignment(), config.clone())
                .unwrap();
        assert_eq!(scoped.best.as_ref().unwrap().score, expected);
        assert_eq!(scoped.best, stream.best);
        assert_eq!(scoped.completed_steps, stream.completed_steps);
        let p = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
            p.clone(),
        );
        let mut n = MixedConstraintNetwork::new(1, 1, 8);
        n.add_constraint(
            "joint",
            vec![MixedScope::Basic(0), MixedScope::ListEntity(0)],
            |facts| {
                let (MixedFact::Basic(v), MixedFact::ListEntity(row)) = (&facts[0], &facts[1])
                else {
                    unreachable!()
                };
                if v.copied() == Some(1) {
                    reward(row)
                } else {
                    SimpleScore::new(0)
                }
            },
        )
        .unwrap();
        let config = MixedLocalSearchConfig {
            search: config,
            joint_moves: true,
        };
        let scoped =
            mixed_local_search_from(&p, &n, p.initial_assignment(), config.clone()).unwrap();
        let mut b = StreamGraphBuilder::new();
        let s = MixedStreamSources::new(&mut b);
        let joined = b
            .join_equal(
                s.basic.port(),
                s.lists.entities.port(),
                |(i, _)| *i,
                |(i, _)| *i,
            )
            .unwrap();
        b.score("joint", &joined, |((_, v), (_, row))| {
            if *v == Some(1) {
                reward(row)
            } else {
                SimpleScore::new(0)
            }
        })
        .unwrap();
        let stream =
            mixed_stream_local_search_from(&p, b.build(), s, p.initial_assignment(), config)
                .unwrap();
        assert_eq!(scoped.best.as_ref().unwrap().score, expected);
        assert_eq!(scoped.best, stream.best);
        assert_eq!(scoped.completed_steps, stream.completed_steps);
    }
}
