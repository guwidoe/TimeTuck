use super::*;
use crate::primitive_rows::{PrimitiveRows, RowsCapture, Storage};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn owner(rows: &PrimitiveRows<usize, i64>, key: usize) -> (*const usize, *const i64) {
    let (k, v) = rows.get_key_value(&key).unwrap();
    (k as *const _, v as *const _)
}

#[test]
fn retired_arcs_reuse_allocation_after_commit_rollback_and_direct_removal() {
    let mut rows = PrimitiveRows::new();
    assert!(rows.bind_primitive());
    rows.insert(0usize, 10i64);
    let original = owner(&rows, 0);
    let frame = rows.begin_journal();
    rows.insert(0, 20);
    let replacement = owner(&rows, 0);
    assert_ne!(original.1, replacement.1);
    rows.commit_journal(frame);
    rows.insert(1, 30);
    assert_eq!(owner(&rows, 1).1, original.1);
    let frame = rows.begin_journal();
    rows.insert(0, 40);
    let trial = owner(&rows, 0);
    rows.rollback_journal(frame);
    assert_eq!(owner(&rows, 0), replacement);
    rows.insert(2, 50);
    assert_eq!(owner(&rows, 2).1, trial.1);
    let retired = owner(&rows, 1);
    rows.remove(&1);
    rows.insert(usize::MAX, 60);
    assert_eq!(owner(&rows, usize::MAX), retired);
    assert_eq!(rows.get(&usize::MAX), Some(&60));
}

#[test]
fn nested_journals_captures_and_full_views_protect_actual_live_owners() {
    let mut rows = PrimitiveRows::new();
    assert!(rows.bind_primitive());
    rows.insert(0usize, 1i64);
    let original = owner(&rows, 0);
    let full = rows.clone();
    let materialized = rows.materialized().clone();
    let outer = rows.begin_journal();
    rows.insert(0, 2);
    let candidate = owner(&rows, 0);
    let inner = rows.begin_journal();
    rows.insert(0, 3);
    rows.commit_journal(inner);
    let capture = rows.capture_current(outer);
    let RowsCapture::Delta(delta) = &capture else {
        unreachable!()
    };
    let captured = delta.entries[0].1.as_ref().unwrap().value.as_ref() as *const i64;
    rows.rollback_journal(outer);
    assert_eq!(owner(&rows, 0), original);
    rows.insert(1, 4);
    assert_eq!(owner(&rows, 1).1, candidate.1);
    assert_ne!(owner(&rows, 1).1, original.1);
    assert_ne!(owner(&rows, 1).1, captured);
    assert_eq!(full.get(&0), Some(&1));
    assert_eq!(materialized[&0], 1);
    rows.remove(&1);
    rows.install_capture(capture);
    assert_eq!(rows.get(&0).unwrap() as *const _, captured);
    assert_eq!(rows.get(&0), Some(&3));
    let Storage::Paged(cloned) = rows.clone().storage else {
        unreachable!()
    };
    assert!(cloned.pools.keys.slots.is_empty());
    assert!(cloned.pools.values.slots.is_empty());
}

#[test]
fn weak_observers_reject_pooling_and_expire_at_the_original_retirement_point() {
    let mut rows = PrimitiveRows::new();
    assert!(rows.bind_primitive());
    rows.insert(0usize, 1i64);
    let handles = rows.shared_entries();
    let key = Arc::downgrade(&handles[0].0);
    let value = Arc::downgrade(&handles[0].1);
    let original = owner(&rows, 0);
    drop(handles);
    let frame = rows.begin_journal();
    rows.remove(&0);
    assert!(key.upgrade().is_some());
    assert!(value.upgrade().is_some());
    rows.commit_journal(frame);
    assert!(key.upgrade().is_none());
    assert!(value.upgrade().is_none());
    rows.insert(1, 2);
    assert_ne!(owner(&rows, 1).0, original.0);
    assert_ne!(owner(&rows, 1).1, original.1);
    let Storage::Paged(paged) = &rows.storage else {
        unreachable!()
    };
    assert!(paged.pools.keys.slots.is_empty());
    assert!(paged.pools.values.slots.is_empty());
}

