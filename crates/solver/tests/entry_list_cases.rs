// Adapted from pinned Community ElementAwareArrayListTest. Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::{EntryList, EntryListError, ListEntry};
fn sample<T: Clone>(v: &[T]) -> (EntryList<T>, Vec<ListEntry<T>>) {
    let mut l = EntryList::new();
    let e = v.iter().map(|x| l.push(x.clone())).collect();
    (l, e)
}
fn values<T: Clone>(l: &EntryList<T>) -> Vec<T> {
    l.snapshot().iter().map(|v| (**v).clone()).collect()
}
fn copy<T: Clone>(l: &mut EntryList<T>) -> Vec<T> {
    let mut v = vec![];
    l.for_each(|x| v.push(x.clone()));
    v
}
fn fill(l: &mut EntryList<String>, n: usize) -> Vec<ListEntry<String>> {
    (0..n).map(|i| l.push(format!("e{i}"))).collect()
}
// Explicit native SplitMix64 streams replace Java Random traces.
fn random(state: &mut u64, bound: usize) -> usize {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    ((z ^ (z >> 31)) % bound as u64) as usize
}
fn seeds() -> [u64; 10] {
    let mut s = 0xcafebabe;
    std::array::from_fn(|_| random(&mut s, usize::MAX) as u64)
}

#[test]
fn empty_list() {
    let l = EntryList::<String>::new();
    assert!(l.is_empty());
}

#[test]
fn add_single_element() {
    let mut l = EntryList::new();
    let e = l.push("first");
    assert!(!l.is_empty());
    assert_eq!(l.len(), 1);
    assert_eq!(*e.value(), "first");
    assert!(!e.is_removed());
}

#[test]
fn add_multiple_elements() {
    let (l, _) = sample(&["first", "second", "third"]);
    assert_eq!(values(&l), ["first", "second", "third"]);
}

#[test]
fn remove_single_element() {
    let (mut l, e) = sample(&["first", "second", "third"]);
    l.remove(&e[1]).unwrap();
    assert_eq!(l.len(), 2);
    assert!(e[1].is_removed());
    assert!(!e[0].is_removed());
    assert!(!e[2].is_removed());
}

#[test]
fn remove_already_removed_element() {
    let (mut l, e) = sample(&["first"]);
    l.remove(&e[0]).unwrap();
    assert_eq!(l.remove(&e[0]), Err(EntryListError::Removed));
    assert!(
        EntryListError::Removed
            .to_string()
            .contains("was already removed")
    );
}

#[test]
fn remove_all_elements() {
    let (mut l, e) = sample(&["first", "second"]);
    for x in &e {
        l.remove(x).unwrap();
    }
    assert!(l.is_empty());
}

#[test]
fn add_entry_after_churn_appends_in_order() {
    let (mut l, e) = sample(&["a", "b", "c"]);
    l.remove(&e[1]).unwrap();
    l.remove(&e[2]).unwrap();
    let x = l.push("x");
    assert_eq!(values(&l), ["a", "x"]);
    assert_eq!(x.position(), Some(1));
}

#[test]
fn add_after_large_array_free_path_preserves_order() {
    let mut l = EntryList::new();
    let e = fill(&mut l, 30);
    for x in &e {
        l.remove(x).unwrap();
    }
    for x in ["x", "y", "z"] {
        l.push(x.to_owned());
    }
    assert_eq!(values(&l), ["x", "y", "z"]);
}

#[test]
fn for_each_on_empty_list() {
    let mut l = EntryList::<String>::new();
    let mut count = 0;
    l.for_each(|_| count += 1);
    assert_eq!(count, 0);
}

#[test]
fn for_each_without_gaps() {
    let (mut l, _) = sample(&["first", "second", "third"]);
    assert_eq!(copy(&mut l), ["first", "second", "third"]);
}

#[test]
fn for_each_with_gaps() {
    let (mut l, e) = sample(&["first", "second", "third"]);
    l.remove(&e[1]).unwrap();
    assert_eq!(copy(&mut l), ["first", "third"]);
}

#[test]
fn for_each_compacts_with_gaps() {
    let (mut l, e) = sample(&["first", "second", "third", "fourth"]);
    l.remove(&e[1]).unwrap();
    l.remove(&e[2]).unwrap();
    l.for_each(|_| {});
    assert_eq!(l.len(), 2);
    assert_eq!(e[0].position(), Some(0));
    assert_eq!(e[3].position(), Some(1));
}

