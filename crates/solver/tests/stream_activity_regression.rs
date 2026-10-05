use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn dead_branch_skips_callbacks_retains_sources_and_reactivates_transactionally() {
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let left = b.source::<usize, i64>();
    let right = b.source::<usize, i64>();
    let mapped = b
        .map(left.port(), |v| {
            calls.set(calls.get() + 1);
            *v
        })
        .unwrap();
    let joined = b.join_equal(&mapped, right.port(), |_| (), |_| ()).unwrap();
    b.score("join", &joined, |(a, c)| SimpleScore::new(a + c))
        .unwrap();
    let mut g = b.build();
    for n in 0..100 {
        let mut tx = g.transaction();
        tx.apply(&left, TupleChange::put(n % 5, n as i64)).unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(calls.get(), 0);
    assert_eq!(g.source_rows(&left).unwrap().len(), 5);
    let dead = g.activity();
    assert!(!dead.get(&mapped).unwrap().active);
    let mut tx = g.transaction();
    tx.apply(&right, TupleChange::put(0, 1)).unwrap();
    tx.commit().unwrap();
    assert_eq!(calls.get(), 5);
    assert_eq!(g.score(), SimpleScore::new((95..100).sum::<i64>() + 5));
    assert!(g.activity().get(&mapped).unwrap().active);
    assert!(!dead.get(&mapped).unwrap().active);
    let before = g.score();
    let mut tx = g.transaction();
    tx.apply(&right, TupleChange::retract(0)).unwrap();
    assert_eq!(tx.score().unwrap(), SimpleScore::ZERO);
    assert!(!tx.graph().unwrap().activity().get(&mapped).unwrap().active);
    drop(tx);
    assert_eq!(g.score(), before);
    assert!(g.activity().get(&mapped).unwrap().active);
    let mut tx = g.transaction();
    tx.apply(&right, TupleChange::retract(0)).unwrap();
    tx.commit().unwrap();
    let old_calls = calls.get();
    let mut tx = g.transaction();
    tx.apply(&left, TupleChange::put(0, 200)).unwrap();
    tx.commit().unwrap();
    assert_eq!(calls.get(), old_calls);
    let mut tx = g.transaction();
    tx.apply(&right, TupleChange::put(0, 2)).unwrap();
    tx.commit().unwrap();
    assert_eq!(
        g.score(),
        SimpleScore::new(200 + (96..100).sum::<i64>() + 10)
    );
    g.assert_consistent().unwrap();
}
#[test]
fn activation_panic_and_overflow_restore_plan_indexes_and_sources() {
    let fail = Cell::new(false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let left = b.source::<usize, i64>();
    let right = b.source::<usize, i64>();
    let mapped = b
        .map(left.port(), |v| {
            assert!(!fail.get());
            *v
        })
        .unwrap();
    let joined = b.join_equal(&mapped, right.port(), |_| (), |_| ()).unwrap();
    b.score("sum", &joined, |(v, _)| SimpleScore::new(*v))
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&left, TupleChange::put(0, i64::MAX)).unwrap();
    tx.apply(&left, TupleChange::put(1, 1)).unwrap();
    tx.commit().unwrap();
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut tx = g.transaction();
            tx.apply(&right, TupleChange::put(0, 0)).unwrap();
        }))
        .is_err()
    );
    fail.set(false);
    assert!(g.source_rows(&right).unwrap().is_empty());
    assert!(!g.activity().get(&mapped).unwrap().active);
    let mut tx = g.transaction();
    assert!(matches!(
        tx.apply(&right, TupleChange::put(0, 0)),
        Err(GraphError::ScoreOverflow { .. })
    ));
    assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
    drop(tx);
    assert_eq!(g.score(), SimpleScore::ZERO);
    assert!(g.source_rows(&right).unwrap().is_empty());
    let mut tx = g.transaction();
    tx.apply(&left, TupleChange::put(1, -1)).unwrap();
    tx.apply(&right, TupleChange::put(0, 0)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(i64::MAX - 1));
    g.assert_consistent().unwrap();
}
#[test]
fn observers_disabled_weights_and_foreign_snapshots_are_respected() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let source = b.source::<usize, i64>();
    let dead = b
        .map(source.port(), |_| panic!("disabled branch ran"))
        .unwrap();
    b.score_weighted("off", &dead, SimpleScore::ZERO, |_: &i64| 1)
        .unwrap();
    let data = b.dataset(source.port()).unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, 7)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.dataset_rows(&data).unwrap()[&0], 7);
    assert!(g.activity().get(source.port()).unwrap().active);
    assert!(!g.activity().get(&dead).unwrap().active);
    let mut other = StreamGraphBuilder::<SimpleScore>::new();
    let foreign = other.source::<usize, i64>();
    assert_eq!(
        g.activity().get(foreign.port()),
        Err(GraphError::ForeignPort)
    );
    g.assert_consistent().unwrap();
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    rows: Vec<Vec<Vec<i64>>>,
    score: SimpleScore,
}
type Reader = dyn Fn(&StreamGraph<'_, SimpleScore>) -> Snapshot;
struct Harness {
    graph: StreamGraph<'static, SimpleScore>,
    left: StreamSource<usize, (i64, i64)>,
    right: StreamSource<usize, (i64, i64)>,
    read: Box<Reader>,
}
fn rows<I: Ord + 'static>(
    g: &StreamGraph<'_, SimpleScore>,
    d: &StreamDataset<I, Vec<i64>>,
) -> Vec<Vec<i64>> {
    let mut out: Vec<_> = g
        .dataset_rows(d)
        .unwrap()
        .iter()
        .map(|(_, v)| v.clone())
        .collect();
    out.sort();
    out
}
fn harness(pruning: bool) -> Harness {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    if pruning {
        b = b.with_activity_pruning();
    }
    let left = b.source::<usize, (i64, i64)>();
    let right = b.source::<usize, (i64, i64)>();
    let facts = b.precompute(left.port(), |r| *r, |r| *r).unwrap();
    let join = b
        .join_equal(&facts, right.port(), |r| r.0, |r| r.0)
        .unwrap();
    let join = b.map(&join, |(a, c)| vec![a.0, a.1, c.1]).unwrap();
    let join_data = b.dataset(&join).unwrap();
    b.score("join", &join, |r| SimpleScore::new(r[1] * r[2]))
        .unwrap();
    let anti = b
        .existence_equal(
            &facts,
            right.port(),
            ExistenceMode::NotExists,
            |r| r.0,
            |r| r.0,
        )
        .unwrap();
    let anti = b.map(&anti, |r| vec![r.0, r.1]).unwrap();
    let anti_data = b.dataset(&anti).unwrap();
    b.score("anti", &anti, |r| SimpleScore::new(-r[1])).unwrap();
    let concat = b.concat(&facts, right.port()).unwrap();
    let concat = b.map(&concat, |r| vec![r.0, r.1]).unwrap();
    let concat_data = b.dataset(&concat).unwrap();
    let refs = b
        .follow_reference(&facts, right.port(), |r| Some(r.0 as usize))
        .unwrap();
    let refs = b
        .map(&refs, |(a, c)| vec![a.0, a.1, c.map_or(-999, |r| r.1)])
        .unwrap();
    let ref_data = b.dataset(&refs).unwrap();
    let grouped = b
        .group_collect_with::<_, _, _, _, CountCollector>(&facts, |r| (r.0, ()))
        .unwrap();
    let grouped = b
        .map(&grouped, |(key, count)| vec![*key, *count as i64])
        .unwrap();
    let group_data = b.dataset(&grouped).unwrap();
    Harness {
        graph: b.build(),
        left,
        right,
        read: Box::new(move |g| Snapshot {
            rows: vec![
                rows(g, &join_data),
                rows(g, &anti_data),
                rows(g, &concat_data),
                rows(g, &ref_data),
                rows(g, &group_data),
            ],
            score: g.score(),
        }),
    }
}
#[test]
fn pruned_and_eager_relations_agree_through_source_emptying_and_rollback() {
    let mut pruned = harness(true);
    let mut eager = harness(false);
    for step in 0..800 {
        let side = step % 3 == 0;
        let id = (step / 2) % 4;
        let event = if (step / 16) % 2 == 0 {
            TupleChange::put(id, ((step % 3) as i64, step as i64 % 17 - 8))
        } else {
            TupleChange::retract(id)
        };
        let mut a = pruned.graph.transaction();
        let mut b = eager.graph.transaction();
        a.apply(
            if side { &pruned.right } else { &pruned.left },
            event.clone(),
        )
        .unwrap();
        b.apply(if side { &eager.right } else { &eager.left }, event)
            .unwrap();
        assert_eq!(
            (pruned.read)(a.graph().unwrap()),
            (eager.read)(b.graph().unwrap()),
            "step {step}"
        );
        if step % 11 == 0 {
            drop(a);
            drop(b);
        } else {
            a.commit().unwrap();
            b.commit().unwrap();
        }
        assert_eq!((pruned.read)(&pruned.graph), (eager.read)(&eager.graph));
    }
    pruned.graph.assert_consistent().unwrap();
    eager.graph.assert_consistent().unwrap();
}

#[test]
fn empty_chain_deactivates_but_predicate_emptiness_is_conservative() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let source = b.source::<usize, i64>();
    let filtered = b.filter(source.port(), |_| false).unwrap();
    let mapped = b.map(&filtered, |v| *v).unwrap();
    b.score("empty", &mapped, |_| SimpleScore::new(-1)).unwrap();
    let mut g = b.build();
    assert_eq!(g.activity().active_count(), 0);
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, 1)).unwrap();
    tx.commit().unwrap();
    assert!(g.activity().get(&filtered).unwrap().can_produce);
    assert!(g.activity().get(&mapped).unwrap().active);
    assert_eq!(g.score(), SimpleScore::ZERO);
}
