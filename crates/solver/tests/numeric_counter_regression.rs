use std::collections::BTreeMap;
use timetuck::*;
#[test]
fn reclaimed_slots_match_independent_counts_and_clone_isolation() {
    let mut c = OccurrenceCounts::new();
    let mut oracle = BTreeMap::<u32, u32>::new();
    let mut maximum = 0;
    for step in 0..3000 {
        let key = (step * 17 + step / 3) % 31;
        if step % 3 == 0 && oracle.contains_key(&key) {
            let n = oracle.get_mut(&key).unwrap();
            *n -= 1;
            let expected = *n;
            if *n == 0 {
                oracle.remove(&key);
            }
            assert_eq!(c.decrement(key).unwrap(), expected);
        } else {
            let n = oracle.entry(key).or_default();
            *n += 1;
            assert_eq!(c.increment(key).unwrap(), *n);
        }
        maximum = maximum.max(oracle.len());
        assert_eq!(c.capacity(), maximum);
        for k in 0..31 {
            assert_eq!(c.get_count(k), oracle.get(&k).copied().unwrap_or(0));
        }
    }
    let copy = c.clone();
    let capacity = c.capacity();
    for (key, n) in oracle {
        for _ in 0..n {
            c.decrement(key).unwrap();
        }
        assert!(copy.get_count(key) > 0);
    }
    assert_eq!(c.to_string(), "{}");
    assert_eq!(c.capacity(), capacity);
    assert_eq!(
        c.decrement(100),
        Err(OccurrenceCountError::MissingCount(100))
    );
    assert_eq!(c.capacity(), capacity);
    c.increment(100).unwrap();
    assert_eq!(c.capacity(), capacity);
    assert_eq!(copy.get_count(100), 0);
}
#[test]
fn explicit_wrapping_and_casts_are_stable_in_debug_and_release() {
    let mut i = SignedCounter32::new(i32::MAX);
    assert_eq!(i.increment(), i32::MIN);
    assert_eq!(i.decrement(), i32::MAX);
    let mut l = SignedCounter64::new(i64::MIN);
    assert_eq!(l.decrement(), i64::MAX);
    assert_eq!(l.increment(), i64::MIN);
    assert_eq!(SignedCounter64::new(0x1_0000_0001).as_i32(), 1);
    assert_eq!(SignedCounter32::new(-1).as_i64(), -1);
    assert!(SignedCounter64::new(-1) < SignedCounter64::new(0));
}
