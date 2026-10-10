//! Original ordered-pair rank, sampling, mutation and scalability regressions.
use timetuck::*;
fn agrees(a: i32, b: i32, c: JoinComparison) -> bool {
    match c {
        JoinComparison::Equal => a == b,
        JoinComparison::LessThan => a < b,
        JoinComparison::LessThanOrEqual => a <= b,
        JoinComparison::GreaterThan => a > b,
        JoinComparison::GreaterThanOrEqual => a >= b,
    }
}
struct Payload(usize); // Neither Clone nor Eq: queries must keep borrowing occurrences.
type Index = CandidateIndex<i32, usize, Payload>;
fn compare(left: &Index, right: &Index, keys: &[i32]) {
    for relation in [
        JoinComparison::Equal,
        JoinComparison::LessThan,
        JoinComparison::LessThanOrEqual,
        JoinComparison::GreaterThan,
        JoinComparison::GreaterThanOrEqual,
    ] {
        let actual = left.join(right, relation).unwrap();
        // Independent old-order oracle: one explicit Cartesian block per matching key pair.
        let mut reference = CandidatePairs::new();
        for &lk in keys {
            for &rk in keys {
                if agrees(lk, rk, relation) {
                    reference
                        .push_product(
                            left.bucket(&lk).iter().as_slice(),
                            right.bucket(&rk).iter().as_slice(),
                        )
                        .unwrap();
                }
            }
        }
        assert_eq!(actual.len(), reference.len());
        let ids = |p: (
            &IndexedCandidate<usize, Payload>,
            &IndexedCandidate<usize, Payload>,
        )| (p.0.id, p.1.id);
        assert_eq!(
            actual.iter().map(ids).collect::<Vec<_>>(),
            reference.iter().map(ids).collect::<Vec<_>>()
        );
        assert_eq!(
            actual.iter().rev().map(ids).collect::<Vec<_>>(),
            reference.iter().rev().map(ids).collect::<Vec<_>>()
        );
        for rank in 0..actual.len() {
            let a = actual.get(rank).unwrap();
            let b = reference.get(rank).unwrap();
            assert!(std::ptr::eq(a.0, b.0) && std::ptr::eq(a.1, b.1));
            assert_eq!((a.0.value.0, a.1.value.0), (b.0.value.0, b.1.value.0));
        }
        for seed in [0, 1, 19, u64::MAX] {
            assert_eq!(
                actual.unique(seed).map(ids).collect::<Vec<_>>(),
                reference.unique(seed).map(ids).collect::<Vec<_>>()
            );
            assert_eq!(
                actual
                    .repeating(seed)
                    .take(300)
                    .map(ids)
                    .collect::<Vec<_>>(),
                reference
                    .repeating(seed)
                    .take(300)
                    .map(ids)
                    .collect::<Vec<_>>()
            );
        }
        assert!(actual.get(actual.len()).is_none());
        assert!(actual.get(u128::MAX).is_none());
    }
}
#[test]
fn group_major_ranks_and_seeded_draws_survive_uneven_buckets_and_mutations() {
    let keys = [i32::MIN, -3, -1, 0, 2, 4, i32::MAX];
    let mut left = Index::new();
    let mut right = Index::new();
    for (i, &key) in keys.iter().enumerate() {
        for j in 0..i % 4 + 1 {
            let id = i * 10 + j;
            left.insert(id, key, Payload(id % 3)).unwrap();
        }
        for j in 0..(i * 3) % 5 + 1 {
            let id = i * 10 + j;
            right.insert(id, key, Payload(id % 2)).unwrap();
        }
    }
    compare(&left, &right, &keys);
    left.remove(&10).unwrap();
    right.remove(&20).unwrap();
    left.replace(&20, i32::MAX, Payload(101)).unwrap();
    right.replace(&30, i32::MIN, Payload(102)).unwrap();
    compare(&left, &right, &keys);
    left.clear();
    compare(&left, &right, &keys);
    right.clear();
    compare(&left, &right, &keys);
}
#[test]
fn many_distinct_keys_use_linear_metadata_for_a_quadratic_pair_space() {
    let mut left = CandidateIndex::new();
    let mut right = CandidateIndex::new();
    let count = 10_000usize;
    for k in 0..count {
        left.insert(k, k, ()).unwrap();
        right.insert(k, k, ()).unwrap();
    }
    let pairs = left.join(&right, JoinComparison::LessThan).unwrap();
    assert_eq!(pairs.len(), (count as u128) * (count as u128 - 1) / 2);
    assert_eq!(pairs.block_count(), count - 1);
    let ids = |p: (&IndexedCandidate<usize, ()>, &IndexedCandidate<usize, ()>)| (p.0.id, p.1.id);
    assert_eq!(ids(pairs.get(0).unwrap()), (0, 1));
    assert_eq!(ids(pairs.get((count - 2) as u128).unwrap()), (0, count - 1));
    assert_eq!(ids(pairs.get((count - 1) as u128).unwrap()), (1, 2));
    assert_eq!(
        ids(pairs.get(pairs.len() - 1).unwrap()),
        (count - 2, count - 1)
    );
    assert!(pairs.unique(17).take(10_000).all(|(a, b)| a.id < b.id));
}
#[test]
fn appended_explicit_products_keep_global_ranks_and_retirement() {
    let mut left = CandidateIndex::new();
    let mut right = CandidateIndex::new();
    for i in 0..8 {
        left.insert(i, i / 2, ()).unwrap();
        right.insert(i, i / 3, ()).unwrap();
    }
    let mut pairs = left
        .join(&right, JoinComparison::GreaterThanOrEqual)
        .unwrap();
    let before: Vec<_> = pairs.iter().map(|(a, b)| (a.id, b.id)).collect();
    let extra_left = [IndexedCandidate { id: 99, value: () }];
    let extra_right = [
        IndexedCandidate { id: 100, value: () },
        IndexedCandidate { id: 101, value: () },
    ];
    pairs.push_product(&extra_left, &extra_right).unwrap();
    pairs.push_product(&[], &extra_right).unwrap();
    let expected: Vec<_> = before.into_iter().chain([(99, 100), (99, 101)]).collect();
    assert_eq!(
        pairs.iter().map(|(a, b)| (a.id, b.id)).collect::<Vec<_>>(),
        expected
    );
    assert_eq!(pairs.len(), expected.len() as u128);
    // Retirement still uses the same global ranks including appended direct blocks.
    let mut sampled = pairs.retiring(42);
    let mut got = std::collections::BTreeSet::new();
    while let Some(item) = sampled.next() {
        got.insert((item.0.id, item.1.id));
        sampled.retire().unwrap();
    }
    assert_eq!(got, expected.into_iter().collect());
}
#[test]
fn ordered_pair_provider_solves_with_both_scoring_backends() {
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
        variables.insert(i, i as i32, i).unwrap();
    }
    for i in 0..4 {
        values.insert(i, i, i).unwrap();
    }
    for stream in [false, true] {
        let pairs = variables
            .join(&values, JoinComparison::LessThanOrEqual)
            .unwrap();
        let mut provider = PairMoveProvider::new(
            pairs,
            PairSampling::Unique,
            14,
            100,
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
            let s = b.source::<usize, (usize, Option<i32>)>();
            b.score("value", s.port(), |r| {
                SimpleScore::new(i64::from(r.1.unwrap()))
            })
            .unwrap();
            stream_local_search_from_with_provider(
                &p,
                b.build(),
                s,
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
        assert_eq!(best.assignment.values(), [Some(3); 3]);
    }
}
