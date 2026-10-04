// Adapted pinned Community ConnectedRangeTrackerTest, Apache-2.0.
// Immutable snapshots replace live views; zero-length overlap sentinels become None.
// The source live-object hash-mutation test is intentionally not credited.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
mod range_support;
use range_support::*;
use timetuck::*;
fn build(rows: &[Row]) -> C {
    let mut c = C::default();
    for r in rows {
        c.insert(r);
    }
    c
}
#[test]
fn non_consecutive_ranges() {
    let rows = [row(0, 0, 2), row(1, 3, 4), row(2, 5, 7)];
    verify(&build(&rows), &rows);
}
#[test]
fn consecutive_ranges() {
    let rows = [row(0, 0, 2), row(1, 2, 4), row(2, 4, 7)];
    verify(&build(&rows), &rows);
}
#[test]
fn duplicate_ranges() {
    let rows = [row(0, 0, 2), row(0, 0, 2), row(1, 4, 7)];
    verify(&build(&rows), &rows);
}
#[test]
fn range_removal() {
    let mut rows = vec![row(0, 0, 2), row(1, 2, 4), row(2, 4, 7)];
    let mut c = build(&rows);
    let mut removed = rows.remove(1);
    removed.1 = 10;
    removed.2 = 12;
    removed.3 = (10, 12);
    assert!(c.remove(&removed));
    verify(&c, &rows);
}
#[test]
fn range_add_updating_old_gap() {
    let mut c = C::default();
    let mut rows = vec![];
    for r in [
        row(0, 1, 2),
        row(4, 9, 10),
        row(2, 4, 5),
        row(3, 6, 7),
        row(1, 3, 8),
    ] {
        c.insert(&r);
        rows.push(r);
        verify(&c, &rows);
    }
}
#[test]
fn overlapping_range() {
    let mut rows = vec![
        row(0, 0, 2),
        row(1, 1, 3),
        row(2, 2, 4),
        row(3, 5, 6),
        row(4, 7, 9),
        row(5, 7, 9),
    ];
    let mut c = build(&rows);
    verify(&c, &rows);
    for (id, a, b) in [(1, 0, 10), (5, 2, 4)] {
        let i = rows.iter().position(|r| r.0 == id).unwrap();
        let mut r = rows.remove(i);
        r.1 = a;
        r.2 = b;
        r.3 = (a, b);
        assert!(c.remove(&r));
        verify(&c, &rows);
    }
    let r = row(6, 6, 7);
    c.insert(&r);
    rows.push(r);
    verify(&c, &rows);
}
#[test]
fn hash_code_consistent_with_equals() {
    use std::hash::{Hash, Hasher};
    let rows = [row(0, 0, 2), row(1, 2, 4)];
    let a = build(&rows).finish();
    let b = build(&rows).finish();
    assert_eq!(a.ranges()[0], b.ranges()[0]);
    let hash = |v: &ConnectedRange<i64, i64, (i64, i64), i128>| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        v.hash(&mut h);
        h.finish()
    };
    assert_eq!(hash(&a.ranges()[0]), hash(&b.ranges()[0]));
}
#[test]
fn random_ranges() {
    use std::collections::BTreeMap;
    let mut c = C::default();
    let mut counts = BTreeMap::<(i64, i64), usize>::new();
    let mut last = 0;
    let mut steps = 0;
    for line in include_str!("fixtures/range-traces.txt").lines() {
        let f = line.split_whitespace().collect::<Vec<_>>();
        let trial = f[0].parse::<usize>().unwrap();
        if trial != last {
            assert_eq!(steps, 100);
            steps = 0;
            c = C::default();
            counts.clear();
            last = trial;
        }
        let a = f[2].parse().unwrap();
        let b = f[3].parse().unwrap();
        let r = row(a * 10 + b, a, b);
        if f[1] == "a" {
            c.insert(&r);
            *counts.entry((a, b)).or_default() += 1;
        } else {
            assert!(c.remove(&r));
            let n = counts.get_mut(&(a, b)).unwrap();
            *n -= 1;
            if *n == 0 {
                counts.remove(&(a, b));
            }
        }
        let rows = counts
            .iter()
            .flat_map(|(&(a, b), &n)| std::iter::repeat_n(row(a * 10 + b, a, b), n))
            .collect::<Vec<_>>();
        verify(&c, &rows);
        steps += 1;
    }
    assert_eq!((last, steps), (99, 100));
}
