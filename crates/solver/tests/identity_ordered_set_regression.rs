// Original tests; no upstream-method credit.
use std::sync::{
    Arc,
    atomic::{AtomicI32, Ordering},
};
use timetuck::{IdentityOrderedSet, IdentitySnapshot};

#[test]
fn mutable_equal_payloads_have_independent_stable_membership() {
    // Interior mutation changes the value, but allocation identity is the whole key.
    let a = Arc::new(AtomicI32::new(5));
    let b = Arc::new(AtomicI32::new(5));
    let absent = Arc::new(AtomicI32::new(5));
    let mut set: IdentityOrderedSet<_> = [a.clone(), b.clone(), a.clone()].into_iter().collect();
    assert_eq!(set.len(), 2);
    a.store(17, Ordering::Relaxed);
    b.store(17, Ordering::Relaxed);
    assert!(set.contains(&a) && set.contains(&b));
    assert!(!set.remove(&absent));
    assert!(set.remove(&a));
    assert!(set.contains(&b));
    assert!(set.insert(a.clone()));
    let order = set.snapshot();
    assert!(Arc::ptr_eq(&order[0], &b));
    assert!(Arc::ptr_eq(&order[1], &a));
}
#[test]
fn removal_and_reinsertion_match_independent_ordered_reference() {
    let objects: Vec<_> = (0..67).map(|_| Arc::new(AtomicI32::new(0))).collect();
    let mut set = IdentityOrderedSet::with_capacity(67);
    let mut reference: Vec<Arc<AtomicI32>> = vec![];
    let mut random = 0x117b_af93_u64;
    for step in 0..10_000 {
        random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
        let object = &objects[((random >> 32) as usize) % objects.len()];
        let position = reference.iter().position(|v| Arc::ptr_eq(v, object));
        match random % 13 {
            0 => {
                set.clear();
                reference.clear();
            }
            1..=5 => {
                assert_eq!(set.remove(object), position.is_some());
                if let Some(position) = position {
                    reference.remove(position);
                }
            }
            _ => {
                assert_eq!(set.insert(object.clone()), position.is_none());
                if position.is_none() {
                    reference.push(object.clone());
                }
            }
        }
        object.store(step, Ordering::Relaxed);
        assert_eq!(set.len(), reference.len());
        assert_eq!(set.is_empty(), reference.is_empty());
        for (a, b) in set.iter().zip(&reference) {
            assert!(Arc::ptr_eq(a, b));
        }
        for (a, b) in set.iter().rev().zip(reference.iter().rev()) {
            assert!(Arc::ptr_eq(a, b));
        }
        assert_eq!(
            set.first().map(Arc::as_ptr),
            reference.first().map(Arc::as_ptr)
        );
        assert_eq!(
            set.last().map(Arc::as_ptr),
            reference.last().map(Arc::as_ptr)
        );
        for object in &objects {
            assert_eq!(
                set.contains(object),
                reference.iter().any(|v| Arc::ptr_eq(v, object))
            );
        }
    }
}
#[test]
fn mixed_direction_iteration_is_exact_fused_and_borrowed() {
    let objects: Vec<_> = (0..5).map(Arc::new).collect();
    let set: IdentityOrderedSet<_> = objects.iter().cloned().collect();
    let mut iter = (&set).into_iter();
    assert_eq!(iter.size_hint(), (5, Some(5)));
    assert!(Arc::ptr_eq(iter.next().unwrap(), &objects[0]));
    assert!(Arc::ptr_eq(iter.next_back().unwrap(), &objects[4]));
    assert_eq!(iter.len(), 3);
    assert!(Arc::ptr_eq(iter.next_back().unwrap(), &objects[3]));
    assert!(Arc::ptr_eq(iter.next().unwrap(), &objects[1]));
    assert!(Arc::ptr_eq(iter.next_back().unwrap(), &objects[2]));
    assert_eq!(iter.len(), 0);
    for _ in 0..3 {
        assert!(iter.next().is_none());
        assert!(iter.next_back().is_none());
    }
    assert!(objects.iter().all(|v| Arc::strong_count(v) == 2));
}
#[test]
fn reusable_buffers_clear_only_terminator_and_leave_short_buffers_intact() {
    let member = Arc::new(1);
    let sentinel = Arc::new(99);
    let set: IdentityOrderedSet<_> = [member.clone()].into_iter().collect();
    let mut large = vec![Some(sentinel.clone()); 4];
    let address = large.as_ptr();
    let result = set.snapshot_into(&mut large);
    assert!(matches!(&result, IdentitySnapshot::Borrowed(_)));
    assert_eq!(result.as_slice().as_ptr(), address);
    assert!(Arc::ptr_eq(result.as_slice()[0].as_ref().unwrap(), &member));
    assert!(result.as_slice()[1].is_none());
    for slot in &result.as_slice()[2..] {
        assert!(Arc::ptr_eq(slot.as_ref().unwrap(), &sentinel));
    }
    let two: IdentityOrderedSet<_> = [member.clone(), sentinel.clone()].into_iter().collect();
    let mut short = [Some(sentinel.clone())];
    let result = two.snapshot_into(&mut short);
    assert!(matches!(&result, IdentitySnapshot::Owned(_)));
    assert_eq!(result.as_slice().len(), 2);
    assert!(Arc::ptr_eq(result.as_slice()[0].as_ref().unwrap(), &member));
    assert!(Arc::ptr_eq(
        result.as_slice()[1].as_ref().unwrap(),
        &sentinel
    ));
    assert!(Arc::ptr_eq(short[0].as_ref().unwrap(), &sentinel));
    let empty = IdentityOrderedSet::<i32>::new();
    let mut buffer = [Some(member), Some(sentinel.clone())];
    assert_eq!(empty.snapshot_into(&mut buffer).as_slice().len(), 2);
    assert!(buffer[0].is_none());
    assert!(Arc::ptr_eq(buffer[1].as_ref().unwrap(), &sentinel));
    assert!(matches!(
        empty.snapshot_into(&mut []),
        IdentitySnapshot::Borrowed(_)
    ));
}
#[test]
fn membership_owns_objects_and_removal_clear_drop_release_them() {
    let mut set = IdentityOrderedSet::new();
    let a = Arc::new(String::from("a"));
    let b = Arc::new(String::from("b"));
    let (wa, wb) = (Arc::downgrade(&a), Arc::downgrade(&b));
    set.extend([a.clone(), b.clone()]);
    assert_eq!(Arc::strong_count(&a), 2);
    assert!(!set.insert(a.clone()));
    assert_eq!(Arc::strong_count(&a), 2);
    assert!(set.remove(&a));
    assert_eq!(Arc::strong_count(&a), 1);
    drop(a);
    assert!(wa.upgrade().is_none());
    drop(b);
    assert!(wb.upgrade().is_some());
    set.clear();
    assert!(wb.upgrade().is_none());
    let c = Arc::new(5);
    let weak = Arc::downgrade(&c);
    let set: IdentityOrderedSet<_> = [c].into_iter().collect();
    assert!(weak.upgrade().is_some());
    drop(set);
    assert!(weak.upgrade().is_none());
}
#[test]
fn clones_share_objects_but_have_independent_membership_and_can_move_threads() {
    struct NoTraits;
    let (a, b) = (Arc::new(NoTraits), Arc::new(NoTraits));
    let mut original: IdentityOrderedSet<_> = [a.clone(), b.clone()].into_iter().collect();
    let mut clone = original.clone();
    original.remove(&a);
    clone.remove(&b);
    assert!(clone.contains(&a));
    assert!(original.contains(&b));
    assert!(!clone.contains(&b));
    assert!(!original.contains(&a));
    std::thread::spawn(move || {
        assert_eq!(clone.len(), 1);
        assert!(Arc::ptr_eq(clone.first().unwrap(), &a));
    })
    .join()
    .unwrap();
    assert!(!original.contains_all([&Arc::new(NoTraits)]));
    assert!(original.contains_all([]));
    assert!(!original.extend([]));
    assert!(!original.remove_all([]));
}