#[test]
fn for_each_clears_when_all_removed() {
    let (mut l, e) = sample(&["first", "second"]);
    for x in &e {
        l.remove(x).unwrap();
    }
    l.for_each(|_| {});
    assert!(l.is_empty());
}

#[test]
fn for_each_compacts_with_tail_gaps() {
    let (mut l, e) = sample(&["first", "second", "third", "fourth"]);
    l.remove(&e[2]).unwrap();
    l.remove(&e[3]).unwrap();
    assert_eq!(copy(&mut l), ["first", "second"]);
    assert_eq!(l.len(), 2);
    assert_eq!(e[0].position(), Some(0));
    assert_eq!(e[1].position(), Some(1));
}

#[test]
fn get_gaps_before_target_does_not_relocate() {
    let mut l = EntryList::new();
    let e = fill(&mut l, 20);
    l.remove(&e[1]).unwrap();
    l.remove(&e[3]).unwrap();
    assert_eq!(l.slot_count(), 20);
    assert_eq!(&*l.get(1).unwrap(), "e2");
    assert_eq!(&*l.get(2).unwrap(), "e4");
    assert_eq!(l.len(), 18);
    for i in [2, 4, 19] {
        assert_eq!(e[i].position(), Some(i));
    }
    assert_eq!(l.slot_count(), 20);
    l.remove(&e[19]).unwrap();
    assert_eq!(l.len(), 17);
    assert!(!copy(&mut l).contains(&"e19".to_owned()));
}

#[test]
fn get_gaps_before_target_trailing_cleanup() {
    let (mut l, e) = sample(&["a", "b", "c"]);
    l.remove(&e[1]).unwrap();
    l.get(1);
    assert_eq!(e[2].position(), Some(1));
    assert_eq!(l.len(), 2);
    assert_eq!(*l.get(0).unwrap(), "a");
    assert_eq!(*l.get(1).unwrap(), "c");
}

#[test]
fn get_gaps_only_after_target() {
    let (mut l, e) = sample(&["a", "b", "c"]);
    l.remove(&e[2]).unwrap();
    l.get(0);
    assert_eq!(e[0].position(), Some(0));
    assert_eq!(e[1].position(), Some(1));
    assert_eq!(l.len(), 2);
}

#[test]
fn get_last_element_trailing_gaps_with_no_prefix() {
    let (mut l, e) = sample(&["a", "b", "c", "d"]);
    l.remove(&e[2]).unwrap();
    l.remove(&e[3]).unwrap();
    l.get(1);
    assert_eq!(e[0].position(), Some(0));
    assert_eq!(e[1].position(), Some(1));
    assert_eq!(l.len(), 2);
    assert_eq!(*l.get(0).unwrap(), "a");
    assert_eq!(*l.get(1).unwrap(), "b");
}

#[test]
fn get_last_element_trailing_gaps_with_prefix() {
    let (mut l, e) = sample(&["a", "b", "c", "d"]);
    for i in [0, 2, 3] {
        l.remove(&e[i]).unwrap();
    }
    l.get(0);
    assert_eq!(e[1].position(), Some(0));
    assert_eq!(l.len(), 1);
    assert_eq!(*l.get(0).unwrap(), "b");
}

#[test]
fn get_gaps_mixed_around_target() {
    let mut l = EntryList::new();
    let e = fill(&mut l, 20);
    for i in [1, 15] {
        l.remove(&e[i]).unwrap();
    }
    assert_eq!(l.slot_count(), 20);
    assert_eq!(&*l.get(5).unwrap(), "e6");
    for i in [6, 19] {
        assert_eq!(e[i].position(), Some(i));
    }
    assert_eq!(l.len(), 18);
    let v = copy(&mut l);
    assert!(!v.contains(&"e1".to_owned()));
    assert!(!v.contains(&"e15".to_owned()));
}

#[test]
fn remove_entry_returned_by_partial_compact() {
    let (mut l, e) = sample(&["a", "b", "c", "d"]);
    l.remove(&e[1]).unwrap();
    l.get(1);
    assert_eq!(e[2].position(), Some(1));
    l.remove(&e[2]).unwrap();
    assert!(e[2].is_removed());
    assert_eq!(l.len(), 2);
    assert_eq!(copy(&mut l), ["a", "d"]);
}

