// Adapted from Timefold Community AbstractTopologicalGraphTest, Apache-2.0.
// Full node/edge permutation sets and all seven method assertions are retained.
// See compat/provenance.json. Native edges reverse the source orientation.
use timetuck::DependencyTopology;

fn permutations<T: Clone>(source: &[T], limit: usize) -> Vec<Vec<T>> {
    if source.is_empty() {
        return vec![vec![]];
    }
    if limit <= 1 {
        return vec![source.to_vec()];
    }
    let tails = permutations(&source[1..], limit / source.len());
    tails
        .into_iter()
        .flat_map(|tail| {
            (0..=tail.len()).map(move |i| {
                let mut out = tail.clone();
                out.insert(i, source[0].clone());
                out
            })
        })
        .collect()
}
// Test-only reproduction of the documented java.util.Random sequence and
// Collections.shuffle traversal. Compared against the Java helper's complete
// permutation-stream checksums; this is not the solver's randomness source.
fn shuffle<T>(values: &mut [T], seed: u64) {
    let mut state = (seed ^ 0x5deece66d) & ((1 << 48) - 1);
    for n in (2..=values.len()).rev() {
        let j = loop {
            state = state.wrapping_mul(0x5deece66d).wrapping_add(11) & ((1 << 48) - 1);
            let bits = state >> 17;
            if n.is_power_of_two() {
                break ((bits * n as u64) >> 31) as usize;
            }
            let value = bits % n as u64;
            if bits - value + (n as u64 - 1) < (1 << 31) {
                break value as usize;
            }
        };
        values.swap(n - 1, j);
    }
}
fn order(graph: &DependencyTopology<usize>, map: &[usize], groups: &[&[usize]]) {
    for (i, group) in groups.iter().enumerate() {
        for earlier in &groups[..i] {
            for &a in *group {
                for &b in *earlier {
                    assert!(graph.rank(&map[a]).unwrap() > graph.rank(&map[b]).unwrap());
                }
            }
        }
    }
}
fn members(graph: &DependencyTopology<usize>, map: &[usize], groups: &[&[usize]]) {
    for group in groups {
        let mut expected: Vec<_> = group.iter().map(|&i| map[i]).collect();
        expected.sort_unstable();
        for &i in *group {
            assert_eq!(graph.component(&map[i]).unwrap(), expected);
        }
    }
}
fn changes(actual: &[usize], map: &[usize], expected: &[usize]) {
    let mut expected: Vec<_> = expected.iter().map(|&i| map[i]).collect();
    expected.sort_unstable();
    assert_eq!(actual, expected);
}
fn run(case: usize) {
    let size = if case < 4 { 5 } else { 6 };
    let input: Vec<Vec<usize>> = match case {
        0 | 1 => vec![vec![1, 2], vec![3], vec![3], vec![4], vec![]],
        2 | 3 => vec![vec![1, 2], vec![3], vec![3], vec![4, 1], vec![]],
        4 | 5 => vec![
            vec![1, 2],
            vec![3, 2],
            vec![4],
            vec![5, 1],
            vec![5, 2, 3],
            vec![],
        ],
        6 => vec![
            vec![1, 2],
            vec![2],
            vec![4, 3],
            vec![4, 5],
            vec![5, 1],
            vec![],
        ],
        _ => unreachable!(),
    };
    let mut count = 0;
    let mut hash = 0xcbf29ce484222325u64;
    for map in permutations(&(0..size).collect::<Vec<_>>(), usize::MAX) {
        let mut edges: Vec<_> = input
            .iter()
            .enumerate()
            .flat_map(|(from, tos)| {
                let map = &map;
                tos.iter().map(move |&to| (map[from], map[to]))
            })
            .collect();
        if case >= 4 {
            shuffle(&mut edges, count);
        }
        for perm in permutations(&edges, if case < 4 { usize::MAX } else { 1000 }) {
            for &(a, b) in &perm {
                for x in [a, b] {
                    hash = (hash ^ x as u64).wrapping_mul(0x100000001b3);
                }
            }
            let mut graph = DependencyTopology::new(0..size).unwrap();
            let native: Vec<_> = perm.iter().map(|&(from, to)| (to, from)).collect();
            let changed = graph.update(&[], &native).unwrap();
            match case {
                0 => {
                    order(&graph, &map, &[&[0], &[1, 2], &[3], &[4]]);
                    changes(&changed, &map, &[]);
                }
                1 => {
                    assert!(changed.is_empty());
                    let changed = graph.update(&[(map[3], map[2])], &[]).unwrap();
                    order(&graph, &map, &[&[0], &[1], &[3], &[4]]);
                    order(&graph, &map, &[&[0], &[2]]);
                    changes(&changed, &map, &[]);
                }
                2 => {
                    order(&graph, &map, &[&[0], &[1, 2, 3], &[4]]);
                    members(&graph, &map, &[&[0], &[1, 3], &[2], &[4]]);
                    changes(&changed, &map, &[1, 3]);
                }
                3 => {
                    let changed = graph.update(&[(map[1], map[3])], &[]).unwrap();
                    order(&graph, &map, &[&[0], &[1, 2], &[3], &[4]]);
                    members(&graph, &map, &[&[0], &[1], &[2], &[3], &[4]]);
                    changes(&changed, &map, &[1, 3]);
                }
                4 => {
                    order(&graph, &map, &[&[0], &[1, 2, 3, 4], &[5]]);
                    members(&graph, &map, &[&[0], &[1, 2, 3, 4], &[5]]);
                    changes(&changed, &map, &[1, 2, 3, 4]);
                }
                5 => {
                    let changed = graph.update(&[(map[3], map[4])], &[]).unwrap();
                    order(&graph, &map, &[&[0], &[1, 2, 3, 4], &[5]]);
                    members(&graph, &map, &[&[0], &[1, 3], &[2, 4], &[5]]);
                    changes(&changed, &map, &[]);
                }
                6 => {
                    let changed = graph.update(&[(map[4], map[2])], &[]).unwrap();
                    order(&graph, &map, &[&[0], &[1, 2, 3, 4], &[5]]);
                    members(&graph, &map, &[&[0], &[1, 2, 3, 4], &[5]]);
                    changes(&changed, &map, &[]);
                }
                _ => unreachable!(),
            }
            count += 1;
        }
    }
    // Captured from TopologyPermutationOracle using the pinned test helpers and
    // the JDK's actual Collections.shuffle / Random. No topology implementation.
    let expected = include_str!("fixtures/topology-permutations.txt");
    let row: Vec<_> = expected
        .lines()
        .nth(case)
        .unwrap()
        .split_whitespace()
        .collect();
    assert_eq!(count, row[1].parse::<u64>().unwrap());
    assert_eq!(hash, u64::from_str_radix(row[2], 16).unwrap());
}
#[test]
fn no_loops() {
    run(0);
}
#[test]
fn no_loops_remove_edge() {
    run(1);
}
#[test]
fn loops() {
    run(2);
}
#[test]
fn loop_remove_edge_in_loop() {
    run(3);
}
#[test]
fn connecting_two_loops() {
    run(4);
}
#[test]
fn connecting_two_loops_remove_joining_edge() {
    run(5);
}
#[test]
fn connecting_two_loops_remove_non_joining_edge() {
    run(6);
}
