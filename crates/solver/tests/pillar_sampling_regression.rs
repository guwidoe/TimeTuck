//! Original bounded sampling and solver integration cases; no upstream credits.
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use timetuck::*;
#[test]
fn direct_sampling_covers_sizes_and_subsets_replays_and_owns_metadata() {
    let p = Pillar::new(0..4).unwrap();
    let policy = PillarSelectionPolicy::subsets(1, 3).unwrap();
    let samples = policy
        .sample(&p, 17, 9000)
        .map(|p| p.variables())
        .collect::<Vec<_>>();
    assert_eq!(
        samples,
        policy
            .sample(&p, 17, 9000)
            .map(|p| p.variables())
            .collect::<Vec<_>>()
    );
    let mut sizes = [0usize; 4];
    let mut frequency = BTreeMap::new();
    for row in samples {
        sizes[row.len()] += 1;
        *frequency.entry(row).or_insert(0usize) += 1;
    }
    assert_eq!(frequency.len(), 14);
    for (row, count) in frequency {
        assert!(row.windows(2).all(|p| p[0] < p[1]));
        let expected = if row.len() == 2 { 500 } else { 750 };
        assert!(
            (expected * 8 / 10..=expected * 12 / 10).contains(&count),
            "{row:?}: {count}"
        );
    }
    for count in &sizes[1..] {
        assert!((2700..=3300).contains(count));
    }
    let owned = {
        let p = Pillar::new(0..80).unwrap();
        PillarSelectionPolicy::subsets(40, usize::MAX)
            .unwrap()
            .sample(&p, 99, 8)
    };
    assert_eq!(
        owned
            .inspect(|p| {
                assert!((40..=80).contains(&p.rows().len()));
            })
            .count(),
        8
    );
    assert!(
        PillarSelectionPolicy::subsets(5, 10)
            .unwrap()
            .sample(&p, 0, 1)
            .next()
            .is_none()
    );
    assert_eq!(
        PillarSelectionPolicy::whole()
            .sample(&p, 0, 3)
            .collect::<Vec<_>>(),
        vec![p.clone(); 3]
    );
    assert_ne!(
        policy.sample(&p, 17, 12).collect::<Vec<_>>(),
        policy.sample(&p, 18, 12).collect::<Vec<_>>()
    );
}
#[test]
fn sequential_sampling_sorts_once_and_preserves_aligned_rows() {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let rank = [1, 3, 0, 2];
    let policy = PillarSelectionPolicy::sequential(
        2,
        2,
        Some(PillarComparator::new(move |a, b| {
            seen.fetch_add(1, Ordering::Relaxed);
            rank[a[0] / 2].cmp(&rank[b[0] / 2])
        })),
    )
    .unwrap();
    let p = Pillar::from_rows(vec![vec![0, 1], vec![2, 3], vec![4, 5], vec![6, 7]]).unwrap();
    assert!(policy.sample(&p, 0, 0).next().is_none());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    let samples = policy.sample(&p, 123, 500);
    let opening_calls = calls.load(Ordering::Relaxed);
    assert!(opening_calls > 0);
    let rows = samples.map(|p| p.rows().to_vec()).collect::<BTreeSet<_>>();
    assert_eq!(calls.load(Ordering::Relaxed), opening_calls);
    assert_eq!(
        rows,
        BTreeSet::from([
            vec![vec![0, 1], vec![4, 5]],
            vec![vec![0, 1], vec![6, 7]],
            vec![vec![2, 3], vec![6, 7]]
        ])
    );
}
fn neighborhood() -> PillarMoveProvider {
    let rank = [0, 2, 1, 3];
    PillarMoveProvider {
        left: PillarSelectionPolicy::sequential(
            2,
            2,
            Some(PillarComparator::new(move |a, b| {
                rank[a[0]].cmp(&rank[b[0]])
            })),
        )
        .unwrap(),
        changes: PillarChangeMode::AssignedOnly,
        swaps: false,
        ..Default::default()
    }
}
fn basic() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 4]).unwrap()
}
#[test]
fn bounded_proposals_replay_per_step_without_consumption_history() {
    let p = basic();
    let a = p.initial_assignment();
    let provider = SampledPillarMoveProvider::new(neighborhood(), 88, 12);
    let once = provider.select(&p, &a, 0).unwrap().collect::<Vec<_>>();
    assert!(!once.is_empty() && once.len() < 12);
    assert!(provider.select(&p, &a, 0).unwrap().next().is_some());
    assert_eq!(
        once,
        provider.select(&p, &a, 0).unwrap().collect::<Vec<_>>()
    );
    assert_ne!(
        once,
        provider.select(&p, &a, 1).unwrap().collect::<Vec<_>>()
    );
    let wrapped = SampledPillarMoveProvider::new(neighborhood(), u64::MAX, 12);
    let zero = SampledPillarMoveProvider::new(neighborhood(), 0, 12);
    assert_eq!(
        wrapped.select(&p, &a, 1).unwrap().collect::<Vec<_>>(),
        zero.select(&p, &a, 0).unwrap().collect::<Vec<_>>()
    );
    let empty = SampledPillarMoveProvider::new(neighborhood(), 1, 0);
    assert!(empty.select(&p, &a, 0).unwrap().next().is_none());
    assert!(provider.select(&p, &Assignment::new(vec![]), 0).is_err());
    let tiny = FiniteProblem::new(vec![Variable::new(vec![0]).with_initial(0)]).unwrap();
    assert!(
        provider
            .select(&tiny, &tiny.initial_assignment(), 0)
            .unwrap()
            .next()
            .is_none()
    );
    let p = FiniteProblem::new(vec![Variable::new(vec![0]).with_initial(0); 80]).unwrap();
    let provider = SampledPillarMoveProvider::new(
        PillarMoveProvider {
            left: PillarSelectionPolicy::subsets(40, 40).unwrap(),
            swaps: false,
            ..Default::default()
        },
        0,
        20,
    );
    assert!(
        provider
            .select(&p, &p.initial_assignment(), 0)
            .unwrap()
            .next()
            .is_none()
    );
}
#[test]
fn sampled_candidates_match_exhaustive_legal_successors_with_schemas_ranges_and_pins() {
    for pinned in [false, true] {
        for restricted in [false, true] {
            let values = [0, 1, 0, 1, 1, 0, 1, 0];
            let p = FiniteProblem::new(
                (0..8)
                    .map(|i| {
                        Variable::new(if restricted && i == 4 {
                            vec![1]
                        } else {
                            vec![0, 1]
                        })
                        .with_initial(values[i])
                        .allowing_unassigned()
                        .with_pinned(pinned && i == 7)
                    })
                    .collect(),
            )
            .unwrap()
            .with_pillar_schema(
                PillarSchema::new(vec![vec![0, 1], vec![2, 3], vec![4, 5], vec![6, 7]]).unwrap(),
            )
            .unwrap();
            let a = p.initial_assignment();
            for changes in [
                PillarChangeMode::Disabled,
                PillarChangeMode::AssignedOnly,
                PillarChangeMode::UnassignOnly,
                PillarChangeMode::All,
            ] {
                let neighborhood = PillarMoveProvider {
                    left: PillarSelectionPolicy::subsets(1, 2).unwrap(),
                    right: PillarSelectionPolicy::whole(),
                    changes,
                    swaps: true,
                };
                let successor = |m| {
                    let mut next = a.clone();
                    p.apply(&mut next, &m).unwrap().commit();
                    assert_ne!(next, a);
                    next.values().to_vec()
                };
                let expected = neighborhood
                    .select(&p, &a)
                    .unwrap()
                    .map(successor)
                    .collect::<BTreeSet<_>>();
                let sampled = SampledPillarMoveProvider::new(neighborhood, 70, 4096);
                let actual = sampled
                    .select(&p, &a, 0)
                    .unwrap()
                    .map(successor)
                    .collect::<BTreeSet<_>>();
                assert_eq!(
                    actual, expected,
                    "pinned={pinned},restricted={restricted},changes={changes:?}"
                );
            }
        }
    }
}
#[test]
fn sampled_windows_drive_two_steps_in_basic_and_stream_search() {
    for stream in [false, true] {
        let p = basic();
        let mut provider = SampledPillarMoveProvider::new(neighborhood(), 42, 64);
        let config = LocalSearchConfig {
            max_steps: 2,
            assert_scores: true,
            ..Default::default()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("ones", source.port(), |(_, v)| {
                SimpleScore::new(v.unwrap_or(0) as i64)
            })
            .unwrap();
            stream_local_search_from_with_provider(
                &p,
                b.build(),
                source,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            let mut n = ConstraintNetwork::new(4);
            for i in 0..4 {
                n.add_constraint(format!("v{i}"), vec![i], |v| {
                    SimpleScore::new(v[0].copied().unwrap_or(0) as i64)
                })
                .unwrap();
            }
            local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut provider)
                .unwrap()
        };
        assert_eq!(result.completed_steps, 2);
        let best = result.best.unwrap();
        assert_eq!(best.assignment.values(), &[Some(1); 4]);
        assert_eq!(best.score, SimpleScore::new(4));
    }
}
#[test]
fn sampled_mixed_backends_regroup_basic_rows_and_keep_lists_unchanged() {
    let p = MixedProblem::new(
        basic(),
        ListProblem::new(vec![10, 20], vec![ListEntity::new(vec![10, 20])]).unwrap(),
    );
    for stream in [false, true] {
        let mut provider = SampledPillarMoveProvider::new(neighborhood(), 42, 64);
        let config = MixedLocalSearchConfig {
            search: LocalSearchConfig {
                max_steps: 2,
                assert_scores: true,
                ..Default::default()
            },
            joint_moves: true,
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = MixedStreamSources::new(&mut b);
            b.score(
                "ones",
                source.basic.port(),
                |(_, v): &(usize, Option<i32>)| SimpleScore::new(v.unwrap_or(0) as i64),
            )
            .unwrap();
            mixed_stream_local_search_from_with_provider(
                &p,
                b.build(),
                source,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            let mut n = MixedConstraintNetwork::new(4, 1, 2);
            for i in 0..4 {
                n.add_constraint(format!("v{i}"), vec![MixedScope::Basic(i)], |v| {
                    let MixedFact::Basic(v) = v[0] else {
                        unreachable!()
                    };
                    SimpleScore::new(v.copied().unwrap_or(0) as i64)
                })
                .unwrap();
            }
            mixed_local_search_from_with_provider(
                &p,
                &n,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        };
        assert_eq!(result.completed_steps, 2);
        let best = result.best.unwrap();
        assert_eq!(best.assignment.basic().values(), &[Some(1); 4]);
        assert_eq!(best.assignment.lists(), p.initial_assignment().lists());
        assert_eq!(best.score, SimpleScore::new(4));
    }
}