#[test]
fn remove_suffix_entry_position_unchanged() {
    let (mut l, e) = sample(&["a", "b", "c", "d", "e"]);
    for i in [1, 3] {
        l.remove(&e[i]).unwrap();
    }
    l.get(1);
    l.remove(&e[4]).unwrap();
    assert!(e[4].is_removed());
    assert_eq!(l.len(), 2);
    assert_eq!(copy(&mut l), ["a", "c"]);
}

#[test]
fn sequential_get_gaps_progress_migrate() {
    let (mut l, e) = sample(&["a", "b", "c", "d", "e"]);
    for i in [1, 3] {
        l.remove(&e[i]).unwrap();
    }
    l.get(1);
    assert_eq!(e[2].position(), Some(1));
    assert_eq!(*l.get(1).unwrap(), "c");
    assert_eq!(*l.get(2).unwrap(), "e");
    assert_eq!(e[4].position(), Some(2));
    assert_eq!(l.len(), 3);
    assert_eq!(copy(&mut l), ["a", "c", "e"]);
}

#[test]
fn get_then_for_each_full_compact_correct() {
    let (mut l, e) = sample(&["a", "b", "c", "d", "e"]);
    for i in [1, 3] {
        l.remove(&e[i]).unwrap();
    }
    l.get(1);
    assert_eq!(copy(&mut l), ["a", "c", "e"]);
    for (p, i) in [0, 2, 4].into_iter().enumerate() {
        assert_eq!(e[i].position(), Some(p));
    }
    assert_eq!(l.len(), 3);
}

#[test]
fn get_then_add_at_full_compact_correct() {
    let (mut l, e) = sample(&["a", "b", "c", "d"]);
    l.remove(&e[1]).unwrap();
    l.get(1);
    l.insert(1, "x").unwrap();
    assert_eq!(l.len(), 4);
    assert_eq!(copy(&mut l), ["a", "x", "c", "d"]);
    for i in [0, 2, 3] {
        assert_eq!(e[i].position(), Some(i));
    }
    assert_eq!(*l.get(0).unwrap(), "a");
    assert_eq!(*l.get(2).unwrap(), "c");
    assert_eq!(*l.get(3).unwrap(), "d");
}

#[test]
fn get_out_of_bounds_throws_i_o_o_b() {
    let (mut l, e) = sample(&["a", "b", "c"]);
    l.remove(&e[1]).unwrap();
    assert!(l.get(2).is_none());
}

#[test]
fn entry_get_element() {
    let mut l = EntryList::new();
    let e = l.push("test");
    assert_eq!(*e.value(), "test");
}

#[test]
fn entry_is_removed_false() {
    let mut l = EntryList::new();
    let e = l.push("test");
    assert!(!e.is_removed());
}

#[test]
fn entry_is_removed_true() {
    let mut l = EntryList::new();
    let e = l.push("test");
    l.remove(&e).unwrap();
    assert!(e.is_removed());
}

#[test]
fn entry_to_string_active() {
    let mut l = EntryList::new();
    let e = l.push("test");
    assert_eq!(e.to_string(), "test@0");
}

#[test]
fn entry_to_string_removed() {
    let mut l = EntryList::new();
    let e = l.push("test");
    l.remove(&e).unwrap();
    assert_eq!(e.to_string(), "null");
}

#[test]
fn add_and_get() {
    let mut l = EntryList::<Option<String>>::new();
    l.push(None);
    assert_eq!(l.len(), 1);
    assert_eq!(*l.get(0).unwrap(), None);
}

#[test]
fn for_each() {
    let (mut l, _) = sample(&[Some("a"), None, Some("b")]);
    assert_eq!(copy(&mut l), [Some("a"), None, Some("b")]);
}

#[test]
fn for_each_with_gaps_2() {
    let (mut l, e) = sample(&[Some("a"), Some("x"), None, Some("b")]);
    l.remove(&e[1]).unwrap();
    assert_eq!(copy(&mut l), [Some("a"), None, Some("b")]);
}

#[test]
fn remove_null_entry() {
    let (mut l, e) = sample(&[Some("a"), None, Some("b")]);
    l.remove(&e[1]).unwrap();
    assert_eq!(copy(&mut l), [Some("a"), Some("b")]);
}

