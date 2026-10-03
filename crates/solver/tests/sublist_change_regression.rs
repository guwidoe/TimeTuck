//! Original span-move, cache and search regressions; no upstream coverage credit.
use std::{
    collections::HashSet,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn span(
    e: usize,
    start: usize,
    end: usize,
    d: usize,
    index: usize,
    reverse: bool,
) -> ListMove<i32> {
    ListMove::SubListChange {
        entity: e,
        start,
        end,
        destination: ListPosition { entity: d, index },
        reverse,
    }
}
#[test]
fn atomic_errors_empty_spans_and_unwinding_preserve_original() {
    let p = ListProblem::new(
        (0..6).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![3, 4]).with_value_range(vec![1, 3, 4]),
            ListEntity::new(vec![5]).with_pinned(true),
        ],
    )
    .unwrap();
    let initial = p.initial_assignment();
    for bad in [
        span(0, 2, 1, 0, 0, false),
        span(0, 0, 4, 0, 0, false),
        span(3, 0, 0, 0, 0, false),
        span(0, 1, 3, 0, 2, false),
        span(0, 1, 3, 3, 0, false),
        span(0, 0, 2, 0, 0, true),
        span(0, 1, 3, 1, 0, false),
        span(0, 1, 2, 2, 1, false),
    ] {
        let mut a = initial.clone();
        let movement = ListMove::Composite(vec![span(0, 1, 3, 0, 1, true), bad]);
        assert!(p.apply(&mut a, &movement).is_err());
        assert_eq!(a, initial);
    }
    let mut a = initial.clone();
    p.apply(&mut a, &span(0, 3, 3, 1, 2, true))
        .unwrap()
        .commit();
    assert_eq!(a, initial);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let trial = p.apply(&mut a, &span(0, 1, 3, 0, 1, true)).unwrap();
        assert_eq!(trial.list(0).unwrap(), &[0, 2, 1]);
        panic!("caller failure");
    }));
    assert!(result.is_err());
    assert_eq!(a, initial);
}

#[test]
fn lazy_neighborhood_matches_independent_splice_successors_and_respects_pins_ranges() {
    let p = ListProblem::new((0..6).collect(), vec![ListEntity::new((0..6).collect())]).unwrap();
    let a = p.initial_assignment();
    assert_eq!(list_sublist_change_moves(&p, &a, 2).unwrap().count(), 45);
    for limit in [0, 1, 2, 3, usize::MAX] {
        let mut expected = HashSet::new();
        for start in 0..6 {
            for end in start + 2..=6 {
                if end - start > limit {
                    continue;
                }
                let remaining: Vec<_> = (0..start).chain(end..6).map(|v| v as i32).collect();
                for index in 0..=remaining.len() {
                    for reversed in [false, true] {
                        if !reversed && index == start {
                            continue;
                        }
                        let mut block: Vec<_> = (start..end).map(|v| v as i32).collect();
                        if reversed {
                            block.reverse();
                        }
                        let next: Vec<_> = remaining[..index]
                            .iter()
                            .chain(&block)
                            .chain(&remaining[index..])
                            .copied()
                            .collect();
                        expected.insert(next);
                    }
                }
            }
        }
        let actual: HashSet<_> = list_sublist_change_moves(&p, &a, limit)
            .unwrap()
            .map(|m| {
                let mut copy = a.clone();
                p.apply(&mut copy, &m).unwrap().commit();
                copy.list(0).unwrap().to_vec()
            })
            .collect();
        assert_eq!(actual, expected);
    }
    let p = ListProblem::new(
        (0..6).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![3, 4]).with_value_range(vec![1, 3, 4]),
            ListEntity::new(vec![5]).with_pinned(true),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let moves: Vec<_> = list_sublist_change_moves(&p, &a, usize::MAX)
        .unwrap()
        .collect();
    assert!(moves.contains(&span(0, 1, 3, 0, 1, true)));
    assert!(!moves.iter().any(|m| m.touched_entities().contains(&2)));
    for m in moves {
        let mut copy = a.clone();
        p.apply(&mut copy, &m).unwrap().commit();
        assert!(p.is_complete(&copy));
    }
    assert!(
        list_sublist_change_moves(&p, &a, usize::MAX)
            .unwrap()
            .take(1)
            .next()
            .is_some()
    );
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
fn scoped_and_stream_caches_match_full_score_through_span_trials_and_commits() {
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
        let moves: Vec<_> = list_sublist_change_moves(&p, scoped.assignment(), 4)
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
fn opt_in_spans_escape_list_and_joint_mixed_plateaus_in_both_backends() {
    fn reward(route: &[i32]) -> SimpleScore {
        SimpleScore::new(i64::from(route == [0, 3, 4, 1, 2, 5]))
    }
    let p = ListProblem::new((0..6).collect(), vec![ListEntity::new((0..6).collect())]).unwrap();
    let mut n = ListConstraintNetwork::new(1, 6);
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
        let scoped = list_local_search_from(&p, &n, p.initial_assignment(), config).unwrap();
        let mut b = StreamGraphBuilder::new();
        let s = ListStreamSources::new(&mut b);
        b.score("target", s.entities.port(), |(_, row)| reward(row))
            .unwrap();
        let stream =
            list_stream_local_search_from(&p, b.build(), s, p.initial_assignment(), config)
                .unwrap();
        assert_eq!(scoped.best.as_ref().unwrap().score, expected);
        assert_eq!(scoped.best, stream.best);
        assert_eq!(scoped.completed_steps, stream.completed_steps);
        let p = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
            p.clone(),
        );
        let mut n = MixedConstraintNetwork::new(1, 1, 6);
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
        let scoped = mixed_local_search_from(&p, &n, p.initial_assignment(), config).unwrap();
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

#[test]
fn sublist_option_preserves_basic_swaps_and_disabled_list_order() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(1),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("target", vec![0, 1], |v| {
        SimpleScore::new(i64::from(v == [Some(&1), Some(&0)]))
    })
    .unwrap();
    let r = local_search(
        &p,
        &n,
        LocalSearchConfig {
            neighborhood: Neighborhood::ChangeSwapAndSubList { max_length: 2 },
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.best.unwrap().score, SimpleScore::new(1));
    let p = ListProblem::new(
        (0..4).collect(),
        vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![2, 3])],
    )
    .unwrap();
    let a = p.initial_assignment();
    let old: Vec<_> = list_moves(&p, &a, Neighborhood::ChangeAndSwap)
        .unwrap()
        .collect();
    for max_length in [0, 1] {
        assert_eq!(
            list_moves(&p, &a, Neighborhood::ChangeSwapAndSubList { max_length })
                .unwrap()
                .collect::<Vec<_>>(),
            old
        );
    }
}
