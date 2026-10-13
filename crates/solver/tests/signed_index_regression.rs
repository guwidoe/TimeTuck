use std::cell::Cell;
use timetuck::*;

#[test]
fn signed_reads_preserve_laziness_cached_errors_and_range_edges() {
    let calls = Cell::new(0);
    let mut list = LazyConcat::new();
    list.push_deferred(2, || {
        calls.set(calls.get() + 1);
        vec![7]
    })
    .unwrap();
    for index in [i128::MIN, -1, i128::MAX, 2] {
        assert!(list.get_signed(index).is_err());
    }
    assert_eq!(calls.get(), 0);
    for _ in 0..2 {
        assert!(matches!(
            list.get_signed(0),
            Err(SignedIndexError::Lazy(
                LazyConcatError::SupplierLength { .. }
            ))
        ));
    }
    assert_eq!(calls.get(), 1);
    assert_eq!(BooleanValueRange.get_signed(0), Some(false));
    assert_eq!(BooleanValueRange.get_signed(1), Some(true));
    assert_eq!(BooleanValueRange.get_signed(2), None);
    let range = IntegerValueRange::new(-2, 2, 1).unwrap();
    assert_eq!(range.get_signed(0), Some(-2));
    assert_eq!(range.get_signed(3), Some(1));
    assert_eq!(range.get_signed(i128::MIN), None);
    assert_eq!(range.get_signed(i128::MAX), None);
}
#[test]
fn failed_signed_insertion_preserves_cursor_revision_holes_and_handles() {
    let mut list = EntryList::new();
    let entries: Vec<_> = (0..20).map(|i| list.push(i)).collect();
    list.remove(&entries[1]).unwrap();
    let slots = list.slot_count();
    let positions: Vec<_> = entries.iter().map(ListEntry::position).collect();
    let before = list.snapshot();
    let mut cursor = list.cursor(0).unwrap();
    for index in [i128::MIN, -1, i128::MAX, 21] {
        assert!(list.insert_signed(index, 99).is_err());
        assert!(list.entry_at_signed(index).is_err());
    }
    assert_eq!(list.slot_count(), slots);
    assert_eq!(list.snapshot(), before);
    assert_eq!(*cursor.next(&list).unwrap().unwrap(), 0);
    assert_eq!(
        entries.iter().map(ListEntry::position).collect::<Vec<_>>(),
        positions
    );
    assert!(list.entry_at_signed(1).unwrap().is_none());
    assert!(
        list.entry_at_signed(19)
            .unwrap()
            .unwrap()
            .same_entry(&entries[19])
    );
    let inserted = list.insert_signed(0, 99).unwrap();
    assert_eq!(*inserted.value(), 99);
    assert!(
        list.entry_at_signed(0)
            .unwrap()
            .unwrap()
            .same_entry(&inserted)
    );
}
