// Original independent graph/enumeration regressions, no upstream method credit.
use num_bigint::BigUint;
use std::collections::BTreeSet;
use timetuck::*;
fn edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

#[test]
fn every_small_degree_two_graph_matches_components_and_move_validation() {
    for n in 3..=6 {
        let shift = 5;
        let original: BTreeSet<_> = (0..n).map(|i| edge(i, (i + 1) % n)).collect();
        let possible: Vec<_> = (0..n)
            .flat_map(|i| (i + 1..n).map(move |j| (i, j)))
            .collect();
        let p = ListProblem::new(
            (0..shift + n).collect(),
            vec![ListEntity::new((0..shift + n).collect()).with_pinned_prefix(shift)],
        )
        .unwrap();
        let before = p.initial_assignment();
        for mask in 0usize..1 << possible.len() {
            if mask.count_ones() as usize != n {
                continue;
            }
            let graph: BTreeSet<_> = possible
                .iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, e)| *e)
                .collect();
            if (0..n).any(|v| graph.iter().filter(|&&(a, b)| a == v || b == v).count() != 2) {
                continue;
            }
            let mut labels = vec![usize::MAX; n];
            let mut count = 0;
            // Independent component flood fill, not ordered cycle traversal.
            for root in 0..n {
                if labels[root] != usize::MAX {
                    continue;
                }
                let mut todo = vec![root];
                while let Some(v) = todo.pop() {
                    if labels[v] != usize::MAX {
                        continue;
                    }
                    labels[v] = count;
                    for &(a, b) in &graph {
                        if a == v {
                            todo.push(b);
                        }
                        if b == v {
                            todo.push(a);
                        }
                    }
                }
                count += 1;
            }
            let mut removed: Vec<_> = original.difference(&graph).copied().collect();
            let mut added: Vec<_> = graph.difference(&original).copied().collect();
            if removed.is_empty() {
                removed = original.iter().take(2).copied().collect();
                added = removed.clone();
            }
            let removed: Vec<_> = removed
                .iter()
                .map(|&(a, b)| (a + shift, b + shift))
                .collect();
            let added: Vec<_> = added.iter().map(|&(a, b)| (a + shift, b + shift)).collect();
            let t = ListKOptTopology::new(shift..shift + n, &removed, &added).unwrap();
            assert_eq!(t.range(), shift..shift + n);
            assert_eq!(t.cycle_ids(), labels);
            assert_eq!(t.cycle_count(), count);
            assert_eq!(t.cycle_id(shift - 1), None);
            assert_eq!(t.cycle_id(shift + n), None);
            for (v, &id) in labels.iter().enumerate() {
                assert_eq!(t.cycle_id(v + shift), Some(id));
            }
            for (id, cycle) in t.cycles().iter().enumerate() {
                assert_eq!(Some(&cycle[0]), cycle.iter().min());
                for i in 0..cycle.len() {
                    assert_eq!(labels[cycle[i] - shift], id);
                    assert!(graph.contains(&edge(
                        cycle[i] - shift,
                        cycle[(i + 1) % cycle.len()] - shift
                    )));
                }
            }
            // Edge order/orientation is metadata, not component identity.
            let reversed_removed: Vec<_> = removed.iter().rev().map(|&(a, b)| (b, a)).collect();
            let reversed_added: Vec<_> = added.iter().rev().map(|&(a, b)| (b, a)).collect();
            let other = ListKOptTopology::new(shift..shift + n, &reversed_removed, &reversed_added)
                .unwrap();
            assert_eq!(other.cycles(), t.cycles());
            for first in shift..shift + n {
                for successor in shift..shift + n {
                    let order = t.order_from(first, successor);
                    if !graph.contains(&edge(first - shift, successor - shift)) {
                        assert_eq!(order, Err(ListKOptError::FirstSuccessor));
                    } else if count != 1 {
                        assert_eq!(order, Err(ListKOptError::Disconnected));
                    } else {
                        let order = order.unwrap();
                        assert_eq!(&order[..2], &[first, successor]);
                        assert_eq!(
                            order.iter().copied().collect::<BTreeSet<_>>(),
                            (shift..shift + n).collect()
                        );
                        for i in 0..n {
                            assert!(
                                graph.contains(&edge(order[i] - shift, order[(i + 1) % n] - shift))
                            );
                        }
                        if first == shift {
                            let movement = p
                                .k_opt_move(&before, 0, &removed, &added, successor)
                                .unwrap();
                            let mut a = before.clone();
                            {
                                let trial = p.apply(&mut a, &movement).unwrap();
                                assert_eq!(
                                    trial.lists()[0],
                                    (0..shift).chain(order).collect::<Vec<_>>()
                                );
                            }
                            assert_eq!(a, before);
                        }
                    }
                }
            }
            let successor = graph
                .iter()
                .find_map(|&(a, b)| if a == 0 { Some(b + shift) } else { None })
                .unwrap();
            if count != 1 {
                assert_eq!(
                    p.k_opt_move(&before, 0, &removed, &added, successor),
                    Err(ListKOptError::Disconnected)
                );
            }
        }
    }
}

