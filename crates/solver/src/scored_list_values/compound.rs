//! Replay validated compound coordinates on borrowed, lazily copied list rows.
use crate::{ListAssignment, ListMove};
use std::collections::BTreeMap;

struct MetadataDraft<'a, V> {
    before: &'a ListAssignment<V>,
    rows: BTreeMap<usize, Vec<&'a V>>,
    captured: Vec<&'a V>,
}
impl<'a, V: Eq> MetadataDraft<'a, V> {
    fn row(&mut self, entity: usize) -> &mut Vec<&'a V> {
        self.rows
            .entry(entity)
            .or_insert_with(|| self.before.lists()[entity].iter().collect())
    }
    fn record(&mut self, value: &'a V) {
        if !self.captured.contains(&value) {
            self.captured.push(value);
        }
    }
    fn apply(&mut self, movement: &'a ListMove<V>) {
        let mut pending = vec![movement];
        while let Some(movement) = pending.pop() {
            match movement {
                ListMove::Assign { value, destination } => {
                    self.record(value);
                    self.row(destination.entity)
                        .insert(destination.index, value);
                }
                ListMove::Change {
                    source,
                    destination,
                } => {
                    let value = self.row(source.entity).remove(source.index);
                    self.record(value);
                    self.row(destination.entity)
                        .insert(destination.index, value);
                }
                ListMove::Swap { left, right } => {
                    let a = self.row(left.entity)[left.index];
                    let b = self.row(right.entity)[right.index];
                    self.record(a);
                    self.record(b);
                    self.row(left.entity)[left.index] = b;
                    self.row(right.entity)[right.index] = a;
                }
                ListMove::SubListChange {
                    entity,
                    start,
                    end,
                    destination,
                    reverse,
                } => {
                    let mut span: Vec<_> = self.row(*entity).drain(*start..*end).collect();
                    for &value in &span {
                        self.record(value);
                    }
                    if *reverse {
                        span.reverse();
                    }
                    self.row(destination.entity)
                        .splice(destination.index..destination.index, span);
                }
                ListMove::SubListSwap {
                    left,
                    right,
                    reverse,
                } => {
                    let mut a = self.row(left.entity)[left.start..left.end].to_vec();
                    let mut b = self.row(right.entity)[right.start..right.end].to_vec();
                    for &value in a.iter().chain(&b) {
                        self.record(value);
                    }
                    if *reverse {
                        a.reverse();
                        b.reverse();
                    }
                    // Same-list ranges use pre-move coordinates, even when lengths
                    // differ. Replace the higher range before shifting its prefix.
                    if left.entity == right.entity && left.start < right.start {
                        self.row(right.entity).splice(right.start..right.end, a);
                        self.row(left.entity).splice(left.start..left.end, b);
                    } else {
                        self.row(left.entity).splice(left.start..left.end, b);
                        self.row(right.entity).splice(right.start..right.end, a);
                    }
                }
                ListMove::Unassign { entity, start, end } => {
                    let removed: Vec<_> = self.row(*entity).drain(*start..*end).collect();
                    for value in removed {
                        self.record(value);
                    }
                }
                ListMove::Composite(children) => pending.extend(children.iter().rev()),
            }
        }
    }
}
// The caller has already scored this move successfully from `before`. Model and
// geometry validation belong to that application, not this metadata-only replay.
pub(super) fn capture<V: Eq + Clone>(before: &ListAssignment<V>, movement: &ListMove<V>) -> Vec<V> {
    let mut draft = MetadataDraft {
        before,
        rows: BTreeMap::new(),
        captured: Vec::new(),
    };
    draft.apply(movement);
    draft.captured.into_iter().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ListEntity, ListPosition, ListProblem, ListRange};
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };
    fn pos(entity: usize, index: usize) -> ListPosition {
        ListPosition { entity, index }
    }
    #[test]
    fn varied_nested_sequences_match_validated_capture_and_final_rows() {
        let p = ListProblem::new(
            (0..12).collect(),
            vec![
                ListEntity::new((0..6).collect()),
                ListEntity::new((6..10).collect()),
            ],
        )
        .unwrap()
        .allowing_unassigned();
        let before = p.initial_assignment();
        let mut seed = 719u64;
        let mut draw = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 32) as usize
        };
        let mut kinds = [0; 6];
        for _ in 0..200 {
            let mut current = before.clone();
            let mut children = vec![];
            for _ in 0..12 {
                let e = draw() % 2;
                let f = draw() % 2;
                let n = current.lists()[e].len();
                let m = current.lists()[f].len();
                let kind = draw() % 6;
                let movement = match kind {
                    0 => ListMove::Assign {
                        value: draw() % 12,
                        destination: pos(f, draw() % (m + 1)),
                    },
                    1 if n > 0 => ListMove::Change {
                        source: pos(e, draw() % n),
                        destination: pos(f, draw() % (m + 1 - usize::from(e == f))),
                    },
                    2 if n > 0 && m > 0 => ListMove::Swap {
                        left: pos(e, draw() % n),
                        right: pos(f, draw() % m),
                    },
                    3 if n > 0 => {
                        let start = draw() % n;
                        let end = start + 1 + draw() % (n - start);
                        ListMove::SubListChange {
                            entity: e,
                            start,
                            end,
                            destination: pos(
                                f,
                                draw() % (m + 1 - if e == f { end - start } else { 0 }),
                            ),
                            reverse: draw() % 2 == 0,
                        }
                    }
                    4 if n > 0 && m > 0 => {
                        let start = draw() % n;
                        let end = start + 1 + draw() % (n - start);
                        let rs = draw() % m;
                        let re = rs + 1 + draw() % (m - rs);
                        ListMove::SubListSwap {
                            left: ListRange {
                                entity: e,
                                start,
                                end,
                            },
                            right: ListRange {
                                entity: f,
                                start: rs,
                                end: re,
                            },
                            reverse: draw() % 2 == 0,
                        }
                    }
                    5 if n > 0 => {
                        let start = draw() % n;
                        ListMove::Unassign {
                            entity: e,
                            start,
                            end: start + 1 + draw() % (n - start),
                        }
                    }
                    _ => continue,
                };
                if let Ok(tx) = p.apply(&mut current, &movement) {
                    tx.commit();
                } else {
                    continue;
                }
                kinds[kind] += 1;
                children.push(ListMove::Composite(vec![
                    ListMove::Composite(vec![]),
                    movement,
                ]));
            }
            let movement = ListMove::Composite(children);
            assert_eq!(
                capture(&before, &movement),
                p.planning_values(&before, &movement).unwrap()
            );
            let mut draft = MetadataDraft {
                before: &before,
                rows: BTreeMap::new(),
                captured: vec![],
            };
            draft.apply(&movement);
            for e in 0..2 {
                assert_eq!(
                    draft.row(e).iter().map(|v| **v).collect::<Vec<_>>(),
                    current.lists()[e]
                );
            }
            assert_eq!(before, p.initial_assignment());
        }
        assert!(kinds.into_iter().all(|n| n > 30));
    }
    #[derive(Debug)]
    struct CompoundValue(usize, Rc<Cell<usize>>, Rc<Cell<Option<usize>>>);
    impl PartialEq for CompoundValue {
        fn eq(&self, r: &Self) -> bool {
            self.0 == r.0
        }
    }
    impl Eq for CompoundValue {}
    impl Clone for CompoundValue {
        fn clone(&self) -> Self {
            assert_ne!(self.2.get(), Some(self.0), "clone failure");
            self.1.set(self.1.get() + 1);
            Self(self.0, self.1.clone(), self.2.clone())
        }
    }
    #[test]
    fn sparse_draft_clones_only_first_touched_values_and_preserves_input_on_panic() {
        let count = Rc::new(Cell::new(0));
        let fail = Rc::new(Cell::new(None));
        let before = ListAssignment::new(
            (0..100)
                .map(|e| {
                    (e * 10..e * 10 + 10)
                        .map(|v| CompoundValue(v, count.clone(), fail.clone()))
                        .collect()
                })
                .collect(),
        );
        let movement = ListMove::Composite(vec![
            ListMove::Change {
                source: pos(0, 0),
                destination: pos(1, 0),
            },
            ListMove::Swap {
                left: pos(0, 0),
                right: pos(1, 0),
            },
            ListMove::Unassign {
                entity: 0,
                start: 0,
                end: 1,
            },
        ]);
        fail.set(Some(999));
        let values = capture(&before, &movement);
        assert_eq!(values.iter().map(|v| v.0).collect::<Vec<_>>(), [0, 1]);
        assert_eq!(count.get(), 2);
        let mut draft = MetadataDraft {
            before: &before,
            rows: BTreeMap::new(),
            captured: vec![],
        };
        draft.apply(&movement);
        assert_eq!(draft.rows.len(), 2);
        fail.set(Some(1));
        assert!(catch_unwind(AssertUnwindSafe(|| capture(&before, &movement))).is_err());
        assert_eq!(
            before.lists()[0].iter().map(|v| v.0).collect::<Vec<_>>(),
            (0..10).collect::<Vec<_>>()
        );
        assert_eq!(before.lists()[1][0].0, 10);
    }
    #[test]
    fn first_touched_payload_survives_unassign_and_equal_identity_reassignment() {
        #[derive(Clone, Debug)]
        struct Tagged(usize, &'static str);
        impl PartialEq for Tagged {
            fn eq(&self, r: &Self) -> bool {
                self.0 == r.0
            }
        }
        impl Eq for Tagged {}
        let before = ListAssignment::new(vec![
            vec![Tagged(1, "original"), Tagged(2, "other")],
            vec![],
        ]);
        let movement = ListMove::Composite(vec![
            ListMove::Unassign {
                entity: 0,
                start: 0,
                end: 1,
            },
            ListMove::Assign {
                value: Tagged(1, "replacement"),
                destination: pos(1, 0),
            },
            ListMove::Swap {
                left: pos(0, 0),
                right: pos(1, 0),
            },
        ]);
        let captured = capture(&before, &movement);
        assert_eq!(
            captured.iter().map(|v| (v.0, v.1)).collect::<Vec<_>>(),
            [(1, "original"), (2, "other")]
        );
        assert_eq!(before.lists()[0][0].1, "original");
    }
}
