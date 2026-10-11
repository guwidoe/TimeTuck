// Adapted from pinned Community CartesianProductMoveSelectorTest, SampleTest,
// DatasetTest and ScalingSequencedSetTest; Apache-2.0. See compat/provenance.json.
use std::{
    cell::RefCell,
    collections::{BTreeSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    num::NonZeroUsize,
};
use timetuck::*;
fn hash(value: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}
struct Catalog<T> {
    members: Vec<T>,
    events: RefCell<Vec<SelectionLifecycleEvent>>,
}
impl<T> Catalog<T> {
    fn new(members: impl IntoIterator<Item = T>) -> Self {
        Self {
            members: members.into_iter().collect(),
            events: RefCell::new(vec![]),
        }
    }
    fn event(&self, event: SelectionLifecycleEvent) {
        self.events.borrow_mut().push(event);
    }
}
#[derive(Debug)]
struct Change {
    entity: &'static str,
    value: i32,
}
fn random_mimic(ignore_empty: bool) {
    let entities = Catalog::new(["a", "b"]);
    let primary = Catalog::new([1, 2, 3]);
    let secondary = Catalog::new([8, 9]);
    let mut source = StreamingSelection::new(
        2,
        ignore_empty,
        NonZeroUsize::new(1).unwrap(),
        |_: &(), child: usize, prefix: &DependentPrefix<Change>| {
            let changes: Vec<_> = if child == 0 {
                entities
                    .members
                    .iter()
                    .flat_map(|&entity| {
                        primary
                            .members
                            .iter()
                            .map(move |&value| Change { entity, value })
                    })
                    .collect()
            } else {
                let entity = prefix[0].as_ref().unwrap().entity;
                secondary
                    .members
                    .iter()
                    .map(|&value| Change { entity, value })
                    .collect()
            };
            changes.into_iter()
        },
        |event, _: &()| {
            entities.event(event);
            primary.event(event);
            secondary.event(event);
        },
    )
    .with_child_catalog_bounds([Some(6), Some(4)])
    .unwrap();
    let events = [
        SelectionLifecycleEvent::SolveStarted,
        SelectionLifecycleEvent::PhaseStarted,
        SelectionLifecycleEvent::StepStarted,
        SelectionLifecycleEvent::StepEnded,
        SelectionLifecycleEvent::PhaseEnded,
        SelectionLifecycleEvent::SolveEnded,
    ];
    for &event in &events[..3] {
        source.event(event, &());
    }
    assert!(source.is_never_ending());
    // A declaration bound, not the number of emitted or feasible combinations.
    assert_eq!(source.catalog_upper_bound(), Some(24));
    assert_eq!(source.size(&()), None);
    let mut cursor = source.open(&()).peekable();
    let actual = cursor
        .by_ref()
        .take(6)
        .map(|row| {
            row.iter()
                .map(|v| {
                    let v = v.as_ref().unwrap();
                    format!("{}->{}", v.entity, v.value)
                })
                .collect::<Vec<_>>()
                .join("+")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        [
            "a->1+a->8",
            "a->2+a->9",
            "a->3+a->8",
            "b->1+a->9",
            "b->2+b->8",
            "b->3+b->9"
        ]
    );
    assert!(cursor.peek().is_some());
    drop(cursor);
    for &event in &events[3..] {
        source.event(event, &());
    }
    for actual in [&entities.events, &primary.events, &secondary.events] {
        assert_eq!(*actual.borrow(), events);
    }
}
#[test]
fn random_mimic_not_ignoring_empty() {
    random_mimic(false);
}
#[test]
fn random_mimic_ignoring_empty() {
    random_mimic(true);
}
#[test]
fn wrap_adopts_instead_of_copying() {
    let set = ScalingSequencedSet::new();
    set.extend(["a", "b"]).unwrap();
    let sample = NeighborhoodSampleView::from_set(&set);
    set.insert("c").unwrap();
    assert_eq!(sample.len(), 3);
    assert!(sample.contains(&"c"));
}
#[test]
fn wrap_accepts_an_array_backed_list() {
    let members = ["c", "a", "b"];
    let wrapped = NeighborhoodSampleView::from_slice(&members);
    let built = NeighborhoodSample::new(["a", "b", "c"]);
    assert!(wrapped == built);
    assert!(built == wrapped);
    assert_eq!(hash(&wrapped), hash(&built));
}
#[test]
fn wrap_adopts_a_scaling_ordered_set_instead_of_copying_it() {
    let set = ScalingSequencedSet::try_with_expected_size(2).unwrap();
    set.extend(["a", "b"]).unwrap();
    let sample = NeighborhoodSampleView::from_set(&set);
    set.insert("c").unwrap();
    assert_eq!(sample.len(), 3);
    assert!(sample.contains(&"c"));
    assert_eq!(*sample.representative().unwrap(), "a");
}
#[test]
fn as_cached_dataset_uni_random_iterator_never_ends_and_can_repeat() {
    // The source's generated two initialized entities, with a two-value range.
    // Identity-indexed native cached storage replaces the Java session/factory.
    let values = [0, 1];
    let entities = [(0, Some(values[0])), (1, Some(values[1]))];
    assert_eq!(values.len(), 2);
    let mut store = NeighborhoodDatasetStore::new();
    store.apply_batch(entities.into_iter().map(|e| TupleChange::put(e.0, e)));
    let dataset = store.snapshot();
    let mut random = SampleRandom::new(0);
    let mut cursor = dataset.random_iter(&mut random);
    let mut draws = vec![];
    for _ in 0..100 {
        assert!(cursor.has_next());
        draws.push(cursor.next().unwrap());
    }
    assert!(draws.iter().any(|e| entities.contains(e)));
    assert!(draws.iter().all(|e| entities.contains(e)));
    assert!(draws.iter().collect::<BTreeSet<_>>().len() < draws.len());
    assert!(cursor.has_next());
    assert_eq!(
        cursor.try_for_each_remaining(|_| {}),
        Err(UnboundedDatasetIteration)
    );
}
#[test]
fn expected_size_constructor_rejects_zero_and_negative() {
    for expected in [0, -1] {
        let error = ScalingSequencedSet::<String>::try_with_expected_size(expected)
            .err()
            .unwrap();
        assert_eq!(error, InvalidSequencedSetSize(expected));
        assert!(
            error
                .to_string()
                .contains(&format!("The expectedSize ({expected})"))
        );
    }
}
