use std::{cell::Cell, collections::BTreeSet};
use timetuck::*;
#[test]
fn block_ranks_preserve_occurrence_identity_and_double_ended_order() {
    struct Value(u8);
    let left = [Value(1), Value(1)];
    let right = [Value(2), Value(3), Value(4)];
    let mut p = CandidatePairs::new();
    p.push_product(&[], &right).unwrap();
    p.push_product(&left, &right).unwrap();
    p.push_product(&left[..1], &right[..1]).unwrap();
    assert_eq!(p.block_count(), 2);
    assert_eq!(p.len(), 7);
    let v: Vec<_> = p.iter().map(|(l, r)| (l.0, r.0)).collect();
    assert_eq!(v, [(1, 2), (1, 3), (1, 4), (1, 2), (1, 3), (1, 4), (1, 2)]);
    assert!(std::ptr::eq(p.get(3).unwrap().0, &left[1]));
    assert!(p.get(7).is_none());
    assert!(p.get(u128::MAX).is_none());
    let mut it = p.iter();
    assert!(std::ptr::eq(it.next().unwrap().0, &left[0]));
    assert_eq!(it.next_back().unwrap().1.0, 2);
    assert_eq!(it.count(), 5);
    assert_eq!(p.unique(9).count(), 7);
    assert_eq!(
        p.repeating(9)
            .take(100)
            .map(|(l, r)| (l.0, r.0))
            .collect::<Vec<_>>(),
        p.repeating(9)
            .take(100)
            .map(|(l, r)| (l.0, r.0))
            .collect::<Vec<_>>()
    );
}
#[test]
fn all_join_relations_match_nested_loops_after_rekeys_and_removals() {
    let mut left = CandidateIndex::new();
    let mut right = CandidateIndex::new();
    let mut lm = std::collections::BTreeMap::new();
    let mut rm = std::collections::BTreeMap::new();
    for step in 0..180usize {
        let (index, model) = if step % 2 == 0 {
            (&mut left, &mut lm)
        } else {
            (&mut right, &mut rm)
        };
        let id = (step * 17 + step / 5) % 21;
        let key = (step * 13) % 9;
        if step % 5 == 0 {
            if model.remove(&id).is_some() {
                index.remove(&id).unwrap();
            }
        } else if model.insert(id, key).is_some() {
            index.replace(&id, key, ()).unwrap();
        } else {
            index.insert(id, key, ()).unwrap();
        }
        for relation in [
            JoinComparison::Equal,
            JoinComparison::LessThan,
            JoinComparison::LessThanOrEqual,
            JoinComparison::GreaterThan,
            JoinComparison::GreaterThanOrEqual,
        ] {
            let mut expected = BTreeSet::new();
            for (a, ak) in &lm {
                for (b, bk) in &rm {
                    let yes = match relation {
                        JoinComparison::Equal => ak == bk,
                        JoinComparison::LessThan => ak < bk,
                        JoinComparison::LessThanOrEqual => ak <= bk,
                        JoinComparison::GreaterThan => ak > bk,
                        JoinComparison::GreaterThanOrEqual => ak >= bk,
                    };
                    if yes {
                        expected.insert((*a, *b));
                    }
                }
            }
            let pairs = left.join(&right, relation).unwrap();
            assert_eq!(pairs.len(), expected.len() as u128);
            assert_eq!(
                pairs
                    .iter()
                    .map(|(a, b)| (a.id, b.id))
                    .collect::<BTreeSet<_>>(),
                expected
            );
            assert_eq!(
                pairs
                    .unique(step as u64)
                    .map(|(a, b)| (a.id, b.id))
                    .collect::<BTreeSet<_>>(),
                expected
            );
            assert!(
                pairs
                    .repeating(step as u64)
                    .take(5)
                    .all(|(a, b)| expected.contains(&(a.id, b.id)))
            );
        }
    }
}
#[test]
fn unique_draw_positions_are_uniform_across_unequal_cartesian_blocks() {
    let left = [0, 1, 2];
    let r1 = [0, 1];
    let r2 = [2, 3, 4];
    let mut p = CandidatePairs::new();
    p.push_product(&left[..1], &r1).unwrap();
    p.push_product(&left[1..], &r2).unwrap();
    let expected: BTreeSet<_> = p.iter().map(|(l, r)| (*l, *r)).collect();
    let mut counts = vec![std::collections::BTreeMap::new(); 8];
    for seed in 0..20_000 {
        let mut seen = BTreeSet::new();
        for (i, (l, r)) in p.unique(seed).enumerate() {
            assert!(seen.insert((*l, *r)));
            *counts[i].entry((*l, *r)).or_insert(0usize) += 1;
        }
        assert_eq!(seen, expected);
    }
    for position in counts {
        for pair in &expected {
            let count = position[pair];
            let sigma = ((count as f64 - 2500.0) / ((20_000.0_f64 * 0.125 * 0.875).sqrt())).abs();
            assert!(sigma < 5.0, "{pair:?} count={count}, sigma={sigma}");
        }
    }
}
#[test]
fn zero_and_rejecting_providers_are_lazy_bounded_and_reproducible() {
    let left = [1, 2];
    let right = [3, 4, 5];
    let calls = Cell::new(0);
    for limit in [0, 7] {
        let mut p = CandidatePairs::new();
        p.push_product(&left, &right).unwrap();
        let mut provider = PairMoveProvider::new(
            p,
            PairSampling::Repeating,
            6,
            limit,
            |_: &(), _: &(), _: u64, _: &i32, _: &i32| {
                calls.set(calls.get() + 1);
                None::<i32>
            },
        );
        assert_eq!(provider.moves(&(), &(), 0).next(), None);
        assert_eq!(calls.get(), limit);
    }
    let mut p = CandidatePairs::new();
    p.push_product(&left, &right).unwrap();
    let mut provider = PairMoveProvider::new(
        p,
        PairSampling::Unique,
        4,
        100,
        |_: &(), a: &i32, s: u64, l: &i32, r: &i32| Some((*l, *r, *a, s)),
    );
    let first: Vec<_> = provider.moves(&(), &8, 0).collect();
    assert_eq!(first.len(), 6);
    assert_eq!(first, provider.moves(&(), &8, 0).collect::<Vec<_>>());
    let changed: Vec<_> = provider.moves(&(), &9, 1).collect();
    assert!(changed.iter().all(|v| v.2 == 9 && v.3 == 1));
    let p = CandidatePairs::<i32, i32>::new();
    assert_eq!(p.repeating(0).next(), None);
    assert_eq!(p.unique(0).next(), None);
}
#[test]
#[cfg(target_pointer_width = "64")]
fn huge_virtual_products_sample_without_materializing_and_overflow_is_atomic() {
    let values = vec![(); usize::MAX];
    let mut p = CandidatePairs::new();
    p.push_product(&values, &values).unwrap();
    let size = (usize::MAX as u128) * (usize::MAX as u128);
    assert_eq!(p.len(), size);
    assert!(p.get(size - 1).is_some());
    assert_eq!(p.unique(19).take(100).count(), 100);
    assert_eq!(p.repeating(19).take(100).count(), 100);
    assert_eq!(p.push_product(&values, &values), Err(PairCountOverflow));
    assert_eq!(p.len(), size);
    assert_eq!(p.block_count(), 1);
}
#[test]
fn indexed_pair_moves_solve_with_both_scoring_backends() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2, 3]).with_initial(0); 3]).unwrap();
    let mut n = ConstraintNetwork::new(3);
    for i in 0..3 {
        n.add_constraint(format!("value{i}"), vec![i], |v| {
            SimpleScore::new(i64::from(*v[0].unwrap()))
        })
        .unwrap();
    }
    let mut variables = CandidateIndex::new();
    let mut values = CandidateIndex::new();
    for i in 0..3 {
        variables.insert(i, (), i).unwrap();
    }
    for i in 0..4 {
        values.insert(i, (), i).unwrap();
    }
    for stream in [false, true] {
        let pairs = variables.join(&values, JoinComparison::Equal).unwrap();
        let mut provider = PairMoveProvider::new(
            pairs,
            PairSampling::Unique,
            14,
            12,
            |_: &FiniteProblem<i32>,
             a: &Assignment<i32>,
             _: u64,
             l: &IndexedCandidate<usize, usize>,
             r: &IndexedCandidate<i32, i32>| {
                (a.values()[l.id] != Some(r.value)).then_some(Move::Change {
                    variable: l.id,
                    value: Some(r.value),
                })
            },
        );
        let config = LocalSearchConfig {
            max_steps: 3,
            selection: MoveSelection::BestAccepted,
            assert_scores: true,
            ..Default::default()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("value", source.port(), |r| {
                SimpleScore::new(i64::from(r.1.unwrap()))
            })
            .unwrap();
            stream_local_search_from_with_provider(
                &p,
                b.build(),
                source,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut provider)
                .unwrap()
        };
        let best = result.best.unwrap();
        assert_eq!(best.score, SimpleScore::new(9));
        assert_eq!(best.assignment.values(), &[Some(3), Some(3), Some(3)]);
    }
}
