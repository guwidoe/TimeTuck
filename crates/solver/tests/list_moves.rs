// Adapted from pinned Timefold Community list move tests. Apache-2.0.
// See THIRD_PARTY_NOTICES.md and compat/provenance.json.
// Stable integer values/entity indices replace Java objects and descriptors;
// native RAII transactions replace permanent/temporary MoveTester execution.
use timetuck::{ListAssignment, ListEntity, ListMove, ListPosition, ListProblem};

fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn assign(value: i32, entity: usize, index: usize) -> ListMove<i32> {
    ListMove::Assign {
        value,
        destination: pos(entity, index),
    }
}
fn change(entity: usize, index: usize, dest: usize, dest_index: usize) -> ListMove<i32> {
    ListMove::Change {
        source: pos(entity, index),
        destination: pos(dest, dest_index),
    }
}
fn swap(entity: usize, index: usize, other: usize, other_index: usize) -> ListMove<i32> {
    ListMove::Swap {
        left: pos(entity, index),
        right: pos(other, other_index),
    }
}
// Inlined TestdataListSolution fixture: value i belongs to entity i % count.
fn generated(
    values: i32,
    entities: usize,
    initialized: bool,
) -> (ListProblem<i32>, ListAssignment<i32>) {
    let mut lists = vec![vec![]; entities];
    if initialized {
        for i in 0..values {
            lists[i as usize % entities].push(i);
        }
    }
    fixture((0..values).collect(), lists)
}
fn fixture(values: Vec<i32>, lists: Vec<Vec<i32>>) -> (ListProblem<i32>, ListAssignment<i32>) {
    let problem =
        ListProblem::new(values, lists.into_iter().map(ListEntity::new).collect()).unwrap();
    let assignment = problem.initial_assignment();
    (problem, assignment)
}