#[test]
fn iterator() {
    let (mut l, _) = sample(&[None, Some("a"), None]);
    let mut c = l.cursor(0).unwrap();
    for v in [None, Some("a"), None] {
        assert_eq!(*c.next(&l).unwrap().unwrap(), v);
    }
    assert!(!c.has_next(&l).unwrap());
    for v in [None, Some("a"), None] {
        assert_eq!(*c.previous(&l).unwrap().unwrap(), v);
    }
    assert!(!c.has_previous(&l).unwrap());
    let mut c = l.cursor(0).unwrap();
    assert_eq!(*c.next(&l).unwrap().unwrap(), None);
    c.remove(&mut l).unwrap();
    assert_eq!(*c.next(&l).unwrap().unwrap(), Some("a"));
    assert_eq!(*c.next(&l).unwrap().unwrap(), None);
    assert!(!c.has_next(&l).unwrap());
    assert_eq!(values(&l), [Some("a"), None]);
}

#[test]
fn entry_element() {
    let mut l = EntryList::<Option<String>>::new();
    let e = l.push(None);
    assert_eq!(*e.value(), None);
}

#[test]
fn multiple_operations() {
    let (mut l, e) = sample(&[1, 2, 3, 4, 5]);
    for i in [1, 3] {
        l.remove(&e[i]).unwrap();
    }
    assert_eq!(l.len(), 3);
    assert_eq!(values(&l), [1, 3, 5]);
}

#[test]
fn remove_from_various_positions() {
    let (mut l, e) = sample(&["a", "b", "c", "d", "e"]);
    for i in [0, 2, 4] {
        l.remove(&e[i]).unwrap();
    }
    assert_eq!(values(&l), ["b", "d"]);
    assert_eq!(l.len(), 2);
}

#[test]
fn interleaved_operations() {
    let (mut l, e) = sample(&[1, 2]);
    l.remove(&e[0]).unwrap();
    l.push(3);
    l.remove(&e[1]).unwrap();
    l.push(4);
    assert_eq!(l.len(), 2);
    assert_eq!(values(&l), [3, 4]);
}

#[test]
fn large_list_with_many_gaps() {
    let (mut l, e) = sample(&(0..100).collect::<Vec<_>>());
    for i in (0..100).step_by(2) {
        l.remove(&e[i]).unwrap();
    }
    assert_eq!(l.len(), 50);
    let v = copy(&mut l);
    assert_eq!(v.len(), 50);
    for (i, x) in v.iter().enumerate() {
        assert_eq!(*x, 2 * i + 1);
    }
}

#[test]
fn remove_all_then_add_new() {
    let (mut l, e) = sample(&["old1", "old2"]);
    for x in &e {
        l.remove(x).unwrap();
    }
    assert!(l.is_empty());
    l.push("new1");
    l.push("new2");
    assert_eq!(l.len(), 2);
    assert_eq!(values(&l), ["new1", "new2"]);
}

#[test]
fn multiple_for_each_calls() {
    let (mut l, e) = sample(&["a", "b", "c"]);
    l.remove(&e[1]).unwrap();
    assert_eq!(copy(&mut l), ["a", "c"]);
    assert_eq!(copy(&mut l), ["a", "c"]);
    assert_eq!(l.len(), 2);
}

#[test]
fn position_tracking_after_compaction() {
    let (mut l, e) = sample(&["a", "b", "c", "d"]);
    l.remove(&e[1]).unwrap();
    l.for_each(|_| {});
    for (p, i) in [0, 2, 3].into_iter().enumerate() {
        assert_eq!(e[i].position(), Some(p));
    }
}

#[test]
fn add_at_end() {
    let (mut l, _) = sample(&["a", "b"]);
    l.insert(2, "c").unwrap();
    assert_eq!(l.len(), 3);
    assert_eq!(*l.get(2).unwrap(), "c");
    assert_eq!(copy(&mut l), ["a", "b", "c"]);
}

#[test]
fn add_at_beginning() {
    let (mut l, e) = sample(&["b", "c"]);
    l.insert(0, "a").unwrap();
    assert_eq!(l.len(), 3);
    assert_eq!(copy(&mut l), ["a", "b", "c"]);
    for x in e {
        assert!(!x.is_removed());
    }
}

#[test]
fn add_at_middle() {
    let (mut l, _) = sample(&["a", "c"]);
    l.insert(1, "b").unwrap();
    assert_eq!(l.len(), 3);
    assert_eq!(copy(&mut l), ["a", "b", "c"]);
}

