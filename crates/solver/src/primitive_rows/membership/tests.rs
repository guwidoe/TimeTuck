use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};
fn contents<I>(bits: &Membership<I>) -> Vec<[usize; 4]> {
    let mut out: Vec<_> = bits.positions().collect();
    out.sort_unstable();
    out
}
fn set<I>(bits: &mut Membership<I>, id: &I, accepted: bool) {
    bits.prepare(id).0.set(accepted);
}

#[test]
fn membership_bits_preserve_boundaries_nested_captures_and_full_install_inside_outer_frames() {
    let mut bits = Membership::<(usize, usize)>::new().unwrap();
    for id in [
        (0, 0),
        (0, 15),
        (4095, 1023),
        (4095, 1024),
        (4096, 0),
        (usize::MAX, usize::MAX),
    ] {
        set(&mut bits, &id, true);
    }
    let base = contents(&bits);
    let full = bits.clone();
    let outer = bits.begin();
    set(&mut bits, &(0, 0), false);
    set(&mut bits, &(0, 16), true);
    let inner = bits.begin();
    set(&mut bits, &(0, 15), false);
    set(&mut bits, &(usize::MAX, usize::MAX), false);
    let candidate = contents(&bits);
    let capture = bits.capture(outer);
    let candidate_full = bits.clone();
    bits.commit(inner);
    bits.rollback(outer);
    assert_eq!(contents(&bits), base);
    let other = bits.begin();
    bits.clear();
    set(&mut bits, &(1, 1), true);
    bits.rollback(other);
    assert_eq!(contents(&bits), base);
    let outer = bits.begin();
    bits.install_capture(capture);
    assert_eq!(contents(&bits), candidate);
    bits.rollback(outer);
    assert_eq!(contents(&bits), base);
    let outer = bits.begin();
    bits.install(candidate_full);
    assert_eq!(contents(&bits), candidate);
    bits.rollback(outer);
    assert_eq!(contents(&bits), base);
    bits.clear();
    bits.install(full);
    assert_eq!(contents(&bits), base);
    assert_eq!(bits.len, base.len());
    assert!(bits.rows.len() <= ROWS);
    assert!(bits.rows.iter().all(|row| row.len() <= COLUMNS));
}

#[test]
fn membership_bits_prepare_and_noop_updates_do_not_publish_or_grow_and_journals_are_lifo() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Membership<usize>>();
    let mut bits = Membership::<(usize, usize)>::new().unwrap();
    let outer = bits.begin();
    {
        let (_prepared, old) = bits.prepare(&(4095, 1023));
        assert!(!old);
    }
    {
        let (_prepared, old) = bits.prepare(&(usize::MAX, usize::MAX));
        assert!(!old);
    }
    assert!(bits.rows.is_empty());
    assert!(bits.tree.is_empty());
    assert!(bits.frames[outer].is_empty());
    set(&mut bits, &(0, 0), true);
    set(&mut bits, &(0, 0), true);
    set(&mut bits, &(0, 1), false);
    assert_eq!(
        bits.frames[outer].len(),
        1,
        "unchanged membership needs no undo entry"
    );
    let inner = bits.begin();
    set(&mut bits, &(0, 15), true);
    assert!(catch_unwind(AssertUnwindSafe(|| bits.rollback(outer))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| bits.commit(outer))).is_err());
    bits.rollback(inner);
    bits.rollback(outer);
    assert!(bits.pages().next().is_none());
    assert!(bits.tree.is_empty());
    assert_eq!(bits.len, 0);
    bits.clear();
    assert!(
        bits.rows.is_empty(),
        "explicit clear releases retained rows"
    );
}

#[test]
fn membership_bits_validate_complete_fixed_and_sparse_pair_universes_including_max() {
    let mut bits = Membership::<usize>::new().unwrap();
    assert!(bits.complete_fixed(0));
    for id in 0..17 {
        set(&mut bits, &id, true);
    }
    assert!(bits.complete_fixed(17));
    assert!(!bits.complete_fixed(16));
    set(&mut bits, &15, false);
    assert!(!bits.complete_fixed(17));
    set(&mut bits, &usize::MAX, true);
    assert!(!bits.complete_fixed(17));
    assert!(contents(&bits).contains(&[0, 0, 0, usize::MAX]));
    let mut pairs = Membership::<(usize, usize)>::new().unwrap();
    set(&mut pairs, &(4095, 1023), true);
    assert!(pairs.fits_pair(4096, 1024));
    assert!(!pairs.fits_pair(4095, 1024));
    assert!(!pairs.fits_pair(4096, 1023));
    set(&mut pairs, &(usize::MAX, usize::MAX), true);
    assert!(!pairs.fits_pair(usize::MAX, usize::MAX));
    assert!(Membership::<String>::new().is_none());
}

#[test]
fn membership_retained_hot_rows_reuse_bounded_storage_through_nested_trials_and_captures() {
    let mut bits = Membership::<(usize, usize)>::new().unwrap();
    // The last hot prefix and final hot cell exercise the maximum retained
    // domain. Full-range IDs remain in the sparse fallback, outside that bound.
    let high = (4095usize, 1023usize);
    let outside = (4096usize, 1024usize);
    set(&mut bits, &high, true);
    let snapshot = bits.clone();
    let allocation = bits.rows[4095].as_ptr();
    let outer_capacity = bits.rows.capacity();
    let row_capacity = bits.rows[4095].capacity();
    let base = contents(&bits);
    for _ in 0..8 {
        let outer = bits.begin();
        set(&mut bits, &high, false);
        assert_eq!(bits.len, 0);
        assert!(bits.pages().next().is_none());
        assert!(bits.complete_fixed(0));
        let nested = bits.begin();
        set(&mut bits, &high, true);
        set(&mut bits, &outside, true);
        set(&mut bits, &(usize::MAX, usize::MAX), true);
        let candidate = contents(&bits);
        let captured = bits.capture(outer);
        bits.commit(nested);
        bits.rollback(outer);
        assert_eq!(contents(&bits), base);
        let enclosing = bits.begin();
        bits.install_capture(captured);
        assert_eq!(contents(&bits), candidate);
        bits.install(snapshot.clone());
        assert_eq!(contents(&bits), base);
        bits.rollback(enclosing);
        assert_eq!(contents(&bits), base);
        assert!(bits.tree.is_empty(), "fallback pages remain sparse");
        assert_eq!(
            bits.rows[4095].as_ptr(),
            allocation,
            "trials keep the trailing row allocation"
        );
        assert_eq!(bits.rows.capacity(), outer_capacity);
        assert_eq!(bits.rows[4095].capacity(), row_capacity);
        assert!(bits.rows.len() <= ROWS);
        assert!(bits.rows.iter().all(|row| row.len() <= COLUMNS));
        assert!(bits.rows.iter().map(Vec::len).sum::<usize>() <= ROWS * COLUMNS);
    }
    let clearing = bits.begin();
    bits.clear();
    assert!(
        bits.rows.is_empty(),
        "explicit journaled clear releases rows"
    );
    assert!(bits.pages().next().is_none());
    assert_eq!(bits.len, 0);
    bits.rollback(clearing);
    assert_eq!(contents(&bits), base);
    bits.clear();
    assert!(bits.rows.is_empty());
    assert!(bits.tree.is_empty());
    assert!(bits.pages().next().is_none());
    assert_eq!(bits.len, 0);
}
