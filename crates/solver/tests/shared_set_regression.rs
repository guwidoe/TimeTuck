//! Original runtime comparisons; no upstream method credit.
use std::{
    cell::Cell,
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn shared_concat_and_distinct_match_multiset_and_set_rebuilds() {
    let concat = SharedConcat::new();
    let distinct = SharedDistinct::new();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let l = b.source::<usize, i32>();
    let r = b.source::<usize, i32>();
    let out = concat.apply(&mut b, l.port(), r.port()).unwrap();
    assert!(out.same_node(&concat.clone().apply(&mut b, l.port(), r.port()).unwrap()));
    let dedup = distinct.apply(&mut b, &out).unwrap();
    assert!(dedup.same_node(&distinct.clone().apply(&mut b, &out).unwrap()));
    let all = b.dataset(&out).unwrap();
    let unique = b.dataset(&dedup).unwrap();
    for name in ["a", "b"] {
        b.score(format!("all{name}"), &out, |v| SimpleScore::new(*v as i64))
            .unwrap();
        b.score(format!("unique{name}"), &dedup, |v| {
            SimpleScore::new(*v as i64)
        })
        .unwrap();
    }
    let mut g = b.build();
    let mut left = BTreeMap::new();
    let mut right = BTreeMap::new();
    let mut seed = 56789u64;
    for step in 0usize..800 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let id = (seed >> 32) as usize % 11;
        let value = (!step.is_multiple_of(4)).then_some(((seed >> 16) % 7) as i32 - 3);
        let mut nl = left.clone();
        let mut nr = right.clone();
        let target = if step.is_multiple_of(2) {
            &mut nl
        } else {
            &mut nr
        };
        if let Some(v) = value {
            target.insert(id, v);
        } else {
            target.remove(&id);
        }
        let expected: BTreeMap<_, _> = nl
            .iter()
            .map(|(id, v)| (ConcatId::Left(*id), *v))
            .chain(nr.iter().map(|(id, v)| (ConcatId::Right(*id), *v)))
            .collect();
        let set: BTreeSet<_> = expected.values().copied().collect();
        let mut tx = g.transaction();
        tx.apply(
            if step.is_multiple_of(2) { &l } else { &r },
            TupleChange { id, value },
        )
        .unwrap();
        let graph = tx.graph().unwrap();
        assert_eq!(
            graph
                .dataset_rows(&all)
                .unwrap()
                .iter()
                .map(|(id, v)| (*id, *v))
                .collect::<BTreeMap<_, _>>(),
            expected
        );
        assert_eq!(
            graph
                .dataset_rows(&unique)
                .unwrap()
                .iter()
                .map(|(id, v)| (*id, *v))
                .collect::<BTreeMap<_, _>>(),
            set.iter().map(|v| (*v, *v)).collect()
        );
        assert_eq!(
            tx.score().unwrap(),
            SimpleScore::new(
                2 * (expected.values().map(|v| *v as i64).sum::<i64>()
                    + set.iter().map(|v| *v as i64).sum::<i64>())
            )
        );
        if step.is_multiple_of(3) {
            drop(tx);
        } else {
            tx.commit().unwrap();
            left = nl;
            right = nr;
        }
    }
    let mut tx = g.transaction();
    for id in left.keys() {
        tx.apply(&l, TupleChange::retract(*id)).unwrap();
    }
    for id in right.keys() {
        tx.apply(&r, TupleChange::retract(*id)).unwrap();
    }
    assert_eq!(tx.score().unwrap(), SimpleScore::ZERO);
    assert!(
        tx.graph()
            .unwrap()
            .dataset_rows(&unique)
            .unwrap()
            .is_empty()
    );
    tx.commit().unwrap();
    g.assert_consistent().unwrap();
}

