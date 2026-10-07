use std::{collections::BTreeSet, num::NonZeroUsize};
use timetuck::*;
fn successors(
    p: &ListProblem<i32>,
    a: &ListAssignment<i32>,
    moves: impl Iterator<Item = ListMove<i32>>,
) -> BTreeSet<Vec<Vec<i32>>> {
    moves
        .map(|m| {
            let mut next = a.clone();
            p.apply(&mut next, &m).unwrap().commit();
            next.lists().to_vec()
        })
        .collect()
}
#[test]
fn cartesian_reaches_existing_swap_successors_for_every_small_partial_state() {
    for code in 0..27 {
        let mut rows = vec![vec![], vec![]];
        let mut state = code;
        for v in [2, 4, 6] {
            let owner = state % 3;
            state /= 3;
            if owner < 2 {
                rows[owner].push(v);
            }
        }
        let p = ListProblem::new(
            vec![2, 4, 6],
            rows.into_iter().map(ListEntity::new).collect(),
        )
        .unwrap()
        .allowing_unassigned();
        let a = p.initial_assignment();
        let selected = list_value_swap_moves(
            &p,
            &a,
            p.values(),
            p.values(),
            ListSwapPairing::Cartesian,
            false,
        )
        .unwrap();
        assert_eq!(
            successors(&p, &a, selected),
            successors(&p, &a, list_swap_moves(&p, &a).unwrap())
        );
    }
}
#[test]
fn bounded_sampling_is_lazy_reproducible_and_does_not_invent_candidates() {
    let p = ListProblem::new(vec![2, 4, 6], vec![ListEntity::new(vec![2, 4])])
        .unwrap()
        .allowing_unassigned();
    let a = p.initial_assignment();
    let all: Vec<_> = list_value_swap_moves(
        &p,
        &a,
        p.values(),
        p.values(),
        ListSwapPairing::Cartesian,
        true,
    )
    .unwrap()
    .collect();
    let mut rng = SampleRandom::new(123);
    let mut same = rng.clone();
    drop(sample_list_value_swaps(&p, &a, &mut rng, 99, true).unwrap());
    let bound = NonZeroUsize::new(1_000_000).unwrap();
    assert_eq!(rng.index(bound), same.index(bound));
    let first: Vec<_> = sample_list_value_swaps(&p, &a, &mut rng, 300, true)
        .unwrap()
        .collect();
    let second: Vec<_> = sample_list_value_swaps(&p, &a, &mut same, 300, true)
        .unwrap()
        .collect();
    assert_eq!(first, second);
    assert_eq!(first.len(), 300);
    assert!(first.iter().all(|m| all.contains(m)));
    for values in [vec![], vec![1]] {
        let p = ListProblem::new(values.clone(), vec![ListEntity::new(values)]).unwrap();
        let a = p.initial_assignment();
        let mut rng = SampleRandom::new(9);
        let mut same = rng.clone();
        assert_eq!(
            sample_list_value_swaps(&p, &a, &mut rng, 50, false)
                .unwrap()
                .count(),
            0
        );
        assert_eq!(rng.index(bound), same.index(bound));
    }
    let p = ListProblem::from_entity_ranges(vec![
        ListEntity::new(vec![1]).with_value_range(vec![1]),
        ListEntity::new(vec![2]).with_value_range(vec![2]),
    ])
    .unwrap();
    let a = p.initial_assignment();
    assert_eq!(
        sample_list_value_swaps(&p, &a, &mut rng, 1000, false)
            .unwrap()
            .count(),
        0
    );
}
#[test]
fn pins_ranges_dependencies_unknowns_and_required_unassigned_are_respected() {
    let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0, 1, 2])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0], vec![1]])
        .unwrap();
    let a = p.initial_assignment();
    assert_eq!(
        list_value_swap_moves(
            &p,
            &a,
            p.values(),
            p.values(),
            ListSwapPairing::Cartesian,
            false
        )
        .unwrap()
        .count(),
        0
    );
    assert!(matches!(
        list_value_swap_moves(&p, &a, &[99], &[], ListSwapPairing::Cartesian, true),
        Err(ListError::ValueOutsideDomain)
    ));
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![1, 2]).with_pinned_prefix(1)],
    )
    .unwrap();
    let a = p.initial_assignment();
    let moves: Vec<_> = list_value_swap_moves(
        &p,
        &a,
        &[1, 3, 2, 2],
        &[1, 3, 2],
        ListSwapPairing::Paired,
        true,
    )
    .unwrap()
    .collect();
    assert_eq!(moves, vec![ListMove::Composite(vec![])]);
    assert_eq!(a, p.initial_assignment());
}
fn provider() -> ListValueSwapProvider<i32> {
    ListValueSwapProvider {
        left: vec![3, 7],
        right: vec![7, 3],
        pairing: ListSwapPairing::Cartesian,
        include_noops: false,
    }
}
fn reward(row: &[i32]) -> SimpleScore {
    SimpleScore::new(i64::from(row == [7, 3]))
}
#[test]
fn provider_improves_list_and_mixed_search_and_reopens_after_moves() {
    let p = ListProblem::new(vec![3, 7], vec![ListEntity::new(vec![3, 7])]).unwrap();
    let mut n = ListConstraintNetwork::new(1, 2);
    n.add_constraint("order", vec![ListScope::Entity(0)], |f| {
        let ListFact::Entity(row) = f[0] else {
            unreachable!()
        };
        reward(row)
    })
    .unwrap();
    let config = LocalSearchConfig {
        max_steps: 2,
        assert_scores: true,
        ..Default::default()
    };
    let r = list_local_search_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        config,
        &mut provider(),
    )
    .unwrap();
    assert_eq!(r.best.unwrap().assignment.lists(), &[vec![7, 3]]);
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![8, 9]).with_initial(8)]).unwrap(),
        p,
    );
    let mut n = MixedConstraintNetwork::new(1, 1, 2);
    n.add_constraint("order", vec![MixedScope::ListEntity(0)], |f| {
        let MixedFact::ListEntity(row) = f[0] else {
            unreachable!()
        };
        reward(row)
    })
    .unwrap();
    let r = mixed_local_search_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        MixedLocalSearchConfig {
            search: config,
            ..Default::default()
        },
        &mut provider(),
    )
    .unwrap();
    let best = r.best.unwrap();
    assert_eq!(best.assignment.lists().lists(), &[vec![7, 3]]);
    assert_eq!(best.assignment.basic().values(), &[Some(8)]);
    let mut provider = provider();
    let moves: Vec<_> = provider.moves(&p, &best.assignment, 1).collect();
    for m in moves {
        let mut trial = best.assignment.clone();
        p.apply(&mut trial, &m).unwrap().commit();
        assert_eq!(trial.lists().lists(), &[vec![3, 7]]);
    }
}
