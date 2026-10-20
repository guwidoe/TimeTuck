use super::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[test]
fn private_page_snapshots_copy_containers_and_share_exact_public_row_owners() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<PrimitiveRows<usize, i64>>();
    struct NoClone(i64);
    let mut rows = PrimitiveRows::new();
    assert!(rows.bind_primitive());
    for key in [0usize, 15, 16, 1023, 1024, usize::MAX] {
        rows.insert(key, NoClone(key as i64));
    }
    let saved = rows.clone();
    let (Storage::Paged(current), Storage::Paged(previous)) = (&rows.storage, &saved.storage)
    else {
        unreachable!()
    };
    for ((key, page), (saved_key, saved_page)) in current.pages.iter().zip(previous.pages.iter()) {
        assert_eq!(key, saved_key);
        assert!(!std::ptr::eq(page.as_ref(), saved_page.as_ref()));
        for (entry, saved_entry) in page.cells.iter().zip(&saved_page.cells) {
            match (entry, saved_entry) {
                (Some(entry), Some(saved_entry)) => {
                    assert!(Arc::ptr_eq(&entry.key, &saved_entry.key));
                    assert!(Arc::ptr_eq(&entry.value, &saved_entry.value));
                }
                (None, None) => {}
                _ => panic!("snapshot changed occupied cells"),
            }
        }
    }
    let key_owner = rows.get_key_value(&15).unwrap().0 as *const usize;
    let untouched = rows.get(&0).unwrap() as *const NoClone;
    let page_before = current
        .pages
        .get(&(current.map)(&15).page)
        .unwrap()
        .as_ref() as *const _;
    rows.insert(15, NoClone(-15));
    let Storage::Paged(current) = &rows.storage else {
        unreachable!()
    };
    assert_eq!(
        current
            .pages
            .get(&(current.map)(&15).page)
            .unwrap()
            .as_ref() as *const _,
        page_before
    );
    assert_eq!(rows.get_key_value(&15).unwrap().0 as *const _, key_owner);
    assert_eq!(rows.get(&0).unwrap() as *const _, untouched);
    assert_eq!(saved.get(&15).unwrap().0, 15);
    assert_eq!(rows.get(&15).unwrap().0, -15);
    assert_eq!(
        &saved.materialized()[&15] as *const _,
        saved.get(&15).unwrap() as *const _
    );
}

#[test]
fn page_snapshot_capture_and_nested_install_match_tree_drop_and_weak_lifetimes() {
    struct Value(&'static str, Rc<RefCell<Vec<&'static str>>>);
    impl Drop for Value {
        fn drop(&mut self) {
            self.1.borrow_mut().push(self.0);
        }
    }
    fn exercise(primitive: bool) -> Vec<Vec<&'static str>> {
        let trace = Rc::new(RefCell::new(Vec::new()));
        let mut checkpoints = Vec::new();
        let mut rows = PrimitiveRows::new();
        if primitive {
            assert!(rows.bind_primitive());
        }
        rows.insert(0usize, Value("base", trace.clone()));
        rows.insert(15usize, Value("untouched", trace.clone()));
        rows.insert(usize::MAX, Value("edge", trace.clone()));
        let baseline = rows.clone();
        let handles = rows.shared_entries();
        let key = Arc::downgrade(&handles[0].0);
        let base = Arc::downgrade(&handles[0].1);
        drop(handles);
        let outer = rows.begin_journal();
        rows.insert(0, Value("candidate", trace.clone()));
        let inner = rows.begin_journal();
        rows.remove(&15);
        rows.insert(16, Value("new", trace.clone()));
        rows.remove(&usize::MAX);
        rows.commit_journal(inner);
        let capture = rows.capture_current(outer);
        let current = rows.clone();
        let rewind = rows.journal_snapshot(outer);
        assert_eq!(rewind.get(&0).unwrap().0, "base");
        assert!(rewind.get(&16).is_none());
        rows.rollback_journal(outer);
        assert!(key.upgrade().is_some() && base.upgrade().is_some());
        checkpoints.push(trace.borrow().clone());
        // Install a full view inside a fresh outer journal, then restore it.
        let install = rows.begin_journal();
        rows.install_snapshot(current.clone());
        assert_eq!(rows.get(&0).unwrap().0, "candidate");
        assert!(rows.get(&15).is_none());
        rows.rollback_journal(install);
        assert_eq!(rows.get(&0).unwrap().0, "base");
        checkpoints.push(trace.borrow().clone());
        rows.install_capture(capture);
        assert_eq!(rows.get(&16).unwrap().0, "new");
        assert!(rows.get(&usize::MAX).is_none());
        drop(rewind);
        drop(baseline);
        assert!(base.upgrade().is_none());
        assert!(key.upgrade().is_some()); // Existing ID keeps its representative.
        checkpoints.push(trace.borrow().clone());
        drop(rows);
        assert!(key.upgrade().is_some()); // Retained full current view owns it.
        drop(current);
        assert!(key.upgrade().is_none());
        checkpoints.push(trace.borrow().clone());
        assert!(checkpoints[0].is_empty());
        assert!(checkpoints[1].is_empty());
        // Tree and page containers already have different final traversal order;
        // compare the exact retirement phase and multiplicity of every payload.
        for phase in &mut checkpoints {
            phase.sort_unstable();
        }
        assert_eq!(checkpoints[2], ["base", "edge", "untouched"]);
        assert_eq!(
            checkpoints[3],
            ["base", "candidate", "edge", "new", "untouched"]
        );
        checkpoints
    }
    assert_eq!(exercise(true), exercise(false));
}

#[test]
fn boxed_page_full_and_delta_views_never_call_borrowed_payload_clone() {
    struct Value<'a>(&'a Cell<usize>, i64);
    impl Clone for Value<'_> {
        fn clone(&self) -> Self {
            self.0.set(self.0.get() + 1);
            panic!("cache snapshots must only clone canonical Arc owners")
        }
    }
    let clones = Cell::new(0);
    let mut rows = PrimitiveRows::new();
    assert!(rows.bind_primitive());
    rows.insert((usize::MAX, 15usize), Value(&clones, 1));
    let saved = rows.clone();
    let outer = rows.begin_journal();
    rows.insert((usize::MAX, 15), Value(&clones, 2));
    let nested = rows.begin_journal();
    rows.insert((usize::MAX, 16), Value(&clones, 3));
    rows.commit_journal(nested);
    let capture = rows.capture_current(outer);
    let baseline = rows.journal_snapshot(outer);
    rows.rollback_journal(outer);
    rows.install_capture(capture);
    assert_eq!(rows.get(&(usize::MAX, 15)).unwrap().1, 2);
    assert_eq!(rows.get(&(usize::MAX, 16)).unwrap().1, 3);
    rows.install_snapshot(baseline);
    assert_eq!(rows.get(&(usize::MAX, 15)).unwrap().1, 1);
    assert!(rows.get(&(usize::MAX, 16)).is_none());
    assert_eq!(saved.get(&(usize::MAX, 15)).unwrap().1, 1);
    assert_eq!(clones.get(), 0);
}
