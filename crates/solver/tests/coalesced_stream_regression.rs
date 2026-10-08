//! Original regressions for final aggregate delivery during solver moves.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn solver_groups_score_final_values_once_and_restore_after_panics() {
    let calls = Cell::new(0);
    let fail = Cell::new(false);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1i64, 2]).with_initial(1),
        Variable::new(vec![1, 2]).with_initial(1),
    ])
    .unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (usize, Option<i64>)>();
    let groups = b.group_collect(s.port(), |r| ((), r.1.unwrap())).unwrap();
    let final_groups = b.coalesce_updates(&groups).unwrap();
    b.score("final group", &final_groups, |(_, stats)| {
        calls.set(calls.get() + 1);
        assert!(!fail.get(), "sink failure");
        assert_eq!(stats.sum % 2, 0, "an intermediate group escaped");
        SimpleScore::new(stats.sum)
    })
    .unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    assert_eq!(d.score(), SimpleScore::new(2));
    assert_eq!(calls.get(), 1);
    let movement = Move::Composite(vec![
        Move::Change {
            variable: 0,
            value: Some(2),
        },
        Move::Change {
            variable: 1,
            value: Some(2),
        },
    ]);
    calls.set(0);
    let trial = d.apply(&movement).unwrap();
    assert_eq!(trial.score(), SimpleScore::new(4));
    assert_eq!(trial.constraint_evaluations(), 1);
    assert_eq!(calls.get(), 1);
    drop(trial);
    assert_eq!(d.score(), SimpleScore::new(2));
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _trial = d.apply(&movement);
        }))
        .is_err()
    );
    fail.set(false);
    assert_eq!(d.score(), SimpleScore::new(2));
    d.apply(&movement).unwrap().commit();
    assert_eq!(d.score(), SimpleScore::new(4));
    d.assert_consistent().unwrap();
}
#[test]
fn grouped_rows_match_scan_through_replacements_deletions_and_rollback() {
    for pruning in [false, true] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if pruning {
            b = b.with_activity_pruning();
        }
        let s = b.source::<usize, (usize, i64)>();
        let groups = b.group_collect(s.port(), |r| *r).unwrap();
        let settled = b.coalesce_updates(&groups).unwrap();
        let data = b.dataset(&settled).unwrap();
        b.score("groups", &settled, |(_, stats)| SimpleScore::new(stats.sum))
            .unwrap();
        let mut g = b.build();
        let mut rows = BTreeMap::new();
        let mut seed = 74743u64;
        let expected = |rows: &BTreeMap<usize, (usize, i64)>| {
            let mut sums = BTreeMap::<usize, (usize, i64)>::new();
            for (group, value) in rows.values() {
                let entry = sums.entry(*group).or_default();
                entry.0 += 1;
                entry.1 += *value;
            }
            sums
        };
        for round in 0..180 {
            let mut next = rows.clone();
            let mut batch = StreamBatch::new();
            for _ in 0..7 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let x = seed >> 32;
                let id = (x % 12) as usize;
                let value = (!x.is_multiple_of(4))
                    .then_some(((x / 12 % 4) as usize, (x / 48 % 25) as i64 - 12));
                if let Some(v) = value {
                    next.insert(id, v);
                } else {
                    next.remove(&id);
                }
                batch.push(&s, TupleChange { id, value }).unwrap();
            }
            if round % 19 == 0 {
                for id in 0..12 {
                    batch.push(&s, TupleChange::retract(id)).unwrap();
                }
                next.clear();
            }
            let wanted = expected(&next);
            let mut tx = g.transaction();
            tx.apply_batch(batch).unwrap();
            assert_eq!(
                tx.graph()
                    .unwrap()
                    .dataset_rows(&data)
                    .unwrap()
                    .iter()
                    .map(|(id, (_, s))| (*id, (s.count, s.sum)))
                    .collect::<BTreeMap<_, _>>(),
                wanted
            );
            assert_eq!(
                tx.score().unwrap(),
                SimpleScore::new(wanted.values().map(|(_, sum)| *sum).sum())
            );
            assert!(tx.score_evaluations() <= 4);
            if round % 5 == 0 {
                drop(tx);
            } else {
                tx.commit().unwrap();
                rows = next;
            }
            assert_eq!(
                g.dataset_rows(&data)
                    .unwrap()
                    .iter()
                    .map(|(id, (_, s))| (*id, (s.count, s.sum)))
                    .collect::<BTreeMap<_, _>>(),
                expected(&rows)
            );
        }
        g.assert_consistent().unwrap();
    }
}
