// Original native link lifetime controls; no upstream method credit.
use std::{cell::Cell, rc::Rc};
use timetuck::*;

#[test]
fn live_links_follow_compaction_and_removed_handles_stay_empty() {
    let mut list = EntryList::new();
    let first = list.push(0);
    let middle = list.push(1);
    let last = list.push(2);
    let [previous, next] = list.link_slots(&middle).unwrap();
    assert!(previous.unwrap().same_entry(&first));
    assert!(next.unwrap().same_entry(&last));
    list.remove(&middle).unwrap();
    assert!(
        list.link_slots(&middle)
            .unwrap()
            .iter()
            .all(Option::is_none)
    );
    assert!(matches!(
        list.next_entry(&middle),
        Err(EntryListError::Removed)
    ));
    assert!(matches!(
        list.previous_entry(&middle),
        Err(EntryListError::Removed)
    ));
    let inserted = list.insert(1, 3).unwrap();
    assert!(
        list.link_slots(&first).unwrap()[1]
            .as_ref()
            .unwrap()
            .same_entry(&inserted)
    );
    assert!(
        list.link_slots(&last).unwrap()[0]
            .as_ref()
            .unwrap()
            .same_entry(&inserted)
    );
    let foreign = EntryList::<i32>::new();
    assert!(matches!(
        foreign.link_slots(&middle),
        Err(EntryListError::ForeignEntry)
    ));
    list.clear();
    for entry in [&first, &inserted, &last] {
        assert!(list.link_slots(entry).unwrap().iter().all(Option::is_none));
    }
}

#[test]
fn retained_removed_handle_does_not_retain_neighbors_or_owner() {
    struct Payload(Rc<Cell<usize>>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let first_drops = Rc::new(Cell::new(0));
    let next_drops = Rc::new(Cell::new(0));
    let mut list = EntryList::new();
    let first = list.push(Payload(first_drops.clone()));
    list.push(Payload(next_drops.clone()));
    list.remove(&first).unwrap();
    assert!(list.link_slots(&first).unwrap().iter().all(Option::is_none));
    drop(list);
    assert_eq!(next_drops.get(), 1);
    assert_eq!(first_drops.get(), 0);
    assert!(first.is_removed());
    drop(first);
    assert_eq!(first_drops.get(), 1);
}
