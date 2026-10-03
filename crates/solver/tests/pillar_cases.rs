// Adapted from Timefold Solver Community tests (Apache-2.0).
// See compat/provenance.json and THIRD_PARTY_NOTICES.md for sources and changes.
use timetuck::*;

#[test]
fn both_old_values_are_read_before_either_pillar_is_mutated() {
    let mut a = Assignment::new(vec![Some(1), Some(2)]);
    let movement = Pillar::new([0])
        .unwrap()
        .swap_move(&Pillar::new([1]).unwrap(), &a)
        .unwrap();
    a.apply(&movement).unwrap().commit();
    assert_eq!(a.values(), &[Some(2), Some(1)]);
    assert_ne!(a.values()[1], Some(2));
}

#[test]
fn unequal_sized_pillars_swap_every_member() {
    let mut a = Assignment::new(vec![
        Some(1),
        Some(1),
        Some(1),
        Some(2),
        Some(2),
        Some(2),
        Some(2),
        Some(2),
    ]);
    let movement = Pillar::new(0..3)
        .unwrap()
        .swap_move(&Pillar::new(3..8).unwrap(), &a)
        .unwrap();
    a.apply(&movement).unwrap().commit();
    assert_eq!(&a.values()[..3], &[Some(2); 3]);
    assert_eq!(&a.values()[3..], &[Some(1); 5]);
}

#[test]
fn multi_variate_swaps_every_differing_variable() {
    // Three slots per entity: primary, secondary, unselected tertiary.
    let mut a = Assignment::new(
        [1, 2, 101, 1, 2, 102, 3, 4, 101, 3, 4, 102]
            .map(Some)
            .to_vec(),
    );
    let left = Pillar::from_rows(vec![vec![0, 1], vec![3, 4]]).unwrap();
    let right = Pillar::from_rows(vec![vec![6, 7], vec![9, 10]]).unwrap();
    let movement = left.swap_move(&right, &a).unwrap();
    a.apply(&movement).unwrap().commit();
    assert_eq!(
        a.values(),
        &[3, 4, 101, 3, 4, 102, 1, 2, 101, 1, 2, 102].map(Some)
    );
}

#[test]
fn undo_restores_every_members_original_value() {
    let mut a = Assignment::new(vec![Some(1), Some(1), Some(2), Some(2)]);
    let movement = Pillar::new([0, 1])
        .unwrap()
        .swap_move(&Pillar::new([2, 3]).unwrap(), &a)
        .unwrap();
    {
        let trial = a.apply(&movement).unwrap();
        assert_eq!(trial.values(), &[Some(2), Some(2), Some(1), Some(1)]);
    }
    assert_eq!(a.values(), &[Some(1), Some(1), Some(2), Some(2)]);
}

fn entity_ranges() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![
        Variable::new(vec![1, 2, 3]).allowing_unassigned(),
        Variable::new(vec![2, 3, 4, 5]).allowing_unassigned(),
        Variable::new(vec![3, 4, 5]).allowing_unassigned(),
    ])
    .unwrap()
}

#[test]
fn is_move_doable_value_range_provider_on_entity() {
    let p = FiniteProblem::new(entity_ranges().variables()[..2].to_vec()).unwrap();
    let a = Assignment::new(vec![Some(2), Some(2)]);
    let pillar = Pillar::new([0, 1]).unwrap();
    for (target, doable) in [(1, false), (2, false), (3, true), (4, false)] {
        let movement = pillar.change_move(&a, &[Some(target)]).unwrap();
        assert_eq!(
            pillar_moves(&p, &a, false).unwrap().any(|m| m == movement),
            doable
        );
    }
}

#[test]
fn do_move() {
    let p = entity_ranges();
    // The source executes raw moves, including no-ops and value 2 outside c's
    // range. Use raw assignment transactions here; validated moves enforce ranges.
    for (members, old) in [(2, 3), (2, 2), (3, 2), (3, 3)] {
        let mut values = p.initial_assignment().values().to_vec();
        values[..members].fill(Some(old));
        let mut a = Assignment::new(values);
        let movement = Pillar::new(0..members)
            .unwrap()
            .change_move(&a, &[Some(2)])
            .unwrap();
        a.apply(&movement).unwrap().commit();
        assert_eq!(&a.values()[..members], vec![Some(2); members]);
    }
}
