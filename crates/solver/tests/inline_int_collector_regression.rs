// Original regression coverage for small/tree frequency storage transitions.
use std::collections::BTreeMap;
use timetuck::*;
fn summary(values: &BTreeMap<i64, usize>) -> IntSummary {
    IntSummary {
        count: values.values().sum(),
        distinct_count: values.len(),
        sum: values
            .iter()
            .fold(0i64, |s, (&v, &n)| s.wrapping_add(v.wrapping_mul(n as i64))),
        min: values.first_key_value().map(|(&v, _)| v),
        max: values.last_key_value().map(|(&v, _)| v),
    }
}
#[test]
fn repeated_promotion_demotion_and_snapshots_match_an_independent_multiset() {
    let mut collector = IntCollector::new();
    let mut expected = BTreeMap::new();
    let mut held = vec![];
    for round in 0..40 {
        let values: Vec<_> = (0..20)
            .map(|i| i - 10 + round)
            .chain([i64::MIN, i64::MAX, 0, 0, -1])
            .collect();
        for &value in &values {
            collector.insert(value);
            *expected.entry(value).or_insert(0) += 1;
            assert_eq!(collector.summary(), summary(&expected));
        }
        held.push((collector.clone(), collector.summary()));
        for &value in values.iter().rev() {
            assert!(collector.remove(value));
            let count = expected.get_mut(&value).unwrap();
            *count -= 1;
            if *count == 0 {
                expected.remove(&value);
            }
            assert_eq!(collector.summary(), summary(&expected));
            // Equality must not expose whether this state previously used a tree.
            let rebuilt: IntCollector = expected
                .iter()
                .flat_map(|(&v, &n)| std::iter::repeat_n(v, n))
                .collect();
            assert_eq!(collector, rebuilt);
        }
        assert!(!collector.remove(500));
        assert_eq!(collector, IntCollector::new());
    }
    for (snapshot, expected) in held {
        assert_eq!(snapshot.summary(), expected);
    }
}
#[test]
fn collector_equality_ignores_insertion_and_retraction_history() {
    for size in 0..=12 {
        let a: IntCollector = (0..size).flat_map(|v| [v, v]).collect();
        let b: IntCollector = (0..size).rev().flat_map(|v| [v, v]).collect();
        assert_eq!(a, b);
        let mut c: IntCollector = (-20..20).collect();
        for v in -20..20 {
            if !(0..size).contains(&v) {
                assert!(c.remove(v));
            }
        }
        for v in 0..size {
            c.insert(v);
        }
        assert_eq!(a, c);
    }
}
