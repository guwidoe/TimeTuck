use timetuck::{DependencyError, DependencyTopology};

fn reachability(n: usize, edges: &[(usize, usize)]) -> Vec<Vec<bool>> {
    let mut reach = vec![vec![false; n]; n];
    for &(i, j) in edges {
        reach[i][j] = true;
    }
    for k in 0..n {
        for i in 0..n {
            for j in 0..n {
                reach[i][j] |= reach[i][k] && reach[k][j];
            }
        }
    }
    reach
}
fn verify(graph: &DependencyTopology<usize>, edges: &[(usize, usize)]) -> Vec<bool> {
    let n = graph.keys().len();
    let reach = reachability(n, edges);
    for (i, row) in reach.iter().enumerate() {
        let expected: Vec<_> = (0..n)
            .filter(|&j| i == j || (reach[i][j] && reach[j][i]))
            .collect();
        assert_eq!(graph.component(&i).unwrap(), expected);
        assert_eq!(graph.is_cyclic(&i).unwrap(), reach[i][i]);
        for (j, &reachable) in row.iter().enumerate() {
            if expected.contains(&j) {
                assert_eq!(graph.rank(&i).unwrap(), graph.rank(&j).unwrap());
            } else if reachable {
                assert!(graph.rank(&i).unwrap() > graph.rank(&j).unwrap());
            }
        }
        let mut expected: Vec<_> = edges
            .iter()
            .filter_map(|&(a, b)| (a == i).then_some(b))
            .collect();
        expected.sort_unstable();
        expected.dedup();
        assert_eq!(
            graph
                .dependencies(&i)
                .unwrap()
                .into_iter()
                .copied()
                .collect::<Vec<_>>(),
            expected
        );
    }
    (0..n).map(|i| reach[i][i]).collect()
}
#[test]
fn all_three_node_graphs_and_each_single_edge_toggle() {
    for mask in 0..512 {
        let edges: Vec<_> = (0..9)
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| (i / 3, i % 3))
            .collect();
        let mut graph = DependencyTopology::new([2, 0, 1]).unwrap();
        let changed = graph.update(&[], &edges).unwrap();
        let before = verify(&graph, &edges);
        assert_eq!(changed, (0..3).filter(|&i| before[i]).collect::<Vec<_>>());
        for bit in 0..9 {
            let edge = (bit / 3, bit % 3);
            let mut trial = graph.clone();
            let mut next_edges = edges.clone();
            let changed = if mask & (1 << bit) == 0 {
                next_edges.push(edge);
                trial.update(&[], &[edge]).unwrap()
            } else {
                next_edges.retain(|&e| e != edge);
                trial.update(&[edge], &[]).unwrap()
            };
            let after = verify(&trial, &next_edges);
            assert_eq!(
                changed,
                (0..3)
                    .filter(|&i| before[i] != after[i])
                    .collect::<Vec<_>>()
            );
            // Querying and modifying a trial never changes the saved snapshot.
            assert_eq!(verify(&graph, &edges), before);
        }
    }
}
#[test]
fn atomic_validation_empty_graph_and_idempotent_batches() {
    assert_eq!(
        DependencyTopology::new([1, 1]),
        Err(DependencyError::DuplicateNode(1))
    );
    let mut empty = DependencyTopology::<usize>::new([]).unwrap();
    assert!(empty.update(&[], &[]).unwrap().is_empty());
    assert_eq!(empty.rank(&0), Err(DependencyError::UnknownNode(0)));
    assert_eq!(empty.component(&0), Err(DependencyError::UnknownNode(0)));
    assert_eq!(empty.dependencies(&0), Err(DependencyError::UnknownNode(0)));
    assert_eq!(empty.is_cyclic(&0), Err(DependencyError::UnknownNode(0)));
    let mut graph = DependencyTopology::new(["c", "a", "b"]).unwrap();
    assert_eq!(graph.keys(), ["a", "b", "c"]);
    graph.update(&[], &[("b", "a"), ("c", "b")]).unwrap();
    let saved = graph.clone();
    assert_eq!(
        graph.update(&[("b", "a")], &[("a", "b"), ("c", "z")]),
        Err(DependencyError::UnknownDependency {
            node: "c",
            dependency: "z"
        })
    );
    assert_eq!(graph, saved);
    assert_eq!(
        graph.update(&[("z", "a")], &[("a", "b")]),
        Err(DependencyError::UnknownNode("z"))
    );
    assert_eq!(graph, saved);
    assert_eq!(
        graph.update(&[("c", "z")], &[]),
        Err(DependencyError::UnknownDependency {
            node: "c",
            dependency: "z"
        })
    );
    assert_eq!(graph, saved);
    assert!(
        graph
            .update(&[("a", "a")], &[("b", "a"), ("b", "a")])
            .unwrap()
            .is_empty()
    );
    assert_eq!(graph, saved);
    // Remove then add the same edge is one unchanged batch, not two events.
    assert!(
        graph
            .update(&[("b", "a")], &[("b", "a")])
            .unwrap()
            .is_empty()
    );
    assert_eq!(graph, saved);
    assert_eq!(graph.update(&[], &[("a", "a")]).unwrap(), ["a"]);
    assert!(!graph.is_cyclic(&"b").unwrap());
    assert!(!graph.is_cyclic(&"c").unwrap());
    assert_eq!(graph.update(&[("a", "a")], &[]).unwrap(), ["a"]);
}
#[test]
fn deep_chain_cycle_and_recovery_without_recursive_stack() {
    let n = 20_000;
    let mut graph = DependencyTopology::new(0..n).unwrap();
    let edges: Vec<_> = (1..n).map(|i| (i, i - 1)).collect();
    assert!(graph.update(&[], &edges).unwrap().is_empty());
    assert!(graph.rank(&0).unwrap() < graph.rank(&(n - 1)).unwrap());
    assert_eq!(
        graph.update(&[], &[(0, n - 1)]).unwrap(),
        (0..n).collect::<Vec<_>>()
    );
    assert_eq!(graph.component(&0).unwrap().len(), n);
    assert_eq!(
        graph.update(&[(0, n - 1)], &[]).unwrap(),
        (0..n).collect::<Vec<_>>()
    );
    assert_eq!(graph.component(&(n - 1)).unwrap(), [n - 1]);
}
