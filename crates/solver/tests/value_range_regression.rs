//! Original overflow, lazy traversal, snapshot and solver-domain integration checks.
use std::{cell::Cell, cmp::Ordering, rc::Rc};
use timetuck::*;
#[test]
fn integer_edges_and_nondivisible_steps_match_independent_enumeration() {
    for start in -8..=8 {
        for end in start..=12 {
            for step in 1..=11 {
                let r = IntegerValueRange::new(start, end, step).unwrap();
                let expected: Vec<_> = (start..end)
                    .filter(|v| (v - start) % step as i64 == 0)
                    .collect();
                assert_eq!(r.iter().collect::<Vec<_>>(), expected);
                assert_eq!(r.len(), expected.len() as u128);
                for value in -10..=14 {
                    assert_eq!(r.contains(&value), expected.contains(&value));
                }
                assert_eq!(r.get(r.len()), None);
                assert_eq!(r.get(u128::MAX), None);
            }
        }
    }
    let huge = IntegerValueRange::new(i64::MIN, i64::MAX, 1).unwrap();
    assert_eq!(huge.len(), u128::from(u64::MAX));
    assert_eq!(huge.get(0), Some(i64::MIN));
    assert_eq!(huge.get(u128::from(u64::MAX) - 1), Some(i64::MAX - 1));
    assert_eq!(huge.get(1u128 << 63), Some(0));
    let wide_step = IntegerValueRange::new(i64::MIN, i64::MAX, u64::MAX).unwrap();
    assert_eq!(wide_step.iter().collect::<Vec<_>>(), [i64::MIN]);
    assert_eq!(
        IntegerValueRange::new(2, 1, 1),
        Err(ValueRangeError::DescendingBounds)
    );
    assert_eq!(
        IntegerValueRange::new(0, 1, 0),
        Err(ValueRangeError::ZeroStep)
    );
}
struct Tracked {
    len: u128,
    reads: Rc<Cell<usize>>,
}
impl ValueRange for Tracked {
    type Value = u128;
    fn len(&self) -> u128 {
        self.len
    }
    fn get(&self, i: u128) -> Option<u128> {
        self.reads.set(self.reads.get() + 1);
        (i < self.len).then_some(i)
    }
    fn contains(&self, i: &u128) -> bool {
        *i < self.len
    }
}
#[test]
fn wide_iterators_skip_by_rank_from_both_ends_without_reading_skipped_values() {
    let reads = Rc::new(Cell::new(0));
    let r = Tracked {
        len: u128::MAX,
        reads: reads.clone(),
    };
    let mut iter = r.iter();
    assert_eq!(iter.size_hint(), (usize::MAX, None));
    assert_eq!(iter.nth(usize::MAX), Some(usize::MAX as u128));
    assert_eq!(
        iter.nth_back(usize::MAX),
        Some(u128::MAX - 1 - usize::MAX as u128)
    );
    assert_eq!(reads.get(), 2);
    let small = EnumeratedValueRange::new([1, 2, 3, 4]);
    let mut iter = small.iter();
    assert_eq!(iter.next(), Some(1));
    assert_eq!(iter.next_back(), Some(4));
    assert_eq!(iter.size_hint(), (2, Some(2)));
    assert_eq!(iter.nth(1), Some(3));
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next(), None);
    assert_eq!(small.iter().nth(usize::MAX), None);
    assert_eq!(small.iter().nth_back(usize::MAX), None);
}
#[test]
fn composition_preserves_duplicates_empty_children_and_wide_prefix_boundaries() {
    let empty =
        || Box::new(EnumeratedValueRange::<i64>::new([])) as Box<dyn ValueRange<Value = i64>>;
    let huge = IntegerValueRange::new(i64::MIN, i64::MAX, 1).unwrap();
    let width = huge.len();
    let r = CompositeValueRange::new(vec![
        empty(),
        Box::new(huge),
        empty(),
        Box::new(EnumeratedValueRange::new([17, 17])),
        empty(),
    ])
    .unwrap();
    assert_eq!(r.len(), width + 2);
    assert_eq!(r.get(width - 1), Some(i64::MAX - 1));
    assert_eq!(r.get(width), Some(17));
    assert_eq!(r.get(width + 1), Some(17));
    assert_eq!(r.get(width + 2), None);
    assert_eq!(
        r.iter().rev().take(3).collect::<Vec<_>>(),
        [17, 17, i64::MAX - 1]
    );
    let nested = CompositeValueRange::new(vec![
        Box::new(r) as Box<dyn ValueRange<Value = i64>>,
        empty(),
    ])
    .unwrap();
    assert_eq!(nested.len(), width + 2);
    assert!(nested.contains(&17));
    let reads = Rc::new(Cell::new(0));
    assert!(matches!(
        CompositeValueRange::new(vec![
            Box::new(Tracked {
                len: u128::MAX,
                reads: reads.clone()
            }),
            Box::new(Tracked {
                len: 1,
                reads: reads.clone()
            })
        ]),
        Err(ValueRangeError::CountOverflow)
    ));
    assert!(matches!(
        OptionalValueRange::new(Tracked {
            len: u128::MAX,
            reads: reads.clone()
        }),
        Err(ValueRangeError::CountOverflow)
    ));
    assert_eq!(reads.get(), 0);
}
#[test]
fn explicit_materialization_limits_prevent_reads_and_preserve_optional_layers() {
    let reads = Rc::new(Cell::new(0));
    let r = Tracked {
        len: 1u128 << 100,
        reads: reads.clone(),
    };
    assert!(matches!(
        r.materialize(10),
        Err(ValueRangeError::MaterializationLimit { limit: 10, .. })
    ));
    assert!(r.sorted_by(10, u128::cmp).is_err());
    assert!(variable_from_range(&r, 10).is_err());
    assert_eq!(reads.get(), 0);
    let r = OptionalValueRange::new(EnumeratedValueRange::<i32>::new([])).unwrap();
    assert!(r.sorted_present_by(0, i32::cmp).is_err());
    assert_eq!(
        r.sorted_present_by(1, i32::cmp)
            .unwrap()
            .materialize(1)
            .unwrap(),
        [None]
    );
    let nested = OptionalValueRange::new(r).unwrap();
    assert_eq!(nested.iter().collect::<Vec<_>>(), [None, Some(None)]);
    assert!(nested.contains(&Some(None)));
    assert!(!nested.contains(&Some(Some(4))));
    assert_eq!(nested.get(2), None);
}
#[test]
fn seeded_sampling_is_lazy_repeatable_and_weights_occurrences() {
    let reads = Rc::new(Cell::new(0));
    let r = Tracked {
        len: 4,
        reads: reads.clone(),
    };
    let mut iter = r.random_iter(1);
    assert_eq!(reads.get(), 0);
    assert!(iter.next().is_some());
    assert_eq!(reads.get(), 1);
    let r = EnumeratedValueRange::new([1, 1, 2]);
    assert_eq!(
        r.random_iter(54).take(500).collect::<Vec<_>>(),
        r.random_iter(54).take(500).collect::<Vec<_>>()
    );
    let mut rank = 0;
    let draws = r
        .random_iter_with(|bound| {
            assert_eq!(bound, 3);
            let i = rank;
            rank = (rank + 1) % 3;
            i
        })
        .take(6)
        .collect::<Vec<_>>();
    assert_eq!(draws, [1, 1, 2, 1, 1, 2]);
    let r = EnumeratedValueRange::<i32>::new([]);
    let mut iter = r.random_iter_with(|_| panic!("no draws"));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
}
#[test]
fn sorting_is_stable_and_keeps_source_snapshots_unchanged() {
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Row {
        key: i32,
        id: usize,
    }
    let r = EnumeratedValueRange::new([
        Row { key: 2, id: 0 },
        Row { key: 1, id: 1 },
        Row { key: 2, id: 2 },
    ]);
    let held = r.clone();
    let sorted = r.sorted_by(3, |a, b| a.key.cmp(&b.key)).unwrap();
    assert_eq!(
        sorted.values().iter().map(|r| r.id).collect::<Vec<_>>(),
        [1, 0, 2]
    );
    assert_eq!(held.values(), r.values());
    assert_eq!(r.values()[0].id, 0);
    let same = r.sorted_by(3, |_, _| Ordering::Equal).unwrap();
    assert_eq!(same.values(), r.values());
    let optional = OptionalValueRange::new(r)
        .unwrap()
        .sorted_present_by(4, |a, b| b.key.cmp(&a.key))
        .unwrap();
    assert_eq!(
        optional.iter().map(|v| v.map(|r| r.id)).collect::<Vec<_>>(),
        [None, Some(0), Some(2), Some(1)]
    );
}
#[test]
fn range_domains_feed_real_exact_solving_and_keep_duplicate_validation() {
    let range = CompositeValueRange::new(vec![
        Box::new(IntegerValueRange::new(-2, 3, 2).unwrap()) as Box<dyn ValueRange<Value = i64>>,
        Box::new(EnumeratedValueRange::new([7])),
    ])
    .unwrap();
    let p = FiniteProblem::new(vec![
        variable_from_range(&range, 4).unwrap(),
        variable_from_range(&IntegerValueRange::new(0, 3, 1).unwrap(), 3).unwrap(),
    ])
    .unwrap();
    let result = exhaustive_search(
        &p,
        |a| SimpleScore::new(a.values().iter().flatten().sum()),
        100,
    );
    let best = result.best.unwrap();
    assert_eq!(best.assignment.values(), &[Some(7), Some(2)]);
    assert_eq!(best.score, SimpleScore::new(9));
    assert_eq!(result.evaluations, 12);
    let duplicates = EnumeratedValueRange::new([1, 1]);
    assert!(matches!(
        FiniteProblem::new(vec![variable_from_range(&duplicates, 2).unwrap()]),
        Err(ModelError::DuplicateValue { variable: 0 })
    ));
}
