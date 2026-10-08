//! Native heterogeneous per-key snapshots, recovery and solver integration.
use SelectionLifecycleEvent as E;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use timetuck::*;
struct Keys {
    ids: Vec<usize>,
    opens: usize,
}
impl<C> SelectionSource<C, usize> for Keys {
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.ids.len())
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        self.opens += 1;
        Box::new(self.ids.iter().copied())
    }
}
struct Values {
    pools: Vec<Vec<i32>>,
    declared: Option<usize>,
    infinite: bool,
    opens: usize,
}
impl<C> PlacementSource<C, usize, i32> for Values {
    fn is_never_ending(&self) -> bool {
        self.infinite
    }
    fn size(&mut self, _: &C, _: &usize) -> Option<usize> {
        self.declared
    }
    fn open<'a>(&'a mut self, _: &'a C, key: &'a usize) -> Box<dyn Iterator<Item = i32> + 'a>
    where
        i32: 'a,
    {
        self.opens += 1;
        Box::new(self.pools[*key].iter().copied())
    }
}
fn values(pools: Vec<Vec<i32>>) -> Values {
    Values {
        pools,
        declared: None,
        infinite: false,
        opens: 0,
    }
}
fn limits() -> KeyedSelectionLimits {
    KeyedSelectionLimits {
        maximum_keys: 4,
        maximum_values_per_key: 4,
        maximum_values: 10,
    }
}
#[test]
fn key_major_pools_keep_duplicate_occurrences_and_survive_invalidation() {
    let mut s = KeyedSelectionSource::new(
        Keys {
            ids: vec![0, 1, 0, 2],
            opens: 0,
        },
        values(vec![vec![4, 4], vec![], vec![8]]),
        limits(),
    );
    let snap = s.try_snapshot(&()).unwrap();
    assert_eq!(snap.len(), 5);
    assert_eq!(snap.key_count(), 3);
    assert_eq!(
        snap.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(),
        [(0, 4), (0, 4), (0, 4), (0, 4), (2, 8)]
    );
    assert_eq!(s.size(&()), Some(5));
    assert_eq!(s.open(&()).count(), 5);
    assert_eq!(s.key_source().opens, 1);
    assert_eq!(s.value_source().opens, 4);
    let rows = snap.iter().collect::<Vec<_>>();
    assert!(Arc::ptr_eq(&rows[0].0, &rows[1].0));
    assert!(!Arc::ptr_eq(&rows[0].0, &rows[2].0));
    assert!(!Arc::ptr_eq(&rows[0].1, &rows[1].1));
    s.value_source_mut().pools[0] = vec![9];
    let new = s.try_snapshot(&()).unwrap();
    assert_eq!(new.len(), 3);
    s.event(E::PhaseEnded, &());
    drop(s);
    let mut iter = snap.iter();
    assert_eq!(iter.len(), 5);
    assert_eq!(iter.next().map(|(k, v)| (*k, *v)), Some((0, 4)));
    assert_eq!(iter.len(), 4);
    assert_eq!(iter.by_ref().count(), 4);
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}
#[test]
fn sampling_observes_selected_key_and_bad_draws_fuse() {
    let mut s = KeyedSelectionSource::new(
        Keys {
            ids: vec![0, 1, 2],
            opens: 0,
        },
        values(vec![vec![1], vec![], vec![2, 3, 4]]),
        limits(),
    )
    .random(7);
    let snap = s.try_snapshot(&()).unwrap();
    assert_eq!(snap.key_count(), 2);
    for index in 0..2 {
        let mut iter = snap.samples_with(|draw| match draw {
            KeyedSelectionDraw::Key { bound } => {
                assert_eq!(bound, 2);
                index
            }
            KeyedSelectionDraw::Value { key, bound } => {
                assert_eq!(*key, if index == 0 { 0 } else { 2 });
                bound - 1
            }
        });
        assert_eq!(
            iter.next().unwrap().map(|(k, v)| (*k, *v)),
            Ok(if index == 0 { (0, 1) } else { (2, 4) })
        );
    }
    for bad_key in [true, false] {
        let mut iter = snap.samples_with(|draw| match draw {
            KeyedSelectionDraw::Key { bound } => {
                if bad_key {
                    bound
                } else {
                    0
                }
            }
            KeyedSelectionDraw::Value { bound, .. } => bound,
        });
        assert!(matches!(
            iter.next(),
            Some(Err(KeyedSelectionError::InvalidDraw { .. }))
        ));
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }
    let first = s
        .open(&())
        .take(30)
        .map(|(k, v)| (*k, *v))
        .collect::<Vec<_>>();
    s.reseed(7);
    assert_eq!(
        first,
        s.open(&())
            .take(30)
            .map(|(k, v)| (*k, *v))
            .collect::<Vec<_>>()
    );
    assert_eq!(s.value_source().opens, 3);
}
#[test]
fn collection_failures_publish_nothing_and_edits_can_retry() {
    let mut s = KeyedSelectionSource::new(
        Keys {
            ids: vec![0, 1],
            opens: 0,
        },
        values(vec![vec![1, 2], vec![3, 4]]),
        KeyedSelectionLimits {
            maximum_keys: 2,
            maximum_values_per_key: 2,
            maximum_values: 3,
        },
    );
    assert!(matches!(
        s.try_snapshot(&()),
        Err(KeyedSelectionError::TotalLimit { maximum: 3 })
    ));
    s.value_source_mut().pools[1].clear();
    assert_eq!(s.try_snapshot(&()).unwrap().len(), 2);
    s.value_source_mut().declared = Some(1);
    assert!(matches!(
        s.try_snapshot(&()),
        Err(KeyedSelectionError::SizeMismatch {
            key: 0,
            declared: 1,
            actual: 2
        })
    ));
    s.value_source_mut().declared = Some(3);
    assert!(matches!(
        s.try_snapshot(&()),
        Err(KeyedSelectionError::ValuesLimit { key: 0, maximum: 2 })
    ));
    s.value_source_mut().declared = None;
    s.value_source_mut().pools[0].push(5);
    assert!(matches!(
        s.try_snapshot(&()),
        Err(KeyedSelectionError::ValuesLimit { key: 0, maximum: 2 })
    ));
    s.value_source_mut().infinite = true;
    assert!(matches!(
        s.try_snapshot(&()),
        Err(KeyedSelectionError::UnboundedValues)
    ));
    s.value_source_mut().infinite = false;
    s.key_source_mut().ids.push(0);
    assert!(matches!(
        s.try_snapshot(&()),
        Err(KeyedSelectionError::Keys(
            SelectionProductError::CandidateLimit { .. }
        ))
    ));
    s.key_source_mut().ids.clear();
    assert!(s.try_snapshot(&()).unwrap().is_empty());
    let mut bounded = LimitedSelectionSource::new(s.random(3), 20);
    assert_eq!(bounded.size(&()), Some(0));
    assert!(bounded.open(&()).next().is_none());
}
struct Payload {
    drops: Arc<AtomicUsize>,
}
impl Drop for Payload {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}
struct NonCloneKeys(Arc<AtomicUsize>);
impl SelectionSource<(), Payload> for NonCloneKeys {
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(2)
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = Payload> + 'a>
    where
        Payload: 'a,
    {
        Box::new((0..2).map(|_| Payload {
            drops: self.0.clone(),
        }))
    }
}
struct NonCloneValues(Arc<AtomicUsize>);
impl PlacementSource<(), Payload, Payload> for NonCloneValues {
    fn size(&mut self, _: &(), _: &Payload) -> Option<usize> {
        Some(2)
    }
    fn open<'a>(&'a mut self, _: &'a (), _: &'a Payload) -> Box<dyn Iterator<Item = Payload> + 'a>
    where
        Payload: 'a,
    {
        Box::new((0..2).map(|_| Payload {
            drops: self.0.clone(),
        }))
    }
}
#[test]
fn held_pairs_own_nonclone_keys_and_values_after_source_cleanup() {
    let keys = Arc::new(AtomicUsize::new(0));
    let values = Arc::new(AtomicUsize::new(0));
    let mut s = KeyedSelectionSource::new(
        NonCloneKeys(keys.clone()),
        NonCloneValues(values.clone()),
        limits(),
    );
    let snapshot = s.try_snapshot(&()).unwrap();
    let pair = snapshot.iter().next().unwrap();
    s.event(E::StepEnded, &());
    drop(s);
    drop(snapshot);
    assert_eq!(keys.load(Ordering::Relaxed), 1);
    assert_eq!(values.load(Ordering::Relaxed), 3);
    drop(pair);
    assert_eq!(keys.load(Ordering::Relaxed), 2);
    assert_eq!(values.load(Ordering::Relaxed), 4);
}
#[test]
fn phase_cached_entity_specific_domains_drive_real_changes_including_initial_noops() {
    let domains = vec![vec![0, 1], vec![0, 2], vec![0, 3]];
    let p = FiniteProblem::new(
        domains
            .iter()
            .map(|d| Variable::new(d.clone()).with_initial(0))
            .collect(),
    )
    .unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("sum", vec![0, 1, 2], |v| {
        SimpleScore::new(v.iter().map(|v| i64::from(v.copied().unwrap())).sum())
    })
    .unwrap();
    let source = KeyedSelectionSource::new(
        Keys {
            ids: vec![0, 1, 2],
            opens: 0,
        },
        values(domains),
        limits(),
    );
    let mut provider = CachedMoveProvider::uniform(
        source,
        CachedProviderConfig {
            scope: ProviderCacheScope::Phase,
            ..Default::default()
        },
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| (),
        |_: &FiniteProblem<i32>,
         _: &Assignment<i32>,
         _: u64,
         pair: &KeyedSelectionPair<usize, i32>| {
            Some(Move::Change {
                variable: *pair.0,
                value: Some(*pair.1),
            })
        },
    )
    .unwrap();
    let mut ext = SolverExtensions::default();
    ext.move_providers.bind(0, &mut provider);
    let result = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 5,
                selection: MoveSelection::BestAccepted,
                acceptance: Acceptance::HillClimbing,
                assert_scores: true,
                ..Default::default()
            })],
            ..Default::default()
        },
        ext,
    )
    .unwrap();
    assert_eq!(result.assignment.values(), &[Some(1), Some(2), Some(3)]);
    assert_eq!(result.score, Some(SimpleScore::new(6)));
    assert_eq!(provider.cache().source().key_source().opens, 1);
    assert_eq!(provider.cache().source().value_source().opens, 3);
}
