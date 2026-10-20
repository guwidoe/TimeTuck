use super::*;
use crate::primitive_rows::{Entry as RowEntry, PrimitiveRows, Storage};
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

fn page() -> Owner<usize, usize> {
    Arc::new(Page::empty())
}
fn boundaries() -> Vec<PageKey> {
    vec![
        PageKey([0, 0, 0, 0]),
        PageKey([0, 0, 0, 63]),
        PageKey([0, 0, 0, 64]),
        PageKey([0, 0, 1, 0]),
        PageKey([0, 0, 1, 64]),
        PageKey([0, 0, 2, 0]),
        PageKey([0, 0, 4095, 63]),
        PageKey([0, 0, 4095, 64]),
        PageKey([0, 0, 4096, 0]),
        PageKey([0, 0, usize::MAX, usize::MAX]),
        PageKey([0, 1, 0, 0]),
        PageKey([usize::MAX, usize::MAX, usize::MAX, usize::MAX]),
    ]
}

#[test]
fn hybrid_directory_merges_boundary_holes_and_interleaved_fallback_in_both_directions() {
    let mut directory = Directory::new();
    let mut expected = boundaries();
    for key in expected.iter().rev() {
        directory.entry(*key).or_insert_with(page);
    }
    expected.sort_unstable();
    assert_eq!(directory.keys().collect::<Vec<_>>(), expected);
    assert_eq!(
        directory.keys().rev().collect::<Vec<_>>(),
        expected.iter().rev().copied().collect::<Vec<_>>()
    );
    assert_eq!(directory.values().len(), expected.len());
    let mut iter = directory.iter();
    let mut reference = expected.iter();
    while reference.len() > 0 {
        assert_eq!(iter.len(), reference.len());
        assert_eq!(iter.next().map(|(key, _)| key), reference.next().copied());
        assert_eq!(
            iter.next_back().map(|(key, _)| key),
            reference.next_back().copied()
        );
    }
    assert_eq!(iter.len(), 0);
    assert!(iter.next().is_none());
    assert!(iter.next_back().is_none());
    for key in &expected {
        assert!(directory.get(key).is_some());
        assert!(directory.get_mut(key).is_some());
    }
    assert!(directory.get(&PageKey([0, 0, 2, 2])).is_none());
    let removed = PageKey([0, 0, 1, 0]);
    directory.remove(&removed);
    expected.retain(|key| *key != removed);
    assert_eq!(directory.keys().collect::<Vec<_>>(), expected);
    directory.retain(|key, _| key.0[2] != 4095);
    expected.retain(|key| key.0[2] != 4095);
    assert_eq!(directory.keys().collect::<Vec<_>>(), expected);
}

#[test]
fn directory_preparation_does_not_grow_dense_holes_and_growth_is_sparse_and_bounded() {
    let mut directory = Directory::<usize, usize>::new();
    let hot = PageKey([0, 0, 4095, 63]);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let entry = directory.entry(hot);
            assert!(entry.get().is_none());
            panic!("prepublication failure");
        }))
        .is_err()
    );
    assert!(directory.dense.is_empty());
    assert!(directory.is_empty());
    let fallback = PageKey([0, 0, 4096, 0]);
    directory.entry(fallback).or_insert_with(page);
    assert!(directory.dense.is_empty());
    let pointer = directory.entry(hot).or_insert_with(page).as_ref() as *const Page<usize, usize>;
    assert_eq!(directory.dense.len(), 4096);
    assert_eq!(
        directory.dense.iter().filter(|row| !row.is_empty()).count(),
        1
    );
    assert_eq!(directory.dense.iter().map(Vec::len).sum::<usize>(), 64);
    assert_eq!(directory.get(&hot).unwrap().as_ref() as *const _, pointer);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _entry = directory.entry(hot);
            panic!("output clone failure");
        }))
        .is_err()
    );
    assert_eq!(directory.get(&hot).unwrap().as_ref() as *const _, pointer);
    directory.entry(hot).remove();
    assert!(directory.dense.is_empty());
    assert_eq!(directory.len(), 1);
    assert!(directory.get(&fallback).is_some());
}

