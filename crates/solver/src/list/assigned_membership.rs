//! Adaptive membership for one validated, exclusively owned application trial.
use super::{ListAssignment, ListProblem};
use std::collections::HashSet;

pub(super) struct AssignedMembership {
    enabled: bool,
    queries: usize,
    remaining: usize,
    members: Option<HashSet<usize>>,
}
impl AssignedMembership {
    pub(super) fn new<V: Eq + Clone>(problem: &ListProblem<V>, assignments: usize) -> Self {
        Self {
            enabled: assignments >= 256 && problem.has_value_index(),
            queries: 0,
            remaining: assignments,
            members: None,
        }
    }
    pub(super) fn contains<V: Eq + Clone>(
        &mut self,
        problem: &ListProblem<V>,
        current: &ListAssignment<V>,
        value: &V,
    ) -> bool {
        if !self.enabled {
            return current.location(value).is_some();
        }
        self.queries += 1;
        self.remaining -= 1;
        // Wait for repeated membership queries and enough remaining work. Retry
        // at powers of two when reconstruction starts with mostly empty lists.
        // A sparse registry consumes storage for assigned IDs and remaining insertions, never the full domain.
        if self.members.is_none()
            && self.remaining >= 192
            && self.queries >= 8
            && self.queries.is_power_of_two()
            && current.lists().len() + current.lists().iter().map(Vec::len).sum::<usize>() >= 512
        {
            let assigned = current.lists().iter().map(Vec::len).sum::<usize>();
            let mut members = HashSet::with_capacity(assigned + self.remaining + 1);
            members.extend(current.lists().iter().flatten().map(|v| {
                problem
                    .value_index(v)
                    .expect("validated intermediate assignment")
            }));
            self.members = Some(members);
        }
        if let Some(members) = &self.members {
            problem
                .value_index(value)
                .is_some_and(|id| members.contains(&id))
        } else {
            current.location(value).is_some()
        }
    }
    pub(super) fn insert<V: Eq + Clone>(&mut self, problem: &ListProblem<V>, value: &V) {
        if let Some(members) = &mut self.members {
            members.insert(
                problem
                    .value_index(value)
                    .expect("validated assigned value"),
            );
        }
    }
    pub(super) fn remove<V: Eq + Clone>(&mut self, problem: &ListProblem<V>, value: &V) {
        if let Some(members) = &mut self.members {
            members.remove(&problem.value_index(value).expect("validated removed value"));
        }
    }
    // Change, Swap and both sublist moves preserve the identity set. Only Assign
    // and Unassign update it; dropping a failed or rolled-back trial drops it too.
}
