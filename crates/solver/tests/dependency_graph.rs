//! Original graph invariants; no upstream method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn reachability(mask: usize) -> [[bool; 3]; 3] {
    let mut reach = [[false; 3]; 3];
    for (i, row) in reach.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = mask & (1 << (i * 3 + j)) != 0;
        }
    }
    for k in 0..3 {
        for i in 0..3 {
            for j in 0..3 {
                reach[i][j] |= reach[i][k] && reach[k][j];
            }
        }
    }
    reach
}
#[test]
fn all_three_node_fixed_graphs_report_exact_cycle_members() {
    for mask in 0..512 {
        let nodes = (0..3).map(|i| {
            (
                i,
                (0..3).filter(|j| mask & (1 << (i * 3 + j)) != 0).collect(),
            )
        });
        let expected: Vec<_> = (0..3).filter(|&i| reachability(mask)[i][i]).collect();
        match DependencyGraph::new(nodes) {
            Ok(_) => assert!(expected.is_empty()),
            Err(DependencyError::FixedCycle(actual)) => assert_eq!(actual, expected, "mask {mask}"),
            other => panic!("{other:?}"),
        }
    }
}
#[test]
fn all_dynamic_graphs_mark_cycles_and_downstream_dependents_and_recover() {
    let g = DependencyGraph::new((0..3).map(|i| (i, vec![]))).unwrap();
    for mask in 0..512 {
        let links: Vec<_> = (0..3)
            .flat_map(|i| {
                (0..3)
                    .filter(move |j| mask & (1 << (i * 3 + j)) != 0)
                    .map(move |j| (i, j))
            })
            .collect();
        let reach = reachability(mask);
        let calls = Cell::new(0);
        let result = g
            .evaluate(&links, |_, inputs| {
                calls.set(calls.get() + 1);
                Some(1 + inputs.iter().map(|(_, v)| v.value.unwrap()).sum::<i32>())
            })
            .unwrap();
        let mut expected_calls = 0;
        for i in 0..3 {
            let blocked = (0..3).any(|c| reach[c][c] && (i == c || reach[i][c]));
            assert_eq!(result[&i].inconsistent, blocked, "mask {mask}, node {i}");
            assert_eq!(result[&i].value.is_none(), blocked);
            expected_calls += usize::from(!blocked);
        }
        assert_eq!(calls.get(), expected_calls);
        assert!(
            g.evaluate(&[], |_, _| Some(1))
                .unwrap()
                .values()
                .all(|v| !v.inconsistent && v.value == Some(1))
        );
    }
}
#[test]
fn missing_values_are_distinct_from_inconsistency_and_inputs_are_sorted_unique() {
    let g = DependencyGraph::new(vec![(3, vec![2, 1, 2]), (1, vec![]), (2, vec![1])]).unwrap();
    let mut order = vec![];
    let result = g
        .evaluate(&[(3, 1), (3, 2)], |key, inputs| {
            order.push(*key);
            assert!(inputs.windows(2).all(|w| w[0].0 < w[1].0));
            if *key == 1 {
                None
            } else {
                assert!(inputs.iter().all(|(_, v)| !v.inconsistent));
                Some(9)
            }
        })
        .unwrap();
    assert_eq!(order, vec![1, 2, 3]);
    assert_eq!(
        result[&1],
        DerivedValue {
            value: None,
            inconsistent: false
        }
    );
    assert_eq!(result[&3].value, Some(9));
}
#[test]
fn invalid_metadata_and_dynamic_edges_fail_before_callbacks() {
    assert_eq!(
        DependencyGraph::new(vec![(1, vec![]), (1, vec![])]).unwrap_err(),
        DependencyError::DuplicateNode(1)
    );
    assert_eq!(
        DependencyGraph::new(vec![(1, vec![2])]).unwrap_err(),
        DependencyError::UnknownDependency {
            node: 1,
            dependency: 2
        }
    );
    let g = DependencyGraph::new(vec![(1, vec![]), (2, vec![])]).unwrap();
    let calls = Cell::new(0);
    for links in [vec![(1, 2), (3, 1)], vec![(1, 2), (2, 3)]] {
        assert!(
            g.evaluate(&links, |_, _| {
                calls.set(calls.get() + 1);
                Some(0)
            })
            .is_err()
        );
    }
    assert_eq!(calls.get(), 0);
    assert_eq!(
        DependencyGraph::<usize>::new([])
            .unwrap()
            .evaluate::<i32>(&[], |_, _| panic!("empty"))
            .unwrap()
            .len(),
        0
    );
}
#[test]
fn long_chains_use_no_recursive_traversal_and_support_nonclone_values() {
    struct Owned(usize);
    let g =
        DependencyGraph::new((0..20000).map(|i| (i, if i == 0 { vec![] } else { vec![i - 1] })))
            .unwrap();
    let values = g
        .evaluate::<Owned>(&[], |_, inputs| {
            Some(Owned(
                inputs
                    .first()
                    .map_or(0, |(_, v)| v.value.as_ref().unwrap().0 + 1),
            ))
        })
        .unwrap();
    assert_eq!(values[&19999].value.as_ref().unwrap().0, 19999);
}
#[test]
fn callback_unwind_does_not_mutate_reusable_fixed_plan() {
    let g = DependencyGraph::new(vec![(0, vec![]), (1, vec![0])]).unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| g.evaluate(&[], |id, _| {
            assert_eq!(*id, 0);
            Some(7)
        })))
        .is_err()
    );
    assert_eq!(g.evaluate(&[], |_, _| Some(8)).unwrap()[&1].value, Some(8));
}
