use super::*;

fn bound() -> GroupCollectors<usize> {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<GroupCollectors<(usize, usize)>>();
    let mut rows = GroupCollectors::new();
    assert!(rows.bind_primitive());
    assert!(rows.enable_int_collector_pool());
    rows.insert(15, [1, 2, 3].into_iter().collect());
    rows.insert(16, [7, 7].into_iter().collect());
    rows
}
fn contents(rows: &GroupCollectors<usize>) -> Vec<(usize, IntCollector)> {
    rows.iter()
        .map(|(key, value)| (*key, value.clone()))
        .collect()
}
fn owner(rows: &GroupCollectors<usize>, key: usize) -> *const IntCollector {
    rows.get(&key).unwrap() as *const _
}

#[test]
fn inverse_edits_keep_each_logical_mutation_and_merge_with_stored_versions_in_reverse_order() {
    let mut rows = bound();
    let initial = contents(&rows);
    let original = owner(&rows, 15);
    let outer = rows.begin_journal();
    assert!(rows.try_edit(&15, ScalarEdit::Replace(1, 4)).is_some());
    assert_eq!(
        owner(&rows, 15),
        original,
        "actual live collector changed in place"
    );
    assert_eq!(rows.inverses[outer].len(), 1);
    assert!(rows.paged().frames[outer].is_empty());
    let middle = contents(&rows);
    let inner = rows.begin_journal();
    assert!(rows.try_edit(&15, ScalarEdit::Remove(2)).is_some());
    rows.insert(16, [8, 9].into_iter().collect());
    assert!(rows.try_edit(&15, ScalarEdit::Insert(5)).is_some());
    // An ordinary replacement between inverse edits must restore before the
    // earlier inverse; neither log may be replayed wholesale before the other.
    rows.insert(15, [10, 11, 12].into_iter().collect());
    assert!(rows.try_edit(&15, ScalarEdit::Replace(10, 13)).is_some());
    rows.commit_journal(inner);
    assert_eq!(rows.inverses[outer].len(), 4);
    assert_eq!(contents(&rows.journal_snapshot(outer)), initial);
    let rollback_only = rows.begin_journal();
    assert!(rows.try_edit(&15, ScalarEdit::Insert(14)).is_some());
    rows.remove(&16);
    rows.rollback_journal(rollback_only);
    assert_eq!(rows.get(&15).unwrap().summary().sum, 36);
    let saved = rows.clone();
    rows.rollback_journal(outer);
    assert_eq!(contents(&rows), initial);
    assert_eq!(owner(&rows, 15), original);
    assert_ne!(contents(&saved), initial);
    assert_ne!(contents(&saved), middle);
    assert_eq!(saved.get(&15).unwrap().summary().sum, 36);
}

#[test]
fn inverse_observers_first_taken_after_edits_copy_only_dirty_collectors_and_keep_canonical_keys() {
    let mut rows = bound();
    let baseline = contents(&rows);
    let untouched = owner(&rows, 16);
    let original = owner(&rows, 15);
    let original_key = rows.get_key_value(&15).unwrap().0 as *const usize;
    let outer = rows.begin_journal();
    rows.try_edit(&15, ScalarEdit::Replace(1, 4)).unwrap();
    let snapshot = rows.clone();
    assert_ne!(owner(&snapshot, 15), original);
    assert_eq!(owner(&snapshot, 16), untouched);
    assert_eq!(
        snapshot.get_key_value(&15).unwrap().0 as *const _,
        original_key
    );
    let capture = rows.capture_current(outer);
    let RowsCapture::Delta(delta) = &capture else {
        unreachable!()
    };
    assert_eq!(delta.entries.len(), 1);
    assert_ne!(
        delta.entries[0].1.as_ref().unwrap().value.as_ref() as *const _,
        original
    );
    let expected = contents(&snapshot);
    rows.try_edit(&15, ScalarEdit::Insert(6)).unwrap();
    rows.rollback_journal(outer);
    assert_eq!(owner(&rows, 15), original);
    assert_eq!(contents(&rows), baseline);
    assert_eq!(contents(&snapshot), expected);
    for value in [20, 30, 40] {
        let trial = rows.begin_journal();
        rows.try_edit(&15, ScalarEdit::Replace(1, value)).unwrap();
        rows.rollback_journal(trial);
    }
    let enclosing = rows.begin_journal();
    rows.install_capture(capture);
    assert_eq!(contents(&rows), expected);
    rows.try_edit(&15, ScalarEdit::Insert(9));
    rows.install_snapshot(snapshot.clone());
    let recapture = rows.capture_current(enclosing);
    rows.rollback_journal(enclosing);
    assert_eq!(contents(&rows), baseline);
    rows.install_capture(recapture);
    assert_eq!(contents(&rows), expected);
    assert_eq!(contents(&snapshot), expected);
}

