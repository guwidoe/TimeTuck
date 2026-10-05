//! Original lifecycle, snapshot, atomic-update and solver integration checks.
use std::{
    cell::Cell,
    collections::BTreeSet,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[test]
fn factories_are_lazy_peek_is_cached_and_terminal_outcomes_are_fused() {
    let calls = Cell::new(0);
    let mut it = repeated_samples(
        |_| {
            calls.set(calls.get() + 1);
            0..5
        },
        SizeSampler::exactly(2).unwrap(),
        7,
    )
    .peekable();
    assert_eq!(calls.get(), 0);
    assert_eq!(it.peek().unwrap().as_ref().unwrap().len(), 2);
    assert!(it.peek().is_some());
    assert_eq!(calls.get(), 1);
    it.next().unwrap().unwrap();
    assert_eq!(calls.get(), 1);
    it.next().unwrap().unwrap();
    assert_eq!(calls.get(), 2);
    let calls = Cell::new(0);
    let mut short = repeated_samples(
        |_| {
            let n = calls.get();
            calls.set(n + 1);
            0..if n == 0 { 1 } else { 5 }
        },
        SizeSampler::exactly(2).unwrap(),
        0,
    );
    assert!(short.next().is_none());
    assert!(short.next().is_none());
    assert_eq!(calls.get(), 1);
    let calls = Cell::new(0);
    let mut bad = repeated_samples(
        |_| {
            calls.set(calls.get() + 1);
            0..3
        },
        |_, _: &usize| SampleDecision::Stop,
        0,
    );
    assert_eq!(
        bad.next().unwrap().unwrap_err(),
        SampleError::StoppedBelowMinimum {
            actual: 0,
            minimum: 1
        }
    );
    assert!(bad.next().is_none());
    assert!(bad.next().is_none());
    assert_eq!(calls.get(), 1);
}
#[test]
fn permutations_samples_and_random_draws_have_their_distinct_consumption_contracts() {
    let d = NeighborhoodDataset::new([0, 0, 1, 2, 3, 4]);
    let mut random = SampleRandom::new(19);
    let mut perm = d.exhaustive_iter(&mut random);
    assert_eq!(perm.len(), 6);
    let mut sorted = perm.by_ref().collect::<Vec<_>>();
    sorted.sort();
    assert_eq!(sorted, vec![0, 0, 1, 2, 3, 4]);
    assert!(perm.next().is_none());
    let draws = d.random_iter(&mut random).take(100).collect::<Vec<_>>();
    assert_eq!(draws.len(), 100);
    assert!(draws.iter().all(|v| d.iter().any(|a| a == v)));
    assert!(
        NeighborhoodDataset::<usize>::default()
            .random_iter(&mut random)
            .next()
            .is_none()
    );
    let run = |seed| {
        d.samples(SizeSampler::exactly(3).unwrap(), seed)
            .take(50)
            .map(|s| s.unwrap().into_iter().collect::<Vec<_>>())
            .collect::<Vec<_>>()
    };
    assert_eq!(run(42), run(42));
    assert_ne!(run(42), run(43));
    assert!(
        run(42)
            .iter()
            .map(|s| s.iter().copied().collect::<BTreeSet<_>>())
            .collect::<BTreeSet<_>>()
            .len()
            > 3
    );
    for members in run(42) {
        assert_eq!(members.len(), 3);
        assert_eq!(members.iter().collect::<BTreeSet<_>>().len(), 3);
    }
}
#[derive(Clone, Debug)]
struct Fragile {
    value: usize,
    panic: Rc<Cell<bool>>,
}
impl PartialEq for Fragile {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for Fragile {}
impl PartialOrd for Fragile {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Fragile {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        assert!(
            !(self.panic.get() && (self.value == 9 || other.value == 9)),
            "comparison failed"
        );
        self.value.cmp(&other.value)
    }
}
#[test]
fn stores_isolate_batches_and_cache_snapshots_until_actual_updates() {
    let flag = Rc::new(Cell::new(false));
    let key = |value| Fragile {
        value,
        panic: flag.clone(),
    };
    let mut store = NeighborhoodDatasetStore::new();
    store.apply(TupleChange::put(key(1), "old"));
    let saved = store.snapshot();
    store.apply(TupleChange::retract(key(8)));
    assert!(saved.same_snapshot(&store.snapshot()));
    flag.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| store.apply_batch([
            TupleChange::put(key(2), "partial"),
            TupleChange::put(key(9), "panic")
        ])))
        .is_err()
    );
    flag.set(false);
    assert_eq!(store.len(), 1);
    assert!(saved.same_snapshot(&store.snapshot()));
    assert_eq!(store.rows()[&key(1)], "old");
    store.apply_batch([
        TupleChange::put(key(1), "new"),
        TupleChange::put(key(2), "second"),
    ]);
    let current = store.snapshot();
    assert!(!saved.same_snapshot(&current));
    assert_eq!(saved.iter().copied().collect::<Vec<_>>(), vec!["old"]);
    let mut held = store.snapshot().samples(SizeSampler::all(), 0);
    store.clear();
    assert!(store.snapshot().is_empty());
    assert_eq!(held.next().unwrap().unwrap().len(), 2);
}
#[test]
fn joins_refresh_atomically_and_held_slice_iterators_remain_snapshots() {
    let left = NeighborhoodDataset::new([0, 1]);
    let right = NeighborhoodDataset::new([0, 1, 2, 3]);
    let mut c = left.join(&right, |a, b| b % 2 == *a).cache();
    let old = c.rows();
    let mut held = c.samples_for(0, PillarSampler(SizeSampler::all()), 2);
    let calls = Cell::new(0);
    let broken = left.join(&right, |a, b| {
        calls.set(calls.get() + 1);
        assert!(calls.get() < 4);
        a == b
    });
    assert!(catch_unwind(AssertUnwindSafe(|| c.refresh(&broken))).is_err());
    assert!(old.same_snapshot(&c.rows()));
    assert_eq!(c.slice(&0).iter().copied().collect::<Vec<_>>(), vec![0, 2]);
    c.refresh(&left.join(&NeighborhoodDataset::new([9]), |_, _| true));
    assert_eq!(c.slice(&0).iter().copied().collect::<Vec<_>>(), vec![9]);
    let s = held.next().unwrap().unwrap();
    assert_eq!(s.len(), 2);
    assert!(s.contains(&0) && s.contains(&2));
    assert_eq!(old.len(), 4);
    assert!(
        c.samples_for(8, PillarSampler(SizeSampler::all()), 0)
            .next()
            .is_none()
    );
    assert!(
        left.join(&right, |_, _| true)
            .samples_for(8, PillarSampler(SizeSampler::all()), 0)
            .next()
            .is_none()
    );
}
#[test]
fn samples_from_duplicate_rows_count_distinct_values_and_filtered_slices_are_exhaustive() {
    let left = NeighborhoodDataset::new([Some(0), None]);
    let right = NeighborhoodDataset::new([None, None, Some(0), Some(1), Some(2)]);
    let j = left.join(&right, |a, b| a == b);
    let c = j.cache();
    assert_eq!(c.len(), 3);
    assert!(
        j.samples_for(None, PillarSampler(SizeSampler::exactly(2).unwrap()), 0)
            .next()
            .is_none()
    );
    let sample = c
        .samples_for(None, PillarSampler(SizeSampler::all()), 0)
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(sample.members(), &[None]);
    let right = NeighborhoodDataset::new(0..1000);
    let j = NeighborhoodDataset::new([999]).join(&right, |a, b| a == b);
    for s in j
        .samples_for(999, PillarSampler(SizeSampler::exactly(1).unwrap()), 7)
        .take(20)
    {
        assert_eq!(s.unwrap().members(), &[999]);
    }
}
struct SampleProvider;
impl MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>> for SampleProvider {
    fn moves<'a>(
        &'a mut self,
        p: &'a FiniteProblem<i32>,
        a: &'a Assignment<i32>,
        step: u64,
    ) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
    where
        Move<i32>: 'a,
    {
        let d = NeighborhoodDataset::new(
            (0..p.variables().len())
                .filter(|&i| !p.variables()[i].is_pinned() && a.values()[i] == Some(0)),
        );
        Box::new(
            d.samples(SizeSampler::up_to(3).unwrap(), 77 + step)
                .take(4)
                .map(|s| {
                    Move::Composite(
                        s.unwrap()
                            .into_iter()
                            .map(|variable| Move::Change {
                                variable,
                                value: Some(1),
                            })
                            .collect(),
                    )
                }),
        )
    }
}
#[test]
fn lazy_sample_providers_reopen_from_each_solver_snapshot_in_both_backends() {
    let p = FiniteProblem::new(
        (0..7)
            .map(|i| {
                Variable::new(vec![0, 1])
                    .with_initial(0)
                    .with_pinned(i == 6)
            })
            .collect(),
    )
    .unwrap();
    for stream in [false, true] {
        let config = LocalSearchConfig {
            max_steps: 8,
            assert_scores: true,
            ..Default::default()
        };
        let r = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("sum", source.port(), |(_, v)| {
                SimpleScore::new(i64::from(v.unwrap()))
            })
            .unwrap();
            stream_local_search_from_with_provider(
                &p,
                b.build(),
                source,
                p.initial_assignment(),
                config,
                &mut SampleProvider,
            )
            .unwrap()
        } else {
            let mut n = ConstraintNetwork::new(7);
            n.add_constraint("sum", (0..7).collect(), |vs| {
                SimpleScore::new(vs.iter().map(|v| i64::from(*v.unwrap())).sum())
            })
            .unwrap();
            local_search_from_with_provider(
                &p,
                &n,
                p.initial_assignment(),
                config,
                &mut SampleProvider,
            )
            .unwrap()
        };
        let best = r.best.as_ref().unwrap();
        assert_eq!(best.score, SimpleScore::new(6));
        assert_eq!(best.assignment.values()[6], Some(0));
        assert!(r.completed_steps <= 6);
        assert!(r.move_evaluations() <= 24);
    }
}
