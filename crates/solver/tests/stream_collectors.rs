// Adapted from Timefold Community 2.7.0 InnerUniConstraintCollectorsTest.
// Apache-2.0; exact method provenance is in compat/provenance.json.
// Immutable captured integers and remove(value) replace Runnable retractors.
use timetuck::IntCollector;

#[test]
fn count() {
    let mut c = IntCollector::new();
    assert_eq!(c.summary().count, 0);
    c.insert(2);
    assert_eq!(c.summary().count, 1);
    c.insert(1);
    assert_eq!(c.summary().count, 2);
    c.insert(1);
    assert_eq!(c.summary().count, 3);
    assert!(c.remove(1));
    assert_eq!(c.summary().count, 2);
    assert!(c.remove(1));
    assert_eq!(c.summary().count, 1);
    assert!(c.remove(2));
    assert_eq!(c.summary().count, 0);
}

#[test]
fn count_distinct() {
    let mut c = IntCollector::new();
    assert_eq!(c.summary().distinct_count, 0);
    c.insert(2);
    assert_eq!(c.summary().distinct_count, 1);
    c.insert(1);
    assert_eq!(c.summary().distinct_count, 2);
    c.insert(1);
    assert_eq!(c.summary().distinct_count, 2);
    assert!(c.remove(1));
    assert_eq!(c.summary().distinct_count, 2);
    assert!(c.remove(1));
    assert_eq!(c.summary().distinct_count, 1);
    assert!(c.remove(2));
    assert_eq!(c.summary().distinct_count, 0);
}

#[test]
fn sum() {
    let mut c = IntCollector::new();
    assert_eq!(c.summary().sum, 0);
    c.insert(2);
    assert_eq!(c.summary().sum, 2);
    c.insert(1);
    assert_eq!(c.summary().sum, 3);
    c.insert(1);
    assert_eq!(c.summary().sum, 4);
    assert!(c.remove(1));
    assert_eq!(c.summary().sum, 3);
    assert!(c.remove(1));
    assert_eq!(c.summary().sum, 2);
    assert!(c.remove(2));
    assert_eq!(c.summary().sum, 0);
}

#[test]
fn min_comparable() {
    let mut c = IntCollector::new();
    assert_eq!(c.summary().min, None);
    c.insert(2);
    assert_eq!(c.summary().min, Some(2));
    c.insert(1);
    assert_eq!(c.summary().min, Some(1));
    c.insert(1);
    assert_eq!(c.summary().min, Some(1));
    assert!(c.remove(1));
    assert_eq!(c.summary().min, Some(1));
    assert!(c.remove(1));
    assert_eq!(c.summary().min, Some(2));
    assert!(c.remove(2));
    assert_eq!(c.summary().min, None);
}

#[test]
fn max_comparable() {
    let mut c = IntCollector::new();
    assert_eq!(c.summary().max, None);
    c.insert(2);
    assert_eq!(c.summary().max, Some(2));
    c.insert(1);
    assert_eq!(c.summary().max, Some(2));
    c.insert(2);
    assert_eq!(c.summary().max, Some(2));
    assert!(c.remove(2));
    assert_eq!(c.summary().max, Some(2));
    assert!(c.remove(2));
    assert_eq!(c.summary().max, Some(1));
    assert!(c.remove(1));
    assert_eq!(c.summary().max, None);
}
