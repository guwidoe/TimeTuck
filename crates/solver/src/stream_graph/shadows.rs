//! Typed shadow sessions inside graph-owned, independently restorable row caches.
use super::*;
use crate::{ShadowSession, ShadowSessionBuilder, ShadowSessionCheckpoint, ShadowSessionError};

type Projection<'a, 's, K, W, U> =
    dyn Fn(&W, &ShadowSession<'a, 's, K, W>) -> Result<U, ShadowSessionError> + 'a;

type Notifier<'a, 's, K, W> =
    dyn Fn(&W, &W, &mut ShadowSession<'a, 's, K, W>) -> Result<(), ShadowSessionError> + 'a;
struct State {
    session: ShadowSessionCheckpoint,
    // The erased input already owns an immutable world snapshot. Retaining its
    // Rc avoids cloning W and keeps checkpoints independent of callback state.
    input: Option<Event>,
}
struct Shadows<'a, 's, I, K, W, U> {
    session: ShadowSession<'a, 's, K, W>,
    initial: ShadowSessionCheckpoint,
    states: SnapshotMap<I, State>,
    notify: Option<Box<Notifier<'a, 's, K, W>>>,
    project: Box<Projection<'a, 's, K, W, U>>,
}
impl<I: Ord + Clone + 'static, K: Eq, W: 'static, U: 'static, S: AdditiveScore> Node<S>
    for Shadows<'_, '_, I, K, W, U>
{
    fn clear(&mut self) {
        self.states.clear();
        self.session.install(&self.initial);
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new(self.states.clone())
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        self.states = *saved.downcast().expect("shadow stream checkpoint type");
        // The session is scratch space. The next update installs the saved state
        // for its input identity before evaluating anything.
        self.session.install(&self.initial);
    }
    fn apply(&mut self, _: bool, event: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
        let change = typed::<I, W>(event);
        let mut states = self.states.clone();
        let output = if let Some(world) = &change.value {
            let previous = states.get(&change.id);
            self.session
                .install(previous.map_or(&self.initial, |s| &s.session));
            let before = previous.and_then(|s| s.input.as_ref()).map(|event| {
                typed::<I, W>(event)
                    .value
                    .as_ref()
                    .expect("retained shadow put")
            });
            if let Some((notify, before)) = self.notify.as_ref().zip(before) {
                notify(before, world, &mut self.session)
                    .map_err(|e| GraphError::Dependency(e.to_string()))?;
                self.session
                    .update_notified(world)
                    .map_err(|e| GraphError::Dependency(e.to_string()))?;
            } else {
                self.session
                    .update(world)
                    .map_err(|e| GraphError::Dependency(e.to_string()))?;
            }
            let value = (self.project)(world, &self.session)
                .map_err(|e| GraphError::Dependency(e.to_string()))?;
            states.insert(
                change.id.clone(),
                State {
                    session: self.session.capture(),
                    input: self.notify.as_ref().map(|_| event.clone()),
                },
            );
            TupleChange::put(change.id.clone(), value)
        } else if states.contains_key(&change.id) {
            states.remove(&change.id);
            TupleChange::retract(change.id.clone())
        } else {
            return Ok(Outputs::new());
        };
        let output = erase([output]);
        self.states = states;
        Ok(output)
    }
}

impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Evaluate typed shadow fields from complete immutable world snapshots.
    /// Each input identity owns a separate dependency cache; all share the given
    /// registration and supplier definitions. Sources may use explicit edges or
    /// compiled paths. Statically guaranteed source-path cycles are rejected
    /// before any callbacks; dynamic reference cycles remain runtime values.
    /// The projection reads the updated session and produces a
    /// normal scored/joinable tuple under the input identity.
    ///
    /// Candidate undo, saved winners and nested sessions restore graph-owned
    /// caches without readers, suppliers or projections. Retraction drops that
    /// identity's cache; reinsertion starts fresh. Diagnostic rebuilds start all
    /// caches fresh and replay current input snapshots.
    ///
    /// Each input row must contain the complete world needed by its session,
    /// including all registered roots. Immutable facts may be captured; mutable
    /// facts must be represented by registered input readers or compiled path
    /// signatures. Suppliers and projections must depend only on these inputs,
    /// not callback history. External callback side effects are not rolled back.
    /// Cached shadow values need not implement Clone. Input snapshots and output
    /// tuples follow the graph's ordinary Clone and immutable-value contracts.
    ///
    /// ```
    /// use timetuck::*;
    /// struct Model;
    /// struct Job;
    /// let schema = PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Job>(vec![
    ///     PlanningVariableDefinition::basic::<i64>("duration", false),
    ///     PlanningVariableDefinition::shadow::<i64>("end"),
    /// ])])?;
    /// let entity = schema.entity::<Job>().unwrap();
    /// let mut shadows = ShadowSessionBuilder::<usize, i64>::new(&schema);
    /// let duration = shadows.input(entity.variable("duration").unwrap(), 0, |w| Some(*w))?;
    /// let read = duration.clone();
    /// let end = shadows.derive(entity.variable("end").unwrap(), 0,
    ///     move |_, i| i.get(&read).unwrap().map(|v| v + 2))?;
    /// shadows.set_dependencies(&end, &[duration.erase()])?;
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// let source = b.source::<usize, i64>();
    /// let finish = b.map_shadows(source.port(), shadows,
    ///     move |_, session| Ok(*session.read(&end)?.value.unwrap()))?;
    /// b.score("end", &finish, |end| SimpleScore::new(-*end))?;
    /// let mut graph = b.build();
    /// let mut trial = graph.transaction();
    /// trial.apply(&source, TupleChange::put(0, 5))?;
    /// assert_eq!(trial.score()?, SimpleScore::new(-7));
    /// trial.commit()?;
    /// graph.assert_consistent()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn map_shadows<'s: 'a, I, K, W, U>(
        &mut self,
        input: &StreamPort<I, W>,
        builder: ShadowSessionBuilder<'a, 's, K, W>,
        project: impl Fn(&W, &ShadowSession<'a, 's, K, W>) -> Result<U, ShadowSessionError> + 'a,
    ) -> Result<StreamPort<I, U>, GraphError>
    where
        I: Ord + Clone + 'static,
        K: Eq + 'a,
        W: Clone + 'static,
        U: Clone + 'static,
    {
        self.register_shadows(input, builder, project, None)
    }

    /// Evaluate shadow fields using complete, explicit change notifications.
    /// The first insertion of each input ID fully initializes its session without
    /// calling `notify`. Later puts invoke `notify(previous, current, session)`
    /// before [`ShadowSession::update_notified`], including identical worlds.
    /// Every changed input, dynamic binding target and direct external-fact
    /// supplier must be notified; manual dependency edits are also supported.
    /// As with cached values, the initial registration and world bindings must
    /// reproduce the resulting topology during a fresh diagnostic rebuild.
    /// A changed source alone does not notify its consumers' bindings. Use
    /// [`Self::map_shadows`] when complete notifications are unavailable.
    ///
    /// Previous worlds are the actual last successfully published snapshot for
    /// that input ID, not the last world seen by shared callback captures. They
    /// are restored with session caches by transaction undo and saved candidates.
    /// Restoration and retraction invoke no notifier, reader or projection.
    /// Reinsertion and diagnostic rebuilds fully initialize fresh sessions.
    /// Notifier errors/panics poison the transaction, just like supplier failures;
    /// rollback restores previous worlds, notifications, topology and outputs.
    /// Callbacks must be deterministic; their external effects are not undone.
    ///
    /// This adapter retains the existing immutable input event per live ID;
    /// retaining it and taking checkpoints do not clone worlds. Notification
    /// discovery, graph propagation and projection may still visit the full
    /// problem. Unnotified input and
    /// binding callbacks are avoided; no overall constant-time guarantee is made.
    /// Session registration is fixed, as for `map_shadows`.
    ///
    /// ```
    /// use timetuck::*;
    /// struct Model; struct Job;
    /// let schema=PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Job>(vec![
    ///     PlanningVariableDefinition::basic::<i64>("cost",false),
    /// ])])?;
    /// let mut shadows=ShadowSessionBuilder::<usize,i64>::new(&schema);
    /// let cost=shadows.input(schema.entity::<Job>()?.variable("cost")?,0,|w|Some(*w))?;
    /// let dirty=cost.clone();
    /// let mut b=StreamGraphBuilder::<SimpleScore>::new();
    /// let source=b.source::<usize,i64>();
    /// let costs=b.map_shadows_notified(source.port(),shadows,
    ///     move |before,after,session| {
    ///         if before!=after {session.mark_dirty(&dirty)?;}
    ///         Ok(())
    ///     },move |_,session|Ok(*session.read(&cost)?.value.unwrap()))?;
    /// b.score("cost",&costs,|cost|SimpleScore::new(*cost))?;
    /// let mut graph=b.build();
    /// let mut tx=graph.transaction();
    /// tx.apply(&source,TupleChange::put(0,3))?;
    /// tx.apply(&source,TupleChange::put(0,7))?;
    /// assert_eq!(tx.score()?,SimpleScore::new(7));
    /// tx.commit()?;
    /// graph.assert_consistent()?;
    /// # Ok::<(),Box<dyn std::error::Error>>(())
    /// ```
    pub fn map_shadows_notified<'s: 'a, I, K, W, U>(
        &mut self,
        input: &StreamPort<I, W>,
        builder: ShadowSessionBuilder<'a, 's, K, W>,
        notify: impl Fn(&W, &W, &mut ShadowSession<'a, 's, K, W>) -> Result<(), ShadowSessionError> + 'a,
        project: impl Fn(&W, &ShadowSession<'a, 's, K, W>) -> Result<U, ShadowSessionError> + 'a,
    ) -> Result<StreamPort<I, U>, GraphError>
    where
        I: Ord + Clone + 'static,
        K: Eq + 'a,
        W: Clone + 'static,
        U: Clone + 'static,
    {
        self.register_shadows(input, builder, project, Some(Box::new(notify)))
    }

    fn register_shadows<'s: 'a, I, K, W, U>(
        &mut self,
        input: &StreamPort<I, W>,
        builder: ShadowSessionBuilder<'a, 's, K, W>,
        project: impl Fn(&W, &ShadowSession<'a, 's, K, W>) -> Result<U, ShadowSessionError> + 'a,
        notify: Option<Box<Notifier<'a, 's, K, W>>>,
    ) -> Result<StreamPort<I, U>, GraphError>
    where
        I: Ord + Clone + 'static,
        K: Eq + 'a,
        W: Clone + 'static,
        U: Clone + 'static,
    {
        self.validate(input)?;
        let session = builder
            .try_build()
            .map_err(|e| GraphError::Dependency(e.to_string()))?;
        let initial = session.capture();
        let output = self.push(Shadows {
            session,
            initial,
            states: SnapshotMap::<I, State>::new(),
            notify,
            project: Box::new(project),
        });
        self.connect(input.node, output.node, false);
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        PlanningEntityDefinition, PlanningSchema, PlanningVariableDefinition, SimpleScore,
    };
    use std::cell::{Cell, RefCell};
    struct Model;
    struct Task;
    #[test]
    fn notified_saved_candidate_restores_previous_world_and_dependencies_without_callbacks() {
        let schema =
            PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Task>(vec![
                PlanningVariableDefinition::basic::<i64>("value", false),
                PlanningVariableDefinition::shadow::<i64>("derived"),
            ])])
            .unwrap();
        let fields = schema.entity::<Task>().unwrap();
        let calls = &Cell::new(0usize);
        let events = &RefCell::new(vec![]);
        let mut shadows = ShadowSessionBuilder::<usize, (i64, i64, bool)>::new(&schema);
        let a = shadows
            .input(fields.variable("value").unwrap(), 0, move |w| {
                calls.set(calls.get() + 1);
                Some(w.0)
            })
            .unwrap();
        let b = shadows
            .input(fields.variable("value").unwrap(), 1, move |w| {
                calls.set(calls.get() + 1);
                Some(w.1)
            })
            .unwrap();
        let value = shadows
            .derive(fields.variable("derived").unwrap(), 0, move |_, values| {
                calls.set(calls.get() + 1);
                Some(
                    values
                        .for_field::<i64>(fields.variable("value").unwrap())
                        .unwrap()
                        .into_iter()
                        .flatten()
                        .sum::<i64>(),
                )
            })
            .unwrap();
        let left = a.clone();
        let right = b.clone();
        shadows
            .bind_dependencies(&value, move |w| {
                calls.set(calls.get() + 1);
                Ok(vec![if w.2 { right.erase() } else { left.erase() }])
            })
            .unwrap();
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let source = builder.source::<usize, (i64, i64, bool)>();
        let target = value.clone();
        let output = builder
            .map_shadows_notified(
                source.port(),
                shadows,
                move |before, after, s| {
                    events.borrow_mut().push((*before, *after));
                    if before.0 != after.0 {
                        s.mark_dirty(&a)?;
                    }
                    if before.1 != after.1 {
                        s.mark_dirty(&b)?;
                    }
                    if before.2 != after.2 {
                        s.mark_dirty(&target)?;
                    }
                    Ok(())
                },
                move |_, s| {
                    calls.set(calls.get() + 1);
                    Ok(*s.read(&value)?.value.unwrap())
                },
            )
            .unwrap();
        let dataset = builder.dataset(&output).unwrap();
        let mut graph = builder.build();
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(7, (1, 2, false)))
            .unwrap();
        tx.apply(&source, TupleChange::put(7, (3, 9, true)))
            .unwrap();
        let saved = tx.capture().unwrap();
        drop(tx);
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(7, (5, 6, false)))
            .unwrap();
        tx.apply(&source, TupleChange::put(8, (7, 8, false)))
            .unwrap();
        tx.commit().unwrap();
        let count = calls.get();
        let notifications = events.borrow().len();
        graph.install(saved);
        assert_eq!(calls.get(), count);
        assert_eq!(events.borrow().len(), notifications);
        assert_eq!(graph.dataset_rows(&dataset).unwrap()[&7], 9);
        assert_eq!(graph.source_rows(&source).unwrap().len(), 1);
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(7, (3, 11, true)))
            .unwrap();
        tx.commit().unwrap();
        assert_eq!(events.borrow().last(), Some(&((3, 9, true), (3, 11, true))));
        assert_eq!(graph.dataset_rows(&dataset).unwrap()[&7], 11);
        assert_eq!(calls.get(), count + 3); // One reader, one supplier, one projection; no binding.
        graph.assert_consistent().unwrap();
    }
}
