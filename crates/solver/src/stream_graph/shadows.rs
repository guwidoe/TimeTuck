//! Typed shadow sessions inside graph-owned, independently restorable row caches.
use super::*;
use crate::{ShadowSession, ShadowSessionBuilder, ShadowSessionCheckpoint, ShadowSessionError};

type Projection<'a, 's, K, W, U> =
    dyn Fn(&W, &ShadowSession<'a, 's, K, W>) -> Result<U, ShadowSessionError> + 'a;

struct Shadows<'a, 's, I, K, W, U> {
    session: ShadowSession<'a, 's, K, W>,
    initial: ShadowSessionCheckpoint,
    states: SnapshotMap<I, ShadowSessionCheckpoint>,
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
            self.session
                .install(states.get(&change.id).unwrap_or(&self.initial));
            self.session
                .update(world)
                .map_err(|e| GraphError::Dependency(e.to_string()))?;
            let value = (self.project)(world, &self.session)
                .map_err(|e| GraphError::Dependency(e.to_string()))?;
            states.insert(change.id.clone(), self.session.capture());
            TupleChange::put(change.id.clone(), value)
        } else if states.contains_key(&change.id) {
            states.remove(&change.id);
            TupleChange::retract(change.id.clone())
        } else {
            return Ok(vec![]);
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
    /// compiled paths. The projection reads the updated session and produces a
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
        self.validate(input)?;
        let session = builder.build();
        let initial = session.capture();
        let output = self.push(Shadows {
            session,
            initial,
            states: SnapshotMap::<I, ShadowSessionCheckpoint>::new(),
            project: Box::new(project),
        });
        self.connect(input.node, output.node, false);
        Ok(output)
    }
}
