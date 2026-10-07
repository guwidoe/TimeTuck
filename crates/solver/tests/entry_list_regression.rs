use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::{EntryList, EntryListError};

#[test]
fn foreign_and_removed_handles_never_remove_a_replacement() {
    let mut a = EntryList::new();
    let mut b = EntryList::new();
    let x = a.push(1);
    let y = b.push(2);
    assert_eq!(a.remove(&y), Err(EntryListError::ForeignEntry));
    assert_eq!(a.remove(&x), Ok(Rc::new(1)));
    let z = a.push(3);
    assert_eq!(a.remove(&x), Err(EntryListError::Removed));
    assert_eq!(*z.value(), 3);
    assert_eq!(z.position(), Some(0));
    assert!(!x.same_entry(&z));
    assert!(z.same_entry(&z.clone()));
    drop(a);
    assert!(z.is_removed());
    assert_eq!(*z.value(), 3);
    assert!(!y.is_removed());
}

#[test]
fn cursor_identity_revision_and_failed_edits_are_checked() {
    let mut a = EntryList::new();
    let mut b = EntryList::new();
    for i in 0..20 {
        a.push(i);
        b.push(i);
    }
    let mut c = a.cursor(0).unwrap();
    assert_eq!(c.next(&b), Err(EntryListError::ForeignCursor));
    assert!(matches!(
        c.add(&mut b, 1),
        Err(EntryListError::ForeignCursor)
    ));
    assert!(a.cursor(21).is_err());
    assert!(a.entry_at(20).is_err());
    assert!(a.entry_at(usize::MAX).is_err());
    assert!(a.get(usize::MAX).is_none());
    assert!(a.insert(21, 3).is_err());
    assert_eq!(*c.next(&a).unwrap().unwrap(), 0);
    let mut second = a.cursor(0).unwrap();
    c.set(&mut a, 30).unwrap();
    assert_eq!(*second.next(&a).unwrap().unwrap(), 30);
    c.add(&mut a, 31).unwrap();
    assert_eq!(second.next(&a), Err(EntryListError::CursorInvalidated));
    assert_eq!(c.remove(&mut a), Err(EntryListError::NoCurrentEntry));
    assert_eq!(c.set(&mut a, 99), Err(EntryListError::NoCurrentEntry));
    assert_eq!(*c.previous(&a).unwrap().unwrap(), 31);
    c.remove(&mut a).unwrap();
    assert_eq!(c.remove(&mut a), Err(EntryListError::NoCurrentEntry));
    let e = a.entry_at(4).unwrap().unwrap();
    a.remove(&e).unwrap();
    assert_eq!(c.previous(&a), Err(EntryListError::CursorInvalidated));
    let mut c = a.cursor(0).unwrap();
    a.compact();
    assert_eq!(c.next(&a), Err(EntryListError::CursorInvalidated));
    let mut c = a.cursor(0).unwrap();
    a.compact();
    assert_eq!(*c.next(&a).unwrap().unwrap(), 30);
}

