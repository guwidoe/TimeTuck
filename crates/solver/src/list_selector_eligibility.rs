//! Metadata-only selector prefilters, separate from executable move validation.
use crate::{ListAssignment, ListError, ListRange};
impl ListRange {
    /// Whether two nonempty selector spans can be exchanged geometrically.
    /// Different entities are independent; same-entity spans must not overlap.
    /// Touching endpoints are allowed. This does not read rows or validate
    /// endpoint bounds, pins, membership, dependencies or actual value changes.
    /// A true result must still pass ListProblem's strict move checker/apply.
    pub fn selector_swap_is_doable(self, other: Self) -> bool {
        self.start < self.end
            && other.start < other.end
            && (self.entity != other.entity || self.end <= other.start || other.end <= self.start)
    }
    /// Additionally check incoming values against each receiving entity's range.
    /// Borrow only the two selected slices; do not validate unrelated or staying
    /// values. This permits prefiltering candidates that can repair an initially
    /// invalid receiver membership. Same-entity swaps have no range arrivals and
    /// retain geometry-only semantics, including deferred endpoint bounds.
    /// Reversing the spans does not affect membership eligibility. The callback
    /// receives (receiving_entity, value); it must reflect stable range metadata.
    pub fn selector_swap_is_doable_with_ranges<V>(
        self,
        other: Self,
        assignment: &ListAssignment<V>,
        mut contains: impl FnMut(usize, &V) -> bool,
    ) -> Result<bool, ListError> {
        if !self.selector_swap_is_doable(other) {
            return Ok(false);
        }
        if self.entity == other.entity {
            return Ok(true);
        }
        let slice = |range: Self| -> Result<&[V], ListError> {
            let row = assignment
                .list(range.entity)
                .ok_or(ListError::EntityIndex {
                    entity: range.entity,
                    count: assignment.lists().len(),
                })?;
            row.get(range.start..range.end)
                .ok_or(ListError::InvalidRange {
                    entity: range.entity,
                    start: range.start,
                    end: range.end,
                })
        };
        let left = slice(self)?;
        let right = slice(other)?;
        Ok(left.iter().all(|value| contains(other.entity, value))
            && right.iter().all(|value| contains(self.entity, value)))
    }
}
