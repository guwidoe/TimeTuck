//! Full pinned Community method adaptations, Apache-2.0; see provenance.
#[path = "fixtures/virtual_seam_fixture.rs"]
mod fixture;
use fixture::*;
use std::{cell::RefCell, sync::Arc};
#[test]
fn multi_entity_three_opt() {
    let schema = schema();
    let field = schema
        .entity::<timetuck::ListEntity<Token>>()
        .unwrap()
        .variable("values")
        .unwrap();
    let v = values(6);
    let p = problem(&v, &[&[1, 2, 3, 6], &[4, 5]], &[0, 0]);
    let mut a = p.initial_assignment();
    let plan = from_edges(
        &p,
        &a,
        &row(&v, &[6, 1, 2, 3, 4, 5]),
        &row(&v, &[1, 3, 2, 5, 4, 6]),
    );
    execute(
        &p,
        &mut a,
        &plan,
        field,
        &[row(&v, &[1, 5]), row(&v, &[2, 6, 4, 3])],
        &[(0, 0, 4, 2), (1, 0, 2, 4)],
    );
}
#[test]
fn multi_entity_three_opt_pinned() {
    let schema = schema();
    let field = schema
        .entity::<timetuck::ListEntity<Token>>()
        .unwrap()
        .variable("values")
        .unwrap();
    let v = values(7);
    let p = problem(&v, &[&[1, 2, 3, 6, 7], &[4, 5]], &[1, 0]);
    let mut a = p.initial_assignment();
    let plan = from_edges(
        &p,
        &a,
        &row(&v, &[7, 2, 3, 6, 4, 5]),
        &row(&v, &[2, 6, 3, 5, 4, 7]),
    );
    execute(
        &p,
        &mut a,
        &plan,
        field,
        &[row(&v, &[1, 2, 5]), row(&v, &[3, 7, 4, 6])],
        &[(0, 1, 5, 3), (1, 0, 2, 4)],
    );
    assert!(Arc::ptr_eq(&a.lists()[0][0], &v[0]));
}
#[test]
fn multi_entity_three_opt_rebase() {
    let schema = schema();
    let field = schema
        .entity::<timetuck::ListEntity<Token>>()
        .unwrap()
        .variable("values")
        .unwrap();
    let source = values(6);
    let destination = values(6);
    for (a, b) in source.iter().zip(&destination) {
        assert_eq!(a.code, b.code);
        assert!(!Arc::ptr_eq(a, b));
        assert_ne!(a, b);
    }
    let p = problem(&source, &[&[1, 2, 3, 6], &[4, 5]], &[0, 0]);
    let a = p.initial_assignment();
    let dest = problem(&destination, &[&[1, 2, 3, 6], &[4, 5]], &[0, 0]);
    let mut dest_assignment = dest.initial_assignment();
    assert!(!std::ptr::eq(&p.entities()[0], &dest.entities()[0]));
    assert!(!std::ptr::eq(&p.entities()[1], &dest.entities()[1]));
    let plan = from_edges(
        &p,
        &a,
        &row(&source, &[6, 1, 2, 3, 4, 5]),
        &row(&source, &[1, 3, 2, 5, 4, 6]),
    );
    let looked_up = RefCell::new(Vec::new());
    let entities = RefCell::new(Vec::new());
    let rebased = plan
        .rebase(
            &dest,
            &dest_assignment,
            |entity| {
                entities.borrow_mut().push(entity);
                Some(entity)
            },
            |value| {
                let index = source.iter().position(|v| Arc::ptr_eq(v, value))?;
                looked_up.borrow_mut().push(index);
                Some(destination[index].clone())
            },
        )
        .unwrap();
    assert_eq!(*entities.borrow(), [0, 1]);
    assert_eq!(*looked_up.borrow(), [5, 0, 1, 2, 3, 4]);
    assert_eq!(a, p.initial_assignment());
    execute(
        &dest,
        &mut dest_assignment,
        &rebased,
        field,
        &[row(&destination, &[1, 5]), row(&destination, &[2, 6, 4, 3])],
        &[(0, 0, 4, 2), (1, 0, 2, 4)],
    );
    for value in dest_assignment.lists().iter().flatten() {
        assert!(destination.iter().any(|v| Arc::ptr_eq(v, value)));
        assert!(!source.iter().any(|v| Arc::ptr_eq(v, value)));
    }
}
