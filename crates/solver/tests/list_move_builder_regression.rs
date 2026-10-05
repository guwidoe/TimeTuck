// Original native regressions; no upstream method credit.
use std::panic::{AssertUnwindSafe, catch_unwind};
use timetuck::*;

fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn span(entity: usize, start: usize, end: usize) -> ListRange {
    ListRange { entity, start, end }
}
fn replay(
    p: &ListProblem<i32>,
    base: &ListAssignment<i32>,
    d: ListMoveBuilder<'_, i32>,
    expected: &[Vec<i32>],
) {
    assert_eq!(d.assignment().lists(), expected);
    let m = d.finish();
    let mut a = base.clone();
    {
        let trial = p.apply(&mut a, &m).unwrap();
        assert_eq!(trial.lists(), expected);
        for (entity, list) in expected.iter().enumerate() {
            for (index, value) in list.iter().enumerate() {
                let e = trial.element(value).unwrap();
                assert_eq!(e.position, pos(entity, index));
                assert_eq!(e.previous, index.checked_sub(1).map(|i| &list[i]));
                assert_eq!(e.next, list.get(index + 1));
            }
        }
    }
    assert_eq!(&a, base);
}

#[test]
fn every_small_replace_and_shift_matches_direct_vector_edits() {
    for n in 1..=6 {
        for split in 0..=n {
            let values: Vec<_> = (10..10 + n as i32).collect();
            let lists = vec![values[..split].to_vec(), values[split..].to_vec()];
            let p = ListProblem::new(values, lists.iter().cloned().map(ListEntity::new).collect())
                .unwrap();
            let a = p.initial_assignment();
            let slots: Vec<_> = lists
                .iter()
                .enumerate()
                .flat_map(|(e, l)| (0..l.len()).map(move |i| pos(e, i)))
                .collect();
            for &source in &slots {
                for &target in &slots {
                    if source == target {
                        continue;
                    }
                    let mut expected = lists.clone();
                    let displaced = expected[target.entity][target.index];
                    expected[target.entity][target.index] = lists[source.entity][source.index];
                    expected[source.entity].remove(source.index);
                    let mut d = ListMoveBuilder::new(&p, &a).unwrap();
                    assert_eq!(d.replace(source, target).unwrap(), displaced);
                    replay(&p, &a, d, &expected);
                    if source.entity == target.entity {
                        let mut expected = lists.clone();
                        let moved = expected[source.entity].remove(source.index);
                        expected[source.entity].insert(target.index, moved);
                        let mut d = ListMoveBuilder::new(&p, &a).unwrap();
                        assert_eq!(
                            d.shift(source, target.index as isize - source.index as isize)
                                .unwrap(),
                            moved
                        );
                        replay(&p, &a, d, &expected);
                    }
                }
            }
        }
    }
}

