// Adapted from pinned Community SubPillarConfigPolicyTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::cmp::Ordering;
use timetuck::*;
fn comparator() -> PillarComparator {
    PillarComparator::new(|_, _| Ordering::Equal)
}
#[test]
fn without_subpillars() {
    let p = PillarSelectionPolicy::whole();
    assert!(!p.is_enabled());
    assert!(p.comparator().is_none());
    assert_eq!(p.sizes(), None);
}
#[test]
fn with_limited_subpillars() {
    let p = PillarSelectionPolicy::subsets(2, 10).unwrap();
    assert!(p.is_enabled());
    assert!(p.comparator().is_none());
    assert_eq!(p.sizes(), Some((2, 10)));
}
#[test]
fn unlimited_sequential() {
    let p = PillarSelectionPolicy::sequential_unlimited(comparator());
    assert!(p.is_enabled());
    assert!(p.comparator().is_some());
    assert_eq!(p.sizes(), Some((1, i32::MAX as usize)));
}
#[test]
fn sequential() {
    let p = PillarSelectionPolicy::sequential(3, 5, Some(comparator())).unwrap();
    assert!(p.is_enabled());
    assert!(p.comparator().is_some());
    assert_eq!(p.sizes(), Some((3, 5)));
}
#[test]
fn validation() {
    for (min, max) in [(0, 1), (1, 0), (2, 1)] {
        assert!(PillarSelectionPolicy::subsets(min, max).is_err());
        assert!(PillarSelectionPolicy::sequential(min, max, Some(comparator())).is_err());
    }
    assert_eq!(
        PillarSelectionPolicy::sequential(1, 2, None),
        Err(PillarSelectionPolicyError::MissingComparator)
    );
}
#[test]
fn equality_sizes() {
    let p = PillarSelectionPolicy::subsets(1, 2).unwrap();
    assert_eq!(p, p);
    let p2 = PillarSelectionPolicy::subsets(1, 2).unwrap();
    assert_eq!(p2, p);
    let p3 = PillarSelectionPolicy::subsets(1, 3).unwrap();
    assert_ne!(p3, p2);
    assert_ne!(PillarSelectionPolicy::subsets(2, 3).unwrap(), p3);
}
#[test]
fn equality_comparator() {
    let c = comparator();
    let p = PillarSelectionPolicy::sequential_unlimited(c.clone());
    assert_eq!(p, p);
    assert_eq!(PillarSelectionPolicy::sequential_unlimited(c), p);
    assert_ne!(PillarSelectionPolicy::sequential_unlimited(comparator()), p);
}
