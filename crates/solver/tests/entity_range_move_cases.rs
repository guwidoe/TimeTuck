// Adapted from pinned Timefold Community move tests (Apache-2.0).
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
// Entity slots and integer value IDs replace Java objects/descriptors. Raw
// execution deliberately bypasses domains, as the source execution cases do;
// doability and caller-misuse cases use the checked native model boundary.
use timetuck::*;

type PillarCase<'a> = (&'a [usize], &'a [usize], i32, i32, bool);

fn model(ranges: &[&[i32]], allows_unassigned: bool) -> FiniteProblem<i32> {
    FiniteProblem::new(
        ranges
            .iter()
            .map(|range| {
                let variable = Variable::new(range.to_vec());
                if allows_unassigned {
                    variable.allowing_unassigned()
                } else {
                    variable
                }
            })
            .collect(),
    )
    .unwrap()
}

fn set(a: &mut Assignment<i32>, members: &[usize], value: i32) {
    for &variable in members {
        a.apply(&Move::Change {
            variable,
            value: Some(value),
        })
        .unwrap()
        .commit();
    }
}

fn assert_members(a: &Assignment<i32>, members: &[usize], value: i32) {
    for &member in members {
        assert_eq!(a.get(member), Some(&Some(value)), "member {member}");
    }
}

#[test]
fn swap_raw_execution_preserves_all_source_states() {
    let p = model(&[&[1, 2, 3], &[1, 2, 3, 4], &[2, 3, 4]], true);
    let mut a = p.initial_assignment();
    // Preserve the source sequence, including equal values, the second repeated
    // b/c case, and a receiving 4 outside its declared [1,2,3] range.
    for (left, right, before_left, before_right, repeat) in [
        (0, 1, 1, 1, false),
        (0, 1, 1, 2, false),
        (0, 1, 2, 3, true),
        (0, 2, 2, 2, false),
        (0, 2, 3, 2, false),
        (0, 2, 3, 4, true),
        (1, 2, 2, 2, false),
        (1, 2, 2, 3, false),
        (1, 2, 2, 3, true),
    ] {
        set(&mut a, &[left], before_left);
        set(&mut a, &[right], before_right);
        let movement = Move::Swap { left, right };
        a.apply(&movement).unwrap().commit();
        assert_members(&a, &[left], before_right);
        assert_members(&a, &[right], before_left);
        if repeat {
            a.apply(&movement).unwrap().commit();
            assert_members(&a, &[left], before_left);
            assert_members(&a, &[right], before_right);
        }
    }
}

#[test]
fn pillar_swap_raw_execution_preserves_all_source_states() {
    let p = model(
        &[&[1, 2, 3, 4], &[2, 3, 4, 5], &[4, 5], &[1, 2, 3, 4, 5]],
        true,
    );
    let mut a = p.initial_assignment();
    let cases: &[PillarCase<'_>] = &[
        (&[0], &[1], 1, 1, false),
        (&[0], &[1], 2, 1, false),
        (&[0], &[1], 3, 2, true),
        (&[0, 1], &[3], 3, 2, true),
        (&[0, 1], &[3], 3, 4, true),
        (&[0], &[1, 2, 3], 2, 3, true),
        (&[0, 1], &[2, 3], 4, 3, true),
    ];
    for &(left, right, before_left, before_right, repeat) in cases {
        set(&mut a, left, before_left);
        set(&mut a, right, before_right);
        let left_pillar = Pillar::new(left.iter().copied()).unwrap();
        let right_pillar = Pillar::new(right.iter().copied()).unwrap();
        let movement = left_pillar.swap_move(&right_pillar, &a).unwrap();
        a.apply(&movement).unwrap().commit();
        assert_members(&a, left, before_right);
        assert_members(&a, right, before_left);
        if repeat {
            // Native pillar moves capture old values, so regenerate against the
            // current assignment to repeat the source's live pillar exchange.
            let movement = left_pillar.swap_move(&right_pillar, &a).unwrap();
            a.apply(&movement).unwrap().commit();
            assert_members(&a, left, before_left);
            assert_members(&a, right, before_right);
        }
    }
}

