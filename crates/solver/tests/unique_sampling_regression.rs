//! Original mathematical, large-range and solver-provider regressions.
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn scripts(bound: u128, prefix: &mut Vec<u128>, out: &mut Vec<Vec<u128>>) {
    if bound <= 1 {
        out.push(prefix.clone());
        return;
    }
    for i in 0..bound {
        prefix.push(i);
        scripts(bound - 1, prefix, out);
        prefix.pop();
    }
}
#[test]
fn every_small_draw_path_has_a_distinct_complete_permutation() {
    for length in 0..=6 {
        let mut paths = vec![];
        scripts(length, &mut vec![], &mut paths);
        let count = paths.len();
        let mut permutations = BTreeSet::new();
        for path in paths {
            let mut draws = path.into_iter();
            let mut q = UniqueIndices::with_draw(length, |bound| {
                let i = draws
                    .next()
                    .expect("one draw per nontrivial remaining pool");
                assert!(i < bound);
                i
            });
            let permutation = q.by_ref().collect::<Result<Vec<_>, _>>().unwrap();
            assert_eq!(permutation.len() as u128, length);
            assert_eq!(
                permutation.iter().copied().collect::<BTreeSet<_>>(),
                (0..length).collect()
            );
            assert_eq!(q.remaining(), 0);
            assert_eq!(q.stored_indices(), 0);
            assert!(!q.has_next());
            assert_eq!(q.next(), None);
            assert!(draws.next().is_none());
            assert!(permutations.insert(permutation));
        }
        assert_eq!(permutations.len(), count);
    }
}
#[test]
fn u128_counts_use_storage_proportional_to_draws_and_never_expand_the_range() {
    let mut q = UniqueIndices::with_draw(u128::MAX, |_| 0);
    assert_eq!(q.size_hint(), (1, None));
    assert_eq!(q.next(), Some(Ok(0)));
    for i in 1..1000 {
        assert_eq!(q.next(), Some(Ok(u128::MAX - i)));
        assert!(q.stored_indices() <= 1);
        assert_eq!(q.remaining(), u128::MAX - i - 1);
    }
    for seed in [0, 1, u64::MAX] {
        let mut q = unique_indices(u128::MAX, seed);
        let mut seen = BTreeSet::new();
        for count in 1..=100 {
            let i = q.next().unwrap().unwrap();
            assert!(i < u128::MAX);
            assert!(seen.insert(i));
            assert!(q.stored_indices() <= count);
        }
    }
}
#[test]
fn empty_singleton_passive_probes_and_bad_draws_obey_iterator_contracts() {
    let calls = Cell::new(0);
    for size in [0, 1] {
        let mut q = UniqueIndices::with_draw(size, |_| {
            calls.set(calls.get() + 1);
            panic!("unneeded draw")
        });
        for _ in 0..10 {
            assert_eq!(q.has_next(), size == 1);
            assert_eq!(q.remaining(), size);
        }
        assert_eq!(q.next(), (size == 1).then_some(Ok(0)));
        assert_eq!(q.next(), None);
        assert_eq!(q.size_hint(), (0, Some(0)));
    }
    assert_eq!(calls.get(), 0);
    let mut q = UniqueIndices::with_draw(3, |bound| {
        calls.set(calls.get() + 1);
        bound
    });
    assert_eq!(q.size_hint(), (1, Some(3)));
    assert_eq!(
        q.next(),
        Some(Err(UniqueSampleError::InvalidDraw { index: 3, bound: 3 }))
    );
    assert_eq!((q.remaining(), q.stored_indices()), (0, 0));
    assert_eq!(q.next(), None);
    assert_eq!(calls.get(), 1);
}
#[test]
fn panicking_draw_can_resume_without_losing_or_repeating_an_occurrence() {
    let first = Cell::new(true);
    let mut q = UniqueIndices::with_draw(3, |_| {
        assert!(!first.replace(false), "draw panic");
        0
    });
    assert!(catch_unwind(AssertUnwindSafe(|| q.next())).is_err());
    assert_eq!((q.remaining(), q.stored_indices()), (3, 0));
    assert_eq!(q.collect::<Result<Vec<_>, _>>().unwrap(), [0, 2, 1]);
}
#[test]
fn slice_samples_borrow_nonclone_duplicate_occurrences_and_repeat_seeded_order() {
    struct Token(u8);
    let values = [Token(7), Token(7), Token(9)];
    let sampled = unique_slice_samples(&values, 10)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let addresses = sampled
        .iter()
        .map(|&v| v as *const Token as usize)
        .collect::<BTreeSet<_>>();
    assert_eq!(addresses.len(), 3);
    assert_eq!(sampled.iter().map(|t| t.0 as usize).sum::<usize>(), 23);
    assert_eq!(
        addresses,
        values.iter().map(|v| v as *const Token as usize).collect()
    );
    for seed in 0..20 {
        let first = unique_indices(80, seed)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            first,
            unique_indices(80, seed)
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        );
    }
    let range = EnumeratedValueRange::new(vec![Some(1), None, Some(1)]);
    let mut values = unique_range_samples(&range, 0)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    values.sort();
    assert_eq!(values, [None, Some(1), Some(1)]);
}
struct HugeRange {
    length: u128,
    reads: RefCell<Vec<u128>>,
    missing: Option<u128>,
}
impl ValueRange for HugeRange {
    type Value = u128;
    fn len(&self) -> u128 {
        self.length
    }
    fn get(&self, index: u128) -> Option<u128> {
        self.reads.borrow_mut().push(index);
        (index < self.length && self.missing != Some(index)).then_some(index)
    }
    fn contains(&self, value: &u128) -> bool {
        *value < self.length
    }
}
#[test]
fn indexed_ranges_fetch_only_sampled_values_and_missing_values_end_with_one_error() {
    let r = HugeRange {
        length: u128::MAX,
        reads: RefCell::new(vec![]),
        missing: None,
    };
    let dynamic: &dyn ValueRange<Value = u128> = &r;
    let mut q = UniqueRangeSamples::with_draw(dynamic, |bound| bound - 1);
    assert!(r.reads.borrow().is_empty());
    assert!(q.has_next());
    assert_eq!(q.stored_indices(), 0);
    let values = q.by_ref().take(17).collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(values, (1..=17).map(|i| u128::MAX - i).collect::<Vec<_>>());
    assert_eq!(*r.reads.borrow(), values);
    assert_eq!(q.remaining(), u128::MAX - 17);
    assert_eq!(q.stored_indices(), 0);
    let r = HugeRange {
        length: 3,
        reads: RefCell::new(vec![]),
        missing: Some(0),
    };
    let mut q = UniqueRangeSamples::with_draw(&r, |_| 0);
    assert_eq!(
        q.next(),
        Some(Err(UniqueSampleError::MissingValue { index: 0 }))
    );
    assert_eq!(q.next(), None);
    assert_eq!(q.size_hint(), (0, Some(0)));
    assert_eq!(*r.reads.borrow(), [0]);
}
struct Candidates {
    seen: RefCell<Vec<i32>>,
}
impl MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>> for Candidates {
    fn moves<'a>(
        &'a mut self,
        p: &'a FiniteProblem<i32>,
        _: &'a Assignment<i32>,
        step: u64,
    ) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
    where
        Move<i32>: 'a,
    {
        Box::new(
            unique_slice_samples(p.variables()[0].domain(), step + 77).map(move |v| {
                let value = *v.unwrap();
                self.seen.borrow_mut().push(value);
                Move::Change {
                    variable: 0,
                    value: Some(value),
                }
            }),
        )
    }
}
#[test]
fn lazy_unique_candidates_initialize_with_both_scoring_backends_without_repeated_work() {
    let p = FiniteProblem::new(vec![Variable::new((0..128).collect())]).unwrap();
    for stream in [false, true] {
        let mut provider = Candidates {
            seen: RefCell::new(vec![]),
        };
        let config = ProviderConstructionConfig {
            max_steps: Some(1),
            ..Default::default()
        };
        let r = if stream {
            let mut b = StreamGraphBuilder::new();
            let s = b.source::<usize, (usize, Option<i32>)>();
            b.score("value", s.port(), |v| {
                SimpleScore::new(i64::from(v.1.unwrap_or(0)))
            })
            .unwrap();
            stream_construction_from_with_provider(
                &p,
                b.build(),
                s,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            let mut n = ConstraintNetwork::new(1);
            n.add_constraint("value", vec![0], |v| {
                SimpleScore::new(i64::from(v[0].copied().unwrap_or(0)))
            })
            .unwrap();
            construction_from_with_provider(&p, &n, p.initial_assignment(), config, &mut provider)
                .unwrap()
        };
        assert!(r.complete);
        assert_eq!(r.score, Some(SimpleScore::new(127)));
        assert_eq!(r.assignment.values(), [Some(127)]);
        assert_eq!(r.evaluations, 129);
        assert_eq!(r.scanned_moves, 128);
        let mut seen = provider.seen.into_inner();
        seen.sort();
        assert_eq!(seen, (0..128).collect::<Vec<_>>());
    }
}