#[test]
fn add_at_compacts_first() {
    let (mut l, e) = sample(&["a", "b", "c"]);
    l.remove(&e[0]).unwrap();
    l.insert(1, "x").unwrap();
    assert_eq!(l.len(), 3);
    assert_eq!(copy(&mut l), ["b", "x", "c"]);
    assert!(!e[2].is_removed());
}

#[test]
fn add_at_fills_gap() {
    let (mut l, e) = sample(&["a", "b", "c", "d"]);
    l.remove(&e[1]).unwrap();
    l.insert(1, "x").unwrap();
    assert_eq!(l.len(), 4);
    for i in [0, 2, 3] {
        assert_eq!(e[i].position(), Some(i));
    }
    assert_eq!(copy(&mut l), ["a", "x", "c", "d"]);
}

#[test]
fn add_at_compacts_gapped_list() {
    let (mut l, e) = sample(&["a", "b", "c", "d"]);
    for i in [0, 2] {
        l.remove(&e[i]).unwrap();
    }
    l.insert(1, "x").unwrap();
    assert_eq!(l.len(), 3);
    assert_eq!(e[1].position(), Some(0));
    assert_eq!(e[3].position(), Some(2));
    assert_eq!(l.slot_count(), 3);
    assert_eq!(copy(&mut l), ["b", "x", "d"]);
}

#[test]
fn add_at_rotates_into_gap() {
    let (mut l, e) = sample(&["a", "b", "c", "d", "e"]);
    l.remove(&e[3]).unwrap();
    l.insert(1, "x").unwrap();
    assert_eq!(l.len(), 5);
    for (i, p) in [(0, 0), (1, 2), (2, 3), (4, 4)] {
        assert_eq!(e[i].position(), Some(p));
    }
    assert_eq!(copy(&mut l), ["a", "x", "b", "c", "e"]);
}

#[test]
fn add_at_beyond_size() {
    let (mut l, _) = sample(&["a"]);
    assert!(matches!(
        l.insert(2, "x"),
        Err(EntryListError::IndexOutOfBounds { index: 2, len: 1 })
    ));
}

#[test]
fn add_at_beyond_logical_size_on_gappy_list() {
    let (mut l, e) = sample(&["a", "b", "c"]);
    l.remove(&e[1]).unwrap();
    let mut c = l.cursor(0).unwrap();
    assert_eq!(*c.next(&l).unwrap().unwrap(), "a");
    assert!(l.insert(3, "x").is_err());
    assert_eq!(*c.next(&l).unwrap().unwrap(), "c");
}

#[test]
fn add_at_empty_list() {
    let mut l = EntryList::new();
    l.insert(0, "a").unwrap();
    assert_eq!(l.len(), 1);
    assert_eq!(*l.get(0).unwrap(), "a");
}

#[test]
fn existing_entries_removable_after_add_at() {
    let (mut l, e) = sample(&["a", "c"]);
    l.insert(1, "b").unwrap();
    l.remove(&e[0]).unwrap();
    assert_eq!(l.len(), 2);
    assert_eq!(copy(&mut l), ["b", "c"]);
}

#[test]
fn add_at_consumes_all_gaps() {
    let (mut l, e) = sample(&["a", "b", "c", "d", "e"]);
    for i in [2, 3] {
        l.remove(&e[i]).unwrap();
    }
    l.insert(1, "x").unwrap();
    assert_eq!(l.len(), 4);
    for (i, p) in [(0, 0), (1, 2), (4, 3)] {
        assert_eq!(e[i].position(), Some(p));
    }
    assert_eq!(l.slot_count(), 4);
    assert_eq!(copy(&mut l), ["a", "x", "b", "e"]);
}

#[test]
fn forward_iteration() {
    let (l, _) = sample(&["a", "b", "c"]);
    let mut c = l.cursor(0).unwrap();
    assert!(c.has_next(&l).unwrap());
    for x in ["a", "b", "c"] {
        assert_eq!(*c.next(&l).unwrap().unwrap(), x);
    }
    assert!(!c.has_next(&l).unwrap());
}

#[test]
fn backward_iteration() {
    let (l, _) = sample(&["a", "b", "c"]);
    let mut c = l.cursor(3).unwrap();
    assert!(c.has_previous(&l).unwrap());
    for x in ["c", "b", "a"] {
        assert_eq!(*c.previous(&l).unwrap().unwrap(), x);
    }
    assert!(!c.has_previous(&l).unwrap());
}

