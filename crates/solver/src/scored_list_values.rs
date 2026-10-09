//! Value-tabu metadata for candidates already validated by a score director.
mod compound;
use crate::{
    ListAssignment, ListMove, ListProblem, MixedAssignment, MixedMove, MixedPlanningValue,
    MixedProblem,
};

impl<V: Eq + Clone> ListProblem<V> {
    // Internal contract: movement was successfully applied/scored from `before`.
    // Public planning_values must retain full model, range, pin and cycle checks.
    pub(crate) fn scored_planning_values(
        &self,
        before: &ListAssignment<V>,
        movement: &ListMove<V>,
    ) -> Vec<V> {
        let mut movement = movement;
        while let ListMove::Composite(children) = movement {
            match children.as_slice() {
                [] => return vec![],
                [child] => movement = child,
                _ => return compound::capture(before, movement),
            }
        }
        let mut values = Vec::new();
        let mut record = |value: &V| {
            if !values.contains(value) {
                values.push(value.clone());
            }
        };
        match movement {
            ListMove::Assign { value, .. } => record(value),
            ListMove::Change { source, .. } => record(&before.lists()[source.entity][source.index]),
            ListMove::Swap { left, right } => {
                record(&before.lists()[left.entity][left.index]);
                record(&before.lists()[right.entity][right.index]);
            }
            ListMove::SubListChange {
                entity, start, end, ..
            }
            | ListMove::Unassign { entity, start, end } => {
                for value in &before.lists()[*entity][*start..*end] {
                    record(value);
                }
            }
            ListMove::SubListSwap { left, right, .. } => {
                for range in [left, right] {
                    for value in &before.lists()[range.entity][range.start..range.end] {
                        record(value);
                    }
                }
            }
            ListMove::Composite(_) => unreachable!("handled above"),
        }
        values
    }
}
impl<B: Eq + Clone, V: Eq + Clone> MixedProblem<B, V> {
    // Both components were jointly validated/scored. Their metadata namespaces
    // remain separate, without cloning and replaying the whole mixed assignment.
    pub(crate) fn scored_planning_values(
        &self,
        before: &MixedAssignment<B, V>,
        movement: &MixedMove<B, V>,
    ) -> Vec<MixedPlanningValue<B, V>> {
        let mut values: Vec<_> = before
            .basic()
            .planning_values(&movement.basic)
            .expect("scored basic move has valid value metadata")
            .into_iter()
            .map(MixedPlanningValue::Basic)
            .collect();
        values.extend(
            self.lists()
                .scored_planning_values(before.lists(), &movement.lists)
                .into_iter()
                .map(MixedPlanningValue::List),
        );
        values
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FiniteProblem, ListEntity, ListPosition, ListRange, Move, Variable};
    use std::{cell::Cell, rc::Rc};
    fn pos(entity: usize, index: usize) -> ListPosition {
        ListPosition { entity, index }
    }
    #[test]
    fn valid_primitive_metadata_matches_full_validation_across_pins_and_ranges() {
        let mut exercised = 0;
        for pins in [false, true] {
            let p = ListProblem::new(
                (0..5).collect(),
                vec![
                    ListEntity::new(vec![0, 1]).with_pinned_prefix(usize::from(pins)),
                    ListEntity::new(vec![2, 3]).with_value_range(vec![1, 2, 3, 4]),
                ],
            )
            .unwrap()
            .allowing_unassigned();
            let a = p.initial_assignment();
            let mut moves = vec![];
            for e in 0..2 {
                for i in 0..=2 {
                    moves.push(ListMove::Assign {
                        value: 4,
                        destination: pos(e, i),
                    });
                    for f in 0..2 {
                        for j in 0..=2 {
                            moves.push(ListMove::Change {
                                source: pos(e, i),
                                destination: pos(f, j),
                            });
                            moves.push(ListMove::Swap {
                                left: pos(e, i),
                                right: pos(f, j),
                            });
                            for end in i..=2 {
                                for reverse in [false, true] {
                                    moves.push(ListMove::SubListChange {
                                        entity: e,
                                        start: i,
                                        end,
                                        destination: pos(f, j),
                                        reverse,
                                    });
                                    for right_end in j..=2 {
                                        moves.push(ListMove::SubListSwap {
                                            left: ListRange {
                                                entity: e,
                                                start: i,
                                                end,
                                            },
                                            right: ListRange {
                                                entity: f,
                                                start: j,
                                                end: right_end,
                                            },
                                            reverse,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    for end in i..=2 {
                        moves.push(ListMove::Unassign {
                            entity: e,
                            start: i,
                            end,
                        });
                    }
                }
            }
            for movement in moves {
                let expected = p.planning_values(&a, &movement);
                let mut applied = a.clone();
                assert_eq!(p.apply(&mut applied, &movement).is_ok(), expected.is_ok());
                if let Ok(expected) = expected {
                    assert_eq!(p.scored_planning_values(&a, &movement), expected);
                    assert_eq!(
                        p.scored_planning_values(
                            &a,
                            &ListMove::Composite(vec![ListMove::Composite(vec![movement])])
                        ),
                        expected
                    );
                    exercised += 1;
                }
                assert_eq!(applied, a);
            }
        }
        assert!(exercised > 100);
    }
    #[derive(Debug)]
    struct Value(usize, Rc<Cell<usize>>);
    impl PartialEq for Value {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl Eq for Value {}
    impl Clone for Value {
        fn clone(&self) -> Self {
            self.1.set(self.1.get() + 1);
            Self(self.0, self.1.clone())
        }
    }
    #[test]
    fn primitive_capture_clones_only_moved_values_and_empty_composites_clone_nothing() {
        let clones = Rc::new(Cell::new(0));
        let values: Vec<_> = (0..1000).map(|i| Value(i, clones.clone())).collect();
        let p = ListProblem::new(
            values.clone(),
            values
                .chunks(10)
                .map(|v| ListEntity::new(v.to_vec()))
                .collect(),
        )
        .unwrap();
        let a = p.initial_assignment();
        let m = ListMove::Change {
            source: pos(0, 0),
            destination: pos(1, 5),
        };
        clones.set(0);
        let expected = p.planning_values(&a, &m).unwrap();
        assert_eq!(clones.get(), 1021);
        clones.set(0);
        assert_eq!(p.scored_planning_values(&a, &m), expected);
        assert_eq!(clones.get(), 1);
        clones.set(0);
        assert!(
            p.scored_planning_values(&a, &ListMove::Composite(vec![]))
                .is_empty()
        );
        assert_eq!(clones.get(), 0);
        assert_eq!(a.lists()[0][0].0, 0);
    }
    #[test]
    fn sequential_composites_and_mixed_namespaces_preserve_capture_order() {
        let p = ListProblem::new(
            vec![0, 1, 2, 3],
            vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![2])],
        )
        .unwrap()
        .allowing_unassigned();
        let a = p.initial_assignment();
        let movement = ListMove::Composite(vec![
            ListMove::Change {
                source: pos(0, 0),
                destination: pos(1, 0),
            },
            ListMove::Swap {
                left: pos(0, 0),
                right: pos(1, 0),
            },
            ListMove::Assign {
                value: 3,
                destination: pos(0, 1),
            },
        ]);
        assert_eq!(p.scored_planning_values(&a, &movement), vec![0, 1, 3]);
        assert_eq!(
            p.scored_planning_values(&a, &movement),
            p.planning_values(&a, &movement).unwrap()
        );
        let mixed = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
            p,
        );
        let a = mixed.initial_assignment();
        let movement = MixedMove {
            basic: Move::Change {
                variable: 0,
                value: Some(1),
            },
            lists: movement,
        };
        assert_eq!(
            mixed.scored_planning_values(&a, &movement),
            mixed.planning_values(&a, &movement).unwrap()
        );
        assert_eq!(
            mixed.scored_planning_values(&a, &movement),
            vec![
                MixedPlanningValue::Basic(Some(1)),
                MixedPlanningValue::List(0),
                MixedPlanningValue::List(1),
                MixedPlanningValue::List(3)
            ]
        );
    }
}
