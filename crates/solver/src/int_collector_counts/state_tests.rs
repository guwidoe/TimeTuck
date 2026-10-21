//! Original representation and independent multiset regression; no upstream credit.
use super::*;
use std::collections::BTreeMap;
fn check(counts: &IntCollectorCounts, model: &BTreeMap<i64, usize>) {
    assert_eq!(counts.len(), model.len());
    assert_eq!(counts.min(), model.first_key_value().map(|(&v, _)| v));
    assert_eq!(counts.max(), model.last_key_value().map(|(&v, _)| v));
    for value in [i64::MIN, -10, -1, 0, 1, 7, 10, i64::MAX] {
        assert_eq!(counts.frequency(value), model.get(&value).copied());
    }
    let mut rebuilt = IntCollectorCounts::default();
    for (&v, &n) in model {
        for _ in 0..n {
            rebuilt.increment(v);
        }
    }
    assert_eq!(counts, &rebuilt);
}
#[test]
fn compact_states_match_multiset_edits_extremes_and_independent_snapshots() {
    let mut counts = IntCollectorCounts::default();
    let mut model = BTreeMap::<i64, usize>::new();
    let mut seed = 913u64;
    let mut saved = Vec::new();
    let values = [i64::MIN, -10, -1, 0, 1, 7, 10, i64::MAX];
    for step in 0..6000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let a = values[((seed >> 16) as usize) % values.len()];
        let b = values[((seed >> 35) as usize) % values.len()];
        match seed % 4 {
            0 | 1 => {
                counts.increment(a);
                *model.entry(a).or_default() += 1;
            }
            2 => {
                let present = model.contains_key(&a);
                assert_eq!(counts.remove(a), present);
                if let Some(n) = model.get_mut(&a) {
                    *n -= 1;
                    if *n == 0 {
                        model.remove(&a);
                    }
                }
            }
            _ => {
                let present = model.contains_key(&a);
                assert_eq!(counts.replace(a, b), present);
                if present {
                    let n = model.get_mut(&a).unwrap();
                    *n -= 1;
                    if *n == 0 {
                        model.remove(&a);
                    }
                    *model.entry(b).or_default() += 1;
                }
            }
        }
        check(&counts, &model);
        if step % 199 == 0 {
            saved.push((counts.clone(), model.clone()));
        }
    }
    for (saved, expected) in saved {
        check(&saved, &expected);
    }
    for (key, n) in model {
        for _ in 0..n {
            assert!(counts.remove(key));
        }
    }
    assert_eq!(counts, IntCollectorCounts::default());
    assert_eq!(
        format!("{counts:?}"),
        "Inline { entries: [(0, 0), (0, 0)], len: 0 }"
    );
    counts.increment(i64::MAX);
    counts.increment(i64::MIN);
    assert_eq!(
        format!("{counts:?}"),
        format!(
            "Inline {{ entries: [({}, 1), ({}, 1)], len: 2 }}",
            i64::MIN,
            i64::MAX
        )
    );
    println!(
        "counts bytes={},collector bytes={}",
        std::mem::size_of::<IntCollectorCounts>(),
        std::mem::size_of::<crate::IntCollector>()
    );
}