#[test]
fn arbitrary_drop_payloads_and_user_clone_schedules_remain_unchanged() {
    struct Value(&'static str, Rc<RefCell<Vec<&'static str>>>);
    impl Drop for Value {
        fn drop(&mut self) {
            self.1.borrow_mut().push(self.0);
        }
    }
    let trace = Rc::new(RefCell::new(Vec::new()));
    let mut rows = PrimitiveRows::new();
    assert!(rows.bind_primitive());
    rows.insert(0usize, Value("old", trace.clone()));
    rows.insert(0, Value("replacement", trace.clone()));
    assert_eq!(&*trace.borrow(), &["old"]);
    let frame = rows.begin_journal();
    rows.insert(0, Value("trial", trace.clone()));
    rows.rollback_journal(frame);
    assert_eq!(&*trace.borrow(), &["old", "trial"]);
    rows.remove(&0);
    assert_eq!(&*trace.borrow(), &["old", "trial", "replacement"]);
    let Storage::Paged(paged) = &rows.storage else {
        unreachable!()
    };
    assert!(paged.pools.keys.slots.is_empty());
    assert!(paged.pools.values.slots.is_empty());
    struct NoDrop<'a>(&'a Cell<usize>);
    impl Clone for NoDrop<'_> {
        fn clone(&self) -> Self {
            self.0.set(self.0.get() + 1);
            Self(self.0)
        }
    }
    let clones = Cell::new(0);
    let mut numeric = PrimitiveRows::new();
    assert!(numeric.bind_primitive());
    numeric.insert(0usize, NoDrop(&clones));
    let frame = numeric.begin_journal();
    numeric.insert(0, NoDrop(&clones));
    numeric.commit_journal(frame);
    numeric.insert(1, NoDrop(&clones));
    numeric.remove(&1);
    assert_eq!(clones.get(), 0);
}

#[test]
fn pool_item_and_byte_caps_reject_oversized_and_overaligned_values() {
    let mut small = Pool::<usize>::new();
    for n in 0..200 {
        small.retire(Arc::new(n));
    }
    assert_eq!(small.slots.len(), MAX_ITEMS);
    #[repr(align(1024))]
    struct Aligned([u8; 1024]);
    let mut aligned = Pool::<Aligned>::new();
    for _ in 0..200 {
        aligned.retire(Arc::new(Aligned([1; 1024])));
    }
    let item = Layout::new::<[usize; 2]>()
        .extend(Layout::new::<Aligned>())
        .unwrap()
        .0
        .pad_to_align()
        .size()
        + mem::size_of::<Arc<Aligned>>();
    assert!(aligned.slots.len() * item <= MAX_BYTES);
    assert!(aligned.slots.len() < MAX_ITEMS);
    assert_eq!(aligned.slots[0].0[0], 1);
    let mut huge = Pool::<[u8; MAX_BYTES]>::new();
    huge.retire(Arc::new([0; MAX_BYTES]));
    assert!(huge.slots.is_empty());
}

#[test]
fn int_collector_opt_in_pools_only_reset_unique_outer_allocations() {
    use crate::IntCollector;
    let populated = || [1i64, 2, 3, 4].into_iter().collect::<IntCollector>();
    let mut rows = PrimitiveRows::new();
    assert!(rows.bind_primitive());
    assert!(rows.enable_int_collector_pool());
    rows.insert(0usize, populated());
    let old = rows.get(&0).unwrap() as *const IntCollector;
    let frame = rows.begin_journal();
    rows.insert(0, populated());
    rows.commit_journal(frame);
    let Storage::Paged(paged) = &rows.storage else {
        unreachable!()
    };
    assert_eq!(paged.pools.values.slots.len(), 1);
    assert_eq!(*paged.pools.values.slots[0], IntCollector::default());
    rows.insert(1, populated());
    assert_eq!(rows.get(&1).unwrap() as *const _, old);
    assert_eq!(rows.get(&1).unwrap().summary().count, 4);
    let saved = rows.clone();
    let held = rows.get(&0).unwrap() as *const IntCollector;
    let before = saved.get(&0).unwrap().summary();
    rows.insert(0, IntCollector::default());
    assert_eq!(saved.get(&0).unwrap().summary(), before);
    assert_eq!(saved.get(&0).unwrap() as *const _, held);
    let Storage::Paged(cloned) = &saved.storage else {
        unreachable!()
    };
    assert!(cloned.pools.values.slots.is_empty());
    assert!(cloned.pools.values.reset.is_some());
}

#[test]
fn abandoned_preflight_leaves_retired_allocations_available_without_cloning_values() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let mut rows = PrimitiveRows::new();
    assert!(rows.bind_primitive());
    rows.insert(0usize, 1i64);
    let frame = rows.begin_journal();
    rows.insert(0, 2);
    let retired = owner(&rows, 0).1;
    rows.rollback_journal(frame);
    let Storage::Paged(paged) = &rows.storage else {
        unreachable!()
    };
    let before = (paged.pools.keys.slots.len(), paged.pools.values.slots.len());
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let (_prepared, _) = rows.prepare_change(&usize::MAX, |old| assert!(old.is_none()));
            panic!("fallible output clone before publication");
        }))
        .is_err()
    );
    let Storage::Paged(paged) = &rows.storage else {
        unreachable!()
    };
    assert_eq!(
        (paged.pools.keys.slots.len(), paged.pools.values.slots.len()),
        before
    );
    rows.insert(usize::MAX, 3);
    assert_eq!(owner(&rows, usize::MAX).1, retired);
    assert_eq!(rows.get(&0), Some(&1));
}
