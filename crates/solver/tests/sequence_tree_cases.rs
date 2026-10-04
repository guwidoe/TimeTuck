// Adapted pinned Community ConsecutiveSetTreeTest; Apache-2.0.
// Fresh immutable snapshots replace Java live views; explicit IDs replace object identity.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
mod sequence_support;
use sequence_support::{check, components};
use timetuck::*;
type C = SequenceCollector<i64, i64, i64>;
fn row(v: i64) -> (i64, i64, i64) {
    (v, v, v)
}
fn build(values: &[i64]) -> C {
    let mut c = C::default();
    for &v in values {
        c.insert(&row(v));
    }
    c
}
fn runs(values: &[&[i64]]) -> Vec<Vec<(i64, i64, i64)>> {
    values
        .iter()
        .map(|v| v.iter().copied().map(row).collect())
        .collect()
}
#[test]
fn nonconsecutive_numbers() {
    let mut c = C::default();
    c.insert(&(1, 1, 3));
    c.insert(&(3, 3, 5));
    c.insert(&(7, 7, 6));
    check(
        &c.finish(),
        &[vec![(1, 1, 3)], vec![(3, 3, 5)], vec![(7, 7, 6)]],
        1,
    );
}
#[test]
fn consecutive_numbers() {
    check(
        &build(&[1, 2, 3, 5, 6, 7, 8]).finish(),
        &runs(&[&[1, 2, 3], &[5, 6, 7, 8]]),
        1,
    );
}
#[test]
fn duplicate_numbers() {
    let mut c = build(&[1, 2, 3, 3, 3]);
    check(&c.finish(), &runs(&[&[1, 2, 3]]), 1);
    for _ in 0..3 {
        assert!(c.remove(&row(3)));
    }
    let mut removed = row(3);
    removed.2 = 0;
    check(&c.finish(), &runs(&[&[1, 2]]), 1);
    assert_eq!(removed.2, 0);
}
#[test]
fn duplicate_indexes() {
    let mut c = C::default();
    for id in 0..3 {
        c.insert(&(id, 3, id));
    }
    for first in 0..3 {
        check(
            &c.finish(),
            &[(first..3).map(|id| (id, 3, id)).collect()],
            1,
        );
        assert!(c.remove(&(first, 3, first)));
    }
    check(&c.finish(), &[], 1);
}
#[test]
fn consecutive_reverse_numbers() {
    check(
        &build(&[3, 2, 1, 8, 7, 6, 5]).finish(),
        &runs(&[&[1, 2, 3], &[5, 6, 7, 8]]),
        1,
    );
}
#[test]
fn join_of_two_chains() {
    let mut c = build(&[1, 2, 3, 5, 6, 7, 8]);
    c.insert(&row(4));
    check(&c.finish(), &runs(&[&[1, 2, 3, 4, 5, 6, 7, 8]]), 1);
}
#[test]
fn break_of_chain() {
    let mut c = build(&[1, 2, 3, 4, 5, 6, 7]);
    let mut removed = row(4);
    assert!(c.remove(&removed));
    removed.2 = 8;
    check(&c.finish(), &runs(&[&[1, 2, 3], &[5, 6, 7]]), 1);
    assert_eq!(removed.2, 8);
}
#[test]
fn chain_removal() {
    let mut c = build(&[1, 2, 3, 5, 6, 7]);
    let mut removed = [row(1), row(2), row(3)];
    for i in [1, 0, 2] {
        assert!(c.remove(&removed[i]));
    }
    for (item, v) in removed.iter_mut().zip([3, 10, -1]) {
        item.2 = v;
    }
    check(&c.finish(), &runs(&[&[5, 6, 7]]), 1);
    assert_eq!(removed.map(|x| x.2), [3, 10, -1]);
}
#[test]
fn shortening_of_chain() {
    let mut c = build(&[1, 2, 3, 4, 5, 6, 7]);
    let mut end = row(7);
    assert!(c.remove(&end));
    end.2 = 3;
    check(&c.finish(), &runs(&[&[1, 2, 3, 4, 5, 6]]), 1);
    let mut start = row(1);
    assert!(c.remove(&start));
    start.2 = 3;
    check(&c.finish(), &runs(&[&[2, 3, 4, 5, 6]]), 1);
    assert_eq!((start.2, end.2), (3, 3));
}
fn random(mode: i64) {
    use std::collections::BTreeMap;
    let mut c = SequenceCollector::<i64, i64, i64, IntegerSequenceMetric<2>>::default();
    let mut counts = BTreeMap::<i64, (i64, usize)>::new();
    let mut steps = 0;
    for line in include_str!("fixtures/sequence-traces.txt").lines() {
        let f: Vec<_> = line.split_whitespace().collect();
        if f[0].parse::<i64>().unwrap() != mode {
            continue;
        }
        let v: i64 = f[2].parse().unwrap();
        let p: i64 = f[3].parse().unwrap();
        let value = (v, p, v);
        if f[1] == "a" {
            c.insert(&value);
            counts.entry(v).and_modify(|x| x.1 += 1).or_insert((p, 1));
        } else {
            assert!(c.remove(&value));
            let x = counts.get_mut(&v).unwrap();
            x.1 -= 1;
            if x.1 == 0 {
                counts.remove(&v);
            }
        }
        let expected = components(counts.iter().map(|(&v, &(p, _))| (v, p, v)).collect(), 2);
        check(&c.finish(), &expected, 2);
        steps += 1;
    }
    assert_eq!(steps, 1000);
}
#[test]
fn random_sequences() {
    random(0);
}
#[test]
fn random_sequences_with_duplicates() {
    random(1);
}
#[derive(Clone, Default)]
struct Days;
impl SequenceMetric<i64> for Days {
    type Distance = SignedDuration;
    fn distance(&self, a: &i64, b: &i64) -> SignedDuration {
        SignedDuration::from_seconds(b - a)
    }
    fn add(&self, a: SignedDuration, b: SignedDuration) -> SignedDuration {
        a.checked_add(b).unwrap()
    }
    fn zero(&self) -> SignedDuration {
        SignedDuration::ZERO
    }
    fn maximum_gap(&self) -> SignedDuration {
        SignedDuration::from_seconds(86400)
    }
}
#[test]
fn timeslot_consecutive() {
    let base = 946684800;
    let slot = |d: i64| {
        (
            d,
            base + d * 86400,
            (base + d * 86400, base + (d + 1) * 86400),
        )
    };
    let mut c = SequenceCollector::<i64, i64, (i64, i64), Days>::default();
    for d in [4, 1, 4, 3, 0, 5] {
        c.insert(&slot(d));
    }
    let chain = c.finish();
    assert_eq!(chain.sequences().len(), 2);
    assert_eq!(chain.breaks().len(), 1);
    for (s, ids) in chain.sequences().iter().zip([vec![0, 1], vec![3, 4, 5]]) {
        assert_eq!(
            s.items().iter().map(|x| x.value).collect::<Vec<_>>(),
            ids.iter().map(|&d| slot(d).2).collect::<Vec<_>>()
        );
        assert_eq!(
            *s.length(),
            SignedDuration::from_seconds(ids.len() as i64 * 86400)
        );
    }
    let gap = chain.first_break().unwrap();
    assert_eq!(gap.previous_sequence_end().value, slot(1).2);
    assert_eq!(gap.next_sequence_start().value, slot(3).2);
    assert_eq!(*gap.length(), SignedDuration::from_seconds(2 * 86400));
}