#[test]
fn remove_fwd() {
    let (mut l, _) = sample(&["a", "b", "c"]);
    let mut c = l.cursor(0).unwrap();
    assert_eq!(*c.next(&l).unwrap().unwrap(), "a");
    c.remove(&mut l).unwrap();
    assert!(!c.has_previous(&l).unwrap());
    assert!(c.has_next(&l).unwrap());
    assert_eq!(*c.next(&l).unwrap().unwrap(), "b");
    assert_eq!(values(&l), ["b", "c"]);
}

#[test]
fn remove_bwd() {
    let (mut l, _) = sample(&["a", "b", "c"]);
    let mut c = l.cursor(0).unwrap();
    for x in ["a", "b", "c"] {
        assert_eq!(*c.next(&l).unwrap().unwrap(), x);
    }
    assert_eq!(*c.previous(&l).unwrap().unwrap(), "c");
    c.remove(&mut l).unwrap();
    assert_eq!(values(&l), ["a", "b"]);
    assert!(!c.has_next(&l).unwrap());
    assert!(c.has_previous(&l).unwrap());
    assert_eq!(*c.previous(&l).unwrap().unwrap(), "b");
    assert_eq!(*c.next(&l).unwrap().unwrap(), "b");
}

#[test]
fn set() {
    let (mut l, _) = sample(&["a", "b"]);
    let mut c = l.cursor(0).unwrap();
    let mut other = l.cursor(0).unwrap();
    assert_eq!(*c.next(&l).unwrap().unwrap(), "a");
    c.set(&mut l, "x").unwrap();
    assert_eq!(values(&l), ["x", "b"]);
    assert_eq!(*other.next(&l).unwrap().unwrap(), "x");
}

#[test]
fn add_at_cursor() {
    let (mut l, _) = sample(&["a", "b", "c"]);
    let mut c = l.cursor(0).unwrap();
    assert_eq!(*c.next(&l).unwrap().unwrap(), "a");
    c.add(&mut l, "x").unwrap();
    assert_eq!(*c.next(&l).unwrap().unwrap(), "b");
    assert_eq!(*c.previous(&l).unwrap().unwrap(), "b");
    assert_eq!(*c.previous(&l).unwrap().unwrap(), "x");
    assert_eq!(values(&l), ["a", "x", "b", "c"]);
}

#[test]
fn add_with_gaps() {
    let (mut l, e) = sample(&["a", "b", "c"]);
    l.remove(&e[1]).unwrap();
    l.cursor(1).unwrap().add(&mut l, "x").unwrap();
    assert_eq!(values(&l), ["a", "x", "c"]);
}

#[test]
fn add_at_end_after_mid_iteration_removal() {
    let (mut l, _) = sample(&["a", "b", "c", "d"]);
    let mut c = l.cursor(0).unwrap();
    for x in ["a", "b"] {
        assert_eq!(*c.next(&l).unwrap().unwrap(), x);
    }
    c.remove(&mut l).unwrap();
    for x in ["c", "d"] {
        assert_eq!(*c.next(&l).unwrap().unwrap(), x);
    }
    c.add(&mut l, "e").unwrap();
    assert!(!c.has_next(&l).unwrap());
    for x in ["e", "d", "c"] {
        assert_eq!(*c.previous(&l).unwrap().unwrap(), x);
    }
    assert_eq!(values(&l), ["a", "c", "d", "e"]);
}

#[test]
fn no_such_element() {
    let (l, _) = sample(&["a"]);
    let mut c = l.cursor(0).unwrap();
    assert_eq!(*c.next(&l).unwrap().unwrap(), "a");
    assert!(c.next(&l).unwrap().is_none());
}

#[test]
fn remove_without_next() {
    let (mut l, _) = sample(&["a"]);
    assert_eq!(
        l.cursor(0).unwrap().remove(&mut l),
        Err(EntryListError::NoCurrentEntry)
    );
}

#[test]
fn start_at_index() {
    let (l, _) = sample(&["a", "b", "c"]);
    let mut c = l.cursor(2).unwrap();
    assert_eq!(*c.next(&l).unwrap().unwrap(), "c");
    assert!(!c.has_next(&l).unwrap());
    let mut c = l.cursor(2).unwrap();
    assert_eq!(*c.previous(&l).unwrap().unwrap(), "b");
}