#[test]
fn inverse_preexisting_shared_and_weak_owners_force_original_replacement_and_retirement() {
    let mut rows = bound();
    let held = rows.shared_entries();
    let weak = Arc::downgrade(&held[0].1);
    let original = held[0].1.as_ref() as *const _;
    let outer = rows.begin_journal();
    assert!(rows.try_edit(&15, ScalarEdit::Insert(4)).is_none());
    let mut copy = rows.get(&15).unwrap().clone();
    copy.insert(4);
    rows.insert(15, copy);
    let candidate = owner(&rows, 15);
    assert_ne!(candidate, original);
    assert!(rows.try_edit(&15, ScalarEdit::Insert(5)).is_some());
    assert_eq!(held[0].1.summary().sum, 6);
    rows.rollback_journal(outer);
    assert_eq!(owner(&rows, 15), original);
    drop(held);
    assert!(weak.upgrade().is_some());
    let outer = rows.begin_journal();
    assert!(
        rows.try_edit(&15, ScalarEdit::Insert(9)).is_none(),
        "Weak alone also prohibits mutation"
    );
    rows.remove(&15);
    assert!(
        weak.upgrade().is_some(),
        "ordinary undo retains the exact old owner"
    );
    rows.commit_journal(outer);
    assert!(
        weak.upgrade().is_none(),
        "pool must not retain a weak-observed owner"
    );
}

#[test]
fn inverse_nested_snapshot_replay_never_mutates_earlier_stored_undo_collectors() {
    let mut rows = bound();
    let original = contents(&rows);
    let outer = rows.begin_journal();
    rows.try_edit(&15, ScalarEdit::Insert(4)).unwrap();
    let middle = contents(&rows);
    let inner = rows.begin_journal();
    rows.insert(15, [50, 60, 70].into_iter().collect());
    rows.try_edit(&15, ScalarEdit::Replace(50, 80)).unwrap();
    let before = contents(&rows);
    assert_eq!(contents(&rows.journal_snapshot(inner)), middle);
    assert_eq!(contents(&rows.journal_snapshot(outer)), original);
    assert_eq!(contents(&rows), before);
    rows.rollback_journal(inner);
    assert_eq!(contents(&rows), middle);
    rows.rollback_journal(outer);
    assert_eq!(contents(&rows), original);
}

#[test]
fn inverse_dirty_tracking_does_not_copy_a_later_clean_representative_or_weaken_its_weak_lifetime() {
    let mut rows = bound();
    let outer = rows.begin_journal();
    rows.try_edit(&15, ScalarEdit::Insert(4)).unwrap();
    rows.remove(&15);
    rows.insert(15, [20, 30].into_iter().collect());
    let handles = rows.shared_entries();
    let clean = rows.get(&15).unwrap() as *const IntCollector;
    assert_eq!(handles[0].1.as_ref() as *const _, clean);
    let weak = Arc::downgrade(&handles[0].1);
    let snapshot = rows.clone();
    assert_eq!(owner(&snapshot, 15), clean);
    drop(handles);
    rows.remove(&15);
    rows.commit_journal(outer);
    assert!(
        weak.upgrade().is_some(),
        "full snapshot retains the actual clean owner"
    );
    drop(snapshot);
    assert!(
        weak.upgrade().is_none(),
        "weak-observed clean owner is not pooled"
    );
}

