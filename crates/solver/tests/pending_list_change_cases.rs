// Adapted from Timefold Community PendingListChangeTrackerTest, pinned revision.
// Apache-2.0; see compat/provenance.json.
use std::rc::Rc;
use timetuck::*;
fn action() -> Rc<()> {
    Rc::new(())
}
#[test]
fn single_slot() {
    let mut t = PendingListChanges::new();
    let a = action();
    t.insert("e", a.clone()).unwrap();
    assert!(Rc::ptr_eq(&t.remove(&"e").unwrap(), &a));
    assert!(t.remove(&"e").is_none());
    assert_eq!(t.storage(), PendingListChangeStorage::Empty);
}
#[test]
fn reopen_same_entity() {
    let mut t = PendingListChanges::new();
    let a = action();
    let b = action();
    t.insert("e", a.clone()).unwrap();
    assert!(Rc::ptr_eq(&t.remove(&"e").unwrap(), &a));
    t.insert("e", b.clone()).unwrap();
    assert_eq!(t.storage(), PendingListChangeStorage::Single);
    assert!(Rc::ptr_eq(&t.remove(&"e").unwrap(), &b));
}
#[test]
fn distinct_entities_promote() {
    let mut t = PendingListChanges::new();
    let a = action();
    let b = action();
    t.insert("a", a.clone()).unwrap();
    t.insert("b", b.clone()).unwrap();
    assert_eq!(t.storage(), PendingListChangeStorage::Tree);
    assert!(Rc::ptr_eq(&t.remove(&"a").unwrap(), &a));
    assert!(Rc::ptr_eq(&t.remove(&"b").unwrap(), &b));
}
#[test]
fn duplicate_single_fails() {
    let mut t = PendingListChanges::new();
    t.insert("e", action()).unwrap();
    let error = t.insert("e", action()).unwrap_err();
    assert_eq!(error, ListRecordingError::AlreadyOpen);
    assert!(
        error
            .to_string()
            .contains("already has an open before list change")
    );
}
#[test]
fn duplicate_after_promotion_fails() {
    let mut t = PendingListChanges::new();
    t.insert("a", action()).unwrap();
    t.insert("b", action()).unwrap();
    let error = t.insert("a", action()).unwrap_err();
    assert_eq!(error, ListRecordingError::AlreadyOpen);
    assert!(
        error
            .to_string()
            .contains("already has an open before list change")
    );
}
#[test]
fn unknown_returns_none() {
    let mut t = PendingListChanges::<&str, Rc<()>>::new();
    assert!(t.remove(&"e").is_none());
}
#[test]
fn clear_promoted() {
    let mut t = PendingListChanges::new();
    t.insert("a", action()).unwrap();
    t.insert("b", action()).unwrap();
    t.clear();
    assert!(t.remove(&"a").is_none());
    assert!(t.remove(&"b").is_none());
    assert_eq!(t.storage(), PendingListChangeStorage::Empty);
}
#[test]
fn clear_single() {
    let mut t = PendingListChanges::new();
    t.insert("e", action()).unwrap();
    t.clear();
    assert!(t.remove(&"e").is_none());
    assert_eq!(t.storage(), PendingListChangeStorage::Empty);
}
