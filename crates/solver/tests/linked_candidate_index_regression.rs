use std::{cell::Cell, rc::Rc};
use timetuck::*;
struct Payload {
    label: String,
    drops: Rc<Cell<usize>>,
}
impl Drop for Payload {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
fn payload(label: &str, drops: &Rc<Cell<usize>>) -> Payload {
    Payload {
        label: label.into(),
        drops: drops.clone(),
    }
}
fn sum_ids<'a, L: CandidateLeafView<'a, usize, Payload>>(leaf: &L) -> usize {
    leaf.iter().map(|entry| entry.id).sum()
}
#[test]
fn borrowed_nodes_preserve_identity_order_and_owned_removal() {
    let drops = Rc::new(Cell::new(0));
    let mut linked = LinkedCandidateIndex::new();
    for id in 0..6 {
        linked.insert(id, "A", payload("equal", &drops)).unwrap();
    }
    let address = linked.get(&4).unwrap() as *const _ as usize;
    let leaf = linked.bucket(&"A");
    assert_eq!(sum_ids(&leaf), 15);
    assert!(matches!(
        leaf.repeating(0),
        Err(LeafRandomAccessUnsupported)
    ));
    assert!(matches!(leaf.unique(0), Err(LeafRandomAccessUnsupported)));
    for id in [0, 2, 5] {
        let removed = linked.remove(&id).unwrap();
        assert_eq!(removed.id, id);
        assert_eq!(removed.value.label, "equal");
        drop(removed);
    }
    assert_eq!(
        linked.bucket(&"A").iter().map(|e| e.id).collect::<Vec<_>>(),
        vec![1, 3, 4]
    );
    assert_eq!(linked.get(&4).unwrap() as *const _ as usize, address);
    assert_eq!(drops.get(), 3);
    let previous = linked.replace(&3, "A", payload("updated", &drops)).unwrap();
    assert_eq!(previous.label, "equal");
    drop(previous);
    assert_eq!(
        linked.bucket(&"A").iter().map(|e| e.id).collect::<Vec<_>>(),
        vec![1, 3, 4]
    );
    assert_eq!(linked.get(&3).unwrap().value.label, "updated");
    let previous = linked.replace(&3, "B", payload("moved", &drops)).unwrap();
    drop(previous);
    assert_eq!(
        linked.bucket(&"A").iter().map(|e| e.id).collect::<Vec<_>>(),
        vec![1, 4]
    );
    assert_eq!(
        linked.bucket(&"B").iter().next().unwrap().value.label,
        "moved"
    );
    assert_eq!(linked.key_count(), 2);
    drop(linked);
    assert_eq!(drops.get(), 8);
}
#[test]
fn both_backends_use_common_leaf_interface_and_dense_sampling_remains_real() {
    let drops = Rc::new(Cell::new(0));
    let mut dense = CandidateIndex::new();
    let mut linked = LinkedCandidateIndex::new();
    for id in 0..8 {
        dense.insert(id, (), payload("dense", &drops)).unwrap();
        linked.insert(id, (), payload("linked", &drops)).unwrap();
    }
    let dense = dense.bucket(&());
    let linked = linked.bucket(&());
    assert_eq!(sum_ids(&dense), sum_ids(&linked));
    let mut repeating = CandidateLeafView::repeating(&dense, 7).unwrap();
    let expected = dense
        .repeating(7)
        .take(20)
        .map(|e| e.id)
        .collect::<Vec<_>>();
    assert_eq!(
        repeating
            .by_ref()
            .take(20)
            .map(|e| e.id)
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        repeating.try_remove(),
        Err(RandomCursorOperationError::RemovalUnsupported)
    );
    assert_eq!(
        repeating.try_for_each_remaining(|_| panic!("must stay silent")),
        Err(RandomCursorOperationError::RepeatingDrainUnsupported)
    );
    let mut unique = CandidateLeafView::unique(&dense, 7).unwrap();
    let mut ids = unique.by_ref().map(|e| e.id).collect::<Vec<_>>();
    ids.sort();
    assert_eq!(ids, (0..8).collect::<Vec<_>>());
    assert!(unique.next().is_none());
    assert!(matches!(
        CandidateLeafView::repeating(&linked, 7),
        Err(LeafRandomAccessUnsupported)
    ));
    assert!(matches!(
        CandidateLeafView::unique(&linked, 7),
        Err(LeafRandomAccessUnsupported)
    ));
    assert_eq!(drops.get(), 0);
}
#[test]
fn unknown_duplicate_empty_and_rekey_errors_keep_current_storage() {
    let mut index = LinkedCandidateIndex::new();
    index.insert(0, "A", "first").unwrap();
    assert_eq!(
        index.insert(0, "B", "duplicate"),
        Err(CandidateIndexError::DuplicateId(0))
    );
    assert_eq!(index.remove(&2), Err(CandidateIndexError::UnknownId(2)));
    assert_eq!(
        index.replace(&2, "C", "unknown"),
        Err(CandidateIndexError::UnknownId(2))
    );
    assert_eq!(index.len(), 1);
    assert_eq!(index.key_count(), 1);
    assert!(index.bucket(&"missing").is_empty());
    assert!(matches!(
        index.bucket(&"missing").unique(0),
        Err(LeafRandomAccessUnsupported)
    ));
    assert_eq!(index.replace(&0, "B", "current").unwrap(), "first");
    assert!(index.bucket(&"A").is_empty());
    assert_eq!(index.key_count(), 1);
    assert_eq!(index.remove(&0).unwrap().value, "current");
    assert!(index.is_empty());
    assert_eq!(index.key_count(), 0);
}
