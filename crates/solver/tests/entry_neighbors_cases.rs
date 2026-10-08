// Adapted from Timefold Community TupleListTest and ElementAwareLinkedListTest,
// Apache-2.0. See compat/provenance.json and THIRD_PARTY_NOTICES.md.
// Stable native handles replace Java tuple identities and intrusive link fields.
use timetuck::{EntryList, ListEntry};

fn same<T>(actual: Option<ListEntry<T>>, expected: Option<&ListEntry<T>>) {
    match (actual, expected) {
        (None, None) => {}
        (Some(a), Some(b)) => assert!(a.same_entry(b)),
        _ => panic!("entry presence differs"),
    }
}
fn order<T>(list: &EntryList<T>, entries: &[&ListEntry<T>]) {
    assert_eq!(list.len(), entries.len());
    same(list.first_entry(), entries.first().copied());
    same(list.last_entry(), entries.last().copied());
    for (i, entry) in entries.iter().enumerate() {
        same(
            list.previous_entry(entry).unwrap(),
            i.checked_sub(1).map(|j| entries[j]),
        );
        same(list.next_entry(entry).unwrap(), entries.get(i + 1).copied());
    }
}
#[test]
fn add_single() {
    let mut l = EntryList::new();
    let a = l.push(());
    order(&l, &[&a]);
}
#[test]
fn add_two_first_and_next_correct() {
    let mut l = EntryList::new();
    let a = l.push(());
    let b = l.push(());
    order(&l, &[&a, &b]);
}
#[test]
fn remove_first() {
    let mut l = EntryList::new();
    let a = l.push(());
    let b = l.push(());
    l.remove(&a).unwrap();
    order(&l, &[&b]);
}
#[test]
fn remove_last() {
    let mut l = EntryList::new();
    let a = l.push(());
    let b = l.push(());
    l.remove(&b).unwrap();
    order(&l, &[&a]);
}
#[test]
fn remove_middle() {
    let mut l = EntryList::new();
    let a = l.push(());
    let b = l.push(());
    let c = l.push(());
    l.remove(&b).unwrap();
    order(&l, &[&a, &c]);
}
#[test]
fn remove_all_empty_after() {
    let mut l = EntryList::new();
    let a = l.push(());
    l.remove(&a).unwrap();
    order(&l, &[]);
}
#[test]
fn clear_calls_consumer_in_order_then_empty() {
    let mut l = EntryList::new();
    let a = l.push(());
    let b = l.push(());
    let mut seen = vec![];
    l.clear_with(|_, e| seen.push(e));
    assert_eq!(seen.len(), 2);
    assert!(seen[0].same_entry(&a));
    assert!(seen[1].same_entry(&b));
    order(&l, &[]);
}
#[test]
fn clear_safe_when_consumer_removes_from_same_list() {
    let mut l = EntryList::new();
    let a = l.push(());
    let b = l.push(());
    let c = l.push(());
    let mut seen = vec![];
    l.clear_with(|list, e| {
        if e.same_entry(&b) {
            list.remove(&e).unwrap();
        }
        seen.push(e);
    });
    assert_eq!(seen.len(), 3);
    for (actual, expected) in seen.iter().zip([&a, &b, &c]) {
        assert!(actual.same_entry(expected));
    }
    order(&l, &[]);
}
#[test]
fn for_each_visit_all_in_order() {
    let mut l = EntryList::new();
    let entries: Vec<_> = (0..3).map(|_| l.push(())).collect();
    let seen: Vec<_> = l.entries().collect();
    assert_eq!(seen.len(), 3);
    for (actual, expected) in seen.iter().zip(&entries) {
        assert!(actual.same_entry(expected));
    }
}
#[test]
fn empty_list_size_zero_first_null() {
    order(&EntryList::<()>::new(), &[]);
}
#[test]
fn add_remove() {
    let mut l = EntryList::new();
    order(&l, &[]);
    let a = l.push("A");
    assert_eq!(*a.value(), "A");
    order(&l, &[&a]);
    let b = l.push("B");
    assert_eq!(*b.value(), "B");
    order(&l, &[&a, &b]);
    l.remove(&a).unwrap();
    order(&l, &[&b]);
    l.remove(&b).unwrap();
    order(&l, &[]);
}
#[test]
fn add_first() {
    let mut l = EntryList::new();
    order(&l, &[]);
    let a = l.push("A");
    let b = l.push("B");
    let c = l.push_front("C");
    order(&l, &[&c, &a, &b]);
}
#[test]
fn add_after() {
    let mut l = EntryList::new();
    order(&l, &[]);
    let a = l.push("A");
    let b = l.push("B");
    let c = l.insert_after(&a, "C").unwrap();
    order(&l, &[&a, &c, &b]);
    let d = l.insert_after(&b, "D").unwrap();
    order(&l, &[&a, &c, &b, &d]);
}
#[test]
fn iterator() {
    let mut l = EntryList::<&str>::new();
    assert!(l.is_empty());
    {
        let mut it = l.entries().peekable();
        assert!(it.peek().is_none());
        assert!(it.next().is_none());
    }
    for value in ["A", "B", "C"] {
        l.push(value);
    }
    let mut it = l.entries().peekable();
    for expected in ["A", "B", "C"] {
        assert!(it.peek().is_some());
        assert_eq!(*it.next().unwrap().value(), expected);
    }
    assert!(it.peek().is_none());
}
#[test]
fn clear() {
    let mut l = EntryList::new();
    l.push("A");
    l.push("B");
    assert_eq!(l.len(), 2);
    l.clear();
    assert!(l.is_empty());
}
