//! Original payload and clone-cost checks for borrowed candidate validation.
use std::{cell::Cell, rc::Rc};
use timetuck::*;
#[derive(Debug)]
struct Value {
    id: usize,
    payload: i64,
    clones: Rc<Cell<usize>>,
}
impl Clone for Value {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            id: self.id,
            payload: self.payload,
            clones: self.clones.clone(),
        }
    }
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
fn value(id: usize, payload: i64, c: &Rc<Cell<usize>>) -> Value {
    Value {
        id,
        payload,
        clones: c.clone(),
    }
}
fn problem(c: &Rc<Cell<usize>>) -> ListProblem<Value> {
    ListProblem::new(
        (0..66).map(|i| value(i, 1000 + i as i64, c)).collect(),
        vec![
            ListEntity::new(vec![value(0, 2000, c), value(1, 2001, c)]),
            ListEntity::new(vec![]),
            ListEntity::new((2..64).map(|i| value(i, 2000 + i as i64, c)).collect())
                .with_pinned(true),
        ],
    )
    .unwrap()
    .allowing_unassigned()
}
fn pos(entity: usize, index: usize) -> Option<ListPosition> {
    Some(ListPosition { entity, index })
}
#[test]
fn changes_clone_only_exposed_payloads_even_with_large_untouched_lists() {
    let clones = Rc::new(Cell::new(0));
    let p = problem(&clones);
    let a = p.initial_assignment();
    // Assigned changes/unassignment need one metadata value; assignment also owns its move payload.
    for (id, destination, include_noops, expected_count, expected_clones) in [
        (1, pos(1, 0), false, 1, 1),
        (1, None, false, 1, 1),
        (64, pos(1, 0), false, 1, 2),
        (1, pos(0, 1), true, 1, 1),
        (1, pos(0, 2), true, 1, 1),
        (1, pos(0, 2), false, 0, 0),
        (64, None, true, 1, 1),
        (64, None, false, 0, 0),
    ] {
        clones.set(0);
        let candidates = list_value_changes(
            &p,
            &a,
            &p.values()[id..id + 1],
            &[destination],
            ListChangePairing::Cartesian,
            include_noops,
        )
        .unwrap();
        assert_eq!(clones.get(), 0);
        let candidates: Vec<_> = candidates.collect();
        assert_eq!(candidates.len(), expected_count);
        assert_eq!(clones.get(), expected_clones);
        for candidate in candidates {
            assert_eq!(candidate.value().payload, 1000 + id as i64);
            if let ListMove::Assign { value, .. } = candidate.movement() {
                assert_eq!(value.payload, 1064);
            }
            let mut copy = a.clone();
            p.apply(&mut copy, candidate.movement()).unwrap().commit();
            if let Some(element) = copy.element(&p.values()[1]) {
                assert_eq!(
                    copy.lists()[element.position.entity][element.position.index].payload,
                    2001
                );
            }
        }
    }
    assert_eq!(a.lists()[0][1].payload, 2001);
}
#[test]
fn swaps_clone_only_replacement_payloads_and_preserve_noop_multiplicity() {
    let clones = Rc::new(Cell::new(0));
    let p = problem(&clones);
    let a = p.initial_assignment();
    for (left, right, include_noops, count, cost) in [
        (0, 1, false, 1, 0),
        (1, 64, false, 1, 1),
        (64, 1, false, 1, 1),
        (64, 65, true, 1, 0),
        (0, 0, true, 1, 0),
        (64, 65, false, 0, 0),
    ] {
        clones.set(0);
        let candidates = list_value_swap_moves(
            &p,
            &a,
            &p.values()[left..left + 1],
            &p.values()[right..right + 1],
            ListSwapPairing::Cartesian,
            include_noops,
        )
        .unwrap();
        assert_eq!(clones.get(), 0);
        let candidates: Vec<_> = candidates.collect();
        assert_eq!(candidates.len(), count);
        assert_eq!(clones.get(), cost);
        for candidate in candidates {
            let mut copy = a.clone();
            p.apply(&mut copy, &candidate).unwrap().commit();
            if left == 0 && right == 1 {
                assert_eq!(copy.lists()[0][0].payload, 2001);
                assert_eq!(copy.lists()[0][1].payload, 2000);
            }
            if (left == 64 || right == 64)
                && let Some(at) = copy.location(&p.values()[64])
            {
                assert_eq!(copy.lists()[at.entity][at.index].payload, 1064);
            }
        }
    }
    let duplicates = vec![p.values()[0].clone(); 3];
    clones.set(0);
    assert_eq!(
        list_value_swap_moves(
            &p,
            &a,
            &duplicates,
            &duplicates,
            ListSwapPairing::Cartesian,
            true
        )
        .unwrap()
        .count(),
        9
    );
    assert_eq!(clones.get(), 0);
}
