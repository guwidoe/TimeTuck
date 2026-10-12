//! Once-only initial-fact capability delivery over the declared graph topology.
use super::*;
type InitialCapabilityCallback<'a> = (usize, Box<dyn Fn(bool) + 'a>);
#[derive(Default)]
pub(super) struct InitialActivityObservers<'a> {
    pub(super) edges: Vec<Vec<Edge>>,
    pub(super) callbacks: Vec<InitialCapabilityCallback<'a>>,
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Observe the producer's structural capability after the initial fact batch.
    /// Observers are passive: they do not enable score branches or run map/filter
    /// callbacks. Forwarding follows declared topology, including disabled edges.
    /// Completion is automatic when a native basic/list/mixed director binds its
    /// planning rows. Standalone graphs call finish_initial_facts after committing
    /// all initial sources and any initialization updates.
    pub fn after_initial_facts<I, T>(
        &mut self,
        port: &StreamPort<I, T>,
        callback: impl Fn(bool) + 'a,
    ) -> Result<(), GraphError> {
        self.validate(port)?;
        self.initial_activity
            .get_or_insert_with(|| Box::new(InitialActivityObservers::default()))
            .callbacks
            .push((port.node, Box::new(callback)));
        Ok(())
    }
}
impl<S: AdditiveScore> StreamGraph<'_, S> {
    /// Complete the initial-fact lifecycle and deliver each registered capability
    /// once in producer order. All source changes must already be committed.
    /// Capabilities use retained source facts and operator requirements; predicate
    /// rejection and key mismatch do not prove a branch can never produce.
    /// Returns the number delivered, or zero when absent/already complete.
    /// Later transactions, rollbacks and candidate installs do not repeat initial
    /// notifications. No observer metadata is allocated when none are registered.
    /// Initial topology/callback storage is released after delivery. A callback panic
    /// propagates; the batch remains consumed to avoid duplicates.
    pub fn finish_initial_facts(&mut self) -> usize {
        let Some(observers) = self.initial_activity.take() else {
            return 0;
        };
        let size = self.nodes.len();
        let mut seen = vec![false; size];
        let mut all = vec![true; size];
        let mut any = vec![false; size];
        let mut left = vec![false; size];
        let mut can = vec![false; size];
        for node in 0..size {
            can[node] = self.nodes[node].source_count().map_or_else(
                || match self.activity_rules[node] {
                    ActivityRule::All => seen[node] && all[node],
                    ActivityRule::Any => any[node],
                    ActivityRule::Left => left[node],
                },
                |count| count != 0,
            );
            for edge in &observers.edges[node] {
                seen[edge.node] = true;
                all[edge.node] &= can[node];
                any[edge.node] |= can[node];
                if !edge.right {
                    left[edge.node] |= can[node];
                }
            }
        }
        for (node, callback) in &observers.callbacks {
            callback(can[*node]);
        }
        observers.callbacks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SimpleScore;
    use std::cell::RefCell;
    #[test]
    fn candidate_install_does_not_revive_completed_initial_notifications() {
        let calls = RefCell::new(Vec::new());
        let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
        let source = b.source::<usize, i64>();
        b.score("sum", source.port(), |v| SimpleScore::new(*v))
            .unwrap();
        b.after_initial_facts(source.port(), |can| calls.borrow_mut().push(can))
            .unwrap();
        let mut g = b.build();
        let saved = {
            let mut tx = g.transaction();
            tx.apply(&source, TupleChange::put(0, 3)).unwrap();
            tx.capture().unwrap()
        };
        assert_eq!(g.finish_initial_facts(), 1);
        assert_eq!(*calls.borrow(), vec![false]);
        g.install(saved);
        assert_eq!(g.score(), SimpleScore::new(3));
        assert_eq!(g.finish_initial_facts(), 0);
        assert_eq!(*calls.borrow(), vec![false]);
        g.assert_consistent().unwrap();
    }
}
