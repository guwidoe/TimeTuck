//! Original sparse numeric contribution controls; no upstream-method credit.
use super::*;
use crate::SimpleScore;
use std::collections::BTreeMap;
type Id = ((usize, usize), (usize, usize));
fn put(rows: &mut Rows<Id, SimpleScore>, id: Id, value: i64) -> Option<SimpleScore> {
    let (prepared, old) = rows.prepare_change(&id, |old| old.copied());
    prepared.insert(id, SimpleScore::new(value));
    old
}
fn remove(rows: &mut Rows<Id, SimpleScore>, id: Id) -> Option<SimpleScore> {
    let (prepared, old) = rows.prepare_change(&id, |old| old.copied());
    prepared.remove();
    old
}
fn values(rows: &Rows<Id, SimpleScore>) -> Vec<(Id, SimpleScore)> {
    let values: Vec<_> = rows.iter().map(|(id, score)| (*id, *score)).collect();
    assert_eq!(rows.len(), values.len());
    values
}
#[test]
fn consuming_scores_sparse_rows_preserve_full_coordinates_same_page_cells_and_global_order() {
    let high = 1usize << (usize::BITS - 1);
    let ids = [
        ((0, 0), (0, 0)),
        ((0, 0), (4095, 1023)),
        ((0, 0), (4095, 1024)),
        ((0, 0), (4096, 0)),
        ((1, 2), (3, 0)),
        ((1, 2), (3, 15)),
        ((1, 2), (3, 16)),
        ((high, 0), (0, 0)),
        ((0, high), (0, 0)),
        ((0, 0), (high, 0)),
        ((0, 0), (0, high)),
        ((usize::MAX, usize::MAX), (usize::MAX, usize::MAX)),
    ];
    let mut rows = Rows::new().unwrap();
    let mut expected = BTreeMap::new();
    for (i, id) in ids.into_iter().rev().enumerate() {
        assert_eq!(put(&mut rows, id, i as i64), None);
        expected.insert(id, SimpleScore::new(i as i64));
    }
    assert_eq!(
        rows.sparse.len(),
        ids.len() - 2,
        "only two IDs use hot pages"
    );
    assert_eq!(
        values(&rows),
        expected.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>()
    );
    for id in ids {
        assert_eq!(
            put(&mut rows, id, -7),
            expected.insert(id, SimpleScore::new(-7))
        );
    }
    // These IDs shared a fallback page before this backend; removing one must
    // retain its neighboring cells and every high-coordinate distinction.
    assert_eq!(remove(&mut rows, ids[4]), expected.remove(&ids[4]));
    assert_eq!(
        values(&rows),
        expected.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>()
    );
    let before = values(&rows);
    let absent = ((9, 8), (7, 6));
    {
        let (_prepared, old) = rows.prepare_change(&absent, |old| old.copied());
        assert_eq!(old, None);
    }
    {
        let (_prepared, old) = rows.prepare_change(&ids[5], |old| old.copied());
        assert_eq!(old, Some(SimpleScore::new(-7)));
    }
    assert_eq!(values(&rows), before, "abandoned prepares publish nothing");
}
#[test]
fn consuming_scores_sparse_rows_every_preimage_zero_absence_nested_capture_full_install_and_outer_rollback()
 {
    let mut rows = Rows::new().unwrap();
    let hot = ((0, 0), (1, 0));
    let a = ((1, 2), (3, 0));
    let neighbor = ((1, 2), (3, 15));
    let max = ((usize::MAX, usize::MAX), (usize::MAX, usize::MAX));
    for (id, value) in [(hot, 1), (a, 0), (neighbor, 3)] {
        put(&mut rows, id, value);
    }
    let base = values(&rows);
    let outer = rows.begin();
    assert_eq!(put(&mut rows, a, 0), Some(SimpleScore::ZERO));
    assert_eq!(put(&mut rows, a, 7), Some(SimpleScore::ZERO));
    assert_eq!(remove(&mut rows, a), Some(SimpleScore::new(7)));
    assert_eq!(remove(&mut rows, a), None);
    assert_eq!(
        rows.frames[outer].len(),
        4,
        "every put/retract retains its preimage"
    );
    assert_eq!(rows.len(), 2, "absent zero and present zero are different");
    let inner = rows.begin();
    assert_eq!(put(&mut rows, a, 0), None);
    remove(&mut rows, neighbor);
    put(&mut rows, max, 9);
    put(&mut rows, hot, 5);
    let wanted = values(&rows);
    let capture = rows.capture(outer);
    let full = rows.clone();
    assert_eq!(values(&rows.journal_snapshot(outer)), base);
    rows.commit(inner);
    rows.rollback(outer);
    assert_eq!(values(&rows), base);
    let trial = rows.begin();
    rows.clear();
    put(&mut rows, max, 100);
    rows.rollback(trial);
    assert_eq!(values(&rows), base);
    let enclosing = rows.begin();
    rows.install_capture(capture);
    assert_eq!(values(&rows), wanted);
    let child = rows.begin();
    rows.clear();
    rows.rollback(child);
    assert_eq!(values(&rows), wanted);
    rows.clear();
    rows.install(full);
    assert_eq!(values(&rows), wanted);
    rows.rollback(enclosing);
    assert_eq!(values(&rows), base);
    assert_eq!(
        rows.sparse.len(),
        2,
        "dead sparse trial rows leave no map entries"
    );
}
