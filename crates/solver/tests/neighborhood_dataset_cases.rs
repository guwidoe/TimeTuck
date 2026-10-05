// Adapted from pinned Community SamplingIteratorTest, DatasetTest and
// PillarDatasetTest, Apache-2.0. See compat/provenance.json and notices.
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
};
use timetuck::*;
fn sample<T: Eq + Clone>(d: &NeighborhoodDataset<T>) -> NeighborhoodSample<T> {
    d.samples(SizeSampler::all(), 0).next().unwrap().unwrap()
}
#[test]
fn sampling_iterator_drains_source_when_sampler_never_stops() {
    let d = NeighborhoodDataset::new(0..5);
    let mut it = d.samples(SizeSampler::all(), 0).peekable();
    for _ in 0..2 {
        assert!(it.peek().is_some());
        let s = it.next().unwrap().unwrap();
        assert_eq!(s.len(), 5);
        assert!((0..5).all(|i| s.contains(&i)));
    }
}
#[test]
fn empty_dataset_has_no_next_sample() {
    assert!(
        NeighborhoodDataset::<usize>::default()
            .samples(SizeSampler::all(), 0)
            .next()
            .is_none()
    );
}
struct Recorder<'a> {
    resets: &'a Cell<usize>,
    calls: &'a RefCell<Vec<usize>>,
}
impl Sampler<usize> for Recorder<'_> {
    fn reset(&mut self, _: &mut SampleRandom) {
        self.resets.set(self.resets.get() + 1);
    }
    fn evaluate(&mut self, n: usize, _: &usize) -> SampleDecision {
        self.calls.borrow_mut().push(n);
        if n + 1 >= 2 {
            SampleDecision::AcceptAndStop
        } else {
            SampleDecision::Accept
        }
    }
}
#[test]
fn sampler_reset_and_decide_are_called_correctly() {
    let d = NeighborhoodDataset::new(0..3);
    let resets = Cell::new(0);
    let calls = RefCell::new(vec![]);
    let mut it = d
        .samples(
            Recorder {
                resets: &resets,
                calls: &calls,
            },
            0,
        )
        .peekable();
    assert!(it.peek().is_some());
    assert!(it.next().unwrap().is_ok());
    assert_eq!(resets.get(), 1);
    assert!(!calls.borrow().is_empty());
    assert_eq!(calls.borrow()[0], 0);
}
#[test]
fn sample_contains_no_duplicates() {
    let s = sample(&NeighborhoodDataset::new(0..5));
    let seen = s.into_iter().collect::<Vec<_>>();
    assert_eq!(seen.len(), seen.iter().collect::<BTreeSet<_>>().len());
}
#[test]
fn sample_can_contain_and_check_for_nullability() {
    assert!(!sample(&NeighborhoodDataset::new((0..3).map(Some))).contains(&None));
}
struct AtLeastTwo;
impl<T> Sampler<T> for AtLeastTwo {
    fn minimum_size(&self) -> usize {
        2
    }
    fn evaluate(&mut self, _: usize, _: &T) -> SampleDecision {
        SampleDecision::Accept
    }
}
#[test]
fn sampler_minimum_size_is_checked_after_deduplication() {
    assert!(
        assemble_sample(
            [String::from("only"), String::from("only")],
            &mut AtLeastTwo,
            &mut SampleRandom::new(0)
        )
        .unwrap()
        .is_none()
    );
}
#[test]
fn duplicate_candidate_does_not_end_sample_below_minimum_size() {
    let s = assemble_sample(
        ["a", "a", "b"],
        &mut SizeSampler::exactly(2).unwrap(),
        &mut SampleRandom::new(0),
    )
    .unwrap()
    .unwrap();
    assert_eq!(s.len(), 2);
    assert!(s.contains(&"a") && s.contains(&"b"));
}
fn equal_prefix(a: &&str, b: &&str) -> bool {
    *a == b.split('-').next().unwrap()
}
fn slice()
-> NeighborhoodJoin<&'static str, &'static str, impl Fn(&&'static str, &&'static str) -> bool> {
    NeighborhoodDataset::new(["probe"]).join(
        &NeighborhoodDataset::new(["probe-1", "probe-2", "other-1"]),
        equal_prefix,
    )
}
#[test]
fn bi_sampling_iterator_empty_slice_has_no_next_sample() {
    let j = NeighborhoodDataset::new(["probe"])
        .join(&NeighborhoodDataset::new(["other-value"]), equal_prefix);
    assert!(
        j.samples_for("probe", PillarSampler(SizeSampler::all()), 0)
            .next()
            .is_none()
    );
}
#[test]
fn bi_sampling_iterator_drains_slice_when_sampler_never_stops() {
    let j = slice();
    let mut it = j
        .samples_for("probe", PillarSampler(SizeSampler::all()), 0)
        .peekable();
    for _ in 0..2 {
        assert!(it.peek().is_some());
        let s = it.next().unwrap().unwrap();
        assert_eq!(s.len(), 2);
        assert!(s.contains(&"probe-1") && s.contains(&"probe-2"));
    }
}
#[test]
fn bi_sampling_iterator_reaches_all_matching_members_with_filtered_join() {
    let values = NeighborhoodDataset::new(
        (0..30)
            .map(|i| format!("probe-match-{i}"))
            .chain((0..70).map(|i| format!("other-{i}"))),
    );
    let j = NeighborhoodDataset::new(["probe"]).join(&values, |a, b| {
        *a != "probe" || b.starts_with("probe-match-")
    });
    let mut it = j
        .samples_for("probe", PillarSampler(SizeSampler::all()), 0)
        .peekable();
    assert!(it.peek().is_some());
    assert_eq!(it.next().unwrap().unwrap().len(), 30);
}
#[test]
fn bi_sampling_iterator_drains_cached_dataset_slice_when_sampler_never_stops() {
    let j = slice().cache();
    let mut it = j
        .samples_for("probe", PillarSampler(SizeSampler::all()), 0)
        .peekable();
    assert!(it.peek().is_some());
    let s = it.next().unwrap().unwrap();
    assert_eq!(s.len(), 2);
    assert!(s.contains(&"probe-1") && s.contains(&"probe-2"));
}
struct Keys<'a>(&'a RefCell<Vec<&'static str>>);
impl Sampler<&'static str> for Keys<'_> {
    fn evaluate(&mut self, _: usize, _: &&'static str) -> SampleDecision {
        SampleDecision::Accept
    }
}
impl KeyedSampler<&'static str, &'static str> for Keys<'_> {
    fn reset_for_key(&mut self, _: &mut SampleRandom, key: &&'static str) {
        self.0.borrow_mut().push(*key);
    }
}
#[test]
fn bi_sampling_iterator_pillar_sampler_receives_the_slice_selector_as_key() {
    let j = NeighborhoodDataset::new(["probe"])
        .join(&NeighborhoodDataset::new(["probe-1"]), equal_prefix);
    let keys = RefCell::new(vec![]);
    let mut it = j.samples_for("probe", Keys(&keys), 0).peekable();
    assert!(it.peek().is_some());
    it.next().unwrap().unwrap();
    assert_eq!(*keys.borrow(), vec!["probe"]);
}
#[test]
fn bi_sampling_iterator_ends_when_sampler_refuses_below_minimum_size() {
    let j = NeighborhoodDataset::new(["probe"]).join(
        &NeighborhoodDataset::new(["probe-1", "probe-2"]),
        equal_prefix,
    );
    assert!(
        j.samples_for("probe", PillarSampler(SizeSampler::exactly(3).unwrap()), 0)
            .next()
            .is_none()
    );
}
#[test]
fn as_cached_dataset_uni_size_and_iterators_visit_every_element_exactly_once() {
    let mut store = NeighborhoodDatasetStore::new();
    store.apply_batch((0..3).map(|i| TupleChange::put(i, (i, i % 2))));
    let d = store.snapshot();
    assert_eq!(d.len(), 3);
    let seen = d
        .exhaustive_iter(&mut SampleRandom::new(0))
        .collect::<BTreeSet<_>>();
    assert_eq!(seen, BTreeSet::from([(0, 0), (1, 1), (2, 0)]));
}
#[test]
fn as_cached_dataset_same_stream_twice_shares_one_dataset() {
    let mut store = NeighborhoodDatasetStore::new();
    store.apply(TupleChange::put(0, 3));
    let first = store.snapshot();
    assert!(first.same_snapshot(&store.snapshot()));
}
#[test]
fn cached_and_just_in_time_join_agree_on_every_row() {
    let entities = NeighborhoodDataset::new((0..2).map(|i| (i, i % 4)));
    let values = NeighborhoodDataset::new(0..4);
    let j = entities.join(&values, |a, b| a.1 == *b);
    let c = j.cache();
    assert_eq!(c.len(), j.len());
    assert_eq!(
        c.rows().iter().copied().collect::<BTreeSet<_>>(),
        j.iter().map(|(a, b)| (*a, *b)).collect()
    );
    for a in entities.iter() {
        assert_eq!(
            c.slice(a).iter().copied().collect::<BTreeSet<_>>(),
            j.iter().filter(|(l, _)| *l == a).map(|(_, b)| *b).collect()
        );
    }
}
#[test]
fn cached_join_per_a_lookup_on_skewed_buckets_always_finds_its_match() {
    let mut entities = NeighborhoodDatasetStore::new();
    entities.apply_batch([TupleChange::put(0, "rare"), TupleChange::put(1, "common")]);
    let mut values = NeighborhoodDatasetStore::new();
    values.apply(TupleChange::put(0, (0, "rare")));
    values.apply_batch((1..100).map(|i| TupleChange::put(i, (i, "common"))));
    let join = |a: &&str, b: &(usize, &str)| *a == b.1;
    let mut c = entities.snapshot().join(&values.snapshot(), join).cache();
    let mut r = SampleRandom::new(0);
    for _ in 0..200 {
        assert!(c.slice(&"rare").random_iter(&mut r).next().is_some());
    }
    let mut seen = BTreeSet::new();
    for _ in 0..2000 {
        seen.insert(c.slice(&"common").random_iter(&mut r).next().unwrap());
    }
    assert_eq!(seen.len(), 99);
    entities.clear();
    values.clear();
    c.refresh(&entities.snapshot().join(&values.snapshot(), join));
    assert_eq!(c.slice(&"rare").len(), 0);
    assert!(c.slice(&"rare").random_iter(&mut r).next().is_none());
}
#[test]
fn cached_join_less_than_produces_pairs_ordered_left_before_right() {
    let v = NeighborhoodDataset::new(0..3);
    let c = v.join(&v, |a, b| a < b).cache();
    assert_eq!(
        c.rows().iter().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([(0, 1), (0, 2), (1, 2)])
    );
}
#[test]
fn just_in_time_join_less_than_produces_pairs_ordered_left_before_right() {
    let v = NeighborhoodDataset::new(0..3);
    let j = v.join(&v, |a, b| a < b);
    assert_eq!(
        j.iter().map(|(a, b)| (*a, *b)).collect::<BTreeSet<_>>(),
        BTreeSet::from([(0, 1), (0, 2), (1, 2)])
    );
}
struct FinishSample;
impl CollectorFinisher<ListSnapshot<usize, usize>> for FinishSample {
    type Output = NeighborhoodSample<usize>;
    fn finish(list: ListSnapshot<usize, usize>) -> Self::Output {
        NeighborhoodSample::new(list.iter().copied())
    }
}
type PillarNode = CollectNode<
    usize,
    usize,
    (usize, usize),
    FinishingCollector<ListCollector<usize, usize>, FinishSample>,
