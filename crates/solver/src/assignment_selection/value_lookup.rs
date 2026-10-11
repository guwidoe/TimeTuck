//! Per-opening ownership lookup; only large indexed models use promotion.
use crate::{ListAssignment, ListPosition, ListProblem};
pub(super) struct ValueOwnership {
    remaining: usize,
    positions: Option<Vec<Option<ListPosition>>>,
}
impl ValueOwnership {
    pub(super) fn useful<V: Eq + Clone>(p: &ListProblem<V>) -> bool {
        p.values().len() >= 64 && p.has_value_index()
    }
    pub(super) fn new<V: Eq + Clone>(p: &ListProblem<V>, a: &ListAssignment<V>) -> Self {
        // Include registry initialization and empty-row traversal in the budget.
        // Short/first-accepted cursors retain direct lookup without an index.
        // Validated assignments contain at most registry_len values. This
        // conservative budget needs no row walk just to open a short cursor.
        let remaining = p
            .values()
            .len()
            .saturating_mul(2)
            .saturating_add(a.lists().len());
        Self {
            remaining,
            positions: None,
        }
    }
    pub(super) fn location<V: Eq + Clone>(
        &mut self,
        p: &ListProblem<V>,
        a: &ListAssignment<V>,
        id: usize,
    ) -> Option<ListPosition> {
        if let Some(positions) = &self.positions {
            return positions[id];
        }
        if self.remaining == 0 {
            // Install only a complete index if a user Hash/Eq callback unwinds.
            self.positions = Some(p.value_locations(a));
            return self.positions.as_ref().unwrap()[id];
        }
        let value = &p.values()[id];
        for (entity, row) in a.lists().iter().enumerate() {
            self.remaining = self.remaining.saturating_sub(1);
            for (index, candidate) in row.iter().enumerate() {
                self.remaining = self.remaining.saturating_sub(1);
                if candidate == value {
                    return Some(ListPosition { entity, index });
                }
            }
        }
        None
    }
}