#[test]
fn padded_mappings_are_shared_across_opposite_parents_and_consumers() {
    let left_calls = Cell::new(0usize);
    let right_calls = Cell::new(0usize);
    let d = SharedPaddedConcat::new(
        |v: &i64| {
            left_calls.set(left_calls.get() + 1);
            (*v, None)
        },
        |v: &(i64, i64)| {
            right_calls.set(right_calls.get() + 1);
            (v.0, Some(v.1))
        },
    );
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let l = b.source::<usize, i64>();
    let r = b.source::<usize, (i64, i64)>();
    let s = b.source::<usize, (i64, i64)>();
    let a = d.apply(&mut b, l.port(), r.port()).unwrap();
    let c = d.apply(&mut b, l.port(), s.port()).unwrap();
    assert!(!a.same_node(&c));
    assert!(a.same_node(&d.clone().apply(&mut b, l.port(), r.port()).unwrap()));
    for (name, port) in [("a", &a), ("again", &a), ("c", &c)] {
        b.score(name, port, |(v, padding)| {
            SimpleScore::new(v + padding.unwrap_or_default())
        })
        .unwrap();
    }
    let ad = b.dataset(&a).unwrap();
    let cd = b.dataset(&c).unwrap();
    assert_eq!((left_calls.get(), right_calls.get()), (0, 0));
    drop(d);
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&l, TupleChange::put(0, 10)).unwrap();
    tx.apply(&r, TupleChange::put(0, (2, 3))).unwrap();
    tx.apply(&s, TupleChange::put(0, (4, 7))).unwrap();
    tx.commit().unwrap();
    assert_eq!((left_calls.get(), right_calls.get()), (1, 2));
    assert_eq!(g.score(), SimpleScore::new(51));
    assert_eq!(
        g.dataset_rows(&ad)
            .unwrap()
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect::<Vec<_>>(),
        vec![
            (ConcatId::Left(0), (10, None)),
            (ConcatId::Right(0), (2, Some(3)))
        ]
    );
    assert_eq!(
        g.dataset_rows(&cd)
            .unwrap()
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect::<Vec<_>>(),
        vec![
            (ConcatId::Left(0), (10, None)),
            (ConcatId::Right(0), (4, Some(7)))
        ]
    );
    let mut tx = g.transaction();
    tx.apply(&l, TupleChange::put(0, 20)).unwrap();
    assert_eq!(tx.score().unwrap(), SimpleScore::new(81));
    drop(tx);
    assert_eq!(left_calls.get(), 2);
    assert_eq!(g.score(), SimpleScore::new(51));
    let mut tx = g.transaction();
    tx.apply(&l, TupleChange::retract(0)).unwrap();
    assert_eq!(tx.score().unwrap(), SimpleScore::new(21));
    tx.commit().unwrap();
    assert_eq!(left_calls.get(), 2);
    g.assert_consistent().unwrap();
}

#[test]
fn self_concat_preserves_both_tags_and_failure_restores_shared_consumers() {
    let fail = Cell::new(false);
    let d = SharedConcat::new();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i32>();
    let other = b.source::<usize, i32>();
    let out = d.apply(&mut b, s.port(), s.port()).unwrap();
    let same = d.clone().apply(&mut b, s.port(), s.port()).unwrap();
    assert!(out.same_node(&same));
    let forward = d.apply(&mut b, s.port(), other.port()).unwrap();
    let reverse = d.apply(&mut b, other.port(), s.port()).unwrap();
    assert!(!forward.same_node(&reverse));
    assert!(
        !out.same_node(
            &SharedConcat::new()
                .apply(&mut b, s.port(), s.port())
                .unwrap()
        )
    );
    b.score("a", &out, |v| SimpleScore::new(*v as i64)).unwrap();
    b.score("b", &same, |v| {
        assert!(!fail.get(), "injected");
        SimpleScore::new(*v as i64)
    })
    .unwrap();
    let data = b.dataset(&out).unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(5, 7)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(28));
    assert_eq!(
        g.dataset_rows(&data)
            .unwrap()
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect::<Vec<_>>(),
        vec![(ConcatId::Left(5), 7), (ConcatId::Right(5), 7)]
    );
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut tx = g.transaction();
            tx.apply(&s, TupleChange::put(5, 9)).unwrap();
            tx.commit().unwrap();
        }))
        .is_err()
    );
    fail.set(false);
    assert_eq!(g.score(), SimpleScore::new(28));
    g.assert_consistent().unwrap();
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::retract(5)).unwrap();
    assert_eq!(tx.score().unwrap(), SimpleScore::ZERO);
    tx.commit().unwrap();
    g.assert_consistent().unwrap();
}

