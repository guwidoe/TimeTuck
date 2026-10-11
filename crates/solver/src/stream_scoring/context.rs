//! Explicit stream-context graph rebuilding and ownership transfer.
use super::*;
impl<'p, 'a, V: Eq + Clone + 'static, S: AdditiveScore + 'static>
    StreamScoreDirector<'p, 'a, V, S>
{
    pub fn working_model(&self) -> &'p FiniteProblem<V> {
        self.problem
    }

    /// Manage phase work with a graph factory called only on mode changes.
    /// The factory receives the outgoing director so it can retain current
    /// external facts. It must produce an equivalent fresh graph with an empty
    /// planning source. Effective named weights and the assignment are carried
    /// automatically. Fact rows/topology are the factory's responsibility.
    /// All graph preparation and callbacks finish before assignment transfer.
    pub fn into_context<'f, F>(
        self,
        global: crate::EnvironmentMode,
        phases: Vec<crate::EnvironmentMode>,
        metrics: crate::ContextMetrics,
        mut graph_factory: F,
    ) -> crate::ScoringContextManager<Self, crate::ContextFactory<'f, Self>>
    where
        'p: 'f,
        'a: 'f,
        F: FnMut(
                &Self,
            )
                -> Result<(StreamGraph<'a, S>, PlanningStreamSource<V>), StreamScoringError>
            + 'f,
    {
        let factory: crate::ContextFactory<'f, Self> = Box::new(move |old, _| {
            let (mut graph, source) = graph_factory(old)?;
            if !graph.source_rows(&source)?.is_empty() {
                return Err(StreamScoringError::PlanningSourceNotEmpty);
            }
            let weights: crate::ConstraintWeightOverrides<S> = old
                .graph
                .constraint_weights()
                .map(|(name, weight)| (name.to_owned(), weight.clone()))
                .collect();
            graph.apply_weight_overrides(&weights)?;
            let mut transaction = graph.transaction();
            let mut updates = StreamBatch::new();
            for (index, value) in old.assignment.values().iter().enumerate() {
                updates.push(&source, TupleChange::put(index, (index, value.clone())))?;
            }
            transaction.bind_planning_rows(updates)?;
            let initial_constraint_evaluations = transaction.score_evaluations();
            transaction.commit()?;
            let assignment = std::mem::replace(&mut old.assignment, Assignment::new(Vec::new()));
            Ok(Self {
                problem: old.problem,
                assignment,
                graph,
                source,
                initial_constraint_evaluations,
                structural: old.structural,
            })
        });
        crate::ScoringContextManager::new(self, global, phases, metrics, factory)
    }
}
