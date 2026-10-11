//! Original exact-composite and payload checks for ranked k-opt relocations.
use std::{cell::Cell, rc::Rc};
use timetuck::*;

fn reference<V>(target: &[usize]) -> ListMove<V> {
    let mut current: Vec<_> = (0..target.len()).collect();
    let mut steps = vec![];
    for (index, &wanted) in target.iter().enumerate() {
        let source = current.iter().position(|&value| value == wanted).unwrap();
        if source != index {
            steps.push(ListMove::Change {
                source: ListPosition {
                    entity: 0,
                    index: source,
                },
                destination: ListPosition { entity: 0, index },
            });
            current.remove(source);
            current.insert(index, wanted);
        }
    }
    ListMove::Composite(steps)
}

#[test]
fn sampled_and_exhaustive_candidates_keep_canonical_composites_and_rollback() {
    for size in [32, 129, 257, 1024] {
        for prefix in [0, size / 2, size - 8] {
            let p = ListProblem::new_indexed(
                (0..size + 1).rev().collect(),
                vec![
                    ListEntity::new((0..size).collect()).with_pinned_prefix(prefix),
                    ListEntity::new(vec![size]).with_pinned(true),
                ],
            )
            .unwrap();
            let a = p.initial_assignment();
            for mode in 0..2 {
                let candidates: Vec<_> = if mode == 0 {
                    sample_list_k_opt_moves(
                        &p,
                        &a,
                        ListKOptSampling::new(ListKOptConfig::new(2, 4).unwrap(), 97, 32),
                    )
                    .unwrap()
                    .collect()
                } else {
                    list_k_opt_moves(&p, &a, ListKOptConfig::new(2, 2).unwrap())
                        .unwrap()
                        .take(3)
                        .collect()
                };
                assert!(!candidates.is_empty());
                for movement in candidates {
                    let mut next = a.clone();
                    let expected;
                    {
                        let trial = p.apply(&mut next, &movement).unwrap();
                        expected = reference::<usize>(&trial.lists()[0]);
                        assert_eq!(&trial.lists()[0][..prefix], &a.lists()[0][..prefix]);
                        assert_eq!(trial.lists()[1], a.lists()[1]);
                    }
                    assert_eq!(next, a);
                    assert_eq!(movement, expected);
                }
            }
        }
    }
}

#[derive(Debug)]
struct Value {
    id: usize,
    payload: usize,
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
impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.id.hash(h)
    }
}
#[test]
fn direct_edge_reconnection_preserves_assigned_payloads_without_cloning() {
    let clones = Rc::new(Cell::new(0));
    let value = |id, payload| Value {
        id,
        payload,
        clones: clones.clone(),
    };
    let size = 513;
    let p = ListProblem::new_indexed(
        (0..size).map(|id| value(id, 0)).collect(),
        vec![
            ListEntity::new((0..size).map(|id| value(id, id + 1000)).collect())
                .with_pinned_prefix(1),
        ],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    clones.set(0);
    let movement = p
        .k_opt_move(&a, 0, &[(3, 4), (500, 501)], &[(3, 500), (4, 501)], 2)
        .unwrap();
    assert_eq!(clones.get(), 0);
    let target: Vec<_> = (0..4).chain((4..=500).rev()).chain(501..size).collect();
    assert_eq!(movement, reference(&target));
    let before: Vec<_> = a.lists()[0].iter().map(|v| (v.id, v.payload)).collect();
    {
        let trial = p.apply(&mut a, &movement).unwrap();
        assert_eq!(
            trial.lists()[0]
                .iter()
                .map(|v| (v.id, v.payload))
                .collect::<Vec<_>>(),
            target.iter().map(|&id| (id, id + 1000)).collect::<Vec<_>>()
        );
    }
    assert_eq!(
        a.lists()[0]
            .iter()
            .map(|v| (v.id, v.payload))
            .collect::<Vec<_>>(),
        before
    );
}