#[test]
fn hybrid_page_rows_keep_sorted_materialization_representatives_and_nested_capture_rollback() {
    struct NoClone(i64);
    let keys = [
        (4095usize, 1023usize),
        (4095, 1024),
        (4096, 0),
        (0, 1024),
        (1, 0),
        (usize::MAX, usize::MAX),
        (0, usize::MAX),
    ];
    let mut rows = PrimitiveRows::new();
    assert!(rows.bind_primitive());
    for (n, key) in keys.into_iter().enumerate() {
        rows.insert(key, NoClone(n as i64));
    }
    let mut expected = keys
        .into_iter()
        .enumerate()
        .map(|(n, key)| (key, n as i64))
        .collect::<BTreeMap<_, _>>();
    let snapshot = rows.clone();
    assert_eq!(
        rows.iter()
            .map(|(key, value)| (*key, value.0))
            .collect::<BTreeMap<_, _>>(),
        expected
    );
    let materialized = rows.materialized().clone();
    let key = (4095usize, 1023usize);
    let original = rows.get_key_value(&key).unwrap();
    let original = (original.0 as *const _, original.1 as *const _);
    let outer = rows.begin_journal();
    rows.prepare_change(&key, |value| assert_eq!(value.unwrap().0, 0))
        .0
        .remove();
    rows.prepare_change(&key, |value| assert!(value.is_none()))
        .0
        .insert(key, NoClone(100));
    let fresh = rows.get_key_value(&key).unwrap();
    let fresh = (fresh.0 as *const _, fresh.1 as *const _);
    assert_ne!(original.0, fresh.0);
    let inner = rows.begin_journal();
    rows.prepare_change(&(4095, 1024), |_| ()).0.remove();
    rows.prepare_change(&(2usize, 1023usize), |_| ())
        .0
        .insert((2, 1023), NoClone(200));
    rows.commit_journal(inner);
    let capture = rows.capture_current(outer);
    rows.rollback_journal(outer);
    assert_eq!(rows.get_key_value(&key).unwrap().0 as *const _, original.0);
    let outer = rows.begin_journal();
    rows.install_capture(capture);
    let installed = rows.get_key_value(&key).unwrap();
    assert_eq!((installed.0 as *const _, installed.1 as *const _), fresh);
    expected.insert(key, 100);
    expected.remove(&(4095, 1024));
    expected.insert((2, 1023), 200);
    assert_eq!(
        rows.iter()
            .map(|(key, value)| (*key, value.0))
            .collect::<BTreeMap<_, _>>(),
        expected
    );
    assert_eq!(
        rows.materialized()
            .iter()
            .map(|(key, value)| (*key, value.0))
            .collect::<BTreeMap<_, _>>(),
        expected
    );
    rows.rollback_journal(outer);
    assert_eq!(rows.get_key_value(&key).unwrap().1 as *const _, original.1);
    assert_eq!(materialized[&key].0, 0);
    assert_eq!(snapshot.get(&key).unwrap().0, 0);
    let Storage::Paged(paged) = &rows.storage else {
        unreachable!()
    };
    assert!(paged.pages.dense.len() <= 4096);
    assert!(paged.pages.dense.iter().all(|row| row.len() <= 64));
}

#[test]
fn hybrid_directory_releases_interleaved_page_payloads_in_legacy_sorted_order() {
    struct Payload(PageKey, Rc<RefCell<Vec<PageKey>>>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.1.borrow_mut().push(self.0);
        }
    }
    // Deliberately exercise Arc-backed page ownership with a single-thread Drop trace.
    #[allow(clippy::arc_with_non_send_sync)]
    fn owner(key: PageKey, trace: &Rc<RefCell<Vec<PageKey>>>) -> Owner<usize, Payload> {
        let mut page = Page::empty();
        page.cells[0] = Some(RowEntry {
            key: Arc::new(0usize),
            value: Arc::new(Payload(key, trace.clone())),
        });
        page.occupied = 1;
        Arc::new(page)
    }
    let keys = boundaries();
    let direct = Rc::new(RefCell::new(Vec::new()));
    let legacy = Rc::new(RefCell::new(Vec::new()));
    let mut directory = Directory::new();
    let mut tree = BTreeMap::new();
    for key in keys.into_iter().rev() {
        directory.entry(key).or_insert_with(|| owner(key, &direct));
        tree.insert(key, owner(key, &legacy));
    }
    directory.clear();
    drop(tree);
    assert_eq!(*direct.borrow(), *legacy.borrow());
    assert!(directory.is_empty());
    assert!(directory.dense.is_empty());
}
