//! Original runtime checks; no upstream method credit.
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
type Row = (i32, i64);
type Aggregates = (CountCollector, SumCollector, DistinctCountCollector<i64>);

#[test]
fn shared_aggregations_match_rebuilding_through_updates_and_rollbacks() {
    let calls: [Cell<usize>; 5] = std::array::from_fn(|_| Cell::new(0));
    let groups = SharedGroup::<usize, Row, i32, ((), i64, i64), Aggregates>::new(|v| {
        calls[0].set(calls[0].get() + 1);
        (v.0, ((), v.1, v.1))
    });
    let totals = SharedCollect::<usize, Row, i64, SumCollector>::new(|v| {
        calls[1].set(calls[1].get() + 1);
        v.1
    });
    let keys = SharedGroupKeys::new(|v: &Row| {
        calls[2].set(calls[2].get() + 1);
        v.0
    });
    let lists = SharedGroupList::new(|v: &Row| {
        calls[3].set(calls[3].get() + 1);
        *v
    });
    let list = SharedCollectList::new(|v: &Row| {
        calls[4].set(calls[4].get() + 1);
        v.1
    });
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Row>();
    let gp = groups.apply(&mut b, source.port()).unwrap();
    let tp = totals.apply(&mut b, source.port()).unwrap();
    let kp = keys.apply(&mut b, source.port()).unwrap();
    let lp = lists.apply(&mut b, source.port()).unwrap();
    let ap = list.apply(&mut b, source.port()).unwrap();
    let gd = b.dataset(&gp).unwrap();
    let td = b.dataset(&tp).unwrap();
    let kd = b.dataset(&kp).unwrap();
    let ld = b.dataset(&lp).unwrap();
    let ad = b.dataset(&ap).unwrap();
    for index in 0..2 {
        let p = groups.clone().apply(&mut b, source.port()).unwrap();
        assert!(gp.same_node(&p));
        b.score(
            format!("groups{index}"),
            &p,
            |(_, (count, sum, distinct))| SimpleScore::new(*count as i64 + sum + *distinct as i64),
        )
        .unwrap();
        let p = totals.clone().apply(&mut b, source.port()).unwrap();
        assert!(tp.same_node(&p));
        b.score(format!("sum{index}"), &p, |sum| SimpleScore::new(*sum))
            .unwrap();
        let p = keys.clone().apply(&mut b, source.port()).unwrap();
        assert!(kp.same_node(&p));
        b.score(format!("keys{index}"), &p, |key| {
            SimpleScore::new(*key as i64)
        })
        .unwrap();
        let p = lists.clone().apply(&mut b, source.port()).unwrap();
        assert!(lp.same_node(&p));
        b.score(format!("lists{index}"), &p, |(_, values)| {
            SimpleScore::new(values.len() as i64)
        })
        .unwrap();
        let p = list.clone().apply(&mut b, source.port()).unwrap();
        assert!(ap.same_node(&p));
        b.score(format!("list{index}"), &p, |values| {
            SimpleScore::new(values.iter().sum())
        })
        .unwrap();
    }
    assert!(calls.iter().all(|c| c.get() == 0));
    let mut g = b.build();
    let mut rows = BTreeMap::new();
    let mut seed = 1234567u64;
    for step in 0usize..600 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let id = (seed >> 32) as usize % 13;
        let value = (!step.is_multiple_of(4)).then_some(((seed % 4) as i32, (seed % 7) as i64 - 3));
        let before: Vec<_> = calls.iter().map(Cell::get).collect();
        let mut next = rows.clone();
        if let Some(value) = value {
            next.insert(id, value);
        } else {
            next.remove(&id);
        }
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange { id, value }).unwrap();
        for (call, before) in calls.iter().zip(before) {
            assert_eq!(call.get() - before, usize::from(value.is_some()));
        }
        let mut expected: BTreeMap<i32, Vec<i64>> = BTreeMap::new();
        for (key, value) in next.values() {
            expected.entry(*key).or_default().push(*value);
        }
        let aggregates: BTreeMap<_, _> = expected
            .iter()
            .map(|(k, v)| {
                (
                    *k,
                    (
                        *k,
                        (
                            v.len(),
                            v.iter().sum::<i64>(),
                            v.iter().collect::<BTreeSet<_>>().len(),
                        ),
                    ),
                )
            })
            .collect();
        let graph = tx.graph().unwrap();
        assert_eq!(
            graph
                .dataset_rows(&gd)
                .unwrap()
                .iter()
                .map(|(k, v)| (*k, *v))
                .collect::<BTreeMap<_, _>>(),
            aggregates
        );
        assert_eq!(
            graph
                .dataset_rows(&kd)
                .unwrap()
                .iter()
                .map(|(k, v)| (*k, *v))
                .collect::<BTreeMap<_, _>>(),
            expected.keys().map(|k| (*k, *k)).collect()
        );
        assert_eq!(
            graph
                .dataset_rows(&ld)
                .unwrap()
                .iter()
                .map(|(k, (_, v))| (*k, v.iter().copied().collect::<Vec<_>>()))
                .collect::<BTreeMap<_, _>>(),
            expected
        );
        let sum = next.values().map(|r| r.1).sum::<i64>();
        assert_eq!(
            graph
                .dataset_rows(&td)
                .unwrap()
                .iter()
                .map(|(_, v)| *v)
                .collect::<Vec<_>>(),
            if next.is_empty() { vec![] } else { vec![sum] }
        );
        assert_eq!(
            graph
                .dataset_rows(&ad)
                .unwrap()
                .iter()
                .map(|(_, v)| v.iter().copied().collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            if next.is_empty() {
                vec![]
            } else {
                vec![next.values().map(|v| v.1).collect::<Vec<_>>()]
            }
        );
        let group_score = aggregates
            .values()
            .map(|(_, (n, s, d))| *n as i64 + s + *d as i64)
            .sum::<i64>();
        let score = 2
            * (group_score
                + sum
                + expected.keys().map(|k| *k as i64).sum::<i64>()
                + next.len() as i64
                + sum);
        assert_eq!(tx.score().unwrap(), SimpleScore::new(score));
        if step % 3 != 0 {
            tx.commit().unwrap();
            rows = next;
        } else {
            drop(tx);
        }
    }
    // Explicitly drain, checking that no empty global or keyed rows survive.
    let mut tx = g.transaction();
    for id in rows.keys() {
        tx.apply(&source, TupleChange::retract(*id)).unwrap();
    }
    assert_eq!(tx.score().unwrap(), SimpleScore::ZERO);
    let graph = tx.graph().unwrap();
    assert!(graph.dataset_rows(&gd).unwrap().is_empty());
    assert!(graph.dataset_rows(&td).unwrap().is_empty());
    assert!(graph.dataset_rows(&kd).unwrap().is_empty());
    assert!(graph.dataset_rows(&ld).unwrap().is_empty());
    assert!(graph.dataset_rows(&ad).unwrap().is_empty());
    tx.commit().unwrap();
    g.assert_consistent().unwrap();
}

