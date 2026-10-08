//! Original tests for native entry navigation and mutation during clearing.
use std::panic::{AssertUnwindSafe, catch_unwind};
use timetuck::{EntryList, EntryListError};

#[test]
fn neighbors_and_relative_insertion_survive_gaps_and_compaction() {
    struct NonClone(usize);
    let mut list = EntryList::new();
    let entries: Vec<_> = (0..12).map(|n| list.push(NonClone(n))).collect();
    list.remove(&entries[4]).unwrap();
    assert_eq!(list.slot_count(), 12);
    assert!(
        list.next_entry(&entries[3])
            .unwrap()
            .unwrap()
            .same_entry(&entries[5])
    );
    assert!(
        list.previous_entry(&entries[5])
            .unwrap()
            .unwrap()
            .same_entry(&entries[3])
    );
    let inserted = list.insert_after(&entries[3], NonClone(40)).unwrap();
    assert!(
        list.next_entry(&inserted)
            .unwrap()
            .unwrap()
            .same_entry(&entries[5])
    );
    let front = list.push_front(NonClone(99));
    assert!(list.first_entry().unwrap().same_entry(&front));
    list.compact();
    assert_eq!(
        list.entries().map(|e| e.value().0).collect::<Vec<_>>(),
        [99, 0, 1, 2, 3, 40, 5, 6, 7, 8, 9, 10, 11]
    );
    let reverse: Vec<_> = list.entries().rev().map(|e| e.value().0).collect();
    assert_eq!(reverse, [11, 10, 9, 8, 7, 6, 5, 40, 3, 2, 1, 0, 99]);
    assert!(matches!(
        list.next_entry(&entries[4]),
        Err(EntryListError::Removed)
    ));
    assert!(matches!(
        list.previous_entry(&entries[4]),
        Err(EntryListError::Removed)
    ));
    let mut other = EntryList::new();
    let foreign = other.push(NonClone(50));
    let mut cursor = list.cursor(0).unwrap();
    assert!(matches!(
        list.insert_after(&foreign, NonClone(51)),
        Err(EntryListError::ForeignEntry)
    ));
    assert!(matches!(
        list.next_entry(&foreign),
        Err(EntryListError::ForeignEntry)
    ));
    assert!(matches!(
        list.previous_entry(&foreign),
        Err(EntryListError::ForeignEntry)
    ));
    assert!(matches!(
        list.insert_after(&entries[4], NonClone(52)),
        Err(EntryListError::Removed)
    ));
    assert_eq!(cursor.next(&list).unwrap().unwrap().0, 99);
}

#[test]
fn clearing_snapshots_identities_and_preserves_callback_insertions() {
    let mut list = EntryList::new();
    let entries: Vec<_> = (0..5).map(|n| list.push(n)).collect();
    let mut seen = vec![];
    list.clear_with(|list, e| {
        seen.push(*e.value());
        match *e.value() {
            0 => {
                list.remove(&entries[3]).unwrap();
                list.push(10);
            }
            1 => {
                list.clear();
                list.push(11);
            }
            2 => {
                assert!(e.is_removed());
                list.push_front(12);
            }
            _ => {}
        }
    });
    assert_eq!(seen, [0, 1, 2, 3, 4]);
    assert_eq!(
        list.entries().map(|e| *e.value()).collect::<Vec<_>>(),
        [12, 11]
    );
    assert!(entries.iter().all(|e| e.is_removed()));
    let mut seen = vec![];
    list.clear_with(|_, e| seen.push(*e.value()));
    assert_eq!(seen, [12, 11]);
    assert!(list.is_empty());
    list.clear_with(|_, _| panic!("empty list must not invoke callback"));
}

#[test]
fn callback_panic_retains_valid_partial_state_and_can_resume() {
    let mut list = EntryList::new();
    let entries: Vec<_> = (0..4).map(|n| list.push(n)).collect();
    assert!(
        catch_unwind(AssertUnwindSafe(|| list.clear_with(|list, e| {
            if *e.value() == 1 {
                list.push(9);
                panic!("callback");
            }
        })))
        .is_err()
    );
    assert!(entries[0].is_removed());
    assert!(!entries[1].is_removed());
    assert_eq!(
        list.entries().map(|e| *e.value()).collect::<Vec<_>>(),
        [1, 2, 3, 9]
    );
    let mut seen = vec![];
    list.clear_with(|_, e| seen.push(*e.value()));
    assert_eq!(seen, [1, 2, 3, 9]);
    assert!(list.is_empty());
}

#[test]
fn clear_invalidates_handles_and_cursors_but_retains_saved_payloads() {
    let mut list = EntryList::new();
    let entry = list.push(String::from("saved"));
    let mut cursor = list.cursor(0).unwrap();
    list.clear();
    assert!(entry.is_removed());
    assert_eq!(&*entry.value(), "saved");
    assert!(matches!(
        list.next_entry(&entry),
        Err(EntryListError::Removed)
    ));
    assert_eq!(cursor.next(&list), Err(EntryListError::CursorInvalidated));
    assert_eq!(list.slot_count(), 0);
    let mut empty_cursor = list.cursor(0).unwrap();
    list.clear();
    assert_eq!(empty_cursor.next(&list).unwrap(), None);
    let fresh = list.push(String::from("fresh"));
    assert!(!fresh.same_entry(&entry));
    assert_eq!(list.remove(&entry), Err(EntryListError::Removed));
    assert_eq!(list.len(), 1);
}

#[test]
fn replacing_list_in_callback_does_not_remove_entries_from_the_new_owner() {
    let mut list = EntryList::new();
    let original: Vec<_> = (0..3).map(|n| list.push(n)).collect();
    let mut transferred = None;
    let mut seen = vec![];
    list.clear_with(|list, entry| {
        seen.push(*entry.value());
        if *entry.value() == 0 {
            transferred = Some(std::mem::take(list));
            list.push(9);
        }
    });
    assert_eq!(seen, [0, 1, 2]);
    assert_eq!(list.entries().map(|e| *e.value()).collect::<Vec<_>>(), [9]);
    let transferred = transferred.unwrap();
    assert_eq!(
        transferred
            .entries()
            .map(|e| *e.value())
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    for entry in &original {
        assert!(!entry.is_removed());
        assert!(matches!(
            list.next_entry(entry),
            Err(EntryListError::ForeignEntry)
        ));
    }
}
