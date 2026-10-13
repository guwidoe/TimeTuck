//! Original merge/split/removal publication, no-clone reads and panic controls.
use std::{
    cell::Cell,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    rc::Rc,
};
use timetuck::*;
fn hash(value: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}
#[test]
fn anchored_views_follow_merges_splits_removal_and_reinsertion_while_snapshots_stay_saved() {
    let mut c = LiveConnectedRangeCollector::<usize, i64, ()>::default();
    c.insert(&(0, 0, 2, ()));
    c.insert(&(1, 4, 6, ()));
    let left = c.view(0).unwrap();
    let right = c.view(1).unwrap();
    let saved = c.snapshot();
    let old_hash = hash(&left);
    c.insert(&(2, 2, 4, ()));
    assert_eq!(hash(&left), hash(&right));
    left.with_current(|r| assert_eq!((*r.unwrap().start(), *r.unwrap().end()), (0, 6)));
    assert!(c.remove(&(2, 99, 100, ())));
    assert_eq!(hash(&left), old_hash);
    right.with_current(|r| assert_eq!((*r.unwrap().start(), *r.unwrap().end()), (4, 6)));
    assert_eq!(saved.ranges().len(), 2);
    assert!(c.remove(&(0, 0, 0, ())));
    left.with_current(|r| assert!(r.is_none()));
    c.insert(&(0, 10, 12, ()));
    left.with_current(|r| assert_eq!((*r.unwrap().start(), *r.unwrap().end()), (10, 12)));
    let before = c.snapshot();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c.insert(&(0, 10, 13, ()))))
            .is_err()
    );
    assert_eq!(c.snapshot(), before);
    left.with_current(|r| assert_eq!(*r.unwrap().end(), 12));
}
#[derive(Eq, PartialEq, Hash)]
struct Payload {
    value: i32,
    clones: Counts,
}
struct Counts(Rc<Cell<usize>>);
impl PartialEq for Counts {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for Counts {}
impl Hash for Counts {
    fn hash<H: Hasher>(&self, _: &mut H) {}
}
impl Clone for Payload {
    fn clone(&self) -> Self {
        self.clones.0.set(self.clones.0.get() + 1);
        Self {
            value: self.value,
            clones: Counts(self.clones.0.clone()),
        }
    }
}
#[test]
fn repeated_hash_reads_borrow_published_component_without_cloning_payloads() {
    let clones = Rc::new(Cell::new(0));
    let mut c = LiveConnectedRangeCollector::<usize, i64, Payload>::default();
    c.insert(&(
        0,
        0,
        2,
        Payload {
            value: 1,
            clones: Counts(clones.clone()),
        },
    ));
    let view = c.view(0).unwrap();
    let before = clones.get();
    let expected = hash(&view);
    for _ in 0..100 {
        assert_eq!(hash(&view), expected);
    }
    assert_eq!(clones.get(), before);
}
