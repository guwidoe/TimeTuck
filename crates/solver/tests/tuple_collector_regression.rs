//! Original collector composition and graph integration regressions.
use std::{
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn distinct_counts_track_duplicates_missing_values_replacement_and_snapshots() {
    let mut c = DistinctCountCollector::default();
    for v in [None, Some(1), Some(1), Some(2)] {
        c.insert(&v);
    }
    let held = c.clone();
    assert_eq!(c.finish(), 3);
    assert!(c.remove(&Some(1)));
    assert_eq!(c.finish(), 3);
    assert!(c.replace(&Some(1), &Some(2)));
    assert_eq!(c.finish(), 2);
    assert!(!c.replace(&Some(1), &Some(3)));
    assert_eq!(c.finish(), 2);
    assert!(c.remove(&None));
    assert!(!c.remove(&None));
    assert_eq!(c.finish(), 1);
    assert!(c.remove(&Some(2)));
    assert_eq!(c.finish(), 1);
    assert!(c.remove(&Some(2)));
    assert_eq!(c.finish(), 0);
    assert_eq!(held.finish(), 3);
}
#[test]
fn failed_later_components_do_not_commit_earlier_component_mutations() {
    type C = (
        DistinctCountCollector<i64>,
        SumCollector,
        SetCollector<usize>,
        CountCollector,
    );
    type V = (i64, i64, usize, ());
    let mut c = C::default();
    c.insert(&(7, i64::MIN, 1, ()));
    let before = <C as ReversibleCollector<V>>::finish(&c);
    assert!(!c.remove(&(7, i64::MIN, 99, ())));
    assert_eq!(<C as ReversibleCollector<V>>::finish(&c), before);
    assert!(!c.replace(&(7, i64::MIN, 99, ()), &(8, i64::MIN, 2, ())));
    assert_eq!(<C as ReversibleCollector<V>>::finish(&c), before);
    // Removing MIN and inserting MAX would fit, but SumCollector's specialized
    // replacement requires the difference itself to fit. Composition must delegate.
    assert!(
        catch_unwind(AssertUnwindSafe(
            || c.replace(&(7, i64::MIN, 1, ()), &(8, i64::MAX, 2, ()))
        ))
        .is_err()
    );
    assert_eq!(<C as ReversibleCollector<V>>::finish(&c), before);
    assert!(catch_unwind(AssertUnwindSafe(|| c.insert(&(8, -1, 2, ())))).is_err());
    assert_eq!(<C as ReversibleCollector<V>>::finish(&c), before);
    assert!(c.replace(&(7, i64::MIN, 1, ()), &(8, -1, 2, ())));
    let (distinct, sum, set, count) = <C as ReversibleCollector<V>>::finish(&c);
    assert_eq!((distinct, sum, count), (1, -1, 1));
    assert_eq!(set.keys().copied().collect::<Vec<_>>(), vec![2]);
}
#[test]
fn tuple_composition_does_not_require_input_clone_and_supports_nested_tuples() {
    struct Token;
    type C = (
        CountCollector,
        CountCollector,
        CountCollector,
        CountCollector,
    );
    type V = (Token, Token, Token, Token);
    let mut c = C::default();
    let v = (Token, Token, Token, Token);
    c.insert(&v);
    assert_eq!(<C as ReversibleCollector<V>>::finish(&c), (1, 1, 1, 1));
    assert!(c.remove(&v));
    assert_eq!(<C as ReversibleCollector<V>>::finish(&c), (0, 0, 0, 0));
    type Nested = (
        (CountCollector, CountCollector),
        DistinctCountCollector<i32>,
    );
    let mut c = Nested::default();
    c.insert(&((Token, Token), 5));
    assert_eq!(
        <Nested as ReversibleCollector<((Token, Token), i32)>>::finish(&c),
        ((1, 1), 1)
    );
}
fn random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed >> 32
}
#[test]
fn four_collectors_match_independent_group_rebuilds_through_graph_trials() {
    type C = (
        CountCollector,
        DistinctCountCollector<i32>,
        SumCollector,
        ConditionalCollector<SetCollector<i32>>,
    );
    type Row = (u8, i32, i64, Option<i32>);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Row>();
    let groups = b
        .group_collect_with::<_, _, _, _, C>(source.port(), |r| (r.0, ((), r.1, r.2, r.3)))
        .unwrap();
    let dataset = b.dataset(&groups).unwrap();
    b.score("aggregate", &groups, |(_, (_, distinct, sum, _))| {
        SimpleScore::new(*sum - *distinct as i64)
    })
    .unwrap();
    let mut graph = b.build();
    let mut rows = BTreeMap::new();
    let mut seed = 20261101;
    for step in 0..500 {
        let mut next = rows.clone();
        let id = (random(&mut seed) % 24) as usize;
        let value = (!random(&mut seed).is_multiple_of(4)).then(|| {
            (
                (random(&mut seed) % 4) as u8,
                (random(&mut seed) % 7) as i32,
                (random(&mut seed) % 21) as i64 - 10,
                (!random(&mut seed).is_multiple_of(3)).then(|| (random(&mut seed) % 5) as i32),
            )
        });
        if let Some(value) = value {
            next.insert(id, value);
        } else {
            next.remove(&id);
        }
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange { id, value }).unwrap();
        let mut expected = BTreeMap::<u8, (usize, BTreeSet<i32>, i64, BTreeSet<i32>)>::new();
        for &(key, distinct, sum, optional) in next.values() {
            let entry = expected.entry(key).or_default();
            entry.0 += 1;
            entry.1.insert(distinct);
            entry.2 += sum;
            if let Some(v) = optional {
                entry.3.insert(v);
            }
        }
        let actual = tx.graph().unwrap().dataset_rows(&dataset).unwrap();
        assert_eq!(actual.len(), expected.len());
        for (key, (count, distinct, sum, set)) in &expected {
            let (outkey, (outcount, outdistinct, outsum, outset)) = &actual[key];
            assert_eq!(
                (outkey, *outcount, *outdistinct, *outsum),
                (key, *count, distinct.len(), *sum)
            );
            assert_eq!(outset.keys().copied().collect::<BTreeSet<_>>(), *set);
        }
        assert_eq!(
            tx.score().unwrap(),
            SimpleScore::new(
                expected
                    .values()
                    .map(|(_, d, s, _)| s - d.len() as i64)
                    .sum()
            )
        );
        if step % 3 == 0 {
            tx.commit().unwrap();
            rows = next;
        } else {
            drop(tx);
        }
        if step % 50 == 0 {
            graph.assert_consistent().unwrap();
        }
    }
}
#[test]
fn grouped_overflow_rolls_back_all_collectors_and_source_rows() {
    type C = (CountCollector, DistinctCountCollector<i64>, SumCollector);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let groups = b
        .group_collect_with::<_, _, _, _, C>(source.port(), |v| ((), ((), *v, *v)))
        .unwrap();
    let dataset = b.dataset(&groups).unwrap();
    b.score("sum", &groups, |(_, (_, _, sum))| SimpleScore::new(*sum))
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, i64::MAX)).unwrap();
    tx.commit().unwrap();
    let held = g.dataset_snapshot(&dataset).unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut tx = g.transaction();
            tx.apply(&source, TupleChange::put(1, 1)).unwrap();
        }))
        .is_err()
    );
    assert_eq!(
        g.dataset_snapshot(&dataset)
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        held.iter().collect::<Vec<_>>()
    );
    assert_eq!(g.source_rows(&source).unwrap().len(), 1);
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(1, -1)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(i64::MAX - 1));
    g.assert_consistent().unwrap();
}
