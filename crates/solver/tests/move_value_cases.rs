// Adapted Apache-2.0 pinned Community move metadata methods.
// See compat/provenance.json for complete source identities and modifications.
use timetuck::*;
#[test]
fn mass_change_values_return_destination() {
    let a = Assignment::new(vec![None]);
    let movement = Pillar::new([0])
        .unwrap()
        .change_move(&a, &[Some(1)])
        .unwrap();
    assert_eq!(a.planning_values(&movement).unwrap(), vec![Some(1)]);
}
#[test]
fn pillar_swap_values_return_values_that_changed_hands() {
    let mut a = Assignment::new(vec![Some(1), Some(2)]);
    let movement = Pillar::new([0])
        .unwrap()
        .swap_move(&Pillar::new([1]).unwrap(), &a)
        .unwrap();
    let mut values = a.planning_values(&movement).unwrap();
    a.apply(&movement).unwrap().commit();
    values.sort();
    assert_eq!(values, vec![Some(1), Some(2)]);
}
fn list_values(initial: Vec<i32>, movement: ListMove<i32>, unassigned: bool) -> Vec<i32> {
    let mut p = ListProblem::new(initial.clone(), vec![ListEntity::new(initial)]).unwrap();
    if unassigned {
        p = p.allowing_unassigned();
    }
    let mut a = p.initial_assignment();
    let values = p.planning_values(&a, &movement).unwrap();
    p.apply(&mut a, &movement).unwrap().commit();
    values
}
#[test]
fn list_change_values_return_moved_value() {
    assert_eq!(
        list_values(
            vec![1, 2],
            ListMove::Change {
                source: ListPosition {
                    entity: 0,
                    index: 0
                },
                destination: ListPosition {
                    entity: 0,
                    index: 1
                }
            },
            false
        ),
        vec![1]
    );
}
#[test]
fn sublist_change_values_return_moved_values() {
    assert_eq!(
        list_values(
            vec![1, 2, 3],
            ListMove::SubListChange {
                entity: 0,
                start: 0,
                end: 2,
                destination: ListPosition {
                    entity: 0,
                    index: 1
                },
                reverse: false
            },
            false
        ),
        vec![1, 2]
    );
}
#[test]
fn sublist_swap_values_return_pre_move_values() {
    assert_eq!(
        list_values(
            vec![1, 2],
            ListMove::SubListSwap {
                left: ListRange {
                    entity: 0,
                    start: 0,
                    end: 1
                },
                right: ListRange {
                    entity: 0,
                    start: 1,
                    end: 2
                },
                reverse: false
            },
            false
        ),
        vec![1, 2]
    );
}
#[test]
fn sublist_unassign_values_return_unassigned_values() {
    assert_eq!(
        list_values(
            vec![1, 2],
            ListMove::Unassign {
                entity: 0,
                start: 0,
                end: 2
            },
            true
        ),
        vec![1, 2]
    );
}
