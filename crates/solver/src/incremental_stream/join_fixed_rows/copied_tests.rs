//! Original private copied-payload controls; shared ownership controls stay above.
use super::*;
type Key = (usize, usize);
type Row = (usize, usize, usize);
type Value = (Key, Row);
fn value(n: usize) -> Value {
    ((usize::MAX, n), (n, 1, usize::MAX))
}
fn rows() -> FixedRows<usize, Value> {
    let mut r = FixedRows::new(
        (0usize..4)
            .map(|id| (Arc::new(id), Arc::new(value(id))))
            .collect(),
        |id| *id,
    );
    r.enable_copied(copied::certify::<Key, Row>().unwrap());
    r
}
#[test]
fn consuming_join_rows_copied_fixed_rows_release_payload_arcs_but_keep_exact_canonical_ids() {
    let keys: Vec<_> = (0usize..4).map(Arc::new).collect();
    let payloads: Vec<_> = (0..4).map(|id| Arc::new(value(id))).collect();
    let weak: Vec<_> = payloads.iter().map(Arc::downgrade).collect();
    let mut r = FixedRows::new(keys.iter().cloned().zip(payloads).collect(), |id| *id);
    r.enable_copied(copied::certify::<Key, Row>().unwrap());
    assert!(weak.iter().all(|v| v.upgrade().is_none()));
    assert!(r.values.is_empty());
    for (id, (key, v)) in r.iter().enumerate() {
        assert!(std::ptr::eq(key, keys[id].as_ref()));
        assert_eq!(*v, value(id));
    }
    let outer = r.begin();
    r.insert(0, value(10));
    let before = Arc::strong_count(&keys[0]);
    let capture = r.capture_current(outer);
    assert_eq!(
        Arc::strong_count(&keys[0]),
        before,
        "touched capture does not copy ID universe"
    );
    assert!(capture.copied.is_some());
    assert!(capture.values.is_empty(), "no Arc-valued capture");
    r.rollback(outer);
    let install = r.begin();
    r.install_capture(capture);
    assert_eq!(r.get(&0), Some(&value(10)));
    r.rollback(install);
    assert_eq!(r.get(&0), Some(&value(0)));
}
#[test]
fn consuming_join_rows_copied_fixed_rows_nested_first_originals_full_capture_clear_restore() {
    let mut r = rows();
    let full = r.clone();
    let canonical = Arc::as_ptr(&r.keys[0]);
    let outer = r.begin();
    r.insert(0, value(10));
    r.insert(0, value(11));
    let child = r.begin();
    r.remove(&0);
    r.insert(0, value(12));
    r.remove(&2);
    r.commit(child);
    let child = r.begin();
    r.insert(3, value(13));
    let capture = r.capture_current(outer);
    r.rollback(child);
    assert_eq!(r.get(&3), Some(&value(3)));
    r.rollback(outer);
    assert_eq!(r.get(&0), Some(&value(0)));
    assert_eq!(r.get(&2), Some(&value(2)));
    let outer = r.begin();
    r.install_capture(capture);
    assert_eq!(r.get(&0), Some(&value(12)));
    assert!(r.get(&2).is_none());
    assert_eq!(r.get(&3), Some(&value(13)));
    r.clear();
    r.install(full);
    assert_eq!(r.get(&0), Some(&value(0)));
    r.rollback(outer);
    for id in 0..4 {
        assert_eq!(r.get(&id), Some(&value(id)));
    }
    assert_eq!(Arc::as_ptr(&r.keys[0]), canonical);
    r.remove(&usize::MAX);
    assert!(r.get(&usize::MAX).is_none());
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || r.insert(usize::MAX, value(0))
        ))
        .is_err()
    );
    assert_eq!(r.get(&0), Some(&value(0)));
}

#[test]
fn consuming_join_rows_copy_certificate_does_not_infer_user_purity_from_copy_or_nodrop() {
    #[derive(Copy)]
    struct UserCopy(usize);
    #[allow(clippy::non_canonical_clone_impl)]
    impl Clone for UserCopy {
        fn clone(&self) -> Self {
            panic!("observable user Clone");
        }
    }
    #[derive(Clone)]
    struct UserDrop;
    impl Drop for UserDrop {
        fn drop(&mut self) {
            panic!("user Drop must not be manufactured");
        }
    }
    #[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
    struct UserKey(usize);
    assert!(!std::mem::needs_drop::<UserCopy>());
    assert!(copied::certify::<usize, UserCopy>().is_none());
    assert!(copied::certify::<usize, UserDrop>().is_none());
    assert!(copied::certify::<UserKey, Row>().is_none());
    // Read the custom Copy payload so the negative control remains warning-free
    // without ever invoking its intentionally noncanonical Clone.
    assert_eq!(UserCopy(7).0, 7);
    assert_eq!(UserKey(9).0, 9);
}
