// Original native regressions; no upstream method credits.
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn virtual_spaces_allocate_only_for_retirements_and_clone_independently() {
    let mut ranks = RetiringIndices::new(u128::MAX, 17);
    for _ in 0..10_000 {
        assert!(ranks.next().is_some());
    }
    assert_eq!(ranks.stored_indices(), 0);
    let mut fork = ranks.clone();
    assert_eq!(ranks.draw(), fork.draw());
    let mut retired = BTreeSet::new();
    for i in 0..1000 {
        let value = ranks.draw().unwrap();
        assert!(retired.insert(value));
        ranks.retire().unwrap();
        assert_eq!(ranks.remaining(), u128::MAX - i - 1);
        assert!(ranks.stored_indices() <= i as usize + 1);
    }
    assert_eq!(fork.remaining(), u128::MAX);
    assert_eq!(fork.stored_indices(), 0);
}
#[test]
fn lazy_groups_errors_and_supplier_panics_preserve_retirement_boundaries() {
    let loads = [Cell::new(0), Cell::new(0), Cell::new(0)];
    let mut view = LazyConcat::new();
    view.push_deferred(2, || {
        loads[0].set(loads[0].get() + 1);
        vec![10, 11]
    })
    .unwrap();
    view.push_deferred(1, || {
        loads[1].set(loads[1].get() + 1);
        vec![]
    })
    .unwrap();
    view.push_deferred(2, || {
        loads[2].set(loads[2].get() + 1);
        vec![12, 13]
    })
    .unwrap();
    let mut draw = view.retiring(7);
    assert_eq!(loads.each_ref().map(Cell::get), [0, 0, 0]);
    assert_eq!(draw.retire(), Err(NoDrawToRetire));
    let mut values = BTreeSet::new();
    let mut errors = 0;
    while let Some(value) = draw.next() {
        match value {
            Ok(v) => {
                assert!(values.insert(*v));
            }
            Err(LazyConcatError::SupplierLength { group: 1, .. }) => errors += 1,
            Err(e) => panic!("{e}"),
        }
        draw.retire().unwrap();
        assert_eq!(draw.retire(), Err(NoDrawToRetire));
    }
    assert_eq!(values, BTreeSet::from([10, 11, 12, 13]));
    assert_eq!(errors, 1);
    assert_eq!(loads.each_ref().map(Cell::get), [1, 1, 1]);
    assert_eq!(draw.remaining(), 0);
    assert!(draw.next().is_none());
    assert_eq!(view.len(), 5);
    let panic_once = Cell::new(true);
    let mut view = LazyConcat::new();
    view.push_deferred(1, || {
        assert!(!panic_once.replace(false), "supplier panic");
        vec![42]
    })
    .unwrap();
    let mut draw = view.retiring(0);
    assert!(catch_unwind(AssertUnwindSafe(|| draw.next())).is_err());
    assert_eq!(draw.remaining(), 1);
    draw.retire().unwrap();
    assert!(draw.next().is_none());
    assert_eq!(view.get(0), Ok(&42)); // retiring the failed occurrence did not damage its source
}
#[test]
fn borrowed_pairs_preserve_duplicate_occurrences_and_independent_queries() {
    struct Payload(usize); // deliberately not Clone
    let left = [Payload(0), Payload(1)];
    let right = [Payload(2), Payload(3), Payload(4)];
    let mut pairs = CandidatePairs::new();
    pairs.push_product(&left, &right).unwrap();
    pairs.push_product(&left[..1], &right[..1]).unwrap();
    let mut expected = BTreeMap::new();
    for (l, r) in pairs.iter() {
        *expected.entry((l.0, r.0)).or_insert(0) += 1;
    }
    let mut actual = BTreeMap::new();
    let mut draw = pairs.retiring(8);
    while let Some((l, r)) = draw.next() {
        *actual.entry((l.0, r.0)).or_insert(0) += 1;
        draw.retire().unwrap();
    }
    assert_eq!(actual, expected);
    assert_eq!(draw.remaining(), 0);
    assert_eq!(draw.stored_indices(), 0);
    assert_eq!(pairs.len(), 7);
    assert_eq!(pairs.unique(8).count(), 7);
    assert_eq!(
        pairs.retiring(8).next().map(|(l, r)| (l.0, r.0)),
        pairs.retiring(8).next().map(|(l, r)| (l.0, r.0))
    );
    let mut empty = CandidatePairs::<usize, usize>::new();
    empty.push_product(&[], &[1]).unwrap();
    assert!(empty.retiring(0).next().is_none());
}
