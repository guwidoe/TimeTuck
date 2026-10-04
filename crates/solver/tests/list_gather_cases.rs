// Adapted Apache-2.0 MassListChangeMoveTest methods from pinned Community.
// Full identities, notices and modifications: compat/provenance.json.
use timetuck::*;
fn problem(lists: Vec<Vec<i32>>, domain: Vec<i32>) -> ListProblem<i32> {
    ListProblem::new(domain, lists.into_iter().map(ListEntity::new).collect()).unwrap()
}
#[test]
fn execute_gathers_every_member_consecutively_at_destination() {
    let p = problem(
        vec![vec![0, 1], vec![2, 3], vec![4, 5, 6]],
        (0..7).collect(),
    );
    let mut a = p.initial_assignment();
    let m = p
        .gather_move(
            &a,
            &[1, 2],
            Some(ListPosition {
                entity: 2,
                index: 1,
            }),
        )
        .unwrap();
    p.apply(&mut a, &m).unwrap().commit();
    assert_eq!(a.lists(), &[vec![0], vec![3], vec![4, 1, 2, 5, 6]]);
}
#[test]
fn destination_index_is_adjusted_only_by_members_originally_before_it() {
    let p = problem(vec![vec![0, 1, 2, 3, 4]], (0..5).collect());
    let mut a = p.initial_assignment();
    let m = p
        .gather_move(
            &a,
            &[0, 2, 3],
            Some(ListPosition {
                entity: 0,
                index: 2,
            }),
        )
        .unwrap();
    p.apply(&mut a, &m).unwrap().commit();
    assert_eq!(a.lists(), &[vec![1, 0, 2, 3, 4]]);
}
#[test]
fn execute_assigns_currently_unassigned_members_too() {
    let p = problem(vec![vec![0], vec![]], vec![0, 1]).allowing_unassigned();
    let mut a = p.initial_assignment();
    let m = p
        .gather_move(
            &a,
            &[1, 0],
            Some(ListPosition {
                entity: 1,
                index: 0,
            }),
        )
        .unwrap();
    p.apply(&mut a, &m).unwrap().commit();
    assert_eq!(a.lists(), &[vec![], vec![1, 0]]);
}
#[test]
fn execute_to_null_destination_unassigns_every_member() {
    let p = problem(vec![vec![0, 1]], vec![0, 1]).allowing_unassigned();
    let mut a = p.initial_assignment();
    let m = p.gather_move(&a, &[0, 1], None).unwrap();
    p.apply(&mut a, &m).unwrap().commit();
    assert!(a.list(0).unwrap().is_empty());
}
#[test]
fn undo_restores_every_members_original_position() {
    let p = problem(vec![vec![0, 1], vec![2]], vec![0, 1, 2]);
    let mut a = p.initial_assignment();
    let m = p
        .gather_move(
            &a,
            &[1],
            Some(ListPosition {
                entity: 1,
                index: 0,
            }),
        )
        .unwrap();
    {
        let trial = p.apply(&mut a, &m).unwrap();
        assert_eq!(trial.list(1), Some(&[1, 2][..]));
    }
    assert_eq!(a.lists(), &[vec![0, 1], vec![2]]);
}
