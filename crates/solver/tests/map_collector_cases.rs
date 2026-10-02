// Adapted from Timefold Community 2.7.0 Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
use std::collections::{BTreeMap, BTreeSet};
use timetuck::*;

type Sets = MapCollector<i64, i64, SetCollector<i64>>;
type Sums = MapCollector<i64, i64, IntCollector>;
fn sets(c: &Sets) -> BTreeMap<i64, BTreeSet<i64>> {
    c.finish()
        .iter()
        .map(|(&k, v)| (k, v.keys().copied().collect()))
        .collect()
}
fn expected_sets(pairs: &[(i64, &[i64])]) -> BTreeMap<i64, BTreeSet<i64>> {
    pairs
        .iter()
        .map(|(k, v)| (*k, v.iter().copied().collect()))
        .collect()
}
fn set_sequence<const N: usize>(inputs: [[i64; N]; 3], map: impl Fn([i64; N]) -> (i64, i64)) {
    let mut c = Sets::default();
    let [first, second, third] = inputs.map(map);
    assert!(sets(&c).is_empty());
    c.insert(&first);
    assert_eq!(sets(&c), expected_sets(&[(2, &[2])]));
    c.insert(&second);
    assert_eq!(sets(&c), expected_sets(&[(2, &[2]), (1, &[1])]));
    c.insert(&third);
    assert_eq!(sets(&c), expected_sets(&[(2, &[2]), (1, &[1])]));
    assert!(c.remove(&second));
    assert_eq!(sets(&c), expected_sets(&[(2, &[2]), (1, &[1])]));
    assert!(c.remove(&third));
    assert_eq!(sets(&c), expected_sets(&[(2, &[2])]));
    assert!(c.remove(&first));
    assert!(sets(&c).is_empty());
}
fn sum_sequence<const N: usize>(
    inputs: [[i64; N]; 3],
    map: impl FnMut([i64; N]) -> (i64, i64),
    keys: [i64; 2],
    sums: [i64; 3],
) {
    let mut c = Sums::default();
    let [first, second, third] = inputs.map(map);
    let read = |c: &Sums| {
        c.finish()
            .iter()
            .map(|(&k, s)| (k, s.sum))
            .collect::<BTreeMap<_, _>>()
    };
    let [a, b] = keys;
    let [x, y, z] = sums;
    assert!(read(&c).is_empty());
    c.insert(&first);
    assert_eq!(read(&c), BTreeMap::from([(a, x)]));
    c.insert(&second);
    assert_eq!(read(&c), BTreeMap::from([(a, x), (b, y)]));
    c.insert(&third);
    assert_eq!(read(&c), BTreeMap::from([(a, x), (b, y + z)]));
    assert!(c.remove(&second));
    assert_eq!(read(&c), BTreeMap::from([(a, x), (b, z)]));
    assert!(c.remove(&third));
    assert_eq!(read(&c), BTreeMap::from([(a, x)]));
    assert!(c.remove(&first));
    assert!(read(&c).is_empty());
}
// Native captured input IDs replace Java slot update/retractor plumbing. An empty
// container is observed as an empty map; this does not create an empty stream row.
fn updates<const N: usize, C: ReversibleCollector<i64>>(
    inputs: [[i64; N]; 3],
    map: impl Fn([i64; N]) -> (i64, i64),
    read: impl Fn(C::Output) -> i64,
) {
    let mut node = CollectNode::<usize, (), (i64, i64), MapCollector<i64, i64, C>>::new();
    let [first, second, replacement] = inputs.map(map);
    let snapshot = |n: &CollectNode<usize, (), (i64, i64), MapCollector<i64, i64, C>>| {
        n.groups()
            .next()
            .map(|(_, m)| {
                m.iter()
                    .map(|(&k, v)| (k, read(v.clone())))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default()
    };
    node.apply(TupleChange::put(0, ((), first)));
    node.apply(TupleChange::put(1, ((), second)));
    assert_eq!(snapshot(&node), BTreeMap::from([(2, 2), (1, 1)]));
    node.apply(TupleChange::put(0, ((), replacement)));
    assert_eq!(snapshot(&node), BTreeMap::from([(1, 1), (3, 3)]));
    node.apply(TupleChange::put(0, ((), replacement)));
    assert_eq!(snapshot(&node), BTreeMap::from([(1, 1), (3, 3)]));
    node.apply(TupleChange::retract(1));
    assert_eq!(snapshot(&node), BTreeMap::from([(3, 3)]));
    node.apply(TupleChange::retract(0));
    assert!(snapshot(&node).is_empty());
}
fn singleton(v: SnapshotMap<i64, ()>) -> i64 {
    assert_eq!(v.len(), 1);
    *v.keys().next().unwrap()
}

#[test]
fn uni_to_map() {
    set_sequence([[2], [1], [1]], |v| {
        let s = v.into_iter().sum();
        (s, s)
    });
}

#[test]
fn uni_to_map_update() {
    updates::<1, SetCollector<i64>>(
        [[2], [1], [3]],
        |v| {
            let s = v.into_iter().sum();
            (s, s)
        },
        singleton,
    );
}

#[test]
fn uni_to_sorted_map() {
    set_sequence([[2], [1], [1]], |v| {
        let s = v.into_iter().sum();
        (s, s)
    });
}

#[test]
fn uni_to_sorted_map_update() {
    updates::<1, SetCollector<i64>>(
        [[2], [1], [3]],
        |v| {
            let s = v.into_iter().sum();
            (s, s)
        },
        singleton,
    );
}

#[test]
fn uni_to_map_merged_update() {
    updates::<1, IntCollector>(
        [[2], [1], [3]],
        |v| {
            let s = v.into_iter().sum();
            (s, s)
        },
        |v| v.sum,
    );
}

#[test]
fn bi_to_map() {
    set_sequence([[2, 0], [1, 0], [1, 0]], |v| {
        let s = v.into_iter().sum();
        (s, s)
    });
}

#[test]
fn bi_to_map_update() {
    updates::<2, SetCollector<i64>>(
        [[2, 0], [1, 0], [3, 0]],
        |v| {
            let s = v.into_iter().sum();
            (s, s)
        },
        singleton,
    );
}

#[test]
fn bi_to_sorted_map() {
    set_sequence([[2, 0], [1, 0], [1, 0]], |v| {
        let s = v.into_iter().sum();
        (s, s)
    });
}

#[test]
fn bi_to_sorted_map_update() {
    updates::<2, SetCollector<i64>>(
        [[2, 0], [1, 0], [3, 0]],
        |v| {
            let s = v.into_iter().sum();
            (s, s)
        },
        singleton,
    );
}

#[test]
fn bi_to_map_merged_update() {
    updates::<2, IntCollector>(
        [[2, 0], [1, 0], [3, 0]],
        |v| {
            let s = v.into_iter().sum();
            (s, s)
        },
        |v| v.sum,
    );
}

#[test]
fn tri_to_map() {
    set_sequence([[2, 0, 0], [1, 0, 0], [1, 0, 0]], |v| {
        let s = v.into_iter().sum();
        (s, s)
    });
}

#[test]
fn tri_to_map_update() {
    updates::<3, SetCollector<i64>>(
        [[2, 0, 0], [1, 0, 0], [3, 0, 0]],
        |v| {
            let s = v.into_iter().sum();
            (s, s)
        },
        singleton,
    );
}

#[test]
fn tri_to_sorted_map() {
    set_sequence([[2, 0, 0], [1, 0, 0], [1, 0, 0]], |v| {
        let s = v.into_iter().sum();
        (s, s)
    });
}

#[test]
fn tri_to_sorted_map_update() {
    updates::<3, SetCollector<i64>>(
        [[2, 0, 0], [1, 0, 0], [3, 0, 0]],
        |v| {
            let s = v.into_iter().sum();
            (s, s)
        },
        singleton,
    );
}

#[test]
fn tri_to_map_merged_update() {
    updates::<3, IntCollector>(
        [[2, 0, 0], [1, 0, 0], [3, 0, 0]],
        |v| {
            let s = v.into_iter().sum();
            (s, s)
        },
        |v| v.sum,
    );
}

#[test]
fn quad_to_map() {
    set_sequence([[2, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0]], |v| {
        let s = v.into_iter().sum();
        (s, s)
    });
}

#[test]
fn quad_to_map_update() {
    updates::<4, SetCollector<i64>>(
        [[2, 0, 0, 0], [1, 0, 0, 0], [3, 0, 0, 0]],
        |v| {
            let s = v.into_iter().sum();
            (s, s)
        },
        singleton,
    );
}

#[test]
fn quad_to_sorted_map() {
    set_sequence([[2, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0]], |v| {
        let s = v.into_iter().sum();
        (s, s)
    });
}

#[test]
fn quad_to_sorted_map_update() {
    updates::<4, SetCollector<i64>>(
        [[2, 0, 0, 0], [1, 0, 0, 0], [3, 0, 0, 0]],
        |v| {
            let s = v.into_iter().sum();
            (s, s)
        },
        singleton,
    );
}

#[test]
fn quad_to_map_merged_update() {
    updates::<4, IntCollector>(
        [[2, 0, 0, 0], [1, 0, 0, 0], [3, 0, 0, 0]],
        |v| {
            let s = v.into_iter().sum();
            (s, s)
        },
        |v| v.sum,
    );
}

#[test]
fn uni_to_map_merged() {
    let mut counter = 0;
    sum_sequence(
        [[2], [1], [1]],
        |[key]| {
            counter += 1;
            (key, if counter % 2 == 1 { key } else { key + 1 })
        },
        [2, 1],
        [2, 2, 1],
    );
    assert_eq!(counter, 3);
}
#[test]
fn bi_to_map_merged() {
    sum_sequence([[2, 1], [1, 1], [1, 2]], |[a, b]| (a, b), [2, 1], [1, 1, 2]);
}
#[test]
fn tri_to_map_merged() {
    sum_sequence(
        [[2, 1, 2], [1, 1, 2], [1, 2, 3]],
        |[a, b, c]| (a, b + c),
        [2, 1],
        [3, 3, 5],
    );
}
#[test]
fn quad_to_map_merged() {
    sum_sequence(
        [[2, 1, 3, 2], [1, 3, 3, 2], [1, 3, 3, 1]],
        |[a, b, c, d]| (a + b, c + d),
        [3, 4],
        [5, 5, 4],
    );
}
#[test]
fn bi_to_map_duplicates() {
    let mut c = MapCollector::<&str, i64, SetCollector<i64>>::default();
    let read = |c: &MapCollector<&str, i64, SetCollector<i64>>| {
        c.finish()
            .iter()
            .map(|(&k, v)| (k.to_string(), v.keys().copied().collect::<BTreeSet<_>>()))
            .collect::<BTreeMap<_, _>>()
    };
    let expected = |pairs: &[(&str, &[i64])]| {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.iter().copied().collect::<BTreeSet<_>>()))
            .collect::<BTreeMap<_, _>>()
    };
    assert!(read(&c).is_empty());
    c.insert(&("A", 1));
    assert_eq!(read(&c), expected(&[("A", &[1])]));
    c.insert(&("B", 1));
    assert_eq!(read(&c), expected(&[("A", &[1]), ("B", &[1])]));
    c.insert(&("B", 2));
    assert_eq!(read(&c), expected(&[("A", &[1]), ("B", &[1, 2])]));
    assert!(c.remove(&("B", 1)));
    assert_eq!(read(&c), expected(&[("A", &[1]), ("B", &[2])]));
    assert!(c.remove(&("B", 2)));
    assert_eq!(read(&c), expected(&[("A", &[1])]));
    assert!(c.remove(&("A", 1)));
    assert!(read(&c).is_empty());
}
