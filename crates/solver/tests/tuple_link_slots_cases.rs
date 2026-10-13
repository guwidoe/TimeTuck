// Adapted from Community TupleListTest.remove_nullsSlotsOnTuple, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[test]
fn remove_nulls_slots_on_tuple() {
    let mut list = EntryList::new();
    let first = list.push(());
    let _second = list.push(());
    list.remove(&first).unwrap();
    let [previous, next] = list.link_slots(&first).unwrap();
    assert!(previous.is_none());
    assert!(next.is_none());
}
