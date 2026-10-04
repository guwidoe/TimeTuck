//! Native round-robin basic-value construction schedule.
use crate::{Assignment, FiniteProblem, OptionalConstruction};

// Borrow distinct values in first model/domain encounter order. Only Eq is
// required; values do not need hashing, sorting or a second identity registry.
pub(crate) struct ValueConstructionQueue<'a, V> {
    values: Vec<&'a V>,
    cursor: usize,
    remaining: usize,
}
impl<'a, V: Eq + Clone> ValueConstructionQueue<'a, V> {
    pub(crate) fn new(problem: &'a FiniteProblem<V>) -> Self {
        let mut values = Vec::new();
        for variable in problem.variables() {
            for value in variable.domain() {
                if !values.contains(&value) {
                    values.push(value);
                }
            }
        }
        let remaining = values.len();
        Self {
            values,
            cursor: 0,
            remaining,
        }
    }
    // A full rotation without an installed assignment ends the queue. Skipped
    // values count toward that rotation but do not consume scored steps.
    pub(crate) fn next(
        &mut self,
        problem: &FiniteProblem<V>,
        assignment: &Assignment<V>,
        optional: OptionalConstruction,
    ) -> Option<(&'a V, Vec<usize>)> {
        while self.remaining > 0 {
            let value = self.values[self.cursor];
            self.cursor = (self.cursor + 1) % self.values.len();
            self.remaining -= 1;
            let indices: Vec<_> = problem
                .variables()
                .iter()
                .enumerate()
                .filter(|(i, v)| {
                    !v.is_pinned()
                        && assignment.values()[*i].is_none()
                        && (!v.allows_unassigned()
                            || optional == OptionalConstruction::NonWorsening)
                        && v.domain().contains(value)
                })
                .map(|(i, _)| i)
                .collect();
            if !indices.is_empty() {
                return Some((value, indices));
            }
        }
        None
    }
    pub(crate) fn installed(&mut self) {
        self.remaining = self.values.len();
    }
}
