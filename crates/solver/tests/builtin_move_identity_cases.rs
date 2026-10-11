// Adapted from Timefold Community builtin move tests at
//8b815bc6b5f3eb81feedc94dea0885a93b267762. Apache-2.0.
// See THIRD_PARTY_NOTICES.md and compat/provenance.json.
// Native moves identify owners by slots; fixture lookup resolves those slots to
// distinct Rc entity identities. Explicit list/basic bindings replace reflection.
use std::{collections::BTreeMap, rc::Rc};
use timetuck::*;
type Entity = Rc<(String, Vec<Rc<String>>)>;
fn value(code: &str) -> Rc<String> {
    Rc::new(code.to_owned())
}
fn entity(code: &str, values: Vec<Rc<String>>) -> Entity {
    Rc::new((code.to_owned(), values))
}
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn change(source: ListPosition, destination: ListPosition) -> ListMove<Rc<String>> {
    ListMove::Change {
        source,
        destination,
    }
}
fn same(actual: &Entity, expected: &Entity) {
    assert!(Rc::ptr_eq(actual, expected));
}
fn members(actual: &[usize], registry: &BTreeMap<usize, Entity>, expected: &[Entity]) {
    assert_eq!(actual.len(), expected.len());
    for expected in expected {
        assert_eq!(
            actual
                .iter()
                .filter(|i| Rc::ptr_eq(&registry[i], expected))
                .count(),
            1
        );
    }
}
fn registry(entities: &[Entity]) -> BTreeMap<usize, Entity> {
    entities
        .iter()
        .enumerate()
        .map(|(i, e)| (i, e.clone()))
        .collect()
}
// Preserve fixture rows and register every fixture value in a real native model.
fn model(entities: &[Entity], optional: bool) -> ListProblem<Rc<String>> {
    let values = entities.iter().flat_map(|e| e.1.iter().cloned()).collect();
    let p = ListProblem::new(
        values,
        entities
            .iter()
            .map(|e| ListEntity::new(e.1.clone()))
            .collect(),
    )
    .unwrap();
    if optional { p.allowing_unassigned() } else { p }
}
fn rebase(
    m: &ListMove<Rc<String>>,
    destination: &[Entity],
) -> (ListMove<Rc<String>>, BTreeMap<usize, Entity>) {
    let registry = destination
        .iter()
        .enumerate()
        .map(|(i, e)| (i, e.clone()))
        .collect();
    (
        rebase_list_move(m, Ok::<_, ()>, |v| Ok(v.clone())).unwrap(),
        registry,
    )
}
#[test]
fn list_change_planning_entities_same_owner() {
    let e = entity("Entity", vec![value("A")]);
    let entities = [e.clone()];
    let _p = model(&entities, false);
    let m = change(pos(0, 0), pos(0, 0));
    members(&m.touched_entities(), &registry(&entities), &[e]);
}
#[test]
fn list_change_planning_entities_different_owners() {
    let a = entity("Entity1", vec![value("A")]);
    let b = entity("Entity2", vec![value("B")]);
    let entities = [a.clone(), b.clone()];
    let _p = model(&entities, false);
    members(
        &change(pos(0, 0), pos(1, 0)).touched_entities(),
        &registry(&entities),
        &[a, b],
    );
}
#[test]
fn list_change_source_and_destination_properties() {
    let a = entity("Entity1", vec![value("A")]);
    let b = entity("Entity2", vec![value("B")]);
    let entities = [a.clone(), b.clone()];
    let _p = model(&entities, false);
    let ListMove::Change {
        source,
        destination,
    } = change(pos(0, 0), pos(1, 1))
    else {
        unreachable!()
    };
    same(&entities[source.entity], &a);
    assert_eq!(source.index, 0);
    same(&entities[destination.entity], &b);
    assert_eq!(destination.index, 1);
}
#[test]
fn list_change_rebase_different_owners() {
    let va = value("A");
    let vb = value("B");
    let a = entity("Entity1", vec![va.clone()]);
    let b = entity("Entity2", vec![vb.clone()]);
    // Source ListChange fixture shares values with the new destination owners.
    let da = entity("Entity1", vec![va]);
    let db = entity("Entity2", vec![vb]);
    let _p = model(&[a, b], false);
    let _d = model(&[da.clone(), db.clone()], false);
    let (m, lookup) = rebase(&change(pos(0, 0), pos(1, 1)), &[da.clone(), db.clone()]);
    let ListMove::Change {
        source,
        destination,
    } = m
    else {
        unreachable!()
    };
    same(&lookup[&source.entity], &da);
    same(&lookup[&destination.entity], &db);
    assert_eq!(source.index, 0);
    assert_eq!(destination.index, 1);
}
#[test]
fn list_change_rebase_same_owner() {
    let v = value("A");
    let original = entity("Entity", vec![v.clone()]);
    let target = entity("Entity", vec![v]);
    let _p = model(&[original], false);
    let _d = model(std::slice::from_ref(&target), false);
    let (m, lookup) = rebase(&change(pos(0, 0), pos(0, 0)), std::slice::from_ref(&target));
    let ListMove::Change {
        source,
        destination,
    } = m
    else {
        unreachable!()
    };
    same(&lookup[&source.entity], &target);
    same(&lookup[&destination.entity], &target);
}
#[test]
fn sublist_change_rebase_owners() {
    let originals = [
        entity("EntityA", vec![value("A")]),
        entity("EntityB", vec![value("B")]),
    ];
    let targets = [
        entity("EntityA", vec![value("A")]),
        entity("EntityB", vec![value("B")]),
    ];
    let _p = model(&originals, false);
    let _d = model(&targets, false);
    let original = ListMove::SubListChange {
        entity: 0,
        start: 0,
        end: 1,
        destination: pos(1, 0),
        reverse: false,
    };
    let (m, lookup) = rebase(&original, &targets);
    let ListMove::SubListChange {
        entity,
        destination,
        ..
    } = m
    else {
        unreachable!()
    };
    same(&lookup[&entity], &targets[0]);
    same(&lookup[&destination.entity], &targets[1]);
}
#[test]
fn sublist_swap_rebase_owners() {
    let originals = [
        entity("EntityA", vec![value("A")]),
        entity("EntityB", vec![value("B")]),
    ];
    let targets = [
        entity("EntityA", vec![value("A")]),
        entity("EntityB", vec![value("B")]),
    ];
    let _p = model(&originals, false);
    let _d = model(&targets, false);
    let original = ListMove::SubListSwap {
        left: ListRange {
            entity: 0,
            start: 0,
            end: 1,
        },
        right: ListRange {
            entity: 1,
            start: 0,
            end: 1,
        },
        reverse: false,
    };
    let (m, lookup) = rebase(&original, &targets);
    let ListMove::SubListSwap { left, right, .. } = m else {
        unreachable!()
    };
    same(&lookup[&left.entity], &targets[0]);
    same(&lookup[&right.entity], &targets[1]);
}
#[test]
fn sublist_unassign_planning_entities() {
    let e = entity("Entity", vec![value("A"), value("B")]);
    let entities = [e.clone()];
    let _p = model(&entities, true);
    let m: ListMove<Rc<String>> = ListMove::Unassign {
        entity: 0,
        start: 0,
        end: 1,
    };
    members(&m.touched_entities(), &registry(&entities), &[e]);
}
#[test]
fn sublist_unassign_rebase_owner() {
    let original = entity("Entity", vec![value("A")]);
    let target = entity("Entity", vec![value("A")]);
    let _p = model(&[original], true);
    let _d = model(std::slice::from_ref(&target), true);
    let (m, lookup) = rebase(
        &ListMove::Unassign {
            entity: 0,
            start: 0,
            end: 1,
        },
        std::slice::from_ref(&target),
    );
    let ListMove::Unassign { entity, .. } = m else {
        unreachable!()
    };
    same(&lookup[&entity], &target);
}
#[test]
fn pillar_swap_planning_entities() {
    let entities = [
        entity("e1", vec![]),
        entity("e2", vec![]),
        entity("e3", vec![]),
    ];
    let assignment: Assignment<Rc<String>> = Assignment::new(vec![None, None, None]);
    let m = Pillar::new([0, 1])
        .unwrap()
        .swap_move(&Pillar::new([2]).unwrap(), &assignment)
        .unwrap();
    members(&m.touched_variables(), &registry(&entities), &entities);
}
#[test]
fn pillar_swap_rebase_planning_entities() {
    let originals = [entity("e1", vec![]), entity("e2", vec![])];
    let targets = [entity("e1", vec![]), entity("e2", vec![])];
    for i in 0..2 {
        assert!(!Rc::ptr_eq(&originals[i], &targets[i]));
    }
    let assignment: Assignment<Rc<String>> = Assignment::new(vec![None, None]);
    let original = Pillar::new([0])
        .unwrap()
        .swap_move(&Pillar::new([1]).unwrap(), &assignment)
        .unwrap();
    let m: Move<Rc<String>> = rebase_basic_move(&original, Ok::<_, ()>, |v| Ok(v.clone())).unwrap();
    let lookup = targets
        .iter()
        .enumerate()
        .map(|(i, e)| (i, e.clone()))
        .collect();
    members(&m.touched_variables(), &lookup, &targets);
}