#[test]
fn failed_edits_keep_prior_steps_and_atomic_groups_can_restore_pins() {
    let p = ListProblem::new(
        (0..6).collect(),
        vec![
            ListEntity::new(vec![0, 1]).with_pinned_prefix(1),
            ListEntity::new(vec![2]).with_value_range(vec![2, 3, 4]),
            ListEntity::new(vec![5]).with_pinned(true),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let mut d = ListMoveBuilder::new(&p, &a).unwrap();
    d.assign(3, pos(1, 1)).unwrap();
    let saved = d.assignment().clone();
    let steps = d.steps().to_vec();
    assert!(d.assign_many([4, 3], pos(1, 2)).is_err()); // late duplicate after first insert
    assert!(d.set_unassigned(4, pos(0, 0)).is_err()); // pin
    assert!(d.replace(pos(1, 1), pos(0, 0)).is_err()); // pin
    assert!(d.assign(4, pos(2, 1)).is_err()); // whole pin
    assert!(d.relocate(pos(0, 1), pos(1, 0)).is_err()); // receiving range
    assert!(d.assign_many([], pos(1, 9)).is_err()); // even empty input validates destination
    assert!(d.assign(99, pos(0, 1)).is_err());
    assert!(d.shift(pos(1, 0), 0).is_err());
    assert!(d.shift(pos(1, 0), isize::MIN).is_err());
    assert!(d.shift(pos(1, 1), isize::MAX).is_err());
    assert!(d.replace(pos(1, 0), pos(1, 0)).is_err());
    assert!(d.relocate_span(span(1, 1, 1), pos(0, 1), false).is_err());
    assert!(d.swap_spans(span(0, 0, 0), span(1, 0, 1), false).is_err());
    assert!(d.unassign(span(1, 0, 3)).is_err());
    assert_eq!(d.assignment(), &saved);
    assert_eq!(d.steps(), steps);
    let iterator = std::iter::once(4).chain(std::iter::from_fn(|| panic!("input failure")));
    assert!(catch_unwind(AssertUnwindSafe(|| d.assign_many(iterator, pos(1, 2)))).is_err());
    assert_eq!(d.assignment(), &saved);
    assert_eq!(d.steps(), steps);
    d.push(ListMove::Composite(vec![
        ListMove::Unassign {
            entity: 0,
            start: 0,
            end: 1,
        },
        ListMove::Assign {
            value: 0,
            destination: pos(0, 0),
        },
        ListMove::Assign {
            value: 4,
            destination: pos(0, 2),
        },
    ]))
    .unwrap();
    replay(&p, &a, d, &[vec![0, 1, 4], vec![2, 3], vec![5]]);
}

fn weighted(lists: &[Vec<i32>]) -> i64 {
    lists.iter().enumerate().map(|(e, l)| one_list(e, l)).sum()
}
fn one_list(entity: usize, values: &[i32]) -> i64 {
    values
        .iter()
        .enumerate()
        .map(|(i, v)| (entity as i64 + 1) * (i as i64 + 1) * i64::from(*v))
        .sum()
}

#[test]
fn compound_drafts_fork_replay_and_score_in_both_backends() {
    let p = ListProblem::new(
        (10..17).collect(),
        vec![
            ListEntity::new(vec![10, 11, 12]),
            ListEntity::new(vec![13, 14]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let mut d = ListMoveBuilder::new(&p, &a).unwrap();
    d.assign_many([15, 16], pos(1, 1)).unwrap();
    let mut fork = d.clone();
    fork.unassign(span(0, 1, 3)).unwrap();
    assert_eq!(fork.assignment().lists(), &[vec![10], vec![13, 15, 16, 14]]);
    d.relocate_span(span(1, 1, 3), pos(0, 1), true).unwrap();
    d.swap_spans(span(0, 1, 3), span(1, 0, 1), false).unwrap();
    // [10,16,15,11,12] / [13,14] -> [10,13,11,12] / [16,15,14]
    d.shift(pos(0, 3), -2).unwrap();
    let expected = vec![vec![10, 12, 13, 11], vec![16, 15, 14]];
    assert_eq!(d.assignment().lists(), expected);
    let mut network = ListConstraintNetwork::new(2, 7);
    for entity in 0..2 {
        network
            .add_constraint(
                format!("entity-{entity}"),
                vec![ListScope::Entity(entity)],
                move |facts| {
                    let ListFact::Entity(v) = facts[0] else {
                        unreachable!()
                    };
                    SimpleScore::new(one_list(entity, v))
                },
            )
            .unwrap();
    }
    let mut scoped = ListScoreDirector::new(&p, &network, a.clone()).unwrap();
    let mut b = StreamGraphBuilder::new();
    let s = ListStreamSources::new(&mut b);
    b.score("positions", s.entities.port(), |(e, values)| {
        SimpleScore::new(one_list(*e, values))
    })
    .unwrap();
    let mut stream = ListStreamScoreDirector::new(&p, b.build(), s, a.clone()).unwrap();
    let m = d.finish();
    for commit in [false, true] {
        let left = scoped.apply(&m).unwrap();
        let right = stream.apply(&m).unwrap();
        assert_eq!(left.assignment().lists(), expected);
        assert_eq!(right.assignment().lists(), expected);
        assert_eq!(left.score(), SimpleScore::new(weighted(&expected)));
        assert_eq!(right.score(), left.score());
        if commit {
            left.commit();
            right.commit();
        } else {
            drop(left);
            drop(right);
            assert_eq!(scoped.assignment(), &a);
            assert_eq!(stream.assignment(), &a);
        }
    }
    scoped.assert_consistent().unwrap();
    stream.assert_consistent().unwrap();
    replay(&p, &a, fork, &[vec![10], vec![13, 15, 16, 14]]);
}

#[test]
fn replace_and_reassign_compound_provider_solves_through_both_backends() {
    let p = ListProblem::new(
        vec![10, 20, 30],
        vec![ListEntity::new(vec![10, 20]), ListEntity::new(vec![30])],
    )
    .unwrap();
    let mut network = ListConstraintNetwork::new(2, 3);
    network
        .add_constraint("first", vec![ListScope::Entity(0)], |f| {
            let ListFact::Entity(v) = f[0] else {
                unreachable!()
            };
            SimpleScore::new(i64::from(v[0]))
        })
        .unwrap();
    for stream in [false, true] {
        let mut provider = |p: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| {
            if a.lists()[0][0] == 30 {
                return vec![];
            }
            let mut d = ListMoveBuilder::new(p, a).unwrap();
            let displaced = d.replace(pos(1, 0), pos(0, 0)).unwrap();
            d.assign(displaced, pos(1, 0)).unwrap();
            vec![d.finish()]
        };
        let config = LocalSearchConfig {
            max_steps: 4,
            max_move_evaluations: Some(10),
            assert_scores: true,
            ..Default::default()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let s = ListStreamSources::new(&mut b);
            b.score("first", s.entities.port(), |(e, v)| {
                SimpleScore::new(if *e == 0 { i64::from(v[0]) } else { 0 })
            })
            .unwrap();
            list_stream_local_search_from_with_provider(
                &p,
                b.build(),
                s,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            list_local_search_from_with_provider(
                &p,
                &network,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        };
        let best = result.best.unwrap();
        assert_eq!(best.assignment.lists(), &[vec![30, 20], vec![10]]);
        assert_eq!(best.score, SimpleScore::new(30));
        assert_eq!(result.completed_steps, 1);
    }
}