#[derive(Clone, Debug)]
struct Item {
    class: i32,
    payload: i64,
}
impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.class == other.class
    }
}
impl Eq for Item {}
impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Item {
    fn cmp(&self, other: &Self) -> Ordering {
        self.class.cmp(&other.class)
    }
}
#[test]
fn shared_distinct_replaces_representatives_without_losing_duplicates() {
    let d = SharedDistinct::new();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, Item>();
    let out = d.apply(&mut b, s.port()).unwrap();
    let same = d.clone().apply(&mut b, s.port()).unwrap();
    for (name, p) in [("a", &out), ("b", &same)] {
        b.score(name, p, |v| SimpleScore::new(v.payload)).unwrap();
    }
    let data = b.dataset(&out).unwrap();
    let mut g = b.build();
    for (change, score) in [
        (
            TupleChange::put(
                9,
                Item {
                    class: 1,
                    payload: 9,
                },
            ),
            18,
        ),
        (
            TupleChange::put(
                2,
                Item {
                    class: 1,
                    payload: 2,
                },
            ),
            4,
        ),
        (
            TupleChange::put(
                9,
                Item {
                    class: 1,
                    payload: 90,
                },
            ),
            4,
        ),
        (TupleChange::retract(2), 180),
        (
            TupleChange::put(
                9,
                Item {
                    class: 2,
                    payload: 3,
                },
            ),
            6,
        ),
        (TupleChange::retract(9), 0),
    ] {
        let mut tx = g.transaction();
        tx.apply(&s, change).unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(score));
        tx.commit().unwrap();
    }
    assert!(g.dataset_rows(&data).unwrap().is_empty());
    g.assert_consistent().unwrap();
}

#[test]
fn tuple_flattening_and_expansion_preserve_prefixes_and_duplicate_positions() {
    type Row = ((i32, i32), Vec<i32>);
    let calls = Cell::new(0usize);
    let expansions = Cell::new(0usize);
    let flattened = SharedFlatMap::new(|((a, b), values): &Row| {
        calls.set(calls.get() + 1);
        values.iter().map(|c| (*a, *b, *c)).collect::<Vec<_>>()
    });
    let expanded = SharedMap::new(|(a, b, c): &(i32, i32, i32)| {
        expansions.set(expansions.get() + 1);
        (*a, *b, *c, a + b + c)
    });
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, Row>();
    let flat = flattened.apply(&mut b, s.port()).unwrap();
    let out = expanded.apply(&mut b, &flat).unwrap();
    let flat_again = flattened.clone().apply(&mut b, s.port()).unwrap();
    let again = expanded.clone().apply(&mut b, &flat_again).unwrap();
    assert!(out.same_node(&again));
    let data = b.dataset(&out).unwrap();
    for (name, p) in [("a", &out), ("b", &again)] {
        b.score(name, p, |(a, b, c, d)| {
            SimpleScore::new((a + b + c + d) as i64)
        })
        .unwrap();
    }
    let mut g = b.build();
    let mut rows = BTreeMap::new();
    for step in 0usize..200 {
        let id = step % 7;
        let value = (!step.is_multiple_of(5))
            .then(|| ((id as i32, step as i32), vec![(step % 3) as i32; step % 4]));
        let before = (calls.get(), expansions.get());
        let count = value.as_ref().map_or(0, |v| v.1.len());
        let mut next = rows.clone();
        if let Some(ref v) = value {
            next.insert(id, v.clone());
        } else {
            next.remove(&id);
        }
        let expected: BTreeMap<_, _> = next
            .iter()
            .flat_map(|(id, ((a, b), values))| {
                values
                    .iter()
                    .enumerate()
                    .map(move |(pos, c)| ((*id, pos), (*a, *b, *c, a + b + c)))
            })
            .collect();
        let present = value.is_some();
        let mut tx = g.transaction();
        tx.apply(&s, TupleChange { id, value }).unwrap();
        assert_eq!(
            (calls.get() - before.0, expansions.get() - before.1),
            (usize::from(present), count)
        );
        assert_eq!(
            tx.graph()
                .unwrap()
                .dataset_rows(&data)
                .unwrap()
                .iter()
                .map(|(k, v)| (*k, *v))
                .collect::<BTreeMap<_, _>>(),
            expected
        );
        assert_eq!(
            tx.score().unwrap(),
            SimpleScore::new(expected.values().map(|v| 4 * v.3 as i64).sum())
        );
        if step.is_multiple_of(3) {
            drop(tx);
        } else {
            tx.commit().unwrap();
            rows = next;
        }
    }
    g.assert_consistent().unwrap();
}
