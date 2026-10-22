//! Original independent collector controls; no upstream-method credit.
use crate::IntCollector;
use crate::int_collector_counts::lean::LeanCounts;
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
fn check(c: &IntCollector, expected: &BTreeMap<i64, usize>) {
    let summary = c.summary();
    assert_eq!(summary.count, expected.values().sum::<usize>());
    assert_eq!(summary.distinct_count, expected.len());
    assert_eq!(summary.min, expected.first_key_value().map(|(&v, _)| v));
    assert_eq!(summary.max, expected.last_key_value().map(|(&v, _)| v));
    assert_eq!(
        summary.sum,
        expected.iter().fold(0i64, |sum, (&v, &n)| sum
            .wrapping_add(v.wrapping_mul(n as i64)))
    );
    assert_eq!(c.summary_seed().materialize(), summary);
    for v in [i64::MIN, -99, -7, 0, 1, 2, 9, i64::MAX] {
        assert_eq!(
            c.values.frequency(v, summary.count),
            expected.get(&v).copied()
        );
    }
    let rebuilt: IntCollector = expected
        .iter()
        .flat_map(|(&v, &n)| std::iter::repeat_n(v, n))
        .collect();
    assert_eq!(*c, rebuilt);
}
#[test]
fn lean_counts_match_independent_multiset_edits_clones_and_complete_summaries() {
    let values = [i64::MIN, -7, 0, 1, 2, 9, i64::MAX];
    let mut seed = 73981u64;
    let mut c = IntCollector::new();
    let mut expected = BTreeMap::<i64, usize>::new();
    let mut snapshots = Vec::new();
    for step in 0..4000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let a = values[(seed >> 19) as usize % values.len()];
        let b = values[(seed >> 37) as usize % values.len()];
        match seed % 4 {
            0 | 1 => {
                c.insert(a);
                *expected.entry(a).or_default() += 1;
            }
            2 => {
                let exists = expected.contains_key(&a);
                assert_eq!(c.remove(a), exists);
                if exists {
                    let n = expected.get_mut(&a).unwrap();
                    *n -= 1;
                    if *n == 0 {
                        expected.remove(&a);
                    }
                }
            }
            _ => {
                let exists = expected.contains_key(&a);
                assert_eq!(c.replace_captured(a, b), exists);
                if exists {
                    let n = expected.get_mut(&a).unwrap();
                    *n -= 1;
                    if *n == 0 {
                        expected.remove(&a);
                    }
                    *expected.entry(b).or_default() += 1;
                }
            }
        }
        check(&c, &expected);
        if step % 193 == 0 {
            snapshots.push((c.clone(), expected.clone()));
        }
    }
    for (saved, model) in snapshots {
        check(&saved, &model);
    }
    for (value, count) in expected {
        for _ in 0..count {
            assert!(c.remove(value));
        }
    }
    assert_eq!(c, IntCollector::new());
    assert_eq!(
        format!("{c:?}"),
        "IntCollector { values: Inline { entries: [(0, 0), (0, 0)], len: 0 }, count: 0, sum: 0 }"
    );
}
#[test]
fn lean_count_promotion_demotion_near_limit_and_failed_overflow_keep_every_frequency() {
    let mut values = LeanCounts::Two([1, 2], usize::MAX - 2);
    values.increment(3, usize::MAX - 1);
    assert_eq!(values.frequency(1, usize::MAX), Some(usize::MAX - 2));
    assert!(values.remove(3, usize::MAX));
    assert!(matches!(values, LeanCounts::Tree(_)));
    assert!(values.equivalent(&LeanCounts::Two([1, 2], usize::MAX - 2), usize::MAX - 1));
    assert!(values.remove(2, usize::MAX - 1));
    assert_eq!(values.frequency(1, usize::MAX - 2), Some(usize::MAX - 2));
    assert!(matches!(values, LeanCounts::One(1)));
    let mut c = IntCollector {
        values: LeanCounts::One(7),
        count: usize::MAX,
        sum: 7i64.wrapping_mul(usize::MAX as i64),
    };
    let before = c.clone();
    assert!(catch_unwind(AssertUnwindSafe(|| c.insert(7))).is_err());
    assert_eq!(c, before);
    assert!(c.replace_captured(7, 9));
    assert!(c.replace_captured(9, 7));
    assert_eq!(c, before);
    if usize::BITS == 64 {
        assert_eq!(std::mem::size_of::<LeanCounts>(), 32);
        assert_eq!(std::mem::size_of::<IntCollector>(), 48);
    }
}

#[test]
fn lean_two_class_frequencies_preserve_both_removal_sides_and_replacement_boundaries() {
    for a in 1..8 {
        for b in 1..8 {
            let mut model = BTreeMap::from([(i64::MIN, a), (i64::MAX, b)]);
            let mut c: IntCollector = std::iter::repeat_n(i64::MIN, a)
                .chain(std::iter::repeat_n(i64::MAX, b))
                .collect();
            let saved = c.clone();
            check(&c, &model);
            assert!(!c.remove(123));
            for value in [i64::MIN, i64::MAX] {
                while model.contains_key(&value) {
                    assert!(c.replace_captured(value, 0));
                    let n = model.get_mut(&value).unwrap();
                    *n -= 1;
                    if *n == 0 {
                        model.remove(&value);
                    }
                    *model.entry(0).or_default() += 1;
                    check(&c, &model);
                }
            }
            for _ in 0..a + b {
                assert!(c.remove(0));
            }
            check(&c, &BTreeMap::new());
            check(&saved, &BTreeMap::from([(i64::MIN, a), (i64::MAX, b)]));
        }
    }
}
