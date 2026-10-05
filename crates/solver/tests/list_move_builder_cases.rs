// Adapted from Community MoveDirectorTest list assignment methods.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;

fn at(index: usize) -> ListPosition {
    ListPosition { entity: 0, index }
}
fn positions(a: &ListAssignment<i32>, expected: &[i32], domain: &[i32]) {
    assert_eq!(a.lists(), &[expected.to_vec()]);
    for value in domain {
        assert_eq!(
            a.location(value),
            expected.iter().position(|v| v == value).map(at)
        );
    }
}
fn run(
    initial: &[i32],
    extra: &[i32],
    edit: impl FnOnce(&mut ListMoveBuilder<'_, i32>),
    expected: &[i32],
) {
    let values: Vec<_> = initial.iter().chain(extra).copied().collect();
    let p = ListProblem::new(values.clone(), vec![ListEntity::new(initial.to_vec())]).unwrap();
    let a = p.initial_assignment();
    let mut draft = ListMoveBuilder::new(&p, &a).unwrap();
    edit(&mut draft);
    positions(draft.assignment(), expected, &values);
    positions(&a, initial, &values);
    let movement = draft.finish();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = ListStreamSources::new(&mut b);
    b.score("Dummy constraint", sources.entities.port(), |_| {
        SimpleScore::new(-1)
    })
    .unwrap();
    let mut director = ListStreamScoreDirector::new(&p, b.build(), sources, a).unwrap();
    {
        let trial = director.apply(&movement).unwrap();
        positions(trial.assignment(), expected, &values);
    }
    positions(director.assignment(), initial, &values);
}

#[test]
fn assign_value_and_set_in_middle() {
    run(
        &[1, 2, 3],
        &[9],
        |d| {
            assert_eq!(d.set_unassigned(9, at(1)).unwrap(), Some(2));
        },
        &[1, 9, 3],
    );
}

#[test]
fn assign_value_and_set_at_start() {
    run(
        &[1, 2],
        &[9],
        |d| {
            assert_eq!(d.set_unassigned(9, at(0)).unwrap(), Some(1));
        },
        &[9, 2],
    );
}

#[test]
fn assign_value_and_set_at_end() {
    run(
        &[1, 2],
        &[9],
        |d| {
            assert_eq!(d.set_unassigned(9, at(2)).unwrap(), None);
        },
        &[1, 2, 9],
    );
}

#[test]
fn assign_value_and_set_on_empty_list() {
    run(
        &[],
        &[9],
        |d| {
            assert_eq!(d.set_unassigned(9, at(0)).unwrap(), None);
        },
        &[9],
    );
}

#[test]
fn assign_value_and_add_to_empty_list() {
    run(
        &[],
        &[9],
        |d| {
            d.assign(9, at(0)).unwrap();
        },
        &[9],
    );
}

#[test]
fn assign_value_and_add_at_start() {
    run(
        &[1, 2],
        &[9],
        |d| {
            d.assign(9, at(0)).unwrap();
        },
        &[9, 1, 2],
    );
}

#[test]
fn assign_value_and_add_in_middle() {
    run(
        &[1, 2, 3],
        &[9],
        |d| {
            d.assign(9, at(1)).unwrap();
        },
        &[1, 9, 2, 3],
    );
}

#[test]
fn assign_value_and_add_at_end() {
    run(
        &[1, 2],
        &[9],
        |d| {
            d.assign(9, at(2)).unwrap();
        },
        &[1, 2, 9],
    );
}

#[test]
fn assign_values_and_add_to_empty_list() {
    run(
        &[],
        &[8, 9],
        |d| {
            d.assign_many(vec![8, 9], at(0)).unwrap();
        },
        &[8, 9],
    );
}

#[test]
fn assign_values_and_add_at_start() {
    run(
        &[1, 2],
        &[8, 9],
        |d| {
            d.assign_many(vec![8, 9], at(0)).unwrap();
        },
        &[8, 9, 1, 2],
    );
}

#[test]
fn assign_values_and_add_in_middle() {
    run(
        &[1, 2, 3],
        &[8, 9],
        |d| {
            d.assign_many(vec![8, 9], at(1)).unwrap();
        },
        &[1, 8, 9, 2, 3],
    );
}

#[test]
fn assign_values_and_add_at_end() {
    run(
        &[1, 2],
        &[8, 9],
        |d| {
            d.assign_many(vec![8, 9], at(2)).unwrap();
        },
        &[1, 2, 8, 9],
    );
}

#[test]
fn assign_value_and_add_fails_when_value_already_assigned() {
    run(
        &[1, 2],
        &[],
        |d| {
            assert_eq!(
                d.assign(1, at(1)).unwrap_err(),
                ListEditError::Model(ListError::AlreadyAssigned)
            );
        },
        &[1, 2],
    );
}

#[test]
fn assign_values_and_add_fails_when_value_already_assigned() {
    run(
        &[1, 2],
        &[9],
        |d| {
            assert_eq!(
                d.assign_many(vec![1, 9], at(1)).unwrap_err(),
                ListEditError::Model(ListError::AlreadyAssigned)
            );
        },
        &[1, 2],
    );
}

#[test]
fn assign_value_and_set_fails_when_value_already_assigned() {
    run(
        &[1, 2],
        &[],
        |d| {
            assert_eq!(
                d.set_unassigned(1, at(1)).unwrap_err(),
                ListEditError::Model(ListError::AlreadyAssigned)
            );
        },
        &[1, 2],
    );
}
