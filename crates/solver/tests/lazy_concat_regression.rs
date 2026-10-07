use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn random_sampling_loads_only_selected_groups_and_preserves_occurrences() {
    let calls = [Cell::new(0), Cell::new(0)];
    let mut c = LazyConcat::new();
    c.push_deferred(2, || {
        calls[0].set(calls[0].get() + 1);
        vec![(0, "a"), (1, "a")]
    })
    .unwrap();
    c.push_deferred(3, || {
        calls[1].set(calls[1].get() + 1);
        vec![(2, "b"), (3, "b"), (4, "c")]
    })
    .unwrap();
    let mut first = c.samples(42);
    assert_eq!(calls.iter().map(Cell::get).sum::<usize>(), 0);
    assert!(first.next().unwrap().is_ok());
    assert_eq!(calls.iter().map(Cell::get).sum::<usize>(), 1);
    drop(first);
    for seed in 0..20 {
        let mut samples = c.samples(seed).map(|v| *v.unwrap()).collect::<Vec<_>>();
        samples.sort();
        assert_eq!(samples, [(0, "a"), (1, "a"), (2, "b"), (3, "b"), (4, "c")]);
    }
    assert_eq!(calls.each_ref().map(|c| c.get()), [1, 1]);
}
#[test]
fn bad_supplier_length_is_cached_and_does_not_shift_later_groups() {
    let calls = Cell::new(0);
    let mut c = LazyConcat::new();
    c.push_deferred(2, || {
        calls.set(calls.get() + 1);
        vec![7]
    })
    .unwrap();
    c.push(vec![9]).unwrap();
    let err = LazyConcatError::SupplierLength {
        group: 0,
        expected: 2,
        actual: 1,
    };
    assert_eq!(c.get(0), Err(err.clone()));
    assert_eq!(c.get(1), Err(err.clone()));
    assert_eq!(c.get(2), Ok(&9));
    assert_eq!(calls.get(), 1);
    assert_eq!(
        c.iter().collect::<Vec<_>>(),
        [Err(err.clone()), Err(err), Ok(&9)]
    );
    assert_eq!(c.samples(9).filter(Result::is_ok).count(), 1);
    assert_eq!(calls.get(), 1);
}
#[test]
fn supplier_panic_can_retry_and_bounds_and_empty_groups_are_passive() {
    let calls = Cell::new(0);
    let mut c = LazyConcat::new();
    c.push_deferred(0, || panic!("empty group must not run"))
        .unwrap();
    c.push_deferred(1, || {
        calls.set(calls.get() + 1);
        assert_ne!(calls.get(), 1, "first attempt");
        vec![3]
    })
    .unwrap();
    assert_eq!(c.len(), 1);
    assert!(c.get(1).is_err());
    assert_eq!(calls.get(), 0);
    assert!(catch_unwind(AssertUnwindSafe(|| c.get(0))).is_err());
    assert_eq!(c.get(0), Ok(&3));
    assert_eq!(c.get(0), Ok(&3));
    assert_eq!(calls.get(), 2);
}
#[test]
fn borrowing_nonclone_values_reverse_iteration_and_overflow() {
    struct Candidate(u8);
    let values = [Candidate(1), Candidate(2)];
    let mut c = LazyConcat::new();
    c.push_slice(&values).unwrap();
    c.push(vec![Candidate(3)]).unwrap();
    assert!(std::ptr::eq(c.get(1).unwrap(), &values[1]));
    let mut it = c.iter();
    assert_eq!(it.len(), 3);
    assert_eq!(it.next_back().unwrap().unwrap().0, 3);
    assert_eq!(it.next().unwrap().unwrap().0, 1);
    assert_eq!(it.len(), 1);
    assert_eq!(it.next().unwrap().unwrap().0, 2);
    assert!(it.next().is_none());
    assert!(it.next_back().is_none());
    let mut huge = LazyConcat::<u8>::new();
    huge.push_deferred(usize::MAX, || panic!("length must be passive"))
        .unwrap();
    assert_eq!(huge.push(vec![1]), Err(LazyConcatError::LengthOverflow));
    assert_eq!(huge.len(), usize::MAX);
    assert_eq!(
        huge.get(usize::MAX),
        Err(LazyConcatError::Index {
            index: usize::MAX,
            length: usize::MAX
        })
    );
}
