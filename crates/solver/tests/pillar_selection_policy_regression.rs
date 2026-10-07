//! Original comparator selection and native search integration checks.
use std::{
    cmp::Ordering,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn ordered_windows_keep_aligned_columns_and_stable_ties() {
    let p = Pillar::from_rows(vec![vec![0, 1], vec![2, 3], vec![4, 5], vec![6, 7]]).unwrap();
    let order = [2, 0, 3, 1];
    let c = PillarComparator::new(move |a, b| order[a[0] / 2].cmp(&order[b[0] / 2]));
    let policy = PillarSelectionPolicy::sequential(2, 2, Some(c)).unwrap();
    let rows: Vec<_> = policy.select(&p).map(|p| p.rows().to_vec()).collect();
    assert_eq!(
        rows,
        [
            vec![vec![2, 3], vec![6, 7]],
            vec![vec![0, 1], vec![6, 7]],
            vec![vec![0, 1], vec![4, 5]]
        ]
    );
    let tied = PillarSelectionPolicy::sequential(
        2,
        2,
        Some(PillarComparator::new(|_, _| Ordering::Equal)),
    )
    .unwrap();
    assert_eq!(
        tied.select(&p).collect::<Vec<_>>(),
        p.subpillars(SubPillarSelection::consecutive(2, 2).unwrap())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        PillarSelectionPolicy::whole()
            .select(&p)
            .collect::<Vec<_>>()
            .as_slice(),
        std::slice::from_ref(&p)
    );
    assert_eq!(
        PillarSelectionPolicy::subsets(5, 10)
            .unwrap()
            .select(&p)
            .count(),
        0
    );
    assert_eq!(
        PillarSelectionPolicy::subsets(1, usize::MAX)
            .unwrap()
            .select(&p)
            .count(),
        15
    );
    let before = p.clone();
    let policy = PillarSelectionPolicy::sequential_unlimited(PillarComparator::new(|_, _| {
        panic!("compare")
    }));
    assert!(catch_unwind(AssertUnwindSafe(|| policy.select(&p))).is_err());
    assert_eq!(p, before);
}
#[test]
fn comparator_selected_subpillars_drive_local_search_in_both_backends() {
    for stream in [false, true] {
        let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 4]).unwrap();
        let whole = Pillar::new(0..4).unwrap();
        let rank = [0, 2, 1, 3];
        let policy = PillarSelectionPolicy::sequential(
            2,
            2,
            Some(PillarComparator::new(move |a, b| {
                rank[a[0]].cmp(&rank[b[0]])
            })),
        )
        .unwrap();
        let mut provider = |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
            policy
                .select(&whole)
                .map(|pillar| pillar.change_move(a, &[Some(1)]).unwrap())
                .collect::<Vec<_>>()
        };
        let config = LocalSearchConfig {
            max_steps: 1,
            ..Default::default()
        };
        let r = if stream {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("value", source.port(), |(i, v)| {
                SimpleScore::new(v.unwrap_or(0) as i64 * [3, 1, 3, 1][*i])
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
            for (i, weight) in [3, 1, 3, 1].into_iter().enumerate() {
                n.add_constraint(format!("value{i}"), vec![i], move |v| {
                    SimpleScore::new(v[0].copied().unwrap_or(0) as i64 * weight)
                })
                .unwrap();
            }
            local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut provider)
                .unwrap()
        };
        let best = r.best.unwrap();
        assert_eq!(
            best.assignment.values(),
            &[Some(1), Some(0), Some(1), Some(0)]
        );
        assert_eq!(best.score, SimpleScore::new(6));
        assert_eq!(r.completed_steps, 1);
    }
}
#[test]
fn policy_selection_owns_metadata_and_is_lazy_for_large_combinations() {
    let selected = {
        let pillar = Pillar::new(0..80).unwrap();
        PillarSelectionPolicy::subsets(40, 40)
            .unwrap()
            .select(&pillar)
    };
    let first = selected.take(2).map(|p| p.variables()).collect::<Vec<_>>();
    assert_eq!(first.len(), 2);
    assert_eq!(first[0], (0..40).collect::<Vec<_>>());
    assert_eq!(first[1], (0..39).chain([40]).collect::<Vec<_>>());
    fn shared<T: Send + Sync>() {}
    shared::<PillarSelectionPolicy>();
}
