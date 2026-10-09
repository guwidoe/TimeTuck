//! Original allocation/payload regression for k-opt relocation generation.
use std::{cell::Cell, rc::Rc};
use timetuck::*;
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
#[test]
fn scratch_tracks_only_suffix_indices_and_preserves_assigned_payloads() {
    let clones = Rc::new(Cell::new(0));
    let value = |id, payload| Value {
        id,
        payload,
        clones: clones.clone(),
    };
    let p = ListProblem::new(
        (0..134).map(|i| value(i, 0)).collect(),
        vec![
            ListEntity::new((0..3).map(|i| value(i, i + 1000)).collect()),
            ListEntity::new((3..6).map(|i| value(i, i + 1000)).collect()),
            ListEntity::new((6..134).map(|i| value(i, i + 1000)).collect()).with_pinned(true),
        ],
    )
    .unwrap();
    let before = p.initial_assignment();
    for lengths in [[0, 6], [6, 0], [2, 4], [3, 3]] {
        clones.set(0);
        let movement = p
            .k_opt_repartition_move(
                &before,
                &[0, 1],
                &[(1, 2), (4, 5)],
                &[(1, 4), (2, 5)],
                (0, 1),
                &lengths,
            )
            .unwrap();
        // One complete validation snapshot plus replacement suffixes. No second
        // full clone and no payload copies for the relocation scratch state.
        assert_eq!(clones.get(), 134 + 6);
        let mut candidate = before.clone();
        {
            let trial = p.apply(&mut candidate, &movement).unwrap();
            assert_eq!(trial.lists()[0].len(), lengths[0]);
            assert_eq!(trial.lists()[1].len(), lengths[1]);
            assert_eq!(
                trial.lists()[..2]
                    .iter()
                    .flatten()
                    .map(|v| v.id)
                    .collect::<Vec<_>>(),
                vec![0, 1, 4, 3, 2, 5]
            );
            assert_eq!(trial.lists()[2], before.lists()[2]);
            for v in trial.lists().iter().flatten() {
                assert_eq!(v.payload, v.id + 1000);
            }
        }
        assert_eq!(candidate, before);
    }
}
