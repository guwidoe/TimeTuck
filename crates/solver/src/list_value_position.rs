//! Build list metadata using a position already found in the current assignment.
use crate::{ListAssignment, ListPosition, ListValueRow};
impl<V: Clone> ListValueRow<V> {
    // Callers must derive the position from this same immutable assignment.
    // None denotes an unassigned value; this helper performs no identity lookup.
    pub(crate) fn from_known_position(
        index: usize,
        value: &V,
        assignment: &ListAssignment<V>,
        position: Option<ListPosition>,
    ) -> Self {
        let row = position.map(|p| (assignment.lists()[p.entity].as_slice(), p.index));
        Self {
            range_index: index,
            value: value.clone(),
            position,
            previous: row
                .and_then(|(r, i)| i.checked_sub(1).and_then(|j| r.get(j)))
                .cloned(),
            next: row.and_then(|(r, i)| r.get(i + 1)).cloned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn positions_keep_registry_payload_and_actual_neighbor_payloads() {
        #[derive(Clone, Debug)]
        struct Value {
            id: usize,
            payload: &'static str,
        }
        impl PartialEq for Value {
            fn eq(&self, other: &Self) -> bool {
                self.id == other.id
            }
        }
        impl Eq for Value {}
        let assignment = ListAssignment::new(vec![
            vec![
                Value {
                    id: 0,
                    payload: "a",
                },
                Value {
                    id: 1,
                    payload: "b",
                },
                Value {
                    id: 2,
                    payload: "c",
                },
            ],
            vec![],
            vec![Value {
                id: 3,
                payload: "d",
            }],
        ]);
        for (id, position) in [
            (
                0,
                Some(ListPosition {
                    entity: 0,
                    index: 0,
                }),
            ),
            (
                1,
                Some(ListPosition {
                    entity: 0,
                    index: 1,
                }),
            ),
            (
                2,
                Some(ListPosition {
                    entity: 0,
                    index: 2,
                }),
            ),
            (
                3,
                Some(ListPosition {
                    entity: 2,
                    index: 0,
                }),
            ),
            (4, None),
        ] {
            let value = Value {
                id,
                payload: "registry",
            };
            let expected = ListValueRow::from_assignment(id, &value, &assignment);
            let actual = ListValueRow::from_known_position(id, &value, &assignment, position);
            assert_eq!(actual, expected);
            assert_eq!(actual.value.payload, "registry");
            assert_eq!(
                actual.previous.as_ref().map(|v| v.payload),
                expected.previous.as_ref().map(|v| v.payload)
            );
            assert_eq!(
                actual.next.as_ref().map(|v| v.payload),
                expected.next.as_ref().map(|v| v.payload)
            );
        }
    }
}