#[test]
fn remove_via_iterator_on_gappy_list() {
    let (mut l, e) = sample(&["a", "b", "c", "d"]);
    l.remove(&e[1]).unwrap();
    let mut c = l.cursor(0).unwrap();
    for x in ["a", "c"] {
        assert_eq!(*c.next(&l).unwrap().unwrap(), x);
    }
    c.remove(&mut l).unwrap();
    assert_eq!(l.len(), 2);
    assert!(c.has_next(&l).unwrap());
    assert!(c.has_previous(&l).unwrap());
    assert_eq!(*c.next(&l).unwrap().unwrap(), "d");
    assert!(!c.has_next(&l).unwrap());
    assert_eq!(copy(&mut l), ["a", "d"]);
}

#[test]
fn next_remove_next_previous_cycle() {
    let (mut l, e) = sample(&["a", "x", "b", "c"]);
    l.remove(&e[1]).unwrap();
    let mut c = l.cursor(0).unwrap();
    for x in ["a", "b"] {
        assert_eq!(*c.next(&l).unwrap().unwrap(), x);
    }
    c.remove(&mut l).unwrap();
    assert_eq!(*c.next(&l).unwrap().unwrap(), "c");
    for x in ["c", "a"] {
        assert_eq!(*c.previous(&l).unwrap().unwrap(), x);
    }
    assert!(!c.has_previous(&l).unwrap());
    assert_eq!(values(&l), ["a", "c"]);
}

#[test]
fn remove_last_element_no_double_cme() {
    let (mut l, _) = sample(&["a", "b", "c"]);
    let mut c = l.cursor(0).unwrap();
    while c.has_next(&l).unwrap() {
        c.next(&l).unwrap();
        c.remove(&mut l).unwrap();
    }
    assert!(l.is_empty());
}

#[test]
fn set_without_next_or_previous() {
    let (mut l, _) = sample(&["a"]);
    assert_eq!(
        l.cursor(0).unwrap().set(&mut l, "x"),
        Err(EntryListError::NoCurrentEntry)
    );
}

#[test]
fn compact_removes_all_gaps() {
    let (mut l, e) = sample(&["a", "b", "c", "d", "e"]);
    for i in [1, 3] {
        l.remove(&e[i]).unwrap();
    }
    l.compact();
    assert_eq!(values(&l), ["a", "c", "e"]);
    for (i, x) in ["a", "c", "e"].into_iter().enumerate() {
        assert_eq!(*l.get(i).unwrap(), x);
    }
}

#[test]
fn compact_no_op_when_already_compact() {
    let (mut l, _) = sample(&["a", "b", "c"]);
    l.compact();
    assert_eq!(values(&l), ["a", "b", "c"]);
}

#[test]
fn compact_empty_list() {
    let mut l = EntryList::<String>::new();
    l.compact();
    assert!(l.is_empty());
}

#[test]
fn get_before_first_gap_fast_path() {
    let (mut l, e) = sample(&["a", "b", "c", "d"]);
    l.remove(&e[2]).unwrap();
    for (i, x) in ["a", "b", "d"].into_iter().enumerate() {
        assert_eq!(*l.get(i).unwrap(), x);
    }
}

#[test]
fn first_gap_advances_after_partial_compact() {
    let (mut l, e) = sample(&["a", "b", "c", "d", "e"]);
    for i in [1, 3] {
        l.remove(&e[i]).unwrap();
    }
    for (i, x) in [(1, "c"), (0, "a"), (1, "c"), (2, "e")] {
        assert_eq!(*l.get(i).unwrap(), x);
    }
    assert_eq!(values(&l), ["a", "c", "e"]);
}

#[test]
fn remove_lowering_boundary_still_compacts() {
    let (mut l, e) = sample(&["a", "b", "c", "d"]);
    l.remove(&e[2]).unwrap();
    l.get(2);
    l.remove(&e[0]).unwrap();
    assert_eq!(*l.get(0).unwrap(), "b");
    assert_eq!(*l.get(1).unwrap(), "d");
    assert_eq!(values(&l), ["b", "d"]);
}