#[test]
fn handles_observe_replacements_but_returned_values_are_snapshots_without_clone() {
    struct Value {
        id: u32,
        drops: Rc<Cell<u32>>,
    }
    impl Drop for Value {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    let drops = Rc::new(Cell::new(0));
    let mut l = EntryList::new();
    let e = l.push(Value {
        id: 1,
        drops: drops.clone(),
    });
    let mut c = l.cursor(0).unwrap();
    let snapshot = c.next(&l).unwrap().unwrap();
    let old = c
        .set(
            &mut l,
            Value {
                id: 2,
                drops: drops.clone(),
            },
        )
        .unwrap();
    assert!(Rc::ptr_eq(&old, &snapshot));
    assert_eq!(snapshot.id, 1);
    assert_eq!(e.value().id, 2);
    drop(old);
    assert_eq!(drops.get(), 0);
    drop(snapshot);
    assert_eq!(drops.get(), 1);
    c.remove(&mut l).unwrap();
    assert!(e.is_removed());
    assert_eq!(drops.get(), 1);
    drop(l);
    assert_eq!(drops.get(), 1);
    drop(c);
    drop(e);
    assert_eq!(drops.get(), 2);
}

#[test]
fn panicking_visitor_leaves_handles_and_compaction_consistent() {
    let mut l = EntryList::new();
    let e: Vec<_> = (0..20).map(|i| l.push(i)).collect();
    l.remove(&e[3]).unwrap();
    assert_eq!(l.slot_count(), 20);
    let outcome = catch_unwind(AssertUnwindSafe(|| l.for_each(|_| panic!("visitor"))));
    assert!(outcome.is_err());
    assert_eq!(l.len(), 19);
    assert_eq!(l.slot_count(), 19);
    for i in 0..19 {
        let h = l.entry_at(i).unwrap().unwrap();
        assert_eq!(h.position(), Some(i));
    }
    l.remove(&e[19]).unwrap();
    assert_eq!(l.len(), 18);
}

#[test]
fn cursor_walks_real_holes_in_both_directions_and_removes_after_exhaustion() {
    let mut l = EntryList::new();
    let e: Vec<_> = (0..100).map(|i| l.push(i)).collect();
    for i in [0, 7, 11, 43, 90] {
        l.remove(&e[i]).unwrap();
    }
    assert_eq!(l.slot_count(), 100);
    let reference: Vec<_> = (0..100)
        .filter(|i| ![0, 7, 11, 43, 90].contains(i))
        .collect();
    let mut c = l.cursor(0).unwrap();
    for v in &reference {
        assert_eq!(*c.next(&l).unwrap().unwrap(), *v);
    }
    assert!(c.next(&l).unwrap().is_none());
    assert!(c.next(&l).unwrap().is_none());
    for v in reference.iter().rev() {
        assert_eq!(*c.previous(&l).unwrap().unwrap(), *v);
    }
    assert!(c.previous(&l).unwrap().is_none());
    assert_eq!(*c.remove(&mut l).unwrap(), 1);
    assert_eq!(*c.next(&l).unwrap().unwrap(), 2);
}

#[test]
fn seeded_cursor_edits_match_a_vector_and_keep_every_handle_addressable() {
    let mut seed = 0x257d87a1_u64;
    let mut random = |bound: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % bound as u64) as usize
    };
    let mut l = EntryList::new();
    let mut reference = vec![];
    let mut handles = vec![];
    let mut cursor = l.cursor(0).unwrap();
    let mut rank = 0;
    let mut current = None;
    for step in 0..30_000 {
        match random(8) {
            0 => {
                let actual = cursor.next(&l).unwrap();
                if rank < reference.len() {
                    assert_eq!(*actual.unwrap(), reference[rank]);
                    current = Some(rank);
                    rank += 1;
                } else {
                    assert!(actual.is_none());
                }
            }
            1 => {
                let actual = cursor.previous(&l).unwrap();
                if rank > 0 {
                    rank -= 1;
                    current = Some(rank);
                    assert_eq!(*actual.unwrap(), reference[rank]);
                } else {
                    assert!(actual.is_none());
                }
            }
            2 => {
                handles.push(cursor.add(&mut l, step).unwrap());
                reference.insert(rank, step);
                rank += 1;
                current = None;
            }
            3 => {
                let actual = cursor.remove(&mut l);
                if let Some(i) = current.take() {
                    assert_eq!(*actual.unwrap(), reference.remove(i));
                    if i < rank {
                        rank -= 1;
                    }
                } else {
                    assert_eq!(actual, Err(EntryListError::NoCurrentEntry));
                }
            }
            4 => {
                let actual = cursor.set(&mut l, step);
                if let Some(i) = current {
                    assert_eq!(*actual.unwrap(), reference[i]);
                    reference[i] = step;
                } else {
                    assert_eq!(actual, Err(EntryListError::NoCurrentEntry));
                }
            }
            5 => {
                rank = random(reference.len() + 1);
                cursor = l.cursor(rank).unwrap();
                current = None;
            }
            6 => {
                l.compact();
                rank = random(reference.len() + 1);
                cursor = l.cursor(rank).unwrap();
                current = None;
            }
            _ => {
                if !reference.is_empty() {
                    let i = random(reference.len());
                    assert_eq!(*l.get(i).unwrap(), reference[i]);
                }
            }
        }
        assert_eq!(l.len(), reference.len());
        assert_eq!(cursor.has_next(&l).unwrap(), rank < reference.len());
        assert_eq!(cursor.has_previous(&l).unwrap(), rank > 0);
        assert!(l.slot_count() * 4 <= l.len() * 5);
        if step % 127 == 0 {
            assert_eq!(
                l.snapshot().iter().map(|v| **v).collect::<Vec<_>>(),
                reference
            );
            for h in &handles {
                if let Some(p) = h.position() {
                    assert!(l.entry_at(p).unwrap().unwrap().same_entry(h));
                }
            }
        }
    }
    assert_eq!(
        l.snapshot().iter().map(|v| **v).collect::<Vec<_>>(),
        reference
    );
}