#[test]
fn list_assign_move_executes_permanently() {
    let (p, mut a) = generated(3, 2, false);
    p.apply(&mut a, &assign(0, 0, 0)).unwrap().commit();
    assert_eq!(a.list(0).unwrap().len(), 1);
    assert_eq!(a.list(0).unwrap()[0], 0);
}
#[test]
fn list_assign_move_multiple_values() {
    let (p, mut a) = generated(4, 1, false);
    for movement in [assign(0, 0, 0), assign(1, 0, 1), assign(2, 0, 1)] {
        p.apply(&mut a, &movement).unwrap().commit();
    }
    assert_eq!(a.list(0).unwrap().len(), 3);
    assert_eq!(a.list(0).unwrap()[0], 0);
    assert_eq!(a.list(0).unwrap()[1], 2);
    assert_eq!(a.list(0).unwrap()[2], 1);
}
#[test]
fn list_assign_move_to_multiple_entities() {
    let (p, mut a) = generated(3, 3, false);
    for i in 0..3 {
        p.apply(&mut a, &assign(i, i as usize, 0)).unwrap().commit();
    }
    assert_eq!(a.list(0).unwrap(), &[0]);
    assert_eq!(a.list(1).unwrap(), &[1]);
    assert_eq!(a.list(2).unwrap(), &[2]);
}
#[test]
fn list_assign_move_executes_temporarily_with_undo() {
    let (p, mut a) = generated(2, 1, false);
    {
        let candidate = p.apply(&mut a, &assign(0, 0, 0)).unwrap();
        assert_eq!(candidate.list(0).unwrap().len(), 1);
        assert_eq!(candidate.list(0).unwrap()[0], 0);
    }
    assert!(a.list(0).unwrap().is_empty());
}
#[test]
fn list_assign_move_temporary_multiple_values() {
    let (p, mut a) = generated(3, 1, false);
    {
        let candidate = p
            .apply(
                &mut a,
                &ListMove::Composite(vec![assign(0, 0, 0), assign(1, 0, 1), assign(2, 0, 1)]),
            )
            .unwrap();
        assert_eq!(candidate.list(0).unwrap().len(), 3);
        assert_eq!(candidate.list(0).unwrap()[0], 0);
        assert_eq!(candidate.list(0).unwrap()[1], 2);
        assert_eq!(candidate.list(0).unwrap()[2], 1);
    }
    assert!(a.list(0).unwrap().is_empty());
}
#[test]
fn list_swap_move_within_same_entity() {
    let (p, mut a) = generated(3, 1, true);
    p.apply(&mut a, &swap(0, 0, 0, 2)).unwrap().commit();
    assert_eq!(a.list(0).unwrap().len(), 3);
    assert_eq!(a.list(0).unwrap()[0], 2);
    assert_eq!(a.list(0).unwrap()[1], 1);
    assert_eq!(a.list(0).unwrap()[2], 0);
}
#[test]
fn list_swap_move_between_entities() {
    let (p, mut a) = generated(4, 2, true);
    p.apply(&mut a, &swap(0, 0, 1, 0)).unwrap().commit();
    assert_eq!(a.list(0).unwrap()[0], 1);
    assert_eq!(a.list(1).unwrap()[0], 0);
}
#[test]
fn list_swap_move_adjacent_positions() {
    let (p, mut a) = generated(4, 1, true);
    p.apply(&mut a, &swap(0, 1, 0, 2)).unwrap().commit();
    assert_eq!(a.list(0).unwrap()[1], 2);
    assert_eq!(a.list(0).unwrap()[2], 1);
}
#[test]
fn multiple_list_swap_moves() {
    let (p, mut a) = generated(6, 2, true);
    p.apply(&mut a, &swap(0, 0, 0, 1)).unwrap().commit();
    p.apply(&mut a, &swap(1, 0, 1, 1)).unwrap().commit();
    assert_eq!(a.list(0).unwrap()[0], 2);
    assert_eq!(a.list(0).unwrap()[1], 0);
    assert_eq!(a.list(1).unwrap()[0], 3);
    assert_eq!(a.list(1).unwrap()[1], 1);
}
#[test]
fn list_swap_move_executes_temporarily_with_undo() {
    let (p, mut a) = generated(3, 1, true);
    let original = a.list(0).unwrap().to_vec();
    {
        let candidate = p.apply(&mut a, &swap(0, 0, 0, 2)).unwrap();
        assert_eq!(candidate.list(0).unwrap()[0], original[2]);
        assert_eq!(candidate.list(0).unwrap()[1], original[1]);
        assert_eq!(candidate.list(0).unwrap()[2], original[0]);
    }
    assert_eq!(a.list(0).unwrap(), original);
}
#[test]
fn list_swap_move_temporary_between_entities() {
    let (p, mut a) = generated(4, 2, true);
    let original1 = a.list(0).unwrap().to_vec();
    let original2 = a.list(1).unwrap().to_vec();
    {
        let candidate = p.apply(&mut a, &swap(0, 0, 1, 0)).unwrap();
        assert_eq!(candidate.list(0).unwrap()[0], original2[0]);
        assert_eq!(candidate.list(1).unwrap()[0], original1[0]);
    }
    assert_eq!(a.list(0).unwrap(), original1);
    assert_eq!(a.list(1).unwrap(), original2);
}
#[test]
fn move_from_first_to_last() {
    let (p, mut a) = generated(3, 1, true);
    p.apply(&mut a, &change(0, 0, 0, 2)).unwrap().commit();
    assert_eq!(a.list(0).unwrap(), &[1, 2, 0]);
}
#[test]
fn move_from_last_to_first() {
    let (p, mut a) = generated(3, 1, true);
    p.apply(&mut a, &change(0, 2, 0, 0)).unwrap().commit();
    assert_eq!(a.list(0).unwrap(), &[2, 0, 1]);
}
#[test]
fn move_middle_to_first() {
    let (p, mut a) = generated(3, 1, true);
    p.apply(&mut a, &change(0, 1, 0, 0)).unwrap().commit();
    assert_eq!(a.list(0).unwrap(), &[1, 0, 2]);
}
#[test]
fn move_middle_to_last() {
    let (p, mut a) = generated(3, 1, true);
    p.apply(&mut a, &change(0, 1, 0, 2)).unwrap().commit();
    assert_eq!(a.list(0).unwrap(), &[0, 2, 1]);
}
#[test]
fn move_from_first_entity_to_second_at_beginning() {
    let (p, mut a) = generated(4, 2, true);
    let size1 = a.list(0).unwrap().len();
    let size2 = a.list(1).unwrap().len();
    let moved = a.list(0).unwrap()[0];
    let dest_first = a.list(1).unwrap()[0];
    p.apply(&mut a, &change(0, 0, 1, 0)).unwrap().commit();
    assert_eq!(a.list(0).unwrap().len(), size1 - 1);
    assert_eq!(a.list(1).unwrap().len(), size2 + 1);
    assert_eq!(a.list(1).unwrap()[0], moved);
    assert_eq!(a.list(1).unwrap()[1], dest_first);
}
#[test]
fn move_from_first_entity_to_second_at_end() {
    let (p, mut a) = fixture(vec![0, 1, 2, 3], vec![vec![0, 1], vec![2, 3]]);
    p.apply(&mut a, &change(0, 0, 1, 2)).unwrap().commit();
    assert_eq!(a.list(0).unwrap(), &[1]);
    assert_eq!(a.list(1).unwrap(), &[2, 3, 0]);
}
#[test]
fn move_from_first_entity_to_second_in_middle() {
    let (p, mut a) = fixture(vec![0, 1, 2, 3], vec![vec![0], vec![1, 2, 3]]);
    p.apply(&mut a, &change(0, 0, 1, 1)).unwrap().commit();
    assert!(a.list(0).unwrap().is_empty());
    assert_eq!(a.list(1).unwrap(), &[1, 0, 2, 3]);
}
#[test]
fn move_to_empty_entity() {
    let (p, mut a) = fixture(vec![0, 1], vec![vec![0, 1], vec![]]);
    p.apply(&mut a, &change(0, 0, 1, 0)).unwrap().commit();
    assert_eq!(a.list(0).unwrap(), &[1]);
    assert_eq!(a.list(1).unwrap(), &[0]);
}
#[test]
fn move_last_value_leaving_entity_empty() {
    let (p, mut a) = fixture(vec![0, 1], vec![vec![0], vec![1]]);
    p.apply(&mut a, &change(0, 0, 1, 1)).unwrap().commit();
    assert!(a.list(0).unwrap().is_empty());
    assert_eq!(a.list(1).unwrap(), &[1, 0]);
}
#[test]
fn unassigns_span() {
    let (p, mut a) = fixture(vec![0, 1, 2, 3], vec![vec![0, 1, 2, 3]]);
    let p = p.allowing_unassigned();
    p.apply(
        &mut a,
        &ListMove::Unassign {
            entity: 0,
            start: 1,
            end: 3,
        },
    )
    .unwrap()
    .commit();
    assert_eq!(a.list(0).unwrap(), &[0, 3]);
}
#[test]
fn execute_temporarily_then_undo() {
    let (p, mut a) = fixture(vec![0, 1, 2], vec![vec![0, 1, 2]]);
    let p = p.allowing_unassigned();
    {
        let candidate = p
            .apply(
                &mut a,
                &ListMove::Unassign {
                    entity: 0,
                    start: 0,
                    end: 2,
                },
            )
            .unwrap();
        assert_eq!(candidate.list(0).unwrap(), &[2]);
    }
    assert_eq!(a.list(0).unwrap(), &[0, 1, 2]);
}