#[test]
fn inverse_collector_snapshots_preserve_actual_key_weak_lifetimes() {
    let mut rows = bound();
    let pos = (rows.paged().map)(&15);
    let weak = Arc::downgrade(&rows.paged().entry(pos).unwrap().key);
    let outer = rows.begin_journal();
    rows.try_edit(&15, ScalarEdit::Replace(1, 4)).unwrap();
    let captured = rows.capture_current(outer);
    let RowsCapture::Delta(delta) = &captured else {
        unreachable!()
    };
    assert!(Arc::ptr_eq(
        &delta.entries[0].1.as_ref().unwrap().key,
        &weak.upgrade().unwrap()
    ));
    rows.rollback_journal(outer);
    rows.remove(&15);
    assert!(
        weak.upgrade().is_some(),
        "captured summary owns the same canonical key"
    );
    drop(captured);
    assert!(
        weak.upgrade().is_none(),
        "a key Weak prevents pool retention"
    );
}

#[test]
fn compact_inverse_addresses_keep_full_keys_nested_rebase_and_dirty_observers() {
    #[cfg(target_pointer_width = "64")]
    assert_eq!(std::mem::size_of::<Inverse>(), 48);
    let mask = POSITION_MASK;
    let keys = [
        (0, 0, 0, 0),
        (mask, mask, mask, mask * CELLS + CELLS - 1),
        (mask + 1, 0, 0, 1),
        (0, mask + 1, 0, 2),
        (0, 0, mask + 1, 3),
        (0, 0, 0, (mask + 1) * CELLS),
        (usize::MAX, usize::MAX, usize::MAX, usize::MAX),
    ];
    let mut rows = GroupCollectors::new();
    assert!(rows.bind_primitive());
    assert!(rows.enable_int_collector_pool());
    for key in keys {
        rows.insert(key, [1, 2].into_iter().collect());
    }
    let observe = |rows: &GroupCollectors<(usize, usize, usize, usize)>| {
        rows.iter()
            .map(|(key, value)| (*key, value.clone()))
            .collect::<Vec<_>>()
    };
    let baseline = observe(&rows);
    let key_owner = rows.get_key_value(&keys[6]).unwrap().0 as *const _;
    let original = rows.get(&keys[6]).unwrap() as *const _;
    let outer = rows.begin_journal();
    for key in keys {
        rows.try_edit(&key, ScalarEdit::Insert(3)).unwrap();
    }
    let inner = rows.begin_journal();
    for key in keys {
        rows.try_edit(&key, ScalarEdit::Replace(1, 4)).unwrap();
    }
    let deepest = rows.begin_journal();
    for key in keys {
        rows.try_edit(&key, ScalarEdit::Remove(2)).unwrap();
    }
    rows.commit_journal(deepest);
    rows.commit_journal(inner);
    assert_eq!(rows.inverses[outer].len(), keys.len() * 3);
    for key in keys {
        assert_eq!(rows.get(&key).unwrap().summary().sum, 7);
    }
    let expected = observe(&rows);
    let snapshot = rows.clone();
    let capture = rows.capture_current(outer);
    assert_eq!(
        rows.get_key_value(&keys[6]).unwrap().0 as *const _,
        key_owner
    );
    assert_eq!(
        snapshot.get_key_value(&keys[6]).unwrap().0 as *const _,
        key_owner
    );
    assert_ne!(snapshot.get(&keys[6]).unwrap() as *const _, original);
    assert_eq!(observe(&rows.journal_snapshot(outer)), baseline);
    rows.rollback_journal(outer);
    assert_eq!(observe(&rows), baseline);
    assert_eq!(rows.get(&keys[6]).unwrap() as *const _, original);
    assert_eq!(observe(&snapshot), expected);
    let trial = rows.begin_journal();
    rows.install_capture(capture);
    assert_eq!(observe(&rows), expected);
    rows.rollback_journal(trial);
    assert_eq!(observe(&rows), baseline);
    rows.install_snapshot(snapshot);
    assert_eq!(observe(&rows), expected);
}
