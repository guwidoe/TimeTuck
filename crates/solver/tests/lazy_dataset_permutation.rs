// Original native regression coverage.
use std::{cell::Cell, num::NonZeroUsize, rc::Rc};
use timetuck::*;
#[derive(Debug)]
struct Counted {
    id: usize,
    clones: Rc<Cell<usize>>,
}
impl Clone for Counted {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            id: self.id,
            clones: self.clones.clone(),
        }
    }
}
impl PartialEq for Counted {
    fn eq(&self, o: &Self) -> bool {
        self.id == o.id
    }
}
impl Eq for Counted {}
#[test]
fn tiny_samples_and_prefixes_do_not_copy_or_shuffle_the_whole_dataset() {
    let clones = Rc::new(Cell::new(0));
    let d = NeighborhoodDataset::new((0..100_000).map(|id| Counted {
        id,
        clones: clones.clone(),
    }));
    let mut random = SampleRandom::new(3);
    let mut order = d.lazy_exhaustive_iter(&mut random);
    assert_eq!(order.stored_indices(), 0);
    assert_eq!(order.len(), 100_000);
    assert_eq!(clones.get(), 0);
    for visited in 1..=5 {
        order.next().unwrap();
        assert!(order.stored_indices() <= visited);
        assert_eq!(clones.get(), visited);
    }
    let mut samples = d.samples(SizeSampler::exactly(1).unwrap(), 19);
    for _ in 0..20 {
        assert_eq!(samples.next().unwrap().unwrap().len(), 1);
    }
    assert_eq!(clones.get(), 25);
}
#[test]
fn full_permutations_preserve_occurrences_replay_and_snapshot_ownership() {
    for n in [0, 1, 2, 17, 100] {
        let d = NeighborhoodDataset::new((0..n).map(|i| i / 2));
        let draw = || {
            d.lazy_exhaustive_iter(&mut SampleRandom::new(7))
                .collect::<Vec<_>>()
        };
        let mut values = draw();
        assert_eq!(values, draw());
        values.sort();
        assert_eq!(values, (0..n).map(|i| i / 2).collect::<Vec<_>>());
        let mut rng = SampleRandom::new(17);
        let mut other = rng.clone();
        let mut a = d.lazy_exhaustive_iter(&mut rng);
        let b = d.lazy_exhaustive_iter(&mut other);
        a.next();
        drop(b);
        assert_eq!(
            rng.index(NonZeroUsize::new(1_000_000).unwrap()),
            other.index(NonZeroUsize::new(1_000_000).unwrap())
        );
    }
    let mut store = NeighborhoodDatasetStore::new();
    store.apply(TupleChange::put(0, 10));
    store.apply(TupleChange::put(1, 20));
    let old = store.snapshot();
    let mut order = old.lazy_exhaustive_iter(&mut SampleRandom::new(0));
    drop(old);
    store.apply(TupleChange::put(0, 99));
    let mut held = order.by_ref().collect::<Vec<_>>();
    held.sort();
    assert_eq!(held, [10, 20]);
    assert_eq!(order.len(), 0);
    assert_eq!(order.next(), None);
    assert!(store.snapshot().iter().any(|v| *v == 99));
}
#[test]
fn keyed_cached_and_uncached_samples_preserve_filters_minimums_and_membership() {
    let left = NeighborhoodDataset::new([0, 1]);
    let right = NeighborhoodDataset::new(0..50);
    let join = left.join(&right, |a, b| b % 2 == *a);
    let cached = join.cache();
    for key in [0, 1] {
        for sample in join
            .samples_for(key, PillarSampler(SizeSampler::exactly(4).unwrap()), 99)
            .take(100)
        {
            let sample = sample.unwrap();
            assert_eq!(sample.len(), 4);
            assert!(sample.members().iter().all(|v| v % 2 == key));
        }
        for sample in cached
            .samples_for(key, PillarSampler(SizeSampler::exactly(4).unwrap()), 99)
            .take(100)
        {
            let sample = sample.unwrap();
            assert_eq!(sample.len(), 4);
            assert!(sample.members().iter().all(|v| v % 2 == key));
        }
    }
    assert!(
        join.samples_for(2, PillarSampler(SizeSampler::exactly(1).unwrap()), 0)
            .next()
            .is_none()
    );
    assert!(
        cached
            .samples_for(0, PillarSampler(SizeSampler::exactly(26).unwrap()), 0)
            .next()
            .is_none()
    );
}

#[test]
fn draining_rules_and_large_minimums_keep_the_eager_order() {
    let dataset = NeighborhoodDataset::new(0..100);
    for mut rule in [SizeSampler::all(), SizeSampler::exactly(100).unwrap()] {
        let mut random = SampleRandom::new(12);
        let expected =
            assemble_sample(dataset.exhaustive_iter(&mut random), &mut rule, &mut random)
                .unwrap()
                .unwrap();
        let actual = dataset.samples(rule, 12).next().unwrap().unwrap();
        assert_eq!(actual.members(), expected.members());
    }
    let keys = NeighborhoodDataset::new([0]);
    let join = keys.join(&dataset, |_, _| true);
    let expected = dataset
        .exhaustive_iter(&mut SampleRandom::new(12))
        .collect::<Vec<_>>();
    assert_eq!(
        join.samples_for(0, PillarSampler(SizeSampler::all()), 12)
            .next()
            .unwrap()
            .unwrap()
            .members(),
        expected
    );
    assert_eq!(
        join.cache()
            .samples_for(0, PillarSampler(SizeSampler::all()), 12)
            .next()
            .unwrap()
            .unwrap()
            .members(),
        expected
    );
}
