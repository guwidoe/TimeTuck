//! Original fixed-relationship graph behavior and work-scaling checks.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[test]
fn asymmetric_batches_match_full_scan_with_rollback_and_rebuild() {
    for pruning in [false, true] {
        let links = vec![
            (9, 0usize, 'a'),
            (2, 1, 'b'),
            (7, 0, 'a'),
            (4, 2, 'c'),
            (5, 3, 'a'),
            (1, 1, 'd'),
        ];
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if pruning {
            b = b.with_activity_pruning();
        }
        let l = b.source::<usize, i64>();
        let r = b.source::<char, i64>();
        let joined = b
            .join_fixed_links_batched(
                l.port(),
                r.port(),
                links.clone(),
                |e, a, c| (a + c + e) % 3 != 0,
                |e, a, c| a * 2 + c + e,
            )
            .unwrap();
        let data = b.dataset(&joined).unwrap();
        b.score("links", &joined, |&v| SimpleScore::new(v)).unwrap();
        let mut g = b.build();
        let mut ls = BTreeMap::new();
        let mut rs = BTreeMap::new();
        for round in 0..180 {
            let mut next_l = ls.clone();
            let mut next_r = rs.clone();
            let mut batch = StreamBatch::new();
            for n in 0..5 {
                let i = (round + n * 7) % 6;
                let j = (b'a' + ((round * 3 + n) % 5) as u8) as char;
                let v = (round as i64 + n as i64) % 17 - 8;
                let lc = if (round + n) % 4 == 0 {
                    next_l.remove(&i);
                    TupleChange::retract(i)
                } else {
                    next_l.insert(i, v);
                    TupleChange::put(i, v)
                };
                let rc = if (round + n) % 5 == 0 {
                    next_r.remove(&j);
                    TupleChange::retract(j)
                } else {
                    next_r.insert(j, -v);
                    TupleChange::put(j, -v)
                };
                batch.push(&l, lc).unwrap();
                batch.push(&r, rc).unwrap();
            }
            // Repeated same-ID changes must expose only the final state.
            batch.push(&l, TupleChange::put(0, 999)).unwrap();
            batch
                .push(&l, TupleChange::put(0, round as i64 % 9))
                .unwrap();
            next_l.insert(0, round as i64 % 9);
            let expected: BTreeMap<_, _> = links
                .iter()
                .filter_map(|&(e, i, j)| {
                    next_l
                        .get(&i)
                        .zip(next_r.get(&j))
                        .filter(|(a, c)| (*a + *c + e) % 3 != 0)
                        .map(|(a, c)| (((e, i), j), a * 2 + c + e))
                })
                .collect();
            let before = g.dataset_rows(&data).unwrap().clone();
            let mut tx = g.transaction();
            tx.apply_batch(batch).unwrap();
            let actual: BTreeMap<_, _> = tx
                .graph()
                .unwrap()
                .dataset_rows(&data)
                .unwrap()
                .iter()
                .map(|(k, v)| (*k, *v))
                .collect();
            assert_eq!(actual, expected, "round {round}");
            assert_eq!(
                tx.score().unwrap(),
                SimpleScore::new(expected.values().sum())
            );
            if round % 37 == 0 {
                tx.assert_consistent().unwrap();
            }
            if round % 3 != 0 {
                tx.commit().unwrap();
                ls = next_l;
                rs = next_r;
            } else {
                drop(tx);
                assert_eq!(g.dataset_rows(&data).unwrap(), &before);
            }
        }
        g.assert_consistent().unwrap();
    }
}
#[test]
fn self_links_visit_only_incident_relationships_and_validate_build_inputs() {
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let rows = b.source::<usize, i64>();
    let foreign = StreamGraphBuilder::<SimpleScore>::new().source::<usize, i64>();
    assert!(matches!(
        b.join_fixed_links_batched(
            rows.port(),
            foreign.port(),
            [(0, 0, 0)],
            |_, _, _| true,
            |_, a, _| *a
        ),
        Err(GraphError::ForeignPort)
    ));
    assert!(matches!(
        b.join_fixed_links_batched(
            rows.port(),
            rows.port(),
            [(0, 0, 0), (0, 1, 1)],
            |_, _, _| true,
            |_, a, _| *a
        ),
        Err(GraphError::Dependency(_))
    ));
    let links = (0..2000)
        .map(|i| (i, i, (i + 1) % 2000))
        .chain([(2000, 0, 0)]);
    let port = b
        .join_fixed_links_batched(
            rows.port(),
            rows.port(),
            links,
            |_, _, _| {
                calls.set(calls.get() + 1);
                true
            },
            |_, a, c| a + c,
        )
        .unwrap();
    let data = b.dataset(&port).unwrap();
    b.score("sum", &port, |&v| SimpleScore::new(v)).unwrap();
    let mut g = b.build();
    let mut batch = StreamBatch::new();
    for i in 0..2000 {
        batch.push(&rows, TupleChange::put(i, 1)).unwrap();
    }
    {
        let mut tx = g.transaction();
        tx.apply_batch(batch).unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(g.dataset_rows(&data).unwrap().len(), 2001);
    assert_eq!(g.score(), SimpleScore::new(4002));
    calls.set(0);
    let mut batch = StreamBatch::new();
    batch.push(&rows, TupleChange::put(0, 2)).unwrap();
    {
        let mut tx = g.transaction();
        tx.apply_batch(batch).unwrap();
        assert_eq!(calls.get(), 3);
        assert_eq!(tx.score().unwrap(), SimpleScore::new(4006));
    }
    assert_eq!(g.score(), SimpleScore::new(4002));
    g.assert_consistent().unwrap();
}
#[derive(Debug)]
struct Row {
    value: i64,
    fail: Rc<Cell<bool>>,
}
impl Clone for Row {
    fn clone(&self) -> Self {
        assert!(
            !self.fail.get() || self.value != 99,
            "installation clone failure"
        );
        Self {
            value: self.value,
            fail: self.fail.clone(),
        }
    }
}
#[test]
fn callback_and_partial_installation_failures_restore_rows_and_retry() {
    for failure in [0, 1, 2] {
        let trigger = Cell::new(false);
        let clone_fail = Rc::new(Cell::new(false));
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let rows = b.source::<usize, Row>();
        let port = b
            .join_fixed_links_batched(
                rows.port(),
                rows.port(),
                [(0, 0, 1), (1, 1, 0)],
                |_, _, _| {
                    assert!(!trigger.get() || failure != 0, "predicate failure");
                    true
                },
                |_, a, c| {
                    assert!(!trigger.get() || failure != 1, "projection failure");
                    if trigger.get() && failure == 2 {
                        clone_fail.set(true);
                    }
                    a.value + c.value
                },
            )
            .unwrap();
        let data = b.dataset(&port).unwrap();
        b.score("sum", &port, |&v| SimpleScore::new(v)).unwrap();
        let mut g = b.build();
        let make = |v| Row {
            value: v,
            fail: clone_fail.clone(),
        };
        {
            let mut batch = StreamBatch::new();
            batch.push(&rows, TupleChange::put(0, make(1))).unwrap();
            batch.push(&rows, TupleChange::put(1, make(2))).unwrap();
            let mut tx = g.transaction();
            tx.apply_batch(batch).unwrap();
            tx.commit().unwrap();
        }
        let held = g.dataset_rows(&data).unwrap().clone();
        trigger.set(true);
        {
            let mut batch = StreamBatch::new();
            batch.push(&rows, TupleChange::put(0, make(3))).unwrap();
            batch.push(&rows, TupleChange::put(1, make(99))).unwrap();
            let mut tx = g.transaction();
            assert!(catch_unwind(AssertUnwindSafe(|| tx.apply_batch(batch))).is_err());
            assert!(tx.score().is_err());
        }
        trigger.set(false);
        clone_fail.set(false);
        assert_eq!(g.dataset_rows(&data).unwrap(), &held);
        assert_eq!(g.score(), SimpleScore::new(6));
        g.assert_consistent().unwrap();
        {
            let mut tx = g.transaction();
            tx.apply(&rows, TupleChange::put(0, make(3))).unwrap();
            tx.commit().unwrap();
        }
        assert_eq!(g.score(), SimpleScore::new(10));
        g.assert_consistent().unwrap();
    }
}
#[test]
fn fixed_relationship_scoring_supports_nested_planning_trials() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(1),
    ])
    .unwrap();
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    let port = b
        .join_fixed_links_batched(
            source.port(),
            source.port(),
            [(0, 0, 1)],
            |_, a, c| a.1 == c.1,
            |_, _, _| 1,
        )
        .unwrap();
    b.score("equal", &port, |&n| SimpleScore::new(n)).unwrap();
    let mut director =
        StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    assert_eq!(director.score(), SimpleScore::ZERO);
    {
        let mut edit = director.edit_session().unwrap();
        edit.apply(&Move::Change {
            variable: 1,
            value: Some(0),
        })
        .unwrap();
        assert_eq!(edit.score(), SimpleScore::new(1));
        {
            let mut nested = edit.nested().unwrap();
            nested
                .apply(&Move::Composite(vec![
                    Move::Change {
                        variable: 0,
                        value: Some(1),
                    },
                    Move::Change {
                        variable: 1,
                        value: Some(1),
                    },
                ]))
                .unwrap();
            assert_eq!(nested.score(), SimpleScore::new(1));
            nested.assert_consistent().unwrap();
        }
        assert_eq!(edit.assignment().values(), &[Some(0), Some(0)]);
    }
    assert_eq!(director.assignment(), &p.initial_assignment());
    assert_eq!(director.score(), SimpleScore::ZERO);
    director.assert_consistent().unwrap();
}