>;
struct Pillars {
    node: PillarNode,
    store: NeighborhoodDatasetStore<usize, NeighborhoodSample<usize>>,
}
impl Pillars {
    fn new(values: &[usize]) -> Self {
        let mut p = Self {
            node: PillarNode::new(),
            store: NeighborhoodDatasetStore::new(),
        };
        for (i, &v) in values.iter().enumerate() {
            p.put(i, v);
        }
        p
    }
    fn put(&mut self, id: usize, value: usize) {
        self.store
            .apply_batch(self.node.apply(TupleChange::put(id, (value, (id, id)))));
    }
    fn rows(&mut self) -> NeighborhoodDataset<NeighborhoodSample<usize>> {
        self.store.snapshot()
    }
}
#[test]
fn one_row_per_distinct_value() {
    let mut p = Pillars::new(&[0, 0, 1]);
    let d = p.rows();
    assert_eq!(d.len(), 2);
    let two = d.iter().find(|s| s.len() == 2).unwrap();
    let one = d.iter().find(|s| s.len() == 1).unwrap();
    assert!(two.contains(&0) && two.contains(&1));
    assert!(one.contains(&2));
}
#[test]
fn pillar_held_across_a_settle_does_not_change() {
    let mut p = Pillars::new(&[0, 0, 1]);
    let d = p.rows();
    let held = d.iter().find(|s| s.len() == 2).unwrap();
    p.put(1, 1);
    assert_eq!(held.len(), 2);
    assert!(held.contains(&0) && held.contains(&1));
    let current = p.rows();
    let now = current.iter().find(|s| s.len() == 1).unwrap();
    assert!(!std::ptr::eq(now, held));
    assert!(now.contains(&0));
    assert!(!now.contains(&1));
}
#[test]
fn empty_group_produces_no_pillar_row() {
    assert!(Pillars::new(&[]).rows().is_empty());
}
#[test]
fn last_member_retraction_does_not_build_an_empty_pillar() {
    let mut p = Pillars::new(&[0]);
    assert_eq!(p.rows().len(), 1);
    p.put(0, 1);
    assert_eq!(p.rows().len(), 1);
    assert!(p.rows().iter().next().unwrap().contains(&0));
}
