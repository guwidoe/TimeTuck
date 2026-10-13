// Adapted from pinned Community TESTs; Apache-2.0. See compat/provenance.json.
use timetuck::*;

#[test]
fn composing_list_index_out_of_bounds_negative() {
    let list = LazyConcat::<String>::new();
    assert!(list.is_empty());
    let error = list.get_signed(-1).unwrap_err();
    assert!(matches!(
        error,
        SignedIndexError::Index {
            index: -1,
            length: 0
        }
    ));
    assert!(error.to_string().contains("Index: -1"));
    assert!(error.to_string().contains("Size: 0"));
}
#[test]
fn boolean_range_get_index_negative() {
    assert_eq!(BooleanValueRange.get_signed(-1), None);
}
#[test]
fn array_list_add_at_negative_index() {
    let mut list = EntryList::new();
    list.push("a");
    assert!(matches!(
        list.insert_signed(-1, "x"),
        Err(SignedIndexError::Index { .. })
    ));
}
#[test]
fn array_list_entry_at_rejects_out_of_range_slots() {
    let mut list = EntryList::new();
    for i in 0..4 {
        list.push(i);
    }
    for index in [4, -1] {
        let error = list.entry_at_signed(index).unwrap_err();
        assert!(matches!(
            error,
            SignedIndexError::Slot { .. }
                | SignedIndexError::Entry(EntryListError::SlotOutOfBounds { .. })
        ));
        assert!(error.to_string().contains("slotCount"));
    }
}
