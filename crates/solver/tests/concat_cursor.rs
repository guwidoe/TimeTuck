//! Original bidirectional cursor, skipping, deferred failures and panic contracts.
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeSet, VecDeque},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn mixed_end_traversal_and_skips_match_rank_oracle_without_loading_skipped_groups() {
    for seed in 0..100_u64 {
        let loads = RefCell::new(vec![]);
        let mut c = LazyConcat::new();
        let mut model = VecDeque::new();
        let mut ordinal = 0;
        for (source, len) in [0, 3, 1, 0, 4, 2, 5].into_iter().enumerate() {
            if len == 0 {
                c.push_deferred(0, || panic!("empty supplier")).unwrap();
                continue;
            }
            let group = ordinal;
            ordinal += 1;
            let load_log = &loads;
            c.push_deferred(len, move || {
                load_log.borrow_mut().push(group);
                if source == 2 {
                    vec![]
                } else {
                    (0..len).map(|i| (source * 10 + i) as i64).collect()
                }
            })
            .unwrap();
            for i in 0..len {
                model.push_back((
                    group,
                    if source == 2 {
                        Err(LazyConcatError::SupplierLength {
                            group,
                            expected: len,
                            actual: 0,
                        })
                    } else {
                        Ok((source * 10 + i) as i64)
                    },
                ));
            }
        }
        let mut iterator = c.iter();
        let mut expected_loads = vec![];
        let mut loaded = BTreeSet::new();
        let mut state = seed;
        assert!(loads.borrow().is_empty());
        for step in 0..40 {
            assert_eq!(iterator.len(), model.len());
            assert_eq!(iterator.size_hint(), (model.len(), Some(model.len())));
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let operation = (state >> 32) % 4;
            let n = if step == 17 {
                usize::MAX
            } else {
                ((state >> 40) % 4) as usize
            };
            let back = operation == 1 || operation == 3;
            let skip = if operation >= 2 {
                n.min(model.len())
            } else {
                0
            };
            for _ in 0..skip {
                if back {
                    model.pop_back();
                } else {
                    model.pop_front();
                }
            }
            let expected = if back {
                model.pop_back()
            } else {
                model.pop_front()
            };
            if let Some((g, _)) = &expected
                && loaded.insert(*g)
            {
                expected_loads.push(*g);
            }
            let actual = match operation {
                0 => iterator.next(),
                1 => iterator.next_back(),
                2 => iterator.nth(n),
                _ => iterator.nth_back(n),
            };
            assert_eq!(
                actual.map(|r| r.copied()),
                expected.map(|(_, v)| v),
                "seed={seed} step={step}"
            );
            assert_eq!(*loads.borrow(), expected_loads);
        }
        assert!(iterator.next().is_none());
        assert!(iterator.next_back().is_none());
    }
}
#[test]
fn large_skips_and_size_queries_never_materialize_virtual_groups() {
    let mut c = LazyConcat::<u8>::new();
    c.push_deferred(usize::MAX, || panic!("must stay unmaterialized"))
        .unwrap();
    let mut it = c.iter();
    assert_eq!(it.len(), usize::MAX);
    assert_eq!(it.size_hint(), (usize::MAX, Some(usize::MAX)));
    assert_eq!(it.nth(usize::MAX), None);
    assert_eq!(it.len(), 0);
    assert_eq!(it.next_back(), None);
    let mut it = c.iter();
    assert_eq!(it.nth_back(usize::MAX), None);
    assert_eq!(it.len(), 0);
    assert_eq!(it.next(), None);
    let mut sparse = LazyConcat::new();
    sparse
        .push_deferred(usize::MAX - 1, || panic!("skip this group"))
        .unwrap();
    sparse.push(vec![7]).unwrap();
    assert_eq!(sparse.iter().nth(usize::MAX - 1), Some(Ok(&7)));
    let mut sparse = LazyConcat::new();
    sparse.push(vec![8]).unwrap();
    sparse
        .push_deferred(usize::MAX - 1, || panic!("skip this group"))
        .unwrap();
    assert_eq!(sparse.iter().nth_back(usize::MAX - 1), Some(Ok(&8)));
}
#[test]
fn caught_supplier_panics_consume_the_selected_rank_and_retry_the_next_rank() {
    for reverse in [false, true] {
        for skip in [false, true] {
            let calls = Cell::new(0);
            let mut c = LazyConcat::new();
            c.push_deferred(4, || {
                calls.set(calls.get() + 1);
                assert_ne!(calls.get(), 1, "first load");
                vec![0, 1, 2, 3]
            })
            .unwrap();
            let mut it = c.iter();
            let result = catch_unwind(AssertUnwindSafe(|| match (reverse, skip) {
                (false, false) => it.next(),
                (true, false) => it.next_back(),
                (false, true) => it.nth(1),
                (true, true) => it.nth_back(1),
            }));
            assert!(result.is_err());
            assert_eq!(it.len(), if skip { 2 } else { 3 });
            let expected = match (reverse, skip) {
                (false, false) => 1,
                (true, false) => 2,
                (false, true) => 2,
                (true, true) => 1,
            };
            assert_eq!(
                if reverse { it.next_back() } else { it.next() },
                Some(Ok(&expected))
            );
            assert_eq!(calls.get(), 2);
        }
    }
}
#[test]
#[allow(clippy::iter_nth_zero)] // Exercise nth's exhausted-cursor path explicitly.
fn borrowed_owned_and_deferred_payloads_keep_identity_under_crossing_cursors() {
    struct Value(usize);
    let borrowed = [Value(0), Value(1)];
    let mut c = LazyConcat::new();
    c.push_slice(&borrowed).unwrap();
    c.push(vec![Value(2), Value(3)]).unwrap();
    c.push_deferred(2, || vec![Value(4), Value(5)]).unwrap();
    let pointers: Vec<_> = (0..6).map(|i| c.get(i).unwrap() as *const Value).collect();
    let mut it = c.iter();
    for (back, index) in [
        (false, 0),
        (true, 5),
        (false, 1),
        (true, 4),
        (true, 3),
        (false, 2),
    ] {
        let value = if back { it.next_back() } else { it.next() }
            .unwrap()
            .unwrap();
        assert_eq!(value.0, index);
        assert_eq!(value as *const Value, pointers[index]);
    }
    assert_eq!(it.len(), 0);
    assert!(it.nth(0).is_none());
    assert!(it.nth_back(0).is_none());
}