#[test]
fn pillar_swap_entity_range_doability_matrix() {
    let p = model(
        &[&[1, 2, 3], &[2, 3, 4, 5], &[4, 5], &[1, 2, 3, 4, 5]],
        true,
    );
    let mut a = p.initial_assignment();
    let cases: &[PillarCase<'_>] = &[
        (&[0], &[1], 1, 2, false),
        (&[0], &[1], 2, 2, false),
        (&[0], &[1], 2, 3, true),
        (&[0], &[1], 3, 2, true),
        (&[0], &[1], 3, 3, false),
        (&[0], &[1], 2, 4, false),
        (&[0], &[2], 1, 4, false),
        (&[0], &[2], 2, 5, false),
        (&[1], &[2], 2, 4, false),
        (&[1], &[2], 4, 5, true),
        (&[1], &[2], 5, 4, true),
        (&[1], &[2], 5, 5, false),
        (&[0, 1], &[3], 2, 4, false),
        (&[0, 1], &[3], 2, 1, false),
        (&[0, 1], &[3], 2, 3, true),
        (&[0, 1], &[3], 3, 2, true),
        (&[0, 1], &[3], 2, 2, false),
    ];
    for &(left, right, before_left, before_right, expected) in cases {
        set(&mut a, left, before_left);
        set(&mut a, right, before_right);
        let movement = Pillar::new(left.iter().copied())
            .unwrap()
            .swap_move(&Pillar::new(right.iter().copied()).unwrap(), &a)
            .unwrap();
        assert_eq!(
            p.is_move_doable(&a, &movement) == Ok(true),
            expected,
            "{left:?}/{right:?}: {before_left}/{before_right}"
        );
    }
}

#[test]
fn list_change_entity_range_doability() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1, 2]).with_value_range(vec![1, 2]),
            ListEntity::new(vec![3]).with_value_range(vec![1, 3]),
        ],
    )
    .unwrap();
    for (index, expected) in [(0, true), (1, false)] {
        let movement = ListMove::Change {
            source: ListPosition { entity: 0, index },
            destination: ListPosition {
                entity: 1,
                index: 0,
            },
        };
        assert_eq!(
            p.is_move_doable(&p.initial_assignment(), &movement) == Ok(true),
            expected
        );
    }
}

#[test]
fn invalid_pillar_cross_value_is_rejected_by_checked_execution() {
    let p = model(&[&[1, 2], &[1], &[1, 2]], false);
    let mut a = Assignment::new(vec![Some(1), Some(1), Some(2)]);
    let movement = Pillar::new([0, 1])
        .unwrap()
        .swap_move(&Pillar::new([2]).unwrap(), &a)
        .unwrap();
    let error = p.apply(&mut a, &movement).err().expect("range rejection");
    assert!(matches!(
        error,
        ModelError::ValueOutsideDomain { variable: 1 }
    ));
    assert!(error.to_string().contains("outside its domain"));
}

// Original regression; no additional upstream method credit.
#[test]
fn checked_rejection_and_raw_rollback_preserve_the_opening_assignment() {
    let p = model(&[&[1, 2], &[1], &[1, 2]], false);
    let original = Assignment::new(vec![Some(1), Some(1), Some(2)]);
    let mut a = original.clone();
    let movement = Pillar::new([0, 1])
        .unwrap()
        .swap_move(&Pillar::new([2]).unwrap(), &a)
        .unwrap();
    assert!(p.apply(&mut a, &movement).is_err());
    assert_eq!(a, original);
    {
        let trial = a.apply(&movement).unwrap();
        assert_eq!(trial.values(), &[Some(2), Some(2), Some(1)]);
    }
    assert_eq!(a, original);
    p.validate(&a).unwrap();
}