#[test]
fn count_matches_independent_endpoint_matchings() {
    fn enumerate(k: usize, partners: &mut [usize], used: &mut [bool]) -> u64 {
        let Some(a) = used.iter().position(|x| !x) else {
            // Fixed paths are (0,1), (2,3), ... . Alternate through a path and
            // proposed connector; a single cycle visits all endpoints.
            let mut v = 0;
            let mut visited = 0;
            loop {
                v = partners[v ^ 1];
                visited += 2;
                if v == 0 {
                    break;
                }
            }
            return u64::from(visited == 2 * k);
        };
        used[a] = true;
        let mut count = 0;
        for b in a + 1..used.len() {
            let is_original_cut = (a == 0 && b == 2 * k - 1) || (a % 2 == 1 && b == a + 1);
            if used[b] || b == a ^ 1 || is_original_cut {
                continue;
            }
            used[b] = true;
            partners[a] = b;
            partners[b] = a;
            count += enumerate(k, partners, used);
            used[b] = false;
        }
        used[a] = false;
        count
    }
    assert_eq!(pure_k_opt_topology_count(0), BigUint::from(0u8));
    assert_eq!(pure_k_opt_topology_count(1), BigUint::from(0u8));
    for k in 2..=7 {
        assert_eq!(
            pure_k_opt_topology_count(k),
            BigUint::from(enumerate(k, &mut vec![0; 2 * k], &mut vec![false; 2 * k]))
        );
    }
    assert!(pure_k_opt_topology_count(30) > BigUint::from(u128::MAX));
}

#[test]
fn circular_reversal_covers_all_boundaries_and_nonclone_payloads() {
    #[derive(Debug, PartialEq)]
    struct Value(usize);
    for n in 0..=30 {
        for start in 0..=n + 1 {
            for end in 0..=n + 1 {
                let mut actual: Vec<_> = (0..n).map(Value).collect();
                let result = reverse_cyclic_span(&mut actual, start, end);
                let mut expected: Vec<_> = (0..n).collect();
                if start > n || end > n {
                    assert_eq!(result, Err(CyclicSpanError { start, end, len: n }));
                } else {
                    result.unwrap();
                    let positions: Vec<_> = if start <= end {
                        (start..end).collect()
                    } else {
                        (start..n).chain(0..end).collect()
                    };
                    for (&to, &from) in positions.iter().zip(positions.iter().rev()) {
                        expected[to] = from;
                    }
                }
                assert_eq!(actual.iter().map(|v| v.0).collect::<Vec<_>>(), expected);
                if start <= n && end <= n {
                    reverse_cyclic_span(&mut actual, start, end).unwrap();
                    assert_eq!(
                        actual.iter().map(|v| v.0).collect::<Vec<_>>(),
                        (0..n).collect::<Vec<_>>()
                    );
                }
            }
        }
    }
}

#[test]
fn malformed_topology_is_rejected_without_panicking() {
    let removed = &[(0, 1), (2, 3)];
    assert_eq!(
        ListKOptTopology::new(usize::MAX..usize::MAX, removed, removed),
        Err(ListKOptError::TooFewValues)
    );
    let backwards = std::ops::Range { start: 6, end: 2 };
    assert_eq!(
        ListKOptTopology::new(backwards, removed, removed),
        Err(ListKOptError::TooFewValues)
    );
    assert_eq!(
        ListKOptTopology::new(0..6, removed, &[(0, 1)]),
        Err(ListKOptError::EdgeCount)
    );
    for bad in [&[(0, 0), (2, 3)][..], &[(0, usize::MAX), (2, 3)]] {
        assert_eq!(
            ListKOptTopology::new(0..6, bad, removed),
            Err(ListKOptError::InvalidEdge)
        );
    }
    assert_eq!(
        ListKOptTopology::new(0..6, &[(0, 1), (1, 0)], removed),
        Err(ListKOptError::DuplicateEdge)
    );
    assert_eq!(
        ListKOptTopology::new(0..6, &[(0, 2), (2, 3)], removed),
        Err(ListKOptError::InvalidEdge)
    );
    assert_eq!(
        ListKOptTopology::new(0..6, removed, &[(0, 5), (1, 3)]),
        Err(ListKOptError::DuplicateEdge)
    );
    assert_eq!(
        ListKOptTopology::new(0..6, removed, &[(0, 2), (0, 3)]),
        Err(ListKOptError::Degree)
    );
    let t = ListKOptTopology::new(0..6, removed, removed).unwrap();
    assert_eq!(
        t.order_from(usize::MAX, 0),
        Err(ListKOptError::FirstSuccessor)
    );
    assert_eq!(
        t.order_from(0, usize::MAX),
        Err(ListKOptError::FirstSuccessor)
    );
}
