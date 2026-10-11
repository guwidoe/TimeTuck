//! Original exact-rank, failure, occurrence identity and real provider regressions.
use std::{cell::Cell, sync::Arc};
use timetuck::*;
struct Groups(Vec<SelectionGroup<usize>>);
impl<C> SelectionSource<C, SelectionGroup<usize>> for Groups {
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.0.len())
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = SelectionGroup<usize>> + 'a>
    where
        SelectionGroup<usize>: 'a,
    {
        Box::new(self.0.iter().cloned())
    }
}
#[test]
fn exact_rank_stream_matches_removal_reference_across_cutoff_and_large_groups() {
    for n in [65, 129, 4096] {
        for k in [1, 32, 63, 64, 65, 128, 2047, 2048]
            .into_iter()
            .filter(|k| *k < n)
        {
            for pattern in 0..3 {
                let group: SelectionGroup<_> = (0..n).map(Arc::new).collect();
                let mut source = SubgroupSelectionSource::new(
                    Groups(vec![group.clone()]),
                    1,
                    n,
                    SubPillarSelection::combinations(k, k).unwrap(),
                    17,
                );
                let mut script = vec![];
                let mut expected = vec![];
                let mut state = 19u64;
                for _ in 0..3 {
                    script.extend([(1, 0), (1, 0)]);
                    let mut remaining = (0..n).collect::<Vec<_>>();
                    let mut picked = vec![];
                    for _ in 0..k {
                        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                        let rank = match pattern {
                            0 => 0,
                            1 => remaining.len() - 1,
                            _ => (state as usize) % remaining.len(),
                        };
                        script.push((remaining.len(), rank));
                        picked.push(remaining.remove(rank));
                    }
                    picked.sort();
                    expected.push(picked);
                }
                let mut draws = script.into_iter();
                let mut cursor = source
                    .try_open_with(&(), |bound| {
                        let (wanted, rank) = draws.next().unwrap();
                        assert_eq!(bound, wanted);
                        rank
                    })
                    .unwrap();
                for wanted in expected {
                    let actual = cursor.next().unwrap().unwrap();
                    assert_eq!(actual.iter().map(|v| **v).collect::<Vec<_>>(), wanted);
                    for (v, id) in actual.iter().zip(wanted) {
                        assert!(Arc::ptr_eq(v, &group[id]));
                    }
                }
                drop(cursor);
                assert!(draws.next().is_none());
            }
        }
    }
}
#[test]
fn invalid_late_rank_fuses_and_panicking_draw_retains_snapshot_for_retry() {
    let group: SelectionGroup<_> = (0..4096).map(Arc::new).collect();
    let mut source = SubgroupSelectionSource::new(
        Groups(vec![group.clone()]),
        1,
        4096,
        SubPillarSelection::combinations(2048, 2048).unwrap(),
        0,
    );
    let calls = Cell::new(0);
    let mut invalid = source
        .try_open_with(&(), |bound| {
            calls.set(calls.get() + 1);
            if calls.get() == 50 { bound } else { 0 }
        })
        .unwrap();
    assert_eq!(
        invalid.next().unwrap().unwrap_err(),
        SubgroupSelectionError::InvalidDraw {
            bound: 4049,
            actual: 4049
        }
    );
    assert!(invalid.next().is_none());
    assert_eq!(calls.get(), 50);
    calls.set(0);
    let panic = Cell::new(true);
    let mut cursor = source
        .try_open_with(&(), |_| {
            calls.set(calls.get() + 1);
            assert!(!panic.get() || calls.get() != 50, "late rank panic");
            0
        })
        .unwrap();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cursor.next())).is_err());
    source.event(SelectionLifecycleEvent::PhaseEnded, &());
    drop(source);
    panic.set(false);
    calls.set(0);
    let result = cursor.next().unwrap().unwrap();
    assert_eq!(calls.get(), 2050);
    assert_eq!(
        result.iter().map(|v| **v).collect::<Vec<_>>(),
        (0..2048).collect::<Vec<_>>()
    );
    for (a, b) in result.iter().zip(group.iter()) {
        assert!(Arc::ptr_eq(a, b));
    }
}
struct Variables;
impl SelectionSource<Assignment<i32>, usize> for Variables {
    fn size(&mut self, a: &Assignment<i32>) -> Option<usize> {
        Some(a.values().len())
    }
    fn open<'a>(&'a mut self, a: &'a Assignment<i32>) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        Box::new(0..a.values().len())
    }
}
#[test]
fn bounded_subgroup_provider_applies_current_assignment_composites() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 4096]).unwrap();
    let mut network = ConstraintNetwork::new(4096);
    network
        .add_constraint("sum", (0..4096).collect(), |values| {
            SimpleScore::new(values.iter().map(|v| i64::from(v.copied().unwrap())).sum())
        })
        .unwrap();
    let grouped = GroupedSelectionSource::new(
        Variables,
        4096,
        |a: &Assignment<i32>, l: &usize, r: &usize| a.values()[*l] == a.values()[*r],
    );
    let subgroup = SubgroupSelectionSource::new(
        grouped,
        4096,
        4096,
        SubPillarSelection::combinations(2048, 2048).unwrap(),
        17,
    );
    let source = LimitedSelectionSource::new(subgroup, 32);
    let observed = Cell::new(0);
    let mut provider = CachedMoveProvider::uniform(
        source,
        CachedProviderConfig::default(),
        |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| a.clone(),
        |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64, g: &SelectionGroup<usize>| {
            assert_eq!(g.len(), 2048);
            assert!(g.windows(2).all(|w| w[0] < w[1]));
            observed.set(observed.get() + 1);
            (a.values()[*g[0]] == Some(0)).then(|| {
                Move::Composite(
                    g.iter()
                        .map(|v| Move::Change {
                            variable: **v,
                            value: Some(1),
                        })
                        .collect(),
                )
            })
        },
    )
    .unwrap();
    let result = local_search_from_with_provider(
        &p,
        &network,
        p.initial_assignment(),
        LocalSearchConfig {
            max_steps: 4,
            selection: MoveSelection::BestAccepted,
            acceptance: Acceptance::HillClimbing,
            assert_scores: true,
            ..Default::default()
        },
        &mut provider,
    )
    .unwrap();
    let best = result.best.unwrap();
    assert_eq!(best.assignment.values(), &[Some(1); 4096]);
    assert_eq!(best.score, SimpleScore::new(4096));
    assert_eq!(observed.get(), 96);
}