#[test]
fn shared_collector_mapping_and_downstream_panics_restore_all_consumers() {
    let panic_mapping = Cell::new(false);
    let panic_terminal = Cell::new(false);
    let d = SharedGroup::<usize, Row, i32, i64, SumCollector>::new(|v| {
        assert!(!panic_mapping.get(), "mapping failure");
        *v
    });
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, Row>();
    let a = d.apply(&mut b, s.port()).unwrap();
    let c = d.clone().apply(&mut b, s.port()).unwrap();
    b.score("first", &a, |(_, n)| SimpleScore::new(*n)).unwrap();
    b.score("second", &c, |(_, n)| {
        assert!(!panic_terminal.get(), "terminal failure");
        SimpleScore::new(*n)
    })
    .unwrap();
    let dataset = b.dataset(&a).unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(0, (1, 5))).unwrap();
    tx.commit().unwrap();
    for flag in [&panic_mapping, &panic_terminal] {
        flag.set(true);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let mut tx = g.transaction();
                tx.apply(&s, TupleChange::put(0, (2, 9))).unwrap();
                tx.commit().unwrap();
            }))
            .is_err()
        );
        flag.set(false);
        assert_eq!(g.score(), SimpleScore::new(10));
        assert_eq!(
            g.dataset_rows(&dataset)
                .unwrap()
                .iter()
                .map(|(k, v)| (*k, *v))
                .collect::<Vec<_>>(),
            vec![(1, (1, 5))]
        );
        g.assert_consistent().unwrap();
    }
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(0, (2, 9))).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(18));
    g.assert_consistent().unwrap();
}

#[test]
fn definitions_separate_parents_builders_and_callbacks_and_outlive_handles() {
    let d = SharedCollect::<usize, i64, i64, SumCollector>::new(|v| *v);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i64>();
    let a = d.apply(&mut b, s.port()).unwrap();
    let filtered = b.filter(s.port(), |_| true).unwrap();
    assert!(!a.same_node(&d.apply(&mut b, &filtered).unwrap()));
    let fresh = SharedCollect::<usize, i64, i64, SumCollector>::new(|v| *v);
    assert!(!a.same_node(&fresh.apply(&mut b, s.port()).unwrap()));
    let mut other = StreamGraphBuilder::<HardSoftScore>::new();
    let os = other.source::<usize, i64>();
    let oa = d.apply(&mut other, os.port()).unwrap();
    assert!(!a.same_node(&oa));
    assert!(matches!(
        d.apply(&mut b, os.port()),
        Err(GraphError::ForeignPort)
    ));
    assert!(a.same_node(&d.clone().apply(&mut b, s.port()).unwrap()));
    b.score("sum", &a, |v| SimpleScore::new(*v)).unwrap();
    drop(d);
    drop(fresh);
    let mut graph = b.build();
    let mut tx = graph.transaction();
    tx.apply(&s, TupleChange::put(0, 17)).unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.score(), SimpleScore::new(17));
    graph.assert_consistent().unwrap();
}

#[test]
fn collected_snapshots_survive_replacement_retraction_and_definition_drop() {
    let d = SharedCollectList::new(|v: &i32| *v);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i32>();
    let p = d.apply(&mut b, s.port()).unwrap();
    let data = b.dataset(&p).unwrap();
    drop(d);
    let mut g = b.build();
    let mut tx = g.transaction();
    for (id, v) in [(9, 2), (1, 2), (4, 7)] {
        tx.apply(&s, TupleChange::put(id, v)).unwrap();
    }
    tx.commit().unwrap();
    let retained = g
        .dataset_rows(&data)
        .unwrap()
        .iter()
        .next()
        .unwrap()
        .1
        .clone();
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(4, 3)).unwrap();
    tx.apply(&s, TupleChange::retract(1)).unwrap();
    tx.commit().unwrap();
    assert_eq!(retained.iter().copied().collect::<Vec<_>>(), vec![2, 7, 2]);
    assert_eq!(
        g.dataset_rows(&data)
            .unwrap()
            .iter()
            .next()
            .unwrap()
            .1
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![3, 2]
    );
    g.assert_consistent().unwrap();
}
