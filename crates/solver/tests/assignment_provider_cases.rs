// Adapted from pinned Community Assign/Unassign/PillarChange/PillarUnassign
// MoveProvider tests. Apache-2.0; see compat/provenance.json and notices.
use timetuck::*;
fn var(domain: Vec<i32>, initial: Option<i32>, optional: bool, pinned: bool) -> Variable<i32> {
    let mut v = Variable::new(domain);
    if optional {
        v = v.allowing_unassigned();
    }
    if let Some(x) = initial {
        v = v.with_initial(x);
    }
    v.with_pinned(pinned)
}
fn model(initial: &[Option<i32>], optional: bool) -> FiniteProblem<i32> {
    FiniteProblem::new(
        initial
            .iter()
            .map(|&x| var(vec![0, 1, 2], x, optional, false))
            .collect(),
    )
    .unwrap()
}
fn singles(p: &FiniteProblem<i32>, unassign: bool) -> Vec<Move<i32>> {
    let a = p.initial_assignment();
    if unassign {
        unassign_moves(p, &a).unwrap().collect()
    } else {
        assign_moves(p, &a).unwrap().collect()
    }
}
fn groups(
    p: &FiniteProblem<i32>,
    c: PillarChangeConfig,
    unassign: bool,
    limit: usize,
) -> Vec<Move<i32>> {
    let a = p.initial_assignment();
    if unassign {
        pillar_unassign_moves(p, &a).unwrap().take(limit).collect()
    } else {
        pillar_change_moves(p, &a, c).unwrap().take(limit).collect()
    }
}
fn change(i: usize, value: Option<i32>) -> Move<i32> {
    Move::Change { variable: i, value }
}
fn mass(members: &[usize], value: Option<i32>) -> Move<i32> {
    Move::Composite(members.iter().map(|&i| change(i, value)).collect())
}
fn restricted() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![
        var(vec![1, 2], Some(1), false, false),
        var(vec![1, 2], Some(1), false, false),
        var(vec![3], Some(3), false, false),
        var(vec![3], Some(3), false, false),
    ])
    .unwrap()
}
fn nullable_pillar() -> FiniteProblem<i32> {
    model(&[Some(0), Some(0), Some(1), None], true)
}
fn optional_pair() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![
        var(vec![0, 1], None, true, false),
        var(vec![0, 1], Some(1), true, false),
    ])
    .unwrap()
}
#[test]
fn assign_required_rejected() {
    let p = model(&[Some(0)], false);
    assert!(assign_moves(&p, &p.initial_assignment()).is_err());
}
#[test]
fn assign_pinned_skipped() {
    let p = FiniteProblem::new(vec![
        var(vec![0, 1], None, true, true),
        var(vec![0, 1], None, true, false),
    ])
    .unwrap();
    let ms = singles(&p, false);
    for x in [0, 1] {
        assert!(ms.contains(&change(1, Some(x))));
        assert!(!ms.contains(&change(0, Some(x))));
    }
}
#[test]
fn assign_entity_ranges() {
    let p = FiniteProblem::new(vec![
        var(vec![1, 2], None, true, false),
        var(vec![1, 3], None, true, false),
    ])
    .unwrap();
    let ms = singles(&p, false);
    for (i, x) in [(0, 1), (0, 2), (1, 1), (1, 3)] {
        assert!(ms.contains(&change(i, Some(x))));
    }
    assert!(!ms.contains(&change(0, Some(3))));
    assert!(!ms.contains(&change(1, Some(2))));
}
#[test]
fn assign_shared_range() {
    let p = FiniteProblem::new(vec![var(vec![0, 1], None, true, false); 2]).unwrap();
    let ms = singles(&p, false);
    for i in 0..2 {
        for x in 0..2 {
            assert!(ms.contains(&change(i, Some(x))));
        }
    }
}
#[test]
fn unassign_pinned_skipped() {
    let p = FiniteProblem::new(vec![
        var(vec![0, 1], Some(0), true, true),
        var(vec![0, 1], Some(1), true, false),
    ])
    .unwrap();
    let ms = singles(&p, true);
    assert!(ms.contains(&change(1, None)));
    assert!(!ms.contains(&change(0, None)));
}
#[test]
fn unassign_missing_skipped() {
    let ms = singles(&optional_pair(), true);
    assert!(ms.contains(&change(1, None)));
    assert!(!ms.contains(&change(0, None)));
}
#[test]
fn unassign_all_assigned() {
    let ms = singles(&model(&[Some(0), Some(1)], true), true);
    assert!(ms.contains(&change(0, None)));
    assert!(ms.contains(&change(1, None)));
}
#[test]
fn unassign_none_assigned() {
    let ms = singles(&model(&[None, None], true), true);
    assert!(!ms.contains(&change(0, None)));
    assert!(!ms.contains(&change(1, None)));
}
#[test]
fn unassign_required_rejected() {
    let p = model(&[Some(0)], false);
    assert!(unassign_moves(&p, &p.initial_assignment()).is_err());
}
#[test]
fn pillar_change_homogeneous_and_no_noops() {
    let p = model(&[Some(0), Some(0), Some(1), Some(2)], false);
    let a = p.initial_assignment();
    let ms = groups(&p, PillarChangeConfig::default(), false, 200);
    assert!(!ms.is_empty());
    for m in ms {
        let ids = m.touched_variables();
        assert!(ids.len() >= 2);
        let current = a.values()[ids[0]];
        assert!(ids.iter().all(|&i| a.values()[i] == current));
        assert!(!a.planning_values(&m).unwrap().contains(&current));
    }
}
#[test]
fn pillar_change_pinned_excluded() {
    let p = FiniteProblem::new(vec![
        var(vec![0, 1], Some(0), false, true),
        var(vec![0, 1], Some(0), false, false),
        var(vec![0, 1], Some(0), false, false),
    ])
    .unwrap();
    let ms = groups(&p, PillarChangeConfig::default(), false, 50);
    assert!(!ms.is_empty());
    assert!(ms.iter().all(|m| !m.touched_variables().contains(&0)));
}
#[test]
fn pillar_change_entity_ranges() {
    let ms = groups(
        &restricted(),
        PillarChangeConfig::default(),
        false,
        usize::MAX,
    );
    assert!(ms.contains(&mass(&[0, 1], Some(2))));
    for x in [1, 2, 3] {
        assert!(!ms.contains(&mass(&[2, 3], Some(x))));
    }
}
#[test]
fn pillar_change_retirement() {
    let mut ms = groups(&restricted(), PillarChangeConfig::default(), false, 200);
    ms.dedup();
    assert_eq!(ms, vec![mass(&[0, 1], Some(2))]);
}
#[test]
fn pillar_change_rare_destination() {
    let p = FiniteProblem::new(vec![
        var((1..=50).collect(), Some(1), false, false),
        var(vec![1, 2], Some(1), false, false),
    ])
    .unwrap();
    assert!(groups(&p, PillarChangeConfig::default(), false, 10).contains(&mass(&[0, 1], Some(2))));
}
#[test]
fn pillar_change_default_null() {
    assert!(
        groups(
            &nullable_pillar(),
            PillarChangeConfig::default(),
            false,
            usize::MAX
        )
        .contains(&mass(&[0, 1], None))
    );
}
#[test]
fn pillar_change_disabled_null() {
    assert!(
        !groups(
            &nullable_pillar(),
            PillarChangeConfig::default().with_crossing_null(false),
            false,
            usize::MAX
        )
        .contains(&mass(&[0, 1], None))
    );
}
#[test]
fn pillar_change_null_only_destination() {
    let p = FiniteProblem::new(vec![var(vec![1], Some(1), true, false); 2]).unwrap();
    let ms = groups(&p, PillarChangeConfig::default(), false, usize::MAX);
    assert!(ms.contains(&mass(&[0, 1], None)));
    assert!(!ms.contains(&mass(&[0, 1], Some(1))));
}
#[test]
fn pillar_change_required_null_rejected() {
    let p = model(&[Some(0)], false);
    assert!(
        pillar_change_moves(
            &p,
            &p.initial_assignment(),
            PillarChangeConfig::default().with_crossing_null(true)
        )
        .is_err()
    );
}
#[test]
fn pillar_unassign_required_rejected() {
    let p = model(&[Some(0)], false);
    assert!(pillar_unassign_moves(&p, &p.initial_assignment()).is_err());
}
#[test]
fn pillar_unassign_assigned_only() {
    let p = optional_pair();
    let a = p.initial_assignment();
    let ms = groups(&p, PillarChangeConfig::default(), true, 50);
    assert!(!ms.is_empty());
    for m in ms {
        let ids = m.touched_variables();
        assert!(ids.contains(&1));
        assert!(!ids.contains(&0));
        assert_eq!(a.planning_values(&m).unwrap().first(), Some(&None));
    }
}
#[test]
fn pillar_unassign_pinned_excluded() {
    let p = FiniteProblem::new(vec![
        var(vec![0], Some(0), true, true),
        var(vec![0], Some(0), true, false),
    ])
    .unwrap();
    let ms = groups(&p, PillarChangeConfig::default(), true, 50);
    assert!(!ms.is_empty());
    assert!(ms.iter().all(|m| m.touched_variables() == [1]));
}
#[test]
fn pillar_unassign_homogeneous() {
    let p = nullable_pillar();
    let a = p.initial_assignment();
    let ms = groups(&p, PillarChangeConfig::default(), true, 200);
    assert!(!ms.is_empty());
    for m in ms {
        let ids = m.touched_variables();
        let current = a.values()[ids[0]];
        assert!(current.is_some());
        assert!(ids.iter().all(|&i| a.values()[i] == current));
        assert_eq!(a.planning_values(&m).unwrap().first(), Some(&None));
    }
}
#[test]
fn pillar_unassign_undo() {
    let p = optional_pair();
    let mut a = p.initial_assignment();
    let original = a.values()[1];
    assert!(original.is_some());
    let m = pillar_unassign_moves(&p, &a).unwrap().next().unwrap();
    {
        let tx = p.apply(&mut a, &m).unwrap();
        assert!(tx.values()[1].is_none());
    }
    assert_eq!(a.values()[1], original);
}