#[test]
fn stress_test_against_reference_array_list() {
    for seed in seeds() {
        let mut rng = seed;
        let mut l = EntryList::new();
        let mut reference = vec![];
        let mut live = vec![];
        let mut counter = 0;
        for _ in 0..10_000 {
            match random(&mut rng, if reference.is_empty() { 2 } else { 5 }) {
                0 | 1 => {
                    let x = format!("e{counter}");
                    counter += 1;
                    reference.push(x.clone());
                    live.push(l.push(x));
                }
                2 => {
                    let i = random(&mut rng, live.len());
                    let e = live.remove(i);
                    let p = reference.iter().position(|v| v == &*e.value()).unwrap();
                    reference.remove(p);
                    l.remove(&e).unwrap();
                }
                3 => {
                    let i = random(&mut rng, reference.len());
                    assert_eq!(*l.get(i).unwrap(), reference[i]);
                }
                4 => {
                    l.compact();
                    assert_eq!(values(&l), reference);
                }
                _ => unreachable!(),
            }
            assert_eq!(l.len(), reference.len());
        }
        assert_eq!(values(&l), reference);
    }
}

#[test]
fn slot_count_covers_gaps() {
    let mut l = EntryList::new();
    assert_eq!(l.slot_count(), 0);
    let e = fill(&mut l, 20);
    assert_eq!(l.slot_count(), 20);
    l.remove(&e[3]).unwrap();
    assert_eq!(l.len(), 19);
    assert_eq!(l.slot_count(), 20);
}

#[test]
fn entry_at_distinguishes_gap_from_null_element() {
    let (mut l, e) = sample(&[None, Some("b"), Some("c"), Some("d"), Some("e")]);
    l.remove(&e[1]).unwrap();
    assert!(l.entry_at(1).unwrap().is_none());
    assert_eq!(*l.entry_at(0).unwrap().unwrap().value(), None);
}

#[test]
fn entry_at_does_not_compact() {
    let mut l = EntryList::new();
    let e = fill(&mut l, 20);
    l.remove(&e[1]).unwrap();
    for i in 0..l.slot_count() {
        l.entry_at(i).unwrap();
    }
    assert_eq!(l.slot_count(), 20);
    assert_eq!(e[19].position(), Some(19));
}

#[test]
fn slot_space_stays_bounded() {
    let mut l = EntryList::new();
    let mut e = fill(&mut l, 2000);
    let mut rng = 0;
    for next in 2000..22_000 {
        let i = random(&mut rng, e.len());
        l.remove(&e.remove(i)).unwrap();
        e.push(l.push(format!("e{next}")));
        assert!(l.slot_count() * 4 <= l.len() * 5);
    }
    assert_eq!(l.len(), 2000);
    assert_eq!(copy(&mut l).len(), 2000);
}

#[test]
fn entries_survive_compaction() {
    let mut l = EntryList::new();
    let e = fill(&mut l, 20);
    for i in (0..20).step_by(2) {
        l.remove(&e[i]).unwrap();
    }
    assert_eq!(l.len(), 10);
    assert_eq!(
        copy(&mut l),
        (1..20)
            .step_by(2)
            .map(|i| format!("e{i}"))
            .collect::<Vec<_>>()
    );
    for (p, i) in (1..20).step_by(2).enumerate() {
        assert_eq!(e[i].position(), Some(p));
    }
    l.remove(&e[5]).unwrap();
    assert!(!copy(&mut l).contains(&"e5".to_owned()));
}

#[test]
fn list_iterator_remove_all_across_compaction() {
    let mut l = EntryList::new();
    fill(&mut l, 50);
    let mut c = l.cursor(0).unwrap();
    let mut seen = vec![];
    while c.has_next(&l).unwrap() {
        seen.push((*c.next(&l).unwrap().unwrap()).clone());
        c.remove(&mut l).unwrap();
    }
    assert_eq!(seen.len(), 50);
    assert_eq!(&seen[..2], ["e0", "e1"]);
    assert_eq!(&seen[48..], ["e48", "e49"]);
    assert!(l.is_empty());
}

#[test]
fn list_iterator_partial_remove_across_compaction() {
    let mut l = EntryList::new();
    fill(&mut l, 40);
    let mut c = l.cursor(0).unwrap();
    let mut seen = 0;
    while c.has_next(&l).unwrap() {
        c.next(&l).unwrap();
        seen += 1;
        if seen % 2 == 1 {
            c.remove(&mut l).unwrap();
        }
    }
    assert_eq!(seen, 40);
    assert_eq!(l.len(), 20);
    assert_eq!(
        copy(&mut l),
        (1..40)
            .step_by(2)
            .map(|i| format!("e{i}"))
            .collect::<Vec<_>>()
    );
}
