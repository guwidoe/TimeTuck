//! Original exhaustive shared-dependency scheduling regressions.
use std::collections::BTreeSet;
use timetuck::*;
#[test]
fn all_three_node_graphs_and_partitions_match_independent_reachability() {
    let g = DependencyGraph::new((0..3).map(|i| (i, vec![]))).unwrap();
    for roots in [[0, 1, 2], [0, 0, 2], [0, 1, 0], [0, 1, 1], [0, 0, 0]] {
        let alignment = (0..3)
            .filter(|&i| roots[i] != i)
            .map(|i| (i, roots[i]))
            .collect::<Vec<_>>();
        for mask in 0..512 {
            let edges = (0..3)
                .flat_map(|i| {
                    (0..3)
                        .filter(move |j| mask & (1 << (i * 3 + j)) != 0)
                        .map(move |j| (i, j))
                })
                .collect::<Vec<_>>();
            let mut reach = [[false; 3]; 3];
            for &(i, j) in &edges {
                reach[roots[i]][roots[j]] = true;
            }
            let direct = reach;
            for k in 0..3 {
                for i in 0..3 {
                    for j in 0..3 {
                        reach[i][j] |= reach[i][k] && reach[k][j];
                    }
                }
            }
            let mut called = BTreeSet::new();
            let values = g
                .evaluate_aligned(&edges, &alignment, |members, inputs| {
                    assert!(members.windows(2).all(|w| w[0] < w[1]));
                    assert!(called.insert(members[0]));
                    assert!(inputs.windows(2).all(|w| w[0].0 < w[1].0));
                    Some(
                        1 + inputs
                            .iter()
                            .map(|(_, v)| v.value.unwrap())
                            .max()
                            .unwrap_or(0),
                    )
                })
                .unwrap();
            let mut expected = [None; 3];
            for _ in 0..3 {
                for i in 0..3 {
                    if roots[i] == i
                        && (0..3)
                            .filter(|&j| direct[i][j])
                            .all(|j| expected[j].is_some())
                    {
                        expected[i] = Some(
                            1 + (0..3)
                                .filter(|&j| direct[i][j])
                                .filter_map(|j| expected[j])
                                .max()
                                .unwrap_or(0),
                        );
                    }
                }
            }
            for i in 0..3 {
                let r = roots[i];
                let bad = (0..3).any(|j| reach[j][j] && (r == j || reach[r][j]));
                assert_eq!(values[&i].inconsistent, bad, "{roots:?} {mask} {i}");
                assert_eq!(values[&i].value, if bad { None } else { expected[r] });
                assert_eq!(called.contains(&r), !bad);
            }
        }
    }
}
#[test]
fn fixed_dependency_collapse_cycles_recover_and_alignment_is_transitive() {
    let g =
        DependencyGraph::new(vec![(0, vec![]), (1, vec![0]), (2, vec![1]), (3, vec![])]).unwrap();
    let values = g
        .evaluate_aligned::<i32>(&[], &[(2, 1), (0, 2), (2, 1), (1, 1)], |members, _| {
            assert_eq!(members, &[3]);
            Some(9)
        })
        .unwrap();
    for i in 0..3 {
        assert!(values[&i].inconsistent);
    }
    assert_eq!(values[&3].value, Some(9));
    assert!(
        g.evaluate_aligned(&[], &[], |_, _| Some(7))
            .unwrap()
            .values()
            .all(|v| v.value == Some(7) && !v.inconsistent)
    );
}
#[test]
fn invalid_metadata_fails_before_callbacks_and_none_remains_consistent() {
    let g = DependencyGraph::new(vec![(0, vec![]), (1, vec![]), (2, vec![0, 1])]).unwrap();
    for (edges, alignment) in [
        (vec![], vec![(0, 3)]),
        (vec![], vec![(3, 0)]),
        (vec![(3, 0)], vec![]),
        (vec![(2, 3)], vec![(0, 1)]),
    ] {
        assert!(
            g.evaluate_aligned::<i32>(&edges, &alignment, |_, _| panic!(
                "invalid metadata callback"
            ))
            .is_err()
        );
    }
    let values = g
        .evaluate_aligned::<i32>(&[], &[(1, 0)], |members, inputs| {
            if members == [0, 1] {
                assert!(inputs.is_empty());
                None
            } else {
                assert_eq!(inputs.len(), 1);
                assert_eq!(*inputs[0].0, 0);
                assert!(!inputs[0].1.inconsistent);
                assert_eq!(inputs[0].1.value, None);
                Some(8)
            }
        })
        .unwrap();
    assert_eq!(values[&0], values[&1]);
    assert!(!values[&0].inconsistent);
    assert_eq!(values[&2].value, Some(8));
    assert!(
        DependencyGraph::<usize>::new([])
            .unwrap()
            .evaluate_aligned::<i32>(&[], &[], |_, _| panic!("empty"))
            .unwrap()
            .is_empty()
    );
}
#[test]
fn callback_unwind_and_large_alignment_leave_plan_reusable() {
    let g = DependencyGraph::new((0..10000).map(|i| (i, vec![]))).unwrap();
    let alignment = (1..10000).rev().map(|i| (i, i - 1)).collect::<Vec<_>>();
    assert!(
        std::panic::catch_unwind(
            || g.evaluate_aligned::<i32>(&[], &alignment, |_, _| panic!("test"))
        )
        .is_err()
    );
    let mut calls = 0;
    let values = g
        .evaluate_aligned(&[], &alignment, |members, _| {
            calls += 1;
            assert_eq!(members.len(), 10000);
            Some(vec![1, 2])
        })
        .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(values[&9999].value, Some(vec![1, 2]));
}
