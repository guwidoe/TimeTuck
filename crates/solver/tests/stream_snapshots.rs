// Original persistent-storage regressions; no upstream method credit.
use std::{cell::Cell, rc::Rc};
use timetuck::*;
#[derive(Debug)]
struct Row {
    key: usize,
    value: i64,
    clones: Rc<Cell<usize>>,
}
impl Clone for Row {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            key: self.key,
            value: self.value,
            clones: self.clones.clone(),
        }
    }
}

#[test]
fn candidate_checkpoints_and_held_snapshots_do_not_clone_unrelated_rows() {
    let mut costs = Vec::new();
    for count in [16, 4096] {
        let clones = Rc::new(Cell::new(0));
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, Row>();
        let filtered = b.filter(source.port(), |row| row.value >= 0).unwrap();
        let pairs = b
            .join_equal(&filtered, &filtered, |row| row.key, |row| row.key)
            .unwrap();
        let terminal = b
            .score("self pairs", &pairs, |(a, b)| {
                SimpleScore::new(a.value + b.value)
            })
            .unwrap();
        let grouped = b.group_collect(&filtered, |row| (0, row.value)).unwrap();
        b.score("sum", &grouped, |(_, s)| SimpleScore::new(s.sum))
            .unwrap();
        let mut graph = b.build();
        let mut tx = graph.transaction();
        for key in 0..count {
            tx.apply(
                &source,
                TupleChange::put(
                    key,
                    Row {
                        key,
                        value: key as i64,
                        clones: clones.clone(),
                    },
                ),
            )
            .unwrap();
        }
        tx.commit().unwrap();
        let score = graph.score();
        clones.set(0);
        let held_rows = graph.source_rows(&source).unwrap().clone();
        let held_matches = graph.constraint_matches(&terminal).unwrap().clone();
        assert_eq!(clones.get(), 0, "inspection snapshots copy only tree roots");
        {
            let mut tx = graph.transaction();
            tx.apply(
                &source,
                TupleChange::put(
                    0,
                    Row {
                        key: 0,
                        value: 10,
                        clones: clones.clone(),
                    },
                ),
            )
            .unwrap();
            assert_eq!(tx.score(), Ok(score + SimpleScore::new(30)));
            let before = clones.get();
            drop(tx);
            assert_eq!(clones.get(), before, "undo does not clone facts");
        }
        assert_eq!(graph.score(), score);
        let mut tx = graph.transaction();
        tx.apply(
            &source,
            TupleChange::put(
                0,
                Row {
                    key: 0,
                    value: 20,
                    clones: clones.clone(),
                },
            ),
        )
        .unwrap();
        let before = clones.get();
        tx.commit().unwrap();
        assert_eq!(clones.get(), before, "commit does not clone facts");
        costs.push(clones.get());
        assert_eq!(held_rows[&0].value, 0);
        assert_eq!(held_matches[&(0, 0)].1, SimpleScore::ZERO);
        assert_eq!(graph.source_rows(&source).unwrap()[&0].value, 20);
        assert_eq!(graph.score(), score + SimpleScore::new(60));
        graph.assert_consistent().unwrap();
    }
    assert_eq!(
        costs[0], costs[1],
        "clone cost must not grow with unrelated facts"
    );
    assert!(
        costs[0] < 100,
        "single-row propagation has bounded payload work: {costs:?}"
    );
}

#[test]
fn collector_snapshots_preserve_duplicate_counts_extrema_and_thread_traits() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<IntCollector>();
    send_sync::<SnapshotMap<i64, usize>>();
    let mut c: IntCollector = (0..4096).chain([0, 0, 4095]).collect();
    let old = c.clone();
    for i in 0..4096 {
        assert!(c.remove(i));
    }
    assert_eq!(
        c.summary(),
        IntSummary {
            count: 3,
            distinct_count: 2,
            sum: 4095,
            min: Some(0),
            max: Some(4095)
        }
    );
    assert_eq!(old.summary().count, 4099);
    assert_eq!(old.summary().distinct_count, 4096);
    assert_eq!(old.summary().sum, (4095 * 4096) / 2 + 4095);
    assert!(c.remove(4095));
    assert!(c.remove(0));
    assert!(c.remove(0));
    assert!(!c.remove(0));
    assert_eq!(c.summary(), IntSummary::default());
    assert_eq!(old.summary().max, Some(4095));
}
