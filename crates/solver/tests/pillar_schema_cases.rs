// Adapted from pinned Timefold Community pillar swap provider tests (Apache-2.0).
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
fn basic(initial: &[usize], domain: usize) -> FiniteProblem<usize> {
    FiniteProblem::new(
        initial
            .iter()
            .map(|&v| Variable::new((0..domain).collect()).with_initial(v))
            .collect(),
    )
    .unwrap()
    .with_pillar_schema(PillarSchema::new((0..initial.len()).map(|i| vec![i]).collect()).unwrap())
    .unwrap()
}
fn multivar(exclude_secondary: bool) -> FiniteProblem<usize> {
    let mut vars = vec![];
    for secondary in [0, 0, 1, 1] {
        vars.push(Variable::new(vec![0, 1]).with_initial(0));
        vars.push(Variable::new(vec![0, 1]).with_initial(secondary));
        vars.push(
            Variable::new(vec![100])
                .with_initial(100)
                .allowing_unassigned(),
        );
    }
    let rows = (0..4)
        .map(|i| {
            if exclude_secondary {
                vec![i * 3, i * 3 + 2]
            } else {
                vec![i * 3, i * 3 + 1, i * 3 + 2]
            }
        })
        .collect();
    FiniteProblem::new(vars)
        .unwrap()
        .with_pillar_schema(PillarSchema::new(rows).unwrap())
        .unwrap()
}
fn illegal_multi() -> FiniteProblem<usize> {
    let vars = vec![
        Variable::new(vec![1, 4])
            .with_initial(1)
            .allowing_unassigned(),
        Variable::new(vec![1, 4])
            .with_initial(1)
            .allowing_unassigned(),
        Variable::new(vec![1]).allowing_unassigned(),
        Variable::new(vec![1, 4])
            .with_initial(4)
            .allowing_unassigned(),
        Variable::new(vec![1, 3, 4])
            .with_initial(3)
            .allowing_unassigned(),
        Variable::new(vec![1]).allowing_unassigned(),
    ];
    FiniteProblem::new(vars)
        .unwrap()
        .with_pillar_schema(PillarSchema::new(vec![vec![0, 1, 2], vec![3, 4, 5]]).unwrap())
        .unwrap()
}
fn sub_config(size: usize) -> SubPillarConfig {
    SubPillarConfig {
        changes: PillarChangeMode::Disabled,
        ..SubPillarConfig::new(SubPillarSelection::combinations(size, size).unwrap())
    }
}
fn expect_both(
    p: &FiniteProblem<usize>,
    moves: Vec<Move<usize>>,
    left: Vec<Vec<usize>>,
    right: Vec<Vec<usize>>,
) {
    let a = p.initial_assignment();
    let l = Pillar::from_rows(left).unwrap();
    let r = Pillar::from_rows(right).unwrap();
    assert!(moves.contains(&l.swap_move(&r, &a).unwrap()));
    assert!(moves.contains(&r.swap_move(&l, &a).unwrap()));
}
#[test]
fn whole_equal_values_produce_no_move() {
    let p = basic(&[0; 4], 1);
    assert!(
        pillar_swap_moves(&p, &p.initial_assignment())
            .unwrap()
            .take(50)
            .next()
            .is_none()
    );
}
#[test]
fn whole_at_least_one_differing_variable_produces_move() {
    let p = multivar(false);
    let moves = pillar_swap_moves(&p, &p.initial_assignment())
        .unwrap()
        .collect();
    expect_both(
        &p,
        moves,
        vec![vec![0, 1, 2], vec![3, 4, 5]],
        vec![vec![6, 7, 8], vec![9, 10, 11]],
    );
}
#[test]
fn whole_out_of_range_cross_value_rejects_whole_pair() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]).with_initial(1),
        Variable::new(vec![1, 2]).with_initial(1),
        Variable::new(vec![2]).with_initial(2),
        Variable::new(vec![2]).with_initial(2),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let l = Pillar::new([0, 1]).unwrap();
    let r = Pillar::new([2, 3]).unwrap();
    let moves: Vec<_> = pillar_swap_moves(&p, &a).unwrap().collect();
    assert!(!moves.contains(&l.swap_move(&r, &a).unwrap()));
    assert!(!moves.contains(&r.swap_move(&l, &a).unwrap()));
}
#[test]
fn whole_mixed_legality_across_variables_rejects_whole_pair() {
    let p = illegal_multi();
    let a = p.initial_assignment();
    let l = Pillar::from_rows(vec![vec![0, 1, 2]]).unwrap();
    let r = Pillar::from_rows(vec![vec![3, 4, 5]]).unwrap();
    let moves: Vec<_> = pillar_swap_moves(&p, &a).unwrap().collect();
    assert!(!moves.contains(&l.swap_move(&r, &a).unwrap()));
    assert!(!moves.contains(&r.swap_move(&l, &a).unwrap()));
}
#[test]
fn whole_pinned_entity_never_participates_in_swap() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0).with_pinned(true),
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(1),
        Variable::new(vec![0, 1]).with_initial(1),
    ])
    .unwrap();
    let moves: Vec<_> = pillar_swap_moves(&p, &p.initial_assignment())
        .unwrap()
        .take(100)
        .collect();
    assert!(!moves.is_empty());
    assert!(moves.iter().all(|m| !m.touched_variables().contains(&0)));
}
#[test]
fn whole_three_against_five_swaps_every_member() {
    let p = basic(&[0, 0, 0, 1, 1, 1, 1, 1], 2);
    let moves = pillar_swap_moves(&p, &p.initial_assignment())
        .unwrap()
        .collect();
    expect_both(
        &p,
        moves,
        (0..3).map(|i| vec![i]).collect(),
        (3..8).map(|i| vec![i]).collect(),
    );
}
#[test]
fn whole_size_one_pillars_are_generated() {
    let p = basic(&[0, 1, 2, 3], 4);
    let moves: Vec<_> = pillar_swap_moves(&p, &p.initial_assignment())
        .unwrap()
        .take(100)
        .collect();
    assert!(!moves.is_empty());
    assert!(moves.iter().all(|m| m.touched_variables().len() == 2));
}
#[test]
fn whole_excluded_variable_is_ignored_for_key_and_swap() {
    let p = multivar(true);
    assert!(
        pillar_swap_moves(&p, &p.initial_assignment())
            .unwrap()
            .take(50)
            .next()
            .is_none()
    );
}
#[test]
fn sub_multivariate_swap_uses_every_listed_variable() {
    let p = multivar(false);
    let moves = subpillar_moves(&p, &p.initial_assignment(), sub_config(2))
        .unwrap()
        .collect();
    expect_both(
        &p,
        moves,
        vec![vec![0, 1, 2], vec![3, 4, 5]],
        vec![vec![6, 7, 8], vec![9, 10, 11]],
    );
}
#[test]
fn sub_excluded_variable_is_ignored_for_key_and_swap() {
    let p = multivar(true);
    assert!(
        subpillar_moves(&p, &p.initial_assignment(), sub_config(2))
            .unwrap()
            .take(50)
            .next()
            .is_none()
    );
}
#[test]
fn sub_mixed_legality_across_variables_rejects_whole_pair() {
    let p = illegal_multi();
    assert!(
        subpillar_moves(&p, &p.initial_assignment(), sub_config(1))
            .unwrap()
            .take(50)
            .next()
            .is_none()
    );
}
