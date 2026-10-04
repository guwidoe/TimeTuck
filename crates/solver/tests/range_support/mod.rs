//! Original independent pairwise connectivity and midpoint overlap reference.
#![allow(dead_code)]
use timetuck::*;
pub type Row = (i64, i64, i64, (i64, i64));
pub type C = ConnectedRangeCollector<i64, i64, (i64, i64)>;
pub type Chain = ConnectedRangeChain<i64, i64, (i64, i64), i128>;
pub fn row(id: i64, a: i64, b: i64) -> Row {
    (id, a, b, (a, b))
}
pub fn reference(mut rows: Vec<Row>) -> Vec<Vec<Row>> {
    rows.sort_by_key(|r| (r.1, r.2, r.0));
    let n = rows.len();
    let mut labels = (0..n).collect::<Vec<_>>();
    for i in 0..n {
        for j in i + 1..n {
            if rows[i].1 <= rows[j].2 && rows[j].1 <= rows[i].2 {
                let old = labels[j];
                let new = labels[i];
                for label in &mut labels {
                    if *label == old {
                        *label = new;
                    }
                }
            }
        }
    }
    let mut groups = std::collections::BTreeMap::<usize, Vec<Row>>::new();
    for (i, r) in rows.into_iter().enumerate() {
        groups.entry(labels[i]).or_default().push(r);
    }
    groups.into_values().collect()
}
pub fn bounds(rows: &[Row]) -> (Option<usize>, Option<usize>) {
    let points = rows
        .iter()
        .flat_map(|r| [r.1, r.2])
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let counts = points
        .windows(2)
        .map(|p| {
            let mid = p[0] as i128 + p[1] as i128;
            rows.iter()
                .filter(|r| 2 * r.1 as i128 <= mid && mid < 2 * r.2 as i128)
                .count()
        })
        .collect::<Vec<_>>();
    (counts.iter().min().copied(), counts.iter().max().copied())
}
pub fn check(c: &Chain, expected: &[Vec<Row>]) {
    assert_eq!(c.ranges().len(), expected.len());
    assert_eq!(c.gaps().len(), expected.len().saturating_sub(1));
    for (r, items) in c.ranges().iter().zip(expected) {
        let actual = r
            .items()
            .iter()
            .map(|x| (x.id, x.start, x.end, x.value))
            .collect::<Vec<_>>();
        assert_eq!(&actual, items);
        let start = items.iter().map(|r| r.1).min().unwrap();
        let end = items.iter().map(|r| r.2).max().unwrap();
        assert_eq!((*r.start(), *r.end()), (start, end));
        assert_eq!(r.count(), items.len());
        assert_eq!(*r.length(), end as i128 - start as i128);
        let (lo, hi) = bounds(items);
        assert_eq!((r.minimum_overlap(), r.maximum_overlap()), (lo, hi));
        assert_eq!(r.has_overlap(), hi.is_some_and(|n| n > 1));
    }
    for (i, g) in c.gaps().iter().enumerate() {
        let end = expected[i].iter().map(|r| r.2).max().unwrap();
        let start = expected[i + 1][0].1;
        assert_eq!(
            (*g.previous_range_end(), *g.next_range_start(), *g.length()),
            (end, start, start as i128 - end as i128)
        );
    }
}
pub fn verify(c: &C, rows: &[Row]) {
    check(&c.finish(), &reference(rows.to_vec()));
}
