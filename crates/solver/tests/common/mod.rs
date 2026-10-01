// Adapted assertion behavior from Timefold Community PlannerAssert, Apache-2.0.
// Pin: 8b815bc6b5f3eb81feedc94dea0885a93b267762.
// Native Rust equality/hash/order checks; see compat/provenance.json fixtures.
// Applicable attribution and license: THIRD_PARTY_NOTICES.md, compat/licenses/.
use std::{
    collections::hash_map::DefaultHasher,
    fmt::Debug,
    hash::{Hash, Hasher},
};

pub fn equal<T: Ord + Hash + Debug>(items: &[T]) {
    for a in items {
        for b in items {
            assert_eq!(a, b);
            assert_eq!(a.cmp(b), std::cmp::Ordering::Equal);
            let mut ha = DefaultHasher::new();
            let mut hb = DefaultHasher::new();
            a.hash(&mut ha);
            b.hash(&mut hb);
            assert_eq!(ha.finish(), hb.finish());
        }
    }
}
pub fn distinct<T: Ord + Debug>(items: &[T]) {
    for (i, a) in items.iter().enumerate() {
        for b in &items[i + 1..] {
            assert_ne!(a, b);
            assert_ne!(a.cmp(b), std::cmp::Ordering::Equal);
        }
    }
}
pub fn ordered<T: Ord + Debug>(items: &[T]) {
    for (i, a) in items.iter().enumerate() {
        assert_eq!(a.cmp(a), std::cmp::Ordering::Equal);
        for b in &items[i + 1..] {
            assert!(a < b, "{a:?} must precede {b:?}");
            assert!(b > a, "{b:?} must follow {a:?}");
        }
    }
}
