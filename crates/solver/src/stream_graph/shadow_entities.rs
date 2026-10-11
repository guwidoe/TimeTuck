//! Stable entity tuples selected from per-world declarative shadow snapshots.
use super::*;
use crate::{
    ShadowEntityFilter, ShadowEntitySelection, ShadowSession, ShadowSessionBuilder,
    ShadowSessionCheckpoint, ShadowSessionError,
};
use std::collections::BTreeSet;
struct EntityState<K> {
    session: ShadowSessionCheckpoint,
    keys: BTreeSet<K>,
}
type Projection<'a, 's, K, W, U> =
    dyn Fn(&W, &K, &ShadowSession<'a, 's, K, W>) -> Result<U, ShadowSessionError> + 'a;
struct ShadowEntities<'a, 's, I, K, W, U> {
    session: ShadowSession<'a, 's, K, W>,
    initial: ShadowSessionCheckpoint,
    states: SnapshotMap<I, EntityState<K>>,
    filter: ShadowEntityFilter,
    mode: ShadowEntitySelection,
    project: Box<Projection<'a, 's, K, W, U>>,
}
impl<I: Ord + Clone + 'static, K: Ord + Clone + 'static, W: 'static, U: 'static, S: AdditiveScore>
    Node<S> for ShadowEntities<'_, '_, I, K, W, U>
{
    fn clear(&mut self) {
        self.states.clear();
        self.session.install(&self.initial);
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new(self.states.clone())
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        self.states = *saved.downcast().expect("shadow entity checkpoint");
        self.session.install(&self.initial);
    }
    fn apply(&mut self, _: bool, event: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
        let change = typed::<I, W>(event);
        let old = self.states.get(&change.id);
        let mut output = vec![];
        let next = if let Some(world) = &change.value {
            self.session
                .install(old.map_or(&self.initial, |s| &s.session));
            self.session
                .update(world)
                .map_err(|e| GraphError::Dependency(e.to_string()))?;
            let selected = self
                .session
                .selected_entities(&self.filter, self.mode)
                .map_err(|e| GraphError::Dependency(e.to_string()))?;
            let mut keys = BTreeSet::new();
            for key in selected {
                let value = (self.project)(world, key, &self.session)
                    .map_err(|e| GraphError::Dependency(e.to_string()))?;
                output.push(TupleChange::put((change.id.clone(), key.clone()), value));
                keys.insert(key.clone());
            }
            if let Some(old) = old {
                for key in old.keys.difference(&keys) {
                    output.push(TupleChange::retract((change.id.clone(), key.clone())));
                }
            }
            Some(EntityState {
                session: self.session.capture(),
                keys,
            })
        } else {
            if let Some(old) = old {
                for key in &old.keys {
                    output.push(TupleChange::retract((change.id.clone(), key.clone())));
                }
            }
            None
        };
        let output = erase(output);
        if let Some(next) = next {
            self.states.insert(change.id.clone(), next);
        } else {
            self.states.remove(&change.id);
        }
        Ok(output)
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Refresh a complete world and emit only assigned/consistent entity rows.
    /// Compile the filter with the supplied builder after all registrations. Each
    /// world ID retains an independent session cache. Output identity is (world
    /// ID, entity key), so one entity becoming ineligible never renumbers others.
    /// Ineligible entities retract; recovery reinserts them. Projection runs only
    /// for selected entities, in registration order, after the refresh completes.
    ///
    /// Refresh uses ordinary full polling. Every selected entity is projected on
    /// each world update; no sparse projection claim is made. Retraction, undo,
    /// saved winners, activity changes and diagnostic rebuilds share ordinary
    /// graph state semantics. Callback errors/panics poison the transaction and
    /// rollback restores cached sessions and selected keys without callbacks.
    ///
    /// ```
    /// use timetuck::*;
    /// struct Model; struct Job;
    /// let schema=PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Job>(vec![
    ///     PlanningVariableDefinition::basic::<i64>("value",true),
    /// ])])?;
    /// let entity=schema.entity::<Job>()?;
    /// let mut shadows=ShadowSessionBuilder::<usize,Option<i64>>::new(&schema);
    /// let value=shadows.input(entity.variable("value")?,7,|w|*w)?;
    /// let filter=shadows.entity_filter(entity,&[])?;
    /// let mut b=StreamGraphBuilder::<SimpleScore>::new();
    /// let source=b.source::<usize,Option<i64>>();
    /// let entities=b.map_shadow_entities(source.port(),shadows,filter,
    ///     ShadowEntitySelection::AssignedAndConsistent,
    ///     move |_,_,s|Ok(*s.read(&value)?.value.unwrap()))?;
    /// b.score("selected",&entities,|v|SimpleScore::new(*v))?;
    /// let mut graph=b.build();
    /// let mut tx=graph.transaction();
    /// tx.apply(&source,TupleChange::put(0,None))?;
    /// assert_eq!(tx.score()?,SimpleScore::ZERO);
    /// tx.apply(&source,TupleChange::put(0,Some(5)))?;
    /// assert_eq!(tx.score()?,SimpleScore::new(5));
    /// tx.commit()?;
    /// graph.assert_consistent()?;
    /// # Ok::<(),Box<dyn std::error::Error>>(())
    /// ```
    pub fn map_shadow_entities<'s: 'a, I, K, W, U>(
        &mut self,
        input: &StreamPort<I, W>,
        builder: ShadowSessionBuilder<'a, 's, K, W>,
        filter: ShadowEntityFilter,
        mode: ShadowEntitySelection,
        project: impl Fn(&W, &K, &ShadowSession<'a, 's, K, W>) -> Result<U, ShadowSessionError> + 'a,
    ) -> Result<StreamPort<(I, K), U>, GraphError>
    where
        I: Ord + Clone + 'static,
        K: Ord + Clone + 'static,
        W: Clone + 'static,
        U: Clone + 'static,
    {
        self.validate(input)?;
        let session = builder
            .try_build()
            .map_err(|e| GraphError::Dependency(e.to_string()))?;
        session
            .validate_entity_filter(&filter)
            .map_err(|e| GraphError::Dependency(e.to_string()))?;
        let initial = session.capture();
        let out = self.push(ShadowEntities {
            session,
            initial,
            states: SnapshotMap::<I, EntityState<K>>::new(),
            filter,
            mode,
            project: Box::new(project),
        });
        self.connect(input.node, out.node, false);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        PlanningEntityDefinition, PlanningSchema, PlanningVariableDefinition, SimpleScore,
    };
    use std::cell::Cell;
    #[test]
    fn saved_selection_restores_sessions_and_entity_keys_without_projections() {
        struct Model;
        struct Job;
        let schema = PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Job>(vec![
            PlanningVariableDefinition::basic::<i64>("value", true),
        ])])
        .unwrap();
        let entity = schema.entity::<Job>().unwrap();
        let calls = Cell::new(0);
        let mut s = ShadowSessionBuilder::<usize, Vec<Option<i64>>>::new(&schema);
        let first = s
            .input(entity.variable("value").unwrap(), 0, |w| w[0])
            .unwrap();
        let second = s
            .input(entity.variable("value").unwrap(), 1, |w| w[1])
            .unwrap();
        let filter = s.entity_filter(entity, &[]).unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, Vec<Option<i64>>>();
        let out = b
            .map_shadow_entities(
                source.port(),
                s,
                filter,
                ShadowEntitySelection::AssignedAndConsistent,
                |_, key, s| {
                    calls.set(calls.get() + 1);
                    Ok(*s
                        .read(if *key == 0 { &first } else { &second })?
                        .value
                        .unwrap())
                },
            )
            .unwrap();
        let data = b.dataset(&out).unwrap();
        let mut g = b.build();
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(9, vec![None, Some(3)]))
            .unwrap();
        let saved = tx.capture().unwrap();
        drop(tx);
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(9, vec![Some(8), None]))
            .unwrap();
        tx.commit().unwrap();
        let count = calls.get();
        g.install(saved);
        assert_eq!(calls.get(), count);
        assert_eq!(
            g.dataset_rows(&data)
                .unwrap()
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            [(9, 1)]
        );
        assert_eq!(g.dataset_rows(&data).unwrap()[&(9, 1)], 3);
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(9, vec![None, None]))
            .unwrap();
        tx.commit().unwrap();
        assert!(g.dataset_rows(&data).unwrap().is_empty());
        g.assert_consistent().unwrap();
    }
}
